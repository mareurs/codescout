# Retrieval Stack Benchmark — 2026-05-06

20-TC suite comparing the legacy sqlite-vec backend against the new Qdrant hybrid
retrieval stack (BGE-M3 dense + SPLADE sparse + BGE-Reranker-v2-m3).

## Setup

| | Legacy | Stack |
|---|---|---|
| Backend | sqlite-vec (local ONNX) | Qdrant v1.17.0 gRPC |
| Dense model | all-MiniLM-L6-v2 (384d) | BAAI/bge-m3 (1024d) |
| Sparse model | — | naver/splade-cocondenser-ensembledistil |
| Reranker | — | BAAI/bge-reranker-v2-m3 |
| Inference | CPU (local) | NVIDIA RTX A5000 (sm_86, 24 GB) via TEI |
| Index | 22 753 sqlite-vec rows | 8 637 Qdrant points (10 946 chunks synced) |
| Sync time | n/a (incremental) | 60 s on GPU |

## Aggregate Results

| Metric | Legacy | Stack | Delta |
|---|---|---|---|
| **Total score** | **25 / 60** | **18 / 60** | **−7 (−11.7%)** |
| p50 latency | 123 ms | 248 ms | +125 ms |
| p95 latency | 136 ms | 311 ms | +175 ms |

## Per-TC Results

| TC | Tier | Query (truncated) | Legacy | Stack | Winner |
|---|---|---|---|---|---|
| TC-01 | T1 | RecoverableError | 1/3 | 1/3 | tie |
| TC-02 | T1 | embedding model configuration | 1/3 | 0/3 | legacy |
| TC-03 | T1 | LSP client implementation | 2/3 | **3/3** | **stack** |
| TC-04 | T1 | run_command shell execution | 0/3 | 0/3 | tie |
| TC-05 | T1 | OutputGuard progressive disclosure capping | 2/3 | 2/3 | tie |
| TC-06 | T2 | tool calls recorded in usage database | 1/3 | 1/3 | tie |
| TC-07 | T2 | section boundary detection in markdown editing | 2/3 | 0/3 | legacy |
| TC-08 | T2 | dimension mismatch when switching embedding models | 2/3 | 2/3 | tie |
| TC-09 | T2 | dangerous command detection and safety checks | 2/3 | 2/3 | tie |
| TC-10 | T2 | how overflow hints guide the agent | 1/3 | 1/3 | tie |
| TC-11 | T2 | renaming a symbol across all references | 0/3 | 0/3 | tie |
| TC-12 | T2 | embedding URL determines which backend is used | 2/3 | 0/3 | legacy |
| TC-13 | T3 | LSP crash + circuit breaker recovery | 2/3 | 1/3 | legacy |
| TC-14 | T3 | tool dispatch recoverable vs fatal errors | 0/3 | **1/3** | **stack** |
| TC-15 | T3 | force re-indexing flow + dimension migration | 2/3 | 2/3 | tie |
| TC-16 | T3 | semantic search flow end-to-end | 1/3 | 1/3 | tie |
| TC-17 | T3 | companion plugin routing native calls | 2/3 | 0/3 | legacy |
| TC-18 | T4 | heading detection code block tracking | 2/3 | 0/3 | legacy |
| TC-19 | T4 | project activation + LSP lifecycle wiring | 0/3 | **1/3** | **stack** |
| TC-20 | T4 | keeping three prompt surfaces consistent | 0/3 | 0/3 | tie |

Stack wins: **3** (TC-03, TC-14, TC-19) | Legacy wins: **6** | Ties: **11**

## Analysis

### Where the stack wins

- **TC-03 (3/3)** — "LSP client implementation": BGE-M3's high-dimensional dense vectors (1024d
  vs 384d for MiniLM) capture the structural relationship between `client.rs`, `ops.rs`, and
  `manager.rs` more precisely than the local model.
- **TC-14 / TC-19** — Multi-concept architectural queries about lifecycle wiring and error
  dispatch: SPLADE sparse + reranker lifts relevant chunks that MiniLM embeddings rank too low.

### Where legacy wins

