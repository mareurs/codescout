# librarian-mcp — Design Spec

**Date:** 2026-04-19
**Status:** Draft
**Owner:** marius

## Problem

A multi-repo workspace (`~/work/mirela/` — backend-kotlin, eduplanner-{ui,mobile,site}, deployment, research) accumulates typed documents that don't share a home: specs, plans, trackers, runbooks, ADRs, memories, audits, handoffs, roadmaps, user docs. Current state varies sharply by repo — backend-kotlin has a rich lifecycle (roadmap + specs + plans + trackers + codescout memories), eduplanner-mobile has a separate `memory/` tree, deployment has runbooks, eduplanner-site is mostly product content. The pattern that actually emerges across the workspace is not "folders per repo" but **artifact types surfacing independently** with no registry, no cross-repo linking, no lifecycle discipline, and no way to query across them.

codescout solves single-project code intelligence but is explicitly project-scoped. Cross-repo artifact queries ("show active trackers in all repos", "find stale plans with no linked spec", "show memories related to school switching") fall outside its boundary.

## Goals

1. Provide a workspace-wide **artifact registry** with typed documents, metadata, and a link graph.
2. Expose registry operations as **MCP tools** an agent can call — find, get, link, update status, archive.
3. **Round-trip writes through files** so artifacts stay reviewable in git and survive DB loss.
4. Let agents **act on rot** (archive superseded plans, link tracker→spec→plan, mark drafts active) without leaving the MCP surface.
5. Work with any MCP client, not just Claude Code. Tools-only surface — no reliance on MCP resources or prompts (GitHub Copilot cloud agent constraint).

## Non-goals (v1)

- File watcher / real-time freshness.
- LLM-based classification.
- HTTP / SSE transport.
- Multi-workspace catalog.
- Authentication.
- Rendering / export of artifacts into repos from DB.

## Prior art reviewed

| Project | Verdict |
|---|---|
| MCP reference memory server | Toy (single JSONL, no indexing). Copy the conceptual entity/relation/observation model. Don't copy the storage. |
| Redis agent-memory-server (Apache-2.0) | Serious prior art but Redis+FastAPI+Docket-coupled. Build fresh, but lift `filters.py` AST, `MemoryRecord` field layout, working-vs-long-term split, topic/entity extraction strategy pattern, and `memory_prompt` packed-context tool. |
| Memori MCP | 130-line SaaS proxy. Don't model after. |
| Ragie MCP | Thin retrieval wrapper with ambiguous filter schema. Avoid that schema shape. |

**Constraint:** GitHub Copilot cloud agent supports only MCP **tools**, not resources or prompts. Every primary operation must be a tool. ([source](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/mcp-and-cloud-agent))

## Architecture

Two processes, one shared crate.

```
┌──────────────────────────────────────────────┐
│ librarian-mcp (new crate, stdio MCP server)  │
│   - workspace indexer                        │
│   - SQLite catalog (metadata + links + vec)  │
│   - frontmatter read/write (round-trip)      │
│   - MCP tools                                │
└──────────────────────────────────────────────┘
         │                           │
         ▼                           ▼
  codescout-embed (new crate)  workspace files
   (model, tokenizer, chunker)  (markdown + frontmatter)
         ▲
         │
┌─────────┴────────────────────────────────────┐
│ codescout (existing, refactored to consume   │
│   codescout-embed instead of in-tree module) │
└──────────────────────────────────────────────┘
```

**Boundaries:**

- `librarian-mcp` is a new stdio MCP server. Agents connect to it in parallel with codescout — two servers, no IPC between them.
- `codescout-embed` is the only shared code. Extracted from codescout's existing `src/embed/` into its own workspace crate. Both binaries depend on it.
- No live coupling to codescout project state. `librarian-mcp import-codescout` is a one-shot CLI command that seeds workspace config from codescout's project registry.
- SQLite DB at `~/.local/share/librarian/catalog.db` (single DB, workspace-wide).

**Metadata authority split:**

