---
kind: bug
status: open
title: 'BUG: symbols(path) on a Kotlin file lists only some members and no top-level functions (Catalog.kt shows 4 of 5 members and none of 4 top-level functions)'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
---

# BUG: symbols(path) on a Kotlin file lists only some members and no top-level functions (Catalog.kt shows 4 of 5 members and none of 4 top-level functions)

## Summary

The overview of `Catalog.kt` against a live `kotlin-lsp` lists `Catalog` with the members `items`, `CatalogStats`, `add` and `search`, and nothing after the class: not the member `stats` (line 24), and none of the top-level functions `createDefaultCatalog`, `createNamedCatalog`, `searchAsync`, `toSearchText`. The tree-sitter path lists `createDefaultCatalog` for the same file, so the symbol exists. Found 2026-10-17 by the first live run of `tests/mcp-smoke-kotlin.sh`, whose first assertion expected `createDefaultCatalog`.

## Class fit

No class on the roster was checked one by one, so the sanctioned `cluster/unclassified` is used rather than a forced fit. What the case shows: an overview that presents itself as the file's symbol list while leaving some symbols out, with nothing in the reply saying it is partial.

## Symptom (Effect)

Reply of `mcp call symbols -p '{"path": "src/main/kotlin/library/services/Catalog.kt"}'` on 2026-10-17, identical in every one of five calls across three sessions (also with `depth: 2`):

```
src/main/kotlin/library/services/Catalog.kt (1)
  Class  7-25  Catalog
      Property  9  Catalog/items
      Class  12  Catalog/CatalogStats
      Method  15-17  Catalog/add
      Method  20-21  Catalog/search
```

The file has `stats` at line 24 and top-level functions at lines 28, 41, 47 and 51. `Advanced.kt` shows the same shape: header `(3)` and rows for `ISBN`, `LazyBook` and `LazyBook/formattedTitle`, with `createBookWithDefaults` missing. A name lookup for `createNamedCatalog` and one for `searchAsync` (no `path`) each came back with an empty reply, no rows and no `0 matches` line.

## Reproduction

As in `docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md`, then the overview call above, then `{"name": "createNamedCatalog"}`.

## Environment

This machine, `kotlin-lsp` at `/usr/bin/kotlin-lsp`, release build of branch `worktree-agent-a2ef1acc5c69d1c42`, the project a `git archive` copy of the fixture with no Gradle import step run.

## Root cause

Not investigated. Whether `kotlin-lsp` returns a partial `documentSymbol` for a project it has not imported, or codescout drops rows, was not checked.

## Evidence

The reply above; and a `symbols(name="Catalog", include_body=true)` reply in the same sessions that came back from the tree-sitter path (rows typed `Struct` and `Constructor`) and did list `Function 28 createDefaultCatalog`.

## Hypotheses tried

None.

## Fix

Unknown.

## Tests added

N/A: open. `tests/mcp-smoke-kotlin.sh` asserts only `Catalog/add` and `Catalog/search` from this overview, and a fix that lists more members keeps it green.

## Workarounds

`grep` for the declaration, or `symbols(name=...)` when the tree-sitter path answers.

## Resume

1. Capture the raw `textDocument/documentSymbol` reply for `Catalog.kt` from the mux log. 2. Compare with the rows `symbols` prints.

## References

- `tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt`
- `tests/mcp-smoke-kotlin.sh` (`test_symbols_overview_catalog`)
