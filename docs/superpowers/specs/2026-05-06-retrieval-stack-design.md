# Retrieval Stack Design — Docker-Compose Hybrid Retrieval

**Date:** 2026-05-06
**Status:** draft
**Branch (target):** `retrieval-stack` (new; not `experiments`)
**Supersedes:** none directly — refactors the embedding/retrieval layer introduced by `docs/superpowers/specs/2026-05-02-hybrid-retrieval-design.md`

---

## Overview

Replace the embedded retrieval stack (`sqlite-vec` + `tantivy` + `fastembed` + client-side RRF) with a separate Docker-Compose stack: **Qdrant** (vector DB with native server-side hybrid) + **embedder service** (Ollama on CPU profile, TEI on GPU profile) + **reranker** (TEI serving Qwen3-Reranker on CPU, BGE-Reranker-v2-m3 on GPU). The codescout MCP server becomes a thin retrieval client.

### Motivations

1. **Heavier retrieval engine** — gain native dense+sparse+rerank pipeline with server-side fusion, advanced filters, multi-collection isolation that the embedded stack cannot offer.
2. **Decouple deploy/ops** — upgrade embedding models or rebuild indexes without touching the MCP server.
3. **Share index across clients** — one stack on the host serves multiple codescout instances (`~/.claude` and `~/.claude-sdd` profiles, librarian-mcp). Eliminates per-instance re-embedding.

### Scope

- **Replacement, not coexistence.** Embedded stack is removed in Phase 7.
- **All four corpora** routed through the new stack: code chunks, markdown chunks, project memories, library indexes.
- **Localhost single-user** — no auth, no TLS, no multi-tenancy. Loopback bind only.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Host machine (localhost only)                               │
│                                                             │
│  ┌──────────────────┐                                       │
│  │ codescout MCP    │── HTTP/gRPC ──┐                       │
│  │ (~/.claude)      │               │                       │
│  └──────────────────┘               │                       │
│                                     ▼                       │
│  ┌──────────────────┐    ┌────────────────────────────┐     │
│  │ codescout MCP    │───▶│ Retrieval stack (compose)  │     │
│  │ (~/.claude-sdd)  │    │                            │     │
│  └──────────────────┘    │  ┌──────────────────────┐  │     │
│                          │  │ qdrant               │  │     │
│  ┌──────────────────┐    │  │ :6333 / :6334        │  │     │
│  │ librarian-mcp    │───▶│  └──────────────────────┘  │     │
│  └──────────────────┘    │  ┌──────────────────────┐  │     │
│                          │  │ embedder (Ollama|TEI)│  │     │
│                          │  └──────────────────────┘  │     │
│                          │  ┌──────────────────────┐  │     │
│                          │  │ reranker (TEI)       │  │     │
│                          │  └──────────────────────┘  │     │
│                          │  Volumes:                  │     │
│                          │   qdrant_storage           │     │
│                          │   model_cache              │     │
│                          │   ollama_models            │     │
│                          └────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### Key properties

- All services on private compose network `retrieval_net`. Only Qdrant + embedder + reranker exposed to host, all `127.0.0.1`-bound.
- Two profiles selected at compose time: `--profile cpu` (Ollama + BGE-M3 + Qwen3-Reranker-0.6B) or `--profile gpu` (TEI + Qwen3-Embedding-8B + BGE-Reranker-v2-m3).
- `model_cache` volume shared between embedder and reranker on GPU profile so HuggingFace model files download once.
- Persistent volumes survive `docker compose down`.

### Data ownership shift

| Old | New |
|---|---|
| Per-project `.codescout/embeddings/index.db` | Qdrant collections, keyed by `project_id` payload |
| Per-project `.codescout/tantivy/` | Qdrant sparse vectors |
| `chunks` table SQLite metadata | Qdrant point payload |
| `last_indexed_commit` row | Per-point payload field |

---

## Service Definitions

### `qdrant`

