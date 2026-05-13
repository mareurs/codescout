# ADR-2026-05-13 — Semantic Anchors Move to Qdrant Payload

## Status

Accepted — implemented as L-01 step 4e on `experiments`.

## Context

`memory(action="write")` on a markdown topic does two things to make the topic
discoverable beyond the topic name:

1. **Cross-embed** the markdown content into a semantic store so `recall`
   can find it by meaning (handled in [[memory-port-to-qdrant-design]] step 4d).

2. **Create semantic anchors**: embed the content, search for similar code
   chunks, deduplicate by file, and store the top hits as a "this memory is
   anchored to file X with similarity Y" mapping. Anchors let callers like
   semantic refresh detect when a memory has gone stale (the file changed).

Before this change, both pieces lived in the sqlite-vec database via two
tables: `vec_memories` (the cross-embed) and `semantic_anchors` (the
mapping). Both used `embed::index::*` helpers, which is exactly what L-01
is trying to remove.

The cross-embed migration (4d) moved the memory itself into a Qdrant
`memories` collection. This ADR records what happens to the anchors.

## Decision

**Anchors become a field on the memory payload, not a separate table.**

`SemanticMemory.anchors: Vec<MemoryAnchor>` already exists in the Qdrant
payload (added during L-01 prep). The migration:

- `create_semantic_anchors(topic, content)` rebuilds via
  `RetrievalClient::search_code(query=content)` instead of
  `embed::index::search`.
- The dedup-by-file + min-similarity filter logic moves client-side, applied
  to the `Hit` stream returned by the retrieval client.
- Anchors are written by re-upserting the memory with `anchors` populated —
  the deterministic point id ensures we overwrite, not duplicate.
- The sqlite `semantic_anchors` table is no longer written. (Its rows
  become dead and will be dropped with the rest of the legacy db in L-01
  step 8.)

**`MemoryAnchor.hash` is dropped.**

The hash field existed so callers could detect drift: if the file's current
hash diverged from the stored hash, the anchor was stale. After the move:

- Drift detection moves to the **trigger**, not the **field**. Anchors are
  rebuilt every time `memory(action="write")` runs on the topic, which is
  the only place anchors are created today.
- A separate `memory(action="refresh_anchors")` action remains for the
  *path-anchor* hash refresh, which is a different feature (file-path
  anchors in the markdown frontmatter, handled by
  `crate::memory::anchors::refresh_hashes`). That path was never on sqlite-vec
  and stays untouched.
- If staleness becomes a real problem for semantic anchors, the right place
  to detect it is the retrieval index (chunk hashes already live in
  `code_chunks` payload), not the memory record. We do not need a duplicate
  field on the memory side.

## Consequences

### New failure mode — anchor refresh now requires the chunks collection

Pre-change: `create_semantic_anchors` ran against the same local sqlite-vec
db that held the chunks index. If the sqlite db existed, the search worked.

Post-change: anchor creation calls `RetrievalClient::search_code`, which
requires:
- The retrieval stack (Qdrant + embedder + reranker) to be reachable.
- The `code_chunks` collection to be populated for this project.

If either is missing, anchor creation fails. The caller is non-fatal
(`tracing::warn!` on failure) so the memory write itself still succeeds —
the user just doesn't get semantic anchors for that write. This matches the
prior best-effort semantics; the *probability* of failure is higher because
we now depend on a remote service instead of a local file.

**Observable consequence**: a freshly cloned repo whose project has not yet
been indexed via `index(action="build")` will get no semantic anchors on
`memory(action="write")`. The user must run an index build first. The old
sqlite path could also "succeed with zero results" against a never-indexed
project, but the failure was silent rather than network-shaped.

### Score range shift

`embed::index::search` returned cosine similarity from the fastembed
embedder. `RetrievalClient::search_code` returns scores from Qdrant's RRF
fusion (Reciprocal Rank Fusion across the dense and sparse legs of
`hybrid_query`).

Qdrant's RRF formula is `score = sum_legs(1 / (1 + rank))` — **not** the
academic `1 / (60 + rank)` from the original RRF paper. Concrete ranges:

- Single-leg fusion (when sparse leg returns empty or matches nothing):
  rank-1 = 0.500, rank-2 = 0.333, rank-3 = 0.250, rank-4 = 0.200, …
- Two-leg fusion (dense + sparse both contribute): scores can stack up to
  ~1.0 when both legs rank the same point at #1.

Default `semantic_anchor_min_similarity = 0.3` therefore admits roughly the
top 1-2 results per leg and filters the long tail. This is narrower than
the legacy cosine-based filter (cosine ~0.7 default admitted hits in the
0.7-1.0 range), so users will see *fewer* anchors per memory, biased
toward the highest-confidence matches.

**Tuning guidance for users**: lower `semantic_anchor_min_similarity` to
~0.1 to admit the top 5-10 ranks. Setting it above ~0.5 effectively
disables semantic anchors entirely (only rank-1 single-leg or perfect
two-leg agreement passes).

**Verification (2026-05-13 smoke test)**: a memory mentioning
`SemanticMemoryStore`, `QdrantWrap`, and `create_semantic_anchors` produced
exactly one anchor: `src/embed/index.rs` at single-leg rank-2 (~0.33), all
other candidates filtered by `exclude_languages: ["markdown"]` or below
0.3. Behavior matches the math; no normalization or score shift in code.

**Original ADR note (incorrect — kept for historical accuracy)**: a prior
revision of this section predicted RRF scores in [0, 0.016] based on the
academic k=60 formula. Qdrant uses k=1; the prediction was an
order-of-magnitude off and the migration is more usable than originally
feared.
### `MemoryAnchor` schema

Drop `hash: String` field. Existing memories in Qdrant deserialize cleanly
because `MemoryAnchor` was new in 4d (no production data on Qdrant yet —
the prior anchor table was sqlite, not Qdrant). No migration of in-place
data is needed.

### Tests

Two anchor-coupled tests in `src/tools/memory/tests.rs` were asserting on
sqlite table state:
- `write_creates_anchor_sidecar` — rewritten to assert on
  `SemanticMemory.anchors` after fetching the memory back from the store.
- `refresh_anchors_clears_staleness` — split: the path-anchor hash refresh
  (markdown frontmatter, `refresh_hashes`) keeps its existing assertion;
  the semantic anchor staleness path is **deleted**, since drift detection
  is no longer the memory record's responsibility (see "Decision" above).

## Alternatives considered

1. **Keep `MemoryAnchor.hash`, read the hash from the chunks collection
   payload at anchor-creation time.** Rejected: couples `MemoryAnchor`'s
   schema to `CodePayload`'s schema; two walls fused at one stone.

2. **Periodic background anchor refresh.** Rejected for now. Adds a
   scheduler, a new failure mode, and operational complexity for a feature
   no caller is asking for. If staleness matters later, add it then.

3. **Keep the sqlite `semantic_anchors` table for staleness detection,
   move only the chunks search to Qdrant.** Rejected: doesn't actually
   remove the legacy dep (the whole goal of L-01); just adds a third
   storage backend behind two walls.

## Related

- [[memory-port-to-qdrant-design]] — parent design doc (steps 4a–4f, 5–8)
- [[2026-05-07-legacy-retrieval-removal]] — the larger L-01..L-11 tracker
- `docs/issues/bug-tracker.md` — log entries if the failure modes above
  bite us
