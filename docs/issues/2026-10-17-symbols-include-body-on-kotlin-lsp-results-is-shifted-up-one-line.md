---
kind: bug
status: wontfix
title: 'BUG: symbols(include_body=true) on a Kotlin member returns the lines shifted up by its KDoc length, so an expression-bodied member loses its last line'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
closed: 2026-10-17
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

Not a codescout defect. The bodies codescout returns are complete; what dropped the last line was the capture of the reply.

`mcp call` prints the tool's text block and the injected guide block (`<!-- auto-injected get_guide(...)`) back to back with no newline between them, so the tool's last line and the marker share one line. measured 2026-10-17 (`repro5.sh`, plain `mcp call` output of `symbols name=Catalog/add include_body=true`, `grep -o` around the marker): `          }<!-- auto-injected`. Any line-oriented strip of the guide block (`sed '/auto-injected/,$d'`, `grep -v`, `head`) deletes that line together with the marker, which is exactly one line off the end of every reply. The smoke script's own `RESULT="${RESULT%%<!-- auto-injected*}"` cuts at the marker and keeps the line.

The reported numbers fit this and not a shifted slice: for each case the body had `end - start + 1` lines because the header's `start` is the NAME line (`selectionRange`) while the body starts at the KDoc (`range.start`), so a body that is correct in both ends is one line longer than the header's range and loses exactly its last line when the last line is cut.

Verified three ways, measured 2026-10-17:

1. The same calls with `mcp call -f json` and `content[0].text` (no line stripping): `Catalog/add` returned all 4 lines including `}`, `search` all 3 including `items.filter { it.searchText().contains(query) }`, `stats` both lines.
2. The raw `textDocument/documentSymbol` bytes for `Catalog.kt` (`tests/fixtures/lsp-replies/kotlin-lsp-catalog-document-symbol.json`): `add` `range` 14–17 / `selectionRange` 15, `search` 19–21 / 20, `stats` 23–24 / 24. `range` includes the KDoc and ends on the last line; kotlin-lsp is right.
3. `symbol_to_json` (`src/symbol/query.rs`) slices `lines[range_start..=end_line]` and `body_start_line` names the KDoc line. Replayed through `MockLspClient`, the 4 characterisation tests below pass on unmodified code and go red when the end of the slice is cut by one line (`add` body loses `    }`).

## Evidence

- `repro.sh` (capture with `sed '/<!-- auto-injected/,$d'`): reproduces the issue's three bodies exactly, `Catalog/stats` printing only `/** Get catalog statistics. */`.
- `repro4.sh` (capture with `-f json` + `content[0].text`): the same calls, complete bodies.
- `repro5.sh`: `6:          }<!-- auto-injected` in the plain output.

## Hypotheses tried

1. **Hypothesis:** the body slice starts at the KDoc while its length comes from the LSP range. **Test:** read `symbol_to_json` and `editing_start_line`; replay the real reply. **Verdict:** rejected; start and end are both taken from `range`.
2. **Hypothesis:** the capture lost the last line. **Test:** `-f json`, plain bytes. **Verdict:** confirmed.

## Fix

None needed in codescout. Status `wontfix`: there is no defect to fix. The two consequences are in the Kotlin smoke script, fixed on branch `worktree-agent-af85b780928eaa450` at `b451979a` (patch-id `a2068783a60e078be34de6abcabd136703aec224`): its `include_body` step targets `Catalog/search` again and asserts the expression-bodied member's last line, and `call()` documents why the marker must be cut by parameter expansion.

Open suggestion, not done: the guide block could start on its own line in `mcp call` text output; that join is made by the `mcp` CLI, not by codescout.

## Tests added

Characterisation tests in `src/tools/symbol/kotlin_lsp_tests.rs`, green on unmodified code and red under an end-of-slice off-by-one (mutation measured 2026-10-17: `add` body lost `    }`, `createNamedCatalog` lost its closing `}`): `include_body_on_a_kotlin_lsp_member_spans_the_kdoc_through_the_last_line` (`add`, `search`, `stats`, `body`, `body_start_line`, `start_line`, `end_line`, and the rendered text), `include_body_on_a_kotlin_lsp_function_with_a_long_kdoc_spans_all_of_it`. The Kotlin smoke's `include_body` step now asserts `search`'s last line.

## Workarounds

When capturing `mcp call` text output, cut the guide block with `${RESULT%%<!-- auto-injected*}` or use `-f json`.

## Resume

N/A: closed as not a defect.

## References

- `tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt`
- `tests/mcp-smoke-kotlin.sh` (`test_symbols_with_body`)
