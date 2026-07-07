---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- audit_doc_refs
- doc-drift
topic: null
time_scope: null
closed: '2026-07-06'
opened: '2026-07-05'
owner: marius
related: []
severity: low
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

Implemented. `build_response` (`src/librarian/tools/audit_doc_refs/mod.rs`) now returns
`Result<Value>` and gates `exit_code` on an explicit severity threshold per level:
`never` (0), `high` (any High-severity non-resolved finding), `med` (High or Med),
`low` (any non-resolved/non-external finding, any severity). The undocumented `"any"`
value is kept as a backward-compatible alias for `low`'s predicate. Any other value
now returns a `RecoverableError` naming the bad value instead of silently gating
nothing.
## Tests added

`src/librarian/tools/audit_doc_refs/mod.rs` tests module: `fail_on_never_is_always_zero`,
`fail_on_high_ignores_med_severity`, `fail_on_high_trips_on_high_severity`,
`fail_on_med_trips_on_med_and_high_but_not_low`, `fail_on_low_trips_on_any_unresolved_severity`,
`fail_on_low_is_silent_when_all_resolved`, `fail_on_any_alias_matches_low_semantics`,
`fail_on_unknown_value_is_rejected` — one case per threshold level plus the alias and
the rejected-unknown-value path.
## Workarounds
Use `fail_on="high"` or `"any"` only.

## Resume
Extend the match in `build_response` (`mod.rs:600-611`) to a severity-threshold comparison +
a `RecoverableError` on unknown values; sync the schema text; add unit tests per level.

## References
`src/librarian/tools/audit_doc_refs/mod.rs:600-611`, `src/librarian/tools/librarian.rs:78-84`.
