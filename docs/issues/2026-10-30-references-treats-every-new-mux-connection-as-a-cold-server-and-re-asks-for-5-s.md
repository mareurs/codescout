---
id: '810ca8288c04661b'
kind: bug
status: wontfix
title: references treats every new mux socket connection as a cold server and re-asks an empty answer for 5 s
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-11-03
opened: 2026-10-30
severity: medium
unverified: The 5 s empty-answer re-ask did NOT reproduce on a warm mux (an empty references answer took 0.95-1.6 s, first call 1.6 s). Whether it fires on a cold mux, which is where the logged 12-20 s first calls come from, is not established.
---

# BUG: references treats every new mux socket connection as a cold server and re-asks an empty answer for 5 s

## Summary

`request_until_answered` (`src/lsp/client.rs:1328-1335`) re-asks an empty `[]` answer every 200 ms for 5 s while the client is in its cold window, or 1 s when warm. The cold window starts when the client object is created if it never ran `initialize` (`src/lsp/client.rs:815-825`). A mux socket client never runs `initialize`, so every new connection to an already-warm rust-analyzer — including the one after a `post_compact` flush — gets a cold window.

## Class fit

`cluster/value-correct-in-a-frame-its-name-does-not-state`: "cold" is correct for the client object's own age but is read as a fact about the server. It differs from other members because the wrong frame costs time, not a wrong answer.

## Symptom (Effect)

`.codescout/debug.log` 2026-10-30: 25 re-asks from 12:06:00.59 to 12:06:05.67 for `QdrantWrap/scroll_chunk_refs`, 5,150 ms, ending "no locations at all". usage.db: 15 `references` calls in the 4.5-6.5 s band; `references` p50 2.4 s, p90 20 s.

## Reproduction

`workspace(action="status", post_compact=true)`, then `references` on a symbol with no callers.

## Environment

Shared rust-analyzer behind the LSP mux.

## Root cause

- `src/lsp/client.rs:815-825` (cold-window start for socket clients), `:1328-1335` (retry policy).
- measured 2026-10-30: the debug.log span above (one call).

## Evidence

See Symptom.

## Hypotheses tried

1. **Hypothesis (as filed):** every new mux connection is treated as cold, so an empty `references` answer is re-asked for 5 s instead of 1 s. The code part is real: `in_cold_start_window` (`src/lsp/client.rs:815-825`) anchors a socket client at construction, so `not_answered_budget` returns 5 s for 5 minutes after every connect.
   **Verdict: not reproduced.** Measured 2026-10-30, release build, fresh server per run over stdio MCP, rust-analyzer behind a warm mux:
   - `references` on `run_stdio_server` (dead code, no references): 1,632 / 1,223 / 947 ms. An empty answer did NOT cost 5 s.
   - `references` on `rehome_call` (3 references): 2,887 / 304 / 280 ms.
   - `references` on `extract_memoized`: server 1 12,040 / 1,296 / 1,299 ms, server 2 1,862 / 1,257 / 1,277 ms.
   The first-call overhead on a warm mux is 0.4-2.6 s. The one 12 s first call was on a mux that was probably cold; that was not established.
2. **Where the logged time is:** `usage.db` holds 143 successful `references` calls (p50 2.3 s, p90 about 20 s, max 55 s, 895 s total) and five 30-40 s timeouts. The p90 comes from cold rust-analyzer and timeouts, not from a fixed 5 s. The split into empty and non-empty answers could not be made from `output_json`.

No code was changed.

## Fix

Socket clients take the server's cold/warm state from the mux; wait past 1 s only while `progress.busy`.

## Tests added

None yet.

## Workarounds

None.

## Resume

Re-verified 2026-10-30 (no code changed): `references` passes `includeDeclaration: true` (`LspClient::references`), so a resolved symbol never answers `[]`; only an unresolved position does, and only that costs the 5 s. `in_cold_start_window` does anchor a socket client at construction (`src/lsp/client.rs`, ~line 820), so the code claim holds, but its cost on a warm mux is not shown. Needs design before a fix: the mux exposes no server cold/warm state to a socket client, so the proposed fix needs a new mux message. Still to do: count `textDocument/references` re-ask runs ending at 5 s vs 1 s per client age across debug.log.


## Re-checked 2026-10-31

No code changed. The one measurement the Resume section lists, counting `textDocument/references` re-ask runs that end at 5 s against 1 s per client age, cannot be made from the logs on this machine now. `.codescout/debug.log` and its three rotated files (`.1`, `.2`, `.3`) all start on 2026-10-31 at 14:51 or later, so the 2026-10-30 span quoted under Symptom is gone. I grepped `debug.log` for `no locations at all` and `debug.log.2` for re-ask text; `debug.log.1` and `.3` (41 KB each) were not searched. The one match in `debug.log` is a tool response and the three in `debug.log.2` are tool output that quotes bug records, so no `references` re-ask run was found.

What is left: a fresh capture on a cold mux, which needs a `post_compact` flush or a mux restart, a `references` call on a symbol with no callers, and `debug.log` kept for that run. The fix still needs the mux to tell a socket client whether the server is cold. That is a new mux message and an owner design choice. The rust-analyzer watcher bug `35bcf086deeb7cbe` made `documentSymbol` answer null for as long as the process ran on 2026-10-31. It is a separate cause of long first calls and may account for part of the logged 12 to 20 s.

## Decision 2026-11-03

**Decision (made by the main session on the operator's behalf):** no code change until a warm-mux stall is measured. Status `wontfix`.

**Facts re-checked 2026-11-03 at the bytes:**
- `src/lsp/client.rs` `LspClient::in_cold_start_window` (L865-869) anchors at `init_completed_at`, else at `started_at`. A mux socket client never runs `initialize`, so each new socket client starts its own 5-minute window.
- `LspClient::not_answered_budget` (L1386-1393) returns 5 s inside that window and 1 s outside it.
- The 5 s cost on a warm mux was never reproduced (§ Hypotheses tried 1: empty answers took 0.95-1.6 s).
- Shortening the window to 1 s for socket clients is unsafe. The mux forwards a server notification such as `$/progress` only to the clients connected at that moment (`src/lsp/mux/process.rs` L211-212 and L629-630, `broadcast_to_clients`); only `client/registerCapability` has a replay cache for late clients. So a client that connects during a cold start never sees the progress `begin`, and it cannot tell a cold server from a warm one.

**Reason:** the only measured cost on a warm mux is 0.4-2.6 s of first-call overhead, not the 5 s re-ask. The safe fix needs a design that does not exist yet.

**Cost if wrong:** each `references` call that answers `[]` (an unresolved position) on a new socket client to a warm server costs up to 4 s more than needed, for 5 minutes after each connect.

**Reopen trigger:** a measured references stall of 1 s or more on a warm mux (`debug.log` kept for the run, showing a re-ask run that ends at 5 s on a client whose server was already warm).

**The design a fix would need:** a mux message that reports the server's cold/warm state (or replays the open progress tokens) to a socket client when it connects. The client then takes its cold window from the server and not from its own construction time, and waits past 1 s only while `progress.busy`.

## References

- 40 s tail: `PROGRESS_CEILING` in `src/lsp/progress.rs`; deliberately chosen in `docs/issues/archive/` (documentSymbol null window) — not part of this bug.
