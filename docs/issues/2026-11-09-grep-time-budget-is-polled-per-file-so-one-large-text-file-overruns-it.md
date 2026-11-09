---
id: '569b13c182dda64f'
kind: bug
status: open
title: 'BUG: grep''s time budget is polled per file, so one large text file overruns it'
owners:
- marius
tags:
- cluster/unclassified
closed: null
opened: 2026-11-09
owner: marius
related:
- docs/issues/archive/2026-08-16-grep-limit-bounds-lines-not-bytes.md
severity: low
unverified: 'Mechanism measured on one synthetic 600 MB file. The three real overruns (usage.db ids 210368, 208396, 226057) are NOT attributed to it: the largest files under the 386 s call''s root are binary and are skipped by the sniff. Disk contention or a stalled walker are not ruled out. Memory use was reasoned from the code, not measured.'
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

Not fixed. Options for the owner: cap the bytes read per file (and count an over-cap file in `skipped_*` with a notice, as binary files are); and poll the clock between chunks of a read, not only between files. Either keeps the "a partial result never reads as complete" contract that `WalkClock` exists for.

## Tests added

N/A — not fixed.

## Workarounds

Callers can narrow `path` or `glob`, and can lower `time_budget_ms`, but a lower budget does not bound a single file.

## Resume

1. Decide a per-file byte cap and whether to poll inside the read.
2. Re-run the 600 MB reproduction: the call should return in about the budget, not the read time.
3. If the three real overruns recur, capture the process state during the call, to attribute them.

## References

- `src/tools/grep.rs`: `WalkClock` (lines 25 to 65), `read_unless_binary` (lines 73 to 85), the polls at lines 292 and 352.
- `docs/issues/archive/2026-08-16-grep-limit-bounds-lines-not-bytes.md` — the sibling.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — why a cut walk must say so.