- **Files own intrinsic metadata** via YAML frontmatter: `id`, `kind`, `status`, `title`, `owners`, `tags`, `topic`, `time_scope`. Reviewable in git; survives DB loss.
- **DB owns relational + derived metadata**: link edges (`supersedes`, `derived_from`, `implements`, `tracks`, `related_to`), embeddings, classifier confidence, observations, last-seen timestamps.
- Writes always round-trip through the file. DB is rebuildable from files (losing DB = full reindex).

## Data model

### SQLite schema

```sql
CREATE TABLE artifact (
  id            TEXT PRIMARY KEY,    -- sha256(repo + rel_path), stable across renames via frontmatter id
  repo          TEXT NOT NULL,
  rel_path      TEXT NOT NULL,
  kind          TEXT NOT NULL,       -- spec|plan|tracker|runbook|adr|memory|audit|handoff|roadmap|doc|unknown
  status        TEXT NOT NULL,       -- draft|active|blocked|done|archived|superseded|unknown
  title         TEXT,
  owners        TEXT,                -- JSON array
  tags          TEXT,                -- JSON array
  topic         TEXT,
  time_scope    TEXT,                -- timeless|dated_snapshot|operational
  source        TEXT,                -- repo|codescout_memory|conversation|generated
  created_at    INTEGER NOT NULL,    -- unix ms
  updated_at    INTEGER NOT NULL,
  file_mtime    INTEGER NOT NULL,    -- drift detection
  file_sha256   TEXT NOT NULL,
  confidence    REAL,                -- 1.0 if from frontmatter, <1 if rule-inferred
  UNIQUE(repo, rel_path)
);

CREATE TABLE artifact_link (           -- typed directed edge
  src_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  dst_id        TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  rel           TEXT NOT NULL,         -- implements|tracks|supersedes|derived_from|related_to
  created_at    INTEGER NOT NULL,
  PRIMARY KEY (src_id, dst_id, rel)
);

CREATE TABLE artifact_observation (    -- atomic fact attached to artifact
  id            INTEGER PRIMARY KEY,
  artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  text          TEXT NOT NULL,
  source        TEXT,                  -- agent|human|classifier
  created_at    INTEGER NOT NULL
);

CREATE VIRTUAL TABLE artifact_vec USING vec0(
  id            TEXT PRIMARY KEY,
  embedding     FLOAT[768]             -- from codescout-embed
);

CREATE INDEX idx_artifact_kind_status ON artifact(kind, status);
CREATE INDEX idx_artifact_repo ON artifact(repo);
CREATE INDEX idx_link_dst ON artifact_link(dst_id, rel);
```

### Frontmatter schema (file-authoritative)

```yaml
---
id: <auto-generated on first index, stable afterward>
kind: spec
status: active
title: Metadata-enriched chunks
owners: [marius]
tags: [embedding, chunker]
topic: embedding
time_scope: dated_snapshot
---
```

Missing fields are derived from classification rules on first index. `id` is written back on first successful classification only — never clobbered on in-flight drafts that already have an id.

### Kind taxonomy (v1)

`spec`, `plan`, `tracker`, `runbook`, `adr`, `memory`, `audit`, `handoff`, `roadmap`, `doc`, `unknown`.

### Status lifecycle

`unknown → draft → active → (blocked ↔ active) → done → archived`. `unknown` is the bootstrap state for artifacts without frontmatter status and no matching rule — an agent promotes them via `artifact_update`. `superseded` is a terminal status set automatically when an `artifact_link(rel=supersedes, src=X, dst=Y)` is created — Y becomes `superseded`, X stays in its current status.

## Tool API

11 tools, all read/write flow through MCP. Tools-only — no resources or prompts.

### Read (6)