| Field | Value |
|---|---|
| Image | `qdrant/qdrant:v1.13.x` (pin minor; track stable) |
| Ports | `6333` (REST), `6334` (gRPC) — bound to `127.0.0.1` |
| Volume | `qdrant_storage:/qdrant/storage` |
| Env | `QDRANT__SERVICE__API_KEY` (optional, off by default), `QDRANT__LOG_LEVEL=INFO` |
| Healthcheck | `GET /readyz` every 10s |
| Profiles | both |

### `embedder` — CPU profile

| Field | Value |
|---|---|
| Image | `ollama/ollama:latest` |
| Ports | `11434` bound to `127.0.0.1` |
| Volume | `ollama_models:/root/.ollama` |
| Init | sidecar `embedder-pull` runs `ollama pull bge-m3` then exits |
| Healthcheck | `GET /api/tags` |
| Profiles | `cpu` |

**Open question (Phase 0 spike):** confirm Ollama exposes BGE-M3 sparse output. If not, fall back to TEI-BGE-M3 on CPU profile (heavier ~2GB RAM but correct).

### `embedder` — GPU profile

| Field | Value |
|---|---|
| Image | `ghcr.io/huggingface/text-embeddings-inference:1.6` |
| Ports | `8080` bound to `127.0.0.1` |
| Volume | `model_cache:/data` |
| Args | `--model-id Qwen/Qwen3-Embedding-8B --dtype float16` |
| Runtime | `nvidia` |
| `shm_size` | `2g` |
| Profiles | `gpu` |

### `reranker`

| Field | Value |
|---|---|
| Image | `ghcr.io/huggingface/text-embeddings-inference:1.6` |
| Ports | `8081` bound to `127.0.0.1` |
| Volume | `model_cache:/data` |
| Args (cpu) | `--model-id Qwen/Qwen3-Reranker-0.6B --dtype float32` |
| Args (gpu) | `--model-id BAAI/bge-reranker-v2-m3 --dtype float16` |
| Runtime | `nvidia` only on `gpu` profile |
| Profiles | both |

### Volumes

| Name | Purpose |
|---|---|
| `qdrant_storage` | Vector + payload data |
| `model_cache` | HuggingFace model files (TEI) |
| `ollama_models` | Ollama blobs |

---

## Qdrant Collection Schema

### Layout

One collection per corpus type. Shared vector configuration so a single embedder+reranker serves all.

| Collection | Vectors | Source |
|---|---|---|
| `code_chunks` | dense + sparse | `src/embed/ast_chunker` pipeline |
| `markdown_chunks` | dense + sparse | `chunker::split_markdown` |
| `memories` | dense only | memory tool |
| `library_chunks` | dense + sparse | library registration flow |

### Vector configuration (shared)

```jsonc
{
  "vectors": {
    "dense": {
      "size": 1024,             // BGE-M3 = 1024; Qwen3-Embedding-8B = 4096
      "distance": "Cosine",
      "on_disk": false
    }
  },
  "sparse_vectors": {
    "sparse": { "modifier": "idf" }
  },
  "hnsw_config": { "m": 16, "ef_construct": 100 },
  "optimizers_config": { "memmap_threshold": 20000 }
}
```

**Profile dim mismatch:** BGE-M3 (1024) and Qwen3-8B (4096) differ. One stack instance commits to one embed model. Switching profiles = recreate collections + reindex. Acceptable as deployment-time decision.

### Payload schemas

#### `code_chunks`

```jsonc
{
  "project_id": "code-explorer",         // indexed; primary tenant key
  "file_path": "src/tools/symbols.rs",   // indexed
  "language": "rust",                    // indexed
  "start_line": 42,
  "end_line": 87,
  "ast_kind": "fn",                      // indexed
  "ast_header": "fn list_symbols(...)",
  "content": "...",
  "content_hash": "sha256:...",          // indexed; dedup + drift
  "last_indexed_commit": "abc123",
  "chunk_id": "uuid"                     // point id
}
```

