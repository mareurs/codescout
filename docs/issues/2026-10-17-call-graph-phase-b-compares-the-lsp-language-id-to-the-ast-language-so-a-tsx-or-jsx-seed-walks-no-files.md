---
kind: bug
status: fixed
title: call_graph's Phase B workspace walk compares the LSP languageId to the AST language, so a .tsx or .jsx seed examines no file at all
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-10-17
severity: low
closed: 2026-10-17
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

Re-verified at HEAD `e277135e` before changing anything (2026-10-17): the probe still printed `found=None` for `lang=typescriptreact`, and the new test `a_react_seed_walk_examines_react_files` FAILED (`mod.rs:1316`) against the unmodified walk. The same two-vocabulary comparison also existed one layer down, which the filing did not know: `CachedResolver::lang` is passed on to `resolve_one_hop`, whose tree-sitter fallbacks (`resolve_via_ts`) fed it to `call_kinds_for`, `get_ts_language`, `position_is_call` and `enclosing_function_name`, all of which key on `"tsx"`/`"jsx"`. For a React seed, `direction=callees` answered "requires LSP callHierarchy" and the callers fallback parsed nothing. Both reproduced red (`resolve_callees_via_ts_accepts_the_lsp_language_id_of_react_files` `resolver.rs:958`, `resolve_callers_via_ts_accepts_the_lsp_language_id_of_react_files` `resolver.rs:1009`).

**One vocabulary, converted at the boundary.**

- `src/lsp/servers/mod.rs`: `ast_language_for_lsp_id`, the inverse of `lsp_language_id` (`typescriptreact`->`tsx`, `javascriptreact`->`jsx`, `shellscript`->`bash`). A test round-trips it over every language `detect_language` can return, so a mapping added to one side only reds.
- `src/ast/mod.rs`: `language_family`, which groups `typescript`/`tsx`/`javascript`/`jsx` and leaves every other language as its own family.
- `src/tools/symbol/call_graph/mod.rs`, `lookup_pos_via_ts_workspace`: converts `self.lang` to the AST vocabulary, then compares `language_family(file_lang)` with the seed's family.
- `src/tools/symbol/call_edges/resolver.rs`, `resolve_via_ts`: converts `language_id` once for every tree-sitter helper; the callers fallback parses each reference with its OWN file's language.
- `src/tools/symbol/references.rs`, `corroborate_with_cap`: compares families as well (it already compared AST to AST, but by exact name, so a `.ts` definition used only from `.tsx` was never corroborated).

**Decision, stated: `.ts` seed examines `.tsx`, and JS/JSX are in the same family.** The rest of the code does not group TypeScript one way: the embedding chunker groups `ts`+`tsx` and `js`+`jsx`+`mjs` separately (`src/embed/mod.rs`), while `default_config` serves `typescript|javascript|tsx|jsx` with ONE `typescript-language-server`, and `ts_classifier`, `resolver.rs` and the structural-edit guard (`src/tools/edit_file/mod.rs`) all treat the four as one. Phase B exists to place a symbol the language server could not, and that server answers for all four, so the definition of a symbol a `.tsx` component calls routinely sits in a `.ts` module and the reverse. The LSP server grouping was chosen over the chunker's. It is a design choice and is cheap to change (`language_family` is the one place).

**Other callers that compare the two vocabularies** (grep of `lsp_language_id` and `detect_language`, 2026-10-17): `lsp_language_id` is called from `src/fs/mod.rs` (`get_lsp_client`), `src/symbol/query.rs:602`, `src/tools/symbol/list_overview.rs` (3 sites) and `src/tools/symbol/symbols.rs` (2 sites); in each the result is only handed to LSP calls, never compared with `detect_language`. The only mixed comparison was `lookup_pos_via_ts_workspace`; the only other consumer of the languageId as an AST key was `resolve_via_ts` in `resolver.rs` (fixed above). `src/lsp/client.rs:315` compares `detect_language` with `"python"` (AST to AST, fine).

Fix commits: recorded under Resume.

## Tests added

- `src/tools/symbol/call_graph/mod.rs`: `a_react_seed_walk_examines_react_files` (tsx and jsx, with the id `get_lsp_client` really produces; RED `mod.rs:1316` before the fix), `the_typescript_family_is_one_population_for_the_walk` (ts seed -> .tsx, tsx -> .ts, js -> .jsx, jsx -> .js, tsx -> .js; RED `mod.rs:1347`), `a_rust_walk_does_not_examine_typescript_files` (the family is not every language; green before and after, a guard).
- `src/tools/symbol/call_edges/resolver.rs`: `resolve_callees_via_ts_accepts_the_lsp_language_id_of_react_files` (tsx and jsx; RED `resolver.rs:958`), `resolve_callers_via_ts_accepts_the_lsp_language_id_of_react_files` (RED `resolver.rs:1009`).
- `src/tools/symbol/tests.rs`: `corroborate_scan_treats_the_typescript_family_as_one_population` (RED `tests.rs:6159`), `corroborate_scan_does_not_cross_into_another_language_family`.
- `src/lsp/servers/mod.rs`: `ast_language_for_lsp_id_inverts_lsp_language_id_for_every_detectable_language`, `react_language_ids_map_to_the_ast_react_languages`. `src/ast/mod.rs`: `language_family_groups_only_the_typescript_javascript_languages`.

Mutations, one per guarded site (observed 2026-10-17, each turned exactly the named tests red): the walk's family compare replaced by the old `file_lang != self.lang` -> both `call_graph` react/family tests; replaced by an exact AST-language compare -> only the family test; the `typescriptreact` arm of `ast_language_for_lsp_id` removed -> both servers tests, the resolver callees test, the react and family walk tests; `parse_ts_tree(&src, file_lang)` reverted to `language_id` -> the callers test; the `references` compare reverted to exact `file_lang == lang` -> the `references` family test.

## Resume

N/A once fixed. Two choices for the human to confirm: the family grouping above (TS and JS are one population), and, for the `.ts`-seed-walks-`.tsx` widening, that a larger candidate population draws on the same 5,000-file budget.

## References

- `src/tools/symbol/call_graph/mod.rs` (`lookup_pos_via_ts_workspace`, `CachedResolver::lang`)
- `src/ast/mod.rs:65-75`
- `src/lsp/servers/mod.rs:191-198`
- `src/fs/mod.rs` (`get_lsp_client`)
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
