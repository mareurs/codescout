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

// ───────────────────────── temp_env users must be serial ─────────────────────────
//
// `temp_env::with_vars*` IS an environment mutation — it is a scoped set/restore around
// the same process-global `environ` — and the scan above cannot see it, because the call
// it looks for lives inside the crate. So a `temp_env` user is the one sanctioned way left
// to mutate env from `src/`, and the only thing standing between it and every concurrent
// reader of the same variables is `#[serial]`.
//
// That is a NARROW protection and this scan claims no more than it: `serial` locks only
// against other `serial` tests (`docs/conventions/test-env-isolation.md`), so it cannot
// protect an untagged reader. What it does buy is the pair that matters in practice — a
// `temp_env` window and the `serial` tests that read the same variables as ground truth.
// `docs/issues/archive/2026-09-24-bare-model-status-test-flips-to-remote-http-under-the-full-lane.md`
// is the measured case: a bare `#[test]` unset window overlapped a `serial` reader's
// "ambient url is set, skip" guard, the ambient value was restored under it, and the
// reader reported `remote-http` in 20 of 30 runs. The scan makes that omission a red
// build rather than a flake that surfaces once a fortnight in the full lane.
//
// The needle is assembled for the reason the module docs give. Only `src/` is scanned:
// `tests/retrieval_unit.rs` has its own, argued isolation helper, and a separate test
// binary is a separate process.

/// The qualified path a `temp_env` call is spelled with, assembled — see above.
fn temp_env_needle() -> String {
    concat!("temp", "_env").to_string()
}

/// 0-indexed line numbers of every `temp_env` use that is NOT inside a function carrying a
/// `serial` attribute — pure over the source text so it can be tested on synthetic input.
///
/// - A comment line (first non-blank characters `//`) is skipped, so prose may name the crate.
/// - The owning function is the nearest preceding line that declares a `fn`; its attribute
///   block is the contiguous run of `#[..]` / comment lines directly above it (a doc comment
///   may sit between two attributes, so those are walked through, not stopped at).
/// - A `use` of the crate is itself a finding: `use temp_env::with_vars;` would let every
///   later call be spelled bare and evade the scan.
fn unserialised_temp_env_lines(src: &str) -> Vec<usize> {
    let needle = temp_env_needle();
    let lines: Vec<&str> = src.lines().collect();
    let is_fn_decl = |l: &str| {
        let t = l.trim_start();
        let t = t.strip_prefix("pub").map_or(t, |r| {
            // `pub`, `pub(crate)`, `pub(super)` …
            let r = r.trim_start();
            if r.starts_with('(') {
                r.split_once(')').map_or(r, |(_, rest)| rest).trim_start()
            } else {
                r
            }
        });
        let t = t.strip_prefix("async ").unwrap_or(t).trim_start();
        t.starts_with("fn ")
    };
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("//") || !line.contains(needle.as_str()) {
            continue;
        }
        if t.starts_with("use ") {
            out.push(i);
            continue;
        }
        let Some(fn_line) = (0..=i).rev().find(|&j| is_fn_decl(lines[j])) else {
            out.push(i);
            continue;
        };
        let mut serial = false;
        let mut j = fn_line;
        while j > 0 {
            j -= 1;
            let a = lines[j].trim_start();
            if a.starts_with("#[") {
                serial |= a.contains("serial");
            } else if !a.starts_with("//") {
                break;
            }
        }
        if !serial {
            out.push(i);
        }
    }
    out
}

/// Repo-relative path and 1-indexed line of every unserialised `temp_env` use under `src/`,
/// plus how many `temp_env` lines the walk saw at all (the non-vacuity count).
fn unserialised_temp_env_sites() -> (Vec<(String, usize)>, usize) {
    let root = repo_root();
    let needle = temp_env_needle();
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    files.sort();

    let (mut offenders, mut seen) = (Vec::new(), 0);
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        seen += text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//") && l.contains(needle.as_str()))
            .count();
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        offenders.extend(
            unserialised_temp_env_lines(&text)
                .into_iter()
                .map(|i| (rel.clone(), i + 1)),
        );
    }
    (offenders, seen)
}

