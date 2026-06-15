# codescout Workspace System Prompt

## Entry Points by Project

| Project | Entry point | Key abstraction |
|---|---|---|
| codescout | `src/server.rs::CodeScoutServer` | `tool.call_content()` dispatches all MCP calls |
| codescout-embed | `crates/codescout-embed/src/lib.rs::create_embedder_with_config` | `Embedder` trait; `split()` / `chunk_markdown()` |
| edit-eval-rust | `tests/e2e/edit_eval/cases.rs::all()` (harness) | `EditCase` + `EditEvalCtx` |
| nav-eval-rust | `tests/fixtures/nav-eval-rust/src/lib.rs` (12 modules) | each `.rs` file is a self-contained nav trap |
| java-library | `tests/fixtures/java-library/src/main/java/library/` | `Catalog<T>`, `Searchable`, `SearchResult` |
| kotlin-library | `tests/fixtures/kotlin-library/src/main/kotlin/library/` | `Catalog<T : Searchable>`, `SearchResult` sealed class |
| python-library | `tests/fixtures/python-library/library/` | `Catalog[T]`, `Searchable` ABC |
| rust-library | `tests/fixtures/rust-library/src/lib.rs` | `Catalog<T: Searchable>`, `Searchable` trait |
| typescript-library | `tests/fixtures/typescript-library/src/index.ts` | `Catalog<T>`, `SearchResult` discriminated union |

## Key Abstractions (codescout)

- `CodeScoutServer` — MCP `ServerHandler`; routes all `CallToolRequest`s
- `Tool` trait + `ToolContext` (`src/tools/core/types.rs`) — extension point every tool implements; `call_content()` is the MCP entry point
- `Agent` / `ActiveProject` (`src/agent/`) — project state (config, memory, write lock); `with_project(|p| ...)`
- `OutputGuard` (`src/tools/output.rs`) — Exploring mode: compact, capped at 200; Focused: full, paginated
- `RecoverableError` — `isError: false`; prevents sibling tool call abort on expected failures

## Search Tips by Project

- **codescout:** `semantic_search(query)` works; `symbols(path="src/tools/")` for tool internals
- **codescout-embed:** no semantic index → `grep(pattern, path="crates/codescout-embed/src/")`
- **All fixture libraries:** no semantic index → `grep(pattern, path="tests/fixtures/<name>/src")` or `symbols(path="tests/fixtures/<name>/")`
- **nav-eval-rust traps:** navigate by module — `symbols(path="tests/fixtures/nav-eval-rust/src/<trap>.rs")`
- **edit-eval scenarios:** cases in `tests/e2e/edit_eval/cases.rs`; fixtures in `tests/fixtures/edit-eval-rust/src/`

## Navigation Strategy

1. **Unknown location** → `semantic_search(query)`, then drill with `symbols`
2. **Known file/dir** → `symbols(path=...)` overview; `symbols(name=..., include_body=true)` for body
3. **Pattern/string** → `grep(pattern, path=...)`; always scope with a `path`
4. **Who calls X** → `references(symbol, path)`, NOT grep
5. **Call chain** → `call_graph(symbol, path, direction="callers")` for blast radius
6. **Fixture projects** → skip `semantic_search`; go directly to `grep` or `symbols`

## Workspace Layout

```
src/                          codescout MCP server source
crates/codescout-embed/       shared embedding crate
tests/fixtures/               all language fixture libraries
  java-library/               Java 21 fixture
  kotlin-library/             Kotlin 2.1 fixture
  python-library/             Python 3.10 fixture
  rust-library/               Rust fixture
  typescript-library/         TypeScript fixture
  edit-eval-rust/             edit_code eval fixture (standalone workspace)
  nav-eval-rust/              nav eval fixture (standalone workspace)
tests/e2e/                    integration test suite
docs/                         architecture, trackers, issues, plans
```