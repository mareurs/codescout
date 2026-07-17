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
- `append_entry` gains an optional `cites` field that creates edges atomically.
- `link_scan` scoped to only its own (scanner-origin) edges so it never prunes
  write-time edges.

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
2. **Reuse `artifact_link` with polymorphic string endpoints** (not a new table):
   an endpoint is either a 16-hex `artifact_id` (file node) or a `<slug>:<local>`
   entry id. `get`/`graph`/`link_scan` already read this one table.
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
ALTER TABLE artifact      ADD COLUMN slug   TEXT;                    -- nullable
CREATE UNIQUE INDEX ux_artifact_slug ON artifact(slug) WHERE slug IS NOT NULL;

ALTER TABLE artifact_link ADD COLUMN origin TEXT NOT NULL DEFAULT 'scanner';
-- existing 452 rows backfill to 'scanner' via the DEFAULT.
```

`origin` domain: `'scanner' | 'write' | 'manual'`. (`'manual'` covers
`artifact(action="link")`; it already coexists today only because it uses non-`cites`
rels — the column makes the ownership explicit and future-proofs `cites` collisions.)

## Write path — `append_entry` gains `cites`

Extend `append_entry` (`src/librarian/tools/append_entry.rs` +
`catalog/augmentation.rs::append_entry`):

- New optional arg `cites: Vec<String>`.
- Inside the existing single `IMMEDIATE` transaction, AFTER the entry is appended
  and its global id `<this-slug>:<new-local>` is known:
  1. Resolve each `cites` ref to a stable endpoint:
     - 16-hex that exists in `artifact` → that `artifact_id`.
     - `<slug>:<local>` whose slug is known and local exists → that entry id.
     - a rel_path that resolves to exactly one `artifact` → that `artifact_id`.
     - anything unresolvable, or a bare token resolving to >1 target → **abort the
       whole transaction** with `RecoverableError` naming the offending ref and the
       accepted forms. (Unambiguous-by-construction: the entry is NOT written if any
       cite is bad — atomic all-or-nothing.)
  2. Insert `artifact_link(src_id = <this-entry-global-id>, dst_id = <resolved>,
     rel = "cites", origin = "write", created_at)` for each, `INSERT OR IGNORE`.
- The source tracker must have a slug; if it has an `entry_collection` but no slug yet
  (pre-existing tracker), mint it in the same transaction before computing the entry
  id.

## Scanner coexistence (`link_scan` demoted to repair)

- `link_scan`'s materialize/prune differ (`src/librarian/tools/link_scan/diff.rs`,
  which loads the scanner edge set via `links::by_rel("cites")`) is scoped to the
  exact predicate **`rel = 'cites' AND origin = 'scanner'`**. It never sees,
  materializes, or prunes `origin = 'write'` or `origin = 'manual'` edges. Concretely,
  `by_rel` (or its caller) gains an `origin='scanner'` filter so the differ's
  "existing scanner edges" set excludes write/manual rows.
- **The manual-link path must set `origin='manual'`.** `artifact(action="link")`
  (`src/librarian/tools/link.rs`) today writes edges that survive only because they
  use non-`cites` rels; to keep a manual `rel="cites"` edge from being pruned, the
  link tool writes `origin='manual'` explicitly rather than relying on the
  `'scanner'` column default.
- Everything else about `link_scan` is unchanged: it still parses prose citations and
  repairs file-grain scanner edges (the backfill/repair path, e.g. after the
  rel_path re-key or a bulk move). Entry-token prose citations continue to resolve to
  their defining artifact (file-grain) as today — write-time is the new *unambiguous*
  path, prose scanning is the *repair* path, exactly as TMR-7 states.

## Read surface (MVP-minimal)

- `get(include_links=true)` and `graph` read `artifact_link` unchanged; edges whose
  endpoints are `<slug>:<local>` surface as-is. Consumers that display endpoints learn
  the format (colon ⇒ entry; 16-hex ⇒ artifact).
- No new query API in Stage 2. "All entries citing X" is deferred until the stored
  edges demonstrate demand.

## Move durability (the payoff)

Because entry ids are `<frozen-slug>:local`, they are invariant under file move. When
a tracker is archived (`docs/trackers/X.md → docs/trackers/archive/X.md`), its
`origin='write'` entry edges keep resolving with **no re-derivation** — in contrast to
the 38 scanner edges the 2026-07-17 archive sweep cascade-dropped and had to heal via
`link_scan`. (The artifact's own file-grain id still churns on move — that is the
deferred rel_path-sha problem — but the entry edges no longer depend on it.)

## Backward compatibility

- Existing 452 `cites` edges → `origin='scanner'`, still managed by `link_scan`.
- Trackers without a slug: unaffected until augmented; their entries remain
  file-grain-addressable via prose + scanner.
- No existing row is rewritten; both new columns are additive with safe defaults.

## Testing

Unit (catalog + tool layer):
- `append_entry` with `cites` writes the entry AND one `origin='write'` edge per
  resolved ref, atomically; the returned id is `<slug>:<local>`.
- `append_entry` with an **ambiguous or unresolvable** cite writes **nothing**
  (entry not appended, no edge) and returns `RecoverableError` naming the ref.
- `append_entry` on a tracker with an `entry_collection` but no slug mints the slug
  in-transaction, then writes the entry+edges.
- slug mint dedupes on collision (`foo`, `foo-2`); a second mint attempt on an
  already-slugged artifact is a no-op (immutability).
- `link_scan(write=true)` prune pass leaves `origin='write'` and `origin='manual'`
  edges intact while still pruning a stale `origin='scanner'` edge.
- **Move durability:** create a tracker with a slug + a write-time entry edge, move it
  via `artifact(move)`, and assert the entry edge still resolves (endpoint unchanged).

Integration:
- End-to-end: `artifact(append_entry, cites=[…])` then `get(include_links=true)`
  shows the entry-endpoint edge; `link_scan(write=true)` does not disturb it.

## Open questions (non-blocking)

- Slug charset collisions with very long titles → the numeric-suffix dedup handles
  uniqueness; readability is best-effort, not a contract.
- Forward references (citing an entry that does not exist yet) are rejected in Stage 2
  (fail-fast). If a real need appears, a pending-ref queue is a follow-on.
