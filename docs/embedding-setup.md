# Embedding Service Setup

codescout requires an external embedding service for semantic search. The
local fastembed/ONNX backend was removed in v1.0.0 (see the ADR at
`docs/adrs/2026-05-11-remote-only-embedding.md`).

This guide gets you a working embedding service in under 5 minutes.

## Quick start with Docker

The repo ships a `docker-compose.yml` that brings up the recommended
configuration: a Hugging Face Text Embeddings Inference (TEI) container
serving `sentence-transformers/all-MiniLM-L6-v2` and a companion reranker.

```bash
# From the codescout repo root:
docker compose up -d

# Wait for both services to be healthy (~60s on first run while models
# download):
docker compose ps
```

Then point codescout at the embedding endpoint. Edit
`.codescout/project.toml`:

```toml
[embeddings]
model = "all-minilm"
url   = "http://127.0.0.1:48080/v1"
```

Restart the codescout MCP server (or your IDE integration) so the new
configuration is picked up.

## GPU acceleration

If your host has an NVIDIA GPU and `nvidia-container-toolkit` installed,
use the GPU overlay:

```bash
docker compose -f docker-compose.yml -f docker-compose.gpu.yml up -d
```

The GPU images (`ghcr.io/huggingface/text-embeddings-inference:86-1.8`)
target compute capability 8.6 (RTX 30-series, A40, A100). For other GPUs
substitute the appropriate TEI tag from
<https://github.com/huggingface/text-embeddings-inference#supported-architectures>.

## Alternative providers

The compose file is one known-good option. Any OpenAI-compatible
`/v1/embeddings` endpoint works:

| Provider | Setup notes |
|---|---|
| **Ollama** (`ollama/ollama`) | `docker run -d -p 11434:11434 ollama/ollama && docker exec ollama ollama pull all-minilm`. Then `url = "http://127.0.0.1:11434/v1"`. |
| **llama-server** (llama.cpp) | Self-host with a downloaded GGUF; serve on a port of your choice; same `url` shape. |
| **OpenAI hosted** | `url = "https://api.openai.com/v1"`, `api_key = "..."`, `model = "text-embedding-3-small"`. |

## Trying a different model

The compose file's default is intentionally low-resource. For better
retrieval quality on code, try (in `docker-compose.yml`):

- `Snowflake/snowflake-arctic-embed-m-long` — 768-d, 2048-token context,
  strong on code
- `nomic-ai/CodeRankEmbed` — code-specialized; codescout's prior benchmark
  defaulted to this model
- `BAAI/bge-m3` — 1024-d, multilingual, 8192-token context

Update the `--model-id=` line in the `embed` service, then run
`docker compose up -d --force-recreate embed` to swap.

## Troubleshooting

**`error sending request for url (http://...)`**
The embedding service isn't reachable. Confirm with
`curl http://127.0.0.1:48080/health` — expected response: `200 OK`. If it
returns nothing, run `docker compose logs embed` to see why.

**`input (N tokens) is too large to process`**
The model's context window is smaller than your chunk size. Either:
1. Lower `[embeddings] chunk_size` in `project.toml` (default is 1600
   characters ≈ 400 tokens — should fit most models out of the box).
2. Switch to a model with a larger context window (see "Trying a
   different model").

**Embedding service starts but model download takes forever**
First-run downloads from Hugging Face Hub. The
`start_period: 60s` health-check grace is conservative; for large models
(>500 MB) it can take a few minutes. Run `docker compose logs -f embed` to
watch progress.

**Models keep redownloading on `docker compose down && up`**
The `tei-models` named volume persists between recreates. If you ran
`docker compose down -v` (note the `-v`), the volume was wiped. Drop the
`-v`. Your existing codescout index auto-wipes on first run after upgrade
from v0.x — that is expected and one-time.
