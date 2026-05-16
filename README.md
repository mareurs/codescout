# codescout
MCP server giving AI coding agents IDE-grade code intelligence — symbol navigation,
semantic search, persistent memory — optimized for token efficiency.

Works with Claude Code, GitHub Copilot, Cursor, and any MCP-capable agent.

## What it does

- **Symbol navigation** — `symbols`, `references`, `symbol_at`, `call_graph`, `edit_code`, backed by LSP across 9 languages
- **Semantic search** — find code by concept using a bundled ONNX embedding model (22 MB, zero setup), not grep
- **Library navigation** — explore dependency source code with scoped search, version tracking, and auto-discovery
- **Multi-project workspaces** — register related projects in `workspace.toml` for cross-project navigation with per-project memory and indexing
- **Token efficiency** — compact by default, details on demand, never dumps full files

## Why not just read files?

| Without codescout | With codescout |
|---|---|
| Agent reads full files to find one function | Navigates by symbol name — zero file reads |
| `grep` returns noise (comments, strings, docs) | `references` returns exact call sites |
| Context burns on navigation overhead | Token-efficient by design — compact by default |
| State lost between sessions | Persistent memory across sessions |
| Re-reads same modules from different entry points | Symbol index built once, queried instantly |

## Quick Start

```
cargo build
./target/debug/codescout start --project /path/to/code
```

Add codescout as an MCP server in `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "codescout",
      "args": ["start", "--project", "."]
    }
  }
}
```

Then use in Claude Code — it will route all file/symbol/search operations through codescout's tools.

> **Onboarding is essential.** Before starting work on a new project, run
> `onboarding()` — it discovers languages, reads key project files, and
> generates a project-specific system prompt and memory files. Without it,
> the agent has no project context and will navigate the codebase blind.
> See the [Claude Code integration guide](docs/agents/claude-code.md) for details.

> **Tip:** Install the [codescout-companion plugin](docs/manual/src/getting-started/routing-plugin.md) to automatically steer Claude toward codescout tools in every session — including subagents.
## Retrieval Stack

codescout uses an external Docker Compose stack (Qdrant + llama-server + TEI) for
semantic embedding and hybrid retrieval. **Required for `semantic_search`.**

Two profiles: `cpu` (laptop / no-GPU dev) and `gpu` (single CUDA card).

```bash
# 1. download the dense embedding model (~90MB, once)
mkdir -p ./models
curl -L -o ./models/CodeRankEmbed-Q4_K_M.gguf \
  https://huggingface.co/brandtcormorant/CodeRankEmbed-Q4_K_M-GGUF/resolve/main/coderankembed-q4_k_m.gguf

# 2. start the stack — pick one profile
docker compose --profile cpu --env-file .env.cpu up -d   # ~3GB RAM, no GPU
# OR
docker compose --profile gpu --env-file .env.gpu up -d   # ~6GB RAM, 1.5GB VRAM

# 3. wait for sparse + rerank to warm up (~30-60s first run, downloads from HF)
docker compose ps

# 4. source env, build the per-project index
set -a; source .env.cpu; set +a    # or .env.gpu
cargo run --release --bin sync_project -- . codescout
```

| Service | Profile | Image | Default URL |
|---|---|---|---|
| qdrant | both | `qdrant/qdrant:v1.17.0` | http://127.0.0.1:6333 (HTTP), :6334 (gRPC) |
| dense (CodeRankEmbed-Q4) | cpu | `ghcr.io/ggml-org/llama.cpp:server` | http://127.0.0.1:48081 |
| dense (CodeRankEmbed-Q4) | gpu | `ghcr.io/ggml-org/llama.cpp:server-cuda` | http://127.0.0.1:48081 |
| sparse (Splade_PP_en_v1) | both | `ghcr.io/huggingface/text-embeddings-inference` | http://127.0.0.1:48084 |
| rerank (bge-reranker-base) | cpu | `text-embeddings-inference:cpu-1.6` | http://127.0.0.1:48083 |
| rerank (bge-reranker-v2-m3) | gpu | `text-embeddings-inference:86-1.8` | http://127.0.0.1:48083 |

CodeRankEmbed is asymmetric — `CODESCOUT_QUERY_PREFIX` in `.env.{cpu,gpu}` is required.
Empirical scores on this stack: 30/60 on the legacy-natural bench (see
[`docs/trackers/retrieval-benchmark.md`](docs/trackers/retrieval-benchmark.md)).

### Indexing speed

