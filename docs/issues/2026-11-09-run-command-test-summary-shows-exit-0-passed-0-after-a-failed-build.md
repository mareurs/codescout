---
id: '762b5810f84398cf'
kind: bug
status: fixed
title: 'run_command: a filtered `cargo test` of a failed build is typed `test` with exit_code 0, passed 0 and an empty stdout'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-11-11
opened: 2026-11-09
severity: low-medium
unverified: Not checked on a rebuilt server. The study runs used the pinned commit f9b0fc9f8 and a likely linker failure; the repro here is a long compile_error line.
---

## Summary

When an agent runs `cargo test … | tail -N` through `run_command` and the build fails to link, the reply is `{"type":"test","exit_code":0,"passed":0,"stdout":"","unfiltered_output":"@cmd_…","unfiltered_output_lines":249,"producer_exit_code":101}`. Read at a glance this is a test run with zero passes and a clean exit. The failure is only in `producer_exit_code`, and the error text is in the unfiltered buffer. Seen in the Setup A/B study runs: 8 of the 28 cs/full runs in tasks cs-p01, cs-p04 (for example `cs-p01.claude-opus-5-5.cs.0`, stream line 221) and cs-p08 (line 398).

## Class fit

`exit_code` is correct in the frame of the filter (`tail` exited 0) but the field is named as if it described the command the agent typed. The class `IC-24` is a value that is correct in one frame and published under a name that states another. This one differs from the class's librarian-adjacent members: the wrong frame is a shell pipeline, and the second frame (`producer_exit_code`) is present but only after the reader has already concluded the run was clean.

## Symptom (Effect)

The agent spends an extra call (a `grep` over the `@cmd_*` buffer) to discover that the build failed. In 8 runs this was the only cost. A model that does not make that call would read `passed: 0, exit_code: 0` as "no tests ran, no error".

## Reproduction

Observed in transcripts, not minimised. The simple case `cargo test --manifest-path /nonexistent/Cargo.toml 2>&1 | tail -5` returns `exit_code: 0`, the error line in `stdout` and `producer_exit_code: 101` (checked 2026-11-09), so the field split is documented behaviour and works. The empty-stdout, `type: test` shape appears when the producer is a failed `cargo test` whose last lines carry no test summary: the summariser still types the reply as a test run and reports `passed: 0`. Reproduced 2026-11-12 outside the study transcripts: see § Hypotheses tried.

## Environment

Codescout at the pinned study commit f9b0fc9f8 (`/opt/sab/bin/codescout start --debug`), Claude Code 2.1.309, task images of the Setup A/B study.

## Root cause

Verified 2026-11-12. The buffered-summary arm of `handle_successful_output_with` (`src/tools/run_command/output.rs`) chose its summarizer from `detect_command_type(original_command)` alone. A piped `cargo test` matches `Test`. `summarize_test_output_within` counts libtest result lines. With none in the text, it reports `passed: 0` and leaves `stdout` out. `exit_code` is the filter's, so it is 0, and the failure sits only in `producer_exit_code`.

The arm is reached only when the filtered text is over the inline budget. A filtered text under the budget is returned raw and carries no `type`. That is why the simple cases below did not reproduce it. The first hypothesis (the summarizer is chosen from the command text and run over the filtered stream) is true in effect: the filtered stream is the only text it has.

## Evidence

`/home/marius/work/claude/setup-ab-data/study/runs/cs-p01.claude-opus-5-5.cs.0/stream.jsonl`, line 221 (the reply above) and line 222 (the follow-up `grep … @cmd_8fdf02e1`). Audit reports B.md and C.md under `/home/marius/.claude-kat/jobs/fbe38184/tmp/audit/` list the other runs.

## Hypotheses tried

Each run used a scratch crate with the real `cargo test`, on the running server, 2026-11-12.

- A compile error, `2>&1 | tail -5`: not reproduced. The reply is raw, with the errors in `stdout`.
- The same crate with no `2>&1`: not reproduced. The compiler text is in `stderr`.
- 40 compile errors (323 lines), `--lib -- f0 f1 2>&1 | tail -40`: not reproduced. The filtered text is under the inline budget.
- One `compile_error!` line of 20,000 bytes, `--lib -- t 2>&1 | tail -40`: reproduced. The reply was `{"type":"test","exit_code":0,"output_id":…,"passed":0,"stdout":"","unfiltered_output":…,"producer_exit_code":101}`, the shape in the study transcript. A linker failure prints very long lines, which is the likely route in the study runs. Not checked there.

## Fix

In the `Test` arm, when `producer_exit_code` is non-zero and the test summary states no verdict, the reply takes the generic summary: the head and tail of the text, bounded by bytes, with `output_id` and `producer_exit_code` as before. `states_a_test_verdict` (`src/tools/command_summary.rs`) is true when the summary counted a pass, counted a failure, or carries a failures section.

Unchanged: a run that states a verdict, and a producer that succeeded, are typed `test`. An unpiped `cargo test` with a failed build still answers `type: test`, `exit_code: 101`, `passed: 0`, with the compiler text in `stderr`. Its `exit_code` is the command's own, so it is not the frame mismatch this bug names.

## Fix provenance

- **SHA:** `5a8064845` (`experiments`)
- **patch-id:** `e7e3d149036ae352fa6ccdbf7e2b5739f6beaa0a`

## Tests added

- `a_failed_producer_with_no_test_verdict_is_not_typed_as_a_test_run` (`src/tools/run_command/tests.rs`): red before the change with exactly the reply above, green after. It also pins two controls: a run with a verdict stays `test`, and a producer that exited 0 stays `test`.
- `a_test_summary_states_a_verdict_only_when_it_counted_something` (`src/tools/command_summary.rs`): the predicate on an empty text, a compile error, a result line of zeros, and two counted results.

Gate on 2026-11-12: clippy, LEAN and DEFAULT green in one run. The format lane was red in that run because a peer's file was transiently unformatted. A separate `./scripts/fmt-mine.sh --check` was green afterwards.

## Workarounds

Run the producer bare and query the `@cmd_*` buffer, as the guide advises.

## Resume

Fixed and committed. One check is left: after a rebuild and `/mcp`, rerun the 20,000-byte `compile_error!` crate from § Hypotheses tried. The reply should be `type: generic` with the text in `stdout` and `producer_exit_code: 101`. Then clear the `unverified` field.

## References

Codescout memory `research/setup-ab-audit-cs-usage-20261109`.
