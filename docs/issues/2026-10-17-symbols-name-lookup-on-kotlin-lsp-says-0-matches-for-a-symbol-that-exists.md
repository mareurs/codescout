---
kind: bug
status: fixed
title: 'BUG: symbols(name=...) on a Kotlin file answers "0 matches" for a symbol that exists, then finds it on the next identical call'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
closed: 2026-10-17
---

# BUG: symbols(name=...) on a Kotlin file answers "0 matches" for a symbol that exists, then finds it on the next identical call

## Summary

Against a fresh copy of `tests/fixtures/kotlin-library`, `symbols` with `name: "search"` and `path: ".../Catalog.kt"` answered `0 matches` once and found `Catalog/search` on the same call a few seconds later. The empty answer is a negative result that does not say it came from a server that had not settled. Found 2026-10-17 by the first live run of `tests/mcp-smoke-kotlin.sh`.

## Class fit

No existing class was checked beyond a read of the roster headings; the closest reading is a failure rendered as an answer, but the roster was not walked class by class, so the sanctioned `cluster/unclassified` is used rather than a forced fit. What the case shows: a not-ready dependency (the Kotlin language server) is reported as an authoritative empty result.

## Symptom (Effect)

Measured 2026-10-17 with the release build of this branch, one `mcp call` per request against one alias, a project copy made with `git archive HEAD tests/fixtures/kotlin-library`:

```
overview 10:41:17   -> Class 7-25 Catalog, members items, CatalogStats, add, search
name try 1 10:41:20 -> 0 matches
name try 2 10:41:26 -> Method 20-21 Catalog/search
name try 3..6       -> Method 20-21 Catalog/search
```

It did not happen on every start: a different fresh project whose first call was the name lookup found `Catalog/search` on the first try (10:39:26). In the second full script run (overview call first) the first name lookup after it failed, and the following one passed; in the first full run the same lookup failed too (its next lookup also failed, but an assertion there was unsatisfiable on its own, see the sibling issue on `include_body`). The cause was not established; "it is the first name lookup after another call" fits three observations and is not shown.

## Reproduction

```
S=$(mktemp -d); mkdir -p $S/p $S/tmp
git archive HEAD tests/fixtures/kotlin-library | tar -x --strip-components=3 -C $S/p
export TMPDIR=$S/tmp
mcp alias add kt-repro ./target/release/codescout start --project $S/p
mcp call symbols -p '{"path": "src/main/kotlin/library/services/Catalog.kt"}' kt-repro
mcp call symbols -p '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt"}' kt-repro   # 0 matches (on the runs above)
sleep 3
mcp call symbols -p '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt"}' kt-repro   # Catalog/search
mcp alias remove kt-repro
```

## Environment

This machine, `kotlin-lsp` at `/usr/bin/kotlin-lsp`, codescout release build of branch `worktree-agent-a2ef1acc5c69d1c42`.

## Root cause

Two mechanisms, each measured on 2026-10-17 against this machine's `kotlin-lsp` with a `kotlin-lsp` wrapper script teeing the mux's server pipes (`in.<pid>.log` / `out.<pid>.log`) and with `pgrep` across calls. They produce the same symptom: a name lookup whose `get_or_start` fails is returned by `search_files_restricted` (`src/tools/symbol/symbols.rs`) as `0 matches` plus a `completeness_warning`.

**1. A client reached a mux that was still starting and was told it was wedged.** The mux takes its ownership flock first (`src/lsp/mux/process.rs`, `run`, step 1) and binds its socket only after the language server has answered `initialize` (step 4, "Bind Unix socket"), seconds for a JVM. `LspManager::get_or_start_via_mux` (`src/lsp/manager.rs`) gave a mux some other process owned 5 connect attempts 200 ms apart (about 1 s), then re-checked the lock in round 1, still found it held, and returned `mux_socket_unreachable_error`. measured 2026-10-17: overview call at 11:40:32, name lookup at 11:40:35 replied `0 matches` with `warning: ... mux startup failed for kotlin: mux for kotlin holds its lock (...codescout-kotlin-mux-c99784f1c17c33c8.lock) but its socket (....sock) is unreachable — the mux process is wedged or mid-restart`, and the same call at 11:40:36 found `Catalog/search` (1 run in 5; repro script `repro4nowrap.sh` in the session scratchpad).

