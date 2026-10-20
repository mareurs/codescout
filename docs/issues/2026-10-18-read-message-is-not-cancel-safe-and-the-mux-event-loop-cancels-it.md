---
kind: bug
status: fixed
title: 'BUG: read_message is not cancel-safe and the mux event loop cancels it in a select!, so a client connecting or a watchdog tick during a half-read server frame desynchronises the stream and shuts the mux down'
tags:
- cluster/unclassified
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
severity: high
unverified: Fixed and covered end to end against the real mux with a fake LSP server that splits a frame (red before, green after, 2026-10-20). Not exercised with a real language server, and the rate at which a real server leaves a frame half written when a client connects is not measured.
closed: 2026-10-20
---

# BUG: read_message is not cancel-safe and the mux event loop cancels it in a select!, so a client connecting or a watchdog tick during a half-read server frame desynchronises the stream and shuts the mux down

## Summary

`event_loop` in `src/lsp/mux/process.rs` races `read_message(server_reader)` against `listener.accept()`, the 10 s watchdog tick and two signals in one `tokio::select!`. `read_message` (`src/lsp/transport.rs`) keeps the `Content-Length` it parsed and the body it has read in local variables. When another arm wins while a frame is half read, the future is dropped, the bytes it had taken from the reader are lost, and the next `read_message` starts in the middle of that frame. It then reads garbage (`missing Content-Length header`, or an invalid body), the event loop logs `LSP server disconnected`, breaks, and `run` shuts the mux down and kills the language server. Pre-existing: not introduced by `fix/mux-connect-2026-10-18`; found by the independent review of that branch (finding F9), reproduced 2026-10-18.

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: an operation with internal state in locals is used as a branch of a `select!`, which drops it at any await point, so a cancellation point the reader function never declared loses data the stream cannot give back.

## Symptom (Effect)

A mux that exits (status 0) while its language server is alive, after a client connected, or the watchdog ticked, while a server message was partly written. Every client of that mux loses its LSP at once. Nothing in the exit says why: the log line is `LSP server disconnected: <parse error>`.

## Reproduction

The reproductions are regular tests since the fix (no `#[ignore]`, nothing needs `--ignored` or `--include-ignored`). Both commands below were run 2026-10-20 and run tests (9 and 4).

Transport level (`src/lsp/transport.rs`):

```
scripts/with-slot.sh cargo test --lib lsp::transport
```

It runs 9 tests, including `a_pinned_read_that_loses_a_select_keeps_the_bytes_it_had_consumed` (the pinned-read pattern the event loop uses) and `a_read_dropped_mid_body_desynchronises_the_stream` (a plain dropped `read_message` still loses the bytes it had consumed, which is the hazard).

End to end against the real binary (`tests/mux_event_loop_cancel_repro.rs`): a fake LSP server answers `initialize`, then writes 60 bytes of a frame, pauses, finishes the frame and sends a second message. A connected client must be served `init` and both notifications:

```
scripts/with-slot.sh cargo test --test mux_event_loop_cancel_repro
```

It runs 4 tests (about 16 s). To see the bug itself, restore the old event loop and run the same command: `git checkout 79dcf2c58 -- src/lsp/mux/process.rs` (the `experiments` commit this fix is based on), then `git checkout HEAD -- src/lsp/mux/process.rs` to put the fix back. With the old loop `a_client_connecting_while_the_server_is_mid_frame_does_not_kill_the_mux` fails (mux exited at 4.56 s) and `the_watchdog_tick_during_a_split_frame_does_not_kill_the_mux` fails (12.53 s).

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

Implemented as a variant of the shape sketched earlier (a pinned read, not a reader task), because it is the smallest change that is correct for every caller of `read_message`.

Callers of `read_message` (grep, `references` was unavailable in the worktree: its LSP mux refused to start, "another mux instance holds the lock"): `src/lsp/mux/process.rs` (the initialize loop in `run`, awaited to completion; the `select!` in `event_loop`; `client_reader_task`, a plain `while let` loop in its own task), `src/lsp/client.rs` (`run_dispatch_loop`, a plain loop; the mux-init read wrapped in `tokio::time::timeout`, where a timeout fails the whole connect and the reader is never reused), `src/peer/client.rs` and `src/peer/server.rs` (plain awaits). Only `event_loop` drops the future and then keeps reading, so only `event_loop` needed to change; a reader task per server was not needed.