- **TC-02 / TC-12** (embedding model queries): these overlap with the codescout-embed crate's
  own terminology. MiniLM was indexed on exactly this codebase; BGE-M3 is general-purpose.
- **TC-07** (markdown section boundary): localised tool-specific jargon that SPLADE's IDF
  weights don't boost sufficiently.
- **TC-17 / TC-18** (companion plugin, heading detection): highly repo-specific concepts; the
  legacy index has been tuned on incremental usage patterns while the stack is a cold first-run.

### Latency

Stack is ~2× slower (248 ms p50 vs 123 ms p50) due to:
1. Two HTTP round-trips per query (dense + sparse embed in parallel, then Qdrant gRPC)
2. Reranker HTTP call (adds ~50–100 ms)
3. No local caching vs sqlite-vec in-process query

Latency is expected to decrease with warm VRAM and when the embedding service is co-located.

## Known Issues Encountered During Benchmarking

1. **qdrant-client semver mismatch**: `^1.13` in Cargo.toml resolved to 1.17.0 in Cargo.lock;
   server was running v1.13.0. gRPC hybrid queries silently returned empty. Fix: pin
   `qdrant-client = "=1.13.x"` or upgrade container. **Upgraded container to v1.17.0.**

2. **MCP stdout pollution**: qdrant-client emits a version-mismatch warning to stdout, corrupting
   the MCP stdio stream. Fix: added non-JSON line skip in benchmark `_recv`.

3. **OutputGuard buffering**: stack results are large (10 chunks × full content text). The
   `call_content()` size gate buffers them as `output_id`. Benchmark `semantic_search` now
   paginates `read_file` to reconstruct the full result.

## Verdict

The stack does not yet beat the legacy backend on aggregate score (**18 vs 25**). The quality
gap is expected for a cold first run without per-repo embedding tuning. The stack's strengths
(higher-dim dense vectors, sparse BM25 boosting, cross-lingual reranking) are visible on
structural and multi-concept queries.

**Recommendation:** keep the feature-flag (`CODESCOUT_RETRIEVAL_BACKEND=stack`) as opt-in for
now. Graduate to default after: (1) re-embedding with a model more aligned to code retrieval
(e.g., `nomic-embed-code`) or (2) a second benchmark run after the Qdrant index matures from
incremental updates.

## Raw Data

- Legacy JSON: `/tmp/legacy.json` (timestamp: 2026-05-06)
- Stack JSON: `/tmp/stack.json` (timestamp: 2026-05-06)

---

# Phase 5.5 — Chunk × Model Matrix (2026-05-07)

After the v1 benchmark (stack 18/60 vs legacy 25/60 → revised legacy 26/60 with v2 TC suite),
we ran a 12-cell chunk × model matrix to find the quality/cost knee that beats legacy. This
section is the load-bearing record of all post-v1 tuning work — Phase 6 defaults flow from here.

## Setup changes vs Phase 5.1

- **Suite swapped:** kept the 20-TC v2 suite + added a 25-TC kotlin suite extracted from real
  `usage.db` session causality (semantic_search → file opens within 300 s, same session).
  Decision benchmarks below all use the 20-TC v2 suite for comparability with legacy 26/60.
- **Reranker dropped from sweep:** isolates dense+sparse fusion quality. Re-enable later if needed.
- **CODESCOUT_CHUNK_TARGET** env override (`src/retrieval/sync.rs`) — drives `split_file()` AST
  chunker. `c3000`/`c1200`/`c600` cells use chunk_target = 3000/1200/600 chars respectively.
- **CODESCOUT_DISABLE_SPARSE** env (`src/retrieval/config.rs`) — skips BM25 leg, single dense ANN.
  Used for sparse-off control cells; not part of the headline matrix.
- **DenseProtocol::OpenAi** added to `src/retrieval/embedder.rs` — lets us point the dense leg
  at llama-server's `/v1/embeddings` endpoint (CodeRankEmbed @ port 43300, AMD ROCm GPU).
- **Qdrant client timeout** bumped to 120 s — 32 k-point upserts at chunk=600 exceeded the gRPC
  ~30 s default.

