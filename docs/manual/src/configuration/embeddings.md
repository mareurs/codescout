# Embeddings

> **Status, corrected 2026-09-17.** This banner previously said the `[embeddings]`
> block was superseded and that "the `model` / `url` / `api_key` fields in
> project.toml no longer drive search", qualified to say only `model` still worked.
> **That was inverted in both directions**, and following it produced the one
> configuration that fails. Measured against a logging `/v1/embeddings` server:
> `url` and `api_key` drove search; `model` was the field being discarded.
>
> All three are live now. The `model` half was a real defect and is fixed — see
> `docs/issues/archive/2026-09-17-the-configured-embedding-model-is-discarded-whenever-a-url-is-set.md`.
>
> **Where embeddings are configured — two places, in this order:**
>
> | | |
> |---|---|
> | `~/.config/codescout/config.toml` | machine-wide default |
> | `<project>/.codescout/project.toml` | per-project override, optional |
>
> They deep-merge **per field**, so a project can override `model` and inherit
> `url` and `api_key` — see [Global Config](global-config.md).
>
> `CODESCOUT_*` environment variables override **both**, and are an escape hatch
> for CI and benchmark runs rather than a third place to keep settings. Note that
> `~/.config/codescout/.env` is loaded into the environment at startup, so values
> written there outrank every project's own config.
>
> **The [Retrieval Stack](../concepts/retrieval-stack.md) is a different axis, not
> a replacement.** It covers Qdrant, the sparse SPLADE leg and the cross-encoder
> reranker — which are configured by `CODESCOUT_*` only. `[embeddings]` configures
> the **dense embedder** on either substrate.

codescout uses embeddings for semantic search — finding code by meaning rather than
exact text matches. This guide covers how to configure the embedding backend.

## Quick Start

codescout works out of the box with a bundled embedding model. No setup needed.

On first `index(action: build)`, it downloads **all-MiniLM-L6-v2** (~22 MB, quantized)
to `~/.cache/huggingface/hub/` and runs it locally via ONNX. This is a one-time download.

```toml
# .codescout/project.toml (default — no changes needed)
[embeddings]
model = "local:AllMiniLML6V2Q"
```

This is fine for single-project use or getting started. For better performance
with multiple projects, see the next section.

## Recommended: External Embedding Server

The bundled model loads into memory per codescout instance. With multiple projects
open, this duplicates memory (~22 MB each for the default model). A dedicated embedding server avoids this:

- **One process** serves all codescout instances
- **No memory duplication** — the model loads once
- **Faster queries** — the model stays warm
- **Model freedom** — use any model and quantization

### Configuration

Point codescout at your server with two fields:

```toml
[embeddings]
model = "nomic-embed-text-v1.5"          # model name (sent in API request)
url = "http://127.0.0.1:43300/v1"        # your server's base URL
# api_key = "optional-key"               # or set EMBED_API_KEY env var
```

The `url` field works with **any server implementing the OpenAI `/v1/embeddings` API**.
codescout normalizes the URL automatically — all of these are equivalent:

- `http://127.0.0.1:43300`
- `http://127.0.0.1:43300/v1`
- `http://127.0.0.1:43300/v1/embeddings`

### Setup Examples

#### llama.cpp

Download a GGUF model and start the server:

```bash
# Download (example: nomic-embed-text quantized)
wget https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF/resolve/main/nomic-embed-text-v1.5.Q8_0.gguf

# Start server
llama-server -m nomic-embed-text-v1.5.Q8_0.gguf --embeddings --port 43300
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

#### Ollama

```bash
ollama pull nomic-embed-text
ollama serve  # if not already running
```

```toml
[embeddings]
model = "nomic-embed-text"
url = "http://127.0.0.1:11434/v1"
```

#### vLLM

```bash
vllm serve nomic-ai/nomic-embed-text-v1.5 --task embed --port 43300
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

#### TEI (HuggingFace Text Embeddings Inference)

