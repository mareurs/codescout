---
kind: bug
status: fixed
title: 'BUG: connecting to a mux that has bound its socket but never sends its init message hangs forever, so the wait window and the wedge memory are never reached'
tags:
- cluster/unclassified
opened: 2026-10-17
closed: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
severity: low
unverified: 'The init bound (5 s) and the per-attempt floor (0.5 s) are judgements, not measured against a real mux (the test stand-ins answer at once). UnixStream::connect itself was not examined for a hang. Fix is on branch fix/mux-connect-2026-10-18, not yet on experiments.'
---

# BUG: connecting to a mux that has bound its socket but never sends its init message hangs forever, so the wait window and the wedge memory are never reached

## Summary

`LspClient::connect` (`src/lsp/client.rs`) read the mux's init message with no timeout. A mux that has bound its socket but never writes the init message (a mux wedged after `bind`, or one whose accept task is stuck) therefore blocked `connect` for good. `connect_to_mux` (`src/lsp/manager.rs`) only checks its deadline between attempts, so its window never expired, and the wedge memory (`LspManager::mux_wedged_at`) was never written: every later call hung the same way. Found by the third review of the Kotlin symbols branch, 2026-10-17. Fixed 2026-10-18: the read is bounded, by what is left of the caller's window.

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: a bound on a wait that is enforced between steps but not inside the step that blocks, so the bound does not hold. The case differs from a wedged flock in that the socket answers the connect.

## Symptom (Effect)

A tool call that needs the mux never returns until the tool timeout (60 s) cuts it. Pre-existing before the peer-wait change, which kept the unbounded read.

## Reproduction

The test `connect_to_mux_gives_up_on_a_socket_that_never_sends_its_init_message` in `src/lsp/manager.rs` (it was `#[ignore]`d while the bug was open):

```
scripts/with-slot.sh cargo test --lib connect_to_mux_gives_up
```

It binds a Unix listener that accepts and says nothing, then runs `connect_to_mux` with a 1 s window inside a 5 s outer timeout.

## Environment

Linux. Red measured on branch `fix/mux-connect-2026-10-18` at its base, `experiments` 7a09623bd, with the test un-ignored and no other change.

## Root cause

measured 2026-10-17: `a 1 s window must end by itself; still waiting after 5.001611297s`. Re-measured 2026-10-18 on 7a09623bd with the test un-ignored: `a 1 s window must end by itself; still waiting after 5.001800259s`. Mechanism read from `LspClient::connect` (`transport::read_message(&mut buf_reader).await` for the init message with no `tokio::time::timeout`) and `connect_to_mux` (deadline tested only after `LspClient::connect` returns).

## Evidence

The measurements above. The same wait shape (a read with no bound) is not present in the direct-process path, which has `init_timeout`.

## Hypotheses tried

None beyond the reproduction.

## Fix

Landed as commit `3da59648c` on branch `fix/mux-connect-2026-10-18` (patch-id `b3b23ff44a0badaa4b2615b7f4bcc7c7f75720f8`); not yet on `experiments`.

