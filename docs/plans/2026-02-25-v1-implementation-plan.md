# code-explorer v1.0 Implementation Plan

> Full product vision: from current state (4 working tools) to release-ready v1.0.
> Sprint-level granularity with dependency ordering and checkpoints.

## Current State

- 32 source files, 9 modules, 60 tests passing
- 4 tools working: `read_file`, `list_dir`, `search_for_pattern`, `execute_shell_command`
- 23 tools stubbed (14 have backing implementations ready to wire)
- MCP server working over stdio (rmcp)
- Core libraries implemented: chunker, embedding index, memory store, git blame/log, config

## Architecture Decision: ToolContext

**Problem**: Tools need access to growing shared state (Agent, LSP client, parser pool, embedder).
Current `Tool::call(&self, input: Value)` has no way to pass context.

**Solution**: Extend the Tool trait with `ToolContext`:

```rust
pub struct ToolContext {
    pub agent: Agent,
    // Phase 3: pub lsp: Arc<LspManager>,
    // Phase 4: pub parser: Arc<ParserPool>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value>;
}
```

**Why not store Agent in each tool**: O(resources × tools) modification surface as shared
resources grow. ToolContext is O(1) — extend the struct, all tools get access automatically.

---

## Phase 0: Architecture Foundation

### Sprint 0.1 — ToolContext Refactor

Introduce `ToolContext`, change `Tool::call` signature, update all 27 tools and server dispatch.
Zero behavioral change — mechanical refactor.

**Tasks:**
- [ ] Create `ToolContext` struct in `src/tools/mod.rs`
- [ ] Change `Tool::call` signature to `async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value>`
- [ ] Update `CodeExplorerServer::call_tool()` to construct and pass `ToolContext`
- [ ] Update all 27 tool implementations (add `ctx` parameter, unused for now)
- [ ] Verify: `cargo test` passes, `cargo clippy` clean

**Files:** `src/tools/mod.rs`, `src/server.rs`, all 8 `src/tools/*.rs` files
**Acceptance:** 60 tests pass, MCP handshake works, no behavioral change

---

## Phase 1: Wire Existing Backends

### Sprint 1.1 — Memory Tools (4 tools)

Wire memory tools to the already-implemented `MemoryStore`.

**Tasks:**
- [ ] `write_memory`: parse topic + content from input, call `ctx.agent` → `active_project.memory.write()`
- [ ] `read_memory`: parse topic, call `memory.read()`, return content or "not found"
- [ ] `list_memories`: call `memory.list()`, return topics array
- [ ] `delete_memory`: parse topic, call `memory.delete()`
- [ ] Add tool-level integration tests (tempdir-based)

**Access pattern:** `ctx.agent.inner.read().await` → `active_project.memory`
**Files:** `src/tools/memory.rs`
**Acceptance:** 4 memory tools work end-to-end via MCP

### Sprint 1.2 — Git Tools (3 tools)

Wire git tools to `git2` backends. One new function needed for diff.

**Tasks:**
- [ ] `git_blame`: parse path + optional line range, call `git::blame::blame_file()`
- [ ] `git_log`: parse optional path + limit, call `git::file_log()`
- [ ] Implement `git::diff_workdir()` — new function using git2 diff API
- [ ] `git_diff`: parse optional path + commit, call `git::diff_workdir()`
- [ ] Add tests using temp git repos with commits

**Access pattern:** `ctx.agent` → project root → `git2::Repository::open(root)`
**Files:** `src/tools/git.rs`, `src/git/mod.rs` (new `diff_workdir`)
**Acceptance:** blame, log, diff all return real data for a git repo

### Sprint 1.3 — Config + Workflow Tools (4 tools)

Wire config and workflow tools to Agent.

**Tasks:**
- [ ] `activate_project`: parse path, call `ctx.agent.activate(path)`
- [ ] `get_current_config`: read Agent inner state, return config as JSON
- [ ] `onboarding`: detect languages in project, create `.code-explorer/project.toml`
- [ ] `check_onboarding_performed`: check if `.code-explorer/` directory exists
- [ ] Add tests for activation roundtrip and onboarding

**Files:** `src/tools/config.rs`, `src/tools/workflow.rs`
**Acceptance:** can activate a project, read its config, run onboarding

### Sprint 1.4 — Semantic Tools (3 tools)

Wire semantic search to the embedding index. Requires embedder for query embedding.

**Tasks:**
- [ ] `index_project`: parse optional `force` flag, call `embed::index::build_index()`
- [ ] `semantic_search`: embed query via `create_embedder()`, call `embed::index::search()`
- [ ] Implement `embed::index::index_stats()` — new function querying SQLite for counts
- [ ] `index_status`: call `index_stats()`, return file/chunk counts + model info
- [ ] Add tests (mock embedder or small test corpus with known embeddings)

