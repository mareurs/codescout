//! Compaction of a short libtest (`cargo test`) run for `run_command`'s inline path.
//!
//! A run below `needs_summary`'s ~10 KB threshold is returned inline, raw — and much of it
//! is cargo's own progress and targets that ran nothing: `Running …` lines, and
//! `running 0 tests` / `0 passed … N filtered out` blocks for every sibling binary a
//! filtered workspace run visits. Measured 2026-09-24 by replaying every recorded inline
//! `cargo test` response in `usage.db` (2026-08-25..09-24) through this function: 1,513
//! responses, 3.57 MB; 347 were compacted, 1.80 MB -> 0.58 MB, saving 1.22 MB. See
//! `docs/research/2026-09-24-rtk-evaluation.pdf` § 7 for why this is done here, not by rtk.
//!
//! **Filter by EXCLUSION.** A line is dropped only when it is positively recognised as
//! noise; everything else — failures, panics, warnings, a test's own `println!`, and any
//! shape nobody anticipated — passes through verbatim. The opposite design (keep what
//! looks important) is what rtk ships, and it is why rtk loses content: a line it does
//! not recognise disappears. Here an unrecognised line survives.
//!
//! **Test names are evidence, so they are grouped, never dropped.** CLAUDE.md § Testing
//! Discipline reads *which* tests ran out of this output; consecutive-or-not `ok` lines
//! sharing a module path collapse to `ok (N): path::{a, b}` with every name recoverable.
//!
//! The caller keeps the raw streams in an `@cmd_*` buffer and computes every diagnostic
//! (`empty_test_selection`, `partial_test_selection`, `wip_authors`) from the RAW output
//! before this runs, so compaction cannot silence them.

use regex::Regex;
use std::sync::OnceLock;

/// Below this many raw bytes (both streams) compaction is not attempted: the saving is a
/// few hundred bytes and the response would lose its byte-faithful `stdout` for it.
// cap-class: NOT_A_CAP — a gate on whether compaction runs, not a bound on output size
pub(crate) const MIN_RAW_BYTES: usize = 1024;

/// Compaction is used only when the result is at most this fraction (in percent) of the
/// raw size — i.e. it saves at least 30%. Otherwise the raw response is returned as today.
// cap-class: NOT_A_CAP — a gate on whether compaction runs, not a bound on output size
pub(crate) const MAX_KEPT_PERCENT: usize = 70;

/// What `compact_libtest_output` produced, and what it left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactedTest {
    pub stdout: String,
    pub stderr: String,
    /// Targets whose whole block was `running 0 tests` + `0 passed; 0 failed; 0 ignored;
    /// 0 measured; N filtered out`.
    pub empty_targets: usize,
    /// Sum of `N filtered out` across those dropped targets — kept so a reader still sees it.
    pub filtered_out: u64,
    /// Cargo progress lines dropped from stderr (`Compiling`, `Finished`, `Running`, …).
    pub progress_lines: usize,
}

impl CompactedTest {
    /// One line naming what was omitted and where the raw output is, for the end of `stdout`.
    ///
    /// Carries the dropped targets' `filtered out` sum so a reader still sees the number the
    /// empty-selection diagnostic is computed from, and a concrete call per stream
    /// (`docs/PROGRESSIVE_DISCOVERABILITY.md` Pattern 1).
    pub fn trailer(&self, output_id: &str) -> String {
        format!(
            "[codescout compacted this run: omitted {} target(s) that ran no tests \
             ({} filtered out) and {} cargo progress line(s). \
             Raw output: read_file(\"{output_id}\") and read_file(\"{output_id}.err\")]",
            self.empty_targets, self.filtered_out, self.progress_lines
        )
    }
}

