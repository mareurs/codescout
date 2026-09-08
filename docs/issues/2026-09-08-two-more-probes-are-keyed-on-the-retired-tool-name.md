---
id: e78057d4df6c2d27
kind: bug
status: fixed
title: Two more probes are keyed on the retired tool name, and one reports zero for the question it exists to answer
tags:
- cluster/selector-narrower-than-its-population
topic: probe correctness
claimed_at: 2026-09-08
claimed_by: ad379a7c-a0cf-4c61-bcdb-f0696fea8c30
closed: 2026-09-08
opened: 2026-09-08
owner: marius
related: []
severity: high
---

# BUG: two more probes were keyed on the retired tool name, and one reported zero for the question it exists to answer

## Summary

The 2026-09-02 rename `artifact` → `doc` (`ceb5b57a`) left **four** probes matching the dead name.
One was fixed the next day (`f41bfeb963dcee56`, `d4ee86da`). One was fixed on 2026-09-08
(`249dcf2690afcd35`). **Two were still blind when that second fix's own ceiling was written down**
— and were found by the grep that ceiling implies, within the hour.

| probe | selector | effect |
|---|---|---|
| `scripts/probe_entry_read_grain.py` | four inline `r["tool_name"] == "artifact"` | **recent bucket read 0 for every row** |
| `scripts/probe_librarian_scope.py` | `LIBRARIAN_TOOLS`, four of five names retired | short **916 of 10110** calls |

`usage.db` holds **6089** `artifact` rows against **838** `doc` and climbing, because a session
keeps its old binary until `cargo rb` + `/mcp`. So the blindness is **monotonic**: these probes
report a shrinking share of reality every day, always in the same direction.

## Symptom (Effect)

`probe_entry_read_grain.py` exists to answer *"where do entry-grain reads land, and is the buffer
leak worth closing?"* — a question about **now**. Measured before and after:

| last 30h | before | after |
|---|---|---|
| doc/artifact calls | **0** | 252 |
| `append_entry` | **0** | 21 |
| `get(heading=)` | **0** | 11 |
| LEAKED entry-grain reads | **0** | 1 |

Every row of the recent column was zero while the `older` column looked healthy. That does not
read as a broken instrument — it reads as *"the behaviour stopped"*, which is a finding a reader
would act on. The leak was still happening; the probe said it had ended.

`probe_librarian_scope.py` moved every ratio it prints: `scope_aware` 18.4% → 18.9%,
`omitted (ran on default)` 80.6% → **82.3%**. Smaller, and the kind of number that gets cited in a
design decision.

## Reproduction

```
grep -rn 'tool_name.*== *"artifact"' scripts/*.py
grep -rn '^[A-Z_]*TOOLS *=' scripts/*.py
```

One grep. Both sites were reachable from the moment the rename landed.

## Root cause

`IC-18`, and specifically the shape recorded on `249dcf2690afcd35`: **an instance fix is not a
class fix.** Each of the four probes was repaired — or not — in isolation, because nothing asked
*"which other selectors name a tool?"*

The second half is why the gate shipped on 2026-09-08 did not catch these. It matched the
**qualified** `mcp__codescout__<name>` form, which is what a *transcript* carries. These probes
read `usage.db`, which carries the **bare** name. That ceiling was measured and stated in that
bug's Fix section — and stating it is what produced the grep that found these two. The value of
naming a ceiling is that someone runs it.

## Evidence

**A bare comparison is unreachable by any gate, and that is the deeper defect.**
`probe_entry_read_grain.py`'s four sites were inline `r["tool_name"] == "artifact"` tests. Outside
a named constant, `"artifact"` is indistinguishable from prose, so no check could see it go stale
however carefully written. Hoisting those into `DOC_TOOLS` is therefore **part of the fix, not
tidying** — it is what makes the site checkable at all.

