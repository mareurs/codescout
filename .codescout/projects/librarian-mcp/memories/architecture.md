# librarian-mcp — Architecture

## Module Map

```
src/
  lib.rs              — entry: build_tool_context(), import_codescout(), reindex_cli()
  server.rs           — LibrarianServer (rmcp ServerHandler); tool dispatch via map_tool_result()
  workspace.rs        — WorkspaceConfig (roots, ignore globs, rules, umbrellas); load/compile
  current_project.rs  — CurrentProject (root, subdir, path, umbrella); resolve() from cwd
  classify.rs         — Rule/CompiledRule/Classification; first-match-wins glob classifier
  indexer.rs          — index_repo_sync() + index_repo(); walk→classify→upsert→embed pipeline
  embedding.rs        — EmbeddingService wrapping codescout_embed::Embedder
  frontmatter.rs      — YAML frontmatter parser (serde_yml); returns (Option<Frontmatter>, body)
  filter.rs           — FilterNode (And/Or/Not/Leaf) + LeafOp; compiles to SqlFragment
  freshness.rs        — Freshness enum (Fresh/Unknown/Stale/Superseded); compute() from inputs
  ids.rs              — artifact_id(repo, rel_path) → deterministic string ID
  util.rs             — normalize_rel_path()
  catalog/
    mod.rs            — Catalog { conn: Connection }; open/open_in_memory; schema bootstrap
    schema.sql        — v1 tables (artifact, artifact_link, artifact_vec, artifact_observation)
                        + v2 TimeMachine (events, commits, sources, event_edges)
    artifact.rs       — upsert/get/delete/delete_orphan_repos; ArtifactRow
    events.rs         — EventRow; insert/latest_for_artifact/timeline_for_artifact/open_intents/orphan_verdicts
    links.rs          — LinkRow; insert/outgoing/incoming
    find.rs           — FindOpts; find() (SQL) + find_semantic() (sqlite-vec KNN + post-filter)
    observations.rs   — artifact_observation rows
    commits.rs        — git commit metadata for TimeMachine cutoff resolution
    sources.rs        — external source ingestion records
    event_edges.rs    — causal edges between events
  tools/
    mod.rs            — Tool trait (name/description/input_schema/call); ToolContext; all_tools()
    find.rs           — ArtifactFind — filter+semantic search with scope, archived-hide, hints
    get.rs            — ArtifactGet — fetch one artifact + links + observations + freshness
    create.rs         — ArtifactCreate — write file + frontmatter + catalog upsert
    update.rs         — ArtifactUpdate — patch frontmatter fields or body + re-index
    link.rs           — ArtifactLink — insert artifact_link; auto-supersede on rel="supersedes"
    observe.rs        — ArtifactObserve — append observation + dual-write note event
    event_create.rs   — ArtifactEventCreate — manually log TimeMachine events
    timeline.rs       — ArtifactTimeline — event log for one artifact
    state_at.rs       — ArtifactStateAt — replay artifact state at commit/timestamp
    workspace_state_at.rs — WorkspaceStateAt — snapshot across all artifacts at a point in time
    list_by_kind.rs   — ArtifactListByKind — scoped listing with count hints
    links.rs          — ArtifactLinks — outgoing/incoming graph edges
    graph.rs          — ArtifactGraph — BFS neighborhood (depth 1–3)
    context.rs        — LibrarianContext — topic/anchor → token-capped markdown context bundle
    reindex.rs        — LibrarianReindex — on-demand scan; scope mirrors read-tool semantics
    scope.rs          — Scope enum + ScopeApplied; apply_scope() builds repo+rel_path filters
  preview/
    mod.rs            — extract(kind, row, body, ctx) — routes to kind-specific renderer
    spec.rs / plan.rs / memory.rs / default.rs / headings.rs / summary.rs
  prompts/
    server_instructions.md  — injected as MCP server info
    companion_hint.md       — companion plugin hint
```

## Key Abstractions

- **`Tool` trait** — `name()`, `description()`, `input_schema()`, `call(&ToolContext, Value) → Result<Value>`
- **`ToolContext`** — shared state: `Arc<Mutex<Catalog>>`, `Arc<WorkspaceConfig>`, `Arc<Vec<CompiledRule>>`, `Option<Arc<EmbeddingService>>`, `Option<Arc<CurrentProject>>`
- **`Catalog`** — wraps a single `rusqlite::Connection`; all catalog modules receive `&Catalog`
- **`FilterNode`** — recursive JSON filter tree; `filter::compile()` → `SqlFragment { sql, params }` for injection-safe parameterized queries
- **`Freshness`** — computed from latest event kind, last-reviewed timestamp, file mtime, topo distance from HEAD, and a configurable horizon
- **`RecoverableError`** — wraps expected failures; `map_tool_result()` maps these to `isError: false` (non-fatal); `anyhow::bail!` maps to `isError: true`

## Data Flow: Indexing

1. `build_tool_context()` loads workspace config + layered classifier rules
   (project-local → workspace → built-in defaults)
2. `index_repo_sync()` walks `.md` files under a root (optionally narrowed to a
   subdir), computes SHA-256, parses frontmatter, classifies via first-match glob,
   compares to existing row — skips if content + meta unchanged
3. Changed/new rows upserted to `artifact`; deleted files removed; embed queue
   populated (content changes only, not re-classification)
4. `index_repo()` async wrapper drains embed queue concurrently
   (`EMBED_CONCURRENCY=4`) via `codescout_embed`, writes vectors to `artifact_vec`

## Data Flow: Query (artifact_find)

1. `ArtifactFind::call()` deserializes `Args` (filter, limit, offset, semantic, scope, include_archived)
2. `scope::apply_scope()` wraps the user filter with repo/rel_path clauses based on `CurrentProject`
3. `combine_user_with_archived_hide()` injects `status NOT IN [archived, superseded]` unless the user filter already constrains status
4. If `semantic` text provided: embed query → `find_semantic()` (KNN in `artifact_vec`, post-filter by metadata), else `find()` (SQL WHERE)
5. Response includes `rows`, `count`, `hints` (more_in_repo, more_in_umbrella, more_in_workspace, hidden_archived, expand suggestions)

## Data Flow: TimeMachine (artifact_state_at)

1. `resolve_cutoff_ts()` converts commit SHA → authored_at from `commits` table, or uses raw timestamp
2. `replay_state_at()` fetches all events for the artifact up to `cutoff_ts`, walks them to reconstruct `status`, `frontmatter patches`, `supersession_chain`
3. Returns `ReplayedState { status, frontmatter, freshness_at_as_of, latest_event_summary, supersession_chain }`

## Scope System

- `Scope` enum: `Project | Repo | Umbrella | All`
- `apply_scope()` builds a `FilterNode` narrowing by `repo` eq + `rel_path` prefix
- Default = `Project`; falls back to `All` when cwd is outside every configured root (`scope_fallback` in hints)
- Umbrella scope joins all member subdirs under one umbrella declared in `workspace.toml`

## Good semantic_search Queries

- `semantic_search("artifact upsert freshness indexer pipeline", project="librarian-mcp")`
- `semantic_search("timemachine event log replay state commit", project="librarian-mcp")`
- `semantic_search("filter AST SQL compilation injection safe", project="librarian-mcp")`
- `semantic_search("scope workspace project umbrella cwd resolution", project="librarian-mcp")`
- `semantic_search("context pack token budget topic anchor", project="librarian-mcp")`