/// Compact a libtest run's two streams, or `None` when compaction does not apply: no
/// libtest `test result:` line (a compile failure, pytest, …), fewer than
/// `MIN_RAW_BYTES`, or a result above `MAX_KEPT_PERCENT` of the raw size.
pub fn compact_libtest_output(stdout: &str, stderr: &str) -> Option<CompactedTest> {
    if !stdout.lines().any(|l| l.starts_with("test result: ")) {
        return None;
    }
    let raw = stdout.len() + stderr.len();
    if raw < MIN_RAW_BYTES {
        return None;
    }

    let (stdout_kept, empty_targets, filtered_out, stdout_progress) = compact_stdout(stdout);
    let (stderr_kept, stderr_progress) = compact_stderr(stderr);
    let c = CompactedTest {
        stdout: stdout_kept,
        stderr: stderr_kept,
        empty_targets,
        filtered_out,
        progress_lines: stdout_progress + stderr_progress,
    };

    // The trailer is shown too, so it is part of what the reader pays for. `@cmd_` ids are
    // eight hex digits, so this placeholder has the real length.
    let kept = c.stdout.len() + c.stderr.len() + c.trailer("@cmd_00000000").len();
    if kept * 100 > raw * MAX_KEPT_PERCENT {
        return None;
    }
    Some(c)
}

/// libtest's own hint, printed inside every failure block; the failure itself is kept.
const BACKTRACE_NOTE: &str =
    "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace";

/// Cargo status lines. Cargo right-aligns its verb to column 12, so each has at least one
/// leading space — which is what keeps a test's own `eprintln!("Running …")` (column 0)
/// from matching.
fn progress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^ +(Compiling|Finished|Running|Doc-tests|Blocking) \S")
            .expect("progress regex")
    })
}

/// Whether `line` is one of cargo's own status lines (`Compiling`, `Finished`, `Running`,
/// `Doc-tests`, `Blocking`). Shared with `command_summary`'s stderr tail so the inline and
/// buffered paths agree on what counts as noise.
pub(crate) fn is_cargo_progress_line(line: &str) -> bool {
    progress_re().is_match(line)
}

/// The summary line of a target that selected nothing and ran nothing.
fn empty_result_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^test result: ok\. 0 passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out",
        )
        .expect("empty result regex")
    })
}

/// A passing test whose name is a module path. A name with spaces (`x - should panic`,
/// a doc-test's `src/lib.rs - f (line 3)`) does not match and stays a verbatim line.
fn ok_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^test (\S+) \.\.\. ok$").expect("ok line regex"))
}

/// Drop empty target blocks, cargo progress (present here when the caller merged the
/// streams with `2>&1`), blank lines and the backtrace note; group passing tests. Returns
/// the kept text, the empty targets dropped, their `filtered out` sum, and progress lines.
fn compact_stdout(stdout: &str) -> (String, usize, u64, usize) {
    let lines: Vec<&str> = stdout.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let (mut empty_targets, mut filtered_out, mut progress_lines) = (0usize, 0u64, 0usize);

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line == "running 0 tests" {
            // An empty block is this line, blanks, then an all-zero result. Anything else
            // between them (a harness's own output) keeps the whole block verbatim.
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if let Some(caps) = lines.get(j).and_then(|l| empty_result_re().captures(l)) {
                empty_targets += 1;
                filtered_out += caps[1].parse::<u64>().unwrap_or(0);
                i = j + 1;
                continue;
            }
        }
        if progress_re().is_match(line) {
            progress_lines += 1;
        } else if !line.trim().is_empty() && line != BACKTRACE_NOTE {
            kept.push(line);
        }
        i += 1;
    }
    (
        group_passing(&kept).join("\n"),
        empty_targets,
        filtered_out,
        progress_lines,
    )
}

/// Group `test a::b::c ... ok` lines by parent path WITHIN each target's block, so a
/// module name shared by two binaries is never merged across them. The groups are emitted
/// where the block's first groupable line was; every other line keeps its position.
fn group_passing(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut segment: Vec<&str> = Vec::new();
    for &line in lines {
        if is_block_boundary(line) {
            out.extend(group_segment(&segment));
            segment.clear();
            out.push(line.to_string());
        } else {
            segment.push(line);
        }
    }
    out.extend(group_segment(&segment));
    out
}

/// `running N test(s)` opens a target's block and `test result:` closes it.
fn is_block_boundary(line: &str) -> bool {
    line.starts_with("test result: ")
        || line
            .strip_prefix("running ")
            .and_then(|rest| {
                rest.strip_suffix(" tests")
                    .or_else(|| rest.strip_suffix(" test"))
            })
            .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))
}

