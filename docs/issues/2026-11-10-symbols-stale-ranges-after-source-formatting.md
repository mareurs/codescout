---
id: '4e7ec7bfc047ac7b'
kind: bug
status: zombie
title: symbols returns stale ranges and mismatched bodies after source formatting
tags:
- navigation
- cluster/unclassified
closed: null
last_observed: 2026-11-10
opened: 2026-11-10
owner: marius
severity: medium
unverified: Not reproduced for symbols on current bytes. Scratch crates only, rust-analyzer, one run per case; the symbols cases ran on the socket transport only (checked in the process tree); the retained worktree from the report was not used.
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

## Measured 2026-11-11: `symbols` resyncs; the stale path is `symbol_at`

Session `5f715d8f`, HEAD `5bccb5bdf`. A scratch crate pinned with `workspace=`, rust-analyzer, file rewritten from a shell outside codescout. Full table in `a73c49456056fb92`.

- **`symbols` after an external edit: not reproduced.** After prepending 4 lines (size change), `symbols(name, path, include_body=true)` returned L9-12 with the correct body, and `symbols(name)` with no path returned L9. After a same-length rewrite that moved the blocks, `symbols(path)` returned the new ranges. `document_symbols` calls `resync_if_drifted` (`src/lsp/client.rs:1512`) and the check works.
- **The documented same-size blind spot is not reachable on this machine.** 0 of 2000 same-length rewrites had an identical mtime (btrfs, kernel 7.2.15; median gap 48.9 µs). It cannot explain a stale `symbols` range here. It might on a filesystem with coarse mtime; that was not tested.
- **A stale path exists, but not for `symbols`.** `symbol_at` (hover, goto_definition) does not resync: after the same external edits it returned "no definition resolvable" at the current position, and `hover` text of the old layout at the old position. One `symbols(path)` call repaired it. Filed as `a73c49456056fb92`. This supports the hypothesis above (the server did not receive the change) for position-based calls only.

**What this does not settle.** A second round (in `a73c49456056fb92`) found that `references` returns a stale location for a file that is open, changed on disk and not named in the call (`other.rs:2  // y`, real line 5). That is a candidate path to a label/bytes mismatch in a `call_graph`-style report like this one, because positions inside other files come from the server. I did not run `call_graph` against it in a way that shows it. The original report used `symbols`, and `symbols` resynced in every case I ran. The mismatch in the report may still come from the socket (mux) transport, from a path other than `document_symbols`, or from a call that read ranges through another tool. My first scratch run used the default rust config, `mux: true`; I confirmed in the process tree that its `rust-analyzer` ran under `codescout mux --socket …`, so the `symbols` result above covers the socket transport the archived fix found fragile. A second crate with `[lsp.rust] mux = false` ran under `codescout start` (stdio) and showed the same cross-file `references` staleness (`a73c49456056fb92`, third round). I did not repeat the `symbols` cases on stdio. This bug stays `open` and unreproduced for `symbols`.

## Fix

Pending; retain as independent navigation work. Use forced bounded reads of actual bytes after named output contradicts them. Structural edit tools validate ranges separately; verify every resulting diff.

## Tests added

None — no navigation fix attempted. Runtime counterexample captured before any subsequent source edits.

## Resume — 2026-11-10

Open. No fix attempted, by design: there is no reproduction on current bytes. Next step if it recurs: capture, BEFORE any flush or edit, the raw `symbols` response, the file's mtime, and the language server's own `textDocument/documentSymbol` answer for the same file, then compare with the bytes. A candidate hardening, only after a mechanism is shown: validate the start line on the `symbols` read path as the edit path does.

## Status: zombie

Set 2026-11-12 by session `5f715d8f`, on the owner's instruction ("da", to closing it as not reproduced for `symbols`). Not `fixed` and not `wontfix`: the root cause is not confirmed.

- **Last observed:** 2026-11-10, in the retained BFS identity worktrees (the parent and an independent reviewer both saw it).
- **Why zombie:** 2026-11-10 and 2026-11-11 checks did not reproduce it for `symbols` on current bytes (see `## Root cause` and the measured section). `symbols` resyncs through `document_symbols`; the stale paths found were `symbol_at` and cross-file `references` (`a73c49456056fb92`, `58a45860b802a477`), fixed by `43c4ca9df`. Whether that fix explains the original report is a lead, not a finding: the report used `symbols`, and I did not run `symbols` on stdio or against the original worktree state.
- **Re-open trigger:** a `symbols` response whose range or body disagrees with the file's bytes, on a binary that contains `43c4ca9df`. Capture, BEFORE any flush or edit, the raw `symbols` response, the file's mtime and the language server's own `textDocument/documentSymbol` answer, as in `## Resume — 2026-11-10`.
