# code-explorer

Rust MCP server giving LLMs IDE-grade code intelligence.

## The Problem

LLMs waste most of their context window on code navigation. `grep` returns walls of text. `cat` dumps entire files when you need one function. There's no way to ask "who calls this?" or "what changed here last?" — the tools are blind to code structure.

The result: shallow understanding, hallucinated edits, constant human course-correction.

## The Solution

code-explorer is an MCP server that gives your AI coding agent the same navigation tools a human developer uses in an IDE — but optimized for token efficiency.

**Four pillars:**

| Pillar | What it does | Tools |
|---|---|---|
| LSP Navigation | Go-to-definition, find references, rename — via real language servers | 7 tools, 9 languages |
| Semantic Search | Find code by concept, not just text match — via embeddings | 3 tools |
| Git Integration | Blame, history, diffs — context no other tool provides | 3 tools |
| Persistent Memory | Remember project knowledge across sessions | 4 tools |

Plus file operations (6 tools), AST analysis (2 tools), workflow (3 tools), and config (2 tools) — **30 tools total**.

## Installation

code-explorer has two components that work together:

1. **MCP Server** — provides the 30 tools (symbol navigation, semantic search, git, etc.)
2. **Routing Plugin** — ensures Claude always uses the right tool, across all sessions and subagents

**Both are recommended.** The MCP server gives Claude the capability; the plugin ensures
that capability is always used correctly. Without the plugin, Claude will occasionally
fall back to `grep`/`cat`/`read` out of habit — especially in subagents that start with
a blank slate.

### Step 1: Install the MCP server

```bash
cargo install code-explorer
```

Register it globally so it's available in every Claude Code session:

```bash
claude mcp add --global code-explorer -- code-explorer start --project .
```

Or per-project (add to your project's `.mcp.json`):

```bash
claude mcp add code-explorer -- code-explorer start --project /path/to/your/project
```

### Step 2: Install the routing plugin

```bash
claude /plugin install code-explorer-routing@sdd-misc-plugins
```

Or add to your user settings (`~/.claude/settings.json`) for all sessions:

```json
{
  "enabledPlugins": {
    "code-explorer-routing@sdd-misc-plugins": true
  }
}
```

The plugin is available from the [claude-plugins marketplace](https://github.com/mareurs/claude-plugins).

### Step 3: Verify

```bash
claude mcp list
# Should show: code-explorer with 30 tools
```

### How They Interact

```
┌─────────────────────────────────────────────────────┐
│                   Claude Code                        │
│                                                      │
│  ┌─────────────────────────────────────────────┐    │
│  │  code-explorer-routing plugin (hooks)        │    │
│  │                                              │    │
│  │  SessionStart  → inject tool selection guide │    │
│  │  SubagentStart → propagate to all subagents  │    │
│  │  PreToolUse    → redirect grep/cat/read to   │    │
│  │                  code-explorer equivalents    │    │
│  └──────────────────────┬──────────────────────┘    │
│                         │ routes to                   │
│  ┌──────────────────────▼──────────────────────┐    │
│  │  code-explorer MCP server (30 tools)         │    │
│  │                                              │    │
│  │  LSP · Semantic · Git · AST · Memory · ...   │    │
│  └──────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

**Without the plugin:** Claude has access to the tools but may not use them
optimally — it might read whole files instead of using `find_symbol`, or grep
instead of `semantic_search`.

**With the plugin:** Every session and subagent gets automatic guidance on which
tool to use for each situation. The `PreToolUse` hook actively intercepts
suboptimal tool calls and redirects them before they execute.

## Tools

### Symbol Navigation (LSP)

| Tool | Purpose |
|---|---|
| `find_symbol` | Find symbols by name (supports glob patterns) |
| `get_symbols_overview` | Symbol tree for a file, directory, or glob |
| `find_referencing_symbols` | Find all callers/usages across the codebase |
| `replace_symbol_body` | Replace a function/method body by name |
| `insert_before_symbol` | Insert code before a symbol |
| `insert_after_symbol` | Insert code after a symbol |
| `rename_symbol` | Rename across the codebase (LSP-powered) |

### File Operations

| Tool | Purpose |
|---|---|
| `read_file` | Read file content (with optional line ranges) |
| `list_dir` | Directory listing (shallow by default) |
| `search_for_pattern` | Regex search across project files |
| `find_file` | Find files by glob pattern |
| `create_text_file` | Create or overwrite a file |
| `replace_content` | Find-and-replace text in a file |

### Git

| Tool | Purpose |
|---|---|
| `git_blame` | Line-by-line authorship with commit info |
| `git_log` | Commit history (filterable by path) |
| `git_diff` | Uncommitted changes or diff against a commit |

### Semantic Search

| Tool | Purpose |
|---|---|
| `semantic_search` | Find code by natural language description |
| `index_project` | Build/rebuild the embedding index |
| `index_status` | Check index health and statistics |

### AST Analysis (tree-sitter)

| Tool | Purpose |
|---|---|
| `list_functions` | Quick function signatures (offline, instant) |
| `extract_docstrings` | Extract doc comments (offline, instant) |

### Memory

| Tool | Purpose |
|---|---|
| `write_memory` | Store project knowledge |
| `read_memory` | Retrieve stored knowledge |
| `list_memories` | List all memory topics |
| `delete_memory` | Remove a memory topic |

### Workflow

| Tool | Purpose |
|---|---|
| `onboarding` | First-time project discovery |
| `check_onboarding_performed` | Check if onboarding is done |
| `execute_shell_command` | Run shell commands in project root |

### Config

| Tool | Purpose |
|---|---|
| `activate_project` | Switch active project |
| `get_current_config` | Show project configuration |

## How It Works

### Progressive Disclosure

Every tool defaults to compact output — names and locations, not full source code. Request details with `detail_level: "full"` only when you need them. This keeps the context window useful throughout long sessions.

- **Exploring mode** (default): compact summaries, capped at 200 items
- **Focused mode**: full detail with pagination via `offset`/`limit`
- **Overflow hints**: "showing 47 of 312 — narrow with a file path" instead of silent truncation

### Architecture

```
MCP Layer (rmcp) → Tool trait → dispatch
    ↓
Agent (project state, config, memory)
    ↓
┌──────────┬──────────┬──────────┬──────────┐
│ LSP      │ AST      │ Git      │ Embedding│
│ (9 langs)│ (t-sitter)│ (git2)  │ (SQLite) │
└──────────┴──────────┴──────────┴──────────┘
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## Configuration

code-explorer stores per-project config in `.code-explorer/project.toml`:

```toml
[embeddings]
model = "ollama:nomic-embed-text"   # or "openai:text-embedding-3-small"
chunk_size = 1500
chunk_overlap = 200
```

### Embedding backends

- **Remote** (default feature): Any OpenAI-compatible API — Ollama, OpenAI, custom endpoints
- **Local**: CPU-based via fastembed-rs — `cargo install code-explorer --features local-embed`

## Supported Languages

### LSP (full navigation)
Rust, Python, TypeScript/JavaScript, Go, Java, Kotlin, C/C++, C#, Ruby

### Tree-sitter (AST analysis)
Rust, Python, TypeScript, Go, Java, Kotlin

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to get started. PRs from Claude Code are welcome!

## License

[MIT](LICENSE)
