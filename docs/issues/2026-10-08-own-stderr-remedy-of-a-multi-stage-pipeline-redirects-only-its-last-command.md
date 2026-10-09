---
id: 8031bda216e96ddd
kind: bug
status: open
title: 'BUG: the own-stderr remedy of a buffer query tells a multi-stage pipeline to append a redirection that only reaches its last command'
tags:
- cluster/unclassified
- run_command
- progressive-disclosure
opened: 2026-10-08
owner: marius
related:
- docs/issues/archive/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md
severity: low
---

# BUG: the own-stderr remedy of a buffer query tells a multi-stage pipeline to append a redirection that only reaches its last command

## Summary

When a buffer query's own stderr is cut, the marker says: rerun the query with `2>&1 >/dev/null | cut -c1-4000`
appended. The 2026-10-08 hint fix gates this advice with `can_append_stage`, which excludes lists. A pipeline
is not a list, but `2>&1` applies to its LAST command only, so for a multi-stage pipeline the advice reads the
wrong stderr.

## Symptom (Effect)

`cat @X | tr -d '\n' >&2 | cat` gets the marker "rerun the query with `2>&1 >/dev/null | cut -c1-4000`
appended". Running that route returns 0 bytes of stdout and a stderr cut again (1,997 bytes).

## Reproduction

Reproduced by the final reviewer (`RunCommand::call_content`, 2026-10-08): the query above on a buffer `@X`
with a stderr stream above the budget. The base before the hint fix behaves the same.

## Environment

codescout on `experiments`. Linux.

## Root cause

`can_append_stage` asks two things of an appended stage: the query is one pipeline (`is_one_pipeline`) and the IL3 gate does not refuse the joined command. Neither asks whether the stage's `2>&1` reaches the stream that was cut. A redirection applies to the LAST command of a pipeline only, so for `a | b >&2 | c` the appended `2>&1 >/dev/null | cut` reads `c`'s stderr and leaves `b`'s on the terminal. A stage that itself ends in `>&2` is worse, even alone: `cut -c1-9000 @X >&2 2>&1 >/dev/null` leaves fd 1 on the old stderr (the redirections run left to right), and the pipe gets nothing.

## Evidence

The reviewer's run above.

## Hypotheses tried

None.

## Fix

Fixed in `065caed6`, `fix/run-command-hint-followups`, `src/tools/run_command/output.rs`. The choice between the two options the record named, made on executed evidence: WRAP.

Executed under the real `sh` (`the_own_stderr_route_reads_the_stream_that_was_cut_whatever_the_shape_of_the_pipeline`), the appended route read the wrong stream for 9 of 11 shapes: a stage that writes its own stderr in the first, middle or last position of a pipeline, a single command that redirects its stdout (`cut -c1-9000 @X >&2`), `! cmd >&2`. It was right for the 2 shapes with one command and no redirection of its own. The wrapped route `{ QUERY; } 2>&1 >/dev/null | cut -c1-4000` read exactly the stream that was cut, cut again, for all 11, and for 87 rows of the adversarial table (each as written and with `>&2` appended, 42 of which write to the stream the route reads). Refusing the remedy for every query with more than one stage would have left the appended form wrong for a single command that redirects its own stdout (`cut -c1-9000 @X >&2`), and would have needed a stage counter, one more parser over the same namespace. The wrap needs neither.

- `bound_buffer_stderr` now says: rerun the whole query as `{ QUERY; } 2>&1 >/dev/null | cut -c1-4000`. `can_wrap_query` offers it when the query is one pipeline (so the `; }` closes it and nothing else). It asks the IL-3 gate nothing (dropped in the review round, `385c6112`): the query already passed the gate when it ran, the wrap adds only `} 2>&1 >/dev/null | cut -c1-4000`, and `cut` is a field selector the gate never counts as a trimmer. Measured: the group also hides an unbounded producer from the gate's segment scan, so a verdict on the wrapped text would say nothing. Otherwise the marker says to narrow the query, as before.
- A brace group gives every stage the same stderr, so the wrapping needs no model of how many stages there are.

## Tests added

All in `src/tools/run_command/tests.rs` unless marked `output.rs`. Every route is EXECUTED and its bytes compared with the stream that was cut.

- `the_own_stderr_route_reads_the_stream_that_was_cut_whatever_the_shape_of_the_pipeline`: 11 shapes. For each it runs the OLD route and asserts whether it reads the right stream (so a shape that did not discriminate fails the test), runs the wrapped route under `sh`, and then follows the marker through `RunCommand::call_content`, building the route by the edit the marker states. The route's answer is claimed to come back inline only below the inline limit: the route holds `>/dev/null`, which makes it a query that is not buffer-only, so a larger answer is buffered under a handle of its own (the stream stays reachable through it).
- `the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts`: the 80-query adversarial table (now `adversarial_table()`, shared with the pager test), every one-pipeline row twice, 87 compared, 42 non-empty. It calls no production code but `is_one_pipeline`; the rest is `sh`. Since "both empty" holds for invalid `sh` too (`{ echo hi } 2>&1 >/dev/null | cut` prints a syntax error on the OUTER shell's stderr and nothing on stdout), it asserts the outer shell's stderr is empty for every route.
- `a_shape_the_wrap_cannot_take_gets_the_narrow_advice_not_a_route`: a list, `&&`, a comment, a heredoc. (`$(...)` is refused by the same predicate but no marker reaches it: such a query is not buffer-only, so its stderr is not cut.)
- `a_query_own_stderr_is_bounded_and_its_marker_does_not_name_a_stored_handle` now follows the marker's route instead of a hard-coded one.
- `output.rs`: `can_wrap_query_decides_whether_the_query_is_one_pipeline`.
- `check_adversarial_table` has a floor of 70 evaluated rows.

Mutation rounds (`./scripts/mutation-probe.sh`, one mutant at a time, the `tools::run_command` suite, 345 tests): KILLED: dropping the pipeline test from `can_wrap_query`, the marker not naming the group, the stage keeping stdout, the route missing its `;` (valid-sh check), the checker visiting no row. SURVIVED: deciding with `can_append_stage` instead of `can_wrap_query` in `bound_buffer_stderr`. Equivalent for every query that can reach the marker: the stage is `| cut`, which the gate never refuses, so the two differ only for a query the gate refused before it ran. No query was found that separates them (`cargo test | head -3` is refused both ways).

Known limits, not fixed here:
- The own-stderr remedy lives in the byte-cut branch of `bound_buffer_stderr` (`src/tools/run_command/output.rs:535`, the `elide_middle_escaped` call after `:542`). A stderr cut only by the 20-line cap carries `stderr_shown`/`stderr_total` and NO remedy: `cat @A >&2` over 3,000 lines gives `stderr_shown 20` / `stderr_total 3000` with no marker.
- The wrapped route holds `>/dev/null`, so it is not a buffer-only query, and an answer over the inline limit is buffered under a new handle (found by a reviewer with `cat @A >&2` over 3,000 lines). The stream stays reachable.

## Workarounds

Narrow the query so it prints less to stderr.

## Resume

`bound_buffer_stderr` and `can_append_stage` in `src/tools/run_command/output.rs`. Execute the chosen route in
a test.

## References

- `docs/issues/archive/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md`
