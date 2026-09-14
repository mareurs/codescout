//! Guard for `docs/issues/2026-09-14-read-file-and-grep-accept-a-err-handle-and-silently-answer-from-stdout.md`.
//!
//! ## What this guards, and why the helper alone would not
//!
//! A buffer handle may carry a `.err` suffix naming the stderr stream. `OutputBuffer::get`
//! resolves it — `id.strip_suffix(".err")` — and returns the whole `BufferEntry`, leaving
//! **stream selection** to the caller. That shape is deliberate: several consumers want
//! `exit_code`, `command` or `truncated` rather than a stream. Its cost is that selection
//! became every caller's job, and **they did not agree**.
//!
//! Enumerated 2026-09-14 — the population, not a sample. Five resolvers, four policies:
//! interpolation selects on the suffix; `run_command/output.rs` attaches `.stderr` as its own
//! field; `peer/server.rs` concatenates both; `read_file.rs` and `grep.rs` took `.stdout`
//! unconditionally and so answered from the wrong stream, silently, for any `.err` handle.
//!
//! `get_stream` fixes those two and makes the right path shorter. It does **not** make the
//! wrong path unavailable — `get()` still exists and must. So the helper is not the guard.
//! This file is (`CLAUDE.md` § *Observer Blindness*, third position).
//!
//! ## Why the existing test did not catch it
//!
//! `output_buffer::tests::stderr_suffix` asserts `get("{id}.err").stderr == "error msg"` — a
//! claim about the **resolver**, **monotone under the caller defect**: it passes whether or not
//! any caller selects the right stream. The suffix was green-and-broken for as long as both
//! callers existed.
//!
//! ## Reading a PASS here
//!
//! A clean corpus yields **zero** offenders, so this guard is silent exactly when it is
//! working — and would be equally silent with a broken matcher. Two things answer that, and
//! neither is "the assertion exists":
//!
//! - [`the_matcher_discriminates_the_defect_shape`] drives the predicate both ways on crafted
//!   inputs, so a matcher that stopped matching reds here.
//! - **An observed RED against the production path.** Reverting `read_file.rs`'s `get_stream`
//!   to `.get(path)…?.stdout` reds [`stream_selection_is_not_reinvented_per_caller`], run via
//!   `./scripts/mutation-probe.sh` and recorded in the bug file. A predicate test is an
//!   assertion about a re-implementation; only the mutation is about what ships.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

/// Files allowed to resolve a handle and choose a stream themselves.
///
/// Each entry states WHY, because an allowlist without reasons decays into a list of whatever
/// happened to exist when it was written.
const ALLOWED: &[(&str, &str)] = &[
    (
        "src/tools/output_buffer.rs",
        "defines get/get_stream; its interpolation path does the suffix selection itself, and \
         additionally pretty-prints @tool_* JSON on the stdout branch only",
    ),
    (
        "src/peer/server.rs",
        "deliberately CONCATENATES both streams — a peer reading a handle wants everything, \
         which is a third policy rather than a missing one",
    ),
    (
        "src/tools/run_command/output.rs",
        "reads `.stderr` deliberately, attaching it to a buffer-query envelope as its own field \
         rather than serving it as the result",
    ),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Tracked `.rs` files under `src/`, excluding dedicated test files.
fn tracked_src_files() -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "src"])
        .output()
        .unwrap_or_else(|e| {
            panic!("git ls-files failed to run — this gate needs a git checkout: {e:?}")
        });
    assert!(
        out.status.success(),
        "git ls-files exited {:?}",
        out.status.code()
    );
    let files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|p| p.ends_with(".rs") && !p.ends_with("/tests.rs"))
        .map(str::to_string)
        .collect();
    assert!(
        files.iter().any(|f| f == "src/tools/grep.rs"),
        "a known src file must be present; got {} files",
        files.len()
    );
    files
}

/// Does one Rust STATEMENT resolve a buffer handle and then pick a stream off it?
///
/// Statement-level, not line-level, and that is the whole reason this is a separate function:
/// the shipping defect was written as a four-line method chain
/// (`ctx.output_buffer\n.get(path)\n.ok_or_else(…)?\n.stdout`), which no single-line predicate
/// can see. Splitting on `;` joins the chain back up.
///
/// Requires all three of a buffer receiver, a `.get(` resolution, and a stream field — so a
/// `serde_json` map read like `val.get("buffer_truncated")` is not a hit, which it was in the
/// first draft of this gate.
fn statement_is_resolve_then_pick(stmt: &str) -> bool {
    let has_receiver = stmt.contains("output_buffer");
    let has_get = stmt.contains(".get(");
    let has_stream = stmt.contains(".stdout") || stmt.contains(".stderr");
    has_receiver && has_get && has_stream
}

