# ADR: artifact_vec stays a single shared table; migration must name affected projects

- **Date:** 2026-07-20
- **Status:** accepted (with one open follow-up)
- **Deciders:** Marius (with the Architecture Snow Lion)
- **Commits:** `109c1ead` (feat(librarian): opt-in artifact_vec dimension
  migration), on `experiments` as of `bb693446` (PR #8, merged 2026-07-19).

## Decision

Keep `artifact_vec` as a single, fixed-width sqlite-vec table per user
catalog — do **not** namespace it per project or per embedding model. The
opt-in `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1` escape hatch (`rebuild_artifact_vec_at_dim`,
`src/librarian/indexer.rs`) is the right shape for recovering from a dimension
mismatch. One gap remains open: the warning it emits describes the blast
radius abstractly ("every project sharing this catalog") instead of naming
the concrete projects it is about to affect.

## Context / forces

- `catalog.db` resolves to `dirs::data_local_dir().join("librarian/catalog.db")`
  by default (`build_tool_context_with`, `src/librarian/mod.rs:70-208`) — one
  file per user machine, shared across every project registered in that
  user's workspace. This is the existing, load-bearing design across the
  whole librarian subsystem, not something introduced by this change.
- `artifact_vec` is a sqlite-vec `vec0` virtual table
  (`src/librarian/catalog/schema.sql:49-52`): `id TEXT PRIMARY KEY, embedding
  FLOAT[768]`. The embedding column's width is a **table-level** property
  fixed at `CREATE VIRTUAL TABLE` time — `vec0` has no per-row dimension.
  Rows are keyed only by a globally-unique artifact id (`sha256(abs_path)`),
  with zero project or model namespacing already present.
- Before this change, `write_embeddings` unconditionally `anyhow::bail!`'d on
  any dimension mismatch, citing prior incidents ("bug-tracker #6/#7"). The
  hard-fail-on-mismatch invariant predates this ADR; what's new is a
  supervised way through it.
- Only one concrete trigger for a mismatch is on record: a single project's
  embedder model/backend being switched after the catalog was already
  indexed (the "F-6b" error-sentinel incident). No second project has ever
  needed a *different* embedding dimension to coexist in the same catalog
  at the same time.
- `sqlite-vec` is not legacy cruft awaiting removal — it was deliberately
  **retained** as a permanent escape hatch when the artifact index was
  ported to Qdrant (`docs/issues/archive/2026-06-14-librarian-artifact-index-port-to-qdrant.md`,
  fixed in `3fbfbe2a`, tracked as **L-11 wontfix** for dropping the
  dependency). It exists for a named, concrete scenario: no Qdrant running
  at all ("the vdi-windows path"), verified by that task's own manual-test
  note. This means the dimension-mismatch migration path this ADR covers is
  a permanent fixture, not a stopgap that disappears once a Qdrant port
  finishes — that port already shipped, and `artifact_vec` still exists by
  design.

## Alternatives considered

- **Per-model namespaced tables** (`artifact_vec_<model_hash>`), so two
  projects could run two different embedders concurrently without evicting
  each other. Rejected for now: only one concrete case exists (a single
  project's model swap), not two. This project already applies a
  rule-of-three discipline against exactly this kind of speculative
  extraction elsewhere (`tool-registration-rule-of-three`) — building the
  namespaced-table shape today would freeze an interface around a sample of
  one.
- **Silent auto-migrate on mismatch.** Rejected — a destructive,
  cross-project operation must never be silent regardless of how the
  boundary is drawn.
- **Status quo: hard `bail!`, no escape hatch.** This was the prior
  behavior. Rejected because it forced every legitimate model-swap onto
  undocumented, manual `DROP TABLE` surgery — strictly worse than a guarded,
  backed-up migration path.

## Mechanism

- Gate: `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1` (`ARTIFACT_VEC_MIGRATE_ENV`,
  `src/librarian/indexer.rs`). Default off; a dimension mismatch without the
  flag stays a loud, safe stop.
- On opt-in, `rebuild_artifact_vec_at_dim` backs up `catalog.db` (skipped for
  in-memory catalogs) to a timestamped sibling
  (`catalog.db.pre-vec-dim-bak.<unix_ts>`), then drops and recreates
  `artifact_vec` at the new dimension, and logs a `tracing::warn!` describing
  the blast radius.
- The warning text today: *"this deletes vectors for every project sharing
  this catalog"* — true, but generic. The data to make it concrete already
  sits in the same table the migration is about to touch: `artifact.abs_path`
  carries every indexed project's root. A `SELECT DISTINCT` project-root
  prefix before the drop would let the warning (and ideally the tool's JSON
  response) name the actual projects about to lose their vectors, not just
  gesture at an abstract set.

## Consequences

- **Now easier:** switching embedding models/backends on an already-indexed
  sqlite-vec catalog has one documented, safe recovery path instead of
  hand-rolled SQL surgery.
- **Now harder:** a user on Project A flipping the env var to fix *their*
  mismatch silently wipes every other project's vectors sharing that
  catalog too, and today's warning doesn't tell them which ones. That's the
  cost of NOT closing the follow-up below — it's a real, currently-open gap,
  not a hypothetical.

## Change scenarios absorbed

- Embedding-model or backend swap on an existing sqlite-vec-backed catalog —
  the one concrete case on record.
- Explicitly does **not** absorb "two projects want two different embedding
  models permanently, at the same time" — no data says that's a real need
  yet (see Revisit-when).

## Revisit-when

- **Open follow-up (this ADR's one actionable item):** upgrade the warning
  in `rebuild_artifact_vec_at_dim` to enumerate the actual project roots
  present in `artifact.abs_path` before dropping the table, so the operator
  sees the concrete blast radius, not an abstract one. Not yet implemented
  or tested as of this ADR — confidence on "this is cheap to query at that
  point" is medium, not high.
- **Namespacing follow-up:** if a second concrete project ever needs a
  genuinely different embedding model to coexist permanently against the
  same shared catalog (not just a one-time swap), the per-model-namespaced-
  table alternative earns its complexity at that point — not before.

## Confidence

**High** that per-model namespacing would be premature right now — the
rule-of-three call is clean and consistent with this project's existing
discipline. **Medium** on the "name the affected projects in the warning"
recommendation — the data path (`artifact.abs_path`) is real and verified,
but the change itself is unwritten and unverified.

## Sites (initial)

- `src/librarian/indexer.rs` — `write_embeddings`, `rebuild_artifact_vec_at_dim`,
  `ARTIFACT_VEC_MIGRATE_ENV`
- `src/librarian/catalog/schema.sql` — `artifact_vec` table + cascade-delete trigger
- `src/librarian/mod.rs` — `build_tool_context_with` (catalog.db path resolution)

## References

- `docs/issues/archive/2026-06-14-librarian-artifact-index-port-to-qdrant.md` — the
  Qdrant port that deliberately retained `sqlite-vec` as a permanent escape
  hatch (L-11 wontfix), fixed in `3fbfbe2a`. This ADR's migration path only
  matters for that retained backend.
