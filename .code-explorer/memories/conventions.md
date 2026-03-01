# Conventions

## Naming
| Entity | Convention | Example |
|---|---|---|
| Tool struct | PascalCase noun | `FindSymbol`, `ListDir`, `RunCommand` |
| Tool MCP name | snake_case | `find_symbol`, `list_dir`, `run_command` |
| Module files | snake_case, by category | `src/tools/symbol.rs`, `src/tools/file.rs` |
| Test modules | `mod tests` inside each file | `#[cfg(test)] mod tests { ... }` |
| Error types | `RecoverableError` for soft, `anyhow::bail!` for hard | See `src/tools/mod.rs` |

## Error Handling
- **RecoverableError** (`isError: false`): For expected, input-driven failures. Includes `message` + optional `hint`. Does NOT abort sibling parallel calls.
- **anyhow::bail!** (`isError: true`): For genuine tool failures (LSP crash, security violation). Aborts sibling calls.
- Use `RecoverableError::with_hint()` to guide the LLM toward correct usage.

## Write Response Pattern
Mutation tools (`create_file`, `edit_file`, `replace_symbol`, etc.) return `json!("ok")` — never echo content back. The caller already knows what it sent.

## Output Sizing
All tools default to `Exploring` mode (compact, 200 item cap). Pass `detail_level: "full"` for `Focused` mode with offset/limit pagination. Overflow includes `hint` + `by_file` distribution map.

## Testing
- Tests live in `#[cfg(test)] mod tests` at the bottom of each file
- LSP tests use `MockLspClient` / `MockLspProvider` (no real LSP needed)
- E2E tests behind feature flags (`e2e-rust`, `e2e-python`, etc.)
- Cache-invalidation tests use three-query sandwich pattern (see CLAUDE.md)
- Test helpers: `tempfile::TempDir` for isolated project dirs

## Code Quality
- `cargo fmt` — rustfmt formatting
- `cargo clippy -- -D warnings` — zero warnings policy
- `cargo test` — all unit tests must pass
- Run all three before completing any task
