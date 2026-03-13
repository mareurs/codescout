# Git Suite Cleanup Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove dead `git_blame` code, add a `github_enabled` security gate for the 5 GitHub tools, fix code quality issues, and improve server instruction triggering for local vs remote git.

**Architecture:** Four independent phases — dead code removal, security gate, code quality, and docs — each producing a clean commit. Phase 2 depends on Phase 1 (both touch the same config structs), but Phases 3 and 4 are independent of each other.

**Tech Stack:** Rust, serde/toml config, MCP server instructions (markdown)

**Spec:** `docs/superpowers/specs/2026-03-12-git-suite-cleanup-design.md`

---

## Chunk 1: Dead Code Removal (Phase 1)

### Task 1: Delete `git_blame` tool and backing library

**Files:**
- Delete: `src/tools/git.rs`
- Delete: `src/git/blame.rs`
- Modify: `src/tools/mod.rs:12` — remove `pub mod git;`
- Modify: `src/git/mod.rs:3` — remove `pub mod blame;`

- [ ] **Step 1: Delete `src/tools/git.rs`**

Remove the entire file. Contains `GitBlame` struct, `impl Tool for GitBlame`, `format_git_blame`, and 9 tests. None of this is referenced elsewhere — the tool was never registered in `server.rs`.

```bash
rm src/tools/git.rs
```

- [ ] **Step 2: Delete `src/git/blame.rs`**

Remove the entire file. Contains `blame_file()`, `committed_content()`, and tests. Only consumer was `GitBlame` in `src/tools/git.rs`.

```bash
rm src/git/blame.rs
```

- [ ] **Step 3: Remove module declarations**

In `src/tools/mod.rs` line 12, remove:
```rust
pub mod git;
```

In `src/git/mod.rs` line 3, remove:
```rust
pub mod blame;
```

- [ ] **Step 4: Build to verify no dangling references**

```bash
cargo build 2>&1
```
Expected: clean build. If anything references `git::blame` or `tools::git`, it will fail here.

- [ ] **Step 5: Run tests**

```bash
cargo test 2>&1
```
Expected: all tests pass (the deleted tests simply no longer exist).

### Task 2: Remove `git_enabled` config flag

**Files:**
- Modify: `src/util/path_security.rs:80-81,101,939`
- Modify: `src/config/project.rs:113-115,136,169,323,336,348`

- [ ] **Step 1: Remove from `PathSecurityConfig`**

In `src/util/path_security.rs`, remove these lines:

From the struct (line ~80-81):
```rust
    /// Enable git tools (default: true)
    pub git_enabled: bool,
```

From `Default` impl (line ~101):
```rust
            git_enabled: true,
```

From the `read_tools_always_allowed` test (line ~939):
```rust
        config.git_enabled = false;
```

- [ ] **Step 2: Remove from `SecuritySection`**

In `src/config/project.rs`, remove these lines:

From the struct (lines ~113-115):
```rust
    /// Enable git tools: blame, log, diff (default: true)
    #[serde(default = "default_true")]
    pub git_enabled: bool,
```

From `Default` impl (line ~136):
```rust
            git_enabled: true,
```

From `to_path_security_config()` (line ~169):
```rust
            git_enabled: self.git_enabled,
```

- [ ] **Step 3: Remove test assertions**

In `src/config/project.rs`, remove these assertions:

From `security_section_default_enables_write_git_indexing` (line ~323):
```rust
        assert!(sec.git_enabled, "git_enabled should default to true");
```

From `project_config_default_for_enables_write_tools` (line ~336):
```rust
        assert!(cfg.security.git_enabled);
```

From `toml_without_security_section_enables_write_tools` (line ~348):
```rust
        assert!(cfg.security.git_enabled);
```

- [ ] **Step 4: Build and test**

```bash
cargo build 2>&1 && cargo test 2>&1
```
Expected: clean build, all tests pass. Grep to confirm no remaining references:
```bash
grep -r "git_enabled" src/
```
Expected: zero matches.

### Task 3: Remove stale `"git_blame"` from server timeout test

**Files:**
- Modify: `src/server.rs:802`

- [ ] **Step 1: Remove `"git_blame"` from test list**

In `src/server.rs`, the `other_tools_do_not_skip_server_timeout` test (line ~798-811) has this list:
```rust
        for name in &[
            "read_file",
            "edit_file",
            "find_symbol",
            "git_blame",
            "semantic_search",
        ] {
```

