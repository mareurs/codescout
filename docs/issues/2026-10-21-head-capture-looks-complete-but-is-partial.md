---
kind: bug
status: open
title: a `| head -N` pipe cuts its own unfiltered capture, and the response reports the partial stream as the whole
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-21
severity: medium
unverified: 'the fix is on feat/il3-run-to-completion-20261021 and has not passed the full gate or reached experiments; no SHA or patch-id is recorded yet; dash (as /bin/sh) was not available to test the drain group.'
---

## Summary

`run_command` splices `| tee <capture> |` before a terminal filter so the unfiltered stream can be offered as `unfiltered_output`. When the filter is `head -N`, `head` exits after N lines, the pipe closes, and `tee` is killed by SIGPIPE, so the capture holds only what had arrived by then. The response states `unfiltered_output_lines` for that partial stream and sets no `unfiltered_truncated`, so a cut stream reads as a complete one.

## Class fit

A partial result is returned with no marker the caller can see, and the line count reads as the stream's total. It differs from the class's byte-budget members: nothing here exceeds a limit. The stream is cut by the pipeline's own shutdown, before the store limit is ever consulted, so the truncation machinery has nothing to flag.

## Symptom (Effect)

Measured live 2026-10-21 on `experiments` (live MCP binary):

```
run_command("seq 1 300000 | head -3")
  -> stdout "1\n2\n3\n", unfiltered_output: @cmd_..., unfiltered_output_lines: 1860, no unfiltered_truncated key
run_command("seq 1 300000 | grep 77777")
  -> unfiltered_output_lines: 300000, unfiltered_truncated: true, unfiltered_buffered_lines: 2222
```

The producer makes 300000 lines in both. The `head` run reports 1860 and no flag; the `grep` run reports the true 300000 and flags the stored cut.

## Reproduction

`run_command("seq 1 300000 | head -3")`, then read `unfiltered_output_lines`. Any producer longer than what `tee` has written when `head` closes the pipe reproduces it; the count varies from run to run (1860 above, 12774 in the RED run of the regression test).

## Environment

Linux, `sh -c` (/bin/sh is bash on the dev machine), codescout 0.15.0, branch `experiments`.

## Root cause

`shell_command_configured` resets SIGPIPE to SIG_DFL in `pre_exec` (`src/platform/unix.rs:262`) so that `| head -N` ends the pipeline instead of letting the producer run on (`src/tools/run_command/inner.rs`, the SIGPIPE comment before the Unix spawn). The splice `A | tee cap | head -N` (`inner.rs:288`, as it was before the fix: no group) therefore lets `head` close the pipe and `tee` die on SIGPIPE, cutting the capture to whatever arrived before head closed the pipe. The capture then goes to the response path, which counts its lines and reports them as the stream's.

measured 2026-10-21: the two `run_command` calls above (1860 lines vs 300000 for the same producer). Repeated in the failing run of `a_head_pipe_still_captures_the_whole_stream` before the fix: 12774.

## Evidence

The two calls in Symptom. A `grep` filter reads its whole input, so `tee` is never cut and the count is the full 300000: the difference between the two runs is the filter, not the producer.

## Hypotheses tried

1. **Hypothesis**: the stored-capture limit (10 KB before Task 1) caused the short count. **Test**: `unfiltered_truncated` is absent in the `head` run, and the `grep` run (which is truncated) reports 300000. **Verdict**: rejected as the cause of the `head` count; the limit only explains `unfiltered_buffered_lines`.

## Fix

Plan: wrap the filter in a group that drains what the filter leaves unread, up to the capture store limit (4 MiB), so `tee` does not see a closed pipe before the whole capturable stream has passed and a finite producer runs to completion: `{A} | tee '{cap}' | { {B}` newline `s=$?; head -c 4194304 >/dev/null; exit $s; }`. The drain is bounded on purpose: an unbounded `cat >/dev/null` would let an endless producer (`yes | head -1`, `cat /dev/urandom | tr -dc a-z | head -c 20`) run to the timeout and fill the capture file, where SIGPIPE ended it at once before. Past the bound the group exits, the pipe closes and the producer stops; such a capture is flagged `unfiltered_truncated`. The newline before `s=$?` (not `;`) keeps a trailing `# comment` in `B` from swallowing the closing brace; the group's `exit $s` keeps the filter's own exit status. Change lives in `inject_tee_in` (`src/tools/run_command/inner.rs`). The `pre_exec` SIGPIPE reset stays: it still ends pipelines that are not spliced.

In a chain (`seq 1 1000 | grep 7 | head -2`) the handle used to be an intermediate stream (271 lines against the producer's 1000, measured 2026-10-21 on the old server); the tee is now spliced at the FIRST pipe, so the handle is the producer's own stream.

Fix SHA / patch-id: not yet recorded (branch `feat/il3-run-to-completion-20261021`, not committed to `experiments`).

Cost: a drained finite producer now runs to completion, so `seq 1 N | head -3` takes as long as the producer; that is the price of a complete capture and is the design in `docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`. A FAST endless producer ends when the 4 MiB drain closes the pipe. A SLOW one (fewer than 4 MiB before `timeout_secs`) runs to `timeout_secs`, where the timeout path runs the filter once over the partial capture and returns that, flagged `capture_incomplete` (measured by hand 2026-10-21: `while :; do echo x; sleep 0.2; done | head -1` -> rc=124 at the timeout; the old layout ended at once). The spec accepts this cost.

Count semantics: `unfiltered_output_lines` is the line count of the capture file. For a stream that ended on its own it is the stream's total. When the drain stopped the stream early (a stream over 4 MiB into `head`), it is a LOWER BOUND of the stream, and `unfiltered_truncated` marks it: `seq 1 1000000 | head -3` reports a number between 500000 and 1000000, where the same stream through `grep` reports 1000000.

## Tests added

In `src/tools/run_command/tests.rs`: `inject_tee_in_wraps_the_filter_in_a_draining_group`, `a_head_pipe_still_captures_the_whole_stream`, `a_filter_exit_status_survives_the_drain`, `a_comment_after_the_filter_does_not_break_the_drain`, `a_producers_stderr_is_not_piped_into_the_filter`, `an_endless_producer_into_head_ends_at_once_with_a_bounded_capture`, `a_urandom_password_idiom_ends_at_once`, `a_slow_endless_producer_into_head_runs_to_the_timeout` (pins the slow-producer cost), `a_head_pipe_over_the_store_limit_reports_a_lower_bound_and_the_truncated_flag`, `a_semicolon_tail_after_the_filter_runs_untouched_with_no_capture`. The first two fail before the fix; the others guard properties the group must keep.

## Workarounds

Before the fix: treat `unfiltered_output_lines` of a `| head` command as a lower bound, or run the producer without the filter.

## Resume

Run the gate on `feat/il3-run-to-completion-20261021`, record the SHA and patch-id (`git show <sha> | git patch-id --stable`), then flip `status` to `fixed` and archive.

## References

- `docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`
- `src/tools/run_command/inner.rs` (`inject_tee_in`)
- `src/platform/unix.rs:262`
