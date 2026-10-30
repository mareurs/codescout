---
id: '95e240ad522e54b8'
kind: bug
status: fixed
title: A project-wide symbols(name=...) waits out rust-analyzer's full 8 s budget, then runs a sequential tree-sitter pass (~11 s per call)
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
severity: high
unverified: Why rust-analyzer never answers workspace/symbol is not established. The 5 min memory window and 1.5 s short budget are chosen, not derived. The first call after a server start still pays the full 8 s when the server does not answer.
---

# BUG: A project-wide symbols(name=...) waits out rust-analyzer's full 8 s budget, then runs a sequential tree-sitter pass (~11 s per call)

## Summary

`search_project_symbols` (`src/tools/symbol/symbols.rs:720-923`) asks each language server for `workspace/symbol` under `PER_LANG_BUDGET = 8s` (`:774`). On this repo rust-analyzer never answers inside the budget, so every call waits 8 s. After that, `files_needing_fallback` (`:676-680`) hands every accepted file to `crate::ast::extract_symbols` in a sequential, synchronous loop inside the async fn (`:894-896`), which adds about 3 s. A call scoped with `path` costs about 0.3 s.

## Class fit

`cluster/unclassified`: no roster class names a timeout budget that is paid in full on every call. What the case shows: a fallback designed for an LSP that is occasionally slow, applied to one that is never ready, so the worst case became the only case.

## Symptom (Effect)

usage.db 2026-09-30 to 2026-10-30, `symbols` with `name` and no `path`: 218 calls, avg 10.9 s, every one over 2 s; 214 of 222 at 8 s or more. Steady at 10-12 s every day. Three calls in this session: 10,659 / 10,906 / 12,935 ms.

`.codescout/debug.log`, one call (2026-10-30, explorer of this session):

```
12:06:03.98  start
12:06:05.99  mux startup failed for kotlin
12:06:12.005 per-language budget exceeded … language="rust" budget_ms=8000
12:06:15.015 tool_done
```

All 33 logged `workspace/symbol` responses were `response_bytes=2` (`[]`).

## Reproduction

`symbols(name="parse_bool_param")` with no `path`, on codescout at HEAD `b27329082`.

## Environment

Linux, shared rust-analyzer behind the LSP mux; repo has 4,803 tracked files, 2,296 of them `.md`.

## Root cause

- `src/tools/symbol/symbols.rs:774, 793`: `tokio::time::timeout(PER_LANG_BUDGET, …)` per language.
- `src/tools/symbol/symbols.rs:894-896`: sequential `extract_symbols` over every accepted file, no prefilter on the searched name.
- measured 2026-10-30: debug.log timestamps above; usage.db query.
- Why rust-analyzer never answers is not established (see Hypotheses).

## Evidence

See Symptom.

## Hypotheses tried

1. **Hypothesis:** rust-analyzer is permanently busy (10.8 GB RSS, ~52% CPU over 9 min on pid 2643509). **Verdict:** deferred, not measured.
2. **Hypothesis:** rust-analyzer's default `workspace.symbol.search.kind` returns types only, so it can never find a function. **Verdict:** deferred; if true the Rust LSP leg adds nothing for function names.

## Fix

Done in `src/tools/symbol/symbols.rs` (2026-10-30):
- `LspLegHealth`: a language whose `workspace/symbol` used the FULL 8 s budget gets `SHORT_LANG_BUDGET` (1.5 s) for `LANG_TIMEOUT_MEMORY` (5 min), then one full-budget call probes it again. A short-budget timeout does not refresh the window; an answer clears it; an LSP error leaves it as it was.
- `name_prefilter_needle`: the tree-sitter pass skips a file whose lowercased text lacks the last segment of the lowercased pattern, only when that segment is `[a-z0-9_]+`. It parses from the text it already read (`extract_symbols_from_text`) instead of reading twice.
- Not done: running the tree-sitter pass concurrently with the LSP legs.

Measured 2026-10-30 (release builds, `symbols(name="parse_bool_param")`, no `path`, over stdio MCP, 4 calls each, same result set in steady state, digest `60079a1889`):

| binary | call 1 | calls 2-4 |
|---|---|---|
| before | 10,870 ms | 3,787 / 4,237 / 4,223 ms |
| after | 2,615 ms | 1,846 / 2,293 / 1,801 ms |

The "before" steady state is about 4 s, not 8 s, because the rust-analyzer in that run answered inside the budget. The fix helps the timeout case most; the prefilter is what lowers the steady state.

## Tests added

`project_search_cost_tests` in `src/tools/symbol/symbols.rs`: budget policy (full, short, expiry, no refresh on a short timeout, answer clears, cap) and `name_prefilter_needle` shapes, including the synthesized Kotlin `Companion` name. Not mutation-tested; the live LSP join is covered only by the measurement above.

## Workarounds

Pass `path` (a directory is enough): ~0.3 s.

## Resume

Send `workspace/symbol("parse_bool_param")` straight to the mux rust-analyzer with no budget and record time and result; check `rust-analyzer/analyzerStatus`. Add a tracing span around the tree-sitter loop logging the accepted-file count and elapsed ms.

## References

- `docs/issues/2026-10-30-a-per-language-budget-timeout-drops-workspace-symbol-without-a-cancel-request.md`
- `docs/issues/2026-10-30-kotlin-and-java-language-servers-start-for-fixture-only-languages-and-the-breaker-never-opens.md`
