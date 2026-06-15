---
status: fixed
opened: 2026-06-14
severity: medium
owner: marius
related: []
tags: [retrieval, librarian, qdrant, sqlite-vec, binary-diet, L-16]
kind: task
closed: 2026-06-15
---

# TASK: Port the librarian artifact vector index (`artifact_vec`) from sqlite-vec to Qdrant

## Summary

The librarian artifact catalog keeps its own dense-vector index in a **sqlite-vec**
virtual table (`artifact_vec`), separate from the Qdrant collections backing memory
(`memories`) and code (`sync_project`). Porting it to a Qdrant `artifacts` collection
gives one vector model + one set of failure modes across all three indexes. It surfaced
in a 2026-06-14 cross-repo audit; tracked as **L-16** in
`docs/trackers/2026-05-07-legacy-retrieval-removal.md`.

> **Scope decision (2026-06-15).** This is a *consistency* port, **not** a dependency
> drop. `sqlite-vec` stays in `Cargo.toml` and the `SemanticMemoryStore` trait stays as
> the seam for a future **daemon-free local-vector-search backend** — low-end / locked-down
> systems (e.g. the `vdi-windows` worktree) can't run a Qdrant daemon and need an embedded
> store. So L-11 ("drop `sqlite-vec`") is **wontfix**; only the artifact index moves to Qdrant.
## Why it matters

- The codebase runs **two different vector stacks** in parallel (Qdrant for memory + code,
  sqlite-vec for artifacts) — duplicated infra, two failure modes, two test surfaces. The port
  collapses artifacts onto Qdrant so the server/default path is uniform.
- This does **not** shrink the binary — `sqlite-vec` is retained on purpose (local backend).
  The win is consistency + one embedding model, not a "binary diet."
- `SemanticMemoryStore` already abstracts the store; reusing it for artifacts keeps the
  embedded/local backend pluggable behind the same seam.
## Where it lives (seams to rewire)
- **Schema:** `src/librarian/catalog/schema.sql:36` — `CREATE VIRTUAL TABLE IF NOT EXISTS artifact_vec USING vec0(...)`.
- **Extension load:** `src/librarian/catalog/mod.rs:35` — `init_sqlite_vec()` (global auto-extension; the only `sqlite_vec::sqlite3_vec_init` caller left).
- **Write path:** `src/librarian/indexer.rs:250-318` — `write_embeddings` inserts/upserts artifact embeddings into `artifact_vec` (driven by `index_repo_sync`, 56-237). Behavior contract is pinned by the tests `embeds_artifact_into_vec_table`, `write_embeddings_is_idempotent_on_same_id`, `removed_file_also_removes_embedding_row` (`indexer.rs` tests module).
- **Read path:** `src/librarian/catalog/find.rs` (semantic/vector query over artifacts).
- **Migration guard:** `src/librarian/catalog/migrate_v6.rs:137` notes vec0 must be statically linked for trigger-body validation — confirm this constraint disappears once the table is gone.

## Suggested shape (mirror the memory port)

The 2026-05-13 memory port is the template — reuse, don't reinvent:
1. New Qdrant collection (e.g. `artifacts`, per-project `project_id` payload filter), payload =
   the artifact identity + metadata currently in `artifact_vec`'s sibling columns.
2. `RetrievalClient` methods mirroring `memory_upsert/search/delete/list` (`src/retrieval/memory.rs`).
3. Rewire `indexer.rs::write_embeddings` + `catalog/find.rs` onto the trait/client.
4. One-shot `migrate-artifacts` subcommand (model on `migrate-memories`, `src/migrate/memories.rs`):
   read `artifact_vec`, re-embed or copy vectors, bulk-upsert, idempotent.
5. Delete `artifact_vec` from `schema.sql` and drop `init_sqlite_vec` from `catalog/mod.rs`
   (its last caller). **Keep `sqlite-vec` in `Cargo.toml`** — retained as the daemon-free
   local-backend seam (L-11 wontfix, see Summary). Re-audit that nothing else relied on
   `init_sqlite_vec` being registered as a global auto-extension.
