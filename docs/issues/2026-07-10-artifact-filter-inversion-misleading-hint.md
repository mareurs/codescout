---
status: fixed
opened: 2026-07-10
closed: 2026-07-10
severity: low
owner: marius
related: []
tags: [librarian, filter, hint, ergonomics]
kind: bug
---

# BUG: artifact(find) inverted filter returned a misleading "unknown field" error

## Summary
`artifact(find)` with an inverted filter leaf — `{op: {field, value}}` instead
of the canonical `{field: {op: value}}`, a very common agent mistake — returned
`unknown field \`contains\`` with a hint listing allowed fields. Misleading: the
agent never meant `contains` as a field name. Fixed by repair-and-continue.

## Symptom (Effect)
```
artifact(find, filter={"contains": {"field": "title", "value": "or-tools"}})
→ error: "unknown field `contains`"
  hint:  "allowed fields: [kind, status, topic, ...]"
```
Reproduced live 2026-07-10 before the fix.

## Reproduction
Any inverted leaf, pre-`19fb6b88`. usage.db sweep (2026-07-10) counted 22+
occurrences across 6 projects — the single most common `artifact` filter error.

## Environment
codescout `experiments`; librarian filter compiler; any MCP client.

## Root cause
`compile_leaf` (`src/librarian/filter.rs`) reads a single-key leaf's key as a
*field* name. An op-keyed inverted leaf therefore falls through to the
`ALLOWED_FIELDS` check and errors with a hint that does not recognize the
inversion — it treats `contains` as a mistyped field rather than a misplaced op.

## Fix
Repair-and-continue (ADR `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`).
`filter::repair_inverted_leaves` rewrites `{op:{field,value}}` → `{field:{op:value}}`
at the `find` handler boundary (`src/librarian/tools/find.rs`), runs the query,
and returns a `corrections` note teaching the canonical shape. `compile` stays
strict as defense-in-depth. Fix on `experiments` `19fb6b88` (pending cherry-pick
to `master` — do not archive this file until then).

## Tests added
- `src/librarian/filter.rs`: `repair_fixes_inverted_op_keyed_leaf`,
  `repair_leaves_canonical_leaf_untouched`, `repair_recurses_into_composition`,
  `repair_ignores_op_keyed_leaf_without_field`.
- `src/librarian/tools/find.rs`: `repairs_inverted_filter_and_notes_correction`.

## Workarounds
Use the canonical leaf shape `{field: {op: value}}` (now auto-corrected anyway).

## Resume
N/A — fixed and verified live.

## References
- ADR: `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`
- Fix: `experiments` `19fb6b88`
