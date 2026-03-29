# Claude Code

## Setup

Prerequisites: Rust toolchain

### Using `cargo install` (recommended)

Install the binary:

```bash
cargo install codescout
# lands at ~/.cargo/bin/codescout
```

For this route, we can register as a user-level MCP server via the CLI:

```bash
claude mcp add --scope user codescout ~/.cargo/bin/codescout -- start
```

This writes the entry into `~/.claude.json` (user-scoped).


For a project-scoped alternative, place a `.mcp.json` file at the project root:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "/absolute/path/to/target/release/codescout",
      "args": ["start", "--project", "/absolute/path/to/your/project"],
      "type": "stdio"
    }
  }
}
```

Claude Code detects `.mcp.json` automatically and prompts for approval on first use. 

### Using a local build

If you're working from source or want to pin to a specific local build:

```bash
cargo build --release
# binary at target/release/codescout
```

If you opt for the local build, manually add to either json based on desired scope. Use absolute paths — relative paths are not reliable here.

## Workflow Skills

Claude Code handles workflow skills differently from Copilot/Cursor — skills are loaded via the Superpowers plugin system, not manually installed files. No manual skill file installation is needed; skills activate automatically once the companion plugin is set up. See [Superpowers workflow](../manual/src/concepts/superpowers.md) for details.

## Routing Plugin (codescout-companion)

The routing plugin is a Claude Code plugin that **enforces** codescout tool use via
`PreToolUse` hooks. Without it, the agent may fall back to native `Read`, `Grep`, and
`Glob` tools — which work but bypass codescout's token-efficient symbol navigation.

**What it blocks:**
- `Read` on source files (`.rs`, `.ts`, `.py`, etc.) → redirects to `list_symbols` / `find_symbol`
- `Grep` / `Glob` on source files → redirects to `search_pattern` / `find_file`
- `Bash` for shell commands → redirects to `run_command`

**What it allows:**
- `Read` on non-source files (markdown, TOML, JSON, config)
- All codescout MCP tools pass through unrestricted

Install via:

```
claude plugin install codescout-companion
```

Or follow the [Routing Plugin guide](../manual/src/getting-started/routing-plugin.md)
for manual setup.

**Debugging:** If the plugin blocks a legitimate operation, create
`.claude/code-explorer-routing.json` with `{"block_reads": false}` to temporarily
disable blocking.

## Verify

Restart Claude Code, then run `/mcp` — confirm `codescout` appears as connected. Then ask: "What symbols are in src/main.rs?" — Claude should call `mcp__codescout__list_symbols`, not read the file.

## Multi-Project Workspaces

codescout supports multi-project workspaces. Register projects in
`.codescout/workspace.toml`:

```toml
[[project]]
id = "backend"
root = "services/backend"

[[project]]
id = "frontend"
root = "apps/frontend"
```

After onboarding, use the `project` parameter to scope tool calls:

```
find_symbol("UserService", project: "backend")
memory(action: "read", project: "frontend", topic: "architecture")
```

See [Multi-Project Workspaces](../manual/src/concepts/multi-project-workspace.md).

## Day-to-Day Workflow

codescout injects tool guidance automatically into every session via the MCP system prompt. For the full disciplined development workflow, see:

- [Superpowers workflow](../manual/src/concepts/superpowers.md)
- [Tool Reference](../manual/src/tools/overview.md)
- [Progressive Disclosure](../manual/src/concepts/progressive-disclosure.md)

## Tips

**Buffer refs** — When `read_file` or `run_command` returns a `@file_*` or `@cmd_*`
handle, the content is stored server-side. Query it with
`run_command("grep pattern @cmd_xxxx")` or read sub-ranges with
`read_file("@file_xxxx", start_line=1, end_line=100)`.

**Semantic search for exploration** — When entering an unfamiliar part of the codebase,
start with `semantic_search("how does X work")` rather than reading files. It returns
ranked code chunks by relevance.

**Memory for cross-session context** — Use `memory(action: "remember", content: "...")`
to store decisions, patterns, or gotchas. Use `memory(action: "recall", query: "...")`
to retrieve them by meaning in future sessions.

**Library navigation** — When `goto_definition` resolves to a dependency, codescout
auto-registers the library. Use `semantic_search(scope: "lib:tokio")` to search
within it.
