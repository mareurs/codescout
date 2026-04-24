# codescout — Conventions

## Rust Patterns

- **Error handling**: `RecoverableError` for expected input-driven failures (path not found, no index,
  unsupported type). `anyhow::bail!` for genuine bugs. Never use `RecoverableError` for internal errors.
- **Write tools**: Always return `json!("ok")` — never echo back content the caller just sent.
- **Tool entry point**: `call_content()` is the MCP entry point. It handles buffer routing.
  Tools implement `call()` which returns `Result<Value>`; `call_content()` wraps it.
- **Async trait**: All tools use `#[async_trait::async_trait]` from the `async-trait` crate.
- **Arc<dyn Tool>**: Tools are stored as `Vec<Arc<dyn Tool>>` — trait objects behind Arc for
  shared ownership across async calls.

## Testing Patterns

- **Cache-invalidation tests**: Three-query sandwich — baseline → stale assert (proves bug) →
  post-invalidation assert (proves fix). See `did_change_refreshes_stale_symbol_positions`.
- **Mock LSP**: Tests use `MockLspClient` (implements `LspClientOps`). Constructed via
  `ctx_with_mock()` helper in `tests/symbol_lsp.rs`. Real LSP tests require `#[cfg(feature="e2e-rust")]`.
- **Integration tests**: `tests/integration.rs` — full tool pipeline with `ToolContext`, real file I/O,
  no LSP. Tests are async (`#[tokio::test]`).
- **Fixture projects**: Language fixtures in `tests/fixtures/{rust,python,typescript,kotlin,java}-library`
  used by e2e tests. Do not add real dependencies to fixtures.
- **Unit tests**: Inline in each module behind `#[cfg(test)] mod tests { ... }`. Use `tempfile::tempdir()`
  for file I/O tests to avoid state pollution.

## Naming & Structure

- Tool structs: PascalCase unit structs (e.g. `FindSymbol`, `ReplaceSymbol`, `SemanticSearch`)
- One `impl Tool for X` block per tool file
- Tool files in `src/tools/` or `src/tools/symbol/` for LSP-backed tools
- `OutputGuard::from_input(&input)` at the top of every tool's `call()` that returns variable output
- Guidance variants: `Hint` (informational), `Warning` (likely wrong), `MustFollow` (hard rule)
- `strip_project_root` applied in `post_process()` — tools do not strip paths themselves

## Memory & Config

- Per-project memories live in `.codescout/memories/<topic>.md` (tracked) and
  `.codescout/private-memories/<topic>.md` (gitignored)
- Project config: `.codescout/project.toml` (optional; sensible defaults)
- Global config: `~/.config/codescout/config.toml` or `~/.codescout/config.toml`
- Library registry: `.codescout/libraries.json`
- Embedding DB: `.codescout/embeddings.db` (code chunks + semantic memories)
- Write lock: `.codescout/write.lock`

## Pre-Commit Requirements

Always run before completing any task:
```
cargo fmt && cargo clippy -- -D warnings && cargo test
```
For live MCP verification: `cargo build --release` then `/mcp` restart.

## Prompt Surface Consistency

When tools are renamed/added/changed: update all three surfaces in the same commit:
1. `src/prompts/server_instructions.md`
2. `src/prompts/onboarding_prompt.md`
3. `build_system_prompt_draft()` in `src/prompts/builders.rs`
Then bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` if the change affects tool names/params.