Indexed fields: `project_id`, `file_path`, `language`, `ast_kind`, `content_hash`.

#### `markdown_chunks`

```jsonc
{
  "project_id": "...",
  "file_path": "docs/ARCHITECTURE.md",
  "heading_path": ["## Storage", "### Volumes"],
  "heading_anchor": "storage-volumes",   // indexed
  "start_line": 12, "end_line": 48,
  "content_hash": "...",
  "content": "..."
}
```

#### `memories`

```jsonc
{
  "project_id": "...",
  "bucket": "code|system|preferences|unstructured",   // indexed
  "title": "...",
  "private": false,                                    // indexed
  "created_at": 1736208000,                            // indexed
  "content": "..."
}
```

Dense only — short prose, sparse adds little.

#### `library_chunks`

`code_chunks` shape plus:
```jsonc
{
  "library_name": "tokio",        // indexed
  "library_version": "1.40.0",    // indexed
  "library_root": "/path/...",
  // project_id absent — libraries are global
}
```

### Tenancy

- `project_id` payload field on every code/markdown/memory point.
- Default query filter: `project_id = <current>`.
- Library points use `library_name` instead, queryable across projects.

### Hybrid query shape

```jsonc
POST /collections/code_chunks/points/query
{
  "prefetch": [
    { "query": <dense vec>,  "using": "dense",  "limit": 60 },
    { "query": <sparse vec>, "using": "sparse", "limit": 60 }
  ],
  "query": { "fusion": "rrf" },
  "filter": { "must": [ { "key": "project_id", "match": { "value": "code-explorer" } } ] },
  "limit": 20,
  "with_payload": true
}
```

Top-20 RRF-fused result feeds the reranker, which returns final top-K (K=8 or K=10).

### Drift detection

Per-file hash check stays. Client computes `sha256` of each chunk; on `sync_project`, query Qdrant for existing `content_hash` set per `file_path`, skip identical hashes, delete obsolete points by `chunk_id`, upsert new ones. No SQLite meta table — Qdrant payload is source of truth.

---

## Codescout Client Surface

### What dies

| Module | Approx LoC | Replacement |
|---|---|---|
| `src/embed/index.rs` | ~1500 | `src/retrieval/sync.rs` (thin) |
| `src/embed/bm25.rs` | ~400 | gone (Qdrant sparse) |
| `src/embed/fusion.rs` | ~150 | gone (server-side RRF) |
| `src/embed/preflight.rs` | ~390 | trimmed → `src/retrieval/preflight.rs` |
| `src/embed/drift.rs` | ~360 | `src/retrieval/drift.rs` |
| `src/embed/schema.rs` | ~50 | kept; types align with payloads |
| `src/embed/ast_chunker.rs` | ~2100 | **kept** |
| `src/embed/chunker.rs` | ~270 | **kept** |
| `crates/codescout-embed` `Embedder` trait | — | deleted (one HTTP client replaces it) |
| `tantivy` dep | — | removed |
| `sqlite-vec` dep | — | removed |
| `fastembed` dep | — | removed |

### What's added

```
src/retrieval/
  mod.rs           // pub use; module wiring
  client.rs        // RetrievalClient: Qdrant + embedder + reranker handles
  config.rs        // RetrievalConfig: URL endpoints, profile, model dim
  sync.rs          // sync_project: AST chunk → embed → upsert with drift
  search.rs        // search_code, search_markdown, search_memories, search_libraries
  drift.rs         // hash compare against Qdrant payload
  preflight.rs     // path scope checks
```

Single new dep: `qdrant-client = "1.13"` (Rust SDK, gRPC).

### `RetrievalClient` API

