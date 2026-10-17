---
kind: bug
status: fixed
title: 'BUG: symbols(path) on a Kotlin file lists only some members and no top-level functions (Catalog.kt shows 4 of 5 members and none of 4 top-level functions)'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
severity: low
closed: 2026-10-17
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

Three separate things, none of them a partial `documentSymbol` from kotlin-lsp for a Gradle-unimported project.

**1. The fixture hid its own functions.** `tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt` had `(from the opening /**)` inside the KDoc of `createNamedCatalog`. Kotlin block comments nest, so that `/**` opens a second comment that nothing closes and the rest of the file is comment text. measured 2026-10-17: kotlin-lsp returned 2 top-level symbols (`Catalog`, `createDefaultCatalog`) for the file as committed and 5 once that phrase was reworded (`lspdrive.py`, raw replies); tree-sitter (`ast::extract_symbols`) returned only `Catalog` as committed and all 5 after. `createNamedCatalog`, `searchAsync`, `toSearchText` were never visible to any tool. Fixed by rewording the phrase on its own line (no line numbers move).

**2. The "missing `stats`" was the capture, not the tool.** Same cause as `docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md`: the guide block follows the last row with no newline, so a line-oriented strip removed the last row. With `-f json` the overview ends with `Method  24  Catalog/stats`.

**3. A single-file overview was rendered by the search renderer, which dropped the `lsp: warming` marker.** `Symbols::format_compact` (`src/tools/symbol/symbols.rs`) chose the overview renderer on `directory` / `pattern` / `files`; a single-file overview is `{file, symbols, ...}` and has none, so it went to `format_search_symbols`, which prints neither `lsp` nor `hint`. The live overviews in this issue were tree-sitter output from a cold server (kinds `Class CatalogStats`, not LSP's `Struct`; reproduced in-process: test `a_cold_language_server_overview_says_it_was_served_from_tree_sitter` printed exactly the issue's rows before the fix) and nothing in them said so. The dispatch was written that way in `47af2676` (2026-05-01, merge of find_symbol + list_symbols) and `format_overview_symbols`'s file mode, tested directly, was unreachable from the tool since.

## Evidence

- `lspdrive.py` against the committed fixture: `[('Catalog', 6, 25, 7), ('createDefaultCatalog', 27, 28, 28)]`; against the reworded one: `... ('createNamedCatalog', 30, 44, 41), ('searchAsync', 46, 48, 47), ('toSearchText', 50, 51, 51)`.
- Red run of `kotlin_lsp_tests` on the unfixed tree: `the_kotlin_catalog_fixture_parses_to_every_top_level_symbol_it_declares` left `["Catalog"]`; `a_cold_language_server_overview_says_it_was_served_from_tree_sitter` printed `Catalog.kt (1)` / `Class 7-25 Catalog` / `Property 9 Catalog/items` / `Class 12 Catalog/CatalogStats` / `Method 15-17 ...` / `20-21` / `24`.
- After the fix, live (`repro6.sh`): `Catalog.kt — 5 symbols`, `[lsp warming] Language server is starting; symbols served from tree-sitter. ...`, and `Function  createNamedCatalog  L41-44`.

## Hypotheses tried

1. **Hypothesis:** kotlin-lsp returns a partial `documentSymbol` for an unimported project. **Test:** raw reply. **Verdict:** rejected for `Catalog`'s members (all 7 children plus `stats`); the top-level shortfall was the fixture's comment.
2. **Hypothesis:** codescout drops rows. **Test:** replay the raw reply through `MockLspClient`. **Verdict:** rejected, all rows pass through (`a_kotlin_lsp_reply_overview_lists_every_member_and_top_level_function`).
3. **Hypothesis:** the reply is tree-sitter's. **Verdict:** confirmed (cause 3).

## Fix

- `Catalog.kt` fixture reworded (cause 1).
- `Symbols::format_compact` now treats a result with `file` and no `files_count` as an overview (cause 3): a search always sets `files_count` and also hoists `file`, so `file` alone cannot decide. Single-file overview text therefore changes from the search renderer's `<file> (N)` / `Class  7-25  Catalog` / `Property  9  Catalog/items` to the overview renderer's `<file> — N symbols` / `Class   Catalog  L7-25` / `Property  items  L9`: member rows print the member's name, not its name-path (this is the format `docs/superpowers/plans/2026-03-01-dual-audience-output-design.md` specifies and the format before `47af2676`). The Kotlin smoke's overview assertions were moved to the new shape.

