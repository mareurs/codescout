---
id: de46d402441e1e2b
kind: bug
status: fixed
title: 'BUG: the fix-anchor check reads a patch-id a record MENTIONS as one it CLAIMS'
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doctor
topic: bug-ledger integrity checks
---

# BUG: the fix-anchor check reads a patch-id a record MENTIONS as one it CLAIMS

## Summary

`librarian(action="doctor")`'s `non_terminal_status_with_fix_anchor` flags a non-terminal bug whose
body "declares" a patch-id, on the reasoning that a patch-id is produced only by
`git show <sha> | git patch-id --stable`, i.e. by closing the bug. It has no way to distinguish a
patch-id the record claims as **its own fix anchor** from one it **cites as another bug's**. Both are
a 40-hex token in prose.

## Symptom (Effect)

Measured 2026-09-13. `docs/issues/archive/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
is reported as:

> status is `open` but the body declares 1 patch-id(s) — `0e7feedf232c5ed9e22fd975c6fe36baa109e1d2` —
> which is produced only by [...] closing this bug. The queryable field and the prose disagree and only
> the prose is true, so every triage query hands this out as unstarted work.

Every clause of that is false for this record. The record is correctly `open`.

## Reproduction

Read the cited line rather than the finding:

```
grep -n '0e7feedf' docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md
```

Line 305-306 reads, in full context:

> **Both of the guards they proposed were then MEASURED AND REJECTED, and what shipped is neither.**
> Superseded by `docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md`
> (`d5af3d3ceff1d08c`), fixed at `74b9cc67`, patch-id `0e7feedf232c5ed9e22fd975c6fe36baa109e1d2`.

The patch-id belongs to `d5af3d3ceff1d08c`, a different bug that supersedes *part* of this one. This
record's own `## Resume` holds a live, undecided choice between two fix directions with a third
explicitly falsified — it is open in the fullest sense.

## Environment

codescout `experiments` @ `d3a2c24f`. `librarian(action="doctor")`, project scope.

## Root cause

The check scans the body for patch-id-shaped tokens and treats presence as a claim. A patch-id has no
syntax distinguishing *use* from *mention*, and the surrounding prose — which does carry the
distinction, unambiguously, for a human — is not consulted.

This is `cluster/addressing-without-an-escape-hatch` exactly: a scheme that interprets every token in
its namespace, with no way to write one as a mention. The defect is not any input it accepts; it is
that "cite a neighbour's fix anchor" is unrepresentable.

## Evidence

The check's own remedy text concedes a *different* limitation and, by naming only that one, implies
the rest are handled:

> This is a worklist, not a verdict: the check cannot tell a complete fix from a landed half.

It cannot tell a complete fix from a landed half — true, and stated. It also cannot tell **whose fix
it is reading**, which is not stated, and which is the failure that actually occurred. A reader who
takes the caveat at face value concludes the only risk is over-eager closure of a partial fix, and
flips the status.

## Hypotheses tried

1. **The record is genuinely a landed half** (the stated caveat). **Refuted** — the patch-id resolves
   to another artifact id named on the same line, and `74b9cc67` is that bug's fix commit, not this
   one's.
2. **`unverified:` is the remedy** — the check names it as the discharge for a legitimately-open
   record. **Rejected as dishonest here**: `unverified:` means *the fix is partial*. There is no fix.
   Using it would silence a false positive by asserting something untrue, and would make the record
   read as half-fixed to every future reader.

## Fix

Not yet implemented. Two directions, neither costed:

1. **Require an anchor to be declared structurally**, as `terminal_status_without_fix_anchor` already
   does — a `## Fix provenance` section with `- **SHA:**` / `- **patch-id:**` bullets. A patch-id in
   running prose would then never be read as a claim, because claims have a shape. This is the escape
   the class asks for, and the parser for it already exists in the sibling check.
2. **Consult the sentence.** Cheaper to state, worse to build, and it re-creates the same problem one
   level down.

Direction 1 is preferred and has a pleasing property: the two checks become inverses over one
grammar rather than two parsers disagreeing about what a patch-id means.