```rust
pub struct RetrievalClient {
    qdrant: QdrantClient,
    embedder: EmbedderHttp,
    reranker: RerankerHttp,
    config:   RetrievalConfig,
}

impl RetrievalClient {
    pub async fn from_env() -> Result<Self>;
    pub async fn health(&self) -> HealthReport;

    pub async fn sync_project(&self, project_id: &str, root: &Path, opts: SyncOpts) -> SyncReport;
    pub async fn sync_library(&self, library: &LibrarySpec) -> SyncReport;

    pub async fn search_code(&self, project_id: &str, q: &str, opts: SearchOpts) -> Vec<Hit>;
    pub async fn search_markdown(&self, project_id: &str, q: &str, opts: SearchOpts) -> Vec<Hit>;
    pub async fn search_memories(&self, project_id: &str, q: &str, opts: SearchOpts) -> Vec<Hit>;
    pub async fn search_libraries(&self, q: &str, opts: SearchOpts) -> Vec<Hit>;
}

pub struct SearchOpts {
    pub limit: usize,            // top-K final
    pub overfetch: usize,        // candidates → reranker (default 20, hard cap)
    pub rerank: bool,            // default true
    pub filter: Option<Filter>,
}
```

### Search flow

```
search_code(project_id, query, opts)
  → embedder.embed_dense_and_sparse(query)
  → qdrant.query_points(prefetch=[dense, sparse], fusion=RRF,
                        limit=overfetch, filter=project_id+opts.filter)
  → reranker.rerank(query, top-20 contents) → scores
  → sort by rerank_score, take(limit)
  → Vec<Hit>
```

Reranker hard-capped at 20 candidates per Qwen3-Reranker-0.6B CPU latency budget (~2.6s for 10 docs).

### Sync flow

```
sync_project(project_id, root)
  → walk repo (existing preflight scope rules)
  → for each file:
      ast_chunker::split_file → chunks
      sha256(chunk.content) → content_hash
  → qdrant.scroll(filter=project_id+file_path, with_payload=[content_hash, chunk_id])
  → diff:
      to_upsert = chunks where new_hash ∉ server_hashes
      to_delete = server points where chunk_id ∉ new_hashes
  → embedder.embed_batch(to_upsert.contents)
  → qdrant.upsert_points(to_upsert)
  → qdrant.delete_points(to_delete)
  → SyncReport { added, updated, deleted, elapsed }
```

Idempotent. Re-running on unchanged repo is one scroll, no embed calls.

### Tool surface (MCP layer)

`semantic_search` keeps its name and arguments. Body changes from "open SQLite + run RRF" to "call `RetrievalClient::search_code`". Same for librarian-mcp `find` → `search_markdown`, memory `recall` → `search_memories`. No tool rename, so the three prompt surfaces (`server_instructions.md`, `onboarding_prompt.md`, `builders.rs`) need only minor error-message updates.

### Configuration

`.codescout/retrieval.toml` per project (or env vars):

```toml
[retrieval]
url        = "http://localhost:6333"
embedder   = "http://localhost:11434"
reranker   = "http://localhost:8081"
profile    = "cpu"
model_dim  = 1024
```

Env precedence: `CODESCOUT_QDRANT_URL`, `CODESCOUT_EMBEDDER_URL`, `CODESCOUT_RERANKER_URL`. If unset, server starts in degraded mode and surfaces missing-dependency error in tool messages.

### Errors

| Failure | Behavior |
|---|---|
| Qdrant unreachable | `RecoverableError("retrieval stack offline; run `docker compose --profile cpu up -d`")` |
| Embedder unreachable | Same shape; suggest model pull command |
| Reranker unreachable | Degraded path: skip rerank, return RRF top-K. Log warning. |
| Dim mismatch | Fatal `anyhow::bail!` with explicit recreate-or-switch-profile guidance |

---

## Migration & Rollout

### Branch strategy

All work on new branch `retrieval-stack`. Cherry-pick to `master` only after Phase 5 benchmark gate clears.

### Phases

#### Phase 0 — Spike (1–2 days)

- Bring up `qdrant` + Ollama-`bge-m3` + TEI-`Qwen3-Reranker-0.6B` via throwaway compose
- Verify Ollama exposes BGE-M3 sparse output; if not, switch CPU embedder to TEI-BGE-M3
- Run 100-chunk synthetic ingest, verify hybrid query
- Measure cold-start latency, idle RAM, single-query latency
- Artifact: `docs/spikes/<date>-retrieval-stack-spike.md`

