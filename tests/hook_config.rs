//! The `cargo-fmt` hook hardcodes a Rust edition; this fails if it stops matching the manifest.
//!
//! Why this exists. `.pre-commit-config.yaml` runs `rustfmt --edition 2021 --check` on the
//! STAGED files rather than `cargo fmt --check` on the workspace, because the whole-tree form
//! parsed all 408 `.rs` files to check the one you staged — ~2000 ms against ~40 ms — and that
//! duration is the window in which pre-commit's whole-tree diff attributes another session's
//! write to your commit. The scoping is the fix; the hardcoded edition is its cost.
//!
//! `cargo fmt` reads the edition from `Cargo.toml`. Standalone `rustfmt` does not: it defaults
//! to **2015**, so the flag is load-bearing rather than decorative — drop it and the hook
//! misparses modern syntax instead of failing cleanly. Bumping the workspace edition without
//! touching the hook would leave a gate that silently checks the wrong grammar, which is the
//! shape `tests/feature_lanes.rs` exists to catch one layer over: a manifest value that a
//! config file restates, with nothing holding the two together.
//!
//! Deliberately NOT a check that the hook is installed or that it passes — pre-commit owns
//! that, and a test asserting it would red on any checkout that has not run `pre-commit
//! install`. This asserts one thing: the two declarations agree.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `edition = "…"` in the workspace manifest.
///
/// Plural on purpose: the root package and `codescout-embed` each declare one, and a hook
/// carrying a single edition is only correct while they agree.
fn manifest_editions() -> Vec<String> {
    let text = std::fs::read_to_string(repo_root().join("Cargo.toml")).expect("read Cargo.toml");
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("edition"))
        .filter_map(|rest| {
            let rest = rest.trim_start().strip_prefix('=')?;
            let start = rest.find('"')? + 1;
            let end = rest[start..].find('"')? + start;
            Some(rest[start..end].to_owned())
        })
        .collect()
}

/// The edition the `cargo-fmt` hook passes to `rustfmt`.
fn hook_edition(config: &str) -> Option<String> {
    let entry = config
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("entry:") && l.contains("rustfmt"))?;
    let rest = entry.split("--edition").nth(1)?.trim_start();
    Some(
        rest.split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned(),
    )
}

/// Editions declared in the manifest that the hook does not pass. Empty means agreement.
///
/// Pure over both inputs so [`the_hook_edition_parser_discriminates`] can feed it a
/// disagreeing pair. A gate whose failing branch is only ever reached by editing a shared
/// config file is a branch nobody runs — and on this checkout, editing that file to test it
/// is a write four other sessions can commit.
fn mismatches(hook: &str, editions: &[String]) -> Vec<String> {
    editions
        .iter()
        .filter(|e| e.as_str() != hook)
        .cloned()
        .collect()
}

#[test]
fn the_fmt_hook_edition_matches_the_manifest() {
    let config = std::fs::read_to_string(repo_root().join(".pre-commit-config.yaml"))
        .expect("read .pre-commit-config.yaml");
    let hook = hook_edition(&config).expect(
        "the cargo-fmt hook must pass --edition to rustfmt: standalone rustfmt defaults to \
         2015 and would misparse this workspace rather than fail cleanly",
    );
    let editions = manifest_editions();
    assert!(
        !editions.is_empty(),
        "no `edition = \"…\"` found in Cargo.toml — this gate parsed nothing and would have \
         passed vacuously"
    );
    let bad = mismatches(&hook, &editions);
    assert!(
        bad.is_empty(),
        "the cargo-fmt hook passes --edition {hook} while Cargo.toml declares {bad:?}.\n\
         `cargo fmt` reads the edition from the manifest; standalone `rustfmt` does not, so the \
         hook would check a different grammar than the code is written in. Update `entry:` in \
         .pre-commit-config.yaml to match — this gate deliberately reddens on an edition bump \
         rather than following it, because the hook is a second declaration of the same fact."
    );
}

/// The parser and the comparison must each be able to return both answers, or the gate above
/// is decoration.
#[test]
fn the_hook_edition_parser_discriminates() {
    let with = "      - id: cargo-fmt\n        entry: rustfmt --edition 2018 --check\n";
    assert_eq!(hook_edition(with).as_deref(), Some("2018"));

    // No `--edition` at all is the case worth catching: the gate must see None, not a default.
    let without = "      - id: cargo-fmt\n        entry: rustfmt --check\n";
    assert_eq!(hook_edition(without), None);

    // A non-rustfmt entry must not be mistaken for the fmt hook.
    let other = "        entry: scripts/pre-commit-ledger-counts.py --edition 1999\n";
    assert_eq!(hook_edition(other), None);

    // The FAILING branch, exercised without editing a file four other sessions can commit.
    let editions = vec!["2021".to_string(), "2021".to_string()];
    assert!(mismatches("2021", &editions).is_empty());
    assert_eq!(mismatches("2018", &editions).len(), 2);
    assert_eq!(
        mismatches("2021", &["2021".to_string(), "2024".to_string()]),
        vec!["2024".to_string()],
        "a workspace whose members disagree must be reported, not averaged"
    );

    // Not `contains("2021")`: pinning the live value here would red on an edition bump for a
    // reason unrelated to what this file gates. Non-empty is the property that matters —
    // it is what stops the real gate passing vacuously.
    assert!(!manifest_editions().is_empty());
}
// ---------------------------------------------------------------------------
// IC-4 — config propagation is additive: a RENAMED path does not propagate
// ---------------------------------------------------------------------------

