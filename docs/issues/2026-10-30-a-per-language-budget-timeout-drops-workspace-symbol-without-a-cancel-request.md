---
id: d7dc4022c21c8c5f
kind: bug
status: fixed
title: A per-language budget timeout drops a workspace/symbol request without sending $/cancelRequest
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
severity: medium
unverified: 'RESOLVED 2026-10-30. Was: inferred from code only. The missing cancel and the leaked pending entry are now shown by a failing test; the real-world load on rust-analyzer is still not measured.'
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

Fix SHA `4633fdc7b` (patch-id `a868351dee0d096a1b8b39db2fa32cdae22b05f7`), branch `worktree-agent-abc8b611cde83b95a`.

- `InFlightRequest` drop guard in `src/lsp/client.rs`: dropping `request_with_timeout` before an outcome removes the pending entry (it leaked before) and, if the request reached the wire, spawns a `$/cancelRequest`.
- Found while checking the mux (the Resume asked): `handle_client_message` tags request ids (`"a:3"`) but forwarded `$/cancelRequest` with the untagged `params.id`, so a cancel through the mux matched nothing. The pre-existing inner 30 s cancel was affected too. Now tagged in `src/lsp/mux/process.rs`.

Not measured: the load abandoned queries put on rust-analyzer.

## Tests added

- `lsp::client::tests::dropping_an_in_flight_request_sends_cancel_request_and_clears_pending` — watched fail pre-fix on the leaked pending entry; a mutation that skips the cancel is killed by its `$/cancelRequest` assertion.
- `lsp::mux::process::tests::a_cancel_request_names_the_tagged_id_the_server_was_given` — watched fail pre-fix (`params.id` stayed `3`, not `"a:3"`).

## Workarounds

None.

## Resume

Read `LspClient::request` in `src/lsp/client.rs` around 948-968; design the drop guard; check whether the mux forwards `$/cancelRequest`.

## References

- `src/tools/symbol/symbols.rs:774-800`
