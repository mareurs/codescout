---
status: open
opened: 2026-07-05
closed:
severity: medium
owner: marius
related: []
tags: [librarian, audit_doc_refs, lsp]
kind: bug
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
Plumb an `LspProvider` from `ToolContext` into `ResolveCtx` (respecting the LSP budget /
`client_within_budget` machinery), or degrade honestly: report `lsp_languages_offline` for
ALL languages so consumers know symbol verdicts are non-authoritative.

## Tests added
N/A — not yet fixed.

## Workarounds
Treat `unknown` symbol verdicts as unvalidated, not as absent; rely on `FilePath`/`FileLine`
verdicts only.

## Resume
Check whether `ToolContext` exposes an LSP handle usable inside `audit_doc_refs::call`
(`src/librarian/tools/audit_doc_refs/mod.rs:162-284`); if yes, wire it behind the existing
`degraded_languages` accounting and un-stub `mod.rs:222`.

## References
`src/librarian/tools/audit_doc_refs/mod.rs:222`, `resolver.rs:214-282`.
