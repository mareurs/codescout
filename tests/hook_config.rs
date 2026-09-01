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
