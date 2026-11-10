---
id: '4e7ec7bfc047ac7b'
kind: bug
status: open
title: symbols returns stale ranges and mismatched bodies after source formatting
tags:
- navigation
- cluster/unclassified
closed: null
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: symbols returns stale ranges after formatting

## Summary

Pinned `symbols` reads on the retained BFS identity worktree label ranges from an earlier, shorter source layout, then extract unrelated current bytes as the named body. A post-compaction workspace cache flush did not clear the observed mismatch. This is parked outside the call-graph repair.

## Class fit

Unclassified until the stale-cache mechanism is established. The measured fault joins a symbol label/range to different current source bytes; it differs from a wrong caller-resolution result because even a direct named body read is misaligned.

## Symptom (Effect)

`symbols(name="call_site", path="src/tools/symbol/call_graph/identity.rs", include_body=true)` returns a function range 136–186 whose body begins `let coverage = budget.coverage_requiring_a_candidate();`. A forced current read at line169 begins the actual `fn call_site(edge: &Edge) -> Option<CallSite>`. The same discrepancy was observed before and after `workspace(post_compact=true)`.

## Reproduction

Use the retained `.worktrees/ecc-bfs-identity-20261110` at base `7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345` plus the immutable initial candidate patch. Run the pinned named body read and compare actual lines169–185. [Exact captured responses](../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/navigation-range-counterexample.json); [source byte manifest](../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/candidate-source.json).

## Environment

Live codescout MCP server, current Codex session, Rust source changed through structural tools and subsequently formatted. Parent and independent reviewer both observed misaligned named ranges in separate retained worktrees.

## Root cause

Still unknown. Checked 2026-11-10 (session a3eb869d), HEAD ba103456c:

- NOT reproduced. In the retained worktree `.worktrees/ecc-bfs-identity-20261110` (HEAD now 537fa820d, moved since the report), `symbols(name="call_site", path="src/tools/symbol/call_graph/identity.rs", include_body=true, workspace=<worktree>)` returns lines 170-261 with the correct body, and `grep -n` confirms `fn call_site` at line 170.
- NOT reproduced by an external rewrite in the main checkout. After `rustfmt` moved lines in `src/tools/symbol/call_edges/resolver.rs`, `symbols(name="nested_callable_kinds_for")` returned 392-403 and `grep -n` confirmed line 392.
- Verified (via `references`): outside tests, `validate_symbol_position` (`src/symbol/query.rs:292`) has one caller, `fetch_validated_symbol` (`query.rs:549`), and only the three `edit_code` sites in `src/tools/symbol/edit_code.rs` call that. The read path of `symbols` has no position check and no retry with a fresh `did_change`. So a stale LSP range that an edit would catch and retry reaches a `symbols` read unchecked.
- NOT established: why the LSP answered with a stale range at all. Hypothesis, untested: the language server did not receive the file change. The prior instance `docs/issues/archive/2026-07-28-edit-code-target-base-from-stale-lsp-range.md` also never confirmed a mechanism; its lesson applies: a single non-reproducing instance is not a mechanism.

## Evidence

The retained JSON contains both tool responses, explicit workspace, source manifest reference and post-compaction flush observation. The earlier similar observation is bug-fix-session-log:F-220; its cache-flush workaround was not sufficient for this current case.

## Fix

Pending; retain as independent navigation work. Use forced bounded reads of actual bytes after named output contradicts them. Structural edit tools validate ranges separately; verify every resulting diff.

## Tests added

None — no navigation fix attempted. Runtime counterexample captured before any subsequent source edits.

## Resume — 2026-11-10

Open. No fix attempted, by design: there is no reproduction on current bytes. Next step if it recurs: capture, BEFORE any flush or edit, the raw `symbols` response, the file's mtime, and the language server's own `textDocument/documentSymbol` answer for the same file, then compare with the bytes. A candidate hardening, only after a mechanism is shown: validate the start line on the `symbols` read path as the edit path does.
