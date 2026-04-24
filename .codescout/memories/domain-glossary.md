# Domain Glossary

Terms used across two or more projects in this workspace.

## Code Intelligence

- **MCP** — Model Context Protocol. JSON-RPC protocol used by all three active Rust binaries to
  communicate with LLM clients (Claude Code, Cursor, etc.).
- **Tool** — A named MCP capability with a JSON Schema input, implemented via the `Tool` trait.
  code-explorer has 27 tools; librarian-mcp has 11 tools.
- **RecoverableError** — code-explorer error type for expected input-driven failures. Produces
  `isError: false` in MCP response so sibling parallel calls are not aborted.
- **OutputGuard** — code-explorer's progressive disclosure enforcer. Caps output in Exploring mode
  (default, 200 items) and paginates in Focused mode. Not present in librarian-mcp.
- **OutputBuffer / @ref** — code-explorer's session-scoped LRU buffer for large command/tool output.
  `@cmd_xxxx`, `@file_xxxx`, `@tool_xxxx` handles. Query with Unix tools in a follow-up call.
- **WriteGuard** — code-explorer's dual-layer write exclusion: in-process async mutex + fs4 cross-
  process file lock on `.codescout/write.lock`.

## Embedding / Semantic Search

- **Embedder** — Trait in codescout-embed. Two concrete backends: LocalEmbedder (fastembed/ONNX)
  and RemoteEmbedder (OpenAI-compat HTTP). Used by both code-explorer and librarian-mcp.
- **sqlite-vec / vec0** — SQLite extension providing KNN vector search via virtual tables.
  Used by code-explorer (chunk_embeddings) and librarian-mcp (artifact_vec).
- **CodeChunk** — code-explorer's unit of semantic indexing: file + start/end lines + embedding.
- **KNN backfill** — librarian-mcp's strategy: if semantic search returns too few results after
  filter post-processing, double K and retry (up to K=2000).

## Artifact / Document System (librarian-mcp)

- **Artifact** — A markdown document cataloged by librarian-mcp. Has an `id` (16-char hex SHA-256
  of repo+rel_path), kind, status, frontmatter fields, and optional link/observation edges.
- **Kind** — Artifact classification: `spec`, `plan`, `memory`, `roadmap`, `adr`, `audit`,
  `handoff`, `runbook` (and others). Determines preview strategy.
- **FilterNode** — Recursive JSON AST for artifact queries: And/Or/Not/Leaf ops (eq/ne/in/nin/
  gt/lt/gte/lte/contains). Compiled to SQL fragments by `filter::compile()`.
- **ArtifactLink** — Directed relation edge between two artifacts: (src_id, dst_id, rel).
  `rel` is a free-form string (e.g., "supersedes", "implements").

## Fixtures

- **Fixture library** — A minimal language-specific codebase under `tests/fixtures/` used as a
  test target for codescout's LSP and symbol navigation tests. All model the same "book catalog"
  domain across Rust, Python, TypeScript, Java, and Kotlin.
- **Book catalog domain** — The shared domain across all 5 fixture libraries: `Book`, `Genre`,
  `Searchable`/`Catalog` generics, `SearchResult`. Consistent across languages for cross-language
  test comparability.