**Direction 1 shipped 2026-09-20, after the migration question this section demanded was answered.** `scan_non_terminal_status_with_fix_anchor` now calls `structured_fix_pointers` — **the same parser, over the same whole-file input, that the sibling `terminal_status_without_fix_anchor` discharges on**. The two checks are now inverses over one grammar, which is the property this section predicted. `declared_patch_ids` and `fix_section_body` had one call site each, are dead, and were deleted.

**The escape for MENTION is prose, and it is the unmarked form.** An author citing a neighbour's patch-id in a sentence needs to know nothing and do nothing to get it right; only the two-bullet shape outside a fence is a claim. That is the escape `IC-6` asks of any parser over a namespace, and here it costs the author zero.

**The migration count — unit, instant, tree.** Counted by the implementing agent at tree `2ef766f4`, and re-derived by this ledger at tree `40370bd1`, instant `2026-09-20T13:58Z`:

| population (unit: **records**) | count |
|---|---|
| live (non-`archive/`) `docs/issues/*.md` | 88 |
| of those, non-terminal (`open`/`taken`/`investigating` — the check's own SQL) | 59 |
| non-terminal carrying a prose patch-id the OLD grammar read as an anchor | **2** |
| live records carrying a **well-formed** structured pointer (both bullets, backticked) | **10** |
| of those 10, non-terminal | **0** |

**Verdict: non-breaking, a no-op on the live corpus.** The 2 prose carriers were already discharged by a non-empty `unverified:`, so the check reported 0 before and reports 0 after, confirmed by running the real CLI both ways. The change also runs in the direction that **cannot red an author's file** — strictly fewer findings, never more — which is the opposite of the 2026-09-14 hazard `src/prompts/mod.rs:2075` pins, where a check began *demanding* a shape ~150 files did not have.

## Fix provenance

- **SHA:** `496dd63e` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `e12594153c73e76c185ee4cc09bc79293fbfeafd` — content hash of the diff; survives rebase and cherry-pick. Derived through a file, never a pipe from `git show`.

**One objection was raised against shipping this, and it deserves recording because it was answered with a measurement rather than waved off.** § *References* carries the archived sibling's correction that `declared_patch_ids` is *"deliberately looser"* because **0 of the LIVE corpus** used the structured form on 2026-09-02 — i.e. a strict read risks being unreachable by any in-tree author, *"decoration however loudly written"*.

The implementing agent re-measured and reported *"still 0 live, 18 days later"*, shipping anyway on the grounds that reachability is a property of the **path** rather than of today's corpus: 167 of 833 archived records write the shape, the guide prescribes it at archive time, and the sibling refuses a terminal record that omits it. That reasoning is sound and is written into the scan's doc comment so a later reader can re-open it.

**The "still 0 live" figure is wrong, and wrong in the direction that strengthens the decision.** It conflates *0 non-terminal records carrying the structured form* — which is correct, and is the number the check's own population cares about — with *0 live records carrying it*, which is false: **10 live records carry the well-formed shape at `40370bd1`** (4 of them written by this campaign today, so 6 predate it). The 2026-09-02 concern no longer describes the corpus. So the check is reachable by direct corpus evidence and not only by the path argument, and a `0` from it means *the window it watches — provenance written, status not yet flipped — is currently empty*, not that nothing can enter it.

**A looser text-grain count disagrees, and the disagreement is instructive rather than a defect.** Grepping `^- **patch-id:**` returns **11** live records, one of them `open`. That eleventh is `fff5758f95e2fda2`, whose line reads `- **patch-id:** N/A` with no `- **SHA:**` bullet and no backticks, so the parser sees nothing and correctly emits no finding. Two instruments, two units, both right — which is exactly why `CLAUDE.md` requires a count to arrive with its unit.

## Tests added

None. A regression test wants a fixture whose body cites a foreign patch-id and asserts **no**
finding — note that is an absence assertion, monotone under removal of the check, so it must be
paired with a positive fixture that does claim its own anchor and must fire.

**Added 2026-09-20.** `non_terminal_status_with_fix_anchor_reads_a_cited_patch_id_as_a_mention_not_a_claim` — **four absence seeds paired with a positive one**, as this section required. The seeds: a `cites-a-neighbour` case reproducing the corpus sentence that produced this bug, inside a `## Fix` section; a wrapped value; a bare 40-hex decoy; and a fenced declaration. The positive `anchored` fixture declares its own pointer and **must** fire — without it the absence half asserts the silence a deleted check also produces.

**Observed red**, from widening the anchor test back to reading prose: `left: 2 right: 1 [anchored, cites-a-neighbour]`, and a sibling seed at `left: 5 right: 1`.

**Every prose fixture in the block was converted to a real declaration, and that is the subtle half.** Left as prose they would have gone on asserting `is_empty()` against a check now silent on that input **for a reason no test named** — inert, and reading as coverage. Mutating the anchor parser to return nothing reds **8** tests, which is what shows none of them is.

**Nine mutations, one per guarded SITE, all KILLED**, via `./scripts/mutation-probe.sh` in an isolated worktree: the anchor read (×2), the archive path-component skip, the status SQL (×2 — dropping `taken`, admitting `zombie`), the `unverified:` discharge, the shared parser's fence skip, the sibling's discharge on that shared parser, and `scope.admit`.

**Two of those nine are the point of the one-mutation-per-SITE law.** The fence skip and the sibling's discharge each killed **two** tests — one in *each* check — because `structured_fix_pointers` is now load-bearing for both. A single kill at that site would have said nothing about the other consumer; the pair is what shows the inverse property actually holds.

**A pre-existing gap surfaced by that run, NOT introduced here, and now costlier than it was.** `terminal_status_without_fix_anchor` has **no test covering the fence escape**: removing the fence skip from `structured_fix_pointers` killed this check's `fenced` seed and the parser's own unit test, but no sibling test — every sibling fixture was read and none is fenced. So a `fixed` record whose provenance block sits inside a fence would wrongly **discharge** the sibling, and nothing reds. Making the parser shared raises the price of that hole rather than creating it.

## Workarounds

Read the cited line before acting on this finding. The class it belongs to is the tell: if the hash
sits next to another artifact id or an "superseded by" phrase, it is a mention.

## Resume

Decide between direction 1 and 2 above. Direction 1 additionally requires a migration question
answered: 115 archived records already use the structured form, so the corpus is mostly there
already — count how many non-terminal records carry a prose patch-id before deciding whether the
stricter read is a breaking change or a no-op.

## References

- **`docs/issues/archive/2026-09-02-declared-patch-ids-per-line-scan-misses-a-wrapped-value.md` —
  the SAME function, failing in the opposite direction, and missed on this file's first pass.** It
  names the implementation this bug is about and which this file did not cite:
  `declared_patch_ids`, `src/librarian/tools/doctor.rs:5245-5278`. There, the scan finds `patch-id`
  and then searches **only the remainder of that same line** for the opening backtick, so a
  declaration whose 40-hex value wraps to the next line yields no match — and
  `non_terminal_status_with_fix_anchor` reports a correct-looking **zero** for a record that did
  declare its provenance. **That is a false NEGATIVE; this file is the false POSITIVE**, and the two
  bound one function from opposite sides: it misses declarations that wrap, and counts mentions that
  do not. Anyone fixing either should read both, because a widening that catches the wrapped value
  also catches more mentions. Surfaced by `git grep -il non_terminal_status_with_fix_anchor --
  'docs/issues/archive/*.md'` — one command, not run before this file was opened.
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
- Sibling defect found the same day, same mechanism, opposite direction:
  `docs/issues/archive/2026-09-13-fix-anchor-check-reports-absent-when-it-means-unparseable.md`
  — fixed and archived 2026-09-15 at `da5c1f8c`. Its § *Resume* now carries a correction
  relevant to **direction 1 above**: `declared_patch_ids` is deliberately looser than
  `structured_fix_pointers`, and its doc comment records that **0 of the LIVE corpus** used the
  structured form on 2026-09-02 — which is the population this check actually examines, since it
  skips `archive/`. That does not contradict the 115 archived records named in § *Resume*; the
  two count different populations, and only the live one binds here. The count that decides
  direction 1 is the one § *Resume* already names.
- CLAUDE.md § *Parsers Over a Namespace* — "owe an escape and a disambiguator"
- CLAUDE.md § *Testing Discipline* — a document that names one failure mode implies by omission that
  the rest are handled
