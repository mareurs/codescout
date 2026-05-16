# Librarian (embedded in codescout)

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
default = ["remote-embed", "http", "librarian"]
librarian = []  # module compilation gate (formerly dep:librarian-mcp)
```

```bash
# Dev build — librarian on
cargo build --release

# Production build — librarian compiled out, zero runtime cost
cargo build --release --no-default-features \
  --features remote-embed,http
```

## Runtime override

Even with the feature compiled in, librarian registration is **enabled by
default**. Opt out per session via env var, or per project via `project.toml`.

| Knob | Value | Effect |
|------|-------|--------|
| `LIBRARIAN_ENABLED` env | `0` / `false` / `off` / `no` | Disable for this codescout process |
| `LIBRARIAN_ENABLED` env | `1` / `true` / `on` / `yes` | Force enable (overrides project.toml) |
| `[librarian] enabled = false` in `<project>/.codescout/project.toml` | bool | Per-project disable when env unset |
| (default) | — | **Enabled** |

The env var wins; project.toml is consulted only when the env var is unset.

To opt out globally, set `LIBRARIAN_ENABLED=0` in the codescout MCP server
launch env (e.g. the `env` block of `.mcp.json` or your shell rc).
## What you lose with `librarian` off

- The 15 librarian tools disappear from `tools/list`.
- The librarian instructions block is omitted from the MCP `instructions`
  field, so the LLM gets no hint that artifact tooling exists.
- The on-disk catalog (SQLite at `$XDG_DATA_HOME/librarian/catalog.db`) and
  workspace.toml are untouched — flipping the feature back on resumes where
  the previous session left off.

## Opting out in production

Production users of codescout-as-MCP without a workspace.toml or a
configured catalog can opt out via `LIBRARIAN_ENABLED=0` to avoid the
token overhead of the librarian tool descriptions. The cargo feature can
also be compiled out entirely (`--no-default-features`) for a leaner
production binary.
## Default scope: project (not workspace)

All listing tools default to `scope="project"`, returning only artifacts
under the agent's current sub-project. The current project resolves from
cwd → nearest `.git` ancestor → workspace root from `~/.config/librarian/workspace.toml`.

`librarian_reindex` follows the same default. A force-wipe under
`scope="project"` only deletes rows whose `rel_path` starts with the
current sub-project's subdir — sibling projects under the same workspace
root are preserved.

| Scope | Coverage |
|-------|----------|
| `project` (default) | Current sub-project only |
| `repo` | Whole workspace root (all sub-projects under it) |
| `umbrella` | All members of the declared umbrella for the current project |
| `all` | Workspace-wide |

Read tools surface a `scope` block + `hints` (`more_in_repo`,
`more_in_workspace`) so the LLM can widen on demand. Reindex echoes its
`scope` and resolved `targets` in the response.

## Per-project classifier overrides

Drop a `<project>/.codescout/librarian.toml` to declare classification
rules for that project's paths without touching the global
`~/.config/librarian/workspace.toml`. Schema matches the global file's
`[[rule]]` blocks. Rule precedence is **project > workspace > built-in
defaults**, first-match-wins.

```toml
# <project>/.codescout/librarian.toml
[[rule]]
glob = "codescout/docs/reviews/**/*.md"
kind = "memory"
time_scope = "dated_snapshot"

[[rule]]
glob = "codescout/docs/agents/*.md"
kind = "doc"
```

Built-in defaults already cover common patterns: `CHANGELOG.md`,
`CONTRIBUTING.md`, `docs/ARCHITECTURE.md`, `docs/QUICK-START.md`,
`docs/concepts/**`, `docs/configuration/**`, `docs/experimental/**`,
`docs/issues/**` (tracker), `docs/TODO-*.md` (tracker), `docs/review-*.md`
(memory, dated), `**/prompts/*.md`, `src/**/prompts/*.md`,
`crates/**/prompts/*.md`. The override file is for project-specific
patterns the defaults can't reasonably guess.

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