#### Phase 1 — Compose stack (1 day)

- `docker-compose.yml` at repo root with `cpu` and `gpu` profiles
- `.env.example` with all env vars
- `scripts/retrieval-stack.sh` wrapper: `up`, `down`, `logs`, `pull`
- README section
- CI does NOT run the stack

#### Phase 2 — Client layer (3–4 days)

- `src/retrieval/` skeleton + `RetrievalClient::from_env`
- `qdrant-client` crate added; gRPC over `127.0.0.1:6334`
- HTTP wrappers for embedder + reranker
- Unit + mocked integration tests
- No production wiring yet

#### Phase 3 — Sync pipeline (2–3 days)

- Port `ast_chunker` consumer → `sync_project`
- Drift via Qdrant scroll
- Library sync variant
- E2E test via testcontainers-rs: ingest fixture project, query, assert hits
- Side-by-side top-10 overlap check vs legacy

#### Phase 4 — Search wiring (2 days)

- Implement four search methods
- Plumb through `semantic_search` behind feature flag `CODESCOUT_RETRIEVAL_BACKEND={legacy,stack}`
- Default `legacy`
- Both backends pass same test suite

#### Phase 5 — Benchmark gate (1 day)

- Run 20-TC suite (`docs/research/2026-04-03-embedding-model-benchmark.md`) against both backends
- **Ship gate (hard):**
  - Aggregate score (new) ≥ aggregate score (legacy)
  - TC-10 + TC-19 + TC-20 do not regress (currently 0 — any non-zero is a win)
  - p95 latency (new) ≤ 2× p95 (legacy)
- If regression: pivot to Option B (Qdrant BM25 sparse instead of BGE-M3 sparse). Cost: ~1 day.
- Result: `docs/research/<date>-retrieval-stack-benchmark.md`

#### Phase 6 — Cutover (1 day)

- Flip `CODESCOUT_RETRIEVAL_BACKEND=stack` default
- Update `server_instructions.md`, `onboarding_prompt.md`, `builders.rs`
- Bump `ONBOARDING_VERSION`
- Update `CONTRIBUTING.md` + manual: stack required for development
- Deprecation note: legacy backend removed in next minor

#### Phase 7 — Delete legacy (1 day, separate commit)

- Remove `src/embed/{index,bm25,fusion}.rs`
- Drop `tantivy`, `sqlite-vec`, `fastembed` deps
- Shrink `crates/codescout-embed` to chunker + types
- Remove backend feature flag
- librarian-mcp gets same treatment in follow-up branch (own design doc)

### Timeline

~10–12 working days. Phases 0–4 reversible. Phase 6 is the irreversible point and gated on Phase 5 pass.

### Compatibility

- **Existing users on `master`:** stack additive. After pulling, run `docker compose --profile cpu up -d`; first sync rebuilds the index.
- **Old `.codescout/embeddings/*`:** orphaned. Cleanup script `scripts/retrieval-stack.sh purge-legacy` removes them.
- **CI:** unchanged. Tests use testcontainers when `RETRIEVAL_E2E=1`; default `cargo test` skips them.

### Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| BGE-M3 sparse worse than Tantivy on TC-10/19/20 | Medium | Phase 5 gate; pivot to Option B (Qdrant BM25 sparse) |
| Qwen3-Reranker CPU too slow for interactive | Medium | overfetch hard cap=20; degrade gracefully on reranker failure |
| Ollama doesn't expose BGE-M3 sparse | High | Phase 0 spike; fallback = TEI-BGE-M3 on CPU |
| Users without docker | Low | Stack required; no embedded fallback after Phase 7. Documented. |
| `qdrant-client` crate API churn | Low | Pin exact version; review quarterly |
| Disk usage growth in `qdrant_storage` | Low | `purge-legacy` CLI; document `du -sh` check |

