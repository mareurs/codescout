# Git Suite Cleanup: Dead Code, Security Gate, Triggering Guidance

**Date:** 2026-03-12
**Status:** Draft
**Scope:** Local git tools, GitHub MCP tools, security configuration, server instructions

## Problem Statement

An audit of all git-related tools in codescout revealed three categories of issues:

1. **Dead code**: The `git_blame` tool is fully implemented but not registered as an MCP tool. Its backing library (`src/git/blame.rs`), its config flag (`git_enabled`), and documentation references are all orphaned. The manual already says it was removed in v1 — but the code was never cleaned up. Note: `git_enabled` has no match arm in `check_tool_access` — it was never wired in. The field exists on both `PathSecurityConfig` and `SecurityConfig` but gates nothing.

2. **Security gap**: All 5 GitHub tools (`github_identity`, `github_issue`, `github_pr`, `github_file`, `github_repo`) shell out to `gh` via `run_gh()` which calls `tokio::process::Command::new("gh")` directly. They have no match arm in `check_tool_access` — they fall through to the `_ => {}` wildcard. Setting `shell_enabled = false` does NOT prevent GitHub API calls. There is no `github_enabled` flag.

3. **Triggering gaps**: The server instructions correctly redirect `run_command("gh ...")` to GitHub tools (anti-pattern table) and already have GitHub rows in the "By task" table. However, the local vs remote git distinction (`run_command("git log")` for local history vs `github_repo(list_commits)` for GitHub API) is never made explicit.

## Design

### Phase 1: Dead Code Removal

**Delete entirely:**
- `src/tools/git.rs` — `GitBlame` struct, `impl Tool`, `format_git_blame`, all 9 tests
- `src/git/blame.rs` — `blame_file()`, `committed_content()`, all tests

**Remove lines:**
- `src/tools/mod.rs` — remove `pub mod git;`
- `src/git/mod.rs` — remove `pub mod blame;` (keep `open_repo`, `DiffEntry`, `DiffStatus`, `diff_tree_to_tree` — used by embed engine)
- `src/server.rs` — remove `"git_blame"` from the `other_tools_do_not_skip_server_timeout` test (line ~802). This test references a tool name that was never registered, making it vacuously pass.

**Remove `git_enabled` config flag:**
- `src/util/path_security.rs`:
  - Remove `pub git_enabled: bool` field from `PathSecurityConfig`
  - Remove `git_enabled: true` from `Default` impl
  - Remove `config.git_enabled = false;` from the all-disabled test
- `src/config/project.rs`:
  - Remove `pub git_enabled: bool` field (with its `#[serde]` attribute and doc comment)
  - Remove `git_enabled: true` from `default_for()`
  - Remove `git_enabled: self.git_enabled` from `Into<PathSecurityConfig>` impl
  - Remove 3 test assertions referencing `git_enabled`

**Serde backward compatibility:** serde ignores unknown fields by default (no `deny_unknown_fields` on the struct). Existing `.codescout/project.toml` files with `git_enabled = true` will have it silently ignored — no breakage.

**Fix stale docs (all files referencing `git_enabled`):**
- `docs/ARCHITECTURE.md` — Git Engine section references `file_log()`, `head_short_sha()`, `CommitSummary` which do not exist. Update to: `open_repo()`, `diff_tree_to_tree()`, `DiffEntry`, `DiffStatus`.
- `docs/manual/src/troubleshooting.md` (lines ~366, 373) — remove/replace `git_enabled` troubleshooting advice
- `docs/manual/src/architecture.md` (line ~70) — remove "If `git_enabled` is false, git tools are blocked"
- `docs/manual/src/tools/workflow-and-config.md` (line ~224) — remove `git_enabled` from JSON example
- `docs/manual/src/concepts/security.md` (line ~160) — remove `git_enabled` from TOML example
- `docs/manual/src/configuration/project-toml.md` (lines ~134, 146, 242) — remove `git_enabled` from config table and examples
- `docs/RELEASE-TODO.md` (line ~46) — remove `git_enabled` from example
- `docs/manual/src/tools/git.md` — review; currently says "removed in v1" which is fine, but should not imply the dead code still exists as fallback

**Mark historical bugs as resolved:**
- `docs/TODO-tool-misbehaviors.md` — BUG-017 (`git_blame` subdirectory bug) should be marked as resolved-by-removal

**What stays untouched:**
- `server_instructions.md` — already correct (directs to `run_command` for local git)
- `src/git/mod.rs` internals (open_repo, diff_tree_to_tree) — active, used by embed engine

### Phase 2: GitHub Security Gate

**Add `github_enabled` flag:**
- `src/config/project.rs` — add `pub github_enabled: bool` with `#[serde(default = "default_true")]` to the security section, default true
- `src/util/path_security.rs` — add `pub github_enabled: bool` to `PathSecurityConfig`, default true

