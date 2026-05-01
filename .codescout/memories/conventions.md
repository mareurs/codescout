# Workspace Conventions — codescout

## Shared Across All Rust Projects (code-explorer, codescout-embed, librarian-mcp)

### Pre-commit Checklist
```
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release   # required before live MCP testing
```

### Error Handling
- `RecoverableError` for expected input-driven failures → `isError: false`
- `anyhow::bail!` for genuine bugs → `isError: true`
- Never leak full error chains to callers; log server-side only

### Naming
- Files: `snake_case.rs`; MCP tool names: `snake_case`; constants: `SCREAMING_SNAKE_CASE`
- Test functions name the invariant, not the action

### Git Workflow
- `experiments` branch for all in-progress work; `master` protected
- Cherry-pick to master only when tests pass + clippy clean
- Feature commits on `experiments` require docs in `docs/manual/src/experimental/`
- See code-explorer `conventions` memory for full cherry-pick + graduation procedure

## Per-Project Conventions

- **code-explorer:** `memory(project="code-explorer", topic="conventions")`
  — Tool authoring rules, OutputGuard, write-tool `json!("ok")`, prompt surface updates
- **codescout-embed:** `memory(project="codescout-embed", topic="conventions")`
  — Embedder trait impl rules, backend feature flag patterns, chunk size formula
- **librarian-mcp:** `memory(project="librarian-mcp", topic="conventions")`
  — FilterNode safety, SQL injection protection, TimeMachine patterns

## Fixture Projects (tests/fixtures/)

All 5 fixtures (java/kotlin/python/rust/typescript) share the same intentional design:
- Model a `Catalog<T: Searchable>` pattern in each language
- No tests, no runtime deps — static navigation targets only
- Java 21 / Kotlin / Python 3.x / Rust stable / TypeScript strict mode
- Naming: `Book`, `Genre`, `Catalog`, `Searchable`, `SearchResult` — consistent across all languages