Remove `"git_blame",` so it becomes:
```rust
        for name in &[
            "read_file",
            "edit_file",
            "find_symbol",
            "semantic_search",
        ] {
```

- [ ] **Step 2: Run tests**

```bash
cargo test other_tools_do_not_skip 2>&1
```
Expected: PASS.

### Task 4: Fix stale documentation

**Files:**
- Modify: `docs/ARCHITECTURE.md:69-73`
- Modify: `docs/manual/src/troubleshooting.md:360-374`
- Modify: `docs/manual/src/architecture.md:69-70`
- Modify: `docs/manual/src/tools/workflow-and-config.md:224`
- Modify: `docs/manual/src/concepts/security.md:157-163`
- Modify: `docs/manual/src/configuration/project-toml.md:134-146,240-243`
- Modify: `docs/RELEASE-TODO.md:46`
- Modify: `docs/TODO-tool-misbehaviors.md:521-550`

- [ ] **Step 1: Fix `docs/ARCHITECTURE.md` Git Engine section**

Replace lines 71-72:
```markdown
- `mod.rs` — `open_repo()`, `head_short_sha()`, `file_log()` returning `Vec<CommitSummary>` via git2
- `blame.rs` — `blame_file()` returning `Vec<BlameLine>` with author, date, SHA, line content
```

With:
```markdown
- `mod.rs` — `open_repo()`, `diff_tree_to_tree()` returning `Vec<DiffEntry>` via git2 (used by embedding drift detection)
```

- [ ] **Step 2: Fix `docs/manual/src/troubleshooting.md`**

Remove the entire "Git is disabled" block (lines ~366-374):
```markdown
2. **Git is disabled.** The `git_enabled` setting is `false` in the security
   configuration.

   **Fix:** Check `project.toml`:

   ```toml
   [security]
   git_enabled = true   # default is true
   ```
```

- [ ] **Step 3: Fix `docs/manual/src/architecture.md`**

Remove the `git_enabled` sentence at line ~70. Change from:
```markdown
   here. If `git_enabled` is false, git tools are blocked.
```
To (just delete the sentence — `github_enabled` reference will be added in Phase 2):
```markdown
   here.
```

- [ ] **Step 4: Review `docs/manual/src/tools/git.md`**

This file already says "removed in v1" which is correct. Verify it does not reference source code paths (`src/tools/git.rs`, `src/git/blame.rs`) that no longer exist. If it does, remove those references.

- [ ] **Step 5: Fix `docs/manual/src/tools/workflow-and-config.md`**

In the JSON example at line ~224, remove `"git_enabled": true, ` from the security object:
```json
"security": { "shell_command_mode": "warn", "shell_output_limit_bytes": 102400, "shell_enabled": false, "file_write_enabled": true, "indexing_enabled": true }
```

- [ ] **Step 6: Fix `docs/manual/src/concepts/security.md`**

In the TOML example at lines ~157-163, remove:
```toml
git_enabled        = true    # git operations via run_command
```

- [ ] **Step 7: Fix `docs/manual/src/configuration/project-toml.md`**

In the TOML example at line ~134, remove:
```toml
git_enabled = true
```

In the config table at line ~146, remove the entire row:
```markdown
| `git_enabled` | bool | `true` | Enables git operations via `run_command`. |
```

In the second TOML example at line ~242, remove:
```toml
git_enabled = true
```

- [ ] **Step 8: Fix `docs/RELEASE-TODO.md`**

At line ~46, remove:
```toml
git_enabled = true                 # Git blame/log/diff (default: true)
```

- [ ] **Step 9: Mark BUG-017 as resolved-by-removal in `docs/TODO-tool-misbehaviors.md`**

The BUG-017 entry is inside the "Template for new entries" section (lines ~521-550). Update its status line from:
```
**Status:** ✅ FIXED — `blame_file` now computes the repo-relative path...
```
To:
```
**Status:** ✅ RESOLVED (removed) — `git_blame` tool and `src/git/blame.rs` deleted in git suite cleanup (2026-03-12). The tool was never registered as an MCP tool; all git operations go through `run_command`.
```

