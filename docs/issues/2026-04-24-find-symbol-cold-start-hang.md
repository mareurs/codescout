---
status: fixed
opened: 2026-04-24
closed: 2026-05-17
severity: medium
owner: marius
related: ["BUG-049"]
tags: ["find_symbol", "lsp", "cold-start", "workspace-symbol", "tree-sitter-fallback"]
---

# BUG: `find_symbol` hung 60 s during LSP cold-start indexing

## Summary

`find_symbol` (workspace/symbol) could hang up to 60 s waiting for a cold-starting LSP server to respond. Mitigated 2026-04-24: `workspace/symbol` bypasses the cold-start retry budget; `find_symbol` falls over to tree-sitter in ~1 s. Per-file paths (`list_symbols`, `hover`, `goto_definition`, `references`) remain unaffected because they have different cold-start semantics.

## Symptom (Effect)

Immediately after `/mcp` reconnect on a large project (rust-analyzer reindex active), `find_symbol("Foo")` waits ~60 s before returning. Calling the same tool after warmup returns instantly.

## Reproduction

`/mcp` reconnect on a project large enough that rust-analyzer takes >10 s to reindex; immediately call `find_symbol("<any symbol>")` and observe latency.

## Environment

- Date observed: 2026-04-24
- Tool: `find_symbol`
- LSP backend: rust-analyzer
- Triggering condition: cold-start indexing on `/mcp` reconnect

## Root cause

The cold-start retry budget in `src/lsp/client.rs` waits for the LSP server to settle before falling back. `workspace/symbol` was included in the retry budget by default, so it sat in the retry loop instead of falling over to tree-sitter.

## Evidence

Latency comparison: cold-start ~60 s, post-warmup <100 ms. Test `workspace_symbol_skips_cold_start_retry_budget` exercises the regression.

## Hypotheses tried

1. **Hypothesis:** `workspace/symbol` should bypass the cold-start retry budget and fall over to tree-sitter immediately. **Verdict:** Confirmed — adopted as the fix. **Evidence link:** see Fix.

## Fix

Applied 2026-04-24: `workspace/symbol` bypasses the cold-start retry budget in `src/lsp/client.rs` via `uses_cold_start_retry_budget`; `find_symbol` falls over to tree-sitter in ~1 s.

**Architectural review 2026-05-17 (M3 closure):** considered replacing the retry loop with a deterministic ready-signal (e.g. `$/progress` for rust-analyzer, server-status notifications). Rejected. The retry loop is the right shape for two reasons:

1. **No universal ready-signal.** rust-analyzer has `$/progress`; kotlin-lsp doesn't; pyright is fast enough that it doesn't matter. Going deterministic per-server adds protocol code AND still needs the retry-loop fallback for non-signaling servers — net more code, same behaviour.
2. **The retry loop is correct for per-file ops.** A `textDocument/documentSymbol` call CAN succeed as soon as that one file is parsed (seconds), even while the project-wide index is still warming. Replacing patient retry with a global ready-signal would needlessly delay per-file ops until the whole project is ready.

The two-path design (bypass for `workspace/symbol`, patient retry for per-file) absorbs the cold-start scenario exhaustively. Status flipped from `mitigated` to `fixed` — the bypass corrects the wrong method classification at the boundary; it isn't a workaround on top of an unresolved root cause.
## Tests added

- `workspace_symbol_skips_cold_start_retry_budget`

## Workarounds

Wait ~60 s after `/mcp` reconnect before calling `find_symbol`. Per-file paths (`list_symbols`, `hover`, `goto_definition`, `references`) are unaffected and can be used during cold-start.

## Resume

If cold-start latency reappears for `find_symbol`, capture the LSP server stderr around the hang and re-verify the bypass is still wired through `uses_cold_start_retry_budget`. If a new LSP backend ships a reliable indexing-done signal and a user reports `workspace/symbol` calls failing during cold-start that *would* succeed post-warmup, revisit the deterministic-signal trade-off for that specific backend.
## References

- Originally tracked as **BUG-048** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: BUG-049 (kotlin-lsp multi-session 90 s hang).
