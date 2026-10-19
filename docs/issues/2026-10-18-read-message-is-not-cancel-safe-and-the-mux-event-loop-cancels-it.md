---
kind: bug
status: open
title: 'BUG: read_message is not cancel-safe and the mux event loop cancels it in a select!, so a client connecting or a watchdog tick during a half-read server frame desynchronises the stream and shuts the mux down'
tags:
- cluster/unclassified
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
severity: high
unverified: 'Reproduced at the transport level and end to end against the real mux with a fake LSP server that splits a frame (measured below). Not observed with a real language server, and the rate at which a real server leaves a frame half written at the moment a client connects or the watchdog ticks is not measured.'
---

# BUG: read_message is not cancel-safe and the mux event loop cancels it in a select!, so a client connecting or a watchdog tick during a half-read server frame desynchronises the stream and shuts the mux down

## Summary

`event_loop` in `src/lsp/mux/process.rs` races `read_message(server_reader)` against `listener.accept()`, the 10 s watchdog tick and two signals in one `tokio::select!`. `read_message` (`src/lsp/transport.rs`) keeps the `Content-Length` it parsed and the body it has read in local variables. When another arm wins while a frame is half read, the future is dropped, the bytes it had taken from the reader are lost, and the next `read_message` starts in the middle of that frame. It then reads garbage (`missing Content-Length header`, or an invalid body), the event loop logs `LSP server disconnected`, breaks, and `run` shuts the mux down and kills the language server. Pre-existing: not introduced by `fix/mux-connect-2026-10-18`; found by the independent review of that branch (finding F9), reproduced 2026-10-18.

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: an operation with internal state in locals is used as a branch of a `select!`, which drops it at any await point, so a cancellation point the reader function never declared loses data the stream cannot give back.

## Symptom (Effect)

A mux that exits (status 0) while its language server is alive, after a client connected, or the watchdog ticked, while a server message was partly written. Every client of that mux loses its LSP at once. Nothing in the exit says why: the log line is `LSP server disconnected: <parse error>`.

## Reproduction

Transport level (ignored, `src/lsp/transport.rs`):

```
scripts/with-slot.sh cargo test --lib -- --ignored a_read_dropped_mid_body
```

End to end against the real binary (`tests/mux_event_loop_cancel_repro.rs`): a fake LSP server answers `initialize`, then writes 60 bytes of a frame, pauses, finishes the frame and sends a second message. The control passes, the two ignored tests fail:

```
scripts/with-slot.sh cargo test --test mux_event_loop_cancel_repro -- --include-ignored
```

## Environment

Linux, branch `fix/mux-connect-2026-10-18` on `experiments` 7a09623bd; python3 as the fake server.

## Root cause

`read_message` at `src/lsp/transport.rs` (`header` String, `content_length`, `body` Vec are locals across `.await`); the call `server_msg = read_message(server_reader) => {...}` inside `tokio::select!` in `event_loop` (`src/lsp/mux/process.rs`). `tokio::select!` drops the futures of the branches that did not win. measured 2026-10-18, see Evidence. Related: `watchdog_tick` is an `interval`, whose first tick completes at once, so at event-loop start the tick arm can win against a read that is already mid-frame; whether it does depends on `select!`'s random branch order (not measured).

## Evidence

Transport level, 2026-10-18: after writing a frame minus its last 10 bytes, a `select!` whose second arm is ready drops the read; the rest of the frame and a second whole frame are then written:

```
the next read must still see the first message whole, got: Err(missing Content-Length header)
```

(The leftover body bytes carry no newline, so `read_line` takes them together with the second frame's `Content-Length: N` line as one unknown header, then the blank line ends the headers with no length.)

End to end, 2026-10-18, real `codescout mux`, fake server (split at 1 s after `ready`, rest at 4 s):

```
control, nothing racing the read:  mux stays up for the 7 s watched (passes)
client connects at 2 s:            the mux exited Some((4.556966095s, ExitStatus(unix_wait_status(0)))) after a client connected mid-frame, although its server was alive
watchdog tick inside the frame:    the mux exited Some((12.519974743s, ExitStatus(unix_wait_status(0)))) after the watchdog tick fell inside a frame, although its server was alive
```

An earlier version of the control, which sent the partial frame before `ready`, also exited (3.1 s) with no client. Candidate cause, not isolated: the immediate first tick of the watchdog interval winning over a read that had already consumed the partial frame. That is why the control now splits the frame after the loop has started.

## Hypotheses tried

1. The exit is the split frame itself, not the cancellation. Rejected: the control, with the same frame and nothing racing the read, stays up.

## Fix

Unknown, not implemented in this branch (on purpose). Shape: make the server read cancel-safe, either (a) move the server reader into its own task that owns `read_message` and forwards whole messages over an `mpsc` channel, and `select!` on `rx.recv()` (cancel-safe) instead of on `read_message`; or (b) keep the partial frame in state that outlives the future (a `FrameReader` struct holding header and body buffers, whose `poll`/`read_frame` can be resumed after a drop). (a) is the smaller change. `client_reader_task` already runs one reader per client in its own task, so only the server side has the problem. Add the end-to-end tests above, un-ignored, as the acceptance test.

## Tests added

`a_read_dropped_mid_body_does_not_lose_the_bytes_it_had_consumed` (ignored, `src/lsp/transport.rs`); in `tests/mux_event_loop_cancel_repro.rs`: `a_split_frame_alone_does_not_kill_the_mux` (control, runs by default, about 3.5 s of watching after the second review cut it from 7 s: the frame is split 1 s after `ready` and finished 1 s later), `a_child_that_never_prints_ready_fails_the_wait_and_is_stopped` (default; the `ready` wait has a 30 s deadline and a `ChildGuard` stops the mux on every exit path, panics included; the first version used a bare `read_line` and left the child running on a panic), and the ignored `a_client_connecting_while_the_server_is_mid_frame_does_not_kill_the_mux` (about 5 s) and `the_watchdog_tick_during_a_split_frame_does_not_kill_the_mux` (about 13 s). The ignored ones are red now. Nothing under `scripts/` or `.github/` passes `--ignored` or `--include-ignored` (grep, 2026-10-19). Mutants of the test helpers: the guard's Drop made a no-op and the `ready` wait without its deadline were both killed by `a_child_that_never_prints_ready_fails_the_wait_and_is_stopped`.

## Workarounds

None known. A reconnect (`/mcp`) restarts the mux after it has died.

## Resume

1. Pick (a) or (b) above. 2. Un-ignore the three repros and make them pass. 3. Measure how often a real language server (kotlin-lsp, rust-analyzer) leaves a frame half written when a client connects.

## References

- `src/lsp/mux/process.rs` (`event_loop`)
- `src/lsp/transport.rs` (`read_message`)
- `tests/mux_event_loop_cancel_repro.rs`
