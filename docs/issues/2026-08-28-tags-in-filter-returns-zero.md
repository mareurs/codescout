---
id: '239227f3228b3460'
kind: bug
status: open
title: 'BUG: filter tags.in returns zero while tags.contains finds the row, and the guide teaches the broken form'
tags:
- librarian
- find
- filter
- misleading-zero
opened: 2026-08-28
owner: marius
severity: med
unverified: Both probes are measured and reproducible; WHICH of the two readings is intended (in should work on arrays, vs in is scalar-only and should be refused) is NOT established, and the fix differs between them. Check the SQL lowering for `in` against tags/owners before choosing. No fix attempted.
---

# BUG: `filter={"tags": {"in": [...]}}` silently returns zero, while the guide teaches it as *the* way to filter by tag

## Summary

`artifact(action="find", filter={"tags": {"in": ["handoff"]}})` returns **0** for an
artifact whose catalog row demonstrably carries that tag. The `contains` form of the same
query returns it correctly.

This is the misleading-zero class: nothing errors, and the answer reads as "no artifact has
this tag" rather than "this operator does not work on this field". `get_guide("librarian")`
§ *Filter Syntax* offers `{"tags": {"in": ["foo", "bar"]}}` as its worked example for tag
filtering, so the documented path is the broken one.

## Symptom (Effect)

Same artifact, same session, two operators:

```
filter={"tags": {"contains": "handoff"}}   -> count: 1   (correct)
filter={"tags": {"in": ["handoff"]}}       -> count: 0   (wrong)
```

`artifact(action="get")` on that id shows the tag is present in the row:

```json
"tags": ["handoff", "bug-ledger", "cross-machine", "session-state"]
```

Both probes ran with default scope against the active project, no `kind`/`status`
shortcut, so no AND-interaction is involved. Adding `kind="tracker"` does not change
either result.

## Reproduction

```
artifact(action="create", kind="tracker", rel_path="docs/trackers/x.md",
         title="x", body="# x", tags=["zzz-probe"])
artifact(action="find", filter={"tags": {"contains": "zzz-probe"}})   -> 1
artifact(action="find", filter={"tags": {"in": ["zzz-probe"]}})       -> 0
```

Observed 2026-08-28 on `experiments` @ `2243b477`, against a tracker created minutes
earlier in the same session (so no reindex-lag explanation: the row `get` returns already
has the tags).

## Environment

- branch `experiments`, commit `2243b477`
- artifact created via `artifact(action="create", tags=[...])`, then `status`/`owners`
  patched via `artifact(action="update")` — tags untouched by the update.

## Root cause

**Not established.** The documented contract (`get_guide("librarian")` § Filter Syntax) is:

> **Ops:** `eq`, `ne`, `in`, `nin`, … `contains` on strings → `LIKE '%v%'`; on tag/owner
> arrays → array membership.

So `contains` is explicitly specified for array membership and works. What is unclear is
what `in` is *supposed* to mean on an array-valued column. Two readings, and they need
different fixes:

1. **`in` is meant to work on tags** (the guide's example implies it) — then the SQL
   lowering for `in` against an array column is wrong, and this is a defect.
2. **`in` is only meaningful on scalar columns** (`{"status": {"in": [...]}}` is the
   canonical use, and it works) — then the defect is in the guide, which teaches an
   operator the engine does not support for that field, and the engine should *reject*
   the combination rather than answer 0.

Reading 2 is the more likely: `in` on a scalar means "value is one of these", which has no
obvious array analogue, and the guide's tag example may simply be wrong. Either way the
current behaviour is the worst of both — a silent, plausible zero.

## Hypotheses tried

- **"The catalog row lacks the tags."** Refuted: `artifact(action="get")` on the id returns
  all four tags.
- **"A `kind`/`status` shortcut is ANDing it away."** Refuted: the bare filter with no
  shortcut behaves identically.
- **"Reindex lag."** Not applicable: `contains` finds the same row in the same session.

## Fix

Not attempted. Whichever reading holds, the outcome should not be a bare `0`:

- if (1), lower `in` on array columns to membership-any, matching `contains`;
- if (2), reject `in` on `tags`/`owners` with a `RecoverableError` naming `contains` as the
  operator that does what the caller meant — the same shape as this repo's other
  did-you-mean routing;
- and correct `get_guide("librarian")` § Filter Syntax either way, since its worked example
  is currently the failing form.

This also wants a `filter_warnings`-style signal: the entry-grain twin already reports
`filter_warnings.unknown_fields` precisely because an empty in-memory result may be a typo
rather than a true zero. The artifact-grain side has no equivalent for an
unsupported-operator-on-this-field.

## Workarounds

Use `{"tags": {"contains": "<one-tag>"}}`. For multiple tags, OR several `contains` leaves:

```
filter={"or": [{"tags": {"contains": "a"}}, {"tags": {"contains": "b"}}]}
```

## References

- `get_guide("librarian")` § *Filter Syntax* — the contract, and the source of the
  misleading example.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the standing rule that a
  suspicious zero must name what it examined; this is a zero that names nothing.

