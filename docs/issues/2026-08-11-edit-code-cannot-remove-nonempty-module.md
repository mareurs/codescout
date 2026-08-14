---
id: '2c8690b597f76fa4'
kind: bug
status: open
title: 'BUG: edit_code cannot remove or relocate a non-empty module, and edit_file''s "fn " content filter blocks pure deletions too'
tags:
- edit-code
- edit-file
- module-removal
- tool-friction
closed: null
opened: 2026-08-11
owner: marius
related: []
severity: medium
---

## Summary

Removing or relocating a non-empty module is not reliably possible with `edit_code` alone, and `edit_file` is not a safe fallback either. `edit_code(insert, position="after")` can only ever land within the anchor's own enclosing scope, so it cannot escape a block to place a new module at file scope. `edit_code(remove/replace, symbol=<module>)` fails on a module with children — both on genuine name ambiguity and, separately, on a "would drop sibling symbols" guard that fires even when the "siblings" are the target module's own children, not accidental collateral. `edit_file`'s "fn " content filter fires on deletions (`new_string=""`) just as much as insertions, so it refuses to delete a function-bearing block even when nothing is being added.


### Update 2026-08-14 — a fourth gap, now fixed; and two corrections to this file

**A fourth gap, unrecorded here, is fixed in `138de7c5`.** The keyword match was
unanchored *within* a line, so a keyword buried in an identifier tripped the
guard: `let via_trait = seam(&a);` matched `trait `, `let my_fn = pick(&b);`
matched `fn `, `self.inner_impl (x)` matched `impl `. Each was reported as a
symbol definition and the only way through was renaming the variable. Fixed by
requiring a word boundary before the match (the keywords already carry a trailing
space, which supplied the right-hand one). Four tests, mutation-verified.

**Correction 1 — "the `\"fn \"` content filter" understates what the guard does.**
It is not a whole-content substring match. It is diff-aware
(`lines_only_in`, so a keyword on a line byte-identical in old and new is treated
as an unchanged anchor), gated on the string being multi-line, and comment-aware
(`find_def_keyword` skips `//`, `/*`, `*`, `#` lines, with its own test). Anyone
working this file should read `guard_structural_rewrite`
(`src/tools/edit_file/mod.rs`) before assuming a simpler mechanism.

**Correction 2 — § Fix candidate (3) is too broad as written, and is downstream of
(2).** It proposes making the filter "check only `new_string`". But the
old-string route exists to block *rewriting an existing symbol via raw text
replacement*, which is BUG-027's LSP-range-corruption case; dropping it would
permit far more than deletions. The narrow version is to exempt
`new_string.is_empty()` specifically — a pure deletion splices nothing.

And it is only worth doing at all because `edit_code`'s removal is broken for
modules: gap (3) bites *because* gaps (1)/(2) do. `edit_code(remove)` already
handles function-bearing code correctly, so fixing (2) largely dissolves (3)'s
motivation. Sequence accordingly — (2) first, then re-ask whether (3) is still a
real gap.
## Symptom (Effect)

