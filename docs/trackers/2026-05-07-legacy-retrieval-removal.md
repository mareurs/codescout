---
kind: tracker
status: draft
title: Legacy Retrieval Removal — Phase 7 residuals
owners: []
tags:
  - retrieval
  - phase-7
---

# Legacy Retrieval Removal Tracker (Phase 7 residuals)

**Created:** 2026-05-07 · **Status:** open

Phase 6 graduated the Qdrant + TEI hybrid stack to default for `semantic_search`
of code. Phase 7 (narrow) drops the `CODESCOUT_RETRIEVAL_BACKEND` knob and the
legacy code-search `else` branch. **Everything else listed here still depends on
`src/embed/index.rs` (sqlite-vec / tantivy / local ONNX embedder) and blocks the
full deletion of `src/embed/index.rs` / `src/embed/bm25.rs` / `src/embed/fusion.rs`.**

Treat this as the punch-list before `cargo rm src/embed/index.rs` becomes safe.

## 2026-06-14 audit — reconciled status

A code audit (run cross-repo from claude-plugins) found this tracker **stale**:
the 2026-05-13 memory-port design landed and closed most items below.

**Done in code (still listed "open" in the table):**
- **L-01** ✅ — `SemanticMemoryStore` trait + `QdrantSemanticMemoryStore` (`src/memory/semantic_store.rs`); `memory_upsert/search/delete/list` on `RetrievalClient` (`src/retrieval/memory.rs`); `migrate-memories` CLI + tests (`src/main.rs`, `src/migrate/memories.rs`).
- **L-03** ✅ — `src/prompts/builders.rs` no longer calls `index::open_db`.
- **L-04 / L-13** ✅ — memory queries route through `RetrievalClient::search_memories` (`src/retrieval/search.rs:126`, `memories` collection).
- **L-05 / L-06 / L-07** ✅ — `embed/{index,bm25,local,drift}.rs` deleted; `tantivy` dropped from `Cargo.toml`.
- **L-09 / L-10** ✅ — `index` tool re-routed to `sync_project`; `ONBOARDING_VERSION=29`.
- Legacy-db detect hint lives at activation (`src/tools/config/mod.rs:552`).

**Superseded:**
- **L-08** — ✅ **corrected 2026-06-15.** The 2026-06-14 reconciliation conflated two adjacent symbols. `schema::SearchResult` **did** graduate to a live consumer (`apply_file_diversity_cap`, `src/tools/semantic/semantic_search.rs`) — **kept**. But `fusion::rrf_fuse` + `BM25Result` (`src/embed/fusion.rs`) had **zero production callers** — test-only (verified via `references`, not file proximity). `fusion.rs` **deleted 2026-06-15**: `pub mod fusion;` dropped from `embed/mod.rs`, the dead `rrf_fuse_integration_*` test removed from `semantic/tests.rs`, full suite green (2869 passed, clippy `-D warnings` clean). `schema.rs` stays. Recon: R-33.

**The real residual (NEW — never in the original L-list):**
- **L-16** — **librarian artifact vector index still on sqlite-vec.** `src/librarian/catalog/schema.sql:36` defines `CREATE VIRTUAL TABLE artifact_vec USING vec0(...)`; `src/librarian/indexer.rs` embeds artifacts into it (lines 241, 572). A *third* vector index, independent of memory + code. It is the **sole remaining `sqlite-vec` consumer** (`init_sqlite_vec` at `src/librarian/catalog/mod.rs:35`), hence the **blocker for L-11**. Porting it is L-01-scale: new Qdrant collection for artifacts, `RetrievalClient` methods (reuse the `memory_*` pattern), rewire `indexer.rs` + librarian search, one-shot migration of existing `artifact_vec` rows, then delete the vec0 table + `init_sqlite_vec`. **Per the 2026-06-15 decision the `sqlite-vec` dep itself is RETAINED** (daemon-free local-backend seam — see decision log), so L-16 no longer ends in a dep drop; it ends at one-vector-model consistency.

**L-11 (drop `sqlite-vec`) is wontfix as of 2026-06-15** — `sqlite-vec` is retained as the daemon-free local-vector-search backend for low-end / locked-down systems (see decision log). L-16 still proceeds (artifacts → Qdrant for uniformity) but keeps the dep + the `SemanticMemoryStore` trait seam.
## Open items

