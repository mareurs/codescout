# Gotchas & Known Issues

See `docs/TODO-tool-misbehaviors.md` for the complete living log (BUG-001 through BUG-025+).
This captures structural gotchas not listed there.

## GitHub Tools Use `gh` CLI (Not HTTP)

All `github_identity`, `github_issue`, `github_pr`, `github_file`, `github_repo` tools
shell out to the `gh` CLI (`src/tools/github.rs::run_gh()`). If `gh` is not installed or
not authenticated, they silently return an error. Not documented in ARCHITECTURE.md.

## sqlite-vec Is NOT Active

`sqlite-vec` is in `Cargo.toml` and `rusqlite` is "bundled", but the extension loading
in `src/embed/index.rs` is commented out (TODO). Semantic search uses pure-Rust cosine
similarity that loads all embeddings into memory. Large indexes can be slow/OOM.
Verify current state: `search_pattern("is_vec0_active", path="src/embed/index.rs")`.

## Tool Count Is 28, Not 23

`CLAUDE.md` says "23 tools registered" — outdated. Check `src/server.rs::from_parts()` for
the actual list. The discrepancy grew as GitHub tools were added without updating CLAUDE.md.

## `find_symbol(include_body=true)` Truncation Bug

`workspace/symbol` returns a single-line name position, not the full declaration range.
Result: `start_line == end_line`, body contains only the signature.
**Workaround:** `list_symbols(path)` for line ranges → `read_file(path, start_line, end_line)`.
See `MEMORY.md` for the detailed workaround pattern.

## `replace_symbol` / `remove_symbol` LSP Range Quirks

LSP sometimes reports `start_line` pointing at the closing `}` of the *preceding* method.
codescout trusts LSP ranges verbatim ("trust LSP" design decision, BUG-013 fix).
This means replacing a symbol can eat the preceding closing brace. See `tests/symbol_lsp.rs`
for the regression tests that document this accepted behavior.

## `system-prompt.md` Takes Precedence Over TOML

`.codescout/system-prompt.md` is read by `agent::project_status()` first; `project.toml`'s
`project.system_prompt` field is only used as fallback. Confirmed in agent tests.

## Worktree Write Safety

`guard_worktree_write()` in `src/tools/mod.rs` checks that write tools aren't targeting
a stale worktree path after `activate_project`. Must call after any `activate_project`
in tests that write files.

## Panic Policy: `abort` in Release

`Cargo.toml` sets `panic = "abort"` for the release profile. Any panic kills the MCP
server immediately (no unwind). In dev/test builds, panics unwind normally.