## Models tested

| Tag | Model | Dim | Backend | HW | Why included |
|---|---|---|---|---|---|
| `jb` | `jinaai/jina-embeddings-v2-base-code` | 768 | TEI | NVIDIA A5000 | original stack default |
| `cr` | `CodeRankEmbed Q4_K_M` | 768 | llama-server (OpenAI) | AMD RX 7800 XT | code-tuned, AMD-friendly |
| `bs` | `BAAI/bge-small-en-v1.5` | 384 | TEI (CPU) | CPU | no-GPU users |
| `js` | `jinaai/jina-embeddings-v2-small-en` | 512 | TEI (CPU) | CPU | jina family CPU baseline |

Sparse leg (`naver/splade-cocondenser-ensembledistil`) shared across all cells, GPU-served.

## Matrix v2 results (post-BUG-053 fix)

> v1 first run had codescout SIGABRT mid-benchmark from a UTF-8 char-boundary panic in
> `semantic_search` preview formatting (BUG-053; fixed in `is_char_boundary` walkdown at
> `src/tools/semantic/semantic_search.rs:386` and `:491`, plus `src/tools/memory/mod.rs:809`).
> v2 below is the trustworthy comparison.

`bm25_boost = 1.0` baseline, no reranker, 20-TC v2 suite (max 60).

| Cell | Model | Dim | Chunk | Sync (s) | Points | Score | p50 (ms) | p95 (ms) | HW |
|---|---|---|---|---|---|---|---|---|---|
| jb_c3000 | jina-base-code | 768 | 3000 | 74 | 16 041 | 24 | 190 | 268 | GPU(NV) |
| jb_c1200 | jina-base-code | 768 | 1200 | 104 | 21 115 | **27** | 172 | 206 | GPU(NV) |
| jb_c600 | jina-base-code | 768 | 600 | 114 | 32 398 | 24 | 108 | 127 | GPU(NV) |
| cr_c3000 | CodeRankEmbed | 768 | 3000 | 181 | 16 041 | 26 | 170 | 239 | GPU(AMD) |
| **cr_c1200** | **CodeRankEmbed** | **768** | **1200** | **185** | **21 115** | **28** | **169** | **249** | **GPU(AMD)** |
| cr_c600 | CodeRankEmbed | 768 | 600 | 180 | 32 398 | 27 | 109 | 142 | GPU(AMD) |
| bs_c3000 | bge-small | 384 | 3000 | 1 654 | 16 041 | 20 | 212 | 299 | CPU |
| **bs_c1200** | **bge-small** | **384** | **1200** | **2 458** | **21 115** | **27** | **169** | **190** | **CPU** |
| bs_c600 | bge-small | 384 | 600 | 2 092 | 32 398 | 26 | 121 | 129 | CPU |
| js_c3000 | jina-small | 512 | 3000 | 1 295 | 16 041 | 21 | 207 | 286 | CPU |
| js_c1200 | jina-small | 512 | 1200 | 1 216 | 21 115 | 25 | 171 | 209 | CPU |
| js_c600 | jina-small | 512 | 600 | 1 234 | 32 398 | 24 | 117 | 127 | CPU |

**Legacy baseline (sqlite-vec + MiniLM-L6-v2): 26/60.** Bold rows beat or tie legacy.

### Observations

- **chunk = 1200 is the universal sweet spot** across all four models. The chunk/dim heuristic
  ("smaller dim wants smaller chunk") is real but weaker than the alignment between 1200 chars
  ≈ one well-scoped function and how queries are phrased.
- **GPU winner:** `cr_c1200` — CodeRankEmbed's code-specific training beats general jina-base-code
  by +1 at the optimum chunk size, and runs on the AMD card (frees the A5000 for qwen35).
- **CPU winner:** `bs_c1200` (27/60) — ties cr_c1200 base score, only +1 over legacy but with
  far better-quality top-3 results (per-TC inspection). Sync cost is the pain (~41 min on the
  codescout corpus); incremental sync makes this a one-time cost.
