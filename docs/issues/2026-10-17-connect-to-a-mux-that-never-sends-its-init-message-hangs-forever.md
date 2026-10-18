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
unverified: 'The init bound (5 s) and the per-attempt floor (0.5 s) are judgements, not measured against a real mux (the test stand-ins answer at once). Fix is on branch fix/mux-connect-2026-10-18, not yet on experiments.'
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

Landed as commit `3da59648c` on branch `fix/mux-connect-2026-10-18` (patch-id `b3b23ff44a0badaa4b2615b7f4bcc7c7f75720f8`); not yet on `experiments`. Review fixes (F3, F4, F5): `35e3375a5` (patch-id `5b1f94d11e00486263176f94d01e6e3598cfd28f`), `4cd6b3a6a` (`ce1468a551b9ff490b40d2c3a2942e5ca86a3b28`), `f30ced059` (`0ca058214b99231da213d2b5ed8007586926926c`).

Owner of the bound: both, in layers. `LspClient::connect_with_init_timeout(socket, root, init_timeout)` (`src/lsp/client.rs`) wraps the init read in `tokio::time::timeout` and returns the typed error `MuxInitTimeout` (`mux accepted the connection at <path> but sent no init message within <d>`; typed so a test reads the bound that was applied instead of timing it). `LspClient::connect` keeps its signature and uses `MUX_INIT_READ_TIMEOUT` (5 s); it has no production caller (only tests call it), so that default governs nothing by itself in production: the production path is `connect_to_mux` to `connect_with_init_timeout`, which is what the tests of this issue run. The 5 s constant is used in production by the spawned-mux path below.

`connect_to_mux` (`src/lsp/manager.rs`) takes a `MuxConnectPlan` built by `plan_mux_connect` per round, and passes each attempt `mux_init_read_bound(remaining, plan.min_init_read)`: what is left of the window, but never less than the plan's floor. Peer rounds use `MUX_INIT_READ_MIN` (0.5 s), so a window of zero (the remembered wedge's one attempt) still gives a live mux time to answer. The path that SPAWNED the mux and saw its `ready` line (800 ms window, no lock to check) is the one that had NO bound on the init read before this issue's fix (the original unbounded read applied to it too), and the first version of the fix gave it only about 0.8 s and a hard error (review F3, red: `a mux that answers 2 s after accepting is alive: ... sent no init message within 799.99967ms`); its floor is now `MUX_INIT_READ_TIMEOUT` (5 s).

The failure is an ordinary failed attempt: the lock is checked after it (holder gone is not a wedge), the deadline ends it, and `get_or_start_via_mux_waiting` records the wedge only when a round that waited on the call's first holder for its whole window ended with the lock still held. The first version of the fix also let a round that waited on a REPLACEMENT holder for only the remainder of the budget record a wedge (review F1, fixed in the races issue).

Consequences, stated from the code and the constants, not from a measurement on a real mux:

- A call that finds a silent socket while a wedge is remembered costs the floor, `MUX_INIT_READ_MIN` (0.5 s), where an unbound socket costs about 0. Measured 2026-10-18 (review F6, a held lock and a silent socket, 1 s wait): the first call took 1.001 s and six calls inside the memory took 3.009 s in total, about 0.5 s each. A loop over about 60 files against such a mux therefore reaches the 60 s tool timeout (60 x 0.5 s = 30 s after the first wait of up to 30 s). Before the fix the first call never returned. Not mitigated.
- Worst-case overrun of one `connect_to_mux` call past its window: a fast-failing attempt just before the deadline, the 200 ms pause, then a last attempt with the 0.5 s floor, so window + 0.7 s; across the 3 rounds of one `get_or_start_via_mux_waiting` call (rounds after the first have about nothing left and cost the floor each) about budget + 1.7 s. Derived from the code, not measured as a worst case. What IS pinned by a test, exactly: the bound each attempt gets (`an_attempts_init_bound_is_what_is_left_of_the_window_and_never_below_the_floor`, read from the failure, not timed: wall-clock slack on a machine at load 13 to 146 is larger than the 0.4 s a mutant needs to hide in).
- `UnixStream::connect` is not bounded by this change, and does not need to be: measured 2026-10-18 (review F8), against a `tokio::net::UnixListener` that never accepts, the 4097th connect failed in 60.9 us with `Resource temporarily unavailable (os error 11)` (kind `WouldBlock`; `somaxconn` is 4096 on this machine), so a full accept backlog is a fast ordinary failure, not a hang.

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
- Added after the independent review (F3, F4, F5): `a_mux_we_just_spawned_gets_the_full_init_bound_not_the_connect_window` (peer answers 2 s after accepting, run with the production spawned plan; red before: `sent no init message within 799.99967ms`); `a_mux_this_call_spawned_is_connected_to_although_it_answers_its_init_late` (the spawn branch end to end: a script stands in for the mux through the test-only seam `mux_binary_for_test`, binds the socket, prints `ready`, sends init 2 s after the client connects, under a 500 ms peer wait); `an_attempts_init_bound_is_what_is_left_of_the_window_and_never_below_the_floor` (the bound read from the typed `MuxInitTimeout`, exactly: at most a 1 s window, exactly 500 ms for a window of zero), plus an upper bound of 1.7 s on the last attempt's bound in `a_silent_socket_that_appears_late_...`; `each_kind_of_round_connects_with_the_documented_window_and_init_floor` and `the_mux_connect_constants_have_the_values_the_docs_quote` (literals for `MUX_INIT_READ_TIMEOUT` 5 s, `MUX_INIT_READ_MIN` 500 ms, the 800 ms spawned window, the 200 ms retry interval, 3 rounds), `the_mux_lock_constants_have_the_values_the_docs_quote` in `src/lsp/mux/process.rs`.

