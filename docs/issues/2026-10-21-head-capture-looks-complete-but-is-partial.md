---
kind: bug
status: fixed
title: a `| head -N` pipe cuts its own unfiltered capture, and the response reports the partial stream as the whole
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-21
severity: medium
unverified: dash (as /bin/sh) was not available to test the drain group itself; commit 2acc553ad made the capture tests portable to dash and BSD seq but they were not run under dash here. A live check of `seq 1 300000 | head -3` on a freshly built server was not done.
closed: 2026-10-24
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

Fix commits, all on `experiments` (verified 2026-10-24 with `git branch --contains`; each patch-id from `git show <sha> | git patch-id --stable`):

- `24c963d3f1d5b64d4c27da5b5876154d75c0f3b0`, "a head pipe no longer cuts its own capture" (the draining group, `splice_tee` in `src/tools/command_summary.rs`, called from `inject_tee_in`): patch-id `b9519bca16155f8276f66d89fd80642d6649956c`
- `8f696c5ee0589bd91c6a28bb38be0033ef55f9a0`, "bound the drain at the capture store limit so an endless producer still ends": patch-id `353226d79a186805ca90f5e3ea418b6dcf89cdc8`
- `026d510b4d4fa4389a13be7a34e58d851cec84bf`, "bound the capture file and its read, decode it lossily, say what a captured timeout is (R26, R27, R29)": patch-id `0ef5a0256d2b548aac20fff698a4a2281de4a49e`

Re-checked 2026-10-24 on the merged integration tree (`integration/fixes-20261024`, which contains all three commits): the full gate exited 0. Its default-feature library lane ran all 7932 library tests (7918 passed, 0 failed, 14 ignored), which include `a_head_pipe_still_captures_the_whole_stream`, `an_endless_producer_into_head_ends_at_once_with_a_bounded_capture` and `a_head_pipe_over_the_store_limit_reports_a_lower_bound_and_the_truncated_flag`. A separate run of the whole `tools::run_command::tests::` module on the pre-merge `fix/c-20261024` tree (also containing the three commits) passed 389 tests.

Cost: a drained finite producer now runs to completion, so `seq 1 N | head -3` takes as long as the producer; that is the price of a complete capture and is the design in `docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`. A FAST endless producer ends when the 4 MiB drain closes the pipe. A SLOW one (fewer than 4 MiB before `timeout_secs`) runs to `timeout_secs`, where the response is flagged `capture_incomplete`: the filter run once over the partial capture when `filter_is_replayable` allows it (`src/tools/command_summary.rs`), else the capture's handle with `filter_not_run` (measured by hand 2026-10-21: `while :; do echo x; sleep 0.2; done | head -1` -> rc=124 at the timeout; the old layout ended at once). The spec accepts this cost.

Capture file bound (R26, 2026-10-22): the producer's stream reaching a whole-stream filter (`grep`, `sort`, `tail`) was written to the capture file in full (`git log --all -p` is 267,840,758 B, on a tmpfs `/tmp` that is RAM) and then read whole into server memory. The splice now runs the tee as `( trap '' XFSZ; ulimit -f N 2>/dev/null; exec tee '{cap}' 2>/dev/null )` with `N = ceil((4 MiB + 64 KiB) / 512)` = 8320: the filter still reads the whole stream, and the file stops at 4,259,840 B under POSIX `sh` (8,519,680 B under a bash outside POSIX mode, which counts 1024-byte units; measured 2026-10-22). `read_capture` also reads at most 4 MiB + 1 MiB of it, decodes it lossily (a byte that is not UTF-8 used to drop the whole handle), and marks the handle truncated when the file went on past what was read or reached the file bound.

Count semantics: `unfiltered_output_lines` is the line count of the capture read. For a stream that fit the capture file it is the stream's total. Whenever `unfiltered_truncated` is set it is a LOWER BOUND of the stream, for every filter: the drain stopped an early-exit filter's producer (a stream over 4 MiB into `head`), or the file bound stopped the capture of a whole-stream filter's. `seq 1 1000000 | head -3` and `seq 1 1000000 | grep zzz` both report a number between 500000 and 1000000 (624421 measured through `grep`, the lines that fit 4,259,840 B), where `seq 1 620000 | grep zzz` (4,228,895 B, under the file bound) reports exactly 620000.

## Tests added

In `src/tools/run_command/tests.rs`: `inject_tee_in_wraps_the_filter_in_a_draining_group`, `a_head_pipe_still_captures_the_whole_stream`, `a_filter_exit_status_survives_the_drain`, `a_comment_after_the_filter_runs_the_command_untouched` (a `#` after the first pipe is not spliced at all), `a_producers_stderr_is_not_piped_into_the_filter`, `an_endless_producer_into_head_ends_at_once_with_a_bounded_capture`, `a_urandom_password_idiom_ends_at_once`, `a_slow_endless_producer_into_head_runs_to_the_timeout` (pins the slow-producer cost), `a_head_pipe_over_the_store_limit_reports_a_lower_bound_and_the_truncated_flag`, `a_semicolon_tail_after_the_filter_runs_untouched_with_no_capture`. The first two fail before the fix; the others guard properties the group must keep. For the file bound (R26): `a_whole_stream_filter_sees_every_line_while_its_capture_is_bounded`, `the_spliced_capture_file_is_bounded_while_the_filter_reads_the_whole_stream`, `a_capture_file_over_the_read_limit_is_read_bounded_and_flagged`, `a_cut_capture_source_is_marked_truncated_whatever_its_size`, `a_capture_that_is_not_utf8_keeps_its_handle`, `store_capture_keeps_exactly_the_limit_and_cuts_one_byte_over`.

## Workarounds

Before the fix: treat `unfiltered_output_lines` of a `| head` command as a lower bound, or run the producer without the filter.

## Resume

Done: on `experiments`, gate green. Archive into `docs/issues/archive/` with `doc(action="move")`. Not done: `dash` as `/bin/sh` was not tested for the drain group (the later commit `2acc553ad` made the capture TESTS portable to dash and BSD `seq`, which is not the same as running the drain group under dash).

## References

- `docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`
- `src/tools/run_command/inner.rs` (`inject_tee_in`)
- `src/platform/unix.rs:262`
