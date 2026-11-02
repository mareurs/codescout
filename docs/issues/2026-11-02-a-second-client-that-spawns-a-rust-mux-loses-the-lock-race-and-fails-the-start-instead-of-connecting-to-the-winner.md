---
id: '8e6268a0577ff543'
kind: bug
status: open
title: 'BUG: a client that spawns a rust mux while a peer is spawning one loses the lock race and fails the LSP start with "another mux instance holds the lock", instead of connecting to the winner''s mux (race 3, seen 15 times in 8 days)'
tags:
- cluster/unclassified
opened: 2026-11-02
owner: marius
related:
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
- docs/issues/2026-10-18-mux-lock-file-truncated-before-its-flock-is-tried.md
severity: low
unverified: Cause not established. The 15 failures match the symptom of race (3) in the parent bug, and all 15 took 1,028 to 1,144 ms, which fits the 1 s lock grace that bug describes, but race (3) was never reproduced and nothing here shows the failing starts ran a build with the fixes for races (1) and (2). `lsp_events` has no build id.
---

# BUG: a client that spawns a rust mux while a peer is spawning one loses the lock race and fails the LSP start with "another mux instance holds the lock", instead of connecting to the winner's mux (race 3, seen 15 times in 8 days)

## Summary

On 2026-11-02 a read of `usage.db` found 15 `lsp_events` rows for rust with `outcome = failed` and the error `mux process failed to start: Error: another mux instance holds the lock ... Resource temporarily unavailable (os error 11)`, between 2026-10-28 and 2026-11-01. The parent bug `docs/issues/2026-10-17-mux-connect-path-lock-races.md` describes this as race (3), "read from the code and not reproduced or fixed", and carries `status: fixed` for races (1) and (2), so a query for open bugs does not show it. This file makes race (3) findable and records the measurement.

## Class fit

No class on the roster fits this one without forcing it. The nearest is `IC-1`, a write whose reach exceeds what the writer can see, but here both clients see the lock; they only race to spawn. `cluster/unclassified` is the sanctioned escape, and the parent bug uses it too.

## Symptom (Effect)

A rust LSP start fails after about one second with the error above, and the tool call that needed the language server gets no symbols. The error already carries a hint to close other sessions on the workspace, which does not help when the other holder is a mux that is only starting.

Per day, rust starts in `lsp_events` (codescout project database):

| Day | failed | success |
|---|---:|---:|
| 2026-10-28 | 9 | 27 |
| 2026-10-29 | 2 | 46 |
| 2026-10-30 | 2 | 35 |
| 2026-10-31 | 1 | 50 |
| 2026-11-01 | 1 | 35 |
| 2026-11-02 (to 09:49) | 0 | 7 |

15 of 244 rust starts since 2026-10-26 (6.1%), and the daily count is falling. All 15 took 1,028 to 1,144 ms (`handshake_ms`).

## Reproduction

Not reproduced. Race (3) as the parent bug states it: two clients both find the lock free, both spawn a mux, one mux wins the lock and the other exits; the loser's call returns the spawn failure instead of connecting to the winner's mux.

## Environment

codescout `experiments`, rust-analyzer behind the mux, many concurrent Claude Code sessions on one workspace (more than 30 on this machine on 2026-11-01). Source of the rows: `.codescout/usage.db`, table `lsp_events`.

## Root cause

Unknown, under investigation. Working reading, not established: race (3) of the parent bug. Not checked: whether the failing starts ran a build that already had the fixes for races (1) and (2), which are on `experiments` as `dd961fe5f` and `e9817239c`, because `lsp_events` records no build id. Not checked: whether the loser retries, because the rows record only the failed start.

## Evidence

- Query: `SELECT substr(started_at,1,10), outcome, COUNT(*) FROM lsp_events WHERE language='rust' AND started_at >= '2026-10-25' GROUP BY 1,2`.
- The error text of the latest row (2026-11-01 22:35:06, 1,028 ms): `mux process failed to start: Error: another mux instance holds the lock`, `Caused by: Resource temporarily unavailable (os error 11)`, with the existing hint about a locked LSP index.
- `SELECT MIN(handshake_ms), MAX(handshake_ms), COUNT(*) FROM lsp_events WHERE language='rust' AND outcome='failed' AND error LIKE '%another mux instance holds the lock%'` gives 1028, 1144, 15.

## Hypotheses tried

1. The failures come from race (3). Consistent with the symptom and with the 1 s timing; not shown.
2. The failures are leftovers of races (1) and (2), from sessions running an older build. Not ruled out; the falling daily count would fit it.

## Fix

None yet. Candidate, from the parent bug: the loser of the spawn race connects to the winner's mux instead of returning the spawn failure.

## Tests added

N/A. Nothing is fixed. A test needs a reproduction of the two-spawner interleaving first.

## Workarounds

Retry the call. The later starts in the same window succeeded, which is why the failure rate is low.

## Resume

Open. Next step: add a build id to `lsp_events`, or read the mux server's stderr for the 15 timestamps, to tell race (3) from an old build. Re-count the failures a few days after the next release; if the daily count reaches 0 the cause was the old build, and this bug should become `zombie`.

## References

- `docs/issues/2026-10-17-mux-connect-path-lock-races.md` (races 1 and 2 fixed; race 3 open there)
- `docs/issues/2026-10-18-mux-lock-file-truncated-before-its-flock-is-tried.md`
- `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`
