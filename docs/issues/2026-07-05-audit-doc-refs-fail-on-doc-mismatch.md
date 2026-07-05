---
status: open
opened: 2026-07-05
closed:
severity: low
owner: marius
related: []
tags: [librarian, audit_doc_refs, doc-drift]
kind: bug
---

# BUG: audit_doc_refs `fail_on` documented as high|med|low|never but code handles only "high" and "any"

## Summary
The tool schema documents `fail_on: high | med | low | never`, but `build_response` branches
only on `"high"` and `"any"`. Passing `fail_on="med"` or `"low"` silently behaves like
`"never"` (exit_code stays 0).

## Symptom (Effect)
A CI gate configured with `fail_on="med"` never fails, regardless of med-severity findings.

## Reproduction
Run with `fail_on="med"` against a corpus with med findings; `exit_code` is 0.

## Environment
codescout `experiments`, observed 2026-07-05 during plan-mode scouting.

## Root cause
`src/librarian/tools/audit_doc_refs/mod.rs:600-611` matches `"high"` and `"any"` only; the
schema/description advertises `high | med | low | never`. Doc and impl drifted; unknown
values fall through to no-gate.

## Evidence
Scout report (plan-mode, 2026-07-05): "the schema doc says `high|med|low|never` but the code
only handles `"high"` and `"any"` (`mod.rs:600-611`)."

## Hypotheses tried
N/A — mechanism read directly from source.

## Fix
Implement `med`/`low` thresholds (gate on findings at-or-above the level) and either accept
`any` as documented alias or fix the docs; reject unknown values with `RecoverableError`
instead of silently not gating.

## Tests added
N/A — not yet fixed.

## Workarounds
Use `fail_on="high"` or `"any"` only.

## Resume
Extend the match in `build_response` (`mod.rs:600-611`) to a severity-threshold comparison +
a `RecoverableError` on unknown values; sync the schema text; add unit tests per level.

## References
`src/librarian/tools/audit_doc_refs/mod.rs:600-611`, `src/librarian/tools/librarian.rs:78-84`.
