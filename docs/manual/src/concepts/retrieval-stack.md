# The Retrieval Stack

> **This page documents the opt-in server stack** (Qdrant + sparse SPLADE +
> cross-encoder reranker), compiled with `--features server-stack`. The default
> `codescout` build runs the daemon-free [lite stack](lite-stack.md) instead —
> no Docker, no Qdrant. Read on only if you are running the server stack.

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
./scripts/fetch-models.sh
```

There is **no published GGUF repo** for CodeRankEmbed. `nomic-ai/CodeRankEmbed-GGUF`,
which this page previously recommended, returns HTTP 401 and is absent from
HuggingFace search (tracked in the repo at
`docs/issues/archive/2026-07-25-coderankembed-gguf-source-404.md`). The script instead builds the Q4_K_M quant from the official, ungated safetensors repo
(`nomic-ai/CodeRankEmbed`, ~173k downloads) using `llama.cpp:full`, producing the
same ~90 MB artifact. It is idempotent and honours `CODESCOUT_MODEL_DIR`.

> **Run it before `docker compose up`.** If a container starts first while the
> model directory is missing, Docker creates the bind-mount target as
> `root:root` and every subsequent write fails with a permission error.

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



## AMD ROCm profile (`docker compose --profile amd`)

The `amd` profile in `docker-compose.yml` runs every leg of the retrieval
stack on the GPU: dense embedder, cross-encoder reranker, and sparse SPLADE
all share the AMD device. Qdrant runs alongside on CPU as usual. This is the
recommended path on any workstation with an AMD GPU and ROCm 7.x installed
on the host.

**Bring up:**

```bash
docker compose --profile amd up -d
```

**Topology when using the `amd` profile:**

| Service | Port | Image | Notes |
|---|---|---|---|
| Qdrant | `6333` HTTP / `6334` gRPC | `qdrant/qdrant:v1.17.0` | Shared across all profiles. |
| Dense (`dense-amd`) | `48081` | `rocm/llama.cpp:llama.cpp-b6652.amd0_rocm7.0.0_ubuntu24.04_server` | llama-server `--embedding --pooling mean`, `CodeRankEmbed-Q4_K_M.gguf`. |
| Reranker (`reranker-amd`) | `48083` | same image | llama-server `--reranking --pooling rank`, `bge-reranker-v2-m3-Q4_K_M.gguf`. |
| Sparse (`sparse-amd`) | `48084` | `codescout/sparse-amd:tei-1588129f93` (built locally) | TEI-on-ROCm running SPLADE-PP_en_v1. See [SPLADE on ROCm](sparse-amd.md). |

**Required model files** in `${CODESCOUT_MODEL_DIR:-./models}`:

```bash
./scripts/fetch-models.sh --amd
```

That builds the dense model (~90 MB) and downloads the reranker (~419 MB). The
reranker *is* published as GGUF (`gpustack/bge-reranker-v2-m3-GGUF`, verified
2026-07-25) so it is a plain download; only the dense model has to be built
locally — see [Bring up the stack](#bring-up-the-stack) for why.

The SPLADE model is pulled by the `sparse-amd` container at first launch
into the `huggingface-cache` volume; no manual download needed.

**Host requirements:**
- AMD GPU (RX 7xxx / MI series), gfx1100+ recommended
- ROCm 7.x installed on host (kernel driver + `/dev/kfd`, `/dev/dri`)
- User in `video` and `render` groups

The compose service declares `devices: [/dev/kfd, /dev/dri]` and
`group_add: ["44", "992"]` (numeric video/render GIDs — the rocm/pytorch
sparse image lacks a `render` group entry, so group names don't resolve).
No NVIDIA-style runtime extension needed; AMD exposes the GPU via standard
Linux character devices.

**Wire codescout:** point at `.env.amd` (in the repo root) — **symlink or
`--env-file`, do not copy.** It sets the ports above plus the reranker protocol
selector for llama-server's Cohere-shape `/rerank` (dense is OpenAI-compatible
by default):

```bash
CODESCOUT_RERANKER_PROTOCOL=llama-server  # Cohere-shape /rerank
```

```bash
# MCP servers: one symlink, so .env.amd stays the single source of truth and
# switching profiles is re-pointing it.
ln -sfn "$PWD/.env.amd" ~/.config/codescout/.env

