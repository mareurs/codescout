# librarian-mcp — Architecture

## Module Structure

```
src/
  lib.rs             — run_stdio_server(), import_codescout(), reindex_cli(); wires workspace,
                       catalog, embedding service, and ToolContext; entry point for all modes
  main.rs            — clap CLI: no args=stdio server; reindex subcommand; import-codescout subcommand
  server.rs          — LibrarianServer: rmcp ServerHandler, dispatches to Tool trait via all_tools()
  tools/
    mod.rs           — Tool trait (name/description/input_schema/call), ToolContext, all_tools()
    find.rs          — ArtifactFind: filter AST + optional semantic KNN search
    get.rs           — ArtifactGet: single artifact + links + observations + kind-specific preview
    list_by_kind.rs  — ArtifactListByKind: paginated list for one kind
    links.rs         — ArtifactLinks: outgoing + incoming edges for one artifact
    graph.rs         — ArtifactGraph: BFS expansion up to depth 1-3 with rel filter
    create.rs        — ArtifactCreate: write frontmatter+body to disk, upsert catalog row
    update.rs        — ArtifactUpdate: patch frontmatter fields and/or body in-place
    link.rs          — ArtifactLink: insert directed relation edge (src, dst, rel)
    observe.rs       — ArtifactObserve: append observation note to an artifact
    reindex.rs       — LibrarianReindex: walk repos, classify, upsert; optional force wipe
    context.rs       — LibrarianContext: topic/anchor-driven markdown context packer
  catalog/
    mod.rs           — Catalog struct (rusqlite::Connection), open()/open_in_memory(), schema init
    schema.sql       — Tables: artifact, artifact_link, artifact_observation; vec0: artifact_vec;
                       cascade-delete trigger; indexes on kind/status, repo, link dst
    artifact.rs      — upsert(), get(), delete(), delete_orphan_repos(), row_from_sql()
    find.rs          — find() (SQL filter) + find_semantic() (KNN with iterative K backfill)
    links.rs         — insert(), outgoing(), incoming() for artifact_link
    observations.rs  — insert(), list_for_artifact() for artifact_observation
  indexer.rs         — index_repo_sync() (sync walk+upsert), index_repo() (async with embeddings),
                       write_embeddings(), first_h1(), EmbedQueueItem, IndexReport
  classify.rs        — Rule / CompiledRule (globset), load_rules(), classify() (first-match-wins)
  filter.rs          — FilterNode enum (And/Or/Not/Leaf), compile() → SqlFragment with param shifting
  frontmatter.rs     — Frontmatter struct, parse(), write(), update_in_place()
  ids.rs             — artifact_id(): sha256(repo+"\n"+rel_path) hex[:16]
  embedding.rs       — EmbeddingService: thin wrapper over codescout_embed::Embedder
  workspace.rs       — WorkspaceConfig (roots+ignore+rules), load(), compile_ignore()
  util.rs            — sha_of_bytes(), normalize_rel_path() (backslash→slash on Windows)
  preview/
    mod.rs           — extract(kind, row, body, ctx) dispatch
    plan.rs          — checklist extraction (open/done tasks, progress %)
    spec.rs          — sections/headings extraction
    memory.rs        — related artifact lookup via catalog
    default.rs       — fallback: first N lines + heading map
    headings.rs      — heading map extraction helper
    summary.rs       — summary extraction helper
  prompts/
    server_instructions.md — injected server instructions (iron laws, tool routing, filter AST)
```

## Key Abstractions

1. **`Tool` trait** (`src/tools/mod.rs`) — `name() / description() / input_schema() / call()`.
   Simpler than codescout's Tool; no `call_content()` or OutputGuard. All errors propagated as
   `anyhow::Error`; `LibrarianServer::call_tool()` formats them as `CallToolResult::error`.

2. **`ToolContext`** — `catalog: Arc<Mutex<Catalog>>`, `workspace: Arc<WorkspaceConfig>`,
   `rules: Arc<Vec<CompiledRule>>`, `embedding: Option<Arc<EmbeddingService>>`. Lock is
   `parking_lot::Mutex`; tools lock+unlock immediately; async embedding done outside the lock.

3. **`Catalog`** — wrapper around `rusqlite::Connection`. Single writer (no connection pool).
   Initialized once at startup with `PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL`.
   sqlite-vec registered as a global auto-extension (Once-guarded).

4. **`FilterNode`** — recursive JSON AST (And/Or/Not/Leaf). `compile()` produces `SqlFragment`
   (sql string + rusqlite params). Leaf ops: eq/ne/in/nin/gt/lt/gte/lte/contains. Array columns
   (tags, owners) use `json_each()` for `contains`. Field allowlist prevents SQL injection.

5. **`ArtifactRow`** — the canonical catalog row type. `owners`/`tags` stored as JSON arrays
   in TEXT columns and deserialized in `row_from_sql`. `id` is 16-char hex SHA-256 of (repo, rel_path).

6. **Preview system** — `artifact_get` dispatches to kind-specific extractors in `preview/`.
   `plan` → task checklist, `spec` → sections, `memory` → related artifacts, default → heading map.

## Data Flow: Indexing (reindex tool or CLI)

1. `LibrarianReindex::call()` or `reindex_cli()` → iterate workspace roots
2. **Phase 1 (sync, lock held)**: `index_repo_sync()` — walk `.md` files, parse frontmatter,
   apply classification rules (first-match-wins), compare mtime+sha256 to existing row,
   upsert changed rows, collect `EmbedQueueItem`s for new/changed artifacts
3. Lock dropped between phases
4. **Phase 2 (async, no lock)**: if embedding service present, embed each artifact title+body
5. **Phase 3 (sync, lock held)**: `write_embeddings()` → bulk upsert into `artifact_vec` (vec0)

## Data Flow: Semantic Search (`artifact_find` with `semantic` param)

1. Tool embeds query text via `EmbeddingService::embed_artifact()`
2. `find_semantic()` → KNN query on `artifact_vec` MATCH vec_f32(?): top-K candidates
3. Fetch full ArtifactRows for candidate IDs; apply filter AST as post-filter
4. If too few results and K < 2000: double K and retry (iterative backfill)
5. Results returned in KNN distance order (closest first)

## Data Flow: Read (artifact_find / artifact_get)

1. MCP client → `LibrarianServer::call_tool()` → find matching `Arc<dyn Tool>` by name
2. Deserialize `args: Value` → tool-specific `Args` struct via `serde_json::from_value()`
3. Lock catalog mutex → run SQL query → unlock → return JSON Value
4. Server wraps result as `CallToolResult::success(vec![Content::text(...)])`

## Database Schema Summary

- `artifact`: id (16-hex), repo, rel_path, kind, status, title, owners (JSON), tags (JSON),
  topic, time_scope, source, created_at/updated_at (ms epoch), file_mtime, file_sha256, confidence
- `artifact_link`: (src_id, dst_id, rel) PK with CASCADE DELETE; rel is free-form string
- `artifact_observation`: append-only notes on artifacts, CASCADE DELETE
- `artifact_vec`: sqlite-vec virtual table, float[768] embeddings, cascade via trigger

## Good semantic_search Queries

- `semantic_search("artifact catalog sqlite indexer walk frontmatter", project_id="librarian-mcp")`
- `semantic_search("filter AST compile SQL fragment leaf op", project_id="librarian-mcp")`
- `semantic_search("KNN semantic search embedding vec0 backfill", project_id="librarian-mcp")`
- `semantic_search("frontmatter parse YAML kind status title", project_id="librarian-mcp")`
- `semantic_search("preview kind plan spec memory extract", project_id="librarian-mcp")`