### Out of scope

- Multi-user / network-shared stack
- Auth / TLS
- Cross-project search UI
- librarian-mcp migration (follow-up branch, own design)
- Migration tool reading sqlite-vec → Qdrant (cheaper to re-embed)

---

## Testing Strategy

### Pyramid

```
                 ┌──────────────────┐
                 │  Benchmark gate  │  Phase 5 ship gate; manual
                 └──────────────────┘
              ┌──────────────────────────┐
              │  E2E (testcontainers)    │  RETRIEVAL_E2E=1; nightly
              └──────────────────────────┘
        ┌──────────────────────────────────────┐
        │  Integration (mocked)                │  Always on
        └──────────────────────────────────────┘
   ┌─────────────────────────────────────────────────┐
   │  Unit                                           │  Always on
   └─────────────────────────────────────────────────┘
```

### Unit (always on)

- `ast_chunker` — already covered, untouched
- `drift::diff_chunks` — pure function: identical, file added, file deleted, file modified, partial overlap
- `config::from_env` — env precedence, TOML overrides, missing-required errors
- Payload codec round-trip (property test)
- Filter builders — correct Qdrant `Filter` shape

### Integration (mocked, always on)

`mockito` for HTTP, mock Qdrant client.

- `search_code` happy path
- Reranker degrade — 503 → RRF top-K returned, warning logged
- Qdrant offline — `RecoverableError` with documented message
- Embedder dim mismatch — fatal error with guidance
- Filter pass-through

### E2E (testcontainers-rs, opt-in via `RETRIEVAL_E2E=1`)

- Sync + query roundtrip on `tests/fixtures/rust-library`
- Idempotent sync — second run reports `added=0 updated=0 deleted=0`
- Drift on file change — sync, mutate, sync, assert only changed file's chunks updated
- Drift on file delete — sync, delete, sync, assert chunks gone
- Multi-corpus isolation — same query distinct hits across collections
- Multi-tenant isolation — `project_id` filter scopes results
- Reranker actually reorders — query where dense top-1 wrong but reranker promotes correct hit

### Three-query staleness sandwich

Project-pattern regression test (per `CLAUDE.md`):

1. Sync, query for known identifier, record top-5
2. Mutate file on disk **without sync**
3. Query again — assert top-5 unchanged (proves not eagerly re-reading)
4. Call `sync_project`
5. Query again — assert top-5 reflects the change

### Benchmark gate (manual, Phase 5)

- 20-TC suite against legacy and stack backends
- Same fixture corpus, captured: aggregate score, per-TC, p50/p95 latency
- Ship gates listed in Phase 5

### Test data

- Existing `tests/fixtures/*-library/` reused
- New: `tests/fixtures/retrieval-corpus/` — hand-crafted code exercising sparse vs dense (~20 files)

### Not tested

- Docker compose itself
- TEI / Ollama / Qdrant internals
- GPU profile in CI (no runners; manual verify)
- Cross-machine networking

---

## Open Questions Resolved at Spike

1. Does Ollama expose BGE-M3 sparse output? If no, swap CPU embedder for TEI-BGE-M3.
2. Cold-start latency, idle RAM, single-query latency end-to-end on developer laptop.
3. Does Qdrant `idf` modifier accept BGE-M3's sparse output natively without client-side normalization?

---

## References

- `docs/trackers/lancedb-upgrade-2026-05.md` — prior eval, deferred
- `docs/superpowers/specs/2026-05-02-hybrid-retrieval-design.md` — current embedded hybrid design
- `docs/research/2026-04-03-embedding-model-benchmark.md` — 20-TC benchmark suite (reused as ship gate)
- Qdrant Universal Query API: https://qdrant.tech/documentation/concepts/hybrid-queries/
- HuggingFace text-embeddings-inference: https://github.com/huggingface/text-embeddings-inference
- BGE-M3 model card: https://huggingface.co/BAAI/bge-m3
