# Embeddings

codescout uses embeddings for semantic search — finding code by meaning rather than
exact text matches. This guide covers how to configure the embedding backend.

## Quick Start

codescout works out of the box with a bundled embedding model. No setup needed.

On first `index_project`, it downloads **all-MiniLM-L6-v2** (~22 MB, quantized)
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
| `drift_detection_enabled` | bool | `true` | Track how much code meaning changes between index builds. |

### Resolution Order

When codescout needs to embed text, it resolves the backend in this order:

1. **`url` is set** → use it as an OpenAI-compatible endpoint
2. **`model` starts with `local:`** → bundled ONNX model via fastembed
3. **`model` starts with `ollama:`** → Ollama API *(deprecated — use `url` instead)*
4. **`model` starts with `openai:`** → OpenAI API with `OPENAI_API_KEY`
5. **No `url`, no prefix** → try as a local model name, then error with suggestions

### Environment Variables

| Variable | Description |
|----------|-------------|
| `EMBED_API_KEY` | API key for the embedding endpoint (alternative to config field) |
| `OPENAI_API_KEY` | OpenAI API key (used with `openai:` prefix) |
| `OLLAMA_HOST` | Ollama daemon URL (deprecated — use `url` field) |

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

## Troubleshooting

### Model mismatch after changing config

If you change the `model` or `url` after indexing, the stored vectors are incompatible.
Rebuild the index:

```
index_project(force: true)
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
