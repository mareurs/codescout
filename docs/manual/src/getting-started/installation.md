# Installation

> **Platform support:** codescout has been tested on **Linux**. macOS and Windows may work but have not been verified. Contributions welcome.

> **This is a Claude Code tool.** codescout is built for [Claude Code](https://code.claude.com/) and currently requires it as the host agent.

## The Easy Way

Clone the repo and let Claude handle the installation. It has access to the full documentation, your system, and the install scripts — it will build the binary, register the MCP server, install LSP servers for your languages, and set up the routing plugin.

```bash
git clone https://github.com/mareurs/codescout.git
cd codescout
claude
# Then ask: "Help me install and set up codescout"
```

## Manual Installation

If you prefer to install manually, follow the steps below.

### Prerequisites

You need a working Rust toolchain. If you do not have one, install it via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify you have Rust 1.75 or later:

```bash
rustc --version
```

You also need [Claude Code](https://code.claude.com/) installed and available as `claude` on your PATH.

## Installing the Binary

Install codescout from crates.io:

```bash
cargo install codescout
```

This builds with the `remote-embed` feature enabled by default, which adds HTTP client support for
talking to an external embedding API (OpenAI-compatible, Ollama, etc.). If you want local CPU-based
embeddings without any external service, see [Feature Flags](#feature-flags) below.

## Registering as an MCP Server

Claude Code discovers MCP servers through its configuration. You can register codescout either
globally (applies to every Claude Code session on your machine) or per-project (applies only when
working in a specific directory tree).

### Global Registration

Run this once to make codescout available in every Claude Code session:

```bash
claude mcp add --global codescout -- codescout start --project .
```

With global registration, codescout starts automatically whenever Claude Code opens. The
`--project .` argument tells it to activate the project you opened Claude Code in.

### Per-Project Registration via `.mcp.json`

For tighter control — useful when different projects need different embedding backends or when you
are sharing configuration with a team — create or edit `.mcp.json` in the project root:

```bash
claude mcp add codescout -- codescout start --project /path/to/your/project
```

This writes an entry to `.mcp.json`. Commit the file so that everyone working on the project gets
the same codescout setup automatically.

A resulting `.mcp.json` entry looks like this:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "codescout",
      "args": ["start", "--project", "/path/to/your/project"]
    }
  }
}
```

Replace `/path/to/your/project` with the absolute path to your repository root.

## Verification

After registering, confirm Claude Code sees the server and all its tools:

```bash
claude mcp list
```

You should see `codescout` listed with 29 tools. If it does not appear, make sure the
`codescout` binary is on your PATH:

```bash
which codescout
codescout --version
```

## Running Onboarding

**This is the most important step.** After registering the MCP server, you must run onboarding once for each project you want to use codescout with. Until you do, codescout has no project context — LSP servers haven't started, no system prompt has been generated, and the agent won't know which tools to use or when.

In a new session in your project directory, ask your agent:

```
Run codescout onboarding
```

The `onboarding` tool will:

1. **Detect your project** — languages, entry points, key files
2. **Start LSP servers** — one per detected language (Rust analyzer, Pyright, ts-server, etc.)
3. **Generate a system prompt** — a project-specific guidance block injected into every future session, covering tool selection rules, entry points, and navigation tips
4. **Write `.codescout/project.toml`** — your project config file, which you can edit to customize embedding backends, ignored paths, and security settings

Onboarding takes 10–30 seconds on first run (LSP server startup). Subsequent sessions reuse the running servers and load instantly.

### When to re-run onboarding

- After adding a new language to a project (new LSP server needed)
- After significantly restructuring your codebase (entry points change)
- After editing `.codescout/project.toml` manually
- If codescout's tool guidance feels stale or wrong

Re-run with `force: true` to rebuild from scratch: ask your agent `"Run codescout onboarding with force: true"`.

> **Note:** If you skip onboarding, tools like `symbols`, `symbols`, and `symbol_at` will return errors — they depend on LSP servers that onboarding starts.

## Feature Flags

> **As of v0.15 the default build is the daemon-free [lite stack](../concepts/lite-stack.md)** —
> in-process `sqlite-vec` plus one remote OpenAI-compatible embedding endpoint.
> `cargo install codescout` and `cargo build --release` give you this: no Docker,
> no Qdrant. The full **[server stack](../concepts/retrieval-stack.md)** (Qdrant +
> sparse SPLADE + cross-encoder reranker) is opt-in — build with
> `--features server-stack`. The vector store is picked at runtime via
> `CODESCOUT_VECTOR_BACKEND` (`sqlite-vec` default | `qdrant`). Lite ranks
> dense-only, which trails the hybrid server config on exact-identifier recall —
> see [the tradeoff](../concepts/lite-stack.md#the-tradeoff). The `local-embed`
> Cargo feature (in-process ONNX) still exists for air-gapped use but is not the
> default and is not the path the team benchmarks against.

The Cargo features below control the codescout binary's compile-time
capabilities. They are independent of whether you run the retrieval stack —
even without `local-embed`, the binary always speaks HTTP to the stack via
`remote-embed`.

| Feature | What it does | When to use it |
|---|---|---|
| `remote-embed` (default) | HTTP client for the dense embedder service (OpenAI-compatible protocol) | Always — required for both stacks and for Ollama / OpenAI / llama.cpp endpoints |
| `server-stack` | Compiles in the Qdrant client + hybrid sparse/rerank query path (the [server stack](../concepts/retrieval-stack.md)) | Only if you run Qdrant; omit for the default daemon-free [lite stack](../concepts/lite-stack.md) |
| `local-embed` | In-process CPU embeddings via fastembed-rs + ONNX Runtime | Air-gapped machines; **requires building from source** and skips the network rerank/sparse pipeline |
| `http` (default) | HTTP/SSE MCP transport (vs stdio-only) | Always — used by `codescout dashboard` and remote MCP clients |
| `librarian` (default) | Embeds the librarian doc/spec/plan indexer (formerly librarian-mcp, now dissolved into `src/librarian/`) | Always — runtime-enabled by default; opt out via `LIBRARIAN_ENABLED=0` |

> **Want free, local embeddings without running docker?** Two options:
>
> 1. **Ollama as the dense endpoint on the default lite stack.** The default
>    build needs no Docker — point it at Ollama: `ollama pull nomic-embed-text`,
>    set `CODESCOUT_EMBEDDER_URL=http://127.0.0.1:11434` and keep
>    `CODESCOUT_VECTOR_BACKEND=sqlite-vec` (the default). No Qdrant, no sparse,
>    no reranker. See [The Lite Stack](../concepts/lite-stack.md). (On the opt-in
>    server stack, Ollama replaces only the dense leg — you still run Qdrant +
>    sparse + reranker.)
> 2. **Build with `local-embed`** for the pure in-process path. Accept the
>    benchmark penalty (no rerank, no sparse fusion) in exchange for zero
>    network dependencies.

### Local Embeddings via fastembed (`local-embed`)

The `local-embed` feature depends on ONNX Runtime as a native system library. Because of
this native dependency, it is **not available via `cargo install codescout`** from crates.io.
To use it you must build from source:

```bash
git clone https://github.com/mareurs/codescout.git
cd codescout
cargo install --path . --no-default-features --features local-embed,http,librarian
```

The first time you build a semantic search index, the local backend model downloads
automatically to `~/.cache/huggingface/hub/`. Subsequent uses are fully offline.

In this mode `semantic_search` falls back to pure dense vector scoring — no
SPLADE sparse leg, no cross-encoder rerank. The retrieval-stack configuration
in this manual does not apply to `local-embed` mode.

### Minimal Install (No Embeddings)

If you only want LSP-backed symbol navigation and git tools and do not need semantic search, you
can build without any embedding feature:

```bash
cargo install codescout --no-default-features --features http,librarian
```

Semantic search tools (`semantic_search`, `index`) will return a clear error if called
without an embedding backend compiled in.
## Next Steps

- [Your First Project](first-project.md) — open a project, run onboarding, and try the basic tools
- [Routing Plugin](routing-plugin.md) — install the plugin that steers Claude toward codescout tools automatically
