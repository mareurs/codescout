---
kind: bug
status: fixed
tags:
- cluster/unclassified
- run_command
- progressive-disclosure
closed: 2026-10-05
opened: 2026-10-05
owner: marius
related:
- docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md
- docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md
severity: medium
unverified: Measured with synthetic 60 KB lines only; whether real failing cargo test runs hit this was not measured. The 5,000-byte budget assumes JSON escaping inflates text by under about 30%; not measured on real failure output. The live MCP binary that ran the probes predates the fix. cluster/unclassified was used because the field had no cap at all, which does not fit IC-13; the owner should reclassify.
---

# BUG: `run_command`'s `test` envelope carries the `failures` field with no bound, so one wide failure re-buffers the whole response under a content-free `@tool_*` envelope

## Summary

`summarize_test_output` puts a `failures` field in its envelope, and `extract_test_failures` fills it with the whole failure section of the output. Nothing bounds that field by bytes or by lines. When the field is large, the envelope is over the inline budget, `call_content` buffers it a second time under `@tool_*`, and the caller receives only `✓ exit 0 · 0 passed  (query @cmd_…)` plus a `json_path="$.field"` hint. The failure text, which is the reason to read a red run, is behind a handle whose hint cannot reach it.

This is the same defect class that was fixed for `summarize_generic` in `2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md`, and for the `stderr` field of this same envelope in the archived `2026-09-14` bug. Those fixes bounded other fields. This one was not bounded.

## Symptom (Effect)

Measured 2026-10-05 on the live MCP binary, which was built before the generic-summary fix:

```
run_command("printf 'failures:\\n%s\\nfailures:\\n' \"$(head -c 60000 /dev/zero | tr '\\0' x)\"; echo cargo test")
→ {"output_id": "@tool_0b9aeaff",
   "summary": "✓ exit 0 · 0 passed  (query @cmd_0b9aeafe)",
   "hint": "read_file(\"@tool_0b9aeaff\", json_path=\"$.field\") …",
   "buffered_bytes": 60116}
```

The command string ends in `echo cargo test` only so that `detect_command_type` classifies the run as `test`.

## Reproduction

The command above. It needs no real test runner.

## Environment

Linux, codescout MCP server used from Claude Code on 2026-10-05. Source read at `e37c98c4` on `experiments`.

## Root cause

Read in `src/tools/command_summary.rs`, and consistent with the probe above:

1. `extract_test_failures` collects every line from the first `failures:` marker to the next `test result:` line, or to the end of the output. It has no line bound and no byte bound.
2. `summarize_test_output` stores that text in `failures` unchanged.
3. `needs_summary` has already decided the output is over the inline budget, so the envelope is built. If `failures` alone is over the budget, `call_content` buffers the envelope again.
4. `extract_error_block`, which fills `first_error` in the build envelope, stops at a blank line or the next diagnostic. That is a structural bound, not a size bound. A diagnostic with no blank line in it can still be arbitrarily wide. Probed after filing, 2026-10-05: a 60 KB block under a rustc-style error line gave a 60,131-byte `@tool_*` envelope on the pre-fix code.

## Evidence

See Symptom. The `stderr` field of the same envelope is bounded by `STDERR_SUMMARY_BYTE_BUDGET`, which is the precedent for the fix.

## Hypotheses tried

1. **Hypothesis:** the `test` envelope bounds `failures` somewhere downstream. **Test:** ran the probe. **Verdict:** rejected: the response is 60,116 bytes and re-buffered.

## Fix

Fixed 2026-10-05 in `e833abeb` on `experiments` (patch-id `faf97e30544df47032c960dd237dae29d6b890e1`; not on `master`), in the same change as `2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md`.

`summarize_test_output` and `summarize_build_output` (`src/tools/command_summary.rs`) now pass `failures` and `first_error` through `bound_stream_bytes` with `FAILURE_FIELD_BYTE_BUDGET` (5,000 bytes). The helper keeps the first and last half and puts a marker between them: `--- failures: <shown> of <total> bytes shown; all of it: output_id (stdout) or output_id.err (stderr) ---`. `<total>` is the length of the extracted section. Both ends are kept because a failure section opens with the first panic and closes on the list of failing test names.

**Why 5,000 and not the generic 2,000.** Failure sections of 2 to 9 KB are inline and complete today, and a 2,000-byte cap would cut output that never needed cutting. The field sits beside at most one `stderr` (about 2,150 bytes with its marker), so the worst raw envelope is about 7,500 bytes. It stays under the 10,000-byte threshold unless JSON escaping inflates the text by more than about 30%. That limit is real: a field that is dense in quotes could still push an envelope over it. Not measured on real failing runs.

Not done: a line bound for `failures`. The byte bound covers it, because any size is bounded.

## Fix provenance

- **SHA:** `e833abeb` (`experiments`)
- **patch-id:** `faf97e30544df47032c960dd237dae29d6b890e1`

## Tests added

All in `src/tools/command_summary.rs` unless stated. Each was red before the fix.

- `summarize_test_output_bounds_one_enormous_failure_line_by_bytes`: one 60 KB line in a `failures:` block; `total` equals the extracted section, and the tail reaches the failing-names list.
- `summarize_test_output_bounds_many_short_failures_by_bytes`: 300 failures of about 30 bytes each. No line is wide, so only the byte bound can bind.
- `summarize_test_output_failures_bound_is_inclusive_at_the_budget` and `summarize_build_output_first_error_bound_is_inclusive_at_the_budget`: one boundary test per field. A field wired to the wrong (2,000-byte) budget passes every "is under the budget" assertion; only these catch it.
- `summarize_test_output_keeps_the_envelope_inline_with_failures_and_stderr_both_at_ceiling`: both budgets together stay under the re-buffer threshold, with quotes in the stderr.
- `summarize_build_output_bounds_one_enormous_error_block_by_bytes`: the `first_error` site.
- `a_huge_failure_line_is_summarized_inline_not_rebuffered` and `a_huge_error_block_is_summarized_inline_not_rebuffered` (`src/tools/run_command/tests.rs`): REACH tests through `RunCommand.call_content`. Red before the fix with `@tool_*` envelopes of 60,116 and 60,131 bytes.

**Cap registration.** `command_summary.failure_field_bytes` is a new `RESULT_CAP` id with a probe row citing `a_huge_failure_line_is_summarized_inline_not_rebuffered`. A row names one test, so the `first_error` site is pinned by the other reach test and not by the row.

**Mutation run, 2026-10-05, final bytes.** M11 (failures bound deleted), M12 (`first_error` bound deleted), M13 and M14 (each field wired to the generic budget) are all killed, as are the shared-helper mutations M3 to M10. M14 survived the first run and led to the `first_error` boundary test. The full list is in the sibling bug's Tests added section.

**Gate.** `./scripts/gate.sh` 2026-10-05: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Workarounds

Read the raw output through the handle named in the summary: `run_command("grep -n 'panicked' @cmd_0b9aeafe")`.

## Resume

Re-run the probe in Symptom on the first binary built from `e833abeb`.

## References

- `src/tools/command_summary.rs`: `extract_test_failures`, `extract_error_block`, `summarize_test_output`, `summarize_build_output`
- `docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md`
- `docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md`
