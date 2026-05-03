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

- Core: see `memory(project_id="core", topic="conventions")` — trailing underscore generics, Config→Factory→Runtime, section separators
- Examples: see `memory(project_id="optaplanner-examples", topic="conventions")` — per-example package structure
- Benchmark: see `memory(project_id="optaplanner-benchmark", topic="conventions")` — JAXB config, result hierarchy
- Test: see `memory(project_id="optaplanner-test", topic="conventions")` — fluent assertion builder
- Spring: see `memory(project_id="optaplanner-spring-integration", topic="conventions")` — @ConditionalOnMissingBean, EntityScanner
- Quarkus: see `memory(project_id="optaplanner-quarkus-integration", topic="conventions")` — build-time/runtime split, Gizmo, ARC workarounds
- Persistence: see `memory(project_id="optaplanner-persistence", topic="conventions")` — serializer per score type
## Fixture Projects (tests/fixtures/)

All 5 fixtures (java/kotlin/python/rust/typescript) share the same intentional design:
- Model a `Catalog<T: Searchable>` pattern in each language
- No tests, no runtime deps — static navigation targets only
- Java 21 / Kotlin / Python 3.x / Rust stable / TypeScript strict mode
- Naming: `Book`, `Genre`, `Catalog`, `Searchable`, `SearchResult` — consistent across all languages
