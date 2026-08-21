---
id: '1b2cdba306f4a7ca'
kind: bug
status: open
title: artifact(create) mints no slug, so a new tracker's entries are invisible to the entry graph until someone runs doctor fix=mint_slugs by hand
tags:
- librarian
- entry-graph
- slug
- statement-validity
---

## Symptom

Create a tracker, add entries that cite other entries, `reindex`, `link_scan write=true` —
and **not one edge materializes**. No error, no warning; the citations are silently counted
as `outside_any_entry`.

Observed 2026-08-21 on the first tracker created after Layer 3a shipped
(`docs/trackers/context-performance.md`, entries `CTX-1` / `CTX-2`, twelve citations between
them):

```
before mint:  derived 1345   outside_any_entry 1740
after  scan:  derived 1345   outside_any_entry 1760      <- +20, zero new edges
              SELECT slug FROM artifact WHERE abs_path LIKE '%context-performance.md'  ->  (empty)
```

After `librarian(action="doctor", fix="mint_slugs", confirm=true)` and a re-scan, all twelve
appear. **7 artifacts corpus-wide were slugless at that moment**, including two bug files
archived earlier the same day.

**It reproduced on the act of filing it.** Immediately after `mint_slugs` took coverage to
4112/4112, creating *this bug file* left exactly one slugless artifact in the catalog:

```
SELECT id, abs_path FROM artifact WHERE slug IS NULL;
1b2cdba306f4 | docs/issues/2026-08-21-create-mints-no-slug-so-new-entries-never-reach-the-entry-graph.md
```

So the reproduction is one call: `artifact(action="create", …)` then query for a NULL slug.
No scan, no comparison, no fixture needed — which is worth stating plainly, because the
*Impact* section below is about how hard the consequence is to notice, and that made the
**cause** look harder to catch than it is.
## Root cause

`entry_cite` is keyed by slug on both sides — `src_slug TEXT NOT NULL REFERENCES
artifact(slug)`, and `dst_ref` is `<dst_slug>:<TOKEN>`. `link_scan`'s
`attribute_entry_edge` therefore returns "no edge" when either endpoint has no slug, which is
correct and deliberate: a slugless endpoint cannot key a row, so degrading to no-edge beats
erroring or fabricating one.

The defect is upstream. **Layer 3a was a one-shot backfill, not an invariant.** It minted
4107/4107 and nothing mints on the create path, so every artifact born after it starts
slugless and stays that way until a human remembers a manual `doctor fix=`.

That is the same shape as
[[cross-cutting-side-effects-at-the-chokepoint]]: a property that must hold "whenever an
artifact exists" was installed at one call site (the backfill) instead of at the chokepoint
that creates them.

## Impact

Silent and self-concealing, which is the part that matters:

- the scan reports success — `written` counts only what it derived, and it derived nothing;
- `outside_any_entry` rises, but that counter has a large legitimate population (1760), so
  +20 is invisible without a before/after;
- `doctor`'s `catalog_health.slug_coverage` **does** report `without_slug`, but nothing in the
  create → reindex → link_scan flow points at it.

The blast radius grows with adoption: every new ledger is born outside the entry graph, and
the graph is now what Layers 4 and 5 consume.

## Fix

Not yet applied. Mint at the chokepoint rather than at a call site — `ensure_slug` already
exists in `src/librarian/catalog/artifact.rs` and is idempotent; the question is which
function is the true chokepoint for "an artifact row now exists" (`create`, `upsert`, and the
reindex classifier all reach it).

Two guards worth keeping whichever way it lands:

1. **Slugs are immutable once minted**, so minting early is safe but minting from an
   incomplete title is not — check what `create` knows at the moment it would mint.
2. A `link_scan` run that finds a slugless artifact should **say so** rather than silently
   attributing its citations to nothing. That is the detector that would have caught this in
   the first minute instead of after a full scan-and-compare.

Interim: run `librarian(action="doctor", fix="mint_slugs", confirm=true)` after creating any
artifact whose entries are meant to be citable.