- [ ] **Step 10: Commit Phase 1**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
grep -r "git_enabled" src/  # expect zero matches
grep -r "git_blame" src/    # expect zero matches (only server.rs timeout test was the last one)
git add -A
git commit -m "refactor: remove dead git_blame tool, git/blame.rs, and orphaned git_enabled config"
```

---

## Chunk 2: GitHub Security Gate (Phase 2)

### Task 5: Add `github_enabled` flag to config

**Files:**
- Modify: `src/config/project.rs` — `SecuritySection` struct + `Default` + `to_path_security_config()`
- Modify: `src/util/path_security.rs` — `PathSecurityConfig` struct + `Default`

- [ ] **Step 1: Add to `SecuritySection` in `src/config/project.rs`**

After the `indexing_enabled` field, add:
```rust
    /// Enable GitHub tools: github_identity, github_issue, github_pr, github_file, github_repo (default: true)
    #[serde(default = "default_true")]
    pub github_enabled: bool,
```

In `Default for SecuritySection`, add after `indexing_enabled: true,`:
```rust
            github_enabled: true,
```

In `to_path_security_config()`, add after `indexing_enabled: self.indexing_enabled,`:
```rust
            github_enabled: self.github_enabled,
```

- [ ] **Step 2: Add to `PathSecurityConfig` in `src/util/path_security.rs`**

After the `indexing_enabled` field (~line 83), add:
```rust
    /// Enable GitHub tools (default: true)
    pub github_enabled: bool,
```

In `Default for PathSecurityConfig`, add after `indexing_enabled: true,`:
```rust
            github_enabled: true,
```

- [ ] **Step 3: Build to verify struct changes compile**

```bash
cargo build 2>&1
```
Expected: clean build.

### Task 6: Wire `github_enabled` into `check_tool_access`

**Files:**
- Modify: `src/util/path_security.rs` — `check_tool_access` function

- [ ] **Step 1: Write failing tests first**

Add these tests in the `tests` module of `src/util/path_security.rs`:

```rust
    #[test]
    fn github_disabled_blocks_all_github_tools() {
        let mut config = PathSecurityConfig::default();
        config.github_enabled = false;
        for tool in &[
            "github_identity",
            "github_issue",
            "github_pr",
            "github_file",
            "github_repo",
        ] {
            assert!(
                check_tool_access(tool, &config).is_err(),
                "{} should be blocked",
                tool
            );
        }
    }

    #[test]
    fn github_enabled_allows_github_tools() {
        let config = PathSecurityConfig::default();
        for tool in &[
            "github_identity",
            "github_issue",
            "github_pr",
            "github_file",
            "github_repo",
        ] {
            assert!(
                check_tool_access(tool, &config).is_ok(),
                "{} should be allowed by default",
                tool
            );
        }
    }
```

Also update the `read_tools_always_allowed` test — add `config.github_enabled = false;` (replacing the old `config.git_enabled = false;` that was removed in Phase 1).

Also update the `check_tool_access_error_message_includes_config_hint` test — add a github check:
```rust
        config.github_enabled = false;
        let err = check_tool_access("github_pr", &config).unwrap_err();
        assert!(
            err.to_string().contains("github_enabled"),
            "error should mention config key"
        );
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test github_disabled 2>&1
cargo test github_enabled_allows 2>&1
```
Expected: `github_disabled_blocks_all_github_tools` FAILS (tools pass through wildcard).

- [ ] **Step 3: Add match arm to `check_tool_access`**

In `src/util/path_security.rs`, in the `check_tool_access` function, add before the `_ => {}` wildcard:

```rust
        "github_identity" | "github_issue" | "github_pr" | "github_file" | "github_repo" => {
            if !config.github_enabled {
                bail!(
                    "GitHub tools are disabled. Set security.github_enabled = true in .codescout/project.toml to enable."
                );
            }
        }
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test check_tool_access 2>&1
cargo test github_ 2>&1
```
Expected: all pass.

### Task 7: Update config tests for `github_enabled`

**Files:**
- Modify: `src/config/project.rs` — 3 existing tests

- [ ] **Step 1: Add assertions to existing tests**

In `security_section_default_enables_write_git_indexing`:
```rust
        assert!(sec.github_enabled, "github_enabled should default to true");
```

In `project_config_default_for_enables_write_tools`:
```rust
        assert!(cfg.security.github_enabled);
```

In `toml_without_security_section_enables_write_tools`:
```rust
        assert!(cfg.security.github_enabled);
