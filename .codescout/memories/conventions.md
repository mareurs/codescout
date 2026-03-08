# Conventions

See `CLAUDE.md § Design Principles` and `CLAUDE.md § Key Patterns` for core rules.
This supplements with naming tables and testing patterns discovered in code.

## Naming

| Entity | Convention | Example |
|---|---|---|
| Tool struct | PascalCase noun | `FindSymbol`, `ListSymbols`, `EditFile` |
| Tool name (MCP) | snake_case verb_noun | `"find_symbol"`, `"list_symbols"`, `"edit_file"` |
| Tool file | category noun plural | `symbol.rs`, `file.rs`, `memory.rs`, `github.rs` |
| LSP ops trait | `*Ops` suffix | `LspClientOps` |
| LSP provider | `*Provider` suffix | `LspProvider` |
| Mock types | `Mock*` prefix | `MockLspClient`, `MockLspProvider` |
| Integration test helpers | `project_with_files` | Creates `TempDir` + `ToolContext` |
| Config files | `project.toml`, `libraries.json` | Under `.codescout/` |
| Anchor sidecars | `.anchors.toml` | Alongside memory topic files |

## Error Handling Pattern

```rust
// Expected / user-fixable → RecoverableError
return Err(RecoverableError::with_hint("message", "what to do").into());

// Bug or genuine failure → anyhow
anyhow::bail!("unexpected state: {}", e);
```

`RecoverableError` has two constructors: `new(msg)` and `with_hint(msg, hint)`.
The `hint` field appears in the JSON response body — give actionable next steps.

## Testing Patterns

- **Unit tests**: inline `#[cfg(test)] mod tests` at bottom of each file
- **Integration tests**: `tests/integration.rs` — multi-tool workflows via `project_with_files()`
  helper that creates a `TempDir` + `ToolContext` with real `Agent` + `LspManager`
- **LSP tests**: `tests/symbol_lsp.rs` and `tests/rename_symbol.rs` — behind `e2e-*` features
- **Cache-invalidation tests**: three-query sandwich (see `CLAUDE.md § Testing Patterns`)
- **Async tests**: `#[tokio::test]` throughout; integration tests are async
- **Mock pattern**: `MockLspClient` / `MockLspProvider` (src/lsp/mock.rs) — injectable
  via `ToolContext.lsp: Arc<dyn LspProvider>`

## Tool Implementation Checklist

Adding a new tool requires 6 locations (see `MEMORY.md § New Tool Checklist`):
1. Tool struct + `impl Tool` in `src/tools/*.rs`
2. `Arc::new(ToolName)` in `server.rs::from_parts`
3. Tool name in `server_registers_all_tools` test
4. If write tool: add to `check_tool_access` match arm in `src/util/path_security.rs`
5. If write tool: add to corresponding `*_disabled_blocks_*` security test
6. Tool description in `src/prompts/server_instructions.md`

## Code Quality

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```
All three must pass before committing. CI enforces on ubuntu/macos/windows.
