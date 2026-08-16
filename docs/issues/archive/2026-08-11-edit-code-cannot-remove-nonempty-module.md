---
id: '4ed74d1556284d90'
kind: bug
status: fixed
title: 'BUG: edit_code cannot remove or relocate a non-empty module, and edit_file''s "fn " content filter blocks pure deletions too'
tags:
- edit-code
- edit-file
- module-removal
- tool-friction
closed: 2026-08-15
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

All four gaps are now closed or refuted.

| Gap | Outcome | Where |
|---|---|---|
| 1 — insert cannot reach file scope | **refuted** — capability existed, now asserted | `80350afe` |
| 2 — remove drops a module's own children | **fixed** | `3baa993d` |
| 3 — `edit_file` refuses pure deletions | **wontfix-by-design** — exemption tried and reverted | `80350afe` → `9b902e0a` |
| 4 — keyword matched mid-identifier | **fixed** | `138de7c5` |

Gap 4 was never in the original filing; it surfaced while working gap 2. The one
item still open is the § Follow-up below, which this file already scoped out as
needing its own decision.

### Gap 2 — done

Root cause at `src/symbol/edit.rs:420`: the sibling-drop filter excluded the
target's own `name_path` but not its **descendants**, so every nested symbol was
in `pre_set`, absent from `post_set`, and counted as collateral. Reproduced
end-to-end first; the error was verbatim:

```
edit_code remove('doomed') would have dropped sibling symbols: doomed/inner.
The range overshot into adjacent code (likely a stale LSP range). File restored.
```

`split_target_subtree` (`src/symbol/edit.rs`) separates descendants from true
siblings by the `<target>/` name-path prefix — the form the AST extractor already
uses for nesting — and the `remove` call site passes the sibling-only set. A
`/`-delimited prefix cannot catch a differently-named neighbour (`mod_a_sibling`
does not start with `mod_a/`), which is asserted.

Three scoping decisions worth keeping:

1. **`remove` only.** For `replace` the caller supplies the new body, so a child
   that vanished is *either* intent or an accident, and today's stricter behaviour
   is the safe default. Widening it would silently lose protection rather than fix
   anything. See the follow-up below.
2. **`corruption_verdict` untouched.** It is safety-critical, pure, and heavily
   documented; the *call site* is where the caller's intent is known. Its existing
   target-exclusion stays the single place that rule lives.
3. **Descendants are reported, not just permitted.** `removed_descendants` in the
   response names what went with the target — expected, but a caller should not
   have to re-read the file to learn the scope of their own removal. Mirrors the
   librarian's `replaced_subsections`.

### Gaps 1 and 3 — resolved 2026-08-15 (`80350afe`)

This file's own sequencing advice — fix (2) first, then re-ask whether the rest
are real — was right, and re-asking changed both answers.

**Gap 3 — wontfix-by-design.** Implemented as an exemption in `80350afe`, then
**reverted the same day in `9b902e0a`.** Recording both, because the reversal is
the useful part.

The exemption made `guard_structural_rewrite` return `Ok(())` when
`new_string.is_empty()`, reasoning that route 1 blocks *rewriting* and a deletion
is not a rewrite. The full test suite refuted it:
`edit_file_warns_hint_suggests_remove_when_new_empty` asserts that exact refusal,
and `infer_edit_hint` carries bespoke machinery to route the deletion case to
`edit_code(action='remove')`. **The block is a contract with purpose-built
support, not an oversight.** This file described it as an oversight, and I took
that at face value.

The supporting argument does not survive either. It was *"`edit_code(remove)` is
unavailable exactly when its AST/LSP naming fails"* — true, but those failures
are two separate open bugs (duplicate `name_path`; `symbols` resolving a path
`edit_code` rejects). The fix for a dead end is to open the road, not to remove
the guardrail beside it. Gap 3 is folded into that name-path work.

A test now records the decision, since from the outside it did not look like one:
`guard_blocks_pure_deletion_of_a_nonempty_module`, pointing at its hint sibling.

**The distinguishing question, worth keeping:** when a test blocks a change, does
it assert a **cause** or a **contract**? Earlier in this same cohort a test
pinned a false *diagnosis* and correcting it was right (`cafa4b37`). This one
pins a contract, so the code was wrong. Same surface, opposite verdicts — the
test's subject decides, not its resistance.

**Gap 1 — not a defect. The capability already existed.** The claim was that
`insert` cannot escape the anchor's enclosing scope, so a module cannot be placed
at file scope. The first half is true and deliberate; the conclusion does not
follow.