**2. Every client's exit stopped the shared language server.** `handle_client_message` in `src/lsp/mux/process.rs` forwarded every client message, `shutdown` and `exit` included, and `LspClient::shutdown` (`src/lsp/client.rs`) sends both on exit, socket transport or not. The kotlin-lsp then died, its mux with it (`event_loop`: "LSP server disconnected"), so the mux's 300 s idle timeout never applied and each next `mcp call` paid a cold start (which is why every first reply in this issue was served from tree-sitter, `lsp: warming`) or reached a mux whose server was going away. measured 2026-10-17: per-process traces show `a:1 documentSymbol` answered, then `a:2 shutdown`, `exit` written to the server, then the next client's `b:1 documentSymbol` never answered (`LSP request timed out after 30s`); and `repro8.sh`: kotlin-lsp/mux pid 1670365 alive right after call 1 and gone within 3 s, call 2 started pid 1676415. A third reply in the same family was `warning: ... Mux connection lost` (12:19:54).

Not the cause: an empty `documentSymbol` answer from a settling server. The raw replies captured (`tests/fixtures/lsp-replies/kotlin-lsp-catalog-document-symbol.json`; first answer 0.3 s after `didOpen` in 6 of 6 fresh servers) were complete every time.

Correction to the report: the `0 matches` reply was not silent. At this commit it carries the `completeness_warning` that `docs/issues/archive/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` (Bug B) added; the reproductions above all show it, and the smoke script's `*"0 matches"*` retry matched it as a substring.

## Evidence

- `seq1.out` (session scratchpad), 11:40:35: `0 matches` + `mux for kotlin holds its lock (...) but its socket (...) is unreachable — the mux process is wedged or mid-restart`; 11:40:36 same call finds `Method  20-21  Catalog/search`.
- `trace4/in.*.log` (3 server instances for 6 calls, every one `initialize`, `didOpen`, `a:1 documentSymbol`, `a:2 shutdown`, `exit`, then an unanswered `b:1 documentSymbol`) and `trace4` replies alternating pass / `LSP request timed out after 30s`.
- `repro8.sh`: `[after call 1 (+0s)] kotlin-lsp java pids: 1670365`, `[after call 1 (+3s)] kotlin-lsp java pids:` (none), `[after call 2 (+0s)] ... 1676415`. After the fix: pids `1911505 1911574` present at every sample across both calls.
- `repro4nowrap.sh`, 6 calls without the wrapper, 6 passes: the failure is a race, not constant.

## Hypotheses tried

1. **Hypothesis:** kotlin-lsp answers `documentSymbol` with `[]` while it has not settled. **Test:** raw replies from 6 fresh servers (`lspdrive.py`). **Verdict:** rejected: non-empty every time, 0.3 s after `didOpen`.
2. **Hypothesis:** `symbols` has no readiness check and reads a cold server's non-answer as absence. **Test:** read `search_files_restricted`. **Verdict:** rejected for this commit: a failed `get_or_start` is already returned as an `unread` file with a `completeness_warning`.
3. **Hypothesis:** the client gives up on a mux that is starting. **Verdict:** confirmed (mechanism 1).
4. **Hypothesis:** a client's exit takes the shared server down. **Verdict:** confirmed (mechanism 2). A first reading of the `kotlin-lsp` wrapper traces alone was not trusted, because the wrapper changed timing; the unwrapped `pgrep` run settled it.

## Fix

Three parts; every commit is on branch `worktree-agent-af85b780928eaa450` (not on experiments).

