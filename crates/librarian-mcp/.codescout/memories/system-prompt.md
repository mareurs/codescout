# codescout Workspace — Code Explorer Guidance

## Entry Points

### code-explorer (rust)
- `src/server.rs::CodeScoutServer::from_parts` — all 29 tools registered here
- `src/tools/mod.rs` — `Tool` trait definition (line ~239); read before adding/modifying any tool
- `src/agent.rs::Agent::new` — project activation and state wiring

### codescout-embed (rust)
- `crates/codescout-embed/src/lib.rs` — public API: `create_embedder_with_config`, `chunk_size_for_model`
- `crates/codescout-embed/src/embedder.rs` — `Embedder` trait
- `crates/codescout-embed/src/chunker.rs` — `RawChunk`, `split()`, `split_markdown()`

### librarian-mcp (rust)
- `crates/librarian-mcp/src/main.rs` — binary entry point
- `crates/librarian-mcp/src/tools/mod.rs` — `ToolContext`, tool dispatch
- `crates/librarian-mcp/src/catalog/` — `Catalog`, `FilterNode`, `ArtifactRow`

### Fixtures (test-only, do not modify)
- `tests/fixtures/{rust,python,typescript,java,kotlin}-library/` — LSP test fixtures

## Key Abstractions

- `Tool` trait (`src/tools/mod.rs`) — name/description/schema/call/call_content/format_compact
- `OutputGuard` (`src/tools/output.rs`) — progressive disclosure; every variable-output tool uses it
- `RecoverableError` (`src/tools/mod.rs`) — recoverable vs fatal error routing
- `LspProvider` / `LspClientOps` (`src/lsp/ops.rs`) — LSP abstraction; `MockLspClient` for tests
- `Agent` / `ActiveProject` (`src/agent.rs`) — project state; all tools access via `ctx.agent.with_project()`
- `Embedder` trait (`crates/codescout-embed/src/embedder.rs`) — embedding backend abstraction

## Search Tips

Good semantic_search queries (scoped to project):
- `semantic_search("OutputGuard cap_items overflow hint", project="code-explorer")`
- `semantic_search("RecoverableError recoverable isError guidance", project="code-explorer")`
- `semantic_search("LSP client document symbols workspace symbols", project="code-explorer")`
- `semantic_search("incremental embedding index build changed files", project="code-explorer")`
- `semantic_search("embedding batch retry backoff", project="codescout-embed")`
- `semantic_search("chunk overlap line tracking", project="codescout-embed")`

Avoid unscoped queries — they return results from all 8 projects. Always pass `project="..."`.

Too-broad terms (scope required): "tool", "error", "file", "book", "search", "catalog"

## Navigation Strategy

### code-explorer
1. New task on a tool → `list_symbols("src/tools/<file>.rs")` + targeted `find_symbol` for bodies
2. Cross-cutting change → `search_pattern` across `src/` + check all 3 prompt surfaces
3. Bug in symbol editing → read `docs/TODO-tool-misbehaviors.md` first
4. LSP behavior → `list_symbols("src/lsp/client.rs")` then targeted reads

### codescout-embed
1. Embedding backend change → start in `src/remote.rs` or `src/local.rs`
2. Chunking change → `src/chunker.rs`; test with `cargo test -p codescout-embed`
3. New model support → `src/lib.rs::create_embedder_with_config` + `chunk_size_for_model`

### librarian-mcp
1. New tool → add to `src/tools/` following existing tool file structure
2. Filter query issue → `src/filter.rs::FilterNode` + `src/catalog/find.rs`
3. Indexing issue → `src/indexer.rs::index_repo_sync`

## Project Rules

- `cargo fmt && cargo clippy -- -D warnings && cargo test` before every completion
- `cargo build --release` + `/mcp` restart for live MCP verification
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures, `anyhow::bail!` for genuine bugs
- Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding any tool with variable-length output
- When renaming tools: update all 3 prompt surfaces + bump `ONBOARDING_VERSION`
- GitHub tools shell to `gh` CLI — not HTTP

## Workspace Projects

| ID | Root | Language | Role |
|----|------|----------|------|
| code-explorer | `.` | Rust | Main MCP server (29 tools) |
| codescout-embed | `crates/codescout-embed` | Rust | Shared embedding lib |
| librarian-mcp | `crates/librarian-mcp` | Rust | Markdown artifact registry MCP |
| java-library | `tests/fixtures/java-library` | Java 21 | LSP test fixture |
| kotlin-library | `tests/fixtures/kotlin-library` | Kotlin 2.1 | LSP test fixture |
| python-library | `tests/fixtures/python-library` | Python 3.10+ | LSP test fixture |
| rust-library | `tests/fixtures/rust-library` | Rust | LSP test fixture |
| typescript-library | `tests/fixtures/typescript-library` | TypeScript | LSP test fixture |
