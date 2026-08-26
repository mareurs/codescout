---
id: 213cf067be79209e
kind: bug
status: fixed
title: '`status: zombie` bug files are reachable by no standard query — not triage, not either doctor check'
tags:
- librarian
- doctor
- tracker-conventions
- triage
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related:
- docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
severity: medium
unverified: Measured which fix option was right (checked the 3 live zombie records for staleness before choosing), but did not implement a regression test -- there is no code path to test for a doc-only guidance change; verification is that the query text now includes zombie in the three prescriptive surfaces, checked by hand.
---

# BUG: `status: zombie` bug files are reachable by no standard query

## Summary

The bug status vocabulary has six values. Every query in the project covers some
subset, and `zombie` falls in the gap between all of them:

| Query | Covers | Reaches zombie? |
|---|---|---|
| canonical triage — `find(kind="bug", status in [open, investigating])` | non-terminal | no |
| `doctor` `terminal_status_with_caveat` — `status IN ('fixed','mitigated','wontfix')` | terminal | no |
| `doctor` `terminal_status_without_fix_anchor` — `status IN ('fixed','mitigated')` | fixed-ish | no |

A zombie is by definition the record *least* safe to forget — *"no longer observed but
root cause unconfirmed"* — and it is the one status nothing can enumerate.

## Symptom (Effect)

Silence, and a confident wrong count. Four zombie records sit in `docs/issues/`;
every "what's open?" report this project runs returns **2**, and the 4 are not in the
terminal population either, so no hygiene sweep reaches them.

This bug was found by counting the directory by hand after the doctor's own worklist
gave a smaller number than the filesystem. The measured pile is 23 files: 2 open, 17
terminal, 4 zombie.

The records themselves are not the problem — all four are well-formed and carry the
`last_observed:` the convention requires. The gap is entirely on the query side.

## Reproduction

Measured 2026-08-26 on `experiments`:

```
artifact(find, kind="bug", filter={"status":{"in":["open","investigating"]}})  → 2
ls docs/issues/*.md | wc -l                                                    → 23 (incl. _TEMPLATE)
status counts: 7 fixed, 9 mitigated, 2 open, 1 wontfix, 4 zombie
```

At the SQL, `src/librarian/tools/doctor.rs`:

```
2935:  WHERE kind = 'bug' AND status IN ('fixed', 'mitigated', 'wontfix')
3397:  WHERE kind = 'bug' AND status IN ('fixed', 'mitigated')
```

Neither names `zombie`; neither does the triage filter.

## Root cause

Not a defect in any one query — each is correct for the population it names. The
vocabulary grew a sixth value whose semantics are *neither* live *nor* terminal, and
no query was widened or added to cover it. `zombie` is precisely the state that is
neither, which is what makes it useful and what makes it unreachable.

The `last_observed:` field the convention pairs with `zombie` compounds it: it exists
so a stale zombie can be re-opened or closed, and nothing reads it. The oldest live
one is `2026-07-07`, 50 days at time of filing.

## Fix

**Measured before choosing, per this file's own recommendation.** Checked the 3 live
`status: zombie` records (not 4 — one had been reclassified since filing):

- `e817931ef9d51dd0` — `last_verified: 2026-08-26` (today). Explicitly `zombie` by
  maintainer decision, actively tracked — working as designed, not neglected.
- `523233935cc53bc4` — title states its underlying bugs ("Bug A", "Bug B") are already
  fixed/mitigated. Residual instrumented-watch state, not neglect.
- `6d6a6efca4d2bdd9` — the stalest (`last_observed: 2026-07-18`), but already went
  through one reopen-and-fail-to-reproduce cycle before settling into `zombie`.

None represents active neglect — exactly the outcome this file predicted would point at
**option 2 alone** (widen the triage guidance) rather than option 1 (a new `doctor`
staleness check).

**Implemented option 2.** Widened the canonical bug-triage query from
`{"status": {"in": ["open", "investigating"]}}` to include `"zombie"`, with an explicit
note that a `zombie` hit is a "has this recurred?" check, not a task to pick up (most
zombie records have no available work by design). Updated the three prescriptive
surfaces that state this query: both auto-injected `get_guide` topics
(`project-activation-bootstrap`, `tracker-conventions`) and `docs/issues/_TEMPLATE.md`.
Left `docs/trackers/open-issue-work-queue.md` alone — its `17 rows` line is a historical
snapshot tied to its own past query run, not general guidance.

**SHA:** `8c12a55a` (`experiments`)
**patch-id:** `962d4704612f674b49915566c0ac6e97d4463c2a`
## Tests added

Pending.

## References

- `get_guide("tracker-conventions")` § Bug files — the status vocabulary, including `zombie` and its `last_observed:` pairing
- `src/librarian/tools/doctor.rs:2935,3397` — the two terminal-population queries