**The convention was discovered, not invented.** Every tool-name list under `scripts/` was already
a module-level `*_TOOLS` constant — `CS_WRITE_TOOLS`, `NATIVE_WRITE_TOOLS`, `MECHANISM_TOOLS`,
`LIBRARIAN_TOOLS`, `TARGET_TOOLS`. The gate keys on that rather than requiring a new marker
discipline nobody would remember.

**`extract-kotlin-tcs.py` is the control, and it is why the two directions are not symmetric.**
Its `TARGET_TOOLS` names `read_markdown` and `edit_markdown`, both retired — but `read_file` and
`edit_file` are present, so it is **not blind**. A dead name in a set is inert; only a missing live
name is a defect. Marking, not repairing, was the correct action there.

## Hypotheses tried

**The first version of the bare-form gate was negative-only, and the mutation run refuted it.**
Dropping `"doc"` from `LIBRARIAN_TOOLS` and from `DOC_TOOLS` both **SURVIVED**: every remaining
name was still live-or-legacy, so the gate was satisfied *by the defect it exists to catch*. That
is precisely the weakness written into the sibling gate's own design note an hour earlier —
*"negative-only … its cheapest repair is to mark the old name legacy, which leaves the
blindness"* — and it was shipped anyway. Knowing the class prevented nothing;
the mutation caught it.

## Fix

**Shipped 2026-09-08 at `cc160413`** (`experiments`), patch-id
`bf95a5e5c367e4fe0ec4d36a637439c26a3aca5f`. Recorded as a pair at fix time: the SHA is positional
and dies when `experiments` is rebased; the patch-id is a content hash of the diff and survives
rebase and cherry-pick.

**The escape hatch was made to do double duty.** A `legacy` marker now declares the retired tool's
successor — `# legacy -> doc:` — and the gate requires that successor to appear as a live name in
**the same block**. Block-scoped deliberately: a successor present in some other constant does not
make this one non-blind.

That converts the weak direction into the strong one. A future rename reds every block naming the
old tool without the new one, which is the failure that actually occurred, four times.

Also fixed here: the gate reported three *live* tools as unregistered on its first real run,
because a `*_TOOLS` block may hold either form and the bare check compared a qualified name
against the bare registry. Normalised by stripping the prefix. Found by the gate, on itself.

## Tests added

No new assertions — the existing gate was extended, and every direction re-mutated. All six kill,
including the two that survived the first attempt:

| mutation | before | after |
|---|---|---|
| drop `doc` from `LIBRARIAN_TOOLS` | **SURVIVED** | killed |
| drop `doc` from `DOC_TOOLS` | **SURVIVED** | killed |
| drop `doc` from `MECHANISM_TOOLS` | — | killed |
| un-mark a retired name `legacy` | killed | killed |
| successor marker names a dead tool | — | killed |
| drop a live tool, keep its `legacy ->` marker | — | killed |

**Each mutation was verified to have APPLIED before its verdict was read.** An unapplied mutation
and an uncovered site both print green, so a `SURVIVED` verdict from a `sed` that matched nothing
is indistinguishable from real coverage. A deliberately bogus pattern is included as the control
and correctly reports `NOT APPLIED`. Raised by `59112612`, who lost an assertion to exactly this
the same morning when `cargo fmt` had rewrapped a literal their mutation pattern targeted.

Two non-vacuity asserts guard the scan's own inputs: the block/name counts, and the presence of at
least one `legacy ->` marker — without the second, the positive half has nothing to check and
passes on any amount of drift.

## Workarounds

None needed. Before the fix, any number from these two probes needed re-deriving with `doc`
included.

## Resume

Nothing outstanding. The remaining `IC-18` surface in this area is prose mentions of tool names in
comments and docstrings, which are deliberately out of scope: a comment naming a retired tool is
inert, and gating prose is the mistake `audit_doc_refs`' `code_block` exemption already exists to
avoid.

## References

- `249dcf2690afcd35` — the sibling fix whose stated ceiling produced this grep.
- `f41bfeb963dcee56` — the first instance, fixed 2026-09-03 and not swept.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
