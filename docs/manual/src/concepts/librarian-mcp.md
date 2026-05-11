# librarian-mcp — workspace artifact registry
Codescout's sister MCP server. Indexes markdown artifacts (specs, plans,
runbooks, ADRs, memories, audits, handoffs, roadmaps, user docs) across **every
repo in a workspace**, stores metadata + link graph in SQLite, and exposes MCP
tools for finding, reading, linking, and packaging those artifacts as context.

Where codescout is project-scoped and code-shaped, librarian-mcp is
workspace-scoped and artifact-shaped. Runs as a separate stdio MCP server —
both can be wired into the same agent.

## Installation

Ships as a sibling binary in the same Cargo workspace. Build with:

```bash
cargo build --release -p librarian-mcp
```

The binary lands at `target/release/librarian-mcp`.

## One-time setup

1. **Seed project registry.** librarian-mcp reads a user-maintained TOML
   listing every repo to index. By default: `~/.codescout-registry.toml` (or
   override via `CODESCOUT_REGISTRY` env). Format:

   ```toml
   [[projects]]
   name = "codescout"
   path = "/home/you/work/codescout"

   [[projects]]
   name = "backend"
   path = "/home/you/work/backend"
   ```

2. **Generate workspace config.** Run once:

   ```bash
   ./target/release/librarian-mcp import-codescout
   ```

   Writes `~/.config/librarian/workspace.toml` with the seeded projects plus 9
   default classification rules (spec / plan / memory / roadmap / adr / audit /
   handoff / runbook / doc).

3. **Index.** Populate the catalog:

   ```bash
   ./target/release/librarian-mcp reindex          # incremental
   ./target/release/librarian-mcp reindex --force  # wipe + rebuild
   ```

4. **Wire into Claude Code:**

   ```bash
   claude mcp add librarian-mcp /absolute/path/to/target/release/librarian-mcp
   ```

   Full Claude Code restart to surface the tools.

## Tools

| Tool | Purpose |
|---|---|
| `artifact_find` | Search by filter AST (kind/status/tags/updated_at) with optional semantic query |
| `artifact_get` | Fetch one artifact, optionally with observations + link neighbourhood |
| `artifact_list_by_kind` | Thin wrapper — `{kind, status?}` |
| `artifact_links` | Outgoing / incoming edges, optionally filtered by relation |
| `artifact_graph` | BFS neighbourhood, depth 1–3 |
| `artifact_create` | Write a new markdown file with frontmatter + index it |
| `artifact_update` | Patch frontmatter or body; round-trips through the file |
| `artifact_link` | Add a relation edge; `supersedes` transitions dst.status = "superseded" |
| `artifact_observe` | Append a note to an artifact's observation log |
| `librarian_reindex` | Manual re-scan. `{repo?, force?}` |
| `librarian_context` | Pack a topic or anchor artifact into a markdown context bundle, token-budgeted |

## Filter AST

JSON tree ported from Redis agent-memory-server. Composition `and` / `or` /
`not`. Leaf ops `eq` / `ne` / `in` / `nin` / `gt` / `lt` / `gte` / `lte` /
`contains`. Example:

```json
{"and": [
  {"kind": {"eq": "spec"}},
  {"status": {"in": ["active", "blocked"]}},
  {"tags": {"contains": "embedding"}}
]}
```

`tags` and `owners` are JSON-array columns — `contains` compiles to a
`json_each` membership test, not `LIKE`.

## Semantic search

Optional. Set `LIBRARIAN_EMBED_MODEL` to any model codescout supports (remote
via Ollama / OpenAI-compatible endpoint — codescout 1.0.0+ no longer ships an
in-process backend). On reindex,
each artifact's first chunk is embedded into sqlite-vec's `vec0` virtual table.
`artifact_find` and `librarian_context` accept `semantic: "<natural language>"`
and fall back to SQL `LIKE` when no embedder is configured.

## Classification

Every indexed file passes through two sources of truth:

1. **Frontmatter** (authoritative if present) — `kind`, `status`, `title`,
   `owners`, `tags`, `topic`, `time_scope`.
2. **Rule match** on relative path — compiled glob patterns from
   `workspace.toml` under `[[rule]]`.

When neither identifies a kind, the row lands as `kind = "unknown"` with
`confidence = 0.5`. `librarian_reindex` reports the unknown ids so you can
triage by adding rules or frontmatter.

## Architecture

- `codescout-embed` — shared crate extracted from codescout. Provides the
  `Embedder` trait + local / remote clients + markdown chunker. Both codescout
  and librarian-mcp depend on it.
- `crates/librarian-mcp` — SQLite catalog (artifact / artifact_link /
  artifact_observation / artifact_vec), indexer, filter AST compiler, 11 MCP
  tools, stdio transport via `rmcp`.
- `~/.config/librarian/workspace.toml` — roots + ignore globs + classification
  rules.
- `~/.local/share/librarian/catalog.db` — SQLite database (override with
  `LIBRARIAN_DB`).

## Known limits

- Indexing is on-demand via `librarian_reindex` or CLI. No file watcher.
- One vector per artifact (first chunk only). Chunk-level semantic search is
  not v1.
- `artifact_update`'s body patch replaces the entire body — no diff semantics.
- No central codescout project registry yet; `import-codescout` reads a
  user-maintained TOML.
- Title derivation falls back to the first `# H1` when frontmatter has no
  `title` — files with neither land with `title: null`.

## Related

- Spec: `docs/superpowers/specs/2026-04-19-librarian-mcp-design.md`
- Plan: `docs/superpowers/plans/2026-04-19-librarian-mcp.md`
- Credits: `crates/librarian-mcp/CREDITS.md`