**Wire into `check_tool_access`:**
```rust
"github_identity" | "github_issue" | "github_pr" | "github_file" | "github_repo" => {
    if !config.github_enabled {
        bail!(
            "GitHub tools are disabled. Set security.github_enabled = true in .codescout/project.toml to enable."
        );
    }
}
```

**Add tests:**
- `github_disabled_blocks_all_github_tools` — set `github_enabled = false`, verify all 5 tools are blocked
- `github_enabled_allows_github_tools` — default config, verify all 5 pass
- Add `config.github_enabled = false` to the existing all-disabled test (replacing the removed `config.git_enabled = false` line)
- Verify `check_tool_access_error_message_includes_config_hint` works for github tools

**Update docs to include `github_enabled`:**
- `docs/manual/src/concepts/security.md` — add `github_enabled` to TOML example
- `docs/manual/src/configuration/project-toml.md` — add `github_enabled` to config table and examples
- `docs/RELEASE-TODO.md` — add `github_enabled` to example

### Phase 3: Code Quality Fixes

**Duplicate `#[async_trait]` on ALL 5 GitHub tool impls:**

| Tool | Lines | Duplicates |
|---|---|---|
| `GithubIdentity` | ~142-143 | 2x → 1x |
| `GithubIssue` | ~215-217 | 3x → 1x |
| `GithubPr` | ~456-458 | 3x → 1x |
| `GithubFile` | ~830-831 | 2x → 1x |
| `GithubRepo` | ~1084-1085 | 2x → 1x |

**5x stacked `#[cfg(test)]`** (lines ~1361-1365):
- Reduce to single `#[cfg(test)]` before `mod tests`.

### Phase 4: Server Instructions Improvements

The "By task" table already has GitHub tool rows (identity, issues, PRs, files, repo). The anti-pattern table already redirects `run_command("gh ...")` to GitHub tools.

**What's missing is the local vs remote git distinction.** Add to the "By task" table:

| Task | Tool | NOT this |
|---|---|---|
| Local git history (blame, log, diff) | `run_command("git blame/log/diff ...")` | ~~`github_repo(list_commits)`~~ |
| GitHub commit history (remote) | `github_repo(method="list_commits", ...)` | ~~`run_command("git log")`~~ when you need remote context |

**Add anti-pattern:**

| Never do this | Do this instead | Why |
|---|---|---|
| `github_repo(list_commits)` for local file history | `run_command("git log src/foo.rs")` | Local git is faster, has full history; GitHub API is paginated and rate-limited |

## Files Changed (Summary)

| File | Phase | Change |
|---|---|---|
| `src/tools/git.rs` | 1 | **Delete** |
| `src/git/blame.rs` | 1 | **Delete** |
| `src/tools/mod.rs` | 1 | Remove `pub mod git;` |
| `src/git/mod.rs` | 1 | Remove `pub mod blame;` |
| `src/server.rs` | 1 | Remove `"git_blame"` from timeout test |
| `src/util/path_security.rs` | 1+2 | Remove `git_enabled`, add `github_enabled` + arm in `check_tool_access` + tests |
| `src/config/project.rs` | 1+2 | Remove `git_enabled`, add `github_enabled` + serde + default + tests |
| `docs/ARCHITECTURE.md` | 1 | Fix Git Engine section (phantom functions) |
| `docs/manual/src/troubleshooting.md` | 1 | Remove `git_enabled` troubleshooting section |
| `docs/manual/src/architecture.md` | 1 | Remove `git_enabled` reference |
| `docs/manual/src/tools/workflow-and-config.md` | 1 | Remove `git_enabled` from JSON example |
| `docs/manual/src/concepts/security.md` | 1+2 | Replace `git_enabled` with `github_enabled` |
| `docs/manual/src/configuration/project-toml.md` | 1+2 | Replace `git_enabled` with `github_enabled` in table + examples |
| `docs/RELEASE-TODO.md` | 1+2 | Replace `git_enabled` with `github_enabled` |
| `docs/manual/src/tools/git.md` | 1 | Review for stale impl references |
| `docs/TODO-tool-misbehaviors.md` | 1 | Mark BUG-017 as resolved-by-removal |
| `src/tools/github.rs` | 3 | Remove duplicate `#[async_trait]` (all 5 impls) and stacked `#[cfg(test)]` |
| `src/prompts/server_instructions.md` | 4 | Add local vs remote git rows + anti-pattern |

## Verification

After all phases:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

No test should reference `git_blame` or `git_enabled` after the change. New tests for `github_enabled` must pass. Grep all three prompt surfaces for stale tool name references. Grep docs for stale `git_enabled` references.

## Out of Scope

- Adding new git MCP tools (git_log, git_diff, etc.) — local git through `run_command` is the intended architecture
- Changing how `run_gh` works internally (e.g., running `gh` through `run_command`'s security pipeline) — the `check_tool_access` gate is sufficient
- Dashboard-related git usage — feature-gated, separate concern
