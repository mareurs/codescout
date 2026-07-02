# Release Readiness Checklist

Tracking remaining hardening and documentation work items for codescout.
Superseded framing: this checklist predates codescout's actual release
history (0.1.0 shipped 2026-02-25; current version is 0.15.0 per
`Cargo.toml`) — "first public release" no longer describes reality.

## Completed

- [x] **Path sandboxing** — Read/write split with deny-list for sensitive paths, write-restricted to project root (`src/util/path_security.rs`)
- [x] **Shell command hardening** — Mode config (warn/unrestricted/disabled), output truncation at 100KB
- [x] **Regex size limits** — 1MB cap via RegexBuilder on SearchForPattern/ReplaceContent
- [x] **HTTP transport auth** — `--auth-token` CLI flag, auto-generation, bind-address warnings
- [x] **Memory store path traversal fix** — Absolute path bypass in `topic_path()`
- [x] **Test coverage boost** — 227 → 296 tests across agent, server, memory, file, path security, shell, regex
- [x] **Tool access controls** — Per-category enable/disable in `[security]` config (shell disabled by default)
- [x] **Librarian project-model redesign (schema v6)** — Catalog now keys artifacts by absolute path instead of `(repo, rel_path)`. First launch on an existing catalog triggers an automatic migration that backfills `abs_path`/`git_root` and creates a `catalog.db.pre-v6-bak.<ts>` backup before dropping legacy columns. `workspace.toml` `[[roots]]` is deprecated (still parsed for the migration; emits a boot warning). New scope ladder: `scope=project|repo|umbrella|all` resolves against the host's active project path. See `docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md`.

- [x] **CreateFile overwrite protection** — `create_file` rejects existing paths unless `overwrite: true` is passed (default `false`). Schema documents the param; tool description says *"Refuses to overwrite an existing file unless `overwrite: true` is passed."* Implementation at `src/tools/create_file.rs::call` checks `if !overwrite && resolved.exists() { return Err(...) }`.

- [x] **CI pipeline** — GitHub Actions workflow at `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (3×3 matrix: linux/macos/windows × default/local-embed/no-features), `tool-docs-sync`, `msrv` (1.82), and `audit-doc-refs` (informational) on every PR + push to `master`/`experiments`. Push triggers fixed from the placeholder `main` branch in the same change.

- [x] **CI gate: `audit-doc-refs` at `--fail-on high`** — shipped 2026-05-24 (`2dcaff2a`, closed H-5 in `docs/trackers/codescout-usage-hookify.md`). This item sat unchecked here for 5+ weeks after landing — verify against `.github/workflows/ci.yml` before trusting an open checklist item in this file again.
- [x] **Per-connection HTTP token validation** — `src/server.rs` HTTP transport wraps `/mcp` in a bearer-auth `axum::middleware::from_fn` layer, constant-time-comparing the `Authorization` header on every request; a mismatch returns 401. Was already true when this item was written; never checked off.
- [x] **`CHANGELOG.md`** — already existed (583 lines, versioned history back to 0.1.0) when this item was written; the actual gap was that `[Unreleased]` hadn't been updated for the current promotion. Populated 2026-07-02.
- [x] **Security documentation** — `docs/SECURITY.md` added 2026-07-02: vulnerability reporting process, supported versions, and an honest threat-model table including two known-open limitations (dashboard has no built-in auth; `default` security profile is a deny-list, not a containment sandbox). Configuration detail stays in `docs/manual/src/concepts/security.md`, which also had a stale `denied_read_patterns` reference (a config field removed in `docs/plans/archive/2026-03-20-phase1-security-profiles.md`) corrected in the same pass.
## High Priority

- [ ] **Integration test: path security through MCP** — End-to-end test via `call_tool` flow (server → tool → path validation → error). Current tests validate the path_security module in isolation; need to confirm wiring through the server layer.
- [ ] **Error message path sanitization** — Error messages leak full filesystem paths (home dirs, mount points) back to the LLM. Relativize paths to project root in tool error responses.

## Medium Priority

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
profile = "default"                # "default" (deny-list applies) | "root" (deny-list skipped for absolute reads)
file_write_enabled = true          # File creation and modification (default: true)
indexing_enabled = true            # Semantic search indexing (default: true)
shell_command_mode = "warn"        # "warn" | "unrestricted" | "disabled" (default: "warn")
extra_write_roots = []             # Additional directories allowing writes
shell_dangerous_patterns = []      # Additional regexes to flag as dangerous shell commands
write_lock_timeout_secs = 5        # Cross-process write-lock wait before RecoverableError
max_index_bytes = 524288000        # ~500MB — above this, index(action="build") requires confirmation
```

`github_enabled` and `denied_read_patterns` (previously listed here) do not
exist in the current `SecuritySection` struct (`src/config/project.rs`) —
removed from this file 2026-07-02. See `docs/SECURITY.md` for the full
threat model.