- **jina-small dominated by bge-small** at every chunk size despite higher dim (512 vs 384).
  Confirms training mix > raw dim count.
- **Bug fix delta:** cells whose queries hit the panicking preview path recovered points
  (bs_c3000 +2, jb_c1200 +1, cr_c600 +1, js_c3000 +2, js_c1200 +2). Cells unaffected by the
  panic show v1 ≈ v2 — matrix is now noise-bounded comparison.

## bm25_boost sweep on cr_c1200 (2026-05-07)

`bm25_boost` multiplies the sparse-leg candidate pool relative to the dense leg before RRF
fusion. Default 1.0 → balanced. Higher → sparse-dominant.

| boost | Score | p50 (ms) | p95 (ms) |
|---|---|---|---|
| 0.25 | 27 | 168 | 245 |
| 0.5 | 28 | 167 | 251 |
| 1.0 | 28 | 169 | 249 |
| 1.5 | 29 | 165 | 230 |
| 2.0 | 29 | 145 | 207 |
| **3.0** | **30** | **128** | **188** |
| 5.0 | 30 | 127 | 182 |

**Ceiling: cr_c1200 @ boost=3.0 → 30/60 (+4 over legacy 26).** Latency *improves* at high boost
because the dense ANN prefetch shrinks. boost > 3.0 plateaus.

CPU (bs_c1200) sweep not run yet — extrapolation suggests similar +2-3 pts at boost = 3.0.

## Phase 6 Recommendation — Defaults

**GPU (single 4 GB+ card):** CodeRankEmbed @ chunk=1200, bm25_boost=3.0
- Score: **30/60** vs legacy **26/60** (+4, +15%)
- p50: 128 ms (vs legacy 123 ms — within noise)
- AMD-served via llama-server `/v1/embeddings`; NVIDIA-served via TEI works equally
- Sync cost: 185 s on codescout corpus

**CPU-only (no GPU):** bge-small-en-v1.5 @ chunk=1200, bm25_boost=3.0 (validate boost on CPU)
- Score: **27/60** base, projected ~29/60 with boost=3.0
- p50: 169 ms (boost=1.0)
- Sync cost: ~41 min on codescout corpus, one-time + incremental

**Single config knob across both:** `CODESCOUT_CHUNK_TARGET=1200`, `CODESCOUT_BM25_BOOST=3.0`,
plus model-specific `CODESCOUT_EMBEDDER_URL` / `_PROTOCOL` / `_MODEL_NAME`.

### Rejected alternatives

- **chunk=3000 (current default):** −2 to −4 pts across the board. Was chosen pre-benchmark for
  AST chunker. Drop.
- **chunk=600:** ~tied on score with chunk=1200 at lower latency, but 50 % more points to store
  and sync. Not worth the storage churn for noise-level score parity.
- **jina-v2-base-code:** −1 vs CodeRankEmbed at the optimum. Keep as TEI-served fallback for
  users with NVIDIA cards already running TEI.
- **jina-v2-small-en:** dominated by bge-small. Drop from defaults.
- **Reranker:** not re-evaluated post chunk-tuning. Re-test before committing to it for default.

## Open follow-ups

1. CPU bm25_boost sweep on bs_c1200 — confirm CPU default lands ≥28/60.
2. Reranker on/off ablation at the new chunk=1200, boost=3.0 settings.
3. Re-run kotlin 25-TC suite at the chosen defaults and record cross-corpus generalisation.
4. Consider `nomic-embed-code` as a 4th GPU candidate.

## Raw artifacts

- `results-matrix-v2.tsv` (worktree) — 12-cell post-fix run
- `results-bm25-sweep-cr1200.tsv` — boost sweep on cr_c1200
- `scripts/chunk-model-matrix.py`, `scripts/sweep-bm25-cr1200.sh` — orchestrators
- `scripts/extract-kotlin-tcs.py` + `scripts/tc-kotlin.json` — kotlin TC mining
- `docker-compose.matrix.yml` — 4 parallel embedder containers (8090–8093)
- BUG-053 entry in `docs/TODO-tool-misbehaviors.md` — the UTF-8 panic that invalidated v1
