# Workspace Architecture — codescout

## Project Map

- **core/** — The engine: solver, domain introspection, score director, constraint streams (see `memory(project_id="core", topic="architecture")`)
  - `core/optaplanner-core-impl/` — solver, config, domain descriptors, heuristics, moves, score director
  - `core/optaplanner-constraint-streams-bavet/` — default constraint stream engine (graph-based DAG)
  - `core/optaplanner-constraint-streams-drools/` — alternative Drools-based constraint evaluation
  - `core/optaplanner-constraint-streams-common/` — shared constraint stream infrastructure
  - `core/optaplanner-constraint-drl/` — DRL rule language support
- **optaplanner-persistence/** — Score serialization (Jackson, JAXB, XStream, JSON-B, JPA) (see `memory(project_id="optaplanner-persistence")`)
- **optaplanner-benchmark/** — Benchmarking framework (see `memory(project_id="optaplanner-benchmark")`)
- **optaplanner-test/** — ConstraintVerifier API (see `memory(project_id="optaplanner-test")`)
- **optaplanner-examples/** — 16 example planning problems (see `memory(project_id="optaplanner-examples")`)
- **optaplanner-spring-integration/** — Spring Boot autoconfigure (see `memory(project_id="optaplanner-spring-integration")`)
- **optaplanner-quarkus-integration/** — Quarkus extension with Gizmo bytecode gen (see `memory(project_id="optaplanner-quarkus-integration")`)
- **optaplanner-docs/** — AsciiDoc documentation (see `memory(project_id="optaplanner-docs")`)
- **build/** — BOM, build-parent, IDE config, javadoc, distribution
## Cross-Project Dependencies

```
code-explorer
  └── codescout-embed   (crates/codescout-embed, path dep)
  └── librarian-mcp     (no code dep; sibling MCP server, shared config)

codescout-embed
  └── (no internal deps)

librarian-mcp
  └── codescout-embed   (crates/codescout-embed, path dep for embeddings)

fixtures (java/kotlin/python/rust/typescript)
  └── (no deps; static targets for codescout tests)
```

## Shared Infrastructure

- **CI:** `.github/workflows/ci.yml` — runs `cargo test`, `cargo clippy`, `cargo fmt --check` on push/PR
- **Workspace Cargo.toml:** single `[workspace]` at root; all Rust crates share dep versions
- **Embedding model cache:** `~/.cache/huggingface/hub/` shared across code-explorer + librarian-mcp
- **Test fixtures:** `tests/fixtures/` — all 5 language fixtures are read-only navigation targets; codescout's integration tests reference them directly
- **Shared config dir:** `.codescout/` — workspace.toml, memories/, embeddings/ live here

## Key Shared Abstractions

- `Embedder` trait (codescout-embed) — consumed by both code-explorer and librarian-mcp
- `Catalog<T: Searchable>` pattern — mirrored in all 5 fixture languages (intentional parallel design)
- `Searchable` interface/trait — same concept in all 5 fixture languages for codescout test coverage
