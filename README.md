# codescout

MCP server giving AI coding agents IDE-grade code intelligence — symbol navigation,
semantic search, persistent memory, **workspace-wide artifact tracking** — optimized for token efficiency.

[![docs](https://img.shields.io/badge/docs-mareurs.github.io%2Fcodescout-blue)](https://mareurs.github.io/codescout/)
[![crates.io](https://img.shields.io/crates/v/codescout.svg)](https://crates.io/crates/codescout)
[![license](https://img.shields.io/crates/l/codescout.svg)](https://github.com/mareurs/codescout/blob/master/LICENSE)

📖 **[Full manual at mareurs.github.io/codescout](https://mareurs.github.io/codescout/)** — installation, agent integrations, every tool, every concept.

Works with Claude Code, GitHub Copilot, Cursor, and any MCP-capable agent.

## What it does

- **Symbol navigation** — `symbols`, `references`, `symbol_at`, `call_graph`, `edit_code`, backed by LSP across 9 languages
- **Semantic search** — find code by concept using a local ONNX model or an external Qdrant + reranker stack
- **Library navigation** — explore dependency source with scoped search and version tracking
- **Artifact tracking** *(new)* — index, query, and link the markdown around your code: specs, plans, ADRs, runbooks, memories. See [Artifacts](#artifacts) below.
- **Multi-project workspaces** — register related projects in `workspace.toml` for cross-project navigation with per-project memory and indexing
- **Token efficiency** — compact by default, details on demand, never dumps full files

## Why not just read files?

| Without codescout | With codescout |
|---|---|
| Agent reads full files to find one function | Navigates by symbol name — zero file reads |
| `grep` returns noise (comments, strings, docs) | `references` returns exact call sites |
| Context burns on navigation overhead | Token-efficient by design |
| State lost between sessions | Persistent memory + artifact catalog across sessions |
| Re-reads same modules from different entry points | Symbol index built once, queried instantly |

## Quick Start

```
cargo build
./target/debug/codescout start --project /path/to/code
```

Register as an MCP server in `~/.claude/settings.json`:

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

> **Onboarding is essential.** Run `onboarding()` once per project on first
> connect — it discovers languages, reads key files, and generates a
> project-specific system prompt and memory files. Without it, the agent
> navigates blind.
>
> **Tip:** Install the [codescout-companion plugin](https://mareurs.github.io/codescout/getting-started/routing-plugin.html) to auto-steer Claude toward codescout tools in every session — including subagents.

→ Full setup, agent configs, and project-toml options: **[mareurs.github.io/codescout](https://mareurs.github.io/codescout/)**

## Artifacts

codescout now embeds **librarian** — a workspace-wide artifact registry that
indexes the markdown around your code (specs, plans, ADRs, runbooks, memories,
audits, handoffs, roadmaps, user docs), stores metadata + a typed link graph
in SQLite, and exposes it to the agent as queryable structured data instead of
opaque file listings.

**Why it matters.** The code is half the project. The other half lives in
markdown files scattered across `docs/`, design notes, and runbooks. Without
indexing them, the agent re-discovers project history every session. With
librarian, "find every active spec touching the embedder" is a single tool call.

**How it works.**
- Every markdown file is classified by frontmatter or path-rule into a *kind*
  (spec / plan / adr / runbook / memory / tracker / doc / ...) and indexed
  into a SQLite catalog at `~/.local/share/librarian/catalog.db`.
- A typed link graph (`supersedes`, `implements`, `references`, ...) connects
  artifacts; walk it transitively with `artifact(action="graph")`.
- A 4-tier scope ladder (`project` → `repo` → `umbrella` → `all`) lets the
  agent widen beyond the current sub-project on demand.
- Optional semantic search via `LIBRARIAN_EMBED_*` turns
  `artifact(action="find", semantic="…")` into vector search.

**How to use it.**

```text
# Find every active spec touching the retrieval stack
artifact(action="find", kind="spec", status="active", semantic="retrieval pipeline")

# Read one, with its outgoing link graph
artifact(action="get", id="abc123", include_links=true)

# Connect a plan to the spec it implements
artifact(action="link", src_id="<plan>", dst_id="<spec>", rel="implements")

# Pack a topic neighbourhood into a single markdown bundle for context
librarian(action="context", topic="hybrid sparse + dense retrieval", max_tokens=4000)
```

**Trackers** — augment any artifact with a persistent prompt + params, and
codescout auto-refreshes it on demand. Friction logs, observation tables,
sprint roadmaps — written once, maintained across sessions.

**Embedded by default.** No separate server to install. Opt out with
`LIBRARIAN_ENABLED=0` for a leaner tool surface.

→ **Full artifact guide:** [librarian embedded](https://mareurs.github.io/codescout/concepts/librarian-embedded.html) · [trackers](https://mareurs.github.io/codescout/concepts/tracker-design.html) · [time-travel](https://mareurs.github.io/codescout/concepts/workspace-state-at.html) · [augmentation templates](https://mareurs.github.io/codescout/concepts/augmentation-render-template.html)

## Retrieval Stack

`semantic_search` runs through an external Docker Compose stack (Qdrant +
dense embedder + SPLADE sparse + cross-encoder rerank). Two profiles: `cpu`
and `gpu`.

```bash
docker compose --profile cpu --env-file .env.cpu up -d
# or --profile gpu --env-file .env.gpu

set -a; source .env.cpu; set +a
cargo run --release --bin sync_project -- . codescout
```

| Profile | Initial sync (~18k chunks) | Sustained throughput |
|---|---:|---:|
| cpu (llama-server, 4 threads) | ~125 min | 2.4 chunks/s |
| gpu (llama-server-cuda, RTX A5000) | ~2.6 min | 117-132 chunks/s |

→ **[Stack setup, model choices, and tuning](https://mareurs.github.io/codescout/concepts/retrieval-stack.html)**

## Agent integrations

| Agent | Guide |
|---|---|
| Claude Code | [mareurs.github.io/codescout/agents/claude-code](https://mareurs.github.io/codescout/agents/claude-code.html) |
| GitHub Copilot | [mareurs.github.io/codescout/agents/copilot](https://mareurs.github.io/codescout/agents/copilot.html) |
| Cursor | [mareurs.github.io/codescout/agents/cursor](https://mareurs.github.io/codescout/agents/cursor.html) |

## Multi-agent infrastructure

codescout's design is informed by research on compound error in multi-agent
systems — failure rates of 41–87% in production pipelines. This drove the
choice of single-session skill-based workflows over agent orchestration chains.
[Read the analysis →](docs/research/multi-agent-context-loss.md)

## Kotlin & Rust LSP multiplexers

Kotlin and Rust LSPs are expensive to boot and allow only one process per
workspace. codescout ships per-language multiplexers — a detached `codescout
mux` process shares one LSP across all codescout instances. Cold-start happens
once; subsequent sessions connect instantly. No configuration needed.

| Metric | Without mux | With mux |
|--------|-------------|----------|
| LSP processes per machine | 1 per session (~2GB each) | 1 shared |
| Cold start on 2nd session | 8–15s | ~0s |

→ [Kotlin LSP multiplexer](https://mareurs.github.io/codescout/concepts/kotlin-lsp-multiplexer.html) · [Rust](https://mareurs.github.io/codescout/concepts/rust-lsp-multiplexer.html)

## Tools

`Symbol navigation` · `File operations` · `Shell` · `Semantic search` · `Memory` · `Library navigation` · `Artifacts` · `Workflow & Config`

Supported languages: Rust, Python, TypeScript/JavaScript, Go, Java, Kotlin, C/C++, C#, Ruby.

→ **[Full tool reference](https://mareurs.github.io/codescout/tools/overview.html)**

## Experimental Features

New features land on `experiments` before reaching `master`. They may change
or be removed without notice.

→ [Browse experimental features](https://github.com/mareurs/codescout/blob/experiments/docs/manual/src/experimental/index.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). PRs from Claude Code are welcome.

## License

[MIT](LICENSE)