| Tool | Args | Returns |
|---|---|---|
| `artifact_find` | `filter` (AST), `semantic?` (query string), `limit`, `offset` | `[{id, kind, status, title, repo, rel_path, score?}]` |
| `artifact_get` | `id`, `include_observations?`, `include_links?` | full record |
| `artifact_list_by_kind` | `kind`, `status?`, `limit`, `offset` | convenience shortcut for the common filter |
| `artifact_links` | `id`, `rel?`, `direction` (out/in/both) | edges with neighbor summaries |
| `artifact_graph` | `id`, `depth` (default 1, max 3), `rels?` | N-hop neighborhood graph |
| `librarian_context` | `topic?` or `anchor_id?`, `max_tokens?` | packed markdown bundle of top-K relevant artifacts (titles + short summaries + links) — for agent task bootstrap |

### Write (4)

| Tool | Args | Behavior |
|---|---|---|
| `artifact_create` | `repo`, `rel_path`, `kind`, `title`, `body`, `owners?`, `tags?` | writes file with frontmatter + indexes; **fails if path exists** |
| `artifact_update` | `id`, `patch: {status?, title?, owners?, tags?, topic?, body?}` | round-trips through file; **requires id** |
| `artifact_link` | `src_id`, `dst_id`, `rel` | creates typed edge; auto-sets `superseded` status when `rel=supersedes` |
| `artifact_observe` | `id`, `text`, `source?` | appends atomic observation — for agent-captured decisions/outcomes |

### Admin (1)

| Tool | Args | Returns |
|---|---|---|
| `librarian_reindex` | `repo?`, `force?` | `{added, updated, removed, unknown_kind, unknown_ids}` |

### Explicitly not tools

- `artifact_archive` / `artifact_supersede` — composed of `artifact_update(status=archived)` + `artifact_link(rel=supersedes)`. Fewer primitives, no redundancy.
- `artifact_delete` — deletion is a file op; removing the file triggers reindex cleanup. Exposing a delete tool invites accidents.
- `classify_unknowns` — deferred to night-job tracker (roadmap).

### Filter AST

Ported from Redis `agent-memory-server/filters.py` (Apache-2.0, attribution in `CREDITS.md`). Shape:

```json
{
  "and": [
    {"kind": {"eq": "spec"}},
    {"status": {"in": ["active", "blocked"]}},
    {"tags": {"contains": "embedding"}},
    {"updated_at": {"gt": 1700000000}}
  ]
}
```

- Composition nodes: `and`, `or`, `not`.
- Leaf ops: `eq`, `ne`, `in`, `nin`, `gt`, `lt`, `gte`, `lte`, `contains`.
- Recursive.
- Ported as a Rust enum (`FilterNode::And(Vec<FilterNode>)`, etc.) with a `to_sql()` compiler that emits parameterized SQL fragments.

## Classification

### Rules (v1 — pure path/filename, no LLM)

Configured in `~/.config/librarian/workspace.toml`:

```toml
[[rule]]
glob = "**/docs/superpowers/specs/*.md"
kind = "spec"
status = "active"

[[rule]]
glob = "**/docs/superpowers/plans/*.md"
kind = "plan"

[[rule]]
glob = "**/docs/research/*.md"
kind = "memory"
time_scope = "dated_snapshot"

[[rule]]
glob = "**/docs/adr/*.md"
kind = "adr"

[[rule]]
glob = "**/docs/trackers/*.md"
kind = "tracker"

[[rule]]
glob = "**/runbooks/**/*.md"
kind = "runbook"
time_scope = "operational"

[[rule]]
glob = "**/docs/audits/*.md"
kind = "audit"

[[rule]]
glob = "**/docs/handoffs/*.md"
kind = "handoff"

[[rule]]
glob = "**/ROADMAP.md"
kind = "roadmap"
```

Ordered evaluation; first match wins. **Frontmatter always wins over rules.** No rule match and no frontmatter → `kind=unknown, confidence<1`.

### Unknown-kind workflow

1. Agent calls `librarian_reindex`. Response includes `{unknown_count, unknown_ids}`.
2. Agent reads each file's first ~40 lines via codescout.
3. Agent decides `kind` per file, calls `artifact_update(id, patch={kind: ...})`.
4. Night-job automation (roadmap) would later eliminate steps 2–3.

## Discovery

**v1: on-demand, not watch.**