`do_insert`'s clamp lives inside `if let Some(parent) = find_parent_symbol(...)`,
and `find_parent_symbol` opens with `if !child_name_path.contains('/') { return
None; }`. Its doc comment says so outright: *"Returns `None` for top-level
symbols"*. **A top-level anchor is never clamped.** Placing an item at file scope
is therefore done by anchoring on a file-scope sibling, which every file has. A
second route exists too: `edit_file(insert="append"|"prepend")` writes at file
start/end and never calls `guard_structural_rewrite` at all.

Loosening the clamp — the fix this gap implied — would have reopened two recorded
regressions: the 2026-06-05 Python trailing-statement orphaning and the
2026-07-19 insert-into-a-sibling's-multiline-macro. The clamp is load-bearing.
### Follow-up worth its own decision

Should `replace` also stop treating a target's missing descendants as dropped
siblings? Its error message ("the range overshot into adjacent code (likely a
stale LSP range)") is a **wrong diagnosis** when the caller's own replacement body
simply defined fewer children — the same misdiagnosis reasoning
`corruption_verdict`'s own doc comment uses to justify its ordering. But relaxing
it lets an incomplete body silently drop children, which nothing else catches. The
librarian solved the identical dilemma by *reporting* (`replaced_subsections`)
rather than refusing; the same shape would work here. Not done — it trades a
safety guard for ergonomics and deserves a deliberate call.
## Tests added

**Gap 3 — one test, `guard_blocks_pure_deletion_of_a_nonempty_module`.** The
five tests written for the exemption went with it. What remains records the
*decision* to refuse, with a pointer to
`edit_file_warns_hint_suggests_remove_when_new_empty`, which pins the hint the
refusal emits. The refusal is only defensible because it routes somewhere that
works, so the two belong read together.

**Gap 1 — `insert_after_top_level_symbol_lands_at_file_scope`**
(`tests/symbol_lsp.rs`). Inserts a module after a top-level `fn` and asserts the
emitted line equals `"mod fresh {"` exactly. Asserting the exact line rather than
`contains` is what makes it a *scope* assertion instead of a presence assertion —
an indented, re-based module would pass a `contains` check.

The capability was real but unasserted before this, which is one refactor away
from not being real — the same failure shape as gap 2's.

**Gap 2 (2026-08-14, `3baa993d`)** — `edit_code_remove_deletes_a_non_empty_module_and_names_its_children`
plus the AST-nesting test that exposed the descendant-vs-sibling confusion.

Gate: `cargo clippy --workspace --all-targets -- -D warnings` clean; all 29
`guard_` tests pass.
## Workarounds

To relocate or delete a non-empty module: (1) if renaming to a unique name is enough, use a single-line `edit_file` rename of the `mod` declaration (explicitly allowed per the tool's own hint); (2) to actually remove content, `edit_code(remove, ...)` each leaf child individually (reliable — no guard tripped on leaf removal); (3) only once the module is empty does `edit_file` accept deleting the shell (no more `fn` content for the filter to object to).

## Resume

Closed. One item deliberately survives this file: § Follow-up worth its own
decision (should `replace` stop treating a target's missing descendants as
dropped siblings?). It was scoped out on purpose — it trades a safety guard for
ergonomics. Gap 3's reversal is a direct argument for leaving it alone.

Three lessons, all about how this file reasoned rather than what it found:

1. **Sequencing worked.** The instruction to fix gap 2 first and then re-ask
   dissolved gap 3's motivation and exposed gap 1 as a non-defect. A bug file
   that records *dependencies between its own gaps* is worth more than one that
   lists them flat.
2. **"Tool X cannot do Y" needs the negative checked, not inferred.** Gap 1 was
   derived from watching `insert` fail against a *nested* anchor and
   generalising to all anchors. One read of `find_parent_symbol`'s doc comment —
   which states the top-level case in plain English — would have bounded it. Same
   shape as R-82: the observed behaviour was real, the explanation invented.
3. **A refusal described as an oversight deserves a search for its test.** Gap 3
   read as an unnoticed side effect. It had a dedicated test *and* dedicated hint
   machinery. Both were one `grep` away, and finding them would have turned a
   day's implement-then-revert into a one-line triage.
## References

- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/task-7-report.md` § "Surprises / tool friction worth recording"
- `src/retrieval/client.rs` — the file the relocation was performed on
