# Domain Glossary

Terms used across multiple projects in this workspace.

## Core MCP / codescout Terms

- **Tool** — An MCP tool implementing `src/tools/mod.rs::Tool` trait. Every capability exposed to LLM clients is a Tool.
- **OutputGuard** — Enforces progressive disclosure. Exploring mode (default, cap 200) vs Focused (full, paginated). Used in every tool with variable-length output.
- **RecoverableError** — Expected input-driven failure (`isError: false`). Sibling parallel tool calls are not aborted. Contrast: `anyhow::bail!` = fatal (`isError: true`).
- **ActiveProject** — The currently focused project in `Agent`. Holds root, config, MemoryStore, LibraryRegistry, write lock.
- **WriteGuard** — Dual-layer write exclusion: in-process async mutex + fs4 cross-process file lock on `.codescout/write.lock`.
- **ToolContext** — Per-call bundle: `agent`, `lsp`, `output_buffer`, `progress`, `peer`.
- **OutputBuffer** — Named buffer for large tool output. Referenced as `@cmd_*`, `@file_*`, `@tool_*`.
- **ONBOARDING_VERSION** — Integer bumped in `src/tools/onboarding.rs` when prompt surfaces change.

## Embedding / Semantic Search Terms

- **Chunk / RawChunk** — Unit of text produced by the chunker for embedding. Has `content`, `start_line`, `end_line`, `metadata`.
- **Embedding / Embedding vector** — `Vec<f32>` produced by an `Embedder`. Used for KNN cosine search via sqlite-vec.
- **vec0 table** — sqlite-vec virtual table storing `Vec<f32>` + KNN search index. Lives in `.codescout/embeddings/project.db`.
- **CodeRankEmbed** — Default embedding model (remote). Uses asymmetric query prefix.
- **AllMiniLML6V2Q** — Default local ONNX embedding model (384d, ~22MB).
- **Drift** — Semantic distance between current file content and its last-indexed chunks. High drift = stale index.

## Librarian Terms

- **Artifact** — A markdown file with YAML frontmatter tracked by librarian-mcp.
- **FilterNode / LeafOp** — JSON filter AST for querying the artifact catalog. Compiled to parameterized SQL.
- **ArtifactRow** — The canonical SQLite row type in librarian-mcp's catalog (16 fields).
- **Context bundle** — Packed markdown from multiple artifacts returned by `librarian_context`.

## Cross-Project Domain (Fixtures)

All 5 language fixtures model the same library catalog domain:
- **Book** — Core domain entity (title, isbn, genre, copiesAvailable)
- **Genre** — Enum (Fiction/NonFiction/Science/History/Biography or language-specific casing)
- **Searchable** — Interface/trait/ABC for items that can be text-searched
- **Catalog<T>** — Generic container for Searchable items with add/search/stats
- **SearchResult** — Typed outcome type (Found/NotFound/Error) — sealed class (Kotlin), sealed interface (Java), enum (Rust), discriminated union (TypeScript), not present in Python fixtures
