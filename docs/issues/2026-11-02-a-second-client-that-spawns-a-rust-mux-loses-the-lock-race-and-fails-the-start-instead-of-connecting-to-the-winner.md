---
id: '8e6268a0577ff543'
kind: bug
status: mitigated
title: 'BUG: a client that spawns a rust mux while a peer is spawning one loses the lock race and fails the LSP start with "another mux instance holds the lock", instead of connecting to the winner''s mux (race 3, seen 15 times in 8 days)'
tags:
- cluster/unclassified
closed: 2026-11-02
opened: 2026-11-02
owner: marius
related:
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
- docs/issues/2026-10-18-mux-lock-file-truncated-before-its-flock-is-tried.md
severity: low
unverified: 'Fixed against a script that plays both muxes (winner and loser); a real mux that loses the race was never reproduced, so the live effect is not shown. The rows that remain after the next release tell: re-count `lsp_events` failures with this error then. The 15 failures since 2026-10-28 are 22 since 2026-10-20 (see Evidence).'
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

Race (3) of the parent bug, read from the code on 2026-11-02. In `get_or_start_via_mux_waiting` (`src/lsp/manager.rs`) the call found the lock free, spawned a mux, and a peer's mux took the lock first. The spawned mux waited its 1 s `MUX_LOCK_GRACE`, gave up with "another mux instance holds the lock", and the spawn branch returned that error at once. It did not connect to the winner's mux, which was starting and would bind its socket. The failing starts already ran the fix for race (2): see Evidence, the failures took about 1 s, not milliseconds.

## Evidence

- Query: `SELECT substr(started_at,1,10), outcome, COUNT(*) FROM lsp_events WHERE language='rust' AND started_at >= '2026-10-25' GROUP BY 1,2`.
- The error text of the latest row (2026-11-01 22:35:06, 1,028 ms): `mux process failed to start: Error: another mux instance holds the lock`, `Caused by: Resource temporarily unavailable (os error 11)`, with the existing hint about a locked LSP index.
- `SELECT MIN(handshake_ms), MAX(handshake_ms), COUNT(*) FROM lsp_events WHERE language='rust' AND outcome='failed' AND error LIKE '%another mux instance holds the lock%'` gives 1028, 1144, 15 for the window from 2026-10-28.
- The same query grouped by day, over the whole database (measured 2026-11-02): every failure from 2026-09-10 to 2026-10-19 took 14 to 256 ms (a mux with the single try); every failure from 2026-10-20 took 1,030 to 1,188 ms (22 rows: 5 on 10-20, 2 on 10-24, 15 from 10-28). A mux without the grace cannot take 1 s to fail, so the failing starts since 2026-10-20 ran the grace fix. The count of 15 in this file's title covers only the window from 2026-10-28.

## Hypotheses tried

1. The failures come from race (3). Consistent with the symptom and with the 1 s timing; not shown.
2. The failures are leftovers of races (1) and (2), from sessions running an older build. Refuted for race (2): failures before 2026-10-19 took 14 to 256 ms and all later ones took about 1 s (see Evidence), so the later ones ran the grace. Not decided for race (1), which fails differently (the wedged error, not the lock error).

## Fix

Commit `45a5085ab` on `experiments`. `mux_lost_the_lock_race` (`src/lsp/manager.rs`) recognises the mux's own lock text in the spawned mux's stderr. The spawn branch of `get_or_start_via_mux_waiting` then sets `last_holder_gone` and goes to the next round, where a held lock is a new peer's starting mux and is waited for within the one wait budget. On the last round (`MUX_CONNECT_ROUNDS`) the original error is kept. A RocksDB lock or any other spawn failure still ends the call at once.

## Tests added

- `a_mux_this_call_spawned_that_loses_the_lock_race_connects_to_the_winner`: a script stands in for the spawned mux and forks the winner (lock taken, socket bound 0.5 s later). Red before the fix: the start failed with "mux process failed to start: Error: another mux instance holds the lock". Green after, with one spawn.
- `mux_lost_the_lock_race_names_only_the_muxs_own_lock` and `a_mux_this_call_spawned_that_fails_for_another_reason_is_not_retried` pin that other failures stay immediate errors with one spawn.
- Gate in a clean worktree: all 0, 8841 tests. Not shown: a real mux losing the race; the script plays both sides.

## Workarounds

Retry the call. The later starts in the same window succeeded, which is why the failure rate is low.

## Resume

Mitigated by `45a5085ab`. After the next release, re-count `lsp_events` failures with "another mux instance holds the lock" (query in Evidence). If none appear for a few days of normal use, mark the bug `fixed`; if they continue, the winner is not binding its socket within the wait, and the next step is the mux's own stderr for those timestamps.

## Fix provenance

- **SHA:** `45a5085abe20efb67a2b9ad989934f0fea97a42d` (`experiments`; the mitigation itself, adding `mux_lost_the_lock_race` to `src/lsp/manager.rs`, the only file it touches; `git log -S mux_lost_the_lock_race -- src/lsp/manager.rs` names no other commit)
- **patch-id:** `6d2fbbb4680360fbcb9f1183d6ebcee7dd1392ad` (`git show 45a5085ab | git patch-id --stable`, 2026-11-03)

This commit is a mitigation, not a fix: the root cause, two clients that both see the lock free and both spawn a mux, is not addressed. The losing call now waits for the winner's mux instead of failing, so the status stays `mitigated`.

## References

- `docs/issues/2026-10-17-mux-connect-path-lock-races.md` (races 1 and 2 fixed; race 3 open there)
- `docs/issues/2026-10-18-mux-lock-file-truncated-before-its-flock-is-tried.md`
- `docs/issues/archive/2026-06-11-mux-failure-masks-rocksdb-lock-collision.md`
