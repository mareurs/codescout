# Plan — Two-Stack Retrieval: Server (Qdrant/hybrid) + Lite (daemon-free, sqlite-vec)

**Status:** draft · **Opened:** 2026-06-16 · **Owner:** marius
**Tracker:** WIN-26 (`docs/trackers/windows-platform-support.md`); extends WIN-22
**Phase 0 shipped:** `825c0c52` (dense always OpenAI-compatible; drop `DenseProtocol::Tei`; dense-only memory-leak fix; delete benchmark matrix scaffolding)

## Problem

A locked-down corporate **VDI** (CrowdStrike EDR, 4 vCPU) **cannot run Docker or
Qdrant**, and must use **remote OpenAI-compatible embeddings only**. Today:

- Code `semantic_search` is **hard-wired to Qdrant** (`RetrievalClient` →
  `QdrantWrap`, no store abstraction). The full stack needs 4 services: Qdrant +
  dense + sparse + reranker.
- Memory + librarian default to Qdrant too (librarian already has a sqlite-vec
  escape hatch; memory does not).
- The legacy in-process sqlite-vec code path was **removed** 2026-05-07
  (`semantic_search.rs` — "the legacy sqlite-vec + tantivy path is gone").

So **WIN-22's remote-dense fix is necessary but not sufficient**: with no Qdrant,
code search can't run at all. The VDI needs a daemon-free retrieval path.

## Decision (ADR)

Support **two stacks**, selected by a single backend switch:

| | **Stack A — "server"** | **Stack B — "lite"** |
|---|---|---|
| Target | powerful / GPU, Docker OK | VDI / air-gapped: no Docker, no Qdrant |
| Vector store | Qdrant (daemon) | **in-process sqlite-vec** (statically-linked `vec0`) |
| Embeddings | local llama.cpp (OpenAI-compatible) | **remote OpenAI-compatible only** |
| Retrieval | dense + sparse + rerank (hybrid RRF) | **dense KNN only** |
| Footprint | 4 containers | **single binary + 1 remote endpoint** |

Reranker and sparse are **Stack-A-only**. The reranker is never redundant with the
vector store — it's a cross-encoder re-scoring layer Qdrant structurally can't do —
Stack B simply forgoes it (same shape memory recall + librarian sqlite-vec already use).

## Why this is assembly, not invention

The daemon-free pattern already **ships** — for librarian artifacts:

- `ArtifactVectorStore` trait (`upsert`/`delete`/`knn`, dense-only) +
  `ArtifactBackend { Qdrant, SqliteVec }` + `ArtifactBackend::resolve()`
  (`src/librarian/artifact_store.rs`).
- `SqliteVecArtifactStore` is "the daemon-free escape hatch"; the two backends are
  tested to give **identical** semantic results (`src/librarian/catalog/find.rs`).
- **`vec0` is statically linked** (`src/librarian/catalog/migrate_v6.rs`;
  `catalog/mod.rs::init_sqlite_vec` registers `sqlite_vec::sqlite3_vec_init` as a
  Once-guarded auto-extension). **No runtime DLL → no CrowdStrike quarantine** —
  unlike the WIN-22 `onnxruntime.dll`. It survives inside `codescout.exe` like the
  rest of the binary.
- `SemanticMemoryStore` trait (`src/memory/semantic_store.rs`) already abstracts the
  memory store (Qdrant impl + an `InMemory` brute-force-cosine test impl).
- `EmbedderHttp::dense_query` (added in Phase 0) is the dense-only OpenAI path.

The **only** consumer with no store abstraction is **code search** — that's the gap
Phase 1 closes.

## EDR safety

`vec0` static linking is the load-bearing property: no foreign DLL, no
`LoadLibrary`, no `PAGE_EXECUTE_READWRITE` codegen → nothing for the behavioral
engine to quarantine. If `vec0` ever causes trouble, the pure-Rust brute-force
cosine (`InMemory*Store`) is a zero-dependency fallback (fine at single-project
scale: ~100k chunks × 768-d brute-force KNN is sub-100ms on the VDI's CPU).

## Phases

**Phase 0 — DONE (`825c0c52`).** Dense always OpenAI-compatible; removed
`DenseProtocol::Tei` + `CODESCOUT_EMBEDDER_PROTOCOL`; fixed `HttpDenseEmbedder::embed`
to skip the wasted sparse call (`dense_query`); deleted `docker-compose.matrix.yml` +
`scripts/chunk-model-matrix.py`.

**Phase 1 — Code-search store trait.** Extract a `CodeVectorStore` trait (mirror
`ArtifactVectorStore`) from `RetrievalClient`; wrap today's Qdrant path as the
`Qdrant` impl. Route `semantic_search` + `sync_project`/`index` through the trait.
Pure seam insertion, no behavior change. Lock it in with a Qdrant-vs-trait parity test.

**Phase 2 — sqlite-vec impls for code + memory.** Add `SqliteVec` impls for the code
store and a production `SqliteVecSemanticMemoryStore`, reusing the librarian
`vec0`/catalog plumbing. Unify backend selection into one
`CODESCOUT_VECTOR_BACKEND=qdrant|sqlite-vec` (default `qdrant`) consulted by all three
consumers (fold in the librarian's existing `CODESCOUT_ARTIFACT_BACKEND`).

**Phase 3 — Lite wiring + build.** `sqlite-vec` backend ⟹ dense-only (skip sparse +
reranker, never connect Qdrant), remote OpenAI dense. Ship `.env.lite` and a VDI
install section extending `docs/manual/src/configuration/embeddings-edr-windows.md`.
Verify with `cargo check --target x86_64-pc-windows-gnu`.

**Phase 4 — Feature-gate the server stack; default lean (tracker task 6).** Put Stack A
(qdrant-client + sparse + reranker, incl. AMD/Infinity) behind a `server-stack` cargo
feature; default the build to the lite stack. The real dep-weight lever is
**`qdrant-client` (gRPC/tonic/prost)** — gating it shrinks the default binary. NOTE:
AMD support alone has **no** Rust deps to gate (HTTP JSON + docker images only).

## Quality tradeoff

Dense-only drops the SPLADE exact-token leg and the cross-encoder rerank; the loss is
worst for exact identifier matches — what code search leans on. Mitigate with a strong
remote code-embedding model (CodeRankEmbed-class) on the endpoint. The existing
benchmark harness (`scripts/sweep-bm25-*.sh`, `scripts/run-tc-benchmark.*`,
`scripts/tc-suites/`) + `CODESCOUT_DISABLE_SPARSE` can quantify the delta before
committing the VDI to lite. (That harness is a **separate** cluster from the matrix
scaffolding removed in Phase 0 — leave it until a benchmark of the lite stack is run.)

## Open questions

- One unified `CODESCOUT_VECTOR_BACKEND` vs per-consumer overrides? Lean unified.
- sqlite-vec ANN vs brute-force cosine for code-scale chunks? sqlite-vec is persistent
  and already wired; brute-force is simplest and zero-dep. Decide in Phase 2.
- Should `dense_query` apply the code-search query prefix for *memory* recall? Phase 0
  preserved existing behavior (prefix applied); revisit if memory recall quality dips.

## Risks

- **Empty sparse on Qdrant.** Lite-mode points have no sparse vector; `upsert_points`
  must omit the `sparse` named vector when empty (the hybrid collection schema accepts
  dense-only points — `ensure_collection` needs no change).
- **Dim mismatch on backend switch.** The vector index is dimension-specific; switching
  embedders or backends requires a reindex (same caveat as WIN-22).
