---
kind: spec
status: active
title: Tracker Entry-Graph Stage 2 — entry-grain IDs + write-time cites
owners: []
tags:
  - tracker-redesign
  - stage-2
  - librarian
  - graph
---

# Tracker Entry-Graph — Stage 2 Design

**Goal:** Make tracker *entries* (not just files) first-class, globally addressable
graph nodes, and let `append_entry` record `cites` edges **at write time** — closing
TMR-1 and TMR-7 of the tracker-management redesign
(`docs/trackers/tracker-management-redesign.md`, id `3e01d4fe6de9d69b`).

**Traceability:**
- **TMR-1** — "Entries, not files, are the graph nodes, with globally unique entry IDs."
- **TMR-7** — "Edges captured at write time (append_entry accepts cites); scanner
  derivation demoted to repair/backfill only."

**Motivation (from the 2026-07-17 survey):** `link_scan` over 821 artifacts produced
2,144 citations but only 487 resolvable edges — 248 ambiguous (a bare `F-1` matches
every session log's `F-1`) and 372 dangling. Entry-grain, unambiguous, write-time
edges attack both losses at the source: an edge minted when the entry is written
carries the exact target, so it never enters the scanner's ambiguity pool.

## Scope

**In scope (additive — no catalog re-key):**
- A frozen per-tracker `slug` and `<slug>:<local>` entry ids.
- `append_entry` gains an optional `cites` field that creates `entry_cite` edges
  atomically.
- A dedicated `entry_cite` table for entry-grain edges; `link_scan` is **unchanged**
  (table separation keeps write-time edges out of its `artifact_link` prune pass).

**Explicitly out of scope (own spec later — Stage 2b / Stage 3):**
- Changing the artifact id from `sha256(abs_path)` to `sha256(rel_path)`. That is a
  **catalog-wide re-key migration** (the id is the PK across `artifact`,
  `artifact_link`, `artifact_augmentation`, `events`, `observations`) and is
  independent of the entry-graph work. It delivers cross-checkout identity
  (the shadow-clone fix), which TMR-1/TMR-7 do not require. Decision 2026-07-17:
  keep the risky re-key out of this spec (sequencing question, brainstorm).
- Full entry-node traversal queries ("all entries citing X") — a follow-on once the
  edges exist and prove useful.
- Backfilling slugs/entry-edges for the existing corpus. Slugs are minted lazily;
  old file-grain edges keep working unchanged.

## Design decisions (brainstorm 2026-07-17)

1. **Entry id = `<frozen-slug>:<local-id>`** (e.g. `tracker-mgmt-redesign:TMR-7`).
   *Why frozen, not derived:* any id derived from the current location (abs_path,
   rel_path, basename) changes on a file move — and archiving IS a move. Write-time
   edges have no prose to re-derive from, so a location-derived entry id would rot
   its edges on the next archive. Only state frozen at creation survives moves.
2. **A dedicated `entry_cite` table, slug-keyed** (NOT `artifact_link`). *Revised
   2026-07-17 during plan-scout:* `artifact_link.src_id`/`dst_id` are foreign keys to
   `artifact(id)` with `ON DELETE CASCADE`, and `PRAGMA foreign_keys = ON` is enforced
   in every `Catalog::open*`. A `<slug>:local` endpoint would be FK-rejected — and even
   with the FK dropped, `artifact(id) = sha256(abs_path)` is move-fragile, the opposite
   of what slug-based ids buy. Move-durable entry edges therefore cannot live in a
   column that FKs to `artifact(id)`. `entry_cite.src_slug` instead FKs
   `artifact(slug)` (UNIQUE) `ON DELETE CASCADE` — move-stable AND cascades cleanly.
3. **`cites` accepts any target** (rel_path | artifact_id | slug:local), resolved at
   write; unresolvable/ambiguous refs are rejected.
## Identity model

### Slug
- **Charset:** `[a-z0-9-]+` (kebab; no colon — so `<slug>:<local>` splits on its one
  colon unambiguously; both `local` ids like `TMR-7` and slugs are colon-free).
- **Minting:** lazy. When a tracker first declares an `entry_collection` (via
  `artifact_augment`) — or on the next augment of one that already has an
  entry_collection but no slug — the catalog mints a slug from
  `slugify(title)` falling back to `slugify(rel_path basename)`, deduped with a
  numeric suffix (`-2`, `-3`, …) against the unique index.
- **Storage:** new `artifact.slug` column (nullable, `UNIQUE`), mirrored into the
  artifact's frontmatter as `slug:` at mint time.
- **Immutability:** once non-null in the catalog, the slug never changes. The catalog
  is authority: on reindex, a frontmatter `slug:` that differs from the stored
  catalog slug does NOT overwrite it; a frontmatter slug that *collides* with another
  artifact's stored slug is rejected (logged, mint skipped) rather than silently
  duplicating.

### Entry id
- `<slug>:<local-id>`. The `local-id` is exactly what `append_entry` already assigns
  (`<id_prefix>-<n>`, e.g. `TMR-7`); no change to local-id assignment.
- An entry id is well-formed iff its slug prefix matches a known `artifact.slug` and
  the local id exists in that artifact's `entry_collection`.

## Schema changes (additive)

```sql
-- fresh DBs: append to schema.sql; existing DBs: idempotent v9 block in
-- apply_migrations_in_txn guarded by column_exists / table-exists.
ALTER TABLE artifact ADD COLUMN slug TEXT;                       -- nullable
CREATE UNIQUE INDEX IF NOT EXISTS ux_artifact_slug ON artifact(slug) WHERE slug IS NOT NULL;

CREATE TABLE IF NOT EXISTS entry_cite (
  src_slug   TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
  src_local  TEXT NOT NULL,                    -- e.g. "TMR-7"; entry id = src_slug||':'||src_local
  dst_ref    TEXT NOT NULL,                    -- a 16-hex artifact_id OR "<slug>:local"
  rel        TEXT NOT NULL,                    -- "cites"
  origin     TEXT NOT NULL DEFAULT 'write',    -- forward-compat; MVP only writes 'write'
  created_at INTEGER NOT NULL,
  PRIMARY KEY (src_slug, src_local, dst_ref, rel)
);
CREATE INDEX IF NOT EXISTS idx_entry_cite_dst ON entry_cite(dst_ref);
```

**No change to `artifact_link`** — it stays FK-clean, file-grain, scanner/manual as
today. (The `origin` column proposed in the first draft is dropped: table separation
makes it unnecessary — see Scanner coexistence.) The `entry_cite.origin` field is a
forward-compat placeholder; MVP inserts only `'write'`.

`artifact.slug` requires the unique `UNIQUE WHERE slug IS NOT NULL` index so the FK
target is valid (SQLite allows a FK to reference a UNIQUE column) and mint dedup can
rely on the constraint.
## Write path — `append_entry` gains `cites`

Extend `append_entry` (`src/librarian/tools/append_entry.rs` +
`catalog/augmentation.rs::append_entry`):

- New optional arg `cites: Vec<String>`.
- Inside the existing single `IMMEDIATE` transaction:
  1. **Ensure the source tracker has a slug.** Call an `ensure_slug(tx, artifact_id)`
     helper: returns the existing `artifact.slug`; if NULL, mint
     `slugify(title)` (fallback `slugify(rel_path basename)`), dedup with a numeric
     suffix against `ux_artifact_slug`, `UPDATE artifact SET slug=?`, return it.
  2. Append the entry (existing logic) → local id `<new-local>` (e.g. `TMR-7`); the
     entry global id is `<slug>:<new-local>`.
  3. Resolve each `cites` ref to a stable `dst_ref`:
     - 16-hex that exists in `artifact` → that `artifact_id`.
     - `<slug>:<local>` whose slug is a known `artifact.slug` and whose local exists
       in that artifact's `entry_collection` → that entry id verbatim.
     - a rel_path that resolves to exactly one `artifact` → that `artifact_id`.
     - unresolvable, or a bare token resolving to >1 target → **abort the whole
       transaction** with `RecoverableError` naming the offending ref and the accepted
       forms. (Unambiguous-by-construction, all-or-nothing: the entry is NOT written
       if any cite is bad.)
  4. `INSERT OR IGNORE INTO entry_cite(src_slug, src_local, dst_ref, rel, origin,
     created_at) VALUES (<slug>, <new-local>, <dst_ref>, 'cites', 'write', now)` per
     resolved ref.
- Worktree: `ensure_slug` mints on / resolves from the **main-root** artifact (see
  Integration section), so a shadow append writes `entry_cite` rows under the shared
  slug.
## Scanner coexistence (`link_scan` demoted to repair)

**`link_scan` is UNCHANGED.** Because write-time entry edges live in the separate
`entry_cite` table, `link_scan` — which only ever reads/materializes/prunes
`artifact_link` `cites` edges — never sees them. Table separation achieves the
coexistence that the first draft's `artifact_link.origin` column was going to enforce;
no filter, no origin column, no change to `diff.rs`/`resolve.rs`/`links.rs::by_rel`.

`link_scan` keeps doing exactly what it does now: parse prose citations and repair the
file-grain scanner edges in `artifact_link` (the backfill/repair path). Write-time is
the new *unambiguous* entry-grain lane (`entry_cite`); prose scanning stays the
file-grain *repair* lane — the two never collide because they live in different tables.
## Read surface (MVP-minimal)

- `get(include_links=true)` and `graph` gain an `entry_cite` read: for an artifact
  with a slug, surface outgoing edges where `src_slug = <slug>` and incoming edges
  where `dst_ref = <slug>:*` OR `dst_ref = <artifact_id>`. Present them under a
  distinct `entry_links` / `cites` grouping so a consumer can tell entry-grain edges
  from the file-grain `artifact_link` edges. This is a read-only UNION over two
  tables — no change to how `artifact_link` itself is read.
- No new standalone query API in Stage 2. "All entries citing X" is deferred until the
  stored edges demonstrate demand.
## Move durability (the payoff)

Because entry ids are `<frozen-slug>:local`, they are invariant under file move. When
a tracker is archived (`docs/trackers/X.md → docs/trackers/archive/X.md`), its
`origin='write'` entry edges keep resolving with **no re-derivation** — in contrast to
the 38 scanner edges the 2026-07-17 archive sweep cascade-dropped and had to heal via
`link_scan`. (The artifact's own file-grain id still churns on move — that is the
deferred rel_path-sha problem — but the entry edges no longer depend on it.)

## Backward compatibility

- `artifact_link` and its 452 existing `cites` edges are **untouched** — no column
  added, no rows rewritten; `link_scan` still owns them exactly as today.
- `entry_cite` is a brand-new, initially-empty table; nothing to backfill.
- `artifact.slug` is nullable and starts NULL for every existing artifact; a slug is
  minted only when an entry is first written with `append_entry` on that tracker
  (lazy). Trackers never touched by an entry-write keep `slug = NULL` and behave
  exactly as before.
- Both schema additions are additive with safe defaults; a fresh DB gets them from
  `schema.sql`, an existing DB from the idempotent v9 migration block.
## Integration with the worktree overlay (parallel work, 2026-07-17)

A parallel work-stream landed worktree-overlay machinery this session
(`CurrentProject.main_root` worktree-aware resolve, `worktree_registration`,
`covering_conn` shared with doctor, `merge_worktree` link handling — commits
`58ec4c1b`, `1bee7f9a`, `c2104e90`, `85e3d72c`). It does **not** touch this design's
five dependency files, so no design assumption changes. Two integration points, both
cleaner under the `entry_cite` model:

- **`graft::repoint_history` never touches `entry_cite`.** It re-points only
  `events`, `artifact_observation`, `artifact_link`, and `event_edges` — all by
  16-hex artifact id. `entry_cite` is a separate table keyed on `slug`, so a
  row-merge leaves entry edges alone (correct: merging two artifact rows is not an
  entry-identity change). When the `from` (shadow) artifact row is cascade-deleted
  after a merge, only its OWN slug's `entry_cite` rows would cascade — and a shadow
  row carries `slug = NULL` (next bullet), so nothing cascades.
- **Slug lives on the main-root row; a worktree shadow keeps `slug = NULL`.** Because
  `ux_artifact_slug` is UNIQUE, shadow and main cannot both hold the same slug value.
  So `ensure_slug`, when called from a worktree, resolves the **main-root** artifact
  (via `main_root` / `worktree_registration`) and mints/returns *its* slug, writing
  `entry_cite.src_slug = <main slug>`; the shadow artifact row is never given a slug.
  On `merge_worktree` the shadow row (slug NULL) is deleted harmlessly and the
  `entry_cite` rows — already keyed to the main slug — survive unchanged.
## Testing

Unit (catalog + tool layer):
- `append_entry` with `cites` appends the entry AND one `entry_cite` row per resolved
  ref, atomically; the entry's global id is `<slug>:<local>`; NO `artifact_link` row
  is created.
- `append_entry` with an **ambiguous or unresolvable** cite writes **nothing** (entry
  not appended, no `entry_cite` row) and returns `RecoverableError` naming the ref.
- `ensure_slug` mints on first entry-write (tracker had `slug = NULL`), dedups on
  collision (`foo`, `foo-2`), and is a no-op returning the same slug on the second
  call (immutability).
- **`link_scan` is undisturbed:** after `append_entry(cites=[…])`, a
  `link_scan(write=true)` run neither creates nor prunes any `entry_cite` row, and
  the entry's edges remain (table-separation guarantee).
- **Cascade:** `artifact(delete)` on a slugged tracker cascade-deletes its
  `entry_cite` rows where `src_slug = <that slug>` (FK ON DELETE CASCADE).
- **Move durability:** create a slugged tracker with an `entry_cite` edge, move it via
  `artifact(move)`, assert the `entry_cite` row is unchanged (src_slug stable) and the
  edge still resolves.
- **Worktree-merge durability:** in a worktree shadow, `append_entry(cites=[…])` on a
  tracker whose main row holds (or lazily mints) the slug; assert the `entry_cite`
  rows use the main slug and the shadow artifact row keeps `slug = NULL`; after
  `merge_worktree`, the `entry_cite` rows survive (not orphaned, not re-pointed).

Integration:
- End-to-end: `artifact(append_entry, cites=[…])` then `get(include_links=true)` shows
  the entry-grain edge under its `entry_links` grouping; `artifact_link` is unchanged.
## Open questions (non-blocking)

- Slug charset collisions with very long titles → the numeric-suffix dedup handles
  uniqueness; readability is best-effort, not a contract.
- Forward references (citing an entry that does not exist yet) are rejected in Stage 2
  (fail-fast). If a real need appears, a pending-ref queue is a follow-on.
