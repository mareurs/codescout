# codescout Workspace — System Prompt

You are working in the `codescout` Rust workspace at `/home/marius/work/claude/code-explorer`.

## Projects

| ID | Root | Purpose |
|----|------|---------| 
| `code-explorer` | `.` | Main codescout MCP server (27 tools, Rust) |
| `codescout-embed` | `crates/codescout-embed/` | Shared embedding abstraction crate |
| `librarian-mcp` | `crates/librarian-mcp/` | Standalone artifact registry MCP server |
| `rust-library` | `tests/fixtures/rust-library/` | Rust LSP/symbol test fixture |
| `python-library` | `tests/fixtures/python-library/` | Python LSP/symbol test fixture |
| `typescript-library` | `tests/fixtures/typescript-library/` | TypeScript LSP/symbol test fixture |
| `java-library` | `tests/fixtures/java-library/` | Java LSP/symbol test fixture |
| `kotlin-library` | `tests/fixtures/kotlin-library/` | Kotlin LSP/symbol test fixture |

## Entry Points

### code-explorer
- `src/server.rs::CodeScoutServer::from_parts` — all 27 tools registered here
- `src/tools/mod.rs:543` — `Tool` trait definition
- `src/agent.rs::Agent::new` — project activation and state wiring

### codescout-embed
- `src/lib.rs` — public API: `create_embedder`, `create_embedder_with_config`, `chunk_size_for_model`
- `src/embedder.rs` — `Embedder` trait
- `src/chunker.rs` — `split`, `split_markdown`, `chunk_markdown`

### librarian-mcp
- `src/lib.rs` — `run_stdio_server()`, entry point
- `src/server.rs` — `LibrarianServer`: rmcp ServerHandler
- `src/tools/mod.rs` — `Tool` trait + `all_tools()`
- `src/catalog/mod.rs` — `Catalog` (rusqlite wrapper)
- `src/indexer.rs` — `index_repo_sync()` / `index_repo()`

## Key Abstractions

### code-explorer
- `Tool` trait — `name/description/input_schema/call/call_content/format_compact/availability`
- `OutputGuard` — progressive disclosure: Exploring (cap 200) vs Focused (paginated)
- `RecoverableError` — expected failures → `isError:false`; `anyhow::bail!` → `isError:true`
- `Agent` / `ActiveProject` — project state; tools access via `ctx.agent.with_project()`
- `WriteGuard` — async mutex + fs4 cross-process lock; acquired for all mutating calls
- `LspProvider` / `LspClientOps` — trait abstraction; `MockLspClient` for tests

### codescout-embed
- `Embedder` trait — `dimensions()`, `embed(texts)`, `embed_query(text)`
- `LocalEmbedder` — fastembed ONNX; constructor runs on `spawn_blocking`
- `RemoteEmbedder` — OpenAI-compat HTTP; 32-text batches, 3 retries, exp backoff
- `create_embedder_with_config(model, url, api_key)` — resolution order: url > local: > ollama: > openai:

### librarian-mcp
- `Tool` trait — simpler: `name/description/input_schema/call` only
- `ToolContext` — `catalog`, `workspace`, `rules`, `embedding` (optional)
- `FilterNode` — recursive JSON filter AST → SQL via `compile()`
- `ArtifactRow` — canonical catalog row; `id` = sha256(repo+"\\n"+rel_path)[:16]
- `EmbeddingService` — thin wrapper over codescout-embed `Embedder`

## Search Tips

### code-explorer
```
semantic_search("OutputGuard cap_items overflow hint", project="code-explorer")
semantic_search("RecoverableError recoverable isError guidance", project="code-explorer")
semantic_search("LSP client document symbols workspace symbols", project="code-explorer")
semantic_search("incremental embedding index build changed files", project="code-explorer")
semantic_search("write guard cross-process file lock mutex", project="code-explorer")
```

### codescout-embed
```
semantic_search("chunk overlap line tracking", project_id="codescout-embed")
semantic_search("remote embedder retry backoff batch size", project_id="codescout-embed")
semantic_search("model prefix resolution factory embedder", project_id="codescout-embed")
```

### librarian-mcp
```
semantic_search("artifact catalog sqlite indexer walk frontmatter", project_id="librarian-mcp")
semantic_search("filter AST compile SQL fragment leaf op", project_id="librarian-mcp")
semantic_search("KNN semantic search embedding vec0 backfill", project_id="librarian-mcp")
```

## Navigation Strategy

1. **New task on a specific tool** → `list_symbols("src/tools/<file>.rs")` then
   `find_symbol("<ToolName>", include_body=true)`
2. **Cross-cutting change** → `search_pattern` across `src/` + check all 3 prompt surfaces
3. **Bug in symbol editing** → `read_markdown("docs/TODO-tool-misbehaviors.md")` first
4. **LSP behavior question** → `list_symbols("src/lsp/client.rs")` then targeted `find_symbol`
5. **Embedding/indexing question** → activate `codescout-embed` project, then `find_symbol`
6. **Artifact registry question** → activate `librarian-mcp`, `list_symbols("src/tools/")`
7. **Unknown concept** → `semantic_search` in the most likely project first, then broaden

## Project Rules

- **Always run** `cargo fmt && cargo clippy -- -D warnings && cargo test` before completing any task
- **Release binary for MCP**: `cargo build --release` then `/mcp` restart — dev builds ignored
- **Write tools return `json!("ok")`** — never echo content back
- **Three prompt surfaces** in code-explorer must stay consistent: `server_instructions.md`,
  `onboarding_prompt.md`, `builders.rs`; bump `ONBOARDING_VERSION` on tool-name/param changes
- **New tool checklist**: 6 locations (struct, server.rs, test, check_tool_access, disabled test, server_instructions.md)
- **No parallel writes** — serialize all write tool calls (BUG-021)
- **Activate home project with write access** at session start: `activate_project(".", read_only: false)`
- **Always restore home project** after switching to another project
- **Master is protected** — cherry-pick from experiments only; never force-push master
