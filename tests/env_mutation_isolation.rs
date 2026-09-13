//! Nothing in this workspace may MUTATE the process environment outside a named allowlist.
//!
//! **Mutation only, and the name says so on purpose.** The sibling guard
//! `tests/embedder_env_isolation.rs` covers the other axis — a test that *reads* ambient
//! env and so asserts about the developer's shell — and its module docs argue that a guard
//! named wider than its trigger is itself a defect class here
//! (`cluster/guard-narrower-than-its-name`). That reasoning applies to this file too, which
//! is why it is not called `env_isolation`: reading is that file's subject, writing is this
//! one's, and neither covers the other. A reader who finds one should know the other exists.
//!
//! **Why a scan rather than a rule.** The rule already exists and is well argued:
//! `docs/conventions/test-env-isolation.md` § *The rule* says resolve config at the edge and
//! pass it inward, and marks the guard-plus-`#[serial]` alternative NOT VIABLE. It was
//! enforced once by a project-wide sweep that took occurrences from 119 to 0, and the doc
//! records that as done — "Nothing remains in this class."
//!
//! It did not stay done. The class has been reintroduced **twice** since that sentence was
//! written, both times by an author who could have quoted the rule:
//!
//! - `docs/issues/archive/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md`
//! - `docs/issues/archive/2026-09-13-an-env-var-set-across-an-await-races-every-sibling-test-that-reads-it.md`
//!   — whose offending site carried a comment arguing, in two clauses, that it was safe.
//!   Both clauses were false.
//!
//! That is the signature of a policy with no mechanism (`CLAUDE.md` § *Observer Blindness*,
//! third position): the party best placed to notice is the author, at the moment they are
//! writing the test, and a convention file is not in front of them then. A compiler error is.
//!
//! **What it cannot tell you.** That the remaining sites are *correct* — only that they are
//! the ones somebody wrote down. The allowlist is a record of decisions, not a proof, and
//! each entry carries its reason so the next reader can disagree with a specific claim
//! rather than with a bare path.
//!
//! **Why the pattern is assembled rather than written out.** A scanner looking for a token
//! cannot express that token literally without matching itself — the escape problem
//! `CLAUDE.md` § *Parsers Over a Namespace* describes. `concat!` keeps the contiguous
//! literal out of this file's bytes so the guard is subject to its own rule instead of
//! exempt from it.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Roots to scan, relative to the repo.
///
/// An explicit list rather than a walk from the repo root, for the reason
/// `tests/safety_comments.rs` gives: this checkout carries `.worktrees/` with full
/// checkouts of other branches, and a root walk would report findings in a copy of this
/// very file that the working tree cannot fix.
const ROOTS: [&str; 3] = ["src", "crates", "tests"];

/// Every file permitted to mutate the process environment, and why.
///
/// Adding an entry is a real decision. The first three are production startup paths that
/// run before any second thread exists; the fourth is the single test in the workspace
/// whose SUBJECT is ambient-environment behaviour, which cannot be tested without it.
const ALLOWED: &[(&str, &str)] = &[
    (
        "src/cli/mod.rs",
        "startup: seeds LIBRARIAN_CWD from --project, before any thread exists",
    ),
    (
        "src/config/global.rs",
        "startup: applies global-config env assignments in that same single-threaded window",
    ),
    (
        "src/agent/mod.rs",
        "the last EnvGuard; server-stack gated, and named as the one exemption in \
         docs/conventions/test-env-isolation.md",
    ),
    (
        "crates/codescout-embed/src/remote.rs",
        "from_url_ignores_an_ambient_env_api_key: to show from_url IGNORES EMBED_API_KEY the \
         variable has to be set while it runs, so the mutation is the subject rather than a \
         knob. Separate crate, so a separate test binary and process. Every other site here \
         was a knob and now passes a value to custom_with instead",
    ),
];