/// What a `core.hooksPath` setting resolves to.
#[derive(Debug, PartialEq, Eq)]
enum HooksPath {
    /// Not set. git uses `.git/hooks`, which is what `scripts/install-hooks.sh` writes.
    Unset,
    /// Set and the directory exists. A deliberate override; not this gate's business.
    PointsAtExisting(String),
    /// Set and the directory does NOT exist. Every hook in the repo is dead and git says nothing.
    PointsAtMissing(String),
}

/// Judge a `core.hooksPath` value against a directory-existence oracle.
///
/// Pure over both inputs so [`the_hooks_path_verdict_discriminates`] can exercise all three
/// arms without touching this machine's `.git/config` — which is shared mutable state that a
/// test has no business writing, and is itself an `OB-10` resource.
fn hooks_path_verdict(configured: Option<&str>, exists: impl Fn(&str) -> bool) -> HooksPath {
    match configured.map(str::trim).filter(|s| !s.is_empty()) {
        None => HooksPath::Unset,
        Some(p) if exists(p) => HooksPath::PointsAtExisting(p.to_owned()),
        Some(p) => HooksPath::PointsAtMissing(p.to_owned()),
    }
}

/// A `core.hooksPath` that names a missing directory silently disables every hook.
///
/// **The failure this catches is silence.** git does not warn when `core.hooksPath` points
/// nowhere; it simply runs no hooks. So `.pre-commit-config.yaml` stops firing, the ledger
/// guards stop refusing, and every commit succeeds — the additive half of a rename landing
/// while the removal does not, which is `IC-4`'s claim. Measured cost when it happened:
/// `docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md`, a day.
///
/// **Why a test and not a hook.** A hook cannot check whether hooks are wired — if the wiring
/// is broken the hook does not run, and its silence is indistinguishable from its approval.
/// This is the one class of check that must live outside the mechanism it guards.
///
/// **Why the trap and not installation.** `scripts/install-hooks.sh --check` already reports
/// far more (shims present, stage log, opt-in trailer) and exits 0 here — but nothing runs it:
/// no CI job, no test, no task runner reference, only a line of prose asking the operator to
/// run it after a clone. Asserting *installation* would red every fresh clone and every CI
/// checkout, which is why `tests/hook_config.rs` declines to. Asserting the *trap* reds nobody
/// legitimately: unset passes, a deliberate override passes, and only a stale path fails.
///
/// **Vacuous in CI, on purpose, and say so rather than bank it.** A fresh checkout never has
/// `core.hooksPath` set, so this passes trivially there and its green tick means nothing.
/// `.git/config` is machine-local and CI cannot host the defect. The party it protects is the
/// developer whose directory was renamed — who, per `IC-4`, verified the change they could see
/// and got positive evidence for the wrong proposition.
/// **The panic arm is unexercised on a healthy machine, so here is how to see it fire.** In a
/// throwaway repo — never here, since setting this key disables every session's hooks:
///
/// ```text
/// git init -q probe && cd probe
/// git config core.hooksPath /home/you/work/OLD-NAME/.git/hooks   # the archived bug's shape
/// printf '#!/bin/sh\nexit 1\n' > .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
/// git commit -qm x                                               # exit 0 — the REFUSING hook never ran
/// ```
///
/// Measured 2026-09-02: that commit succeeds. A hook whose only job is to refuse is skipped in
/// silence, which is the whole of the defect — not a weakened guard, an absent one.
#[test]
fn a_set_core_hookspath_must_point_at_a_directory_that_exists() {
    let out = Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(repo_root())
        .output()
        .expect("git config failed to run");
    // Exit 1 with no output is git's "not set", which is the healthy case rather than an error.
    let configured = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let root = repo_root();
    let verdict = hooks_path_verdict(Some(configured.as_str()), |p| {
        let path = std::path::Path::new(p);
        if path.is_absolute() {
            path.is_dir()
        } else {
            root.join(path).is_dir()
        }
    });

    if let HooksPath::PointsAtMissing(p) = verdict {
        panic!(
            "core.hooksPath is set to `{p}`, which is not a directory.\n\n\
             git overrides .git/hooks with it unconditionally and does NOT warn when it names \
             nothing, so EVERY hook in this repo is currently dead: pre-commit does not run, \
             the ledger-count and foreign-index guards do not refuse, and commits succeed \
             looking exactly as they should. The usual cause is a directory rename — the new \
             value propagated, the old absolute path did not.\n\n    \
             git config --unset core.hooksPath && scripts/install-hooks.sh\n\n\
             Verify with `git config --get core.hooksPath` returning nothing: an --unset that \
             was recorded but never run is how the original bug survived being marked fixed \
             (docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md)."
        );
    }
}

/// The verdict must be able to return each arm, or the gate above is decoration.
///
/// Note the live test can only ever observe one arm on a given machine, and on a healthy one
/// that arm is `Unset` — so without this, `a_set_core_hookspath_must_point_at_a_directory_that_exists`
/// is a test whose only exercised path is the one that cannot fail.
#[test]
fn the_hooks_path_verdict_discriminates() {
    let never = |_: &str| false;
    let always = |_: &str| true;

    assert_eq!(hooks_path_verdict(None, always), HooksPath::Unset);
    // git prints nothing when the key is unset; the empty string must read as unset, not as a
    // relative path that happens to resolve to the repo root.
    assert_eq!(hooks_path_verdict(Some(""), always), HooksPath::Unset);
    assert_eq!(hooks_path_verdict(Some("   "), always), HooksPath::Unset);

    assert_eq!(
        hooks_path_verdict(Some(".husky"), always),
        HooksPath::PointsAtExisting(".husky".into()),
        "a deliberate override that exists is not this gate's business"
    );
    assert_eq!(
        hooks_path_verdict(Some("/old/name/.git/hooks"), never),
        HooksPath::PointsAtMissing("/old/name/.git/hooks".into()),
        "the pre-rename absolute path is the exact shape of the archived bug"
    );
}
