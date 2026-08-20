---
id: de718d28ad65a035
kind: bug
status: fixed
title: entry-grain edges are dropped for every intra-ledger citation, because SelfCite is decided at file grain before attribution runs
tags:
- link_scan
- entry-graph
- statement-validity
closed: 2026-08-21
---

## Symptom

No `entry_cite` row is ever created for a citation whose target is defined in the
**same file** — so a ledger's internal cross-references (`**Kin:** R-3, R-77`,
`**Chain.** R-3 → R-77 → R-79`, `kin R-3/R-28`) contribute nothing to the entry graph.

These are not a tail case. They are the densest, most deliberate edges a ledger has:
an author writing `**Kin:**` is asserting a relationship between two entries by hand.

Measured 2026-08-21 on the live catalog — `R-3` in `docs/trackers/reconnaissance-patterns.md`
is cited from inside six other entries' bodies (lines 1089, 1132, 2509, 2510, 2564, 2778):

```sql
SELECT src_slug, src_local, dst_ref FROM entry_cite WHERE dst_ref LIKE '%:R-3';
prompt-surface-compaction-session-log|F-3|reconnaissance-patterns:R-3
session-log-bug-fix-work-stream       |W-10|reconnaissance-patterns:R-3
tracker-hygiene-log                   |HY-11|reconnaissance-patterns:R-3
```

Three inbound edges, all cross-file. Zero from the ledger that actually reasons about it.

## Root cause

`src/librarian/tools/link_scan/resolve.rs`, `CitationKind::EntryToken`:

```rust
let definers = index.definers(&citation.raw);
if definers.iter().any(|d| d.artifact_id == src_id) {
    return Some(Outcome::SelfCite);
}
```

`src_id` is the **artifact** id. So "does the citing file define this token?" is answered
at file grain, and `Outcome::SelfCite` is returned. In `mod.rs` the attribution branch
lives under `Outcome::Edge` only:

```rust
Some(resolve::Outcome::Edge { dst_id }) => { ... entry_section_at(sections, c.line) ... }
Some(resolve::Outcome::SelfCite) => self_cites += 1,
```

`entry_section_at` is therefore **unreachable for same-file citations**. The classification
short-circuits before attribution, so this is not a coverage gap that better attribution
could close — the citation never arrives.

## Why the file-grain rule is right where it came from

The exclusion is correct, and load-bearing, for its original consumer.
`doctor.rs::entry_indegree` documents it:

> Same-file citations are excluded, and that is load-bearing. Measured 2026-08-20: 407 of
> 1427 ledger citations (28.5%) sit above the first definition — hand-maintained `## Index`
> rows. Counting them would let an entry's own index row inflate its own exposure, which is
> a self-reference wearing exposure's clothes.

That reasoning is sound **for exposure**, a file-level metric asking "how many other files
reach this token". It does not transfer to the entry graph, where two entries in one file
are two distinct nodes and an edge between them is exactly what the graph is for.

The defect is the same shape as the one in
`2026-08-21-entry-attribution-follows-the-first-mention-only.md`: one classification serving
two consumers whose grains differ, with the coarser consumer's rule silently inherited.

## Relationship to the first-mention bug

They are independent and they **compound**. For `R-41 → R-3` above, two suppressors stack:

1. `extract` dedupes to one `Citation` per `(kind, raw)` per document, keeping line 90 —
   a preamble line, outside every entry.
2. That surviving citation resolves `SelfCite` and is dropped before attribution.

Fixing either alone yields no edge for this case. Fixing the first-mention bug alone
recovers **zero** intra-ledger edges, because every one of them is still short-circuited
here.

## Impact

Bounded above by `counts.self_cites`, measured 2026-08-21 on a project-scoped
`link_scan(write=false)` run: **867 of 4053 citations (21%)**. That is a count of resolved
self-cite *citations* post-`extract`-dedup, not of recoverable edges — it includes
citations sitting in preambles (correctly attributable to no entry) and it under-counts
distinct `(src_entry, dst_entry)` pairs, since one document's `R-41 → R-3` and
`R-79 → R-3` collapse to a single `Citation`. It is the right order of magnitude next to
the **325 attributed / 323 derived** the same run reports, not a prediction of the delta.

## Fix

Fixed on `experiments` in **`b750419a`**, patch-id **`b25db18090f7187b1ae87f079b74f318de17a447`**
(the SHA dies on the next rebase; the patch-id survives rebase and cherry-pick).

`Outcome::SelfCite` now carries `dst_id`, so one resolution serves both grains. The caller
still refuses the file-grain edge — no `desired.insert`, `self_cites` unchanged — while
`attribute_entry_edge` keeps the entry-grain one. The only same-file citation still refused
is the true self-reference, where the citing entry IS the defining entry.

**Measured live after the fix** (project-scoped `link_scan`, same server, freshness
confirmed via `/proc/<pid>/exe` and the binary's baked-in SHA):

| | before | after |
|---|---:|---:|
| `self_cites` | 867 | **867** |
| `attributed` | 325 | 393 |
| `derived` | 323 | **391** |
| `entry_cite` rows, `origin='scan'` | 322 | **391** |
| of those, intra-ledger | 0 | **68** |
| self-loops | 0 | **0** |

**68 new edges across 17 ledgers, a 21% larger entry graph.** `self_cites` unchanged is the
load-bearing check: the file-grain verdict never moved, so exposure cannot have.

The partition closes exactly, which is what makes the delta trustworthy rather than
plausible: `attributed + outside_any_entry` went 1724 -> 2596, and the difference of 872 is
867 self-cites joining the entry partition plus 5 citations from one newly-indexed file.
Nothing appeared from nowhere.

Mutation-verified — four applied to the real source, suite run, **all four killed**: drop the
same-file guard (1 test), invert the sibling comparison (2), revert the fix (2), reintroduce
the file-grain self-loop (1, caught via `edges_missing` reporting `led -> led`). Reverting
the fix is *confirmed surviving on the prior tree* — the 4389-test suite was green with
every intra-ledger edge discarded, which is why this needed new tests rather than a green
run.

Regression tests: `an_entry_citing_a_sibling_in_its_own_ledger_records_an_edge` and
`an_entry_naming_itself_records_no_edge` (`src/librarian/tools/link_scan/mod.rs`).

### What this does NOT fix

The exemplar in *Symptom* above — `R-3` in `reconnaissance-patterns.md` — is **still
unrecovered**, and correctly so. Its first in-document mention is line 90, in the preamble,
so `extract`'s dedup hands attribution a line outside every entry and
`entry_section_at` returns `None`. That is the first-mention bug
(`2026-08-21-entry-attribution-follows-the-first-mention-only.md`), still open. The two
suppressors are independent and this fix removes one of them.

The 799 self-cites that resolved but did not attribute are now a **measured floor** on what
fixing the first-mention bug would additionally unlock — replacing that file's guessed 1461
upper bound with a number taken from the same instrument.
