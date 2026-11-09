---
id: '569b13c182dda64f'
kind: bug
status: fixed
title: 'BUG: grep''s time budget is polled per file, so one large text file overruns it'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-09
opened: 2026-11-09
owner: marius
related:
- docs/issues/archive/2026-08-16-grep-limit-bounds-lines-not-bytes.md
severity: low
unverified: 'The 600 MB reproduction was not re-run against a rebuilt binary: the bound is shown by tests (a counting reader and a sparse file through the tool call), not by that run. The three real overruns (usage.db ids 210368, 208396, 226057) are NOT attributed to this mechanism and this fix is not known to remove them: the largest files under the 386 s call''s root are binary and were already skipped by the sniff. Disk contention or a stalled walker are not ruled out.'
---

## Summary

`grep` promises a wall-clock budget (`time_budget_ms`, default 20000 ms), but the budget is only checked once per walk entry, and a text file is read whole before it is checked again. So one large file can overrun the budget by as long as that file takes to read, and it costs memory up to the file size (plus a second copy). In the last 30 days, 3 of 8,534 `grep` calls overran the 20-second budget by 5 to 19 times. I could measure the mechanism. I could not attribute those three calls to it.

## Class fit

A bound that holds for the common case and not for the one input that is unbounded: the same shape as `grep-limit-bounds-lines-not-bytes` (2026-08-16), where a limit counted matches but not bytes. Here the budget counts entries visited but not the time spent inside one entry. It differs in that no output is wrong; only the latency and memory are.

## Symptom (Effect)

`usage.db`, `tool_calls` where `tool_name = 'grep'`, last 30 days (2026-10-10 to 2026-11-09): 8,534 calls, median 14 ms, 95th percentile 497 ms. Seven calls took more than 20 seconds. Four stopped at 20,001 to 20,009 ms, so the budget held for them. Three did not:

- id 210368, 2026-10-20: 386,711 ms. `{"pattern":"\\bJev\\b|\\bjev\\b|JEV","mode":"files","path":"/home/marius/work/claude","limit":20}`, `overflowed` 1.
- id 208396, 2026-10-19: 135,109 ms. `{"pattern":"SessionMessagesArgs","path":"/home/marius","glob":"**/*.d.ts","mode":"files","include_hidden":true,"limit":5}`.
- id 226057, 2026-10-29: 99,824 ms. The recorded input is empty.

## Reproduction

Mechanism, measured 2026-11-09: a directory with one 600,000,000-byte text file (`big.txt`, 100-byte lines) and one 6-byte file, then:

```
grep(mode="files", path=<that dir>, pattern="zzzz_not_present_anywhere", time_budget_ms=100)
```

Result: `walk cut: the 100 ms wall-clock budget ran out after 1 files were read`, and `usage.db` records 499 ms for the call (id 282421). That is about 5 times the budget, at about 1.2 GB/s from the page cache. A cold or contended disk is slower, and the overrun scales with the file.

## Environment

Linux, HEAD `931b975c5`. Walk root and filters as in the calls above.

## Root cause

Established in the source (`src/tools/grep.rs`):

- `WalkClock::expired` is polled once per walk entry (lines 292 and 352), before the file is read. Its doc comment says so.
- `read_unless_binary` (lines 73 to 85) sniffs the first window and skips a binary file cheaply. For a text file it then calls `read_to_end`, which loads the whole file. There is no size cap.
- The caller turns the bytes into text with `String::from_utf8_lossy` (a second copy when the bytes are valid) and scans line by line.

Not established: that this is what happened in the three real calls. The files over 1 GB under `/home/marius/work/claude` are all binary (`libMIOpen.so`, `libmagma.so`, two `codex` executables, and `best.pt`), which the sniff skips after one window, so they do not explain the 386-second call. Disk contention at the time, a large text file elsewhere, or a stalled walker are all possible, and I did not distinguish them.

## Evidence

- The `usage.db` rows above (ids 210368, 208396, 226057, 282421).
- The measurement above.
- `find /home/marius/work/claude -type f -size +1G` run 2026-11-09: five files, all binary.

## Hypotheses tried

- A large text file under the 386-second call's root: not supported (no such file over 1 GB there).
- The mechanism in general: supported by the measurement.

## Fix

Fixed in `2b05a80f9`, the first of the two options: a per-file byte cap.

- `MAX_FILE_BYTES` is 64 MiB (`src/tools/grep.rs`). A file over it is not searched. It is counted in `skipped_large`, and `format_grep` renders "N file(s) over 64 MiB were not searched; a missing match proves nothing about them" in both modes, over a zero too.
- `read_unless_binary` returns `FileRead::{Text, Binary, TooLarge}`. The sniff runs first, so a large binary stays `skipped_binary` and costs one window. A declared length over the cap returns after the sniff. Otherwise the read stops one byte past the cap, which bounds the memory of a file whose length is under-reported.
- 64 MiB is a judgment, not a measurement.

The second option, polling the clock inside a read, was not done. With the cap, one read costs about 0.1 s, so a poll inside it would not change the budget in a way a caller could see.

This fixes the mechanism. It does not claim to explain the three real overruns (see `unverified`).

## Tests added

In `src/tools/grep.rs`:

- `a_file_over_the_cap_is_skipped_and_named_in_both_modes`: through the tool call, in both modes. A sparse file one byte over the cap, whose leading text carries the pattern, so a search that opened it would list it. Asserts the count, the rendered notice, that the small file still matches and that the big one was not read. It is the cited test of the new probe row `grep.max_file_bytes`; deleting the notice's render made it fail, so the row is `Killed`.
- `a_text_stream_past_the_cap_is_too_large_and_never_held_past_it`, `a_declared_length_past_the_cap_costs_one_sniff_window`, `a_large_binary_is_binary_not_too_large`, `the_cap_is_inclusive_a_file_of_exactly_the_cap_is_read`: the primitive, with counting readers.
- `a_search_with_no_file_over_the_cap_reports_no_skipped_large`: no key when nothing was skipped.
- `a_text_file_within_the_cap_is_returned_whole` replaces `a_text_file_is_returned_whole_whatever_its_size`, whose name no longer held.

## Workarounds

Callers can narrow `path` or `glob`, and can lower `time_budget_ms`, but a lower budget does not bound a single file.

## Resume

1. Rebuild (`./scripts/rb.sh`, then `/mcp`) and re-run the 600 MB reproduction. The call should return in about the budget and report `skipped_large: 1`. Then archive this record.
2. If the three real overruns recur, capture the process state during the call, to attribute them. This fix is not known to address them.


## Fix provenance

- **SHA:** `2b05a80f90eefd2f7ca1219af1dce8eae76a37eb` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `22b0697cf6195ca92469366e0d564aae3005600c` — content hash of the diff; survives rebase and cherry-pick.

## References

- `src/tools/grep.rs`: `WalkClock` (lines 25 to 65), `read_unless_binary` (lines 73 to 85), the polls at lines 292 and 352.
- `docs/issues/archive/2026-08-16-grep-limit-bounds-lines-not-bytes.md` — the sibling.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — why a cut walk must say so.