```bash
docker run -p 43300:80 ghcr.io/huggingface/text-embeddings-inference \
  --model-id nomic-ai/nomic-embed-text-v1.5
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

#### OpenAI

```toml
[embeddings]
model = "text-embedding-3-small"
url = "https://api.openai.com/v1"
api_key = "sk-..."  # or set EMBED_API_KEY env var
```

## Configuration Reference

### `[embeddings]` fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `model` | string | `"local:AllMiniLML6V2Q"` | Model name. With `url`: sent in API body. Without `url`: prefix determines backend. |
| `url` | string | *(none)* | Base URL for any OpenAI-compatible `/v1/embeddings` endpoint. |
| `api_key` | string | *(none)* | API key sent as Bearer token. Also available via `EMBED_API_KEY` env var. |


### Resolution Order

When codescout needs to embed text, it resolves the backend in this order:

1. **`url` is set** → use it as an OpenAI-compatible endpoint
2. **`model` starts with `local:`** → bundled ONNX model via fastembed
3. **`model` starts with `ollama:`** → Ollama API *(deprecated — use `url` instead)*
4. **`model` starts with `openai:`** → OpenAI API with `OPENAI_API_KEY`
5. **No `url`, no prefix** → try as a local model name, then error with suggestions

### Environment Variables

Every variable here **overrides both config layers**. They exist for CI and
benchmark runs; they are not a third place to keep settings.

Since 2026-09-17 they share one prefix, `CODESCOUT_EMBEDDING_*`, so the surface is
greppable as a set. **Every older name still works** and warns once naming its
replacement.

| Canonical | Deprecated alias(es) | Description |
|---|---|---|
| `CODESCOUT_EMBEDDING_URL` | `CODESCOUT_EMBEDDER_URL`, `CODESCOUT_EMBED_URL` | Endpoint base URL |
| `CODESCOUT_EMBEDDING_MODEL` | `CODESCOUT_EMBEDDER_MODEL`, `CODESCOUT_EMBED_MODEL` | Model spec, or the wire name when `url` is set |
| `CODESCOUT_EMBEDDING_API_KEY` | `EMBED_API_KEY` | Bearer token; dropped unless https or loopback |
| `CODESCOUT_EMBEDDING_DIM` | `CODESCOUT_MODEL_DIM` | Pin the expected dimension; unset means "ask the model" |
| `CODESCOUT_EMBEDDING_QUERY_PREFIX` | `CODESCOUT_QUERY_PREFIX` | Query-side prefix for asymmetric models |

**Why they were deprecated rather than left alone.** Eight names reached three
settings, and the two families applied at *different layers of the same
resolution* — `CODESCOUT_EMBED_*` inside the project-config load,
`CODESCOUT_EMBEDDER_*` in the merge below it. Which one won depended on where you
looked, and nothing declared the set, so it could grow without anyone noticing.

Not in the family, and deliberately so — these configure separate services rather
than the dense embedder:

| Variable | Description |
|---|---|
| `CODESCOUT_SPARSE_EMBEDDER_URL` | SPLADE sparse leg |
| `CODESCOUT_RERANKER_URL` | Cross-encoder reranker |
| `OPENAI_API_KEY` | Fallback for the `openai:` prefix only — a third-party convention, not ours |
| `OLLAMA_HOST` | Ollama daemon URL, for the `ollama:` prefix |
| `CODESCOUT_EMBEDDER_MODEL_NAME` | **Deprecated override**, see below |

**`CODESCOUT_EMBEDDER_MODEL_NAME` is now redundant.** It sets the model name sent
on the wire when a `url` is configured, and it wins over everything else. It exists
because `[embeddings].model` used to be *discarded* on that path — that defect is
fixed, so the configured model reaches the wire on its own. It is kept on top
because every stack deployment sets it while leaving `model` at the built-in
default: letting `model` win would silently repoint all of them. Unset it and the
resolved model is used, with its routing prefix stripped (`local:X` is sent as `X`).

`~/.config/codescout/.env` is read into the environment at startup, so anything set
there behaves as an override of every project — not as a default beneath them.
codescout warns when a value from that file shadows one your config set. Put
machine-wide defaults in `~/.config/codescout/config.toml` instead, where the
project layer can override them.

## Model Recommendations

Minimum recommended: **768 dimensions** for good code search quality.

| Model | Dims | Download | Context | Best For |
|-------|------|----------|---------|----------|
| nomic-embed-text-v1.5 | 768 | ~158 MB (Q) / ~547 MB | 8192 | General purpose, good quality |
| jina-embeddings-v2-base-en | 768 | ~300 MB | 8192 | Code-specialized |
| bge-m3 | 1024 | ~1.2 GB | 8192 | Best quality, needs external server |
| CodeSage-small-v2 | 1024 | ~500 MB | — | Purpose-built for code retrieval |
| text-embedding-3-small | 1536 | API only | 8191 | OpenAI hosted, no self-hosting |

### Bundled Local Models

These work with the `local:` prefix (no server needed):

| Model ID | Dims | Size | Context | Notes |
|----------|------|------|---------|-------|
| `NomicEmbedTextV15Q` | 768 | ~158 MB | 8192 | General purpose, good quality |
| `NomicEmbedTextV15` | 768 | ~547 MB | 8192 | Full precision variant |
| `JinaEmbeddingsV2BaseCode` | 768 | ~300 MB | 8192 | Code-specialized |
| `AllMiniLML6V2Q` | 384 | ~22 MB | 256 | **Default** — bundled, zero-config |
| `AllMiniLML6V2` | 384 | ~90 MB | 256 | Full precision lightweight |

## How It Works

1. **AST-aware chunking** — tree-sitter extracts top-level definitions (functions, classes, structs). Each chunk is a complete semantic unit, not an arbitrary text window.

2. **Chunk size auto-derived** — codescout calculates chunk size from the model's context window. No manual tuning needed.

3. **Vector storage** — embeddings are upserted into Qdrant's `code_chunks` collection over gRPC (default `localhost:6334`). Both a dense and a sparse vector are stored per chunk; query-time hybrid search fuses them via RRF inside Qdrant. See [Hybrid Dense + Sparse Retrieval](../concepts/hybrid-bm25-vector.md) for the topology.

4. **Bundled model lifecycle** — when using the `local:` prefix (compile-time `local-embed` feature), the ONNX model is loaded lazily on first `semantic_search` or `index(action="build")`, cached for 5 minutes, then unloaded to free memory. The default substrate is the HTTP dense embedder service, not the bundled ONNX path.
## Choosing a Model

Not sure which model to use? See the [Embedding Model Comparison](embedding-model-comparison.md)
for benchmark results across three models, real-world usage data, and recommendations.

**TL;DR:** The default (`local:AllMiniLML6V2Q`) is within 2 points of the best model on a
60-point benchmark, indexes 21x faster, and requires zero setup. Keep it unless you have
a specific reason to change.

On a first `onboarding` run the default is not what you get unconditionally —
the model written into `.codescout/project.toml` is ranked from the host's RAM,
cores, GPU and reachable Ollama, and from the features the binary was built
with. What each probe is evidence for, and the 16 GB / 8-core crossover above
which the code-specialized model leads, are in
[Onboarding § Hardware-Aware Model Selection](../concepts/onboarding-improvements.md).

## Troubleshooting

### Model mismatch after changing config

If you change the `model` or `url` after indexing, the stored vectors are incompatible.
Rebuild the index:

```
index(action: build, force: true)
```

### Endpoint unreachable

Check that the server is running and the URL is correct:

```bash
curl http://127.0.0.1:43300/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"nomic-embed-text","input":["test"]}'
```

### Corporate proxy blocking downloads

The bundled model downloads from HuggingFace. If your proxy blocks this:

1. Download the model on an unrestricted machine
2. Copy to `~/.cache/huggingface/hub/models--nomic-ai--nomic-embed-text-v1.5/`
3. Or use an external server instead (set `url`)

## Migration from Prefix Syntax

The `ollama:` prefix is deprecated and will be removed in a future version.
Migrate to the `url` field:

```toml
# Before (deprecated)
[embeddings]
model = "ollama:nomic-embed-text"
```

```toml
# After
[embeddings]
model = "nomic-embed-text"
url = "http://localhost:11434/v1"
```

The `custom:` prefix has been removed. Migrate to the `url` field:

```toml
# Before (removed)
[embeddings]
model = "custom:my-model@http://my-server:8080"
```

```toml
# After
[embeddings]
model = "my-model"
url = "http://my-server:8080/v1"
```
