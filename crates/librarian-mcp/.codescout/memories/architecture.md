# librarian-mcp — Architecture

## Module Structure

```
src/
  lib.rs            — top-level: run_stdio_server, import_codescout, reindex_cli
  main.rs           — CLI entry (clap), delegates to lib functions
  server.rs         — LibrarianServer: rmcp ServerHandler, tool dispatch, error shaping
  tools/
    mod.rs          — Tool trait, ToolContext, all_tools() registry
    find.rs         — artifact_find (FilterNode + optional semantic)
    get.rs          — artifact_get (single artifact + previews + observations)
    list_by_kind.rs — artifact_list_by_kind (paginated, filter by kind)
    links.rs        — artifact_links (outgoing/incoming edges)
    graph.rs        — artifact_graph (BFS up to depth 3)
    create.rs       — artifact_create (write new file + upsert)
    update.rs       — artifact_update (patch frontmatter + body)
    link.rs         — artifact_link (insert relation edge)
    observe.rs      — artifact_observe (append observation note)
    reindex.rs      — librarian_reindex (trigger full scan)
    context.rs      — librarian_context (topic/anchor → packed markdown bundle)
  catalog/
    mod.rs          — Catalog struct (wraps rusqlite::Connection), schema init
    artifact.rs     — upsert / get / delete / delete_orphan_repos
    find.rs         — find() / find_semantic() — SQL + vec0 KNN path
    links.rs        — insert / outgoing / incoming link rows
    observations.rs — insert / list_for_artifact observation rows
    schema.sql      — embedded DDL (artifact, artifact_link, artifact_observation, artifact_vec)
  filter.rs         — FilterNode enum (And/Or/Not/Leaf), LeafOp, SQL transpiler
  frontmatter.rs    — parse / write / update_in_place (serde_yml, deny_unknown_fields)
  indexer.rs        — index_repo_sync (walk+upsert), index_repo (async embed queue)
  classify.rs       — CompiledRule (glob → kind/status/time_scope), classify()
  workspace.rs      — WorkspaceConfig (TOML): roots[], ignore[], rule[]
  ids.rs            — artifact_id(repo, rel_path) → 16-hex-char deterministic ID
  embedding.rs      — EmbeddingService wrapping Arc<dyn Embedder>
  preview/          — per-kind markdown preview extractors (spec, plan, memory, …)
```

## Key Abstractions

- `Catalog` — thin newtype around `rusqlite::Connection`; all DB ops are free functions
  in submodules taking `&Catalog`. Wrapped in `Arc<parking_lot::Mutex<Catalog>>` in
  `ToolContext`.
- `ToolContext` — shared state passed to every tool: catalog mutex, workspace config Arc,
  compiled rules Arc, optional embedding service Arc.
- `Tool` trait — `name()`, `description()`, `input_schema()`, async `call(&ToolContext, Value)`.
- `FilterNode` — recursive JSON filter AST (`And`/`Or`/`Not`/`Leaf`). Transpiled to SQL
  `WHERE` clause by `filter.rs`. Fields allowed: id, kind, status, repo, title, topic,
  time_scope, tags, owners, rel_path, updated_at, created_at, confidence.
- `ArtifactRow` — canonical in-memory representation mirroring the `artifact` table.
- `Frontmatter` — serde_yml struct; `deny_unknown_fields` enforces schema discipline.

## SQLite Schema (schema.sql)

- `artifact` — primary table; `(repo, rel_path)` unique; owners/tags stored as JSON arrays
- `artifact_link` — directional edges `(src_id, dst_id, rel)` PK triplet
- `artifact_observation` — append-only notes keyed to `artifact_id`
- `artifact_vec` — vec0 virtual table: `id TEXT PRIMARY KEY, embedding FLOAT[768]`
- Trigger: `artifact_vec_cascade_delete` auto-deletes vec rows on artifact delete
- Indexes: `(kind, status)`, `repo`, `link.dst_id`

## Data Flows

**Indexing (read path):**
`index_repo_sync` walks `.md` files via `WalkBuilder`, computes SHA-256, parses
frontmatter + classifies via `CompiledRule` globs, upserts `ArtifactRow`, collects
embed queue. `index_repo` (async) calls embed service concurrently
(`EMBED_CONCURRENCY = 4`) then writes float vectors to `artifact_vec`.

**Query path:**
`find()` → transpile `FilterNode` → SQL `WHERE` + `ORDER BY updated_at`.
`find_semantic()` → vec0 KNN subquery (`ORDER BY distance LIMIT top_k`) → join with
filter SQL as post-filter on candidates.

**Write path (tools):**
`artifact_create` / `artifact_update` write the file on disk (frontmatter round-trip),
then call `artifact::upsert` to sync the DB row.

## Semantic Search Query Examples (librarian-mcp scope)

- `semantic_search("embedding vector search artifact")` — find EmbeddingService, find_semantic, artifact_vec
- `semantic_search("frontmatter round-trip write update")` — find artifact_update, update_in_place, frontmatter::write
- `semantic_search("classify rule glob kind status")` — find CompiledRule, classify(), WorkspaceConfig rules
- `semantic_search("context packing token budget markdown bundle")` — find LibrarianContext, char_cap logic
- `semantic_search("filter AST SQL transpiler leaf op")` — find FilterNode, LeafOp, filter.rs SQL emit