Fix commits on branch `worktree-agent-af85b780928eaa450` (not on experiments): `b451979a` (patch-id `a2068783a60e078be34de6abcabd136703aec224`) and the follow-up `0575b868` (patch-id `4cc0ce8698a8373c307fb659c13b52c94538ad5b`).

The renderer change alone dropped what `include_body` / `include_docs` ask for: `format_symbol_tree` printed names and ranges, and on the inline path (JSON under 10 KB) there is no buffer to reach a body from (found in review: `symbols(path, include_body=true)` showed no body lines). `0575b868` makes the tree print a symbol's signature, the rest of its docstring and its body (a container's own body is not repeated per member) under its row; the warming marker is unchanged. A `members_hint` branch added with it was dead (only `focus_single_symbol`, called only by the search path, sets it) and was removed in `4c6b9e5b`. Decision flagged: `format_overview_symbols_renders_per_symbol_doc` pinned "only the first doc line belongs in overview"; with `include_docs=true` promised in overview mode it now asserts the full docstring follows under the row.

**Scope: this applies to directory and glob overviews too, not only a single file.** `format_symbol_tree` serves all three, and the `0575b868` commit message says "single-file overview"; the behaviour is wider. Kept deliberately: restricting it would leave `include_body` / `include_docs` still dropped on the inline path for a small directory, the same defect in another shape. Measured 2026-10-17 on a 5-file Rust directory (`src` of a scratch crate holding `src/lsp/mux/*.rs`, `transport.rs`, `symbols.rs`, rust-analyzer warm), baseline release 5145544f vs this branch: `symbols(path="src")` 3475 B -> 5480 B (+58%, signatures on every row); `symbols(path="src/**/*.rs")` 3505-3582 B -> 3188-3449 B (that result is buffered, so the compact summary is capped near 3 KB whatever the rows carry); `include_body=true` on the directory 3328 B -> 3315 B (buffered, same cap); single file `transport.rs` 613 B -> 794 B. So the cap is respected where a result is large; a small directory that stays inline grows by its signatures. The tool description and `long_docs` say nothing about overview row shape, so nothing there needed changing.

## Tests added

`src/tools/symbol/kotlin_lsp_tests.rs`: `the_kotlin_catalog_fixture_parses_to_every_top_level_symbol_it_declares` (red before), `a_single_file_overview_is_rendered_as_an_overview_with_its_warming_marker` (red before), `a_cold_language_server_overview_says_it_was_served_from_tree_sitter` (red before), `a_single_file_overview_with_include_body_and_include_docs_shows_both` and `a_cold_overview_with_include_body_keeps_the_warming_marker_and_the_bodies` (both red against `b451979a`: bodies missing), `a_capped_single_file_overview_is_rendered_as_an_overview` (red when the discriminator keys on `total`), `a_search_that_hoists_its_file_is_still_rendered_as_a_search` (guard), `a_kotlin_lsp_reply_overview_lists_every_member_and_top_level_function`. `src/tools/symbol/tests.rs`: `format_overview_symbols_renders_per_symbol_doc` updated as above, and `overview_rows_render_signature_docs_and_body_each_from_its_own_field` (synthetic rows whose text appears nowhere else; red when the top-level signature, the member signature, or the container-body guard is removed). The Kotlin docstring assertion now needs the comment-prefixed `// @param ...` form, which the body text (` * @param`) cannot satisfy. The raw reply is `tests/fixtures/lsp-replies/kotlin-lsp-catalog-document-symbol.json`.

## Workarounds

N/A once fixed.

## Resume

N/A: fixed. Open question for a human, not blocking: whether overview member rows should print name-paths (`Catalog/add`), which the search renderer did and the overview renderer does not.

## References

- `tests/fixtures/kotlin-library/src/main/kotlin/library/services/Catalog.kt`
- `tests/mcp-smoke-kotlin.sh` (`test_symbols_overview_catalog`)