# Compose: name the profile explicitly rather than copying it into .env.
docker compose --env-file .env.amd --profile amd up -d
```

**Why not `cp .env.amd .env`?** Two reasons, both observed in practice:

- A copy has no link to its source, so editing `.env.amd` leaves `.env` silently
  stale. That pinned a nonexistent `CODESCOUT_MODEL_DIR` in the repo-root `.env`
  with no signal — `docs/issues/archive/2026-07-25-env-copy-flow-stale-model-dir.md`.
- Repo-root `.env` also holds secrets (`CARGO_REGISTRY_TOKEN`) that the profile
  files deliberately do not carry, so a copy **destroys** them. Keep `.env` for
  secrets only and let profile config come from `.env.amd` / `.env.gpu`.

The symlink is the mechanism the self-load-dotenv design settled on — see
`docs/superpowers/specs/2026-07-10-codescout-self-load-dotenv-design.md`.

**Why dense + reranker use llama.cpp instead of TEI:**
- TEI's ROCm path is fragile and lags upstream; `rocm/llama.cpp` is AMD-built.
- Same binary serves the dense embedder and the cross-encoder reranker
  (`--reranking` mode), so one image covers two services.

**Why sparse uses TEI:** SPLADE is an MLM-style model with no llama.cpp
implementation, and CPU latency saturates 32 cores on a full reindex of a
21k-chunk project. Building TEI from source against ROCm 7.1 + PyTorch 2.8
(see [SPLADE on ROCm](sparse-amd.md)) puts SPLADE on the
GPU and drops a full reindex from "minutes of CPU melting" to ~6 m 36 s at
121 % CPU.
## How codescout finds the stack

codescout reads endpoints from environment variables. Where a fallback exists it
is the **host** port `docker-compose.yml` publishes — not the container-internal
port, which is unreachable from the host. The dense URL has no fallback at all:

> Env-var names here are exact. There is no `CODESCOUT_SPARSE_URL` — setting that
> name configures nothing and fails silently, which is what this table said until
> 2026-08-14.

| Env | Default | Effect |
|---|---|---|
| `CODESCOUT_QDRANT_URL` | `http://127.0.0.1:6334` | Qdrant gRPC URL |
| `CODESCOUT_EMBEDDER_URL` | *(unset)* | Dense embedder base URL. **No default** — unset means "resolve the backend from the embed model", so a `local:` model runs in-process and needs no server. Set it to `http://127.0.0.1:48081` to select the compose dense service. |
| `CODESCOUT_RERANKER_URL` | `http://127.0.0.1:48083` | Reranker base URL |
| `CODESCOUT_SPARSE_EMBEDDER_URL` | `http://127.0.0.1:48084` | Sparse SPLADE base URL |
| `CODESCOUT_EMBEDDER_MODEL_NAME` | (empty) | Model id sent in OpenAI-protocol JSON payloads |
| `CODESCOUT_QUERY_PREFIX` | (empty) | Prepended to query text only. Required by some asymmetric models (e.g. Nomic, BGE-large). |
| `CODESCOUT_RERANKER_PROTOCOL` | `tei` | `tei` (HuggingFace TEI) or `llama-server`/`infinity`/`cohere` (Cohere-shape `/rerank`, used by llama-server `--reranking`) |
| `CODESCOUT_RERANKER_MODEL` | (unset) | Override the reranker model id (Infinity-protocol only) |

### Three embedder config families, and they are not the same one

"The embedder is configured" can be true and false at the same time here, because
three independently-read families of env vars exist and their names are
near-identical. Configuring one gives you no signal that the others exist.

| Family | Read by | Consumers | If unset |
|---|---|---|---|
| `CODESCOUT_EMBEDDER_URL`, `CODESCOUT_SPARSE_EMBEDDER_URL`, `CODESCOUT_RERANKER_URL`, `CODESCOUT_EMBEDDER_MODEL` | `src/retrieval/config.rs` | the retrieval stack: `semantic_search` over code, semantic memory cross-embedding, anchor creation | sparse/reranker fall back to the published host ports; the dense URL falls back to nothing and the backend is resolved from the model |
| `CODESCOUT_EMBED_URL`, `CODESCOUT_EMBED_MODEL` | `src/config/project.rs` | the same retrieval stack, at **lower precedence** — a project-config layer beneath the `CODESCOUT_EMBEDDER_*` names | ignored |
| `LIBRARIAN_EMBED_MODEL`, `LIBRARIAN_EMBED_URL`, `LIBRARIAN_EMBED_API_KEY` | `src/librarian/mod.rs` | the librarian artifact index only — `doc(semantic=…)`, `librarian(context)` | absent `LIBRARIAN_EMBED_MODEL` disables the librarian embedding service entirely |

Two consequences worth knowing before you debug an embedding failure:

- **Fixing one family does not fix another.** A working `LIBRARIAN_EMBED_URL`
  leaves code `semantic_search` exactly as broken as it was, and vice versa.
