# Librarian (embedded in codescout)

> ⚠ Experimental — may change without notice.

`librarian-mcp` is no longer a standalone MCP server. It is embedded inside
the codescout binary as an opt-in subsystem behind the `librarian` cargo
feature, which is on by default in dev builds and off in `--no-default-features`
production builds.

When active, the 15 librarian tools (`artifact_*`, `librarian_*`,
`workspace_state_at`) are advertised alongside codescout's core toolset and
the librarian server instructions block is appended to codescout's MCP
`instructions` field.

## Build-time control

```toml
# Cargo.toml
[features]
default = ["remote-embed", "local-embed", "dashboard", "http", "librarian"]
librarian = ["dep:librarian-mcp"]
```

```bash
# Dev build — librarian on
cargo build --release

# Production build — librarian compiled out, zero runtime cost
cargo build --release --no-default-features \
  --features remote-embed,local-embed,dashboard,http
```

## Runtime override

Even with the feature compiled in, librarian registration can be disabled
per session or per project without rebuilding.

| Knob | Value | Effect |
|------|-------|--------|
| `LIBRARIAN_ENABLED` env | `0` / `false` / `off` / `no` | Disable for this codescout process |
| `LIBRARIAN_ENABLED` env | `1` / `true` / `on` / `yes` | Force enable (overrides project.toml) |
| `[librarian] enabled = false` in `<project>/.codescout/project.toml` | bool | Per-project disable when env unset |
| (default) | — | Enabled when feature compiled in |

The env var wins; project.toml is consulted only when the env var is unset.

## What you lose with `librarian` off

- The 15 librarian tools disappear from `tools/list`.
- The librarian instructions block is omitted from the MCP `instructions`
  field, so the LLM gets no hint that artifact tooling exists.
- The on-disk catalog (SQLite at `$XDG_DATA_HOME/librarian/catalog.db`) and
  workspace.toml are untouched — flipping the feature back on resumes where
  the previous session left off.

## Why this is opt-in for production

Production users of codescout-as-MCP rarely have a workspace.toml or a
configured catalog, and the library lookups + tool descriptions are token
overhead they don't need. Keeping the cargo feature off by default in
publish builds keeps the production binary lean.

## Migration from standalone librarian-mcp

Earlier sessions ran `librarian-mcp` as a separate stdio MCP server. That
binary no longer exists — `crates/librarian-mcp` is now lib-only. The
`codescout-companion` plugin's session-start hook no longer injects a
separate librarian companion-hint block; the librarian instructions are
served through codescout's own `instructions` field.

`~/.claude/.claude.json` should have only one MCP server entry (`codescout`)
with optional `LIBRARIAN_EMBED_*` envs:

```json
{
  "mcpServers": {
    "codescout": {
      "type": "stdio",
      "command": "/abs/path/to/codescout",
      "args": ["start", "--debug"],
      "env": {
        "LIBRARIAN_EMBED_MODEL": "CodeRankEmbed",
        "LIBRARIAN_EMBED_URL": "http://localhost:43300/v1"
      }
    }
  }
}
```
