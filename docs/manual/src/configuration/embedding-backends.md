# Embedding Backends

Semantic search requires converting source code into vector embeddings. codescout
talks to a single backend type: an external OpenAI-compatible `/v1/embeddings`
HTTP endpoint. There is no in-process embedding backend.

Configure it in `.codescout/project.toml`:

```toml
[embeddings]
model = "all-minilm"
url   = "http://localhost:11434/v1"
```

Any server implementing the OpenAI `/v1/embeddings` API works — Ollama,
llama.cpp, vLLM, TEI, OpenAI, Azure OpenAI, Together AI, and others.

---

## Quick Start (Ollama)

```bash
docker run -d --name ollama -p 11434:11434 ollama/ollama
docker exec ollama ollama pull all-minilm
```

```toml
[embeddings]
model = "all-minilm"
url   = "http://localhost:11434/v1"
```

For other providers see [Embeddings setup examples](embeddings.md#setup-examples).

---

## Recommended Models

| Model | Dims | Context | Notes |
|---|---|---|---|
| `all-minilm` | 384 | 256 tok | Smallest, quick start |
| `nomic-embed-text` | 768 | 8192 tok | Solid general-purpose default |
| `nomic-embed-text-v1.5` | 768 | 8192 tok | Slightly improved variant |
| `jina-embeddings-v2-base-code` | 768 | 8192 tok | Code-specialized |
| `bge-m3` | 1024 | 8192 tok | Best retrieval quality; ~1.2 GB |
| `text-embedding-3-small` | 1536 | — | OpenAI hosted; pay-per-token |
| `text-embedding-3-large` | 3072 | — | OpenAI; overkill for most codebases |

**Switching models requires a full reindex** — see
[Rebuilding After a Model Change](#rebuilding-after-a-model-change) below.
Scores are not comparable across models; a score of 0.75 means different things
with different models.

---

## Authentication

For endpoints that require auth (OpenAI, Together AI, hosted TEI behind a proxy),
set the API key via `[embeddings] api_key` or the `EMBED_API_KEY` environment
variable. The key is sent as a Bearer token.

```bash
export EMBED_API_KEY=sk-...
```

```toml
[embeddings]
model = "text-embedding-3-small"
url   = "https://api.openai.com/v1"
# api_key is read from EMBED_API_KEY by default
```

---

## Batching

codescout sends texts in batches of 8. This avoids HTTP 400 errors from servers
that have payload size limits and keeps per-request latency manageable. The
batch size is fixed and not configurable.

---

## Rebuilding After a Model Change

The embedding index records the model used to build it. If you change the
`model` field, you must rebuild the index:

```json
{ "name": "index", "arguments": { "action": "build", "force": true } }
```

codescout warns if it detects a mismatch between the configured model and the
model recorded in the existing index. Legacy indexes built with the removed
`local:`-prefix backend (codescout < 1.0.0) are auto-wiped on first run after
upgrade.

---

## Migrating From the Removed Local Backend

Before 1.0.0 codescout could embed in-process via `fastembed` / ONNX with a
`local:` model prefix. That backend was removed. Migration:

```toml
# Before
[embeddings]
model = "local:AllMiniLML6V2Q"
```

```toml
# After
[embeddings]
model = "all-minilm"
url   = "http://localhost:11434/v1"
```

Start any OpenAI-compatible embedding server (Ollama is the easiest path). The
existing index auto-wipes on first run after upgrade.

See `docs/adrs/2026-05-11-remote-only-embedding.md` for the rationale and
trade-offs.
