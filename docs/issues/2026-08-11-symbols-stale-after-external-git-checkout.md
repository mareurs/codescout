---
id: '197843bd0a5c4fd5'
kind: bug
status: open
title: 'BUG: symbols() keeps reporting pre-checkout content after an external git checkout, until an explicit LSP flush'
tags:
- symbols
- lsp
- staleness
- tool-friction
closed: null
opened: 2026-08-11
owner: marius
related:
- '9e649674c95cd7bd'
severity: medium
---

## Summary

After running `git checkout -- <file>` from outside codescout's own edit tools (to revert an in-progress edit that had produced a duplicate symbol — see `docs/issues/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md`), `symbols()` continued to report the *stale*, pre-checkout (already-duplicated) file content for several subsequent calls — even though `git diff` and `read_file(force=true)` both confirmed the on-disk content was correctly reverted. `workspace(action="status", post_compact=true)` ("flush all LSP clients") repaired it; `symbols()` matched disk again immediately after.

## Symptom (Effect)

```
git checkout -- src/retrieval/embedder.rs      # reverts a duplicated-impl mistake
git diff -- src/retrieval/embedder.rs           # clean — file matches HEAD
symbols(path="src/retrieval/embedder.rs")       # still lists the pre-checkout duplicated impl blocks
...
workspace(action="status", post_compact=true)   # "flushed": true
symbols(path="src/retrieval/embedder.rs")       # now matches disk
```

## Reproduction

1. Make an `edit_code` edit that grows or duplicates a symbol in a file.
2. `git checkout -- <file>` to revert, from outside codescout's own tools.
3. `symbols(path=<file>)` — reports the pre-checkout (stale) shape.
4. `workspace(action="status", post_compact=true)` — flushes all LSP clients.
5. `symbols(path=<file>)` — now matches disk.

Branch: `feat/local-onnx-query-path`, Task 5.

## Environment

codescout `feat/local-onnx-query-path`, Linux, MCP stdio, rust-analyzer LSP mux.

## Root cause

Unknown in detail — not instrumented this session. Inferred: the LSP client's in-memory document state is synced only by codescout's own write tools sending `did_change` notifications; a filesystem mutation made by a process other than codescout (a bare `git checkout`, another editor, a generator) has no notification path to the LSP, so the server keeps serving its last-synced buffer until something forces a flush. *Inferred from observed behavior across two tool responses — the LSP sync code was not read this session.*

## Evidence

Quoted from `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)":

> Second hazard from the same revert: after the external `git checkout`, `symbols()` kept reporting the *stale* (duplicated) state for several calls even though `git diff`/`read_file(force=true)` confirmed the on-disk content was correctly reverted — the LSP's in-memory document model doesn't observe filesystem changes made outside codescout's own edit tools. Fixed by calling `workspace(action="status", post_compact=true)` ("flush all LSP clients"), after which `symbols()` matched disk again.

## Hypotheses tried

None — the mitigation (`post_compact=true`) was applied on notice, without reading the LSP sync path.

## Fix

Not implemented. Candidate: detect file mtime/hash drift between the LSP's last-known state and disk on a `symbols()`/`edit_code` read, and auto-flush that one file's client rather than requiring the caller to already know to call `post_compact`.

## Tests added

None.

## Workarounds

After any edit made outside codescout's own tools (external `git checkout`, `git stash`, another editor, a build step that rewrites generated code), call `workspace(action="status", post_compact=true)` before trusting `symbols()`/`edit_code` output for the affected file(s).

## Resume

Find where codescout's own write tools send `did_change` to the LSP mux (`src/lsp/`), and check whether a cheap mtime/hash check could be added to `symbols`'s read path to detect drift and self-heal per-file, rather than requiring the caller to know about `post_compact`.

## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-5-report.md` § "A real tool hazard hit mid-fix (worth recording)"
- Related but a different mechanism (stale `range_start_line` inside a single session's own prior edits, not staleness from an external write): `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`

