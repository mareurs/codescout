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
/// The shared commit-sequence tail exists and says something.
///
/// **Why this needs a gate at all.** All three refusing hooks read
/// `scripts/commit-sequence-tail.txt` **best-effort** — bash guards with `[ -r … ]`, Python
/// swallows `OSError` — because a missing tail must never turn a hook's own verdict into a
/// crash. That is the right failure mode and it is also a silent one: delete the file and
/// every refusal quietly reverts to its pre-2026-09-02 text, all three at once, with no
/// error anywhere and every test still green.
///
/// This is CLAUDE.md § *Testing Discipline*'s loudness law read from the other side: an
/// alarm nothing reaches is as informative as no alarm, and a best-effort read whose
/// absence nobody observes is a mechanism that can leave without being noticed. The
/// observer who would otherwise notice is a session mid-collision, which is the worst
/// possible moment to discover the guidance is gone.
///
/// **Asserts content, not existence.** An empty file is readable, so `exists()` alone
/// passes in the broken world — the same empty-string trap `heading_hint` carried at
/// `a35a9c35`. The two anchors below are the load-bearing halves: the opening phrase is
/// what makes the text recognisable as the shared tail rather than any other file, and the
/// pointer is the only route from a terse refusal to the reasoning. Either going is a
/// silent downgrade.
#[test]
fn the_shared_commit_sequence_tail_is_present_and_non_trivial() {
    let path = repo_root().join("scripts/commit-sequence-tail.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\n\nAll three refusing pre-commit hooks read this file \
             best-effort, so its absence is SILENT: every refusal reverts to its own rule \
             alone and nothing reports it. If the tail moved, move this gate with it — do \
             not delete it.",
            path.display()
        )
    });

    assert!(
        text.contains("This is one rule in a sequence"),
        "the tail must open with the phrase that makes it recognisable as the shared \
         sequence; found: {:?}",
        text.chars().take(80).collect::<String>()
    );
    assert!(
        text.contains("docs/conventions/shared-checkout-commit-sequence.md"),
        "the tail is the summary and that page is its source — without the pointer a \
         reader who just tripped a hook has no route to the reasoning"
    );
}

