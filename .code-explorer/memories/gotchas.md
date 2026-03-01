# Gotchas & Known Issues

## find_symbol body truncation
- **Problem:** `find_symbol(include_body=true)` uses LSP `workspace/symbol` which returns the *name position* (single line), not the full declaration range. Results in `start_line == end_line` and body containing only the signature.
- **Fix:** Use `list_symbols(path)` first to get correct line ranges, then `find_symbol(name_path=..., include_body=true)` for methods. For top-level functions in large files, `list_functions(path)` gives accurate spans.

## read_file rejects source code
- **Problem:** `read_file` without `start_line`+`end_line` rejects source code files (.rs, .py, .ts, etc.).
- **Fix:** Use symbol tools for source code. Use `read_file` for configs (TOML, JSON, YAML, MD). For targeted source reads, provide line range.

## LSP server startup latency
- **Problem:** First LSP operation for a language can be slow (seconds) while the LSP server initializes and indexes.
- **Fix:** This is expected. Subsequent calls are fast. The `LspManager` caches running servers.

## sqlite-vec loading
- **Problem:** `init_sqlite_vec()` uses `rusqlite::Connection::load_extension` which can fail on systems without the shared library.
- **Fix:** The `bundled` feature on rusqlite includes libsqlite3. sqlite-vec is loaded via `sqlite_vec::sqlite3_vec_init`.

## Worktree write guard
- **Problem:** After `EnterWorktree`, write tools are blocked until `activate_project` is called with the worktree path.
- **Fix:** Always call `activate_project("/path/to/worktree")` after entering a worktree.

## OutputGuard max_results vs max_files
- **Problem:** `cap_items` and `cap_files` are separate — some tools cap items (search results), others cap files (symbol listings). Using the wrong one produces incorrect overflow info.
- **Fix:** Check existing tools in the same category for which cap method they use.

## Tool misbehavior log
- **Problem:** Tool bugs are easy to forget.
- **Fix:** Always check and update `docs/TODO-tool-misbehaviors.md` before and during work.
