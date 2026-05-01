# Workspace Architecture — codescout

## Project Map

| Project | Root | Purpose |
|---------|------|---------|
| `code-explorer` | `.` | Main codescout MCP server — IDE-grade code intelligence over stdio/HTTP |
| `codescout-embed` | `crates/codescout-embed/` | Shared embedding library (local ONNX + remote HTTP backends) |
| `librarian-mcp` | `crates/librarian-mcp/` | Standalone MCP server for indexing markdown docs across git repos |
| `java-library` | `tests/fixtures/java-library/` | Java 21 fixture: records, sealed interfaces, generics, annotations |
| `kotlin-library` | `tests/fixtures/kotlin-library/` | Kotlin fixture: sealed classes, coroutines, delegates, value classes |
| `python-library` | `tests/fixtures/python-library/` | Python fixture: dataclasses, ABC, Protocol, generics |
| `rust-library` | `tests/fixtures/rust-library/` | Rust fixture: traits, generics, lifetimes, custom iterators |
| `typescript-library` | `tests/fixtures/typescript-library/` | TypeScript fixture: discriminated unions, decorators, mapped types |

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