/// The two calls this file exists to find, assembled so this file does not contain either
/// as a contiguous literal — see the module docs.
fn needles() -> [String; 2] {
    [
        concat!("env::", "set_var", "(").to_string(),
        concat!("env::", "remove_var", "(").to_string(),
    ]
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `target` can appear under any crate root; skip build output wherever it is.
        if entry.file_name() == "target" {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Repo-relative path and 1-indexed line of every environment mutation.
///
/// Comment lines are skipped, which is what lets the convention doc, the bug files and
/// this module's own prose name the calls without becoming findings. A line is a comment
/// when its first non-whitespace characters are `//` — that covers `//`, `///` and `//!`.
fn mutation_sites() -> Vec<(String, usize)> {
    let root = repo_root();
    let needles = needles();
    let mut files = Vec::new();
    for r in ROOTS {
        rust_files(&root.join(r), &mut files);
    }
    files.sort();

    let mut out = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        for (i, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if needles.iter().any(|n| line.contains(n.as_str())) {
                out.push((rel.clone(), i + 1));
            }
        }
    }
    out
}

#[test]
fn no_site_outside_the_allowlist_mutates_the_process_environment() {
    let offenders: Vec<(String, usize)> = mutation_sites()
        .into_iter()
        .filter(|(f, _)| !ALLOWED.iter().any(|(a, _)| a == f))
        .collect();

    assert!(
        offenders.is_empty(),
        "environment mutation outside the allowlist:\n{}\n\n\
         `cargo test` runs one binary with a thread per core, so a test that mutates process \
         env mutates every concurrent sibling's input — and since Rust 2024 the call is \
         `unsafe` for exactly that reason. `#[serial]` is NOT the fix: it locks only against \
         tests that opt in, so any untagged reader still races, and the underlying UB (glibc \
         may realloc `environ` under a concurrent getenv) is untouched.\n\n\
         The remedy is docs/conventions/test-env-isolation.md option A: resolve the value at \
         the edge and take it as an argument. Worked examples in this tree — \
         `AttributionEnv::from_env` (src/tools/run_command/attribution.rs), \
         `BuildCheckEnv::from_env` (src/agent/build_check.rs), and \
         `RemoteEmbedder::custom_with` (crates/codescout-embed/src/remote.rs), each a `_with` \
         seam taking values beside a thin edge that reads env once.\n\n\
         If a site genuinely cannot be written that way, add it to ALLOWED in this file with \
         the reason — that is a decision someone can later disagree with, which a bare \
         `#[allow]` is not.",
        offenders
            .iter()
            .map(|(f, l)| format!("  {f}:{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The detector's own positive control, and the reason the test above is a measurement
/// rather than a green tick over nothing.
///
/// `no_site_outside_the_allowlist_...` is an ABSENCE assertion, so it is monotone under
/// removal: a walk that returns zero files satisfies it perfectly, and so does a needle
/// that stopped matching, a pruned directory, or a `read_dir` permission error swallowed by
/// the `let Ok(..) else`. Each of those produces a pass over zero coverage.
///
/// This asserts the scan still finds every site somebody wrote down. It fails if the walk
/// breaks, and it also fails when an allowlisted site is legitimately REMOVED — which is
/// the point: the entry has become a lie about the tree and should go.
#[test]
fn every_allowlisted_file_still_has_a_site() {
    let sites = mutation_sites();

    assert!(
        sites.len() >= ALLOWED.len(),
        "the scan found {} site(s) for {} allowlist entries — the detector is broken, not the \
         tree",
        sites.len(),
        ALLOWED.len()
    );

    let missing: Vec<&str> = ALLOWED
        .iter()
        .map(|(f, _)| *f)
        .filter(|f| !sites.iter().any(|(s, _)| s == f))
        .collect();

    assert!(
        missing.is_empty(),
        "allowlisted but no longer matching: {missing:?}\n\n\
         Either the scan stopped working — in which case the sibling test is passing over \
         nothing — or these sites were cleaned up, in which case delete their ALLOWED entries. \
         An allowlist entry for a site that no longer exists grants permission nobody is using \
         and hides a broken detector."
    );
}
