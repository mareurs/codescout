---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: medium
owner: marius
related: []
tags: [lsp, retry, rust-analyzer]
kind: bug
---

# BUG: LSP `-32801 ContentModified` errors not auto-retried — bubble up as fatal

## Summary

When rust-analyzer (or any LSP server) advances its analysis snapshot mid-request,
it cancels the in-flight call with code `-32801 ContentModified`. codescout's
retry loop only matched `-32800 RequestCancelled`, so `-32801` fell through as a
fatal `isError: true` result, aborting deeper traversals (notably `call_graph`)
and disrupting the caller. Both codes are explicit LSP "retry on new snapshot"
signals per spec — they should share the retry path.

## Symptom (Effect)

```
LSP error (code -32801): content modified
```

`call_graph(direction="callers", max_depth=3, ...)` returns this error to the
caller and reports `isError: true`. Lower depths (`max_depth=2`) on the same
symbol succeed on retry. The error surface text is the only signal — no hint,
no retry indication.

## Reproduction

1. Fresh `/mcp` reconnect (or first MCP request after `cargo build --release`
   restart with a previously stopped rust-analyzer).
2. Within ~10s of reconnect, invoke `call_graph` with `max_depth=3` on any
   symbol whose `callHierarchy/incomingCalls` traversal touches multiple files.
3. Observe the `-32801` error returned to the tool caller.

Git commit at observation: `a00c94cb` (on `experiments`).

## Environment

- Linux 7.0.0-15-generic
- rust-analyzer (via codescout LSP mux at `/run/user/1000/codescout-rust-mux-*.sock`)
- codescout 0.12.1
- MCP transport: stdio
- Branch: `experiments`

## Root cause

`src/lsp/client.rs:553` (pre-fix) — the retry matcher was
`e.to_string().contains("code -32800")`, narrow to RequestCancelled only.
Per LSP spec, `-32801 ContentModified` is semantically identical for retry
purposes: "snapshot moved, retry on the new one." Rust-analyzer issues it
whenever a new analysis snapshot becomes authoritative mid-request, which is
the dominant failure mode during the warmup window right after a server
restart or `/mcp` reconnect.

`src/server.rs:859` (pre-fix) — `route_tool_error` had the same narrow match,
so even if the retry budget was exhausted (or the method was non-idempotent),
the error rendered as fatal `isError: true` instead of a recoverable response
with a retry hint.

## Evidence

### `.codescout/debug.log` (instance `02dd`, 2026-05-18)

```
01:16:51.810  call_graph: max_depth=3 on RecoverableError/with_hint → start
01:16:51.813  textDocument/documentSymbol → OK (18223 bytes)
01:16:55.693  [LSP notification] textDocument/publishDiagnostics
01:16:55.693  textDocument/prepareCallHierarchy → OK
01:16:55.770  [LSP notification] textDocument/publishDiagnostics
01:16:56.143  callHierarchy/incomingCalls → ERROR -32801 content modified
01:16:56.176  tool_done ok=false (4365ms)
01:16:59.192  call_graph retry max_depth=2 → OK (snapshot stabilized)
```

Two `publishDiagnostics` notifications fired in 76ms (01:16:55.693 →
01:16:55.770) immediately before the `incomingCalls` failure. Each
`publishDiagnostics` is a snapshot-advance signal; the in-flight
`incomingCalls` was racing against rust-analyzer committing a fresh analysis
view, and lost.

## Hypotheses tried

1. **Hypothesis**: User edit during the call. **Test**: Check git status during
   the window. **Verdict**: rejected — no edits made between the two adjacent
   call_graph invocations. **Evidence link**: Evidence above.
2. **Hypothesis**: Workspace lock contention (the existing `-32800` retry
   hint's main cause). **Test**: Check whether any other LSP-running process
   was active. **Verdict**: rejected — only one codescout instance running;
   error fired against rust-analyzer not kotlin-lsp. **Evidence link**:
   Evidence above (rust-analyzer was the LSP server).
3. **Hypothesis**: Indexer snapshot advance mid-request, same retry semantics
   as `-32800`. **Test**: Inspect LSP spec for `-32801`; check timing of
   `publishDiagnostics` notifications relative to the failure. **Verdict**:
   confirmed — spec describes `-32801` as retry-able, and the two
   `publishDiagnostics` immediately before the failure are the
   snapshot-advance trigger. **Evidence link**: Evidence above.

## Fix

Two-part change in the same commit:

1. `src/lsp/client.rs` — extracted `is_retryable_lsp_error()` helper matching
   both `-32800` and `-32801`. Replaced the narrow `.contains("code -32800")`
   in the retry loop with this helper. Same backoff (cold-start 10×3s,
   warm-start 3×300ms), same idempotency guard (`is_idempotent_lsp_method`).
   Non-idempotent methods still fail-fast with the partial-mutation warning.
2. `src/server.rs` — merged `-32800` and `-32801` into a single transient
   branch in `route_tool_error`. Unified hint mentions both codes and notes
   the auto-retry already happens at the client layer.

Commit SHA: `<TBD-on-commit>` on `experiments`.

## Tests added

- `src/lsp/client.rs::tests::is_retryable_lsp_error_matches_both_transient_codes`
  — pins both retryable codes against the matcher; asserts that `-32603`
  (internal error) and timeout-shaped errors do NOT match.
- `src/server.rs::tests::lsp_content_modified_routes_to_recoverable_not_fatal`
  — parallel to the existing `lsp_request_cancelled_routes_to_recoverable_not_fatal`;
  asserts `-32801` produces `isError: false` with a hint mentioning `-32801`
  or `ContentModified`.

## Workarounds

Before fix:
- Retry the failing call manually (delay 2-5s).
- Reduce `max_depth` to shrink the BFS working set.
- Wait 1-2 minutes after `/mcp` reconnect before issuing deep call-graph
  traversals.

After fix: none needed for idempotent methods — the client retries
automatically with backoff. Non-idempotent methods (rename, applyEdit)
still surface as RecoverableError with the unified hint.

## Resume

N/A — fixed.

## References

- LSP spec, JSON-RPC error codes: `RequestCancelled = -32800`, `ContentModified = -32801`.
- Sibling case already handled: `src/lsp/client.rs:60-83` `is_idempotent_lsp_method`,
  `src/lsp/client.rs:530-552` cold-start retry budget.
- Diagnostic log surfacing the timing: `.codescout/debug.log` lines 115-140
  (instance 02dd, 2026-05-18 01:16:47–01:16:59).
