---
id: fe2de4189ce2abbd
kind: bug
status: open
title: edit_code replace with a multi-symbol body adds the extra symbols, and the response does not say so
tags:
- cluster/unclassified
opened: 2026-10-30
owner: marius
severity: low
unverified: The leftover-duplicate behaviour was observed on 2026-09-01 and not reproduced on 2026-10-30; only the response shape was read in do_replace.
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
Not started. Proposal from the entry: when `body` introduces symbols the target span did not contain, report `also_defined: [names]` in the response. The caller needs that information and already has to go and find it.

## Tests added
None. Status is `open`.

## Workarounds
Run `symbols(path)` after any multi-symbol replace, then `edit_code(action="remove", symbol=…, at_line=…)` for the leftovers. Better: one `edit_code` call per symbol.

## Resume
Reproduce with the recipe above, then check whether `do_replace` in `src/tools/symbol/edit_code.rs` can diff the symbol set before and after the write that it already extracts for the corruption verdict.

## References
`docs/trackers/archive/catalog-audit-trail-session-log.md` § `catalog-audit-trail-session-log:F-3`; `src/tools/symbol/edit_code.rs`.
