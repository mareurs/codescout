---
id: '3c8e811ae778d074'
kind: bug
status: open
title: 'BUG: the stderr tail marker says the buffer lacks the stderr that .err now serves'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
opened: 2026-09-24
related: []
severity: medium
---

# BUG: the stderr tail marker says the buffer lacks the stderr that `.err` now serves

## Summary

`summarize_stderr` (`src/tools/command_summary.rs`) ends every cut stderr tail in a `test` or
`build` envelope with *"Full stderr is NOT in the @cmd_* buffer — buffer reads return stdout
only."* That was true when written (`9c2b542f`, 2026-09-14). The `.err` handle suffix, which came
later and is documented in `get_guide("progressive-disclosure")` § *Reading the stderr of a buffered
command*, made it false. The marker therefore sends every reader away from the stream it describes.

A second, related defect lives in the same field: the 20-line / 2 KB tail budget is spent on cargo's
progress lines. A workspace `cargo test` ends in one `Running …` line per target, so the tail is
wholly `Running` lines and the warning above them is exactly the line that gets cut.

## Symptom (Effect)

Observed 2026-09-24 on a buffered `cargo test --workspace <filter>`:
`"stderr": "--- stderr TAIL: 37 earlier line(s) dropped; 12 of 49 line(s) shown. Full stderr is NOT in
the @cmd_* buffer — buffer reads return stdout only. ---\n     Running tests/rename_symbol.rs … (12 Running lines)"`.
In the same session, `read_file("@cmd_d1f0b8b1.err")` returned that kind of run's full stderr.

## Reproduction

Any buffered `cargo test` (stdout over ~10 KB) whose stderr exceeds 20 lines. The marker claims the
stream is absent; `read_file("<output_id>.err")` returns it.

## Environment

codescout `experiments` @ `36999188` plus uncommitted work, Linux.

## Root cause

Remedy text written against the buffer's behaviour at the time. The `.err` fix changed that
behaviour and did not touch the marker. Suites test a guard's predicate, never its remedy text (CLAUDE.md
§ Testing Discipline, *Loudness is a property of a PATH*), so nothing went red. The progress noise
is a separate cause: the tail is taken over every stderr line, and cargo's own status lines count
against the budget like any other.

## Evidence

The live response quoted above, and the live `.err` read, both in session
3b4fae98-500a-4fa9-8127-b16642a8c23d, 2026-09-24.

## Hypotheses tried

N/A.

## Fix

In the working tree, uncommitted as of 2026-09-24 (session 3b4fae98), pending the user's commit:

- `summarize_stderr` removes cargo progress lines (shared predicate
  `libtest_compact::is_cargo_progress_line`) before taking the tail, and announces
  `N cargo progress line(s) omitted`. A progress-only stderr is omitted like an empty one.
- The remedy becomes `Full stderr: read_file("<output_id>.err")`. `rebuild_buffered_summary` fills
  in the real handle, since it is the one place that holds the id.

Record the SHA and patch-id here once committed.

## Tests added

- `command_summary::tests::cargo_progress_does_not_spend_the_stderr_tail_budget`
- `command_summary::tests::a_stderr_of_only_cargo_progress_is_omitted`
- `run_command::tests::a_cut_stderr_tail_names_the_err_handle_that_holds_the_rest`: asserts the
  real handle is named, the false claim is gone, and the named handle really holds the stderr.

All three were observed red before the fix.

## Workarounds

Ignore the marker's claim and read `<output_id>.err`.

## Resume

Commit, record SHA and patch-id, then archive.

## References

- `docs/issues/archive/2026-09-14-every-reader-of-a-cmd-buffer-takes-stdout-only-so-the-stored-stderr-reaches-nobody.md` (the `.err` fix that made the marker false)
- `docs/trackers/context-injection-session-log.md` (quotes the marker as written)