1. **Wait for a mux another process is starting** (`b451979a`, patch-id `a2068783a60e078be34de6abcabd136703aec224`; refined by `ae4b7bca`, patch-id `e9a3cac0a2962a00949522d2f28374931b34640c`). `src/lsp/manager.rs`: `connect_to_mux` + `MUX_PEER_STARTUP_WAIT` (30 s) for a mux whose lock another process holds, ending at once when the lock is freed (the holder died). The wait is paid once per (language, workspace): `LspManager::mux_wedged_at` remembers for `MUX_WEDGED_MEMORY` (60 s) that a full wait ended unreachable, and later attempts make ONE connect attempt (a mux that has come up still connects) and fail fast with the same error; a successful connect clears it. Why 30 s and not the 120 s a spawning client allows its own mux: the wait is served inside a tool call whose server timeout is 60 s; the error it ends in says to retry, and the retry connects at once. The spawned-by-us path keeps its ~0.8 s window.
2. **The mux no longer forwards a client's `shutdown`/`exit`** (`c9fc5575`, patch-id `a0a3d8806bdd869b7edb9af73211f0e99593043b`). `src/lsp/mux/process.rs` `handle_client_message` answers `shutdown` itself (`result: null`, the requesting client only, under its own id: number, `0` or string) and drops `exit`.
3. **A socket client disconnects instead of shutting down** (`0dbea4ef`, patch-id `2049b1c10b693c065f1574e6d95917c73dc325bd`; follow-ups `97a9b746`, `9c7ac4e1`, `9a5cc0e4`, `92e76d3b`, `36d09dd3`). Part 2 alone was NOT mergeable: the mux now keeps the socket open after answering, `LspClient::shutdown` waited 5 s for a reader task that ends only on EOF and then dropped its JoinHandle (which detaches the task), so the connection stayed open and the mux's idle timer never started (reviewer's probe, rerun here: `workspace(post_compact)` 7.52 s / 5.04 s, client connections 1,1,2,2,3, server exit 5.04 s). `src/lsp/client.rs` `LspClient::shutdown` for the socket transport now sends neither `shutdown` nor `exit` (an older mux would forward them and stop the shared server), aborts the reader, awaits it (pinned by `socket_shutdown_has_dropped_the_reader_when_it_returns` and its direct twin), and shuts its write half, so the mux sees EOF; a direct client's timed-out wait now aborts and awaits its reader instead of detaching it. The aborted reader never reaches its own end-of-task `drain_pending_disconnect`, so a request in flight on another holder of the `Arc<LspClient>` (LRU/idle eviction, `shutdown_all`) waited out its 30 s timeout (and `symbols` turned that into `0 matches`): both branches now drain (`97a9b746`). Narrowed, not closed: `request_with_timeout` checks `alive` and inserts into `pending` with no await between them, but a request that inserts after the drain and before the write half is shut down (the drain runs first) writes successfully and then waits its full 30 s; from reading, not reproduced (it needs a multi-thread interleaving inside one `shutdown`). The remembered-wedge memory (part 1) is recorded only when the lock was found HELD at the final check after the whole window: `connect_to_mux` checks the lock after every failed attempt, the one that finds the deadline passed included, and reports `holder_gone` when it was free; that case records nothing (`9a5cc0e4`, `92e76d3b`); the memory's clock is tokio's so its 60 s edge is tested on a paused clock.

**Behaviour change for other clients (mux + client together).**
- A mux client that exits, is evicted (idle TTL, LRU) or flushed (`workspace(post_compact)`) now only hangs up. Before: it stopped the shared language server under every other client and the mux with it. The server now lives until the mux's own lifecycle ends it: idle timer after the last client leaves (rust 180 s, kotlin and others 300 s) plus up to one 10 s watchdog tick, server death, or SIGTERM/SIGINT. A JVM therefore stays up to 300 s after the last client where it used to die at once; that is the documented design (`src/lsp/mux/process.rs` header).
- Direct (non-mux) clients are unchanged except that a shutdown whose reader outlives the 5 s wait aborts it.
- A new client against an OLD mux is safe (it sends nothing to stop the server); an OLD client against a NEW mux is answered by the mux and its socket stays open until that client process ends.
- Re-measured with the reviewer's probe on the branch binary: flush 0.04 s and 0.53 s, server exit 0.02 s, client connections 1,0,1,0,1 (no leak), and the idle mux exited about 195 s after its last client left (rust 180 s + tick).

## Tests added

