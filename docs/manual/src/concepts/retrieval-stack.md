# The Retrieval Stack

> **As of v0.12 codescout's default retrieval substrate is a network-attached stack
> (Qdrant + three embedding services), not the in-process local-embed
> path.** The `local-embed` Cargo feature still exists for air-gapped use, but it
> is no longer the default and is no longer the path the team benchmarks against.
> If you upgrade from <0.12 and want to keep working, you must either bring up
> the stack or rebuild with `--features local-embed` and accept the older
> sqlite-vec code path. See [Migration from local-embed](#migration-from-local-embed)
> below.

## What runs where

| Service | Default port | Image / binary | Role |
|---|---|---|---|
| Qdrant | `6334` (gRPC), `6333` (HTTP) | `qdrant/qdrant:v1.17.0` | Vector storage. Two collections: `code_chunks`, `memories`. |
| Dense embedder | `48081` (HTTP) | `llama.cpp:server` running `CodeRankEmbed-Q4_K_M.gguf` (default) | Text → 768-dim dense vector. Speaks TEI protocol; switchable to OpenAI protocol for Ollama / OpenAI / Anthropic-compatible endpoints. |
| Sparse SPLADE | `48084` (HTTP) | `text-embeddings-inference` running `prithivida/Splade_PP_en_v1` | Text → sparse vector for lexical complement. |
| Reranker | `48083` (HTTP) | `text-embeddings-inference` running `BAAI/bge-reranker-base` (CPU) or `bge-reranker-v2-m3` (GPU) | Cross-encoder pairwise re-rank of fused candidates. |

codescout connects to these services on `127.0.0.1`. There is no per-project
substrate — the stack is shared across all projects on a machine.

## Bring up the stack

```bash
# CPU profile (default — works on any Linux/macOS machine, ~3 GB RAM idle):
docker compose --profile cpu up -d

# GPU profile (CUDA — uses NVIDIA runtime, ~2.5 GB VRAM idle):
docker compose --profile gpu up -d
```

The dense embedder needs a GGUF model file. First-run setup:

```bash
mkdir -p models
cd models
huggingface-cli download nomic-ai/CodeRankEmbed-GGUF \
    CodeRankEmbed-Q4_K_M.gguf --local-dir .
# Or: wget https://huggingface.co/nomic-ai/CodeRankEmbed-GGUF/resolve/main/CodeRankEmbed-Q4_K_M.gguf
```

If your `models/` directory is somewhere else, set `CODESCOUT_MODEL_DIR` before
`docker compose up`.

Verify everything is healthy:

```bash
docker compose ps                 # all services "healthy"
curl -fsS http://127.0.0.1:48081/health   # dense
curl -fsS http://127.0.0.1:48083/health   # reranker
curl -fsS http://127.0.0.1:48084/health   # sparse
curl -fsS http://127.0.0.1:6333/healthz   # qdrant
```

## How codescout finds the stack

codescout reads endpoints from environment variables and falls back to the
defaults above:

| Env | Default | Effect |
|---|---|---|
| `CODESCOUT_QDRANT_URL` | `http://127.0.0.1:6334` | Qdrant gRPC URL |
| `CODESCOUT_EMBEDDER_URL` | `http://127.0.0.1:48081` | Dense embedder base URL |
| `CODESCOUT_RERANKER_URL` | `http://127.0.0.1:48083` | Reranker base URL |
| `CODESCOUT_SPARSE_URL` | `http://127.0.0.1:48084` | Sparse SPLADE base URL |
| `CODESCOUT_EMBEDDER_PROTOCOL` | `tei` | `tei` (TEI/llama-server native) or `openai` (Ollama, OpenAI, Anthropic-compatible) |
| `CODESCOUT_EMBEDDER_MODEL_NAME` | (empty) | Model id sent in OpenAI-protocol JSON payloads |
| `CODESCOUT_QUERY_PREFIX` | (empty) | Prepended to query text only. Required by some asymmetric models (e.g. Nomic, BGE-large). |
| `CODESCOUT_RERANKER_PROTOCOL` | `tei` | `tei` (HuggingFace TEI) or `infinity` (Cohere/Infinity-compatible) |
| `CODESCOUT_RERANKER_MODEL` | (unset) | Override the reranker model id (Infinity-protocol only) |

## Using Ollama / llama.cpp / OpenAI as the dense embedder

The shipped stack uses `llama.cpp:server` for the dense leg, but the dense
service is just an HTTP endpoint behind `CODESCOUT_EMBEDDER_URL`. Any
TEI-compatible or OpenAI-compatible server will work.

### Ollama

Ollama exposes an OpenAI-compatible embeddings endpoint at
`http://localhost:11434/v1`. Pull a model and point codescout at it:

```bash
ollama pull nomic-embed-text         # or any model with /api/embeddings
export CODESCOUT_EMBEDDER_URL=http://127.0.0.1:11434
export CODESCOUT_EMBEDDER_PROTOCOL=openai
export CODESCOUT_EMBEDDER_MODEL_NAME=nomic-embed-text

# Optional — Nomic needs a query prefix for asymmetric search:
export CODESCOUT_QUERY_PREFIX="search_query: "
```

You still need Qdrant + the reranker + the sparse service running from the
docker-compose stack — Ollama only replaces the dense leg. Stop the compose
`dense-cpu` or `dense-gpu` container so the port is free:

```bash
docker compose --profile cpu stop dense-cpu
```

### llama.cpp (standalone)

If you already run `llama-server` outside docker, the same approach applies:

```bash
llama-server -m ~/models/CodeRankEmbed-Q4_K_M.gguf \
    --port 48081 --embedding --pooling mean --ctx-size 8192
```

…then leave `CODESCOUT_EMBEDDER_URL` and `CODESCOUT_EMBEDDER_PROTOCOL` at
their defaults. The compose `dense-*` service is just a packaged version of
this command — see `docker-compose.yml` for the full flag list.

### OpenAI / Anthropic-compatible APIs

```bash
export CODESCOUT_EMBEDDER_URL=https://api.openai.com/v1
export CODESCOUT_EMBEDDER_PROTOCOL=openai
export CODESCOUT_EMBEDDER_MODEL_NAME=text-embedding-3-small
# (codescout reads OPENAI_API_KEY from the environment automatically)
```

Cost: a full index of a ~10k-file Rust project is roughly 8 M tokens at
~768-dim. Budget accordingly.

## How we chose the components — benchmark summary

A 75-query retrieval benchmark was run across ~15 candidate stacks on a
pinned worktree of this repo. The full history lives in
[`docs/trackers/retrieval-benchmark.md`](https://github.com/mareurs/codescout/blob/master/docs/trackers/retrieval-benchmark.md).
Headline results below — all measured on the same query set at
`bm25_boost=5.0`, `mode=code`, with cross-encoder rerank enabled unless
noted.

### Dense embedder

| Model | Quantization | Query prefix | Score (out of 75) | Notes |
|---|---|---|---|---|
| **CodeRankEmbed** | Q4_K_M (90 MB) | none | **37** | **Champion.** Best on env-var / identifier-bag queries. Q4 loses asymmetric subspace if a prefix is forced. |
| CodeRankEmbed | f16 (~550 MB) | required | 34 | f16 with prefix peaked one point below Q4 no-prefix. |
| jina-embeddings-v2-base-code | (native) | none | 36 | Strong general-code model; +2 vs jina without sparse fusion. |
| Nomic Embed Code 7B | Q4 | required | 24 | "Claimed CoIR SOTA" failed on real-world queries — bigger is not better. |
| Tavily-stack baseline (CodeRank, no rerank, sqlite-vec + tantivy) | Q4_K_M | none | 28 | **Reference point** for the legacy substrate we replaced. |

**Why Q4 over f16:** Q4_K_M scores higher than f16 in our query set when no
prefix is set, and runs in ≤1 GB RAM. The f16 advantage only appears when
the model's asymmetric query prefix is enabled, and even then it caps one
point below Q4 no-prefix. We default to Q4 no-prefix.

### Sparse leg

We initially shipped a local Tantivy BM25 leg. It scored similarly to
SPLADE on lexical queries but was a maintenance burden (tantivy compile
time, on-disk index drift, separate rebuild step) and could not run as a
service. We migrated to SPLADE-PP_en_v1 via TEI — same conceptual role,
runs as a container, no per-project index. The benchmark showed sparse
fusion gives +2 points over dense-only at `bm25_boost=5.0`.

### Reranker

| Model | Protocol | T5 (real-usage tier, /15) | Full /75 | Latency (p95) |
|---|---|---|---|---|
| **bge-reranker-v2-m3** | TEI | 10 | **37** | ~80 ms (GPU) |
| bge-reranker-base | TEI | 9 | 35 | ~250 ms (CPU) |
| jina-rerank-v2 | Infinity | **11** | 38 (jina-v2 dense), 36 (CodeRank Q4 dense) | ~120 ms |

bge-v2-m3 wins on the full suite and is the default. jina-rerank-v2 lifts
the T5 (real-usage) tier by +1 every time but loses on long natural-language
queries. The protocol toggle (`CODESCOUT_RERANKER_PROTOCOL=infinity`) lets
you swap with a single env var — no rebuild needed.

### Stack-wide latency (champion config)

| Stage | CPU profile | GPU profile |
|---|---|---|
| Dense embed (single query) | ~30 ms | ~5 ms |
| Sparse embed (single query) | ~80 ms | ~30 ms |
| Qdrant hybrid search (RRF) | ~10 ms | ~10 ms |
| Cross-encoder rerank (top-20) | ~250 ms | ~80 ms |
| **End-to-end `semantic_search`** | **~370 ms** | **~125 ms** |

Indexing throughput on the codescout repo itself (~3.5 k chunks):

| Profile | Wall time | Throughput |
|---|---|---|
| CPU | ~45 s | ~80 chunks/s |
| GPU | ~12 s | ~290 chunks/s |

## Migration from local-embed

If you have a `.codescout/embeddings/project.db` from a pre-v0.12 install:

```bash
# 1. Stand up the stack (see above)
# 2. Re-embed legacy memories into Qdrant:
codescout migrate-memories --dry-run    # preview
codescout migrate-memories              # execute
# 3. Re-index your project:
codescout index
```

The legacy sqlite-vec file is no longer read after migration. You can delete
it once you've verified `memory recall` works against the new substrate.

If you cannot run the stack (air-gapped, embedded environment), build with
`local-embed`:

```bash
cargo install codescout --no-default-features --features local-embed,http,librarian
```

This restores the in-process ONNX + fastembed path. Note: the network
retrieval pipeline (sparse fusion, cross-encoder rerank) is not available in
this mode — `semantic_search` falls back to pure dense vector scoring.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `semantic_search` returns "stack unreachable" | dense/sparse/rerank/qdrant container not running | `docker compose ps` then start the missing profile |
| Empty results despite indexed data | wrong project_id namespace | `workspace status` to confirm the active project_id; `codescout index --force` to rebuild |
| Slow first query (10+ s) | model warmup on cold container | normal — subsequent queries hit the loaded model |
| `migrate-memories` reports "db not found" | legacy file at unexpected path | pass `--db-path /path/to/embeddings.db` explicitly |
