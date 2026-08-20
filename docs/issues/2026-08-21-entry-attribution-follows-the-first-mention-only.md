---
kind: bug
status: open
title: entry-grain attribution follows a token's FIRST mention, so a passing reference above an entry consumes the real one
closed:
---

## Symptom

`link_scan`'s entry-grain materializer (`origin='scan'`, Layer 3b) attributes an edge to
the entry containing a citation's line. When one document mentions a token more than
once, only the **first** mention survives to be attributed — so a passing reference in a
preamble, an index table, or a `## Summary` **consumes** the citation, and the entry that
genuinely rests on that token records no edge at all.

Pinned by `a_token_first_mentioned_outside_an_entry_loses_its_entry_attribution`
(`src/librarian/tools/link_scan/mod.rs`), which asserts `derived == 0` for a file whose
`## W-1` body cites `F-1` directly, because a line above it mentioned `F-1` first.

## Root cause

`extract::push_citation` (`src/librarian/tools/link_scan/extract.rs`):

```rust
if seen.insert((kind, raw.clone())) {
    out.citations.push(Citation { raw, kind, line });
}
```

One `Citation` per `(kind, raw)` per document, carrying the first occurrence's line.
Every later occurrence is discarded **with its line**.

**That dedup is correct and load-bearing for its original consumer.** `entry_indegree`'s
doc comment is explicit: exposure is "how many other files reach this token at least
once", and file-level counting is what stops one chatty file inflating a token's apparent
reach. The brief that built it assumed occurrence-counting and its own test was wrong
about the number for exactly this reason.

The defect is that Layer 3b added a **second consumer with a different need**. Attribution
wants per-occurrence position; exposure wants per-document presence. One `Vec<Citation>`
serves both, and the dedup that makes exposure right makes attribution lossy.

## Impact

Under-counts the entry graph, silently, with a bias toward the ledgers where the graph
would be most useful.

**Measured 2026-08-21** by running the real `extract` over each entry section's own text
(same fence and definition rules — no second token detector) and counting citations whose
first mention falls outside every entry while the same token IS cited from inside one:

```
docs read              4103
of those, ledgers       277
citations (all)        6380
first mention outside  3374
SHADOWED               1461   in 139 ledger(s)
```

Worst offenders, and they are exactly the predicted shape — ledgers carrying a
hand-maintained `## Index` table that lists every id near the top of the file, so the
table's mention always precedes the entry's own:

| shadowed | ledger |
|---:|---|
| 107 | `codescout docs/trackers/reconnaissance-patterns.md` |
| 94 | `southpole/MRV-poc docs/trackers/reconnaissance-patterns.md` |
| 89 | `backend-kotlin docs/trackers/gantt-rebalance-session-log.md` |
| 74 | `codescout docs/trackers/bug-fix-session-log.md` |
| 74 | `backend-kotlin docs/trackers/solver-invariants.md` |

**1461 is an upper bound on recoverable edges, not an estimate of them.** Two reasons it
overstates, both worth stating rather than quietly correcting for:

1. It is **unfiltered by resolution** — it counts every shadowed citation, including ones
   that would resolve `Ambiguous`, `Dangling` or `CrossRepo` and so would never become an
   edge. On the project-scoped run, roughly half of all citations reach `Edge`.
2. It is **corpus-wide** (4103 docs, every repo), while the materializer that produced the
   current 322 edges ran project-scoped over 1099 artifacts. The two numbers describe
   different populations and must not be divided by one another.

Even discounted for both, the shadowed population is the same order as the entire
materialized graph — so this is the dominant limitation on entry-grain coverage, not a
tail case.

There is also a second-order effect worth naming: `## Index` tables are precisely what
`get_guide("tracker-conventions")` § *One entry format, never two* already discourages,
for an unrelated reason (rows define no citable token). This bug gives that guidance a
second, independent cost.
## Why it is not fixed here

Two options, both larger than the change that surfaced this:

1. **Emit every occurrence and let consumers dedupe.** Correct, and it moves the
   file-level guarantee `entry_indegree` depends on out of `extract` and into its caller
   — a behaviour change to the exposure metric that three shipped `doctor` checks are
   gated on.
2. **Attribute by re-scanning for occurrences inside each entry section.** Leaves
   `extract` alone but creates a second implementation of token detection, which is the
   drift this module's own doc comments repeatedly argue against ("so the three can never
   disagree about what a definition, a fence, or frontmatter is").

Option 1 is probably right, sequenced with its own measurement of the exposure delta. It
should not ride along with the materializer that revealed it.

### Correction, 2026-08-21 — this rationale omitted a resolution outcome

The *Impact* caveat above lists the outcomes that stop a shadowed citation becoming an
edge as "`Ambiguous`, `Dangling` or `CrossRepo`". **`SelfCite` is missing, and it was the
one that dominated this file's own top-offenders table** — every ledger listed there is
shadowed largely by ids it defines itself.

That omission mattered twice over:

- It **inflated the 1461**. Shadowed citations resolving `SelfCite` were, at the time this
  was written, discarded before attribution regardless of the dedup, so they were not
  recoverable by fixing the first-mention bug at all.
- It **hid a cheap fix**. `SelfCite` being decided at file grain was a separate defect with
  a fix confined to `link_scan`, touching no `extract` behaviour and therefore unable to
  move exposure — shipped as `b750419a`,
  `docs/issues/archive/2026-08-21-selfcite-is-file-grain-so-intra-ledger-entry-edges-never-materialize.md`.
  The two-options survey read as exhaustive and was not.

This is the R-95 shape: a deferral rationale is written at the moment someone decides to
stop, so nothing in it argues the work is cheaper than the decision to stop implies.

### The estimate, replaced by a measurement

With `SelfCite` fixed, the population this bug still blocks is directly observable rather
than bounded. Of the 867 self-cite citations now reaching attribution, **68 attributed and
799 did not** — and the 799 are overwhelmingly index-table mentions, i.e. exactly this bug.

**799 is a measured floor on what fixing this would additionally unlock**, project-scoped,
on the same instrument that reports the current 391 edges. It supersedes the 1461, which
was corpus-wide, unfiltered by resolution, and inflated by the omission above. The two
numbers describe different populations; do not compare them.

The exemplar is `R-3` in `docs/trackers/reconnaissance-patterns.md`: cited from inside six
entries' bodies, first mentioned on line 90 in the preamble, and still recording zero
intra-ledger edges after `b750419a` for that reason alone.
## Fix

Not yet. Tracked for Layer 3 follow-up; see `statement-validity-session-log:F-5` for the
sibling reporting defect found in the same pass.
