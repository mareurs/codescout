# Experimental Features

> These features are available on `master` and the `experiments` branch.
> APIs and behaviour may change without notice. When a feature graduates to
> stable, its page moves into the main manual.

## Available Features

- [Asymmetric query prefix for embedding models](./asymmetric-query-prefix.md) — automatic query-side prefix for models trained asymmetrically (e.g. CodeRankEmbed), restoring `semantic_search` recall without user configuration.
- [Global config](./global-config.md) — `~/.config/codescout/config.toml` (XDG) merged with per-project `.codescout/config.toml`; set workspace-wide defaults without touching every project.
- [Index Scope Guard](./index-scope-guard.md) — confirmation prompt before `index_project` walks home/system directories or oversized trees.
- [librarian-mcp — workspace artifact registry](./librarian-mcp.md) — sibling MCP server that indexes markdown artifacts (specs, plans, memories, ADRs, docs) across every repo in a workspace, with filter AST, link graph, and semantic search.
- [Metadata-Enriched Chunks](./metadata-enriched-chunks.md) — file path, container context, and symbol name prepended to chunk embeddings for better multi-concept keyword query matching.
