# Gotchas & Known Issues

See `docs/TODO-tool-misbehaviors.md` for the complete living log (BUG-001 through BUG-025+).
This captures structural gotchas not listed there.

## MCP Client Sends Booleans as JSON Strings (Critical)

Claude Code's MCP client serializes boolean tool parameters as **JSON strings**, not native
JSON booleans. `input["force"]` arrives as `Value::String("true")`, not `Value::Bool(true)`.

**Impact:** `val.as_bool()` returns `None` for strings → silently defaults to `false`.

**Fix (applied):** Use `parse_bool_param()` from `src/tools/mod.rs:159` at every boolean
input site. Applied across all tool files (37 sites, commit `03382cc`, 2026-03-06):
`workflow.rs`, `file.rs`, `symbol.rs`, `semantic.rs`, `library.rs`, `memory.rs`, `github.rs`.

```rust
let force = parse_bool_param(&input["force"]);
```

**Rule:** Never use `.as_bool().unwrap_or(false)` on tool inputs.

## GitHub Tools Use `gh` CLI (Not HTTP)

All `github_identity`, `github_issue`, `github_pr`, `github_file`, `github_repo` tools
shell out to the `gh` CLI (`src/tools/github.rs::run_gh()`). If `gh` is not installed or
not authenticated, they silently return an error.

## sqlite-vec Is NOT Active

`sqlite-vec` is in `Cargo.toml` and `rusqlite` is "bundled", but the extension loading
in `src/embed/index.rs` is commented out (TODO). Semantic search uses pure-Rust cosine
similarity that loads all embeddings into memory. Large indexes can be slow/OOM.
Verify current state: `search_pattern("is_vec0_active", path="src/embed/index.rs")`.

## `find_symbol(include_body=true)` — FIXED (was: body truncation)

~~`workspace/symbol` returns a single-line name position, `start_line == end_line`.~~

**Fixed:** `validate_symbol_range()` now detects degenerate ranges and falls back to
`resolve_range_via_document_symbols()` to get the real declaration range from
`textDocument/documentSymbol`. See `src/tools/symbol.rs:260` and `src/tools/symbol.rs:820`.
No workaround needed.

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