Red: the repro on 7a09623bd, above. With the bound removed from the fixed tree (a one-year timeout in place of `init_timeout`), six of the seven tests then in the group failed with their outer `Elapsed` (the repro itself and the late-socket test were not in that run); `a_window_of_zero_...` passed there, as it guards the floor, not the bound. Green after the fix.

Mutation probes (one per guarded site, each applied to the exact bytes of a copy of the tree and restored; "killed" = at least one test failed). First round, before the review: 13 killed, none survived. Re-run after the review fixes on the same set plus one per new branch, 39 mutants over `manager.rs`, `client.rs` and `process.rs`: all killed except one, which was then killed too. Table for this issue's sites (tests that failed in brackets, abbreviated): `mux_init_read_bound` without the floor [4, incl. `a_window_of_zero_...`]; ignoring the remaining window [3]; `min` for `max` [7]; floor 5 s [6]; floor 50 ms [4]; floor 150 ms (review F4, had survived) [3, incl. the constant pin]; the whole window as the bound [1, `..._appears_late_...`]; +400 ms at the call site (review F5, had survived) [2]; `LspClient::connect` default 1 s [1]; caller's bound ignored [4]; descriptor leaked on the timeout path [1]; task leaked [1]; the reported `after` not the applied bound [1]; `MUX_INIT_READ_TIMEOUT` 2 s (review F4, had survived) [3]; spawned plan with the peer floor [3]; spawned window zero [1]; spawned plan checking the lock [1]; the spawn branch given a peer plan at the call site [SURVIVED when only the plan was tested, then killed by `a_mux_this_call_spawned_...`]; `holder_gone` forced false [9]; the wedge recorded though the holder is gone [8]; the wedge never recorded [2].

Live smoke: `tests/mcp-smoke-kotlin.sh` on the fixed tree, 2026-10-18 (kotlin-lsp at `/usr/bin/kotlin-lsp`): `15/15 passed (0 failed) in 17 steps`. That the run went through the mux connect path was not checked separately, so it shows no regression in the Kotlin calls, not that this path was exercised. `~/.mcpt/aliases.json` had the same md5 before and after, and no kotlin-lsp, mux or gradle process started by the run was left.

Flake loops: see the races issue; after the review fixes the group of new and changed tests (29 tests, real clock, run in parallel inside one test process) passed 60 of 60 runs of `scripts/with-slot.sh cargo test --lib -- <filters>` on a shared machine whose load average read 56 and 73 (`uptime`) during the run. The earlier 21-test group had given 59 of 60, then 60 of 60.

## Workarounds

Retry after the tool timeout; kill the wedged mux (`fuser` the lock file).

## Resume

Done. Open follow-up, not started: measure the init latency of a real mux to replace the judgement values (5 s, 0.5 s).

## References

- `src/lsp/client.rs` (`LspClient::connect`, `LspClient::connect_with_init_timeout`, `MUX_INIT_READ_TIMEOUT`)
- `src/lsp/manager.rs` (`connect_to_mux`, `mux_init_read_bound`, `MUX_INIT_READ_MIN`)