## Acceptance criteria

- Artifact semantic search returns equivalent results pre/post (port the contract tests).
- `init_sqlite_vec` no longer called for `artifact_vec`; the `artifact_vec` table is gone from `schema.sql`; build + full suite green.
- `sqlite-vec` **stays** in `Cargo.toml` (local backend) — do NOT assert it is removed.
- Existing on-disk `artifact_vec` data migrates without re-indexing-from-scratch being mandatory.
## Fix

Shipped on `experiments` in **3fbfbe2a** (`feat(librarian): port artifact vector
index to Qdrant + sqlite-vec escape hatch`). **Update this SHA to the master-side
SHA after cherry-pick** (Standard Ship Sequence).

**Approach** (per the 2026-06-15 plan `groovy-floating-flamingo`):
- New `ArtifactVectorStore` trait (`src/librarian/artifact_store.rs`) with two
  impls — `QdrantArtifactStore` (default) and `SqliteVecArtifactStore` (the
  retained escape hatch, delegating to `write_embeddings` to inherit its dim
  validation + BUG-045 idempotency). QdrantWrap artifact ops
  (`src/retrieval/artifact.rs`) mirror the memory port.
- Backend selection via `ArtifactBackend::resolve` — env
  `CODESCOUT_ARTIFACT_BACKEND` → `[librarian] vector_backend` → default qdrant.
  Qdrant unreachable degrades to `None` (artifact semantic search unavailable),
  not a crash.
- `find_semantic` split into a sync `find_by_ids_filtered` (hydrate + filter,
  backend-agnostic) and an async `semantic_find` coordinator (iterative-K
  backfill). MCP `reindex`, CLI `reindex_cli`/`index_repo`, and the
  `find`/`context` tools all route through the store. Project-scoped via the
  containing workspace root (stable at index, superset-safe at query; the
  catalog `scoped_filter` is the authoritative backstop).

**Scope vs. the original bug:** `sqlite-vec` is **retained** — the dep is NOT
dropped (**L-11 wontfix**). This is a consistency port + escape hatch, not the
binary-diet removal originally described.

**Tests:** 5 new (backend parse; in-memory KNN project filter; idempotent
delete; coordinator KNN-order + catalog-filter). The 3 existing contract tests
cover the sqlite-vec write path via the `write_embeddings` delegation. Full
suite **2874 passed**; clippy `-D warnings` clean.

**Deferred (non-blocking):** Qdrant delete-propagation — stale ids don't hydrate
(the catalog `IN` filter drops them), so it's a storage-hygiene follow-up (a
reindex-reconciliation pass), not a correctness gap. Live `QdrantArtifactStore`
calls are `retrieval-e2e`-gated (mirrors the memory port); covered by manual
`/mcp` verification.

**Manual verify:** `/mcp` restart → `reindex` → `artifact(find, semantic=…)`
ranks results (Qdrant default); then `[librarian] vector_backend = "sqlite-vec"`,
restart, reindex → search works with **no Qdrant running** (the vdi-windows path).
## Out of scope (related, separate)
- **Consumer-facing drift signal.** The codescout-companion's `session-start.sh` drift-warnings
  block reads the now-frozen legacy `embeddings.db::drift_report` and is silently inert. There is
  no Qdrant-era drift signal exposed for out-of-process consumers (`src/retrieval/drift.rs` only has
  internal `diff_chunks`). Re-enabling companion drift warnings needs a producer-side signal
  (mirror `index-state.json`). Track separately if desired — not part of this port.

## References
- Tracker: `docs/trackers/2026-05-07-legacy-retrieval-removal.md` (L-16 + 2026-06-14 reconciled-status section; L-11 now depends on this)
- Template: `docs/superpowers/specs/2026-05-13-memory-port-to-qdrant-design.md` (the memory port to copy)
- Freshness-signal precedent: `docs/trackers/2026-06-09-index-freshness-signal-for-consumers.md` (286ac62b) — the producer/consumer split to mirror for the drift signal