#[test]
fn every_src_function_that_uses_temp_env_is_serial() {
    let (offenders, _) = unserialised_temp_env_sites();
    assert!(
        offenders.is_empty(),
        "a `temp_env` user without `#[serial_test::serial]`:\n{}\n\n\
         `temp_env` unsets or sets process-global variables for the length of its closure \
         and restores the ambient values afterwards. A bare `#[test]` doing that races \
         every `#[serial]` test that reads the same variables as ground truth — it can \
         pass such a reader's skip-guard inside the window and then hand it the restored \
         value (docs/issues/archive/2026-09-24-bare-model-status-test-flips-to-remote-http-under-the-full-lane.md). \
         Add `#[serial_test::serial]` (default group), or — better — stop mutating env: \
         docs/conventions/test-env-isolation.md option A. A `use` of the crate is also \
         refused, because it lets later calls hide from this scan: call it by its full path.",
        offenders
            .iter()
            .map(|(f, l)| format!("  {f}:{l}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Non-vacuity control for the scan above. It is an ABSENCE assertion, monotone under
/// removal: a walk that finds no files, a needle that stopped matching, or every user
/// having migrated off the crate would each pass it while looking at nothing. The floor is
/// the number of real call sites when this was written (three: one bare-`#[test]` user that
/// is now serial, two async ones in `tools::config::tests`); lower it deliberately, and say
/// why, when a user is genuinely removed.
#[test]
fn the_temp_env_scan_is_not_vacuous() {
    let (_, seen) = unserialised_temp_env_sites();
    assert!(
        seen >= 3,
        "the walk of src/ saw {seen} `temp_env` line(s), below the floor of 3 — the scan \
         above is probably blind (moved directory, changed spelling), not the tree clean"
    );
}

/// The checker's own negative control and positive twins, on synthetic source. The two tests
/// above run it over the real tree, where it currently finds nothing — so on their own they
/// cannot tell a checker that works from one that never matches. These can: each case below
/// is one the checker must get right, including the three ways a lazy implementation would
/// quietly pass a violation.
#[test]
fn the_temp_env_checker_flags_exactly_the_unserialised_users() {
    let call = format!(
        "    {}::with_vars_unset([\"X\"], || {{}});",
        temp_env_needle()
    );

    // Negative control: a bare #[test] user is flagged, on the line of the call.
    let bare = format!("#[test]\nfn t() {{\n{call}\n}}\n");
    assert_eq!(unserialised_temp_env_lines(&bare), vec![2]);

    // Positive twin: the same body with `serial` above the fn is clean.
    let serial = format!("#[test]\n#[serial_test::serial]\nfn t() {{\n{call}\n}}\n");
    assert!(unserialised_temp_env_lines(&serial).is_empty());

    // `serial` under a doc comment that sits between two attributes still counts…
    let doc_between = format!("#[test]\n/// why\n#[serial_test::serial]\nfn t() {{\n{call}\n}}\n");
    assert!(unserialised_temp_env_lines(&doc_between).is_empty());

    // …and so do the other spellings: a named group, an async test, a `pub(crate)` helper.
    let grouped = format!("#[serial(env)]\n#[tokio::test]\nasync fn t() {{\n{call}\n}}\n");
    assert!(unserialised_temp_env_lines(&grouped).is_empty());
    let helper = format!("pub(crate) fn h() {{\n{call}\n}}\n");
    assert_eq!(unserialised_temp_env_lines(&helper), vec![1]);

    // The attribute belongs to ITS function: a serial neighbour does not cover a bare one.
    let neighbours =
        format!("#[serial_test::serial]\nfn a() {{}}\n\n#[test]\nfn b() {{\n{call}\n}}\n");
    assert_eq!(unserialised_temp_env_lines(&neighbours), vec![5]);

    // A comment naming the crate is not a use.
    let prose = format!(
        "// {}::with_vars is mentioned here\nfn f() {{}}\n",
        temp_env_needle()
    );
    assert!(unserialised_temp_env_lines(&prose).is_empty());

    // An import is a finding on its own — it would let later calls evade the scan.
    let import = format!("use {}::with_vars;\n", temp_env_needle());
    assert_eq!(unserialised_temp_env_lines(&import), vec![0]);
}
