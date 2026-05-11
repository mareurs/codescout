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

## Embedding Setup

codescout requires an external OpenAI-compatible embedding endpoint for
semantic search. The repo ships a `docker-compose.yml` that brings up the
recommended configuration (Hugging Face TEI serving `all-MiniLM-L6-v2` +
a reranker):

```bash
# From the codescout repo root:
docker compose up -d
```

Then in `.codescout/project.toml`:

```toml
[embeddings]
model = "all-minilm"
url   = "http://127.0.0.1:48080/v1"
```

For GPU acceleration:

```bash
docker compose -f docker-compose.yml -f docker-compose.gpu.yml up -d
```

Any server speaking the OpenAI `/v1/embeddings` API works — Ollama,
llama.cpp, vLLM, TEI, OpenAI. See [`docs/embedding-setup.md`](../../../../embedding-setup.md)
for troubleshooting and alternative providers, or
[Embedding Backends](../configuration/embedding-backends.md) for the
full per-provider reference.

> **Why no bundled local backend?** codescout 1.0.0 removed the in-process
> fastembed/ONNX backend. The runtime dependency on ONNX Runtime, native-library
> distribution friction (couldn't ship via `cargo install`), and a peak-memory
> footprint that interacted badly with the rest of the server made the trade-off
> not worth it. See `docs/adrs/2026-05-11-remote-only-embedding.md` for the full
> rationale.
## Next Steps

- [Your First Project](first-project.md) — open a project, run onboarding, and try the basic tools
- [Routing Plugin](routing-plugin.md) — install the plugin that steers Claude toward codescout tools automatically
