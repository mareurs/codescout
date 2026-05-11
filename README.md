# codescout
MCP server giving AI coding agents IDE-grade code intelligence — symbol navigation,
semantic search, persistent memory — optimized for token efficiency.

Works with Claude Code, GitHub Copilot, Cursor, and any MCP-capable agent.

## What it does

- **Symbol navigation** — `symbols`, `references`, `symbol_at`, `call_graph`, `edit_code`, backed by LSP across 9 languages
- **Semantic search** — find code by concept via any OpenAI-compatible embedding endpoint (Ollama, llama.cpp, vLLM, TEI, OpenAI), not grep
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

> **Tip:** Install the [codescout-companion plugin](docs/manual/src/getting-started/companion-plugin.md) to automatically steer Claude toward codescout tools in every session — including subagents.
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

codescout requires an external embedding service for semantic search.
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

Any OpenAI-compatible `/v1/embeddings` endpoint works (Ollama, llama.cpp,
vLLM, TEI, OpenAI). See [Embedding configuration](docs/manual/src/configuration/embeddings.md).

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
