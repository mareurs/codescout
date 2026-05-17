---
status: fixed
opened: 2026-05-07
closed: 2026-05-07
severity: high
owner: marius
related: []
tags: ["semantic_search", "utf-8", "panic", "mcp-crash"]
---

# BUG: `semantic_search` MCP server panicked on UTF-8 multi-byte char near byte 47 of result preview

## Summary

`semantic_search` returned `RpcError` / "MCP server closed stdout" when a result chunk's first line contained a non-ASCII char (`→`, `—`, smart quotes, accented letters) crossing byte index 47. The subprocess exited via SIGABRT mid-tool-call; subsequent calls failed with broken pipe.

## Symptom (Effect)

- `mcp__codescout__semantic_search` call returned `RpcError` to the LLM.
- MCP transport reported "MCP server closed stdout".
- Subprocess SIGABRT mid-tool-call.
- All subsequent codescout tool calls failed with broken pipe until `/mcp` reconnect.

## Reproduction

Trigger: a returned chunk's first line contained a non-ASCII char (`→`, `—`, smart quotes, accented letters) crossing byte index 47.

## Environment

- Date: 2026-05-07
- Tool: `mcp__codescout__semantic_search`

## Root cause

`src/tools/semantic/semantic_search.rs:491` did `&first_line[..47]` — a byte-index slice that panics if byte 47 is mid-UTF-8 sequence. Any tool that builds string previews via byte-slice `&s[..N]` is a panic waiting to happen on UTF-8 input.

## Evidence

Backtrace from the panic naming the byte-slice operation; subprocess exit code reflecting SIGABRT.

## Hypotheses tried

*N/A — migrated from compact form; root cause was immediately obvious from the panic backtrace.*

## Fix

Applied 2026-05-07: use `is_char_boundary` to floor the slice end to the nearest valid char boundary; also count chars (not bytes) for the >50 threshold.

## Tests added

*N/A — migrated from compact form; specific regression test not named in the original entry. Audit recommendation: any tool building string previews via `&s[..N]` is a panic risk on UTF-8 input.*

## Workarounds

Reconnect via `/mcp` after each panic. No mitigation available without the source fix.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-053** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Audit suggestion in original entry: scan other `[..N]` usages in formatter code paths.
