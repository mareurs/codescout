---
id: fe2de4189ce2abbd
kind: bug
status: fixed
title: edit_code replace with a multi-symbol body adds the extra symbols, and the response does not say so
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
owner: marius
severity: low
---

# BUG: edit_code replace with a multi-symbol body adds the extra symbols, and the response does not say so

## Summary
`edit_code(action="replace", symbol=X, body=…)` replaces exactly X's span. When `body` also defines other symbols, those are inserted, not reconciled against same-named definitions elsewhere in the file. The response reports only `status: ok` and `replaced_lines`, so the leftover duplicate is invisible until a later `symbols()` call. Recorded from `catalog-audit-trail-session-log:F-3` when that log was archived.

## Class fit
No existing class fits. The nearest are `IC-13` (a capped result presented as complete) and `IC-21` (an instrument omits the dimension that grows), but neither names a write whose response is accurate about what it replaced and silent about what it added. Filed as `cluster/unclassified` per the template's escape hatch.

## Symptom (Effect)
Observed 2026-09-01 in `src/librarian/catalog/audit.rs`. One `body` held four functions (`value_expr`, `changed_predicate`, and rewritten `old_image_expr` and `update_diff_expr`), targeted at the single symbol `old_image_expr`. Afterwards `symbols()` listed `update_diff_expr` twice, at lines 141-151 and 154-163. Removing one then needed `at_line`, because the two were name-identical.

## Reproduction
Not re-run on 2026-10-30. Best lead: `edit_code(action="replace", symbol=<fn A>, body=<new A + a second, already-existing fn B>)` in a Rust file, then `symbols(path)`.

## Environment
Rust file, rust-analyzer LSP, branch `experiments`, as recorded by the 2026-09-01 session.

## Root cause
The tool's contract is symbol-scoped by design. `do_replace` (`src/tools/symbol/edit_code.rs`, `impl EditCode`) returns `json!({"status": "ok", "replaced_lines": …})` on the clean path, plus optional `warning`, `removed_attributes`, `indent_converted` and `corruption_check` keys. Its corruption verdicts (`TargetDropped`, `SiblingsDropped`, `SyntaxBroken`) look only for symbols that disappeared, never for symbols that were added. measured 2026-10-30: read of the response construction in `do_replace`. The leftover duplicate itself was observed on 2026-09-01 and not reproduced on 2026-10-30.

## Evidence
`docs/trackers/archive/catalog-audit-trail-session-log.md` § `catalog-audit-trail-session-log:F-3` (this record was opened as it moved to the archive). Its text: "the edit's own response … returned `status: ok` with a `replaced_lines` span that was accurate about what it replaced and silent about what it added."

## Hypotheses tried
1. **Hypothesis:** the duplicate is a correctness hazard. **Test:** read the 2026-09-01 entry. **Verdict:** rejected for Rust, where duplicate definitions do not compile. The entry notes it may be silent in Python, TypeScript or Go, which this record did not check.

## Fix
Fixed in `6da10f715` and `69121f8c8` (clippy `repeat_n`). `do_replace` now compares name-path counts before and after the write with `added_outside_target` (`src/symbol/edit.rs`) and adds `also_defined: [names]` to the response, and the compact form prints `also defined outside the target: …`. Counts, not set membership, because the observed case redefined a name already in the file. The target's own new children are not reported. It reports and never rolls back, since a body may add a helper on purpose. Not reproduced against rust-analyzer: the end-to-end test uses the mock LSP and the tree-sitter AST.

## Tests added
Three unit tests on `added_outside_target` (`symbol::edit::tests`) and `tools::symbol::tests::edit_code_replace_reports_a_symbol_the_body_added_outside_the_target`. Mutation-applied: a set-membership version of the helper turned two of them red; reverted. Clippy clean and 8051 lib tests passed on a clean worktree at `69121f8c8`. The full gate was red on `tests/result_caps.rs::every_cap_constant_is_classified` because peer commit `70740aaf4` added `SHORT_LANG_BUDGET` (`src/tools/symbol/symbols.rs:722`) without a `cap-class:` annotation. That failure is not from this change.

## Workarounds
Run `symbols(path)` after any multi-symbol replace, then `edit_code(action="remove", symbol=…, at_line=…)` for the leftovers. Better: one `edit_code` call per symbol.

## Resume
Reproduce with the recipe above, then check whether `do_replace` in `src/tools/symbol/edit_code.rs` can diff the symbol set before and after the write that it already extracts for the corruption verdict.

## References
`docs/trackers/archive/catalog-audit-trail-session-log.md` § `catalog-audit-trail-session-log:F-3`; `src/tools/symbol/edit_code.rs`.