**Access pattern:** `ctx.agent` → project root + config → `open_db()` + `create_embedder()`
**Files:** `src/tools/semantic.rs`, `src/embed/index.rs` (new `index_stats`)
**Acceptance:** can index a project and search it; stats reflect indexed content
**Note:** Requires running embedding API (Ollama/OpenAI) for full integration test

---

## Phase 2: Complete File Tools

### Sprint 2.1 — Missing File Tools (3 new tools)

Add file manipulation tools expected by coding agents.

**Tasks:**
- [ ] `create_text_file`: write content to path, create parent dirs via `util::fs::write_utf8()`
- [ ] `find_file`: glob-based search via `globset` + `walkdir`
- [ ] `replace_content`: regex or literal find-and-replace in a file
- [ ] Register all 3 in `server.rs`
- [ ] Add tests: write/read roundtrip, glob matching, regex replacement

**Files:** `src/tools/file.rs`, `src/server.rs`
**Acceptance:** all 3 tools work, total working tools: 18-21

### ⟹ CHECKPOINT: Usable MCP Server

At this point: memory, git, semantic search, config, file manipulation all working.
The server is genuinely useful as a coding agent backend.

**Evaluation criteria:**
- All Phase 0-2 tools pass tests
- MCP server handles real-world tool sequences (index → search → read → edit)
- Decide if Phase 3 priorities need adjustment

---

## Phase 3: LSP Client

The core value proposition — symbol-level intelligence across languages.
Sequential sprints, each building on the previous.

### Sprint 3.1 — LSP Process Lifecycle

Spawn and manage language server processes.

**Tasks:**
- [ ] `LspManager` struct: manages per-language server instances
- [ ] `LspManager::get_or_start(language, workspace_root)` — lazy startup
- [ ] Spawn process with stdio pipes (`tokio::process::Command`)
- [ ] Graceful shutdown on drop
- [ ] Crash detection + automatic restart with backoff
- [ ] Add `lsp: Arc<LspManager>` to `ToolContext`
- [ ] Tests: spawn a language server, verify it starts and stops

**Files:** `src/lsp/client.rs`, `src/tools/mod.rs` (ToolContext)
**Acceptance:** can spawn rust-analyzer or pyright, process lifecycle works

### Sprint 3.2 — JSON-RPC Protocol Layer

Implement the LSP JSON-RPC communication layer.

**Tasks:**
- [ ] JSON-RPC message framing (Content-Length header + JSON body)
- [ ] Request/response correlation via ID tracking
- [ ] `initialize` / `initialized` handshake per LSP spec
- [ ] `textDocument/didOpen` notification
- [ ] Request timeout handling
- [ ] Notification dispatch (diagnostics, etc. — log and discard initially)
- [ ] Tests: full handshake with a real LSP server

**Files:** `src/lsp/client.rs` (or new `src/lsp/transport.rs`)
**Acceptance:** can send requests and receive correlated responses

### Sprint 3.3 — Document Symbols → `get_symbols_overview`

First LSP-backed tool: file/directory symbol tree.

**Tasks:**
- [ ] Send `textDocument/documentSymbol` request
- [ ] Parse `DocumentSymbol[]` response into `Vec<SymbolInfo>` tree
- [ ] Wire `get_symbols_overview` tool: accept file path, return symbol tree
- [ ] Handle directory paths: aggregate symbols from all files
- [ ] Tests: get symbols from Rust and Python test files

**Files:** `src/tools/symbol.rs`, `src/lsp/client.rs`
**Acceptance:** `get_symbols_overview` returns real symbols for a source file

### Sprint 3.4 — Definition + References

Navigation tools — find where things are defined and used.

**Tasks:**
- [ ] `textDocument/definition` → convert URI+position response to file:line
- [ ] `textDocument/references` → collect all locations
- [ ] Wire `find_symbol` tool: search by name, resolve to definition location
- [ ] Wire `find_referencing_symbols` tool: find all references to a symbol
- [ ] LSP position ↔ line number conversion utilities
- [ ] Tests: find function definition, find all call sites

**Files:** `src/tools/symbol.rs`, `src/lsp/client.rs`
**Acceptance:** can navigate to definitions and find all references

### Sprint 3.5 — Rename + Symbol Editing

The most complex LSP tools — modifying code through symbol operations.

**Tasks:**
- [ ] `textDocument/rename` → apply workspace edit → wire `rename_symbol` tool
- [ ] Symbol body extraction: use document symbols + line ranges to identify symbol text
- [ ] `replace_symbol_body`: extract current body, replace with new content
- [ ] `insert_before_symbol` / `insert_after_symbol`: locate symbol, insert adjacent text
- [ ] Tests: rename a function, replace a method body, insert code before/after

**Files:** `src/tools/symbol.rs`, `src/lsp/client.rs`
**Acceptance:** all 7 symbol tools work end-to-end

### ⟹ CHECKPOINT: Serena Parity

