# Domain Glossary

**OutputGuard** — Progressive disclosure controller used by all variable-output tools.
Reads `detail_level`, `offset`, `limit` from tool input and enforces item/file caps.
See `src/tools/output.rs` and `docs/PROGRESSIVE_DISCOVERABILITY.md`.

**OverflowInfo** — JSON shape emitted when results are capped: `{ shown, total, hint,
next_offset?, by_file?, by_file_overflow }`. The `by_file` array is always JSON array
(not object) per `docs/TODO-by-file-serialization.md` decision.

**RecoverableError** — Tool error that routes to `isError: false` so Claude Code does
not abort sibling parallel calls. See `src/tools/mod.rs:78` and `CLAUDE.md § Key Patterns`.

**OutputBuffer** — Session-scoped LRU buffer (50 slots) for large tool output. Assigns
`@tool_xxx` ref IDs. Different from `@cmd_xxx` refs (run_command) and `@file_xxx` refs
(read_file). See `src/tools/output_buffer.rs` and `MEMORY.md § run_command Redesign`.

**ActiveProject** — Struct inside `Agent` holding the project root, config, both memory
stores, library registry, and dirty file tracking. All tools access via `ctx.agent.with_project(|p| ...)`.

**LspProvider / LspClientOps** — Traits in `src/lsp/ops.rs` abstracting LSP access.
`LspManager` is the production impl; `MockLspProvider` / `MockLspClient` for tests.

**StartingCleanup** — RAII guard in `LspManager::do_start` that removes the per-language
barrier from `self.starting` on any exit path, including async cancellation.

**Scope** — Enum controlling which project a symbol/semantic tool searches:
`Project` (default), `Library(name)`, `Libraries` (all registered), `All`.
Parsed from the `scope` string parameter.

**drift** — Cosine distance score between old and new embeddings for a code chunk after
re-indexing. High drift (≥ `staleness_drift_threshold`) triggers memory anchor staleness.

**anchor sidecar** — `.anchors.toml` file alongside each memory topic. Tracks source file
paths referenced in the memory content, with SHA-256 hashes for staleness detection.

**name_path** — Hierarchical symbol identifier used in LSP tools: `Struct/method`,
`impl Block/method`. The separator is `/`. Used in `find_symbol(name_path=...)`,
`replace_symbol`, `rename_symbol`, etc.

**tool_timeout_secs** — Per-project config in `.codescout/project.toml` controlling
how long `call_tool_inner` waits before timing out a tool call. Skipped for slow tools
like `index_project` and `onboarding` (`tool_skips_server_timeout` in server.rs).

**run_gh** — Internal helper in `src/tools/github.rs` that shells to the `gh` CLI
subprocess. All 5 GitHub tools use this — they do NOT call the GitHub REST API directly.

**classify_bucket** — Keyword heuristic in `src/memory/classify.rs` that auto-classifies
semantic memories into buckets (code/system/preferences/unstructured) when the agent
doesn't specify one explicitly.