Reindex triggers:
- Explicit `librarian_reindex` tool call.
- Server startup (incremental — walk + compare `file_mtime`/`file_sha256`).
- Targeted single-file reindex after every `artifact_create` / `artifact_update`.

No file watcher in v1. Avoids cross-platform `notify` complexity until usage proves the need.

## Deployment

### Crate layout (inside the existing codescout workspace)

```
codescout/
├── Cargo.toml                      # [workspace] members add the two below
├── crates/
│   ├── codescout-embed/            # extracted from src/embed/
│   │   ├── src/
│   │   │   ├── lib.rs              # Embedder, Tokenizer, Chunker
│   │   │   └── ...
│   │   └── Cargo.toml
│   └── librarian-mcp/              # new binary crate
│       ├── src/
│       │   ├── main.rs             # stdio MCP transport
│       │   ├── catalog/            # SQLite schema + migrations
│       │   ├── filter.rs           # FilterNode AST + to-SQL compiler
│       │   ├── frontmatter.rs      # parse + round-trip YAML frontmatter
│       │   ├── classify.rs         # path-rule classifier
│       │   ├── indexer.rs          # walk + reindex
│       │   ├── tools/              # one file per MCP tool
│       │   └── server.rs           # tool registry + dispatch
│       └── Cargo.toml
└── src/                            # existing codescout binary
    └── embed/ → replaced by `use codescout_embed::*`
```

`codescout-embed` exports the minimal surface: `Embedder`, `EmbedConfig`, `chunk_markdown()`, `embed_texts()`. codescout's existing callsites update to the new path with no behavior change.

### Binary + transport

`librarian-mcp` is a stdio MCP server.

```json
{"mcpServers": {
  "codescout": {"command": "codescout", "args": ["start"]},
  "librarian": {"command": "librarian-mcp"}
}}
```

CLI subcommands (non-MCP):
- `librarian-mcp import-codescout` — one-shot seed of workspace roots from codescout's project registry.
- `librarian-mcp reindex [--repo X]` — manual reindex without MCP.

### Config + data locations

| Path | Purpose |
|---|---|
| `~/.config/librarian/workspace.toml` | roots, ignore globs, classification rules |
| `~/.local/share/librarian/catalog.db` | SQLite catalog |
| `~/.cache/librarian/embed/` | shared with codescout-embed |

### Workspace scope

Explicit config (`workspace.toml` lists roots). No auto-discovery. No marker files. `import-codescout` seeds from codescout's known projects as a one-shot, not a live binding.

## Testing

- **Unit:** filter AST → SQL compiler (round-trip every op), frontmatter parser (malformed YAML, missing / trailing `---`, CRLF), classification rules (glob precedence, frontmatter-wins).
- **Integration:** fixture workspace in `crates/librarian-mcp/tests/fixtures/workspace/` with two fake repos containing specs/plans/memories. Tests: cold index, reindex-after-edit, reindex-after-delete, round-trip update, link graph queries, semantic search on fixture corpus.
- **MCP-level:** spawn binary as subprocess, send JSON-RPC, assert tool responses.
- **Three-query stale test** (project convention): query → mutate file on disk → query (assert stale) → reindex → query (assert fresh). Validates reindex invalidation works.

## Roadmap (explicitly out of v1)

1. **Night-job LLM augmentation.** A5000 + qwen3.5-35 (or similar local model) running scheduled classification + topic/entity extraction + summary generation for `unknown` artifacts. Replaces the on-demand agent classification workflow.
2. **File watcher.** `notify`-crate-based incremental reindex for sub-second freshness.
3. **HTTP/SSE transport.** For remote agents.
4. **Cross-workspace federation.** Multiple workspace.toml files, multiple catalogs, federated `artifact_find`.

## Attribution

- Filter AST shape ported from Redis `agent-memory-server` — Apache-2.0.
- Artifact row schema inspired by Redis `MemoryRecord` — Apache-2.0.
- Entity/Relation/Observation conceptual model from MCP reference memory server.

Credits recorded in `crates/librarian-mcp/CREDITS.md` at implementation time.