Goal: move a `#[cfg(test)] mod selection_tests { ... }` (14 leaf test functions) from mid-file to end-of-file (required by clippy's `items_after_test_module` under `-D warnings`).

- `edit_code(insert, symbol="impl RetrievalClient", position="after")` → `"cannot determine end of 'impl RetrievalClient' for insert-after — AST parse failed"` on an otherwise syntactically valid file (two cfg-gated same-named `qdrant_code_store` overloads confusing the boundary detector).
- `edit_code(insert, symbol="RetrievalClient/project_has_chunks", position="after")` → correctly refused as unbalanced (would orphan the impl's own closing brace) — correct behavior, but it means "insert after a leaf" can never by itself escape an enclosing block.
- `edit_file` on the same operation (pure deletion, `new_string=""`) → `"edit contains a symbol definition ('fn ')"`, regardless of whether the change is a pure deletion or a rename.
- What worked: `edit_code(replace, symbol="impl RetrievalClient", ...)` with a body extending past the impl's own closing brace correctly landed the module at file scope — but this necessarily created a second, ambiguously-named `selection_tests` module. `edit_code(remove/replace, symbol="selection_tests")` then failed on both name ambiguity AND the sibling-drop guard (which fired even though the "siblings" were the target module's own children).

## Reproduction

1. In a file with an `impl <Struct> { ... }` block followed later by a `mod <name> { ... }` containing ≥1 leaf function, attempt to relocate the module to end-of-file via `edit_code(insert, ...)`.
2. Observe the AST-parse-failure / unbalanced-brace refusals above.
3. Fall back to `edit_code(replace, symbol="impl <Struct>", body=<impl body + trailing module text>)` — succeeds but creates a duplicate-named module.
4. Attempt `edit_code(remove, symbol=<module-name>)` on the now-duplicated module — fails.
5. Attempt `edit_file` to delete the module's now-redundant original occurrence — fails on the "fn " content filter.

Branch: `feat/local-onnx-query-path`, Task 7 (`src/retrieval/client.rs`, `selection_tests` module).

## Environment

codescout `feat/local-onnx-query-path`, Linux, MCP stdio, rust-analyzer LSP.

## Root cause

Three independent gaps compound:

1. `edit_code(insert, position="after")` can only ever land within the target symbol's own enclosing scope — there is no "insert at file scope regardless of anchor nesting" mode.
2. The "would drop sibling symbols" guard (a pre/post-write AST name-set diff) does not distinguish "this write drops code the caller didn't ask to touch" from "the target being removed/replaced has children, which are its own subtree, not siblings." It currently treats a non-empty module's own children as if they were accidental collateral of removing the module.
3. `edit_file`'s "contains a symbol definition ('fn ')" content filter checks `old_string` (what is being replaced/deleted), not just `new_string` (what would be introduced) — so it blocks legitimate deletions of function-bearing code, not only insertions of new ones.

*Inferred from tool responses in one session — none of the three guard implementations were read this session.*

## Evidence

Quoted from `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-7-report.md` § "Surprises / tool friction worth recording":

> This is a real gap worth a bug file... **removing or relocating a non-empty module is not reliably possible with `edit_code` alone**, and `edit_file` is not always the correct fallback because its "fn " content filter fires on deletions too, not just insertions of new function bodies.

## Hypotheses tried

None — the 14-step leaf-by-leaf workaround (rename the `mod` line via `edit_file` to a unique name, `edit_code(remove, ...)` each of the 14 now-uniquely-addressable leaf functions individually, then `edit_file` the now-empty module shell) was applied on notice rather than investigating the three guards.

## Fix

Not implemented. Candidates, one per gap: (1) an explicit "insert at file/module scope" mode for `edit_code(insert)` that walks up past the anchor's enclosing blocks; (2) scope the sibling-drop guard to exclude a target's own descendant subtree when the target itself is what's being removed/replaced; (3) make `edit_file`'s "fn " filter check only `new_string` (what would be introduced), not `old_string`, so pure deletions of function-bearing code are not blocked.

## Tests added

None.

## Workarounds

To relocate or delete a non-empty module: (1) if renaming to a unique name is enough, use a single-line `edit_file` rename of the `mod` declaration (explicitly allowed per the tool's own hint); (2) to actually remove content, `edit_code(remove, ...)` each leaf child individually (reliable — no guard tripped on leaf removal); (3) only once the module is empty does `edit_file` accept deleting the shell (no more `fn` content for the filter to object to).

## Resume

Still the sibling-drop guard (gap 2) — it degrades a correct operation (removing a
module's own children as part of removing the module) into a false positive,
rather than being a missing feature. Find its implementation (search for "would
drop sibling symbols" / "would have dropped sibling symbols" in the `edit_code`
source) and scope it to exclude a target's own descendant subtree.

Then re-ask whether gap (3) is still real: see § Summary → *Correction 2*. Gap (4)
(the unanchored keyword match) is fixed in `138de7c5`; do not re-derive it.

One more `edit_code` defect surfaced 2026-08-14 while working around this one, and
it belongs with gap (1)/(2) rather than here: `edit_code(insert, position="after")`
cannot bound a method inside a trait impl that is itself nested in a test fn —
*"cannot determine end of 'embed' for insert-after — AST parse failed"* — on a file
with no syntax errors, and the error's hint blames syntax errors misleadingly. See
`docs/issues/2026-08-11-edit-code-no-disambiguator-for-duplicate-name-path.md`
§ Update 2026-08-14, where it is recorded with its workaround
(`action="replace"` on the whole impl object).
## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-7-report.md` § "Surprises / tool friction worth recording"
- `src/retrieval/client.rs` — the file the relocation was performed on
