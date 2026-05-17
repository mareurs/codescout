---
status: fixed
opened: 2026-05-07
closed: 2026-05-07
severity: medium
owner: marius
related: []
tags: ["symbols", "lsp", "cold-start", "silent-empty", "tree-sitter-fallback"]
---

# BUG: `symbols(path)` returned silent empty `[]` during LSP cold-start indexing

## Summary

`symbols(path)` returned `{"file": "<path>", "symbols": []}` shortly after session start for files that did have symbols. Agent saw an empty list, thought the file was empty, and abandoned it. Caused by rust-analyzer responding to `textDocument/documentSymbol` with `Ok([])` (not `RequestCancelled`) during cold-start indexing — the cold-start retry budget never fired and tree-sitter fallback was not invoked because the LSP call did not error. Fixed by routing `Ok([])` for a non-empty source file through tree-sitter fallback.

> Originally filed as BUG-054 on a parallel branch; renumbered on merge to avoid ID collision with the `edit_code` stray-brace BUG-054.

## Symptom (Effect)

Called `symbols("src/tools/mod.rs")`, `symbols("src/agent/mod.rs")`, `symbols("src/tools/output.rs")` shortly after session start. All three returned `{"file": "<path>", "symbols": []}`.

Calling the same tool with `detail_level="full"` returned the full symbol set. Re-running the compact call ~1 minute later also returned the full set. Empty result was non-deterministic and tied to LSP cold-start.

## Reproduction

Right after a fresh `/mcp` connect on a large Rust project, call `symbols("src/tools/mod.rs")` (or any other top-level mod file). Repeat after ~60 s to see the symbol set populated.

## Environment

- Date: 2026-05-07
- Tool: `symbols` (path-only / `list_overview` dispatch)
- Backend: rust-analyzer
- Session phase: immediately post `/mcp` connect

## Root cause

rust-analyzer responds to `textDocument/documentSymbol` with `Ok([])` (success, empty list) during initial indexing rather than `-32800 RequestCancelled`. `is_idempotent_lsp_method` would have triggered the cold-start retry budget on a `RequestCancelled`, but `Ok([])` is treated as a valid empty result and propagated to the caller. Tree-sitter fallback (which would have populated module decls) is not invoked because the LSP call did not error.

## Evidence

- Three different module files all returned empty within seconds of `/mcp` reconnect.
- `detail_level="full"` returned populated symbols (different code path warming LSP first).
- Re-run ~60 s later: populated.

## Hypotheses tried

1. **Hypothesis:** rust-analyzer returns `RequestCancelled` during indexing — cold-start retry budget should cover this. **Test:** Inspect actual LSP response. **Verdict:** Rejected — observed response was `Ok([])`, not `RequestCancelled`. **Evidence link:** see Root cause.
2. **Hypothesis:** Tree-sitter fallback should run when LSP returns empty for a non-empty file. **Verdict:** Confirmed — adopted as the fix. **Evidence link:** see Fix.

## Fix

In `list_overview`'s single-file branch, when `client.document_symbols(...)` returns an empty Vec for a file with non-empty source AND tree-sitter detects the language, fall over to tree-sitter symbol extraction.

## Tests added

*N/A — migrated from compact form; specific regression test not named in the original entry. Recommend adding `list_overview_falls_back_to_treesitter_on_empty_lsp_for_nonempty_file`.*

## Workarounds

Pre-fix: retry the call after ~30–60 s; or pass `detail_level="full"` (no different code path on this — the bug appears to require LSP warmup, and by the time the second call runs LSP is warm).

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-057** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Renaming note: originally **BUG-054** on a parallel branch; renumbered on merge to avoid ID collision with the `edit_code` stray-brace BUG-054.
