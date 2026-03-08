# Gotchas & Known Issues

## Embedding / Semantic Search

- **sqlite-vec loads all embeddings into memory**: `sqlite-vec` extension loading is
  commented out in `src/embed/index.rs`. Pure-Rust cosine search scans ALL chunk
  embeddings in memory per query. Verify current state at `src/embed/index.rs` before
  assuming KNN is active.

- **Semantic index 7+ commits behind**: The `git_sync` warning in `semantic_search`
  results is expected during active development. Results from existing chunks are still
  valid; only newly added code is missing.

## GitHub Tools

- **Requires `gh` CLI**: All 5 GitHub tools (`github_identity`, `github_issue`, etc.)
  shell to `gh` via `run_gh()` in `src/tools/github.rs`. If `gh` is not installed or
  not authenticated, every GitHub tool call fails with a non-recoverable error.

## LSP

- **Kotlin LSP multi-session conflict**: kotlin-lsp ≤ v0.253 uses an MVStore index
  that only allows one session. If VS Code is open on the same project, `code -32800`
  errors occur. `route_tool_error` catches this and provides the hint. Upgrade to v261+.

- **LSP cold start latency**: First call for a language spawns the LSP server. This can
  take 1–5 minutes for large projects (Kotlin/Java). The watch-channel barrier in
  `LspManager` deduplicates concurrent starts, but the first caller still blocks.

## run_command Security

- **Source file access blocked in run_command**: `check_source_file_access()` in
  `path_security.rs` blocks commands like `cat src/foo.rs` or `grep pattern src/foo.rs`.
  Use `read_file` / `search_pattern` / `find_symbol` instead. Check
  `SOURCE_EXTENSIONS` constant for the current blocked extension list.

## Parallel Writes

- **Never dispatch parallel write calls**: See `MEMORY.md § Parallel Write Safety (BUG-021)`.
  rmcp 0.1.5 has a cancellation race that can crash the server if a parallel write is
  denied in the permission dialog. Always wait for one write to finish before starting
  the next.

## OutputBuffer

- **LRU eviction**: OutputBuffer holds 50 entries. In a long session with many large
  tool calls, early `@tool_xxx` refs may be evicted. If you get "buffer not found",
  re-run the original tool call.

## Memory Staleness

- **Stale memory check is opt-in**: `project_status` shows `memory_staleness` with
  `stale`, `fresh`, and `untracked` entries. Only memories with `.anchors.toml` sidecars
  are tracked. Old memories written before the anchor system may show as `untracked`.

## Tool Docs Sync

- **CI enforces docs/manual sync**: The `tool-docs-sync` CI job diffs actual tool names
  (from `fn name(&self)`) against `docs/manual/src/tools/*.md`. Adding a tool without
  updating docs will fail CI.

## Rust std::path

- **`Path::file_stem()` does NOT return `None` for dotfiles**: On a path like `.hidden`,
  Rust treats the entire name as the stem — `file_stem()` returns `Some(".hidden")` and
  `extension()` returns `None`. `file_stem()` only returns `None` when `file_name()`
  itself is `None` (e.g. paths ending in `..`), which never occurs in `read_dir` output.
  Consequence: a guard like `let Some(stem) = path.file_stem() else { continue }` placed
  after an `extension() == "md"` filter is unreachable dead code — dotfiles are already
  excluded by the extension filter. Keep such guards with a `// Defensive: unreachable
  from read_dir` comment rather than removing them. See `src/memory/anchors.rs:183`.
