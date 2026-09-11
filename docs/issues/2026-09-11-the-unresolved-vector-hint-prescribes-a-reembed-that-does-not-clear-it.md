---
id: '43f3e527cda1f9af'
kind: bug
status: open
title: 'BUG: the unresolved-vector hint prescribes a reembed that does not clear it, and reports a per-query count as a store total'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
---

# BUG: the unresolved-vector hint prescribes a reembed that does not clear it, and reports a per-query count as a store total

## Summary
`doc(action="find", semantic=…)` emits `hints.unresolved` with:

> *"N vector(s) the store returned resolved to no chunk row and were discarded before ranking.
> Retrying will not change this: the vectors are stale, or the store holds ids at a grain this
> reader cannot resolve. Re-index to refresh them -- `librarian(action="reindex", reembed=true)`."*

Two defects, and the second is why the first goes unnoticed:

1. **The prescribed remedy does not clear the condition.** A full `reindex(reembed=true)` ran to
   completion — 30364 embedded, 9 minutes, holding the shared catalog write lock — and the hint
   still fires afterwards.
2. **`N` is a PER-QUERY count published in the grammar of a store total.** *"the store
   returned"* reads as a property of the store. It is the count of unresolvable vectors in that
   one query's ANN neighbourhood, so it cannot be trended and two readings never compare.

## Symptom (Effect)
A reader sees a specific number and a specific command, runs the command, waits nine minutes,
blocks every peer on the shared write lock, and gets the same hint back. Because the number
*moves* between readings, the natural conclusion is "partially fixed, run it again" rather than
"this remedy cannot reach it". Two sessions were queued behind the lock during the run that
produced this file.

## Reproduction
Measured 2026-09-11 at `13859878`, release binary (identity confirmed from compiled-in schema
strings, not mtime).

Before: `doc(action="find", kind="bug", limit=3, query="shrink guard on code edits")`
→ `unresolved: 38`.

Then `librarian(action="reindex", reembed=true)` → `added: 0, updated: 37, unchanged: 1552,
embedded: 30364, vectorless: 0, orphans_removed: 0`.

After, same query text → `unresolved: 31`. **The hint still recommends the same command.**

Then three DIFFERENT queries at one instant against the same store:

| semantic query | `unresolved` |
|---|---|
| `shrink guard on code edits` | 31 |
| `gitignore anchoring of patterns with internal slashes` | 21 |
| `peer session enumeration across profiles` | 46 |

## Environment
codescout MCP, `doc(action="find", semantic=…)` + `librarian(action="reindex")`. Project
codescout, branch `experiments`, HEAD `13859878`. Local embeddings (540 MB,
`.codescout/embeddings`, 4 files).

## Root cause
Not established at the code level; two candidates, both consistent with the measurements, and
the evidence below rules out a third.

**Ruled out — the `vectorless` population.** This is NOT
`docs/issues/archive/2026-09-07-vectorless-note-prescribes-a-reembed-that-cannot-reach-it.md`
recurring. That bug's population is artifacts with **no chunk rows**, and its remedy is
`codescout backfill-chunks`. The reindex here reported `vectorless: 0` —
*"every indexed artifact has a vector"* — so no artifact lacks chunk rows and `backfill-chunks`
has nothing to backfill. The archived fix is intact; this is a **second surface carrying the
same defective prescription**, which the fix at `45eac50e` corrected in `reindex.rs`'s
`vectorless_note` and did not sweep for elsewhere.

**Candidate A — stale vector ids.** Re-chunking mints new `artifact_chunk` ids; vectors keyed to
retired ids survive in the store. `reembed=true` requeues *existing* chunks for embedding and
prunes nothing, so an orphaned vector is untouched by construction — the identical mechanism the
archived sibling identified, reached by a different route. `orphans_removed: 0` is consistent
with an orphan sweep that looks at augmentations rather than vectors.

**Candidate B — the hint's own second clause**, *"the store holds ids at a grain this reader
cannot resolve"*. Unfalsified here, and it would make the population permanent rather than
stale.

Either way there is no shipped command that prunes the vector store, which is why both the hint
and this file can only name a remedy that does not perform.

