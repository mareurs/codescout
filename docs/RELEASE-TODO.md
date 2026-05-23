# Release Readiness Checklist

Tracking remaining work items for the first public release of codescout.

## Completed

- [x] **Path sandboxing** — Read/write split with deny-list for sensitive paths, write-restricted to project root (`src/util/path_security.rs`)
- [x] **Shell command hardening** — Mode config (warn/unrestricted/disabled), output truncation at 100KB
- [x] **Regex size limits** — 1MB cap via RegexBuilder on SearchForPattern/ReplaceContent
- [x] **HTTP transport auth** — `--auth-token` CLI flag, auto-generation, bind-address warnings
- [x] **Memory store path traversal fix** — Absolute path bypass in `topic_path()`
- [x] **Test coverage boost** — 227 → 296 tests across agent, server, memory, file, path security, shell, regex
- [x] **Tool access controls** — Per-category enable/disable in `[security]` config (shell disabled by default)
- [x] **Librarian project-model redesign (schema v6)** — Catalog now keys artifacts by absolute path instead of `(repo, rel_path)`. First launch on an existing catalog triggers an automatic migration that backfills `abs_path`/`git_root` and creates a `catalog.db.pre-v6-bak.<ts>` backup before dropping legacy columns. `workspace.toml` `[[roots]]` is deprecated (still parsed for the migration; emits a boot warning). New scope ladder: `scope=project|repo|umbrella|all` resolves against the host's active project path. See `docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md`.

- [x] **CreateFile overwrite protection** — `create_file` rejects existing paths unless `overwrite: true` is passed (default `false`). Schema documents the param; tool description says *"Refuses to overwrite an existing file unless `overwrite: true` is passed."* Implementation at `src/tools/create_file.rs::call` checks `if !overwrite && resolved.exists() { return Err(...) }`.## High Priority

- [ ] **CI pipeline** — GitHub Actions workflow running `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` on every PR. Single biggest protection against bad contributions.
- [ ] **Integration test: path security through MCP** — End-to-end test via `call_tool` flow (server → tool → path validation → error). Current tests validate the path_security module in isolation; need to confirm wiring through the server layer.
- [ ] **Error message path sanitization** — Error messages leak full filesystem paths (home dirs, mount points) back to the LLM. Relativize paths to project root in tool error responses.

## Medium Priority

- [ ] **Per-connection token validation for HTTP** — Currently the auth token is generated and displayed but not validated per-request (blocked on rmcp middleware support). Track rmcp upstream for SSE auth hooks.
- [ ] **CHANGELOG.md** — Document security features, breaking changes, and migration guide for users upgrading from development builds.
- [ ] **Security documentation** — README section or dedicated `docs/SECURITY.md` explaining the threat model, what's protected, and how to configure `[security]` in `project.toml`.
- [ ] **Fuzz testing for path validation** — The path security module handles untrusted input from LLMs. Fuzz `validate_read_path` and `validate_write_path` with random/adversarial inputs to find edge cases.

## Low Priority (Post-v1)

- [ ] **Rate limiting** — Throttle tool calls to prevent runaway LLM loops. Configurable per-tool or global rate.
- [ ] **Audit logging** — Log all tool invocations (tool name, args, result status) to a file for post-incident review.
- [ ] **ActivateProject restrictions** — Currently accepts any directory. Consider requiring a marker file (`.git/`, `.codescout/`, `Cargo.toml`) to prevent activating arbitrary system directories.
- [ ] **Symlink deny-list for writes** — While `canonicalize()` catches symlinks pointing outside the project, consider an explicit `follow_symlinks: false` option for extra safety.
- [ ] **Content-type validation for file writes** — Prevent writing binary/executable content through `create_file` (e.g., reject files with null bytes or shebang lines to unexpected paths).

## Configuration Reference

All security settings live in `.codescout/project.toml` under `[security]`:

```toml
[security]
# Tool category toggles
shell_enabled = false              # Shell command execution (default: false)
file_write_enabled = true          # File creation and modification (default: true)
indexing_enabled = true            # Semantic search indexing (default: true)
github_enabled = true              # GitHub API tools (default: true)

# Shell command settings (only relevant if shell_enabled = true)
shell_command_mode = "warn"        # "warn" | "unrestricted" | "disabled"
shell_output_limit_bytes = 102400  # Max output bytes (default: 100KB)

# Path security
denied_read_patterns = []          # Additional paths to block reads from
extra_write_roots = []             # Additional directories allowing writes
```
