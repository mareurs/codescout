---
id: '810ca8288c04661b'
kind: bug
status: open
title: references treats every new mux socket connection as a cold server and re-asks an empty answer for 5 s
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
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

## References

- 40 s tail: `PROGRESS_CEILING` in `src/lsp/progress.rs`; deliberately chosen in `docs/issues/archive/` (documentSymbol null window) — not part of this bug.
