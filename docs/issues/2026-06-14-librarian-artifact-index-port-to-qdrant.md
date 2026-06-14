---
status: open
opened: 2026-06-14
severity: medium
owner: marius
related: []
tags: [retrieval, librarian, qdrant, sqlite-vec, binary-diet, L-16]
kind: task
---

# TASK: Port the librarian artifact vector index (`artifact_vec`) from sqlite-vec to Qdrant

## Summary
The librarian artifact catalog keeps its own dense-vector index in a **sqlite-vec**
virtual table (`artifact_vec`). It is the **sole remaining `sqlite-vec` consumer** in
the tree and therefore the last blocker for dropping `sqlite-vec` (and the transitive
`fastembed`) from `Cargo.toml` — the "binary diet" that motivated the Qdrant migration.
Memory (`memories` collection) and code (`sync_project`) are already on Qdrant; this
third index was never in the original Phase-7 punch-list and surfaced in a 2026-06-14
cross-repo audit. Tracked as **L-16** in `docs/trackers/2026-05-07-legacy-retrieval-removal.md`.

## Why it matters
- `sqlite-vec = "0.1"` stays in `Cargo.toml` (×2) only because of this table → binary diet stalls.
- The codebase now runs two embedding/vector stacks in parallel (Qdrant for memory+code,
  sqlite-vec for artifacts) — duplicated infra, two failure modes, two test surfaces.

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
5. Delete `artifact_vec` from `schema.sql`, drop `init_sqlite_vec` from `catalog/mod.rs`, then
   remove `sqlite-vec` (and re-audit `fastembed`) from `Cargo.toml` — closes **L-11**.

## Acceptance criteria
- Artifact semantic search returns equivalent results pre/post (port the contract tests).
- `git grep sqlite_vec -- 'src/**/*.rs'` → empty; `sqlite-vec` gone from `Cargo.toml`; build + full suite green.
- Existing on-disk `artifact_vec` data migrates without re-indexing-from-scratch being mandatory.

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