/// Every hook that refuses actually REACHES the tail — call site, not mention.
///
/// The test above proves the file has content; this proves something runs it. Together they
/// are the two halves the loudness law asks for — the emitter and the path to it — and
/// neither implies the other: a present file nothing reads is decoration, and a reader
/// pointed at an absent file is silence.
///
/// **This assertion is anchored on the CALL, and the first version was not.** It grepped
/// each script for `commit-sequence-tail.txt`, which survives in
/// `_emit_sequence_tail`'s own body after its call site is deleted — so the mutation
/// "stop calling it" left the test green. That is `IC-3`, declared-not-wired, occurring
/// inside the test written to prevent it: the helper was declared, nothing reached it, and
/// the check could not tell the difference. Measured 2026-09-02, and the reason the
/// anchors below are per-language rather than one shared substring.
///
/// Greps rather than executes because the refusal path is environment-shaped (a seeded
/// `session-stage-log`, a `next-index-*` `GIT_INDEX_FILE`, a `CLAUDE_CODE_SESSION_ID`), and
/// a test that cannot construct those would assert nothing. The end-to-end run was done
/// once by hand against `pre-commit-foreign-index.sh` — exit 1, refusal carrying both its
/// own rule and the tail — and is recorded in
/// `docs/plans/archive/2026-09-01-read-surface-fix-queue.md` § 3 rather than re-derived here.
#[test]
fn every_refusing_hook_emits_the_shared_tail() {
    // Each anchor is the line that RUNS the emission in that script's language, chosen so
    // that deleting the call — while leaving any helper, comment or path string in place —
    // fails this test.
    for (script, call_anchor) in [
        ("scripts/pre-commit-foreign-index.sh", "cat \"$_tail\""),
        ("scripts/pre-commit-unreviewed-content.sh", "cat \"$_tail\""),
        (
            "scripts/pre-commit-ledger-counts.py",
            "\n    _emit_sequence_tail()",
        ),
        // Added with the migration off the pre-commit framework. The retired `cargo-fmt`
        // entry ran `rustfmt` directly and could not emit the tail at all — there was no
        // script to put it in. Its replacement refuses commits, so it owes the same route
        // out, and this list is the only thing that notices if that call is dropped.
        ("scripts/pre-commit-cargo-fmt.sh", "cat \"$_tail\""),
    ] {
        let text = std::fs::read_to_string(repo_root().join(script))
            .unwrap_or_else(|e| panic!("cannot read {script}: {e}"));
        assert!(
            text.contains(call_anchor),
            "{script} refuses commits but never REACHES the shared sequence tail: no \
             {call_anchor:?} on any path. A hook that teaches only its own rule is how the \
             sequence came to be learned one collision at a time — nine cross-session \
             messages for one two-author commit, measured 2026-09-01. Note this asserts the \
             CALL, not a mention of the filename: the filename survives in a helper's body \
             after its call site is gone, which is how the first version of this gate \
             passed with the emission dead."
        );
    }
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

/// Editions declared in the manifest that the hook does not pass. Empty means agreement.
///
/// Pure over both inputs so [`the_script_edition_parser_discriminates`] can feed it a
/// disagreeing pair. A gate whose failing branch is only ever reached by editing a shared
/// config file is a branch nobody runs — and on this checkout, editing that file to test it
/// is a write four other sessions can commit.
///
/// That caller used to be `the_hook_edition_parser_discriminates`, deleted with the
/// pre-commit framework. The link was updated rather than dropped: this comment is the
/// only thing recording that the failing branch has a caller at all, and a doc link to a
/// removed item is exactly the stale citation `audit_doc_refs` exists to catch.
fn mismatches(hook: &str, editions: &[String]) -> Vec<String> {
    editions
        .iter()
        .filter(|e| e.as_str() != hook)
        .cloned()
        .collect()
}

/// The edition `scripts/pre-commit-cargo-fmt.sh` passes to `rustfmt`.
///
/// THE ONLY EDITION GATE, since the pre-commit framework was retired. It briefly had a
/// twin — `hook_edition`, which parsed the `entry:` line out of `.pre-commit-config.yaml`
/// — and that twin was deleted with the hook it gated rather than left passing. The file
/// it read still exists (retained so 33 backticked references stay resolvable, and marked
/// RETIRED in its own header), so those tests would have gone on being green about a hook
/// that no longer runs. A passing test over a retired surface is false coverage, which is
/// worse than none: it is what stops the next person looking.
///
/// The failing branch of `mismatches` moved into this parser's discriminator when that
/// twin was removed — see the comment there. Without it the real gate's
/// `assert!(bad.is_empty())` could not fail for a reason any test had observed.
fn script_edition(script: &str) -> Option<String> {
    let line = script
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("EDITION=") && l.contains("--edition"))?;
    let rest = line.split("--edition").nth(1)?.trim_start();
    Some(
        rest.split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches('"')
            .to_owned(),
    )
}

#[test]
fn the_fmt_script_edition_matches_the_manifest() {
    let script = std::fs::read_to_string(repo_root().join("scripts/pre-commit-cargo-fmt.sh"))
        .expect("read scripts/pre-commit-cargo-fmt.sh");
    let hook = script_edition(&script).expect(
        "the fmt script must pass --edition to rustfmt: standalone rustfmt defaults to 2015 \
         and would misparse this workspace rather than fail cleanly",
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
        "scripts/pre-commit-cargo-fmt.sh passes --edition {hook} while Cargo.toml declares \
         {bad:?}.\nThe script feeds rustfmt on STDIN, which reads no manifest at all, so this \
         is the only thing keeping the two in step. Update `EDITION=` in that script."
    );
}