- `src/lsp/manager.rs`: `connect_to_mux_waits_for_a_peer_mux_that_binds_its_socket_late` (red before: `Failed to connect to mux socket ... No such file or directory`), `connect_to_mux_stops_waiting_when_the_lock_holder_dies` (red with the early stop disabled: 20.1 s), `a_peer_mux_that_never_binds_is_reported_wedged_after_the_wait` (waits 1.5 s, longer than the 0.8 s spawned window, so a call site passing the wrong window reds), `a_peer_mux_that_never_binds_costs_one_wait_not_one_per_file` (red before: 6 files took 3.62 s against a 0.6 s wait; red again with the memory disabled), `a_remembered_wedged_mux_is_still_tried_once_and_connects_when_it_has_come_up`, `the_production_peer_wait_is_thirty_seconds` (paused clock; red when the constant is 800 ms and when the spawned window is passed instead of the caller's).
- `src/lsp/mux/process.rs`: `a_clients_shutdown_request_is_answered_by_the_mux_not_forwarded_to_the_server`, `a_clients_exit_notification_is_not_forwarded_to_the_server` (both red before: the server saw `Content-Length: 62` / `33`), `an_ordinary_client_request_is_still_forwarded_with_a_tagged_id`, `a_shutdown_reply_goes_only_to_the_requesting_client_under_its_own_id` (two clients; number, 0, string ids; red when the reply is broadcast and when the id is narrowed to `as_i64`), `a_client_that_shuts_down_and_hangs_up_starts_the_idle_timer` (real reader task over a socket pair).
- `src/lsp/client.rs`: `socket_client_shutdown_closes_its_connection_promptly_and_sends_no_shutdown` (3 connect/flush cycles against a stand-in mux that never hangs up: returns under 2 s, the peer sees EOF, the peer saw no `shutdown`/`exit`, the reader task is gone (observed via `Arc` strong counts, see below); red with the old path: 35 s).
- `tests/mcp-smoke-kotlin.sh`: the `0 matches` retry in `call()` is gone, so a recurrence reds the smoke.
- Added by the second review round: `src/lsp/client.rs`: `socket_shutdown_resolves_a_request_still_in_flight` and `direct_shutdown_that_times_out_resolves_a_request_still_in_flight` (both red before: the in-flight request was still pending; each red again when its drain is removed); the stand-in-mux test now asserts the reader task is gone through `Arc` strong counts with the peer's end held open (the old `reader_handle.is_none()` was true after `take()` whatever shutdown did; red when abort+await is replaced by `drop(handle)`). `src/lsp/mux/process.rs`: the idle-timer test bounds its reply read (with the shutdown arm renamed it hung past 60 s; now fails in 2.00 s). `src/lsp/manager.rs`: `an_early_stop_because_the_holder_died_is_not_remembered_as_a_wedged_mux` (red before; red when `!holder_gone` is dropped), `the_wedge_memory_lasts_sixty_seconds_and_not_longer` (paused clock; red before and red under `|| true`), and the remembered-wedge test asserts the memory is empty after a successful connect (red when the clear is removed).
- Real-time bounds that remain, because the code under test does real I/O: `socket_client_shutdown_closes_its_connection_promptly_and_sends_no_shutdown` (shutdown < 2 s, EOF within 2 s), `connect_to_mux_stops_waiting_when_the_lock_holder_dies` (< 5 s against a 20 s window; a paused clock raced ahead of a concurrently forked child that held the lock fd, 2 failures in 60 runs), and the mux tests' 2 s `timeout` guards. The wait/memory tests in `manager.rs` that only needed time are on a paused clock.
- Not covered by a default test: a real mux + real server end to end (the live probe and the Kotlin smoke are the evidence).

## Workarounds

N/A once fixed. Before: repeat the call.

## Resume

N/A: fixed. Two predicted lock races in the connect path are not reproduced and are filed separately: `docs/issues/2026-10-17-mux-connect-path-lock-races.md`. If `0 matches` plus a mux warning returns, read the warning's cause first: `holds its lock ... unreachable` after 30 s is a wedged mux (`fuser` the lock), `Mux connection lost` is a server that died under the mux.

## References

- `tests/mcp-smoke-kotlin.sh` (`call`)
- `docs/manual/src/troubleshooting.md`
