# Embeddings

codescout uses embeddings for semantic search — finding code by meaning rather than
exact text matches. This guide covers how to configure the embedding backend.

## Quick Start

codescout requires an external embedding service for semantic search. There is
no in-process embedding backend — `[embeddings] url` is mandatory.

Quick start with Ollama:

```bash
docker run -d --name ollama -p 11434:11434 ollama/ollama
docker exec ollama ollama pull all-minilm
```

Then in `.codescout/project.toml`:

```toml
[embeddings]
model = "all-minilm"
url   = "http://localhost:11434/v1"
```

The `url` field works with **any server implementing the OpenAI `/v1/embeddings` API**.
codescout normalizes the URL automatically — all of these are equivalent:

- `http://127.0.0.1:11434`
- `http://127.0.0.1:11434/v1`
- `http://127.0.0.1:11434/v1/embeddings`

## Setup Examples

### Ollama

```bash
ollama pull nomic-embed-text
ollama serve  # if not already running
```

```toml
[embeddings]
model = "nomic-embed-text"
url = "http://127.0.0.1:11434/v1"
```

### llama.cpp

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

### vLLM

```bash
vllm serve nomic-ai/nomic-embed-text-v1.5 --task embed --port 43300
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

### TEI (HuggingFace Text Embeddings Inference)

```bash
docker run -p 43300:80 ghcr.io/huggingface/text-embeddings-inference \
  --model-id nomic-ai/nomic-embed-text-v1.5
```

```toml
[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
```

### OpenAI

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
| `model` | string | `"all-minilm"` | Model name sent in the API request body. |
| `url` | string | *(required)* | Base URL for any OpenAI-compatible `/v1/embeddings` endpoint. |
| `api_key` | string | *(none)* | API key sent as Bearer token. Also available via `EMBED_API_KEY` env var. |
| `chunk_size` | integer | `1600` | Character budget per chunk before AST sub-splitting. |
| `drift_detection_enabled` | bool | `true` | Track how much code meaning changes between index builds. |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `EMBED_API_KEY` | API key for the embedding endpoint (alternative to config field) |

## Model Recommendations

Minimum recommended: **768 dimensions** for good code search quality.

| Model | Dims | Context | Best For |
|-------|------|---------|----------|
| all-minilm | 384 | 256 | Quick start, low resource use |
| nomic-embed-text-v1.5 | 768 | 8192 | General purpose, good quality |
| jina-embeddings-v2-base-code | 768 | 8192 | Code-specialized |
| bge-m3 | 1024 | 8192 | Best quality, larger download |
| CodeSage-small-v2 | 1024 | — | Purpose-built for code retrieval |
| text-embedding-3-small | 1536 | 8191 | OpenAI hosted, no self-hosting |

## How It Works

1. **AST-aware chunking** — tree-sitter extracts top-level definitions (functions, classes, structs). Each chunk is a complete semantic unit, not an arbitrary text window.

2. **Chunk size** — default is 1600 characters. Override via `[embeddings] chunk_size`. AST chunker preserves leaf symbols above the target; oversized leaves are line-split with a signature-prefix header on every sub-chunk.

3. **Vector storage** — embeddings are stored in sqlite-vec (`vec0` virtual tables) for fast KNN search.

## Choosing a Model

See the [Embedding Model Comparison](embedding-model-comparison.md) for benchmark
results across multiple models, real-world usage data, and recommendations.

## Troubleshooting

### Model mismatch after changing config

If you change the `model` or `url` after indexing, the stored vectors are incompatible.
Rebuild the index:

```
index(action: "build", force: true)
```

Legacy indexes built with the removed `local:` backend are auto-wiped on first
run after upgrade.

### Endpoint unreachable

Check that the server is running and the URL is correct:

```bash
curl http://127.0.0.1:11434/v1/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"all-minilm","input":["test"]}'
```
