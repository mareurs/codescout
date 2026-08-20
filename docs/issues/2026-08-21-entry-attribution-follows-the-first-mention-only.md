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

Under-counts the entry graph, silently and with a bias: the citations most likely to be
shadowed are the ones in ledgers that maintain a hand-written index table or a
`## Template` section, because those repeat every token near the top of the file. Those
are also the largest, most-cited ledgers — so the loss concentrates where the graph would
be most useful.

Not measured yet. Measuring it needs a probe that counts, per document, tokens appearing
both outside and inside an entry section. `outside_any_entry` = 1397 of 1719 attributable
citations on this corpus (2026-08-21) is an upper bound on the affected population, not an
estimate of it — most of that 1397 is documents that are not ledgers at all and have no
entries to attribute to.

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

## Fix

Not yet. Tracked for Layer 3 follow-up; see `statement-validity-session-log:F-5` for the
sibling reporting defect found in the same pass.