| ID | Surface | What still uses legacy | What replacing it requires |
|---|---|---|---|
| L-01 | `src/tools/memory/mod.rs` (8 call sites: 276, 277, 278, 318, 319, 322, 325, 336, 348, 707, 716, 790) | `memory(action='remember' \| 'recall' \| 'forget')` writes/reads sqlite-vec via `open_db`, `ensure_vec_memories`, `upsert_memory_by_title`, `ensure_memory_anchors`, `delete_semantic_anchors`, `insert_semantic_anchor`, `search_memories`, `delete_memory`, `get_file_hash` | New Qdrant collection `memories` (per-project), payload schema { bucket, title, content, anchor_path, anchor_hash, created_at, updated_at }. Re-implement upsert/scroll/search/delete on top of `RetrievalClient`. Migration tool that reads the existing sqlite-vec store and bulk-imports. |
| L-02 | ~~`src/memory/anchors.rs` (4 sites)~~ | ✅ DONE 2026-05-07 — moved to `src/memory/hash.rs` with re-export shim at `embed::index::hash_file` for in-file callers (deletes for free with index.rs). Tests moved with the function. | — |
| L-03 | `src/prompts/builders.rs:429-430` | System-prompt generation calls `index::open_db` + `ensure_vec_memories` to inject memory pointers into the per-project draft | Rewire to read from the new Qdrant memories collection (depends on L-01) OR drop memory-pointer injection from the draft and rely on the agent's own `memory(action='list')` call after onboarding. |
| L-04 | `src/tools/semantic/semantic_search.rs:352` | When the user passes `topic="memories"`, the LEGACY else branch in semantic_search calls `index::search_memories(...)`. The narrow Phase 7 deletes the legacy code-CHUNK branch but the memory branch may share the same code path. | Re-route memory queries to `RetrievalClient::search_memories` (already exists in `src/retrieval/search.rs:118`) once L-01 has populated the `memories` collection. |
| L-05 | `src/embed/local.rs` | `LocalEmbedder` (ONNX) — only consumed by legacy code paths; no stack consumer | Will fall away when L-01 lands and the legacy index is removed. No work needed standalone — track as "deletes for free with index.rs". |
| L-06 | `src/embed/drift.rs` | Legacy drift detection over sqlite-vec. The stack has its own drift in `src/retrieval/drift.rs`, which is what `sync_project` uses. | Delete with index.rs after L-01. Confirm no external callers (none found 2026-05-07). |
| L-07 | `src/embed/bm25.rs` (tantivy-backed) | Legacy keyword index. Stack uses Qdrant SPLADE sparse leg instead. Only consumer is `index.rs::search_code` | Delete with index.rs. Drops `tantivy` from Cargo.toml. |
| L-08 | `src/embed/fusion.rs` | Legacy RRF over sqlite-vec dense + tantivy BM25 results. Stack does fusion in Qdrant (`Fusion::Rrf`). Only consumer is `index.rs` | Delete with index.rs. |
| L-09 | `src/tools/onboarding.rs` (Phase 0/1 in `onboarding_prompt.md`) | Onboarding teaches the user to pick an embedding model and `index(action='build')` — the legacy flow | Replace Phase 0/1 with stack quickstart: detect `./scripts/retrieval-stack.sh ps`, prompt to start it, run `sync_project`. The current `onboarding_prompt.md` already adds a dual-backend note (Phase 6.2); the legacy steps below the note can be deleted once L-01 lands. |
| L-10 | `index(action='build' \| 'status')` tool | Builds the legacy sqlite-vec index | Either (a) delete the tool entirely once stack is sole backend, or (b) re-route to `sync_project` semantics so existing agent prompts that call `index(action='build')` still work. Plan-time decision. |
| L-11 | `Cargo.toml` deps (`sqlite-vec`, `tantivy`, transitive `fastembed`) | Direct deps of legacy index | Drop after L-01 + index.rs deletion. Knock-on: re-audit `codescout-embed` crate to ensure it doesn't carry legacy assumptions. |
| L-12 | `src/tools/semantic/semantic_search.rs` (Phase 7 narrow) | `scope='lib:<name>'` returns `RecoverableError` — stack `search_code` ignores library scope; only the active project is searched | Wire `RetrievalClient::search_libraries` into the stack path with the requested library name. Validate the library is registered. Mirror the legacy scope-validation behavior. |
| L-13 | `src/tools/semantic/semantic_search.rs` (Phase 7 narrow) | `include_memories=true` returns `RecoverableError` pointing at `memory(action='recall')` | Resolves automatically once L-01 lands and there is a stack-side memories collection — wire `RetrievalClient::search_memories` into the optional path. Until then the error message is the contract. |
| L-14 | `src/tools/semantic/semantic_search.rs` (Phase 7 narrow) | Stack search has no equivalent of the legacy `check_index_staleness` warning that surfaced in the result envelope | Stack staleness is per-chunk via `sync_project` — agent can call `sync_project` itself. If we want a freshness signal on every search, expose it via a Qdrant payload field (e.g. last-synced timestamp on each point) and surface it when older than a threshold. |
| L-15 | `src/tools/semantic/semantic_search.rs::apply_file_diversity_cap` | Helper kept `#[allow(dead_code)]` — stack search returns chunks ranked by Qdrant + reranker without a per-file diversity cap. Can over-represent one file in top-K | Apply the cap on `Vec<Hit>` after `client.search_code(...)` returns, before formatting. Carry MAX_CHUNKS_PER_FILE = 3 default. |

