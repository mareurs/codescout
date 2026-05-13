# Embedding Backends

Semantic search requires converting source code into vector embeddings. codescout supports
four backends, selected at runtime by the `model` field in `[embeddings]` inside
`.codescout/project.toml`. The prefix before the colon determines which backend is used.

```toml
[embeddings]
model = "ollama:nomic-embed-text"   # recommended when Ollama is available
```

The `onboarding` tool detects your hardware at setup time and writes the best model for
your machine into this field automatically. You rarely need to set it manually — see
[Choosing a Backend](#choosing-a-backend) if you want to override it.

---

## Backend Comparison

| Backend | Speed | Quality | Cost | Privacy | Setup |
|---|---|---|---|---|---|
| Local (fastembed, **default**) | Fast (CPU) | Good–Excellent | Free | Fully local | Bundled — no setup needed |
| Ollama | Medium | Good | Free | Local | Install Ollama + pull model |
| OpenAI | Fast (network) | Excellent | Pay-per-token | Data sent to OpenAI | Set `OPENAI_API_KEY` |
| Custom endpoint | Varies | Varies | Varies | Depends on host | Point at any compatible server |

---

## Recommended Models

`onboarding` picks the best model for your machine automatically. This table is a reference
for manual overrides or when comparing options.

| Model string                    | Backend  | Dims | Context | Code quality | Notes                                        |
|---------------------------------|----------|------|---------|--------------|----------------------------------------------|
| `local:AllMiniLML6V2Q`          | fastembed|  384 |  256 tok | Good       | **Default.** 22 MB, zero-config, bundled.    |
| `local:JinaEmbeddingsV2BaseCode`| fastembed|  768 | 8192 tok | Excellent  | **Recommended (CPU-only).** Code-specific.   |
| `ollama:nomic-embed-text`       | Ollama   |  768 | 8192 tok | Good       | Recommended if Ollama is already running.    |
| `ollama:bge-m3`                 | Ollama   | 1024 | 8192 tok | Excellent  | Best Ollama quality; slower, ~1.2 GB.        |
| `openai:text-embedding-3-small` | OpenAI   | 1536 | —       | Excellent    | Best quality/cost if cloud is acceptable.    |
| `openai:text-embedding-3-large` | OpenAI   | 3072 | —       | Best         | Overkill for most codebases.                 |
| `ollama:mxbai-embed-large`      | Ollama   | 1024 |  512 tok | Good       | Legacy. Short context truncates most code.   |

**Switching models requires a full reindex** — see
[Rebuilding After a Model Change](#rebuilding-after-a-model-change) below.
Scores are not comparable across models; a score of 0.75 means different things
with different models.

---

## Ollama

Uses a locally running [Ollama](https://ollama.com/) daemon. No API key is required.

**Model string format:** `"ollama:<model-name>"`

**Endpoint:** `$OLLAMA_HOST/v1/embeddings` (default: `http://localhost:11434/v1/embeddings`)

### Setup

1. Install Ollama from [ollama.com](https://ollama.com/).
2. Pull the embedding model:

   ```bash
   ollama pull mxbai-embed-large
   ```

3. Make sure the daemon is running:

   ```bash
   ollama serve
   ```

### Configuration

```toml
[embeddings]
model = "ollama:mxbai-embed-large"
```

To use a different Ollama host (e.g. a remote machine or a custom port), set the
`OLLAMA_HOST` environment variable before starting the MCP server:

```bash
export OLLAMA_HOST=http://192.168.1.50:11434
```

### Automatic CPU Fallback

If Ollama is not running when codescout tries to connect, you'll see a clear error:

```
Ollama is not reachable at http://localhost:11434
```

**Options:**
- Start Ollama: `ollama serve`
- Switch to bundled ONNX: set `model = "local:AllMiniLML6V2Q"` in `[embeddings]`
- Use a different server: set `url = "http://your-server:port/v1"` in `[embeddings]`

### Recommended Ollama Models

| Model | Dimensions | Context | Notes |
|---|---|---|---|
| `nomic-embed-text` | 768 | 8192 tok | **Recommended default.** Fast indexing, 137 MB. |
| `bge-m3` | 1024 | 8192 tok | Best retrieval quality; ~1.2 GB download. |
| `mxbai-embed-large` | 1024 | 512 tok | Legacy; short context truncates most functions. |

---

## OpenAI

Calls the OpenAI embeddings API. Requires an active OpenAI account and an API key.

**Model string format:** `"openai:<model-name>"`

**Endpoint:** `https://api.openai.com/v1/embeddings`

**Authentication:** `$OPENAI_API_KEY` environment variable (required)

### Setup

Set your API key in the environment before starting the MCP server:

```bash
export OPENAI_API_KEY=sk-...
```

### Configuration

```toml
[embeddings]
model = "openai:text-embedding-3-small"
```

### Recommended OpenAI Models

| Model | Dimensions | Notes |
|---|---|---|
| `text-embedding-3-small` | 1536 | Low cost, good quality, recommended |
| `text-embedding-3-large` | 3072 | Highest quality, higher cost |
| `text-embedding-ada-002` | 1536 | Legacy model, still widely used |

---

## Custom Endpoint

Points at any OpenAI-compatible embeddings API — useful for self-hosted models, Azure OpenAI,
Together AI, or other third-party providers.

**Model string format:** `"custom:<model-name>@<base-url>"`

codescout appends `/v1/embeddings` to `<base-url>`, so a base URL of
`http://localhost:1234` becomes `http://localhost:1234/v1/embeddings`.

**Authentication:** `$EMBED_API_KEY` environment variable (optional — set it if the server
requires a bearer token)

### Setup

Start your compatible server, then set the API key if needed:

```bash
export EMBED_API_KEY=your-token-here
```

### Configuration

```toml
[embeddings]
model = "custom:mxbai-embed-large@http://localhost:1234"
```

Examples for common providers:

```toml
# Azure OpenAI
model = "custom:text-embedding-3-small@https://my-resource.openai.azure.com/openai/deployments/my-deployment"

# Together AI
model = "custom:togethercomputer/m2-bert-80M-8k-retrieval@https://api.together.xyz"

# Hugging Face Text Embeddings Inference (TEI)
model = "custom:BAAI/bge-large-en-v1.5@http://localhost:8080"
```

---

## Local (fastembed)

Runs entirely on-device using [fastembed-rs](https://github.com/Anush008/fastembed-rs) and
ONNX Runtime. No external daemon, no API key, and no network traffic after the initial model
download.

**Model string format:** `"local:<EmbeddingModel-variant>"`

**Requires:** building from source with the `local-embed` Cargo feature. This backend is
**not available in the published `cargo install codescout` binary** because ONNX Runtime is
a native system library that cannot be bundled through crates.io.

> **Looking for free local embeddings without building from source?** Use [Ollama](#ollama-default)
> instead — it is the recommended path for most users.

**Model cache:** `~/.cache/huggingface/hub/` — downloaded on first use, then fully offline

### Build

Clone the repository and build with local embedding support:

```bash
git clone https://github.com/mareurs/codescout.git
cd codescout
cargo install --path . --features local-embed
```

Or, to have both local and remote backends available simultaneously:

```bash
cargo install --path . --features remote-embed,local-embed
```

### Configuration

```toml
[embeddings]
model = "local:AllMiniLML6V2Q"
```

### Supported Local Models

| Model string | Dimensions | Download size | Notes |
|---|---|---|---|
| `local:JinaEmbeddingsV2BaseCode` | 768 | ~300 MB | Code-specific, highest quality for source code |
| `local:AllMiniLML6V2Q` | 384 | ~22 MB | INT8-quantized, CPU-safe, recommended for most users |
| `local:BGESmallENV15Q` | 384 | ~20 MB | GPU-optimized export; may fail on CPU-only machines  |
| `local:BGESmallENV15` | 384 | ~65 MB | Full f32 precision variant of BGESmallENV15Q |
| `local:AllMiniLML6V2` | 384 | ~90 MB | Full f32 precision variant of AllMiniLML6V2Q |

For most local setups, `AllMiniLML6V2Q` gives the best tradeoff: small download, CPU-safe
inference, and solid retrieval quality. Use `JinaEmbeddingsV2BaseCode` when search quality
on code is the priority and the larger download is acceptable.

### Error When Feature Is Missing

If you try to use a `local:` model without the `local-embed` feature compiled in, you will
see an error like:

```
Local embedding requires the 'local-embed' feature.
Rebuild with: cargo build --features local-embed
```

---

## Batching

All backends send texts in batches of 8. This avoids HTTP 400 errors from servers that have
payload size limits (Ollama is particularly strict about this) and keeps per-request latency
manageable. The batch size is fixed and not configurable.

---

## Rebuilding After a Model Change

The embedding index stores which model was used to build it. If you change the `model` field
in `project.toml`, you must rebuild the index with the `force` flag to avoid mixing vectors
from different models:

```json
{ "name": "index(action: build)", "arguments": { "force": true } }
```

codescout will warn if it detects a mismatch between the configured model and the model
recorded in the existing index.

---

## Choosing a Backend

**In most cases you do not need to choose** — `onboarding` probes your hardware and writes
the recommended model into `.codescout/project.toml` automatically. The decision tree below
is for manual overrides.

- **Default / getting started** → `local:AllMiniLML6V2Q` — bundled, 22 MB, no setup.
  Already active out of the box; no config change needed.
- **Better code search, can build from source** → `local:JinaEmbeddingsV2BaseCode`
  (code-specific, 8192-token context, ~300 MB). Build with `--features local-embed` from the
  [repository](https://github.com/mareurs/codescout). Outperforms general-purpose models on code.
- **Ollama is already running** → `ollama:nomic-embed-text` (fast, 8192-token context, 137 MB).
  Upgrade to `ollama:bge-m3` for higher retrieval quality at the cost of a 1.2 GB download.
- **Best search quality, cloud acceptable** → `openai:text-embedding-3-small`.
- **Air-gapped or full data privacy required** → `local:JinaEmbeddingsV2BaseCode` or
  `local:AllMiniLML6V2Q` (already the default — no external calls are made).
- **Self-hosted TEI, vLLM, or similar** → set `url = "http://your-server:port/v1"` in `[embeddings]`.
