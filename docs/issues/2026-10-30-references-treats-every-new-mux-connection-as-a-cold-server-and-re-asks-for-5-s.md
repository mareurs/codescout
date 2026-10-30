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
unverified: Observed in one call in debug.log; the share of all references latency it explains is not measured.
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

None.

## Fix

Socket clients take the server's cold/warm state from the mux; wait past 1 s only while `progress.busy`.

## Tests added

None yet.

## Workarounds

None.

## Resume

Count `textDocument/references` re-ask runs ending at 5 s vs 1 s per client age across debug.log to confirm the share beyond one call.

## References

- 40 s tail: `PROGRESS_CEILING` in `src/lsp/progress.rs`; deliberately chosen in `docs/issues/archive/` (documentSymbol null window) — not part of this bug.
