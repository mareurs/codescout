---
kind: bug
status: open
title: 'BUG: symbols(include_body=true) on a Kotlin member returns the lines shifted up by its KDoc length, so an expression-bodied member loses its last line'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
---

# BUG: symbols(include_body=true) on a Kotlin member returns the lines shifted up by its KDoc length, so an expression-bodied member loses its last line

## Summary

For a Kotlin member with a one-line KDoc, `symbols(include_body=true)` prints the KDoc line and then as many lines as the reported range has, starting at the KDoc line. The reported range is right; the body printed is the same number of lines but begins one line too early, so the last source line is cut. Found 2026-10-17 by the first live run of `tests/mcp-smoke-kotlin.sh`, whose first assertion (`filter` and `searchText` in the body of `search`) could not be satisfied.

## Class fit

No class on the roster was checked one by one, so the sanctioned `cluster/unclassified` is used rather than a forced fit. What the case shows: two parts of one answer (the header's line range and the printed body) disagree about where the symbol starts.

## Symptom (Effect)

`tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt` lines 14-24, and the live replies (2026-10-17, `kotlin-lsp`):

```
14  /** Add an item to the catalog. */
15  fun add(item: T) {
16      items.add(item)
17  }
19  /** Search for items matching a query. */
20  fun search(query: String): List<T> =
21      items.filter { it.searchText().contains(query) }
23  /** Get catalog statistics. */
24  fun stats(): CatalogStats = CatalogStats(items.size, name)
```

- `name: "Catalog/add"`: header `Method 15-17 Catalog/add`; body printed: lines 14, 15, 16. Line 17 (`}`) is missing.
- `name: "search"`: header `Method 20-21 Catalog/search`; body printed: lines 19 and 20. Line 21 (`items.filter ...`) is missing.
- `name: "Catalog/stats"`: header `Method 24 Catalog/stats`; body printed: line 23 only. The `fun stats` line itself is missing.

In each case the body has `end - start + 1` lines and begins at the KDoc line.

## Reproduction

As in `docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md`, then:

```
mcp call symbols -p '{"name": "Catalog/stats", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}' kt-repro
```

## Environment

This machine, `kotlin-lsp` at `/usr/bin/kotlin-lsp`, release build of branch `worktree-agent-a2ef1acc5c69d1c42`. The same calls made through the tree-sitter path (a `symbols(name="Catalog", include_body=true)` reply that listed `Struct`/`Constructor` rows) printed the whole class body correctly, so only the LSP-derived rows were seen wrong.

## Root cause

Not investigated. The observation fits a body slice that starts at the KDoc line while its length is taken from the LSP range, which does not include the KDoc; this was not checked in the source.

## Evidence

The three replies above, each read from a live `mcp call` on 2026-10-17.

## Hypotheses tried

None.

## Fix

Unknown.

## Tests added

N/A: open. `tests/mcp-smoke-kotlin.sh` asserts only text that survives a fix (`fun add(item: T)` and `items.add(item)`), and its comment cites this file.

## Workarounds

Read the range with `read_file(start_line, end_line, force=true)`.

## Resume

1. Reproduce `Catalog/stats`. 2. Find where the body slice start is chosen for LSP rows and where the KDoc is added.

## References

- `tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt`
- `tests/mcp-smoke-kotlin.sh` (`test_symbols_with_body`)