- **The retrieval stack's failures are non-fatal.** A wrong endpoint surfaces as
  `dense embed connect failed: …` while the surrounding operation still reports
  success — `memory(action="write")` stores the topic and only *degrades* the
  semantic anchor. So silence is not evidence that this is configured.

For the retrieval model specifically, precedence is highest-first:
`CODESCOUT_EMBEDDER_MODEL`, then project `[embeddings]`, then
`CODESCOUT_EMBED_MODEL`, then the built-in default `local:AllMiniLML6V2Q`.
## Using Ollama / llama.cpp / OpenAI as the dense embedder

The shipped stack uses `llama.cpp:server` for the dense leg, but the dense
service is just an HTTP endpoint behind `CODESCOUT_EMBEDDER_URL`. Any
OpenAI-compatible server will work.

### Ollama

Ollama exposes an OpenAI-compatible embeddings endpoint at
`http://localhost:11434/v1`. Pull a model and point codescout at it:

```bash
ollama pull nomic-embed-text         # or any model with /api/embeddings
export CODESCOUT_EMBEDDER_URL=http://127.0.0.1:11434
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

…then leave `CODESCOUT_EMBEDDER_URL` at its default. The compose `dense-*` service is just a packaged version of
this command — see `docker-compose.yml` for the full flag list.

### OpenAI / Anthropic-compatible APIs

```bash
export CODESCOUT_EMBEDDER_URL=https://api.openai.com/v1
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

> ⚠ **Latency here is per serving runtime, not per model.** The `Protocol` column is
> load-bearing: the same `bge-reranker-v2-m3` weights served as a Q4_K_M GGUF on
> llama-server (the swap made on 2026-07-27 to work around a CUDA OOM) measured
> **p95 3091 ms** — roughly an order of magnitude above the TEI row above. If you are
> on the AMD profile, which sets `CODESCOUT_RERANKER_PROTOCOL=llama-server`, budget
> from that number, not this table. Measurements and the open question of whether the
> reranker earns its keep at all are in
> `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`; note that
> file's own retraction — the arms it compared differ in four dimensions, so it
> establishes the latency cost but **not** a score regression.

> ⚠ **The reranker is OFF by default as of 2026-08-07. Set `CODESCOUT_RERANK=1` to enable
> it.** The open question above was answered by a clean A/B — same rebuilt index, same
> binary, same 25-TC suite, arms differing in exactly **one** dimension, which is what the
> earlier four-dimension comparison could not do:
>
> | arm | full /75 | warm median latency |
> |---|---|---|
> | reranker on | 23 | 1559 ms |
> | **reranker off** | **26** | **990 ms** |
>
> About **569 ms per query** for a result that did not improve — it helped 4 of the 25 test
> cases and hurt 5. A 3-point net on a 25-case suite is not a large effect and should not be
> read as one; the sufficient claim is that reranking does not measurably improve retrieval
> on this stack while costing half a second per query. Not a trade-off — a coin flip with a
> price.
>
> **This is a statement about *this* serving runtime, not about reranking.** The rows above
> are `llama-server` on the AMD profile; the same weights over TEI measure ~80 ms, where the
> calculus is entirely different — hence a knob rather than a deletion. If you run a
> different cross-encoder or protocol, measure it yourself with
> `scripts/run-tc-benchmark.py`, and **discard the first query of the run**: cold-start
> inflated the sparse-embed figure ~9× in this same session, and a mean over a run that
> started cold folds a one-off into every per-query number.
>
> These latencies are full **MCP round-trip**, not the retrieval-stage figures in the
> stack-wide table below — different quantities, not comparable.

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

**This is a bigger step than it looks.** The ONNX path currently implies a
model change, a full reindex of every code index and every memory, and
dense-only retrieval — not just a backend swap. `local:<name>` only resolves a
fixed allowlist (`NomicEmbedTextV15`, `JinaEmbeddingsV2BaseCode`,
`BGESmallENV15`, `AllMiniLML6V2`, and their quantized variants) — it does not
include `CodeRankEmbed`, so an index built against the network stack cannot be
reused; every index and every memory needs a fresh `index(action="build",
force=true)` / re-embed after switching. Budget for that before treating this
as a quick escape hatch.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `semantic_search` returns "stack unreachable" | dense/sparse/rerank/qdrant container not running | `docker compose ps` then start the missing profile |
| Empty results despite indexed data | wrong project_id namespace | `workspace status` to confirm the active project_id; `codescout index --force` to rebuild |
| Slow first query (10+ s) | model warmup on cold container | normal — subsequent queries hit the loaded model |
| `migrate-memories` reports "db not found" | legacy file at unexpected path | pass `--db-path /path/to/embeddings.db` explicitly |
