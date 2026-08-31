---
kind: bug
status: fixed
tags:
- librarian
- audit_doc_refs
- lsp
- cluster/declared-not-wired
closed: 2026-07-06
opened: 2026-07-05
owner: marius
related: []
severity: medium
---

# BUG: audit_doc_refs never resolves symbols — LSP is stubbed off in the live tool

## Summary
`librarian(action="audit_doc_refs")` documents symbol-reference validation, but the live
call path passes `lsp: None`, so every `FileSymbol` candidate resolves to `Verdict::Unknown`
in production. Symbol validation only ever runs in resolver unit tests.

## Symptom (Effect)
All `path:symbol` / `path::symbol` references in scanned markdown come back `unknown`
regardless of whether the symbol exists; `n_refs_unknown` is inflated; genuinely-broken
symbol refs are never flagged `symbol_missing`.

## Reproduction
Run `librarian(action="audit_doc_refs")` on any doc citing a real symbol (e.g.
`src/librarian/ids.rs::artifact_id`) — verdict is `unknown`, not `resolved`.

## Environment
codescout `experiments`, observed 2026-07-05 during plan-mode scouting (Explore agent pass).

## Root cause
`src/librarian/tools/audit_doc_refs/mod.rs:222` sets `lsp: None` on `ResolveCtx` with the
comment "v1: LSP not plumbed through ToolContext yet". `resolver.rs:214-282`
(`resolve_file_symbol`) supports LSP lookup but only receives a provider in unit tests.

## Evidence
Scout report (plan-mode, 2026-07-05): "LSP is not actually wired in v1 — `call()` sets
`lsp: None` (`mod.rs:222`) … every `FileSymbol` resolves to `Unknown`."

## Hypotheses tried
N/A — mechanism read directly from source.

## Fix

Wired the real LSP through, reusing the shared instance:

1. `librarian::tools::ToolContext` (`src/librarian/tools/mod.rs`) gained
   `pub lsp: Arc<dyn crate::lsp::LspProvider>`.
2. `build_tool_context()` / `try_build_runtime()` now take that `Arc` as a parameter
   instead of constructing nothing.
3. `src/server.rs`'s `CodeScoutServer::from_parts` — which already holds the server's
   one shared `Arc<dyn LspProvider>` in scope — now passes `lsp.clone()` into
   `try_build_runtime`, so the librarian tool context shares the *same* LSP manager as
   the core MCP tools, never a second instance. The CLI (`open_ctx`, a one-shot process
   with no pre-existing shared manager) constructs its own `LspManager::new_arc()`,
   mirroring `CodeScoutServer::new`'s own default-path construction.
4. `audit_doc_refs::call()` now passes `Some(ctx.lsp.clone())` instead of hardcoded
   `None`. `ResolveCtx.lsp` changed from `Option<&'a dyn LspProvider>` to
   `Option<Arc<dyn LspProvider>>` so `resolve_file_symbol` could adopt
   `client_within_budget` (the same bounded-acquisition helper `list_overview.rs`
   already uses) instead of an unbounded `get_or_start`.

**A second, previously-latent bug surfaced and was fixed in the same pass:**
`resolve_file_symbol` span­ned a *fresh* `tokio::runtime::Runtime` and blocked on it —
harmless as unreachable dead code while `ctx.lsp` was always `None`, but a hard panic
("Cannot start a runtime from within a runtime") once real LSP calls made that branch
live, since `call()` already runs inside the server's own tokio runtime. Fixed by
branching on `tokio::runtime::Handle::try_current()`: inside an existing (multi-threaded)
runtime, use `tokio::task::block_in_place` on that handle; with no ambient runtime
(plain sync unit tests), spin a throwaway one exactly as before. The server's
`#[tokio::main]` uses the (default) multi-threaded flavor, so `block_in_place` is valid
in production; the one test exercising this branch is marked
`#[tokio::test(flavor = "multi_thread")]` for the same reason.
## Tests added

`src/librarian/tools/audit_doc_refs/resolver.rs`: `resolver_resolved_when_lsp_returns_matching_symbol`
(new — no prior test exercised the "LSP returns a matching symbol → Resolved" path at
all); existing `resolver_symbol_missing_for_renamed_symbol` / `resolver_prefers_disk_truth_on_lsp_lag`
updated for the `Arc`-based `ResolveCtx.lsp`.

`src/librarian/tools/audit_doc_refs/mod.rs`: `lsp_wiring_resolves_real_symbol_end_to_end`
(`#[tokio::test(flavor = "multi_thread")]`) — a `MockLspProvider` with a real symbol
registered, driven through `call()` end-to-end, asserting the reference resolves
(not `unknown`) and `scan_meta.degraded` is `false`. This is the test that caught the
nested-runtime panic described above.

Mechanical: every other librarian-tool test file needed a `lsp:` field added to its
`ToolContext` literal (compiler-driven, ~35 call sites across 25 files + 3 external
integration test files) — a `MockLspProvider::with_client(MockLspClient::default())`
placeholder for tests that don't exercise LSP behavior.
## Workarounds
Treat `unknown` symbol verdicts as unvalidated, not as absent; rely on `FilePath`/`FileLine`
verdicts only.

## Resume
Check whether `ToolContext` exposes an LSP handle usable inside `audit_doc_refs::call`
(`src/librarian/tools/audit_doc_refs/mod.rs:162-284`); if yes, wire it behind the existing
`degraded_languages` accounting and un-stub `mod.rs:222`.

## References
`src/librarian/tools/audit_doc_refs/mod.rs:222`, `resolver.rs:214-282`.