Change: `event_loop` pins ONE read future outside the loop (`std::pin::pin!(read_message_returning_reader(&mut *server_reader))`), polls it by `&mut` in the `select!`, and when it completes starts the next read with `server_read.set(...)` from the reader the completed future hands back. The new `read_message_returning_reader` (`src/lsp/transport.rs`) exists only to hand the `&mut` back: the pinned slot still borrows the reader until it is overwritten, so a second `&mut *server_reader` taken at that point is rejected by the borrow checker. `read_message` and the wire protocol are unchanged; its doc now says it is NOT cancel-safe and names the pinned pattern. Accept, watchdog tick and signal arms can now win mid-frame without touching the pending read.

Cost of the choice: the safety is a discipline of the one `select!` that races a read, not a property of `read_message`. A `FrameReader` that keeps header and body state in a struct would make it a property of the type, at about 80 more lines; not done because there is one such caller.

- Fix commit (branch `fix/mux-read-cancel-2026-10-20`, based on `experiments` 79dcf2c58): SHA `dc87cefaf90199eb4e02107acebf4fba16d89e9e`.
- patch-id (`git show dc87cefaf | git patch-id --stable`): `33a546b0512421631e0e0c7b20fca92759702d8f`.
- Where: `src/lsp/mux/process.rs` (`event_loop`, the `use` line), `src/lsp/transport.rs` (`read_message` doc, `read_message_returning_reader`).

## Tests added

Two end-to-end tests in `tests/mux_event_loop_cancel_repro.rs` were ignored (`a_client_connecting_while_the_server_is_mid_frame_does_not_kill_the_mux`, `the_watchdog_tick_during_a_split_frame_does_not_kill_the_mux`); they are now un-ignored and run in the default suite, with the two that already ran (`a_split_frame_alone_does_not_kill_the_mux`, `a_child_that_never_prints_ready_fails_the_wait_and_is_stopped`). Review follow-up 2026-10-20: the three mux tests also require that the connected client is served `init` and both `window/logMessage` notifications (the first with its 200-`x` body), because asserting only that the mux stayed up let a server-read arm that never fires survive. Measured mutant: `, if false` added to the server-read arm of the `select!` in `event_loop` was NOT killed before that assertion (4 passed) and is killed after (3 failed, each `got ["init"] from 95 bytes`); the file was restored byte for byte (`git diff HEAD` empty).

Red before, green after, measured 2026-10-20 with `scripts/with-slot.sh cargo test --test mux_event_loop_cancel_repro`:

```
before (src/lsp/mux/process.rs restored to 79dcf2c58, tests as committed):
test a_client_connecting_while_the_server_is_mid_frame_does_not_kill_the_mux ... FAILED
test the_watchdog_tick_during_a_split_frame_does_not_kill_the_mux ... FAILED
  the mux exited Some((4.560070607s, ExitStatus(unix_wait_status(0)))) after a client connected mid-frame, although its server was alive
  the mux exited Some((12.530297987s, ExitStatus(unix_wait_status(0)))) after the watchdog tick fell inside a frame, although its server was alive
test result: FAILED. 2 passed; 2 failed

after (three runs):
test result: ok. 4 passed; 0 failed; 0 ignored; finished in 15.80s / 15.75s / 15.75s
```

Transport level (`src/lsp/transport.rs`, `tests` module): `a_pinned_read_that_loses_a_select_keeps_the_bytes_it_had_consumed` (was the ignored `a_read_dropped_mid_body_does_not_lose_the_bytes_it_had_consumed`; now drives the pinned-read pattern with `read_message_returning_reader` through a `select!` that another arm wins, then checks the first message arrives whole and the second frame is still framed) and `a_read_dropped_mid_body_desynchronises_the_stream` (guards that `read_message` is still NOT cancel-safe, so its doc and the pinned read stay needed; it does not stop anyone moving the read back into the `select!` arm, only the end-to-end tests do that). The transport test cannot be red against the old code (it names the new function); the end-to-end tests are the red-before evidence.

## Workarounds

None known. A reconnect (`/mcp`) restarts the mux after it has died.

## Resume

N/A once the fix is on `experiments`. Optional follow-up: measure how often a real language server (kotlin-lsp, rust-analyzer) leaves a frame half written when a client connects (not measured).

## References

- `src/lsp/mux/process.rs` (`event_loop`)
- `src/lsp/transport.rs` (`read_message`)
- `tests/mux_event_loop_cancel_repro.rs`
