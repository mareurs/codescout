# Domain Glossary

**Exploring mode** — Default output mode. Compact, capped at 200 items, no bodies. Controlled by `OutputGuard`. Use for initial navigation.

**Focused mode** — Full-detail output mode. Activated via `detail_level: "full"`. Supports offset/limit pagination. Use after identifying targets.

**OutputGuard** — Struct in `src/tools/output.rs` that enforces output sizing across all tools. Parses `detail_level`, `offset`, `limit` from tool input and caps/paginates results.

**RecoverableError** — Non-fatal error type in `src/tools/mod.rs`. Produces `isError: false` in MCP response so Claude Code doesn't abort sibling parallel tool calls. Includes a `hint` field to guide the LLM.

**Progressive disclosure** — Design principle: show minimal information first, let the agent drill down. Documented in `docs/PROGRESSIVE_DISCOVERABILITY.md`.

**Scope** — Enum in `src/library/scope.rs` controlling search breadth: `Project` (default), `Libraries`, `All`, or `Lib(name)` for a specific library.

**Library discovery** — When `goto_definition` returns a path outside the project root, the library is auto-registered in `LibraryRegistry` (`src/library/registry.rs`). Can then be navigated and indexed.

**Drift detection** — Semantic change measurement in `src/embed/drift.rs`. After re-indexing, compares old vs new embeddings to detect meaningful code changes (not just byte changes). Surfaced via `index_status(threshold)`.

**Staleness** — How far behind the embedding index is from HEAD. Measured in commits since last indexed commit. Checked via `check_index_staleness()` in `src/embed/index.rs`.

**ChangeSet** — Struct in `src/embed/index.rs:513` tracking which files need re-embedding. Built via `find_changed_files()` using git diff → mtime → SHA-256 fallback chain.

**name_path** — Hierarchical symbol identifier like `MyStruct/my_method`. Used in `find_symbol`, `replace_symbol`, `insert_code` to target nested symbols unambiguously.

**ToolContext** — Shared state passed to every tool call. Contains `agent` (project state), `lsp` (LSP provider), `output_buffer` (for run_command output storage).

**Output buffer** — Storage for large `run_command` output. Returns an `@output_id` reference that can be queried with Unix tools: `grep FAILED @cmd_abc123`.

**Three-query sandwich** — Testing pattern for cache invalidation: baseline query → mutate → assert stale → invalidate → assert fresh. See `did_change_refreshes_stale_symbol_positions`.
