---
kind: bug
status: open
title: 'BUG: two lock races in the mux connect path: a wait ended by a freed lock can report wedged without waiting, and the lock probe can make a starting mux exit'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md
severity: low
unverified: 'Neither race was reproduced; both are read from the code (src/lsp/manager.rs, src/lsp/mux/process.rs) and from the review of 2026-10-17.'
---

# BUG: two lock races in the mux connect path: a wait ended by a freed lock can report wedged without waiting, and the lock probe can make a starting mux exit

## Summary

Added with the wait for a peer's starting mux (`connect_to_mux`, `get_or_start_via_mux_waiting` in `src/lsp/manager.rs`): the wait ends early when the mux's ownership flock is found free, and it checks that by briefly TAKING the flock every 200 ms. Both halves have a window.

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: a liveness decision made by acquiring the very lock another process is racing to acquire, so the observer changes the outcome it is observing.

## Symptom (Effect)

Not observed. Two predicted ones: (1) a client reports "mux ... is wedged or mid-restart" at once, with no wait; (2) a mux that was starting exits with "another mux instance holds the lock".

## Reproduction

None yet. (1) needs: client A waits on a peer mux whose process dies, A sees the lock free and ends its wait, a third process starts a new mux and takes the lock before A's round 1 claims it. (2) needs a new mux's `try_lock_exclusive` (`src/lsp/mux/process.rs`, step 1) to land while a waiting client's probe holds the flock.

## Environment

Branch worktree-agent-af85b780928eaa450, commit `ae4b7bca` and before.

## Root cause

Inferred from the code, not measured. (1) `get_or_start_via_mux_waiting` round 1 returns `mux_socket_unreachable_error` the moment `claim_mux_lock` finds the lock held, with no wait, even when the holder is a mux that started a moment ago. (2) `connect_to_mux` calls `claim_mux_lock(lock)` on every failed attempt when given a `holder_lock`; the guard is dropped at once, but for that instant the flock is held, and the mux's lock attempt does not retry.

## Evidence

Source reading only; the second race predates the wait in kind (the round-0 claim probe) and is repeated here once per 200 ms per waiting client.

## Hypotheses tried

None.

## Fix

Unknown. Options: give round 1 the peer wait too when the lock is held by a process that was not held in round 0; replace the acquire-and-drop probe with a probe that does not take the lock (a shared-lock attempt still conflicts; reading `/proc/locks`, or having the mux write its pid and checking liveness, does not).

## Tests added

N/A: open, not reproduced.

## Workarounds

Retry the call.

## Resume

1. Try to reproduce (1) with an in-process test that frees the lock and re-takes it from a second holder between the early stop and round 1. 2. Decide whether the probe should stop taking the flock.

## References

- `src/lsp/manager.rs` (`connect_to_mux`, `get_or_start_via_mux_waiting`)
- `src/lsp/mux/process.rs` (`run`, lock step)