Owner of the bound: both, in layers. `LspClient::connect_with_init_timeout(socket, root, init_timeout)` (`src/lsp/client.rs`) wraps the init read in `tokio::time::timeout` and returns an error `mux accepted the connection at <path> but sent no init message within <d>`; `LspClient::connect` keeps its signature and uses `MUX_INIT_READ_TIMEOUT` (5 s). `connect_to_mux` (`src/lsp/manager.rs`) passes each attempt `mux_init_read_bound(remaining)`: what is left of the window, but never less than `MUX_INIT_READ_MIN` (0.5 s), so a window of zero (the remembered wedge's one attempt) still gives a live mux time to answer. The failure is then an ordinary failed attempt: the lock is checked after it (holder gone is not a wedge), the deadline ends it, and `get_or_start_via_mux_waiting` records the wedge only when the whole window passed with the lock still held, as before. No change to that recording.

Consequences, stated from the code and the constants, not from a measurement on a real mux:

- A call that finds a silent socket while a wedge is remembered now costs up to `MUX_INIT_READ_MIN` (0.5 s) where an unbound socket costs about 0; a loop over N files against a wedged-after-bind mux costs up to N x 0.5 s once the first call has paid the wait. Before the fix the first call never returned. The test `a_peer_mux_that_accepts_but_never_sends_init_is_wedged_and_remembered` asserts only that the second call takes under the 2 s wait.
- An attempt can end up to 0.5 s after the window does.
- `UnixStream::connect` itself is not bounded by this change. Not examined.

## Tests added

All in `src/lsp/manager.rs` unless stated. Real clock throughout: measured 2026-10-18, a paused tokio clock with real socket I/O jumps over the connect (the tests failed with the outer `Elapsed` in 0.00 s with the fix in place), so a paused clock cannot serve these. Time bounds are generous and listed:

- `connect_to_mux_gives_up_on_a_socket_that_never_sends_its_init_message` (the original repro, un-ignored): returns within 5 s for a 1 s window.
- `a_silent_socket_ends_the_connect_when_the_window_ends`: window 1 s; elapsed at least 1 s and under 4 s; exactly one connection accepted by the peer; failure says `no init message`, `holder_gone` false.
- `a_silent_socket_that_appears_late_still_ends_the_connect_with_the_window`: window 3 s, socket appears at 1.5 s; elapsed at least 3 s and under 3.9 s.
- `a_window_of_zero_still_lets_a_live_mux_send_its_init_message`: peer answers 100 ms after accepting; window zero still connects (floor 0.5 s).
- `an_attempt_that_gets_no_init_message_closes_its_connection_and_leaves_no_task`: the peer sees end of file for every accepted connection and `num_alive_tasks` is back to its value before the call.
- `a_peer_mux_that_accepts_but_never_sends_init_is_wedged_and_remembered`: held lock plus silent socket gives the `wedged or mid-restart` error after at least the 2 s wait and under 5 s, records the wedge, one attempt only, and the next call is under 2 s.
- `a_silent_socket_whose_holder_died_is_not_remembered_as_a_wedged_mux`: nothing is recorded when the holder died.
- `src/lsp/client.rs`: `connect_gives_up_on_a_silent_socket_after_the_default_init_bound` (between 5 s and 8 s) and `connect_with_init_timeout_uses_the_callers_bound` (300 ms bound, under 3 s).

Red: the repro on 7a09623bd, above. With the bound removed from the fixed tree (a one-year timeout in place of `init_timeout`), six of the seven tests then in the group failed with their outer `Elapsed` (the repro itself and the late-socket test were not in that run); `a_window_of_zero_...` passed there, as it guards the floor, not the bound. Green after the fix.

Mutation probes (one per guarded site, each reverted; "killed" = at least one test failed): `mux_init_read_bound` without the floor killed (`a_window_of_zero_...`); ignoring the remaining window killed (`a_silent_socket_ends_...`); `min` for `max` killed (2); floor 5 s killed (3), floor 50 ms killed (`a_window_of_zero_...`); the call site passing the whole window instead of what is left killed (`..._appears_late_...`); `LspClient::connect` default 1 s killed; `connect_with_init_timeout` ignoring the caller's bound killed (4); a leaked descriptor on the timeout path killed (`an_attempt_that_gets_no_init_message_...`) and a leaked task on that path killed (the same test, by its task count); `holder_gone` forced false killed (3); the wedge recorded although the holder is gone killed (2); the wedge never recorded killed (2). None survived.

Live smoke: `tests/mcp-smoke-kotlin.sh` on the fixed tree, 2026-10-18 (kotlin-lsp at `/usr/bin/kotlin-lsp`): `15/15 passed (0 failed) in 17 steps`. That the run went through the mux connect path was not checked separately, so it shows no regression in the Kotlin calls, not that this path was exercised. `~/.mcpt/aliases.json` had the same md5 before and after, and no kotlin-lsp, mux or gradle process started by the run was left.

Flake loops: the new and changed tests of this issue and of `2026-10-17-mux-connect-path-lock-races.md` were run as one group (21 tests, real clock, run in parallel inside one test process) 60 times with `scripts/with-slot.sh cargo test --lib -- <filters>`, on a machine other agents were also building on: 59 of 60 passed; the one failure was in the setup of a race-(1) seam test (see that issue), not in a test of this one; after that setup was made tolerant, 60 of 60 passed.

## Workarounds

Retry after the tool timeout; kill the wedged mux (`fuser` the lock file).

## Resume

Done. Open follow-ups, not started: bound `UnixStream::connect` if a full accept backlog turns out to block it (not examined); measure the init latency of a real mux to replace the judgement values.

## References

- `src/lsp/client.rs` (`LspClient::connect`, `LspClient::connect_with_init_timeout`, `MUX_INIT_READ_TIMEOUT`)
- `src/lsp/manager.rs` (`connect_to_mux`, `mux_init_read_bound`, `MUX_INIT_READ_MIN`)