## Decision log

- **2026-06-15** — **L-08 corrected · L-11 wontfix · L-16 reframed** (user decision). (1) L-08: the 2026-06-14 "fusion/schema graduated" call was half-wrong — `schema::SearchResult` is live (kept); `fusion::rrf_fuse`/`BM25Result` were **test-only** and are now **deleted** (`src/embed/fusion.rs` removed; suite green). Caught by a `references` call-graph scout the proximity-based audit missed (recon **R-33**). (2) **L-11 → wontfix:** `sqlite-vec` is **retained** as the daemon-free local-vector-search backend for low-end / locked-down systems (e.g. the `vdi-windows` worktree — no Qdrant daemon). The dep-drop "binary diet" goal is abandoned. (3) **L-16 reframed:** still port `artifact_vec` → Qdrant, but for one-vector-model *consistency*, not a dep drop; keep `sqlite-vec` + the `SemanticMemoryStore` trait as the local-backend seam.

- **2026-06-14** — Reconciliation audit (see "## 2026-06-14 audit" above): L-01/03/04/05-07/09/10/13 confirmed done in code; L-08 superseded (fusion/schema graduated to live consumers); the real residual is the librarian `artifact_vec` index (new item **L-16**), which blocks the L-11 sqlite-vec drop.

- **2026-05-07** — Phase 7 narrowed to "remove backend knob + legacy code-search else
  branch only" (L-04 partially closed; the memory query path inside that else branch
  must stay until L-01 lands). Tracker opened to capture the rest. User chose
  narrow scope over a months-long memory-port effort.

## Suggested ordering for full removal

1. ~~L-02 — move `hash_file` out of `embed::index`.~~ ✅ DONE 2026-05-07.
2. L-01 — design + implement Qdrant `memories` collection, port `memory.remember/recall/forget`,
   write a one-shot migration script. **Largest single piece. Needs its own design doc.**
3. L-03 + L-04 + L-09 — rewire builders, semantic_search else branch, onboarding flow on
   top of the new memory backend.
4. L-10 — decide `index` tool fate.
5. L-05/L-06/L-07/L-08 + L-11 — delete files + Cargo deps. Mechanical.

## Cross-references
- **L-14 consumer requirement** → `docs/trackers/2026-06-09-index-freshness-signal-for-consumers.md` (id `286ac62b5a821cec`): expose a Qdrant-era freshness signal so an out-of-process consumer (the codescout-companion plugin) can re-enable session-start auto-reindex. `codescout index` now drives `sync_project` and never advances `project.db`'s `meta.last_indexed_commit`, so the old companion read surface is permanently frozen.

- Plan: `docs/superpowers/plans/2026-05-06-retrieval-stack-plan.md` § Phase 7 (incomplete)
- Spec: `docs/superpowers/specs/2026-05-06-retrieval-stack-design.md`
- Empirical record: `docs/research/2026-05-06-retrieval-stack-benchmark.md`
- Phase 6 commit baseline: `master @ 62a0d50`
- L-02 follow-up (S-17): when L-01 deletes `src/embed/index.rs`, also flip
  the two `run_command/tests.rs` callers (lines 2948, 3002) from
  `crate::embed::index::hash_file` to `crate::memory::hash::hash_file`.
  Re-export shim is the only thing keeping them green today.