/// Strip `#[cfg(test)]` module bodies — fixtures legitimately poke at both streams.
fn without_test_module(src: &str) -> &str {
    match src.find("\nmod tests") {
        Some(i) => &src[..i],
        None => src,
    }
}

fn offending_sites() -> Vec<(String, String)> {
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(f, _)| *f).collect();
    let mut hits = Vec::new();
    for rel in tracked_src_files() {
        if allowed.contains(rel.as_str()) {
            continue;
        }
        let src = std::fs::read_to_string(repo_root().join(&rel))
            .unwrap_or_else(|e| panic!("failed to read {rel}: {e}"));
        for stmt in without_test_module(&src).split(';') {
            if statement_is_resolve_then_pick(stmt) {
                let flat = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
                hits.push((rel.clone(), flat.chars().take(140).collect()));
            }
        }
    }
    hits
}

/// THE GUARD. A consumer of a buffer handle goes through `get_stream`, or says why not.
#[test]
fn stream_selection_is_not_reinvented_per_caller() {
    let offenders = offending_sites();
    let rendered: Vec<String> = offenders
        .iter()
        .map(|(f, s)| format!("  {f}\n      {s}"))
        .collect();

    assert!(
        offenders.is_empty(),
        "{} site(s) resolve a buffer handle and pick a stream themselves.\n\n{}\n\n\
         A handle may carry a `.err` suffix. `get()` resolves it and hands back BOTH streams, so \
         a site reaching for `.stdout` answers from the WRONG STREAM when the caller asked for \
         stderr — with no error, and for `grep` with a LINE NUMBER from the wrong stream, which \
         is a confident well-formed wrong citation rather than a miss anyone would notice.\n\n\
         Use `ctx.output_buffer.get_stream(path)`, which applies the suffix policy once. If this \
         site genuinely needs the whole entry or a different policy, add it to ALLOWED in \
         tests/buffer_stream_policy.rs WITH ITS REASON — the reason is the point; a bare \
         allowlist entry records only that someone wanted the build green.\n\n\
         BUG docs/issues/2026-09-14-read-file-and-grep-accept-a-err-handle-and-silently-answer-from-stdout.md",
        offenders.len(),
        rendered.join("\n")
    );
}

/// The matcher fires on the shape that shipped and stays quiet on the shapes that fooled it.
///
/// Both directions, because a predicate that always returns `false` passes the guard above on
/// any corpus and a predicate that always returns `true` is caught by the first commit.
#[test]
fn the_matcher_discriminates_the_defect_shape() {
    // The exact shape that shipped in read_file.rs and grep.rs, chain flattened by the split.
    assert!(statement_is_resolve_then_pick(
        "let raw = ctx\n.output_buffer\n.get(path)\n.ok_or_else(|| err())?\n.stdout"
    ));
    assert!(statement_is_resolve_then_pick(
        "let raw = ctx.output_buffer.get(raw_path).unwrap().stderr"
    ));

    // A serde_json map read — the first draft of this gate reported this as an offender.
    assert!(!statement_is_resolve_then_pick(
        "match val.get(\"buffer_truncated\").and_then(|v| v.as_array())"
    ));
    // Resolution with no stream pick: legitimate, wants exit_code or command.
    assert!(!statement_is_resolve_then_pick(
        "let entry = ctx.output_buffer.get(path)?"
    ));
    // A stream pick with no resolution: already holds the entry.
    assert!(!statement_is_resolve_then_pick("let s = entry.stdout"));
    // The repaired form must NOT be reported, or the fix cannot be adopted.
    assert!(!statement_is_resolve_then_pick(
        "let raw = ctx.output_buffer.get_stream(path).ok_or_else(|| err())?"
    ));
}

/// Every ALLOWED entry names a live file and carries a usable reason.
///
/// A stale row is a permanent exemption for a file nobody can find; an empty reason is an
/// exemption nobody can evaluate.
#[test]
fn every_allowlist_entry_is_live_and_justified() {
    for (file, reason) in ALLOWED {
        assert!(
            repo_root().join(file).exists(),
            "ALLOWED names {file}, which does not exist — stale exemption"
        );
        assert!(
            reason.len() > 40,
            "ALLOWED entry for {file} has no usable reason: {reason:?}"
        );
    }
}
