---
kind: bug
status: open
title: call_graph's Phase B workspace walk compares the LSP languageId to the AST language, so a .tsx or .jsx seed examines no file at all
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-10-17
severity: low
---

## Summary

`CachedResolver::lookup_pos_via_ts_workspace` selects candidate files with `crate::ast::detect_language(path) == self.lang`. `self.lang` is the second value of `get_lsp_client`, which is `lsp_language_id(...)`: `"typescriptreact"` for `.tsx` and `"javascriptreact"` for `.jsx`. `detect_language` answers `"tsx"` / `"jsx"` for those files, so the equality never holds and the walk parses zero files. Pre-existing; found while reviewing the scan-cap fix. Not fixed here.

## Class fit

A selector (the same-language filter) narrower than the population it names: for a React seed it selects the empty set, so the fallback that exists to place a symbol the LSP could not place can never place one. It is the ADR 2026-08-27 "ran and examined nothing" case, which is a different outcome from the scan-cap members: no cap is reached, so `WorkspaceScan::CutShort` can never be returned and the result is a plain `NotFound`.

## Symptom (Effect)

For a `.tsx` or `.jsx` seed, any non-seed symbol the LSP does not place through `workspace/symbol` is recorded in `not_found` after a walk that read no file, and its edges are dropped with no warning. A `.ts` seed also never examines sibling `.tsx` files (`"typescript"` vs `"tsx"`), and a `.js` seed never examines `.jsx`.

Measured 2026-10-17 with a throwaway probe (deleted after the run): four `.tsx` files, one of them `export function ghost() { return 1; }`, resolver cap 2:

```
PROBE5  lang=typescriptreact found=None not_found={"ghost"} cut_short={}
PROBE5b lang=tsx             found=Some(("…/a.tsx", 0, 7)) cut_short={}
```

The same files, the same symbol, found with `lang = "tsx"` and not found with the id `get_lsp_client` actually hands the resolver.

## Reproduction

`src/tools/symbol/call_graph/mod.rs`, `lookup_pos_via_ts_workspace`: `if file_lang != self.lang { continue; }`, with `file_lang` from `detect_language` (`src/ast/mod.rs:65-75`, `"tsx" => Some("tsx")`, `"jsx" => Some("jsx")`) and `self.lang` from `CallGraph::call` (`let (client, lang) = get_lsp_client(...)`), where `get_lsp_client` returns `lsp_language_id(lang)` (`src/fs/mod.rs`, `src/lsp/servers/mod.rs:191-198`: `"tsx" => "typescriptreact"`, `"jsx" => "javascriptreact"`). `references` is not affected: it compares `detect_language` with `detect_language` (`raw_lang`).

## Root cause

Two id spaces share a field name. `CachedResolver::lang` is documented as the file language but carries the LSP languageId; Phase A (`lookup_pos_via_ts_in_seed_files`) re-derives the language from the path and is correct, Phase B trusts the field. Measured 2026-10-17 by the probe above.

## Fix

Not designed. Candidate: keep both on the resolver (`lang` for the LSP calls, an AST-language key for the walk) and compare the walk's `detect_language` against a grammar family (`tsx`/`typescript`, `jsx`/`javascript`) so a React project's mixed `.ts`/`.tsx` tree is one population. Then add the same-population test next to `lookup_pos_falls_back_to_ts_workspace_when_def_in_sibling_file`, with a `.tsx` fixture.

## Tests added

None — filing only.

## Resume

1. Re-run the probe shape above (a `.tsx` fixture through `resolver_over` with `lang = lsp_language_id("tsx")`) before working it; it prints `found=None`.
2. Decide the family mapping (`.ts` + `.tsx`, `.js` + `.jsx` + `.mjs` + `.cjs`).

## References

- `src/tools/symbol/call_graph/mod.rs` (`lookup_pos_via_ts_workspace`, `CachedResolver::lang`)
- `src/ast/mod.rs:65-75`
- `src/lsp/servers/mod.rs:191-198`
- `src/fs/mod.rs` (`get_lsp_client`)
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
