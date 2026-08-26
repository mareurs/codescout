---
id: '3efdeb057c0deea9'
kind: bug
status: open
title: '`status: zombie` bug files are reachable by no standard query — not triage, not either doctor check'
tags:
- librarian
- doctor
- tracker-conventions
- triage
closed: null
opened: 2026-08-26
owner: marius
related:
- docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
severity: medium
unverified: 'The blind spot is measured at the SQL level and by count. NOT established: whether any of the 4 live zombie records is actually stale — verifying them is the work this bug makes possible, not work it did.'
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

Options, not yet chosen:

1. **A `zombie_unreviewed` doctor check** — report zombie records whose `last_observed:`
   is older than some window. Closest in shape to the checks that already exist, and it
   gives `last_observed:` its first reader. Needs a window, and the window is a judgement.
2. **Widen the triage guidance** — teach `get_guide("tracker-conventions")` that the
   canonical query is `[open, investigating, zombie]`. Cheapest; catches it at read time
   rather than by a scan, and does nothing for anyone who runs the old query.
3. **Both.** 1 makes it enumerable, 2 makes it habitual.

Prefer measuring first whether any of the four is actually stale — if all four are
still genuinely unobserved-but-unconfirmed, the exposure is smaller than the count
suggests, and option 2 alone may be right.

## Tests added

Pending.

## References

- `get_guide("tracker-conventions")` § Bug files — the status vocabulary, including `zombie` and its `last_observed:` pairing
- `src/librarian/tools/doctor.rs:2935,3397` — the two terminal-population queries

