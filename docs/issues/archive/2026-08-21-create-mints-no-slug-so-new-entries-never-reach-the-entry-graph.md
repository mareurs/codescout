---
id: 0f8be14975d1ac4a
kind: bug
status: fixed
title: artifact(create) mints no slug, so a new tracker's entries are invisible to the entry graph until someone runs doctor fix=mint_slugs by hand
tags:
- librarian
- entry-graph
- slug
- statement-validity
closed: 2026-08-21
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

Applied. Added `catalog::artifact::upsert_and_mint_slug` (`upsert` + `ensure_slug`) and switched `create`, `update`, and `indexer::index_repo_sync` to call it instead of bare `upsert`.

`mv` was deliberately left calling bare `upsert`: a move mints a fresh id while the old row (and its slug) still exists until `graft_rows` deletes it, so minting eagerly there would find the old slug "taken" and hand the new row a needless, permanent `-2` suffix (slugs are immutable once minted — found while tracing the fix, not in the original report). `graft_rows` now carries the old row's slug forward explicitly, capturing it before the delete and writing it after, since the UNIQUE(slug) index rejects both rows holding it at once.

Guard 2 from the original write-up (`link_scan` should say so when it finds a slugless artifact, rather than silently attributing its citations to nothing) was **not** implemented — out of scope for this fix, left as a follow-up idea.

- **SHA (experiments):** `05a3ab168840f41c8b6ba1c93ff1e99a9fde4879`
- **patch-id:** `9d36384adb23a474ccf1aeb7f970bed27feda1fb`

## Tests added

All in the same commit:

- `catalog::artifact::tests::upsert_and_mint_slug_mints_where_bare_upsert_does_not` — the wrapper mints where bare `upsert` doesn't.
- `tools::create::tests::create_mints_a_slug_for_the_new_artifact` — the bug as reported.
- `tools::update::tests::update_mints_a_slug_for_a_row_that_was_never_given_one` — same chokepoint, patch path.
- `indexer::tests::reindex_mints_a_slug_for_every_newly_indexed_row` — same chokepoint, reindex path.
- `catalog::graft::tests::graft_carries_the_slug_forward_so_a_move_does_not_orphan_it` — regression test for the move-collision hazard found while implementing the fix; RED against the pre-fix `graft_rows` (asserted `None`, expected `Some("my-tracker")").

All 5 written RED-first (4 failed on missing mint / lost slug; 1 failed to compile — `upsert_and_mint_slug` didn't exist yet), confirmed GREEN after the fix. Full `cargo test` + `cargo clippy --all-targets -- -D warnings` clean on `experiments`.
