---
id: '44ebfc3bd48ceb53'
kind: bug
status: open
title: 'BUG: doc(action=move)''s inbound_path_citations cannot see a slug-form citation, and reports [] rather than the form it searched'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: medium
---

## Summary

`doc(action="move")` returns `inbound_path_citations` — the citations it believes point at
the artifact being moved, so a caller can re-point them in the same commit. It resolves
citations in the **path** form (`docs/issues/<date>-<slug>.md`). A citation written as a
bare **slug** is never examined, and the response reports `[]` rather than saying so.

`IC-18` exactly: *"a zero reads as 'not present' rather than 'not looked at'."*

Slug-form citations are not an edge case here. **Every `**Members:**` line in
`docs/trackers/issue-clusters/` cites its members by slug** — that is the ledger's house
form — so the population this field cannot see includes the entire cluster corpus.

## Symptom (Effect)

Two instances, 2026-09-13, both real moves:

| # | session | result | was the zero right? |
|---|---|---|---|
| 1 | `f3c594ce` | `inbound_path_citations: []` while `IC-6`'s Members line cited the file by slug | **No.** Caught only because the slug had been grepped BEFORE the move, for an unrelated reason. |
| 2 | `8bd791df` | `[]`, and no citation was broken | **Yes — by ordering, not by the field.** They wrote the `IC-2` Members entry *after* moving, so it already named the new slug. |

**Instance 2 is the more dangerous of the two and is the reason this is filed.** A broken
instrument that happens to return the correct answer produces no prompt to look again. Had
the ordering been reversed — Members line first, as in instance 1 — the same `[]` would
have been false and nothing would have marked it.

## Reproduction

Move an artifact that is cited by slug rather than by path:

```
doc(action="move", id="<id>", new_rel_path="docs/issues/archive/<same-name>.md")
-> inbound_path_citations: []      # while `grep -F '<slug>'` returns the citing line
```

Confirmed against `70e16d65`, where the citing line was `IC-6`'s `**Members:**`.

## Environment

`experiments`, 2026-09-13. Independent of the archive/rename distinction — instance 1 was
a move that also renamed, instance 2 a move that did not.

## Root cause

Not established in code; the behaviour is black-box so far. What is established is the
**shape**: the resolver's selector is the path form, the population is "citations of this
artifact", and the two differ by exactly the slug-only form. Whether that is a regex, a
`LIKE` over a stored citation table, or the `cites` edge set is unread — and the direction
does not depend on which.

## Fix

Not implemented. Two halves, and only the first is code:

1. **Widen or report.** Either resolve slug-form citations too, or name the form that was
   searched so `[]` carries its scope — `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
   is this repo's own ruling on exactly this, and the field predates or ignores it.
2. **The ordering rule, which needs no code and is already recorded.** Move BEFORE writing
   the citation; a Members line written afterwards already names the new path. Contributed
   by `8bd791df` from instance 2 and now in
   `docs/issues/2026-09-07-verifying-a-move-by-slug-cannot-distinguish-the-two-path-forms.md`
   § Fix. It is a **sequence rather than a check**, so it does not require anyone to
   remember the field is unreliable.

**This is testable, which the sibling file's own § Tests added denies of its class.** A
fixture holding one slug-form and one path-form citation, asserting both come back, reds
today. That sibling concluded *"a defect in how a human-or-agent verifies, not in a code
path"* — true of its six reader-side instances, and false here.

## Tests added

None; not fixed. The fixture is named above so whoever picks it up does not have to
rediscover it.

## Resume

Found while archiving the fenced-line-attribution bug, and filed **separately from**
`b8c0716a0fd4c17d` on that ledger's explicit rule: *"if a finding satisfies a second
class's claim, it is a second bug file. The one-tag rule is what makes `n` a partition
rather than a tally."*

It first went in as a paragraph inside that file (`663f7efd`), which was the wrong unit
and carried a wrong conclusion with it — that `IC-18` should widen to cover both
directions. It should not: `docs/trackers/issue-clusters.md` § *The entry shape* already
rules that `IC-6` and `IC-18` are separated **by direction**, `IC-6` matching too much and
`IC-18` too little, and widening would collapse the discriminator. The host file's defect
is the over-match (two path forms that collide under one slug, `IC-6`); this one is the
under-match (`IC-18`). Same namespace ambiguity, two classes, by design.

## References

- `docs/issues/2026-09-07-verifying-a-move-by-slug-cannot-distinguish-the-two-path-forms.md`
  — the mirror direction, and where the ordering rule lives.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the ruling half (1) above
  would satisfy.
- `70e16d65` — the move that produced instance 1.
