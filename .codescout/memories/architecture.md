# Workspace Architecture

## Project Map

- **code-explorer** (`src/`) — Main `codescout` MCP server binary. 27 registered tools covering
  file ops, LSP symbol navigation, semantic search, project memory, shell execution, and git.
  Primary consumer of codescout-embed and librarian-mcp.

- **codescout-embed** (`crates/codescout-embed/`) — Shared embedding primitives crate. Backend-
  agnostic `Embedder` trait with LocalEmbedder (fastembed/ONNX) and RemoteEmbedder (OpenAI-compat
  HTTP). Used by both code-explorer and librarian-mcp.

- **librarian-mcp** (`crates/librarian-mcp/`) — Standalone MCP server for workspace artifact
  registry. Indexes markdown across git repos, classifies via frontmatter/glob rules, exposes
  11 MCP tools for discovery, traversal, and context packing. Uses codescout-embed for semantic
  search over indexed documents.

- **rust-library** (`tests/fixtures/rust-library/`) — Rust test fixture for codescout's LSP and
  symbol navigation tests. Book catalog domain; exercises structs, enums, traits, generics,
  lifetimes, and re-exports.

- **python-library** (`tests/fixtures/python-library/`) — Python test fixture. Exercises
  dataclasses, enums, ABCs, Protocols, generics, mixins, type aliases.

- **typescript-library** (`tests/fixtures/typescript-library/`) — TypeScript test fixture. Exercises
  decorators, generics, overloads, mapped/conditional types, namespace merging.

- **java-library** (`tests/fixtures/java-library/`) — Java 21 test fixture. Exercises records,
  sealed interfaces, generics, annotations.

- **kotlin-library** (`tests/fixtures/kotlin-library/`) — Kotlin test fixture. Exercises data
  classes, sealed classes, inline/value classes, suspend extensions, scope functions.

## Cross-Project Dependencies

```
code-explorer  ──depends on──►  codescout-embed  (embedding backends)
librarian-mcp  ──depends on──►  codescout-embed  (embedding backends)
code-explorer  ──ships with──►  librarian-mcp    (companion binary in workspace)
code-explorer  ──tests against─►  rust/python/typescript/java/kotlin fixture libraries
```

All fixture libraries have zero external dependencies (stdlib only) and no test suites of
their own — they are exercised exclusively by code-explorer's Rust integration tests.

## Shared Infrastructure

- **SQLite storage**: code-explorer and librarian-mcp both use rusqlite (bundled) + sqlite-vec
  (vec0 virtual tables) for persistent state and KNN vector search.
- **MCP protocol**: all three active Rust projects use rmcp (same workspace dependency version).
- **Async runtime**: tokio throughout all Rust crates.
- **Vector dimensionality**: float[768] in librarian-mcp; code-explorer matches the chosen
  embedding model's dimensions (model-dependent).
- **Cargo workspace**: single `Cargo.toml` at repo root governs all crates; shared dependency
  versions for rmcp, tokio, rusqlite, serde, anyhow, etc.
- **CI**: `cargo fmt && cargo clippy -- -D warnings && cargo test` required before every commit.