```

- [ ] **Step 2: Run tests**

```bash
cargo test security_section_default 2>&1
cargo test project_config_default_for 2>&1
cargo test toml_without_security 2>&1
```
Expected: all PASS.

- [ ] **Step 3: Update docs with `github_enabled`**

These doc changes belong in the Phase 2 commit alongside the code — shipping a new config flag without documenting it creates a gap.

In `docs/manual/src/architecture.md`, update the sentence that was emptied in Phase 1 (line ~70). Change from:
```markdown
   here.
```
To:
```markdown
   here. If `github_enabled` is false, GitHub tools are blocked.
```

In `docs/manual/src/concepts/security.md`, add after the `indexing_enabled` line:
```toml
github_enabled     = true    # GitHub API tools (github_issue, github_pr, etc.)
```

In `docs/manual/src/configuration/project-toml.md`, add `github_enabled = true` to both TOML examples, and add a row to the config table:
```markdown
| `github_enabled` | bool | `true` | Enables GitHub tools: `github_identity`, `github_issue`, `github_pr`, `github_file`, `github_repo`. Set to `false` to block all GitHub API access. |
```

In `docs/RELEASE-TODO.md`, add after the `indexing_enabled` line:
```toml
github_enabled = true              # GitHub API tools (default: true)
```

In `docs/manual/src/tools/workflow-and-config.md`, add `"github_enabled": true` to the JSON example security object.

- [ ] **Step 4: Commit Phase 2**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add -A
git commit -m "feat(security): add github_enabled gate for all 5 GitHub tools"
```

---

## Chunk 3: Code Quality + Server Instructions (Phases 3-4)

### Task 8: Fix duplicate `#[async_trait]` on all GitHub tool impls

**Files:**
- Modify: `src/tools/github.rs:142-143,215-217,456-458,830-831,1084-1085`

- [ ] **Step 1: Fix all 5 impls**

Each GitHub tool impl has duplicate `#[async_trait]` attributes. Reduce each to a single `#[async_trait]`:

| Tool | Lines | Fix |
|---|---|---|
| `GithubIdentity` | 142-143 | Remove line 142 (keep one `#[async_trait]`) |
| `GithubIssue` | 215-217 | Remove lines 215-216 (keep one `#[async_trait]`) |
| `GithubPr` | 456-458 | Remove lines 456-457 (keep one `#[async_trait]`) |
| `GithubFile` | 830-831 | Remove line 830 (keep one `#[async_trait]`) |
| `GithubRepo` | 1084-1085 | Remove line 1084 (keep one `#[async_trait]`) |

- [ ] **Step 2: Fix stacked `#[cfg(test)]`**

Lines ~1361-1365 have 5x `#[cfg(test)]`. Reduce to single:
```rust
#[cfg(test)]
mod tests {
```

- [ ] **Step 3: Build and test**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1
```
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/tools/github.rs
git commit -m "style(github): remove duplicate #[async_trait] and #[cfg(test)] attributes"
```

### Task 9: Update server instructions — local vs remote git

**Files:**
- Modify: `src/prompts/server_instructions.md:61,83`

- [ ] **Step 1: Enhance the "By task" local git row**

In `server_instructions.md`, the existing row at line ~61:
```markdown
| Local git (blame, log, diff) | `run_command("git blame/log/diff ...")` | — |
```

Replace with:
```markdown
| Local git (blame, log, diff) | `run_command("git blame/log/diff ...")` | ~~`github_repo(list_commits)`~~ — local git is faster, has full history |
```

- [ ] **Step 2: Add anti-pattern for remote-when-local**

Add a new row to the anti-patterns table (after the `gh issue list` row at line ~83):

```markdown
| `github_repo(list_commits)` for local file history | `run_command("git log src/foo.rs")` | Local git has full history; GitHub API is paginated and rate-limited |
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "docs: add local-vs-remote git guidance in server instructions"
```

---

## Final Verification

- [ ] **Full test suite**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

- [ ] **Grep for stale references**

```bash
grep -r "git_enabled" src/          # expect: 0 matches
grep -r "git_blame" src/            # expect: 0 matches
grep -r "git_enabled" docs/         # expect: 0 matches in non-spec/plan files
grep -r "file_log\|head_short_sha\|CommitSummary" src/git/  # expect: 0 matches
```

- [ ] **Verify prompt surface consistency**

Check all 3 prompt surfaces for stale tool names:
```bash
grep -n "git_blame\|git_enabled" src/prompts/server_instructions.md
grep -n "git_blame\|git_enabled" src/prompts/onboarding_prompt.md
```
Expected: 0 matches.