fn group_segment(segment: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut groups_at: Option<usize> = None;
    // (parent, [(name, original line)]) in order of first appearance.
    let mut groups: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();

    for &line in segment {
        let parsed = ok_line_re()
            .captures(line)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().rsplit_once("::"));
        let Some((parent, name)) = parsed else {
            out.push(line.to_string());
            continue;
        };
        groups_at.get_or_insert(out.len());
        match groups.iter_mut().find(|(p, _)| *p == parent) {
            Some((_, members)) => members.push((name, line)),
            None => groups.push((parent, vec![(name, line)])),
        }
    }

    if let Some(at) = groups_at {
        let rendered: Vec<String> = groups
            .into_iter()
            .map(|(parent, members)| {
                if members.len() == 1 {
                    members[0].1.to_string()
                } else {
                    let names: Vec<&str> = members.iter().map(|(n, _)| *n).collect();
                    format!("ok ({}): {parent}::{{{}}}", names.len(), names.join(", "))
                }
            })
            .collect();
        out.splice(at..at, rendered);
    }
    out
}

/// Drop cargo progress lines; collapse runs of blank lines and trim blank ends, so a
/// warning block keeps its separating blank line. Returns the kept text and the count.
fn compact_stderr(stderr: &str) -> (String, usize) {
    let mut kept: Vec<&str> = Vec::new();
    let mut progress_lines = 0usize;
    for line in stderr.lines() {
        if progress_re().is_match(line) {
            progress_lines += 1;
            continue;
        }
        let blank = line.trim().is_empty();
        if blank && kept.last().is_none_or(|l| l.trim().is_empty()) {
            continue;
        }
        kept.push(line);
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    (kept.join("\n"), progress_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A filtered WORKSPACE run: one target ran the match, five siblings ran nothing.
    // Load-bearing details, each named so a tidy-up cannot remove them silently:
    // - SEVEN `Running` lines against SIX stdout blocks: a real run showed 20 vs 19 (one
    //   binary printed nothing), which is why compaction must not pair them by position.
    // - `P50_DEBUG_TOTAL=11946` is a test's own eprintln between `Running` lines; it is the
    //   line an inclusion-based filter would lose, and it must survive.
    // - the five empty blocks' `filtered out` counts sum to 30 (0+4+9+15+2); the `0` one is
    //   a target with no tests at all, which is still an empty block.
    const WORKSPACE_STDERR: &str =
        "   Compiling codescout v0.15.0 (/home/marius/work/claude/codescout)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 13.81s
     Running unittests src/lib.rs (target/debug/deps/codescout-abe48e7f59a7161f)
P50_DEBUG_TOTAL=11946
     Running unittests src/main.rs (target/debug/deps/codescout-8a214943111ff1fb)
     Running tests/audit_doc_refs.rs (target/debug/deps/audit_doc_refs-eaa5462f2843ab20)
     Running tests/bug_regression.rs (target/debug/deps/bug_regression-403c50d5d427ce9f)
     Running tests/cli_artifact.rs (target/debug/deps/cli_artifact-d7fa82706169fa18)
     Running tests/link_scan.rs (target/debug/deps/link_scan-48c68e17171d75f4)
     Running tests/symbol_lsp.rs (target/debug/deps/symbol_lsp-4c27c61aaa67bd08)

";

    const WORKSPACE_STDOUT: &str = "
running 1 test
test server::guide_hint_tests::a_p50_session_stays_under_the_committed_guide_byte_ceiling ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4479 filtered out; finished in 0.09s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

";

    #[test]
    fn a_filtered_workspace_run_keeps_only_the_target_that_ran() {
        let c = compact_libtest_output(WORKSPACE_STDOUT, WORKSPACE_STDERR)
            .expect("a 1.9 KB workspace run that is 80% empty targets must compact");
        assert_eq!(
            c.stdout,
            "running 1 test\n\
             test server::guide_hint_tests::a_p50_session_stays_under_the_committed_guide_byte_ceiling ... ok\n\
             test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4479 filtered out; finished in 0.09s"
        );
        assert_eq!(c.stderr, "P50_DEBUG_TOTAL=11946");
    }

    #[test]
    fn what_was_dropped_is_counted_so_the_trailer_can_name_it() {
        let c = compact_libtest_output(WORKSPACE_STDOUT, WORKSPACE_STDERR).unwrap();
        assert_eq!(c.empty_targets, 5);
        assert_eq!(c.filtered_out, 30);
        // Compiling + Finished + seven Running.
        assert_eq!(c.progress_lines, 9);
    }

    #[test]
    fn the_trailer_names_the_omissions_and_a_concrete_call_for_the_raw_output() {
        let c = compact_libtest_output(WORKSPACE_STDOUT, WORKSPACE_STDERR).unwrap();
        assert_eq!(
            c.trailer("@cmd_abc123"),
            "[codescout compacted this run: omitted 5 target(s) that ran no tests \
             (30 filtered out) and 9 cargo progress line(s). \
             Raw output: read_file(\"@cmd_abc123\") and read_file(\"@cmd_abc123.err\")]"
        );
    }

    // Real bytes of a `cargo test --lib libtest_compact 2>&1` run from 2026-09-24 (deps path
    // shortened). `2>&1` is how most runs here are typed, and it moves cargo's progress
    // lines into STDOUT — load-bearing: a compactor that filters progress from stderr only
    // passes every other case and keeps all six of these.
    const MERGED_STREAMS_STDOUT: &str = "    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
   Compiling codescout v0.15.0 (/home/marius/work/claude/codescout)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 23.72s
     Running unittests src/lib.rs (target/debug/deps/codescout-8bfe3e1d659d82b2)

running 8 tests
test tools::libtest_compact::tests::a_run_under_the_size_gate_is_left_alone ... ok
test tools::libtest_compact::tests::a_run_with_no_libtest_summary_is_left_alone ... ok
test tools::libtest_compact::tests::a_run_that_would_barely_shrink_is_left_alone ... ok
test tools::libtest_compact::tests::passing_tests_group_by_module_path_and_every_name_survives ... ok
test tools::libtest_compact::tests::failures_warnings_and_unrecognised_lines_pass_through_verbatim ... ok
test tools::libtest_compact::tests::what_was_dropped_is_counted_so_the_trailer_can_name_it ... ok
test tools::libtest_compact::tests::a_filtered_workspace_run_keeps_only_the_target_that_ran ... ok
test tools::libtest_compact::tests::the_trailer_names_the_omissions_and_a_concrete_call_for_the_raw_output ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 5756 filtered out; finished in 0.00s

";

    #[test]
    fn progress_lines_are_dropped_from_stdout_when_the_streams_were_merged() {
        let c = compact_libtest_output(MERGED_STREAMS_STDOUT, "").unwrap();
        assert_eq!(
            c.stdout,
            "running 8 tests\n\
             ok (8): tools::libtest_compact::tests::{a_run_under_the_size_gate_is_left_alone, \
             a_run_with_no_libtest_summary_is_left_alone, a_run_that_would_barely_shrink_is_left_alone, \
             passing_tests_group_by_module_path_and_every_name_survives, \
             failures_warnings_and_unrecognised_lines_pass_through_verbatim, \
             what_was_dropped_is_counted_so_the_trailer_can_name_it, \
             a_filtered_workspace_run_keeps_only_the_target_that_ran, \
             the_trailer_names_the_omissions_and_a_concrete_call_for_the_raw_output}\n\
             test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 5756 filtered out; finished in 0.00s"
        );
        assert_eq!(c.progress_lines, 6);
    }

    // Padding: twelve `Running` lines make the input clear BOTH size gates, so this case
    // exercises grouping and nothing else refuses it first (CLAUDE.md § Testing Discipline,
    // "a case only exercises the guard it NAMES if every OTHER guard admits its input").
    const GROUPING_STDERR: &str = "     Running tests/a.rs (target/debug/deps/a-0000000000000001)
     Running tests/b.rs (target/debug/deps/b-0000000000000002)
     Running tests/c.rs (target/debug/deps/c-0000000000000003)
     Running tests/d.rs (target/debug/deps/d-0000000000000004)
     Running tests/e.rs (target/debug/deps/e-0000000000000005)
     Running tests/f.rs (target/debug/deps/f-0000000000000006)
     Running tests/g.rs (target/debug/deps/g-0000000000000007)
     Running tests/h.rs (target/debug/deps/h-0000000000000008)
     Running tests/i.rs (target/debug/deps/i-0000000000000009)
     Running tests/j.rs (target/debug/deps/j-0000000000000010)
     Running tests/k.rs (target/debug/deps/k-0000000000000011)
     Running tests/l.rs (target/debug/deps/l-0000000000000012)
";

    // Interleaved on purpose: libtest prints `ok` lines in COMPLETION order, so two modules'
    // tests alternate. `top_level_case` has no `::` and is alone in its group, so it must
    // stay a verbatim line rather than become `ok (1): {top_level_case}`.
    const GROUPING_STDOUT: &str = "
running 5 tests
test librarian::doctor::tests::alpha ... ok
test tools::grep::tests::beta ... ok
test librarian::doctor::tests::gamma ... ok
test top_level_case ... ok
test tools::grep::tests::delta ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.01s
";

    #[test]
    fn passing_tests_group_by_module_path_and_every_name_survives() {
        let c = compact_libtest_output(GROUPING_STDOUT, GROUPING_STDERR).unwrap();
        assert_eq!(
            c.stdout,
            "running 5 tests\n\
             ok (2): librarian::doctor::tests::{alpha, gamma}\n\
             ok (2): tools::grep::tests::{beta, delta}\n\
             test top_level_case ... ok\n\
             test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 12 filtered out; finished in 0.01s"
        );
    }

    // Two targets that both have a `tests` module — the ordinary shape of a workspace run.
    // Load-bearing: the SAME parent path in two blocks. Grouping across the whole stdout
    // would merge them into one `ok (4): tests::{…}` and lose which target ran which test.
    // Padded with `Running` lines so both size gates admit it.
    #[test]
    fn passing_tests_are_never_grouped_across_two_targets() {
        let stdout = "
running 2 tests
test tests::first_target_a ... ok
test tests::first_target_b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 2 tests
test tests::second_target_a ... ok
test tests::second_target_b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
        let c = compact_libtest_output(stdout, GROUPING_STDERR).unwrap();
        assert_eq!(
            c.stdout,
            "running 2 tests\n\
             ok (2): tests::{first_target_a, first_target_b}\n\
             test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\
             running 2 tests\n\
             ok (2): tests::{second_target_a, second_target_b}\n\
             test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s"
        );
    }

    // Real libtest/rustc bytes from a fixture crate run on 2026-09-24, padded with ten
    // `Running` lines (a workspace run's shape) so both size gates admit it.
    // Load-bearing: `panics_ok - should panic ... ok` has spaces in the name part and must
    // NOT be grouped; the RUST_BACKTRACE note is the only failure-block line dropped; the
    // `error: test failed, to rerun pass` line is what names the failing target once the
    // `Running` lines are gone, so it must survive.
    const FAILING_STDERR: &str = "warning: variable does not need to be mutable
 --> src/lib.rs:2:34
  |
2 | pub fn unused_mut() -> i32 { let mut x = 1; x }
  |                                  ----^
  |                                  |
  |                                  help: remove this `mut`
  |
  = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: `fx` (lib) generated 1 warning (run `cargo fix --lib -p fx` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.01s
     Running unittests src/lib.rs (target/debug/deps/fx-4a99a53287a4b62f)
     Running tests/a.rs (target/debug/deps/a-0000000000000001)
     Running tests/b.rs (target/debug/deps/b-0000000000000002)
     Running tests/c.rs (target/debug/deps/c-0000000000000003)
     Running tests/d.rs (target/debug/deps/d-0000000000000004)
     Running tests/e.rs (target/debug/deps/e-0000000000000005)
     Running tests/f.rs (target/debug/deps/f-0000000000000006)
     Running tests/g.rs (target/debug/deps/g-0000000000000007)
     Running tests/h.rs (target/debug/deps/h-0000000000000008)
     Running tests/i.rs (target/debug/deps/i-0000000000000009)
error: test failed, to rerun pass `--lib`
";

    const FAILING_STDOUT: &str = "
running 5 tests
test tests::ignored_one ... ignored
test tests::fails_with_context ... FAILED
test tests::panics_ok - should panic ... ok
test tests::passes_1 ... ok
test tests::passes_2 ... ok

failures:

---- tests::fails_with_context stdout ----

thread 'tests::fails_with_context' (2328521) panicked at src/lib.rs:10:9:
assertion `left == right` failed: add(2,2) should be 5 per the spec in docs/x.md
  left: 4
 right: 5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    tests::fails_with_context

test result: FAILED. 3 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

";

    #[test]
    fn failures_warnings_and_unrecognised_lines_pass_through_verbatim() {
        let c = compact_libtest_output(FAILING_STDOUT, FAILING_STDERR).unwrap();
        assert_eq!(
            c.stdout,
            "running 5 tests\n\
             test tests::ignored_one ... ignored\n\
             test tests::fails_with_context ... FAILED\n\
             test tests::panics_ok - should panic ... ok\n\
             ok (2): tests::{passes_1, passes_2}\n\
             failures:\n\
             ---- tests::fails_with_context stdout ----\n\
             thread 'tests::fails_with_context' (2328521) panicked at src/lib.rs:10:9:\n\
             assertion `left == right` failed: add(2,2) should be 5 per the spec in docs/x.md\n  \
             left: 4\n \
             right: 5\n\
             failures:\n    \
             tests::fails_with_context\n\
             test result: FAILED. 3 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s"
        );
        assert_eq!(
            c.stderr,
            "warning: variable does not need to be mutable\n \
             --> src/lib.rs:2:34\n  \
             |\n\
             2 | pub fn unused_mut() -> i32 { let mut x = 1; x }\n  \
             |                                  ----^\n  \
             |                                  |\n  \
             |                                  help: remove this `mut`\n  \
             |\n  \
             = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default\n\
             \n\
             warning: `fx` (lib) generated 1 warning (run `cargo fix --lib -p fx` to apply 1 suggestion)\n\
             error: test failed, to rerun pass `--lib`"
        );
    }

    // --- the three refusal gates, each fed an input the OTHER two admit ---

    // Compressible (thirty `Compiling` lines) and over 1 KB, but a compile failure prints
    // no `test result:` line — this is exactly the run whose every line a reader needs.
    #[test]
    fn a_run_with_no_libtest_summary_is_left_alone() {
        let mut stderr = String::new();
        for i in 0..30 {
            stderr.push_str(&format!("   Compiling dep{i:02} v0.1.0\n"));
        }
        stderr.push_str(
            "error[E0425]: cannot find value `x` in this scope\n --> src/lib.rs:3:5\n  |\n3 |     x\n  |     ^ not found in this scope\n\nerror: could not compile `fx` (lib test) due to 1 previous error\n",
        );
        stderr.push_str(
            &"     Running tests/pad.rs (target/debug/deps/pad-0000000000000001)\n".repeat(6),
        );
        assert!(
            stderr.len() >= MIN_RAW_BYTES,
            "fixture must clear the size gate"
        );
        assert_eq!(compact_libtest_output("", &stderr), None);
    }

    // Has a libtest summary and would shrink ~75%, but is under 1 KB.
    #[test]
    fn a_run_under_the_size_gate_is_left_alone() {
        let stderr = "     Running tests/a.rs (target/debug/deps/a-0000000000000001)
     Running tests/b.rs (target/debug/deps/b-0000000000000002)
     Running tests/c.rs (target/debug/deps/c-0000000000000003)
";
        let stdout = "
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s


running 1 test
test a::b ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
        assert!(
            stdout.len() + stderr.len() < MIN_RAW_BYTES,
            "fixture must be under the gate"
        );
        assert_eq!(compact_libtest_output(stdout, stderr), None);
    }

    // Has a libtest summary and is over 1 KB, but is one long assertion payload: nothing
    // here is noise, so the saving is far under 30%.
    #[test]
    fn a_run_that_would_barely_shrink_is_left_alone() {
        let payload = "x".repeat(1200);
        let stdout = format!(
            "running 1 test\ntest a::b ... FAILED\nfailures:\n---- a::b stdout ----\n\
             assertion `left == right` failed: {payload}\n\
             test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        );
        assert!(
            stdout.len() >= MIN_RAW_BYTES,
            "fixture must clear the size gate"
        );
        assert_eq!(compact_libtest_output(&stdout, ""), None);
    }
}
