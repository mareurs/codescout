# Conventions

See `CLAUDE.md § Design Principles` and `docs/PROGRESSIVE_DISCOVERABILITY.md` for
the canonical output-sizing rules. This memory captures naming and code patterns.

## Naming

| Entity | Convention | Example |
|---|---|---|
| Tool structs | PascalCase, noun phrase | `FindSymbol`, `ListSymbols`, `ReplaceSymbol` |
| Tool names (MCP) | snake_case | `"find_symbol"`, `"list_symbols"` |
| `impl Tool for X` method `name()` | matches snake_case struct | `fn name() { "find_symbol" }` |
| Error variants | `RecoverableError::new("msg").with_hint("hint")` | path not found, unsupported type |
| Helper fns in tool files | snake_case, descriptive | `get_lsp_client`, `collect_matching`, `symbol_to_json` |
| Test modules | `mod tests` inline in file | all tests co-located with implementation |
| Test helpers | descriptive snake_case | `ctx_with_mock`, `project_with_files`, `make_server` |

## Tool Implementation Pattern

```rust
struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "..." }
    fn input_schema(&self) -> Value { json!({ "type": "object", "properties": { ... } }) }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let guard = OutputGuard::from_input(&input);          // always first
        let path = require_str_param(&input, "path")?;        // use helpers from mod.rs
        // ... logic ...
        let (items, overflow) = guard.cap_items(results, "hint for narrowing");
        Ok(json!({ "items": items, "overflow": overflow }))   // overflow is Option<OverflowInfo>
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format!("..."))  // compact summary for @tool_* buffer ref line
    }
}
```

## Error Handling

- Expected, input-driven failures → `RecoverableError::new("msg")` or `.with_hint("fix")`
- Genuine tool failures (LSP crash, programming bug) → `anyhow::bail!("...")`
- Never return `anyhow::bail!` for missing paths, unsupported types, empty results

## Testing Patterns

- Framework: `tokio::test` for async, `#[test]` for sync
- No test fixtures on disk — `tempdir()` per test, files written inline
- Mock LSP via `MockLspClient::new().with_symbols(path, vec![sym(...)])` in `tests/symbol_lsp.rs`
- Integration tests build a full `ToolContext` in `tests/integration.rs::project_with_files()`
- Three-query sandwich for cache-invalidation tests (see `CLAUDE.md § Testing Patterns`)
- E2E tests behind feature flags (`e2e-rust`, `e2e-python`, etc.) — require live LSP servers

## Code Quality Commands

See `CLAUDE.md § Development Commands`. Always run in order: `cargo fmt` → `cargo clippy -- -D warnings` → `cargo test`.

## Prompt Surface Rule

Three files must stay in sync when tools are renamed: `src/prompts/server_instructions.md`,
`src/prompts/onboarding_prompt.md`, `src/tools/workflow.rs::build_system_prompt_draft()`.
Grep all three when modifying tool names.
