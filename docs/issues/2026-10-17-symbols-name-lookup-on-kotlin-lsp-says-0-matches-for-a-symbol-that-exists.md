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

- `src/lsp/manager.rs`: `connect_to_mux` + `MUX_PEER_STARTUP_WAIT` (30 s). A mux another process owns is waited for until its socket binds; the wait ends at once when its lock is freed (the holder died), and a holder that never binds is still reported wedged after the wait. The spawned-by-us path keeps its ~1 s window. `get_or_start_via_mux_waiting` takes the wait as a parameter so tests do not sit out 30 s.
- `src/lsp/mux/process.rs`: `handle_client_message` answers a client's `shutdown` itself (`result: null`, the client's own id) and drops `exit`. A client's session already ends at disconnect (`handle_client_disconnect`: `didClose`, idle timer).

Fix commits on branch `worktree-agent-af85b780928eaa450` (not on experiments): `b451979a` (patch-id `a2068783a60e078be34de6abcabd136703aec224`, the wait) and `c9fc5575` (patch-id `a0a3d8806bdd869b7edb9af73211f0e99593043b`, the mux). Verified live: the Kotlin smoke 15/15 four runs in a row with its `0 matches` retry removed.

## Tests added

- `src/lsp/manager.rs` tests: `connect_to_mux_waits_for_a_peer_mux_that_binds_its_socket_late` (red before: `Failed to connect to mux socket ... No such file or directory`), `connect_to_mux_stops_waiting_when_the_lock_holder_dies` (red when the early stop is disabled: took 20.1 s), `a_peer_mux_that_never_binds_is_reported_wedged_after_the_wait`.
- `src/lsp/mux/process.rs` tests: `a_clients_shutdown_request_is_answered_by_the_mux_not_forwarded_to_the_server`, `a_clients_exit_notification_is_not_forwarded_to_the_server` (both red before: the server saw `Content-Length: 62` / `33`), and the guard `an_ordinary_client_request_is_still_forwarded_with_a_tagged_id`.
- `tests/mcp-smoke-kotlin.sh`: the `0 matches` retry in `call()` is gone, so a recurrence reds the smoke.

## Workarounds

N/A once fixed. Before: repeat the call.

## Resume

N/A: fixed. If `0 matches` plus a mux warning returns, read the warning's cause first: `holds its lock ... unreachable` after 30 s is a wedged mux (`fuser` the lock), `Mux connection lost` is a server that died under the mux.

## References

- `tests/mcp-smoke-kotlin.sh` (`call`)
- `docs/manual/src/troubleshooting.md`