## Evidence
### The delta is not evidence, and the correction is the point
`38 -> 31` looks like a partial fix and **is not interpretable**. The per-query table above
settles it: `unresolved` varies 21/31/46 across three queries at a single instant, so it is a
sample, not a total. After a full reembed every vector is new, so the ANN neighbourhood differs
even for byte-identical query text — the before and after are different samples of different
populations.

**So neither "7 rows were fixed" nor "nothing moved" is supportable**, and this file asserts
neither. What survives without the delta is the only claim needed: the condition is present
AFTER a full reembed, at 21, 31 and 46 on three queries.

This correction was forced by a peer (sessionId `b0b9bc40-5358-4a44-b342-a2a71dc50fad`) pointing
out that my "opposite direction, orthogonal operation" account predicts `38 -> 38` and seven
rows had moved. Their two candidate readings — partial population overlap, or corpus churn —
were both closer than my account and both wrong about the mechanism; the metric being
query-scoped is neither. Recorded because the reasoning is the transferable part: **a moving
number is what a future reader will use to argue this file is wrong**, so the file has to say
why it moves.

### 46 > 38 — the direction that would read as regression
The highest of the three post-reembed readings exceeds the original 38. A reader treating the
number as a store total concludes the reembed made it worse. It did not; the number is not a
total. That is defect 2 doing the damage, and it is what makes defect 1 survive contact with
anyone who checks.

## Hypotheses tried
1. **Hypothesis:** the archived `vectorless_note` bug has recurred.
   **Test:** read the archived file; compare its population against this reindex's report.
   **Verdict:** rejected. Its population is `vectorless > 0`; this run reports `vectorless: 0`.
   Same mechanism class, different population, different surface, and its fix is intact.

2. **Hypothesis:** `reembed=true` is simply orthogonal, so the count should not move at all.
   **Test:** the three-query table.
   **Verdict:** rejected as *stated*, because it made a prediction about a number that is not a
   total. The orthogonality claim may still hold at the row level; the count cannot test it.

## Fix
*Not fixed here.* Two separable changes, and the second is cheap:

**The hint must not prescribe `reembed=true`.** It is the same correction `45eac50e` applied to
`reindex.rs`'s `vectorless_note`, at the surface that fix did not reach. Until a pruning path
exists, the honest text names the condition and says no remedy is available — a documented
limitation costs a reader far less than a nine-minute no-op that takes the shared write lock.

**The count needs its unit.** *"N vectors the store returned"* → *"N of this query's candidate
vectors"*, so nobody trends it or reads a rise as regression.

**And sweep for siblings rather than fixing this one site.** Two surfaces have now carried this
prescription and one was fixed in isolation. `grep` for `reembed=true` across hint and note
strings is the whole check — CLAUDE.md § *Testing Discipline*: mutate once per guarded SITE, not
once per feature, and a remedy string is a site.

## Tests added
None — nothing is fixed yet. The test worth having with the fix is a shape assertion in both
directions, exactly as `45eac50e` built for its sibling: that the hint does **not** contain
`reembed`, and that it does name whatever the real escape turns out to be. Either assertion
alone is satisfiable by the wrong text.

## Workarounds
None that clear it. **Do not run `reindex(reembed=true)` for this hint** — it costs ~9 minutes,
takes the shared catalog write lock, and blocks every peer's `edit_code`/`edit_file`.

Read the hint as "semantic recall on this query is a floor" and proceed. Ranking is unaffected
for the vectors that do resolve; the discarded ones only cost recall.

## Resume
Decide between correcting the hint text alone (cheap, honest, leaves recall degraded) and adding
a vector-store prune path (the actual repair). Then sweep every `reembed=true` occurrence in
user-facing strings — this is the second such surface found, so the population is at least 2 and
was never enumerated.

## References
- `docs/issues/archive/2026-09-07-vectorless-note-prescribes-a-reembed-that-cannot-reach-it.md`
  — the sibling, fixed `45eac50e` / patch-id `0f70f33bbc19f0a95ecb08a1966592e63a9b05d0`; same
  class, different population, and the fix this file shows was applied at one site of two
- `docs/issues/archive/2026-09-07-backfill-chunks-walks-the-whole-catalog-not-the-project.md`
  — the sibling's own sibling, on the remedy it names
- `docs/trackers/issue-clusters/IC-20-floor-published-under-the-name-of-a-total.md` — the second
  axis: a per-query sample published in the grammar of a total