/// The parser must return both answers, or the gate above is decoration.
#[test]
fn the_script_edition_parser_discriminates() {
    assert_eq!(
        script_edition("EDITION=\"--edition 2018\"\n").as_deref(),
        Some("2018")
    );

    // No `--edition` at all is the case worth catching: rustfmt would silently fall back
    // to 2015 rather than fail, so the gate must see None and not a default.
    assert_eq!(script_edition("EDITION=\"--check\"\n"), None);

    // PROSE THAT MENTIONS THE FLAG IS NOT A DECLARATION OF IT. This script's own header
    // contains the line "--edition IS NOT OPTIONAL: standalone rustfmt defaults to 2015",
    // so a parser keyed on the substring alone would read a sentence and report `IS`.
    assert_eq!(
        script_edition("# --edition IS NOT OPTIONAL: standalone rustfmt defaults to 2015\n"),
        None
    );
    assert_eq!(
        script_edition("    echo \"pass --edition 1999 to rustfmt\"\n"),
        None
    );

    // The real script must actually parse — otherwise the gate above passes vacuously on
    // its own `expect`, which is a panic and not a pass, but the discriminator is cheap.
    let script = std::fs::read_to_string(repo_root().join("scripts/pre-commit-cargo-fmt.sh"))
        .expect("read scripts/pre-commit-cargo-fmt.sh");
    assert!(
        script_edition(&script).is_some(),
        "the live script must expose an EDITION= line this parser can read"
    );

    // THE COMPARISON'S FAILING BRANCH, inherited from the retired YAML gate's
    // discriminator when that gate was deleted. It is exercised here rather than dropped
    // because nothing else reaches it: `mismatches` is only ever called with agreeing
    // inputs in the passing path, so without these three lines its non-empty return is
    // unreachable in the suite and the real gate's `assert!(bad.is_empty())` could not
    // fail for a reason any test had observed. Kept out of the live gate deliberately —
    // reaching that branch there would mean editing a shared file four sessions can commit.
    let editions = vec!["2021".to_string(), "2021".to_string()];
    assert!(mismatches("2021", &editions).is_empty());
    assert_eq!(mismatches("2018", &editions).len(), 2);
    assert_eq!(
        mismatches("2021", &["2021".to_string(), "2024".to_string()]),
        vec!["2024".to_string()],
        "a workspace whose members disagree must be reported, not averaged"
    );

    // Not `contains("2021")`: pinning the live value would red on an edition bump for a
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

// ---------------------------------------------------------------------------------------
// `.gitignore`: the nested-`.claude/` rule must not swallow the skills negation.
//
// WHY THIS IS GATED AND THE SIBLING `.gitignore` RULES ARE NOT.
// `goal-stop-hook.mjs:16` does `mkdirSync(join(cwd, '.claude'))`, so a session running from a
// subdirectory drops a log dir there. The anchored `/.claude/*` two lines up cannot match below
// the root, so those dirs showed up as untracked noise inside tracked doc directories
// (`docs/issues/2026-09-07-gitignore-claude-rule-is-root-anchored-so-nested-dirs-escape.md`).
//
// The obvious repair — append `**/.claude/` — is WRONG, and wrong in a way no other check here
// would catch. It matches at every depth INCLUDING the root, and git will not descend into an
// excluded directory to reconsider a negation, so it silently disables `!/.claude/skills/`.
// Measured 2026-09-07 in a scratch repo: `**/.claude/` appended reports the skill file IGNORED,
// and placing it FIRST does the same — reordering does not help, same descent reason.
//
// THE FAILURE IS DEFERRED AND SILENT, WHICH IS THE WHOLE REASON FOR A TEST.
// `.gitignore` never untracks: the two files already under `.claude/skills/` stay tracked, the
// working tree stays clean, and the gate stays green. What breaks is the NEXT skill added
// there — it never appears in `git status`, and whoever adds it gets no error. Nothing else in
// this repo asserts on that, so a future tidy-up that "simplifies" the scoped rule to a global
// one ships a hole with a green suite.
//
// The first assertion is the load-bearing one; it reds on exactly that change. The second reds
// on reverting the original fix. Read the pair as one guard: they are monotone in opposite
// directions, so either alone would miss the other's regression.

#[derive(Debug, PartialEq, Eq)]
enum IgnoreVerdict {
    Ignored,
    NotIgnored,
}

/// `git check-ignore -q` for one path, with the error case PANICKING rather than answering.
///
/// Exit 0 = ignored, 1 = not ignored, anything else = git itself failed. That third case is the
/// one worth spelling out: folding it into `NotIgnored` would make a broken invocation — wrong
/// cwd, no git, a corrupt index — read as "the negation survives", which is the exact answer
/// this test exists to trust. A guard whose error path produces its own pass condition is not a
/// guard.
fn ignore_verdict(rel: &str) -> IgnoreVerdict {
    let out = Command::new("git")
        .args(["check-ignore", "-q", "--", rel])
        .current_dir(repo_root())
        .output()
        .expect("git check-ignore must be runnable from the repo root");
    match out.status.code() {
        Some(0) => IgnoreVerdict::Ignored,
        Some(1) => IgnoreVerdict::NotIgnored,
        other => panic!(
            "git check-ignore exited {other:?} for {rel:?} — that is git failing, not a verdict. \
             stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}

/// The skills negation still works — reds if anyone widens the nested rule to `**/.claude/`.
///
/// **This probes a path that does NOT exist, and that is the whole design.** The first version
/// of this test asserted over `git ls-files -- .claude/skills/` and was VACUOUS: `git
/// check-ignore` consults the index, and for a **tracked** path it answers "not ignored"
/// whatever the patterns say. Measured 2026-09-07 with the broken `**/.claude/` rule in place —
/// tracked file, default flags: exit 1 (not ignored); same file with `--no-index`: exit 0,
/// blamed on `**/.claude/`. The index was masking the very defect the test existed for, so the
/// assertion had no input that could fail it and the mutation run passed.
///
/// The population was wrong too, not just the flag. The harm this guards is the **next** skill
/// added under `.claude/skills/` silently never appearing in `git status` — an *untracked* file.
/// Asserting over tracked ones tested the one set that cannot exhibit it. So the probe is a
/// path nobody has created: untracked by construction, and the same shape a real new skill
/// would have.
///
/// `--no-index` would also work here and is deliberately not used: it would make the test pass
/// for a reason the real `git status` does not share, and this guard is about what an author
/// sees, not about what the pattern matcher can be coaxed into reporting.
#[test]
fn the_claude_skills_negation_survives_the_nested_log_rule() {
    // Non-vacuity: the negation must actually be present, or the probe below is asserting that
    // an absent rule fails to match — true, and about nothing.
    let gitignore = std::fs::read_to_string(repo_root().join(".gitignore"))
        .expect(".gitignore must be readable");
    assert!(
        gitignore.lines().any(|l| l.trim() == "!/.claude/skills/"),
        "`!/.claude/skills/` is gone from .gitignore. Either it was deliberately retired — in \
         which case delete this test with it — or it was lost, which is the regression this \
         test exists to catch and it can no longer catch anything."
    );

    // A skill that does not exist yet. This is the input a tracked path cannot provide.
    let probe = ".claude/skills/a-skill-nobody-has-added-yet/SKILL.md";
    assert!(
        !repo_root().join(probe).exists(),
        "{probe} now exists, so it may be tracked and the index would mask the defect. Pick \
         another name that nobody has created."
    );

    assert_eq!(
        ignore_verdict(probe),
        IgnoreVerdict::NotIgnored,
        "a NEW file under .claude/skills/ would be IGNORED. `!/.claude/skills/` has been \
         defeated — almost certainly by a nested `.claude/` rule widened to `**/.claude/`, \
         which matches the root too and which git will not descend past to reconsider a \
         negation. Scope the rule to a subtree (`docs/**/.claude/`) instead. Note nothing \
         untracks and `git status` stays clean, so this is the only signal you get."
    );
}

/// The hook-created log dirs under `docs/` are ignored — reds if the scoped rule is dropped.
#[test]
fn hook_created_claude_dirs_under_docs_are_ignored() {
    for path in [
        "docs/superpowers/plans/.claude/codescout-companion.log",
        "docs/trackers/issue-clusters/.claude/codescout-companion.log",
    ] {
        assert_eq!(
            ignore_verdict(path),
            IgnoreVerdict::Ignored,
            "{path} is NOT ignored. The scoped `docs/**/.claude/` rule is missing or has been \
             re-anchored; `goal-stop-hook.mjs` creates these at whatever cwd a session holds, so \
             they return as untracked noise inside tracked doc directories."
        );
    }
}

/// The verdict helper discriminates — otherwise both tests above could be reading one constant.
#[test]
fn the_ignore_verdict_helper_discriminates() {
    assert_eq!(
        ignore_verdict("target/some-build-artifact"),
        IgnoreVerdict::Ignored,
        "`target/` is gitignored; a helper that cannot report Ignored makes \
         `hook_created_claude_dirs_under_docs_are_ignored` vacuous"
    );
    assert_eq!(
        ignore_verdict("Cargo.toml"),
        IgnoreVerdict::NotIgnored,
        "`Cargo.toml` is tracked and unignored; a helper that cannot report NotIgnored makes \
         `the_claude_skills_negation_survives_the_nested_log_rule` vacuous"
    );
}