All scaffolded tools working. Symbol intelligence over LSP.

**Evaluation criteria:**
- All symbol tools work with at least rust-analyzer and pyright
- Performance acceptable for interactive use
- Decide if tree-sitter fallback is worth the effort or if LSP-only is sufficient

---

## Phase 4: Tree-sitter AST Engine

Offline symbol extraction as LSP fallback.

### Sprint 4.1 — Grammar Loading + Symbol Extraction

**Tasks:**
- [ ] Add grammar crates: `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, `tree-sitter-go`
- [ ] Implement `extract_symbols_from_source()` in `src/ast/parser.rs`
- [ ] Extract: functions, classes, methods, structs, interfaces with name + line range
- [ ] Add `parser: Arc<ParserPool>` to `ToolContext` (optional, lazy-init)
- [ ] Tests: parse Rust, Python, TypeScript, Go files; verify symbol extraction

**Files:** `src/ast/parser.rs`, `Cargo.toml`, `src/tools/mod.rs`
**Acceptance:** can extract symbols from source without any LSP running

### Sprint 4.2 — Wire to Tools + Fallback

**Tasks:**
- [ ] Wire `list_functions` tool → tree-sitter symbol extraction
- [ ] Wire `extract_docstrings` tool → tree-sitter comment/docstring nodes
- [ ] Implement fallback logic in symbol tools: try LSP → fall back to tree-sitter
- [ ] Update `get_symbols_overview` to use tree-sitter when LSP unavailable
- [ ] Tests: verify fallback behavior (simulate LSP failure)

**Files:** `src/tools/ast.rs`, `src/tools/symbol.rs`
**Acceptance:** tools work with or without LSP; graceful degradation

---

## Phase 5: Polish & v1.0

Independent sprints — can be done in any order.

### Sprint 5.1 — HTTP/SSE Transport

**Tasks:**
- [ ] Add `rmcp` feature: `transport-sse-server`
- [ ] Add `axum` dependency
- [ ] Implement HTTP transport in `server::run()` (the `"http"` arm)
- [ ] Tests: HTTP handshake, tool call over HTTP

**Files:** `src/server.rs`, `Cargo.toml`
**Acceptance:** `code-explorer start --transport http` works

### Sprint 5.2 — Production Vector Search + Local Embeddings

**Tasks:**
- [ ] sqlite-vec extension loading: `conn.load_extension("sqlite_vec")`
- [ ] Replace naive cosine with `vec_distance_cosine()` SQL query
- [ ] Optional: local embedding via `candle`/`ort` + `jina-embeddings-v2-base-code`
- [ ] Benchmark: pure Rust cosine vs sqlite-vec on 10K+ chunks

**Files:** `src/embed/index.rs`, `Cargo.toml`, optionally new `src/embed/local.rs`
**Acceptance:** vector search performance suitable for large projects

### Sprint 5.3 — Integration Tests + Release

**Tasks:**
- [ ] End-to-end MCP tests: handshake → tool calls → results
- [ ] Multi-tool workflow tests: index → search → read_file → replace_content
- [ ] `cargo build --release` optimization (strip, LTO)
- [ ] README.md with install/usage instructions
- [ ] Binary packaging (optional: cargo-dist or manual)

**Files:** `tests/` (new integration test directory), `README.md`, `Cargo.toml`
**Acceptance:** release binary works, documentation complete

### ⟹ CHECKPOINT: v1.0 Release

Production-ready binary with all transports, optimized search, tested workflows.

---

## Dependency Graph

```
Sprint 0.1 (ToolContext)
    ├── Sprint 1.1 (Memory)
    ├── Sprint 1.2 (Git)
    ├── Sprint 1.3 (Config)
    ├── Sprint 1.4 (Semantic)
    ├── Sprint 2.1 (File tools)       ← independent of Phase 1
    └── Sprint 3.1 (LSP lifecycle)
            └── 3.2 (JSON-RPC)
                └── 3.3 (Symbols)
                    └── 3.4 (Def+Refs)
                        └── 3.5 (Rename+Edit)
    Sprint 4.1 (Grammars)             ← independent of Phase 3
        └── 4.2 (Wire+Fallback)       ← depends on 3.x for fallback logic
    Sprint 5.1 (HTTP)                 ← independent after Phase 0
    Sprint 5.2 (sqlite-vec)           ← after Sprint 1.4
    Sprint 5.3 (Release)              ← after everything
```

## Parallelism Opportunities

- **Phase 1 sprints** (1.1–1.4) are independent of each other
- **Sprint 2.1** (file tools) is independent of all Phase 1 sprints
- **Sprint 5.1** (HTTP) can start anytime after Phase 0
- **Sprint 4.1** (grammars) can start anytime after Phase 0
- **Within Phase 3**: strictly sequential

---

*Created: 2026-02-25*
*Plan covers: current state → v1.0 release*
