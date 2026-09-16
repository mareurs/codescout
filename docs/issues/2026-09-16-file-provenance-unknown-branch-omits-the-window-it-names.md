---
id: '89212e943c525d5d'
kind: bug
status: open
title: 'BUG: file-provenance prints the window on every verdict except UNKNOWN, the one whose meaning is the window'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- file-provenance
- shared-checkout
- diagnostics
topic: authorship attribution on a shared checkout
---

# BUG: file-provenance's UNKNOWN branch omits the one line that explains it

## Summary

`scripts/file-provenance.py` prints `window: writes at or after <floor>` on `MINE`, `SHARED`
and `PEER`, and **not** on `UNKNOWN` — the single verdict whose entire meaning is the window.
The default floor is *"since this path was last committed"* (`:599-608`), so a **clean file
always returns UNKNOWN**, correctly and uninformatively.

The accompanying prose then explains `UNKNOWN` via exactly one cause — a Bash write the
heuristics missed — and never mentions the far more common one: the file is committed, so
nobody has bytes at risk. Two opposite situations render in identical text:

| input | what UNKNOWN means | what the output says |
|---|---|---|
| clean file | **nobody, dispositively** — nothing is at risk | "statement about coverage, not ownership" |
| dirty file, unattributed write | **a real coverage gap** | the same sentence |

## Symptom (Effect)

Measured 2026-09-16, in the field rather than constructed. Session `9e022ef0` ran the tool on
three `.rs` files I had committed minutes earlier, got `UNKNOWN` on all three, and reported it
as the tool failing to attribute — concluding that diffing a symbol against `HEAD` was
*"strictly better than provenance."* Those are not competing instruments; one answers *who has
uncommitted bytes here* and the other *is this mechanism new*. The `UNKNOWN` text is what
licensed the comparison.

Nothing failed. No error, no exit code, and the reader was experienced enough to be running
provenance unprompted on a shared checkout — which is the population this tool exists for.

## Reproduction

```
git commit -- <path>                       # any path, any session
python3 scripts/file-provenance.py <path>
  -> UNKNOWN   <path>
     no record of any session writing this path in the window. ...
     (N write(s) exist but predate the window; re-run with --all to see them)
```

The `hidden` count is the only hint the window is doing the work, and it reads as an aside.
Compare a **dirty** path, where the same tool prints `window: writes at or after <ts>` and the
reader can see immediately what was excluded.

## Root cause

Read, not inferred. `scripts/file-provenance.py:623-640` versus `:642-647`:

```python
if not who_set:                       # UNKNOWN
    print(f"UNKNOWN   {rel}")
    print("          no record of any session writing this path in the window. ...")
    if hidden: print(f"          ({hidden} write(s) exist but predate the window; ...)")
    continue
...
print(f"{verdict:9} {rel}")
if floor:                             # MINE / SHARED / PEER only
    print(f"          window: writes at or after {floor}")
```

**The author already performed this exact harmonisation for the sibling field**, and the
comment at `:617-620` states the general reason:

> Records the window excluded — present regardless of verdict, because a hidden write is
> exactly as real on a MINE path as on an UNKNOWN one. Equals `len(records)` whenever
> `who_set` is empty, which is what makes this a drop-in for the count the UNKNOWN branch
> used to compute only for itself.

`hidden` was lifted to print on every verdict. `floor` was not, and the reason given for
`hidden` applies to it verbatim. One law, two sites, fixed at one.

## Classification

`cluster/value-correct-in-a-frame-its-name-does-not-state` (`IC-24`), on the **verdict word**,
which is a fifth axis alongside coordinate space, unit, citation form and scope word. `UNKNOWN`
is exactly right in the since-last-commit frame and exactly recoverable (`--all`, or running
before you commit); the word means *epistemic uncertainty* and the computation means *an empty
window*. IC-24's blind party holds here without adjustment: the producer is the party for whom
"no writes in window" and "unknown owner" coincide, and re-running the tool returns the same
correct value.

**Weighed and rejected.** `cluster/guard-narrower-than-its-name` — nothing is refused.
`cluster/selector-narrower-than-its-population` (`IC-18`) — the selector is right and the
population is right; only the report of them is short. **And stated because the fit should be
contestable rather than asserted:** a reader who thinks the naming half is incidental and the
branch asymmetry is the whole defect would file this under a class about a fix applied at one
of N sites, and that reading is defensible. It is IC-24 here because the asymmetry is only
expensive *through* the word — a reader who knew the window would not be misled by the missing
line.

## Fix

Not attempted. Two halves, and unlike the last bug I wrote that sentence about, the cheap half
is genuinely safe alone — it is the *same* half, not a louder version of a quieter one.

1. **Print `window: …` on `UNKNOWN` too.** One line moved above the branch, exactly as `hidden`
   was. This is the whole mechanical defect.
2. **Name the benign cause first in the prose.** The current text leads with the blind spot,
   which is the rarer case; a clean file is the common one and currently goes unnamed. Something
   of the shape *"this path is committed, so nobody holds uncommitted bytes in it"* when
   `hidden == len(records)` and the worktree is clean, with the coverage caveat kept second.

Half 1 without half 2 already fixes the reported misreading: `window: writes at or after
<a timestamp two minutes ago>` is self-explaining next to a file you just committed.

## Tests

None yet. The shape a guard needs, noting that this is the half `tests/file-provenance.sh` is
structurally weakest on: 140 assertions there are about **verdicts** — which party is named —
and this defect is entirely in the **caveat lines** beside a verdict that is already correct.
`CLAUDE.md` § *Testing Discipline* names the general case (a suite tests a guard's predicate and
never its remedy text) and its measured remedy is a **shape** assertion, not a pinned sentence:
assert `UNKNOWN`'s output contains a `window:` line whenever a floor exists. That reds on the
deletion and survives rewording.

## References

- `scripts/file-provenance.py:599-608` (floor), `:617-620` (the sibling harmonisation and its
  stated reason), `:623-640` (UNKNOWN), `:642-647` (the branch that prints the window).
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a zero owes its scope. This one
  names the scope *word* and withholds the scope *value*, on the branch where the value is the
  answer.
- `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`
