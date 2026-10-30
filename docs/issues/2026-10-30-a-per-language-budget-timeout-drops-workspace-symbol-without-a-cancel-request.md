---
id: d7dc4022c21c8c5f
kind: bug
status: open
title: A per-language budget timeout drops a workspace/symbol request without sending $/cancelRequest
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-30
severity: medium
unverified: Inferred from code only; no trace shows the missing cancel on the wire, and the load it causes on rust-analyzer is not measured.
---

# BUG: A per-language budget timeout drops a workspace/symbol request without sending $/cancelRequest

## Summary

When `PER_LANG_BUDGET` fires in `search_project_symbols` (`src/tools/symbol/symbols.rs:793`), `tokio::time::timeout` drops the future. `LspClient` sends `$/cancelRequest` only when its own inner 30 s timeout fires (`src/lsp/client.rs:948-968`). So rust-analyzer keeps computing every abandoned query.

## Class fit

`cluster/unclassified`: no roster class names a cancellation that never reaches the peer. What the case shows: an outer timeout that frees the caller but not the server, so abandoned work keeps loading a shared resource.

## Symptom (Effect)

Not observed directly. Inferred from code: no drop guard on the request future.

## Reproduction

Not yet reproducible as an observation — best lead: trace the mux socket during a project-wide `symbols(name=X)` call and look for `$/cancelRequest` after the 8 s budget.

## Environment

Shared rust-analyzer behind the LSP mux.

## Root cause

inferred from `src/lsp/client.rs:948-968` and `src/tools/symbol/symbols.rs:793` — not measured.

## Evidence

Code read 2026-10-30 by the LSP explorer of this session.

## Hypotheses tried

1. **Hypothesis:** piled-up abandoned queries are part of why rust-analyzer never answers (`docs/issues/2026-10-30-project-wide-symbols-waits-out-the-8-s-rust-budget-then-runs-a-sequential-tree-sitter-pass.md`). **Verdict:** deferred.

## Fix

A drop guard on the in-flight request that sends `$/cancelRequest` with the request id when the future is dropped before a response.

## Tests added

None yet.

## Workarounds

None.

## Resume

Read `LspClient::request` in `src/lsp/client.rs` around 948-968; design the drop guard; check whether the mux forwards `$/cancelRequest`.

## References

- `src/tools/symbol/symbols.rs:774-800`
