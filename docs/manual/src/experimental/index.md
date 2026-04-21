# Experimental Features

> These features are available on `master` and the `experiments` branch.
> APIs and behaviour may change without notice. When a feature graduates to
> stable, its page moves into the main manual.

## Available Features

- [Asymmetric query prefix for embedding models](./asymmetric-query-prefix.md) — automatic query-side prefix for models trained asymmetrically (e.g. CodeRankEmbed), restoring `semantic_search` recall without user configuration.
- [Bash language support](./bash-language-support.md) — full symbol extraction and LSP navigation for `.sh` / `.bash` files via `bash-language-server`.
- [`list_symbols` progressive directory overview](./list-symbols-progressive-dir.md) — adaptive three-mode output (full tree / class overview / directory map) for directory-level symbol queries.
- [MCP resources, tool diet, progress notifications](./mcp-resources.md) — token-efficient resource sharing, short descriptions with on-demand guides, and progress notifications for long operations.
- [Project hints in `activate_project`](./project-hints.md) — manifest-derived primary language, entry points, and build commands surfaced in the activation response so agents have context without running onboarding.
- [`read_markdown` improvements](./read-markdown-improvements.md) — adaptive output tiers, `@file_*` buffer ref support, and heading navigation for large markdown files.
- [Rust LSP multiplexer](./mux-rust.md) — share a single `rust-analyzer` process across multiple `codescout` instances on the same project, eliminating stale-hover / stale-goto bugs.
- [Tool usage doctor](./tool-usage-doctor.md) — `doctor://tool-usage` MCP resource reporting per-tool call counts, error/overflow rates, and prune candidates for the next prompt-surface review.
- [Cross-process write serialization](./cross-process-write-serialization.md) — advisory file lock serializes write-tool calls across concurrent codescout instances on the same project; contention returns a recoverable error instead of corrupting files.
- [Index Scope Guard](./index-scope-guard.md) — confirmation prompt before `index_project` walks home/system directories or oversized trees.
- [Metadata-Enriched Chunks](./metadata-enriched-chunks.md) — file path, container context, and symbol name prepended to chunk embeddings for better multi-concept keyword query matching.
- [File-Diversity Re-Rank for `semantic_search`](./file-diversity-rerank.md) — per-file cap on semantic search results prevents a single file from saturating the top-K; overfetch-then-filter preserves score ordering.

- [librarian-mcp — workspace artifact registry](./librarian-mcp.md) — sibling MCP server that indexes markdown artifacts (specs, plans, memories, ADRs, docs) across every repo in a workspace, with filter AST, link graph, and semantic search.
- [Tool description diet & tool guide resource](./tool-description-diet.md) — caps tool descriptions at 300 characters; full usage notes served on demand via `doc://codescout-tool-guide` resource.
- [Global config](./global-config.md) — `~/.config/codescout/config.toml` (XDG) merged with per-project `.codescout/config.toml`; set workspace-wide defaults without touching every project.