Measured against this codebase (~18k chunks, dense embedder is the only
meaningful bottleneck; sparse/rerank are not used during sync):

| Profile | Single-chunk p50 | Sustained throughput | Initial sync (~18k chunks) |
|---|---:|---:|---:|
| cpu (llama-server, 4 threads) | 600ms | 2.4 chunks/s | **~125 min** |
| gpu (llama-server-cuda, RTX A5000) | 7.6ms | 117-132 chunks/s | **~2.6 min** |

The ~50× gap matches typical CPU↔GPU ratios for a quantized 137M embedder.
Incremental syncs after the initial index are fine on both profiles — only
changed chunks re-embed. If you're on CPU and your project is larger than
~2k chunks, expect to leave the first sync running.

**Stop the stack:**
```bash
docker compose --profile cpu down   # or --profile gpu
```
## Agent integrations

| Agent | Guide |
|---|---|
| Claude Code | [docs/agents/claude-code.md](docs/agents/claude-code.md) |
| GitHub Copilot | [docs/agents/copilot.md](docs/agents/copilot.md) |
| Cursor | [docs/agents/cursor.md](docs/agents/cursor.md) |

## Multi-agent infrastructure

> codescout's design is informed by research on compound error in multi-agent systems — research and empirical evidence confirm failure rates of 41–87% in production pipelines. This finding drove the choice of single-session skill-based workflows over agent orchestration chains. [Read the analysis →](docs/research/multi-agent-context-loss.md)


## Kotlin

codescout has first-class Kotlin support built around the reality that Kotlin
projects are expensive to boot and JetBrains' kotlin-lsp allows only one LSP
process per workspace.

- **LSP multiplexer** — a detached `codescout mux` process shares a single
  kotlin-lsp JVM across all codescout instances. No configuration needed.
  Cold-start (8–15s JVM boot) happens once; subsequent sessions connect
  instantly.
- **Concurrent instance safety** — each instance gets an isolated system path
  to prevent IntelliJ platform lock contention, with a circuit-breaker that
  fails fast instead of timing out.
- **Gradle isolation** — per-instance `GRADLE_USER_HOME` eliminates daemon
  lock contention between parallel sessions.

| Metric | Without mux | With mux |
|--------|-------------|----------|
| kotlin-lsp JVMs per machine | 1 per session (~2GB each) | 1 shared (~2GB total) |
| Cold start on 2nd session | 8–15s | ~0s (mux already warm) |
| Typical LSP response | 120s+ timeout | 30–270ms |

→ [Kotlin LSP Multiplexer docs](docs/manual/src/concepts/kotlin-lsp-multiplexer.md)

## Tools (20)

`Symbol navigation (5)` · `File operations (7)` · `Shell (1)` · `Semantic search (2)` · `Memory (1)` · `Library navigation (1)` · `Workflow & Config (3)`

Supported languages: Rust, Python, TypeScript/JavaScript, Go, Java, Kotlin, C/C++, C#, Ruby.

→ [Tool reference](docs/manual/src/tools/overview.md)
## Semantic Search & Embeddings

codescout bundles **all-MiniLM-L6-v2** (quantized, 22 MB) as its default embedding model.
It runs locally via ONNX — no external server, no API key, no GPU needed. On first
`index(action: build)`, the model is downloaded once to `~/.cache/huggingface/hub/`.

For users with Ollama or a GPU, codescout also supports external embedding servers
(Ollama, OpenAI, llama.cpp, vLLM, TEI) via the standard `/v1/embeddings` API.

→ [Embedding configuration](docs/manual/src/configuration/embeddings.md)
→ [Model comparison & benchmark](docs/manual/src/configuration/embedding-model-comparison.md)

## Experimental Features

New features land on the `experiments` branch before reaching `master`.
They may change or be removed without notice, and may not be in your installed release yet.

→ [Browse experimental features](https://github.com/mareurs/codescout/blob/experiments/docs/manual/src/experimental/index.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to get started. PRs from Claude Code are welcome!

## Features

- Multi-project workspace support with per-project LSP, memory, and semantic indexing
- Library navigation with per-library embedding databases and version staleness hints
- LSP idle TTL — idle language servers are shut down automatically (Kotlin: 2h, others: 30min) and restarted transparently on next query
- Persistent memory across sessions with semantic recall
- Output buffers (`@cmd_*`, `@file_*`) for token-efficient large output handling
- Progressive disclosure — compact by default, full detail on demand

## License

[MIT](LICENSE)
