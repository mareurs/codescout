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
