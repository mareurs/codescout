# `approve_write` — Session-Scoped Write Access Outside Project Root

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `approve_write` tool that lets an LLM grant itself write access to a directory outside the project root for the current MCP session.

**Architecture:** A new `session_write_roots` field (Arc-wrapped Mutex vec) on `ActiveProject` stores session-approved roots; two new `Agent` methods expose it. `validate_write_path` gains a `session_roots: &[PathBuf]` parameter and checks them alongside static roots. A new `approve_write` tool validates + appends to session state.

**Tech Stack:** Rust, tokio, serde_json, existing `crate::util::path_security`, `crate::tools::RecoverableError`, `crate::agent::{Agent, ActiveProject}`

---

## File Map

| Action | File | What changes |
|--------|------|-------------|
| Modify | `src/agent/mod.rs` | Add `session_write_roots` field to `ActiveProject`; initialize in `activate`; add two `Agent` methods |
| Modify | `src/util/path_security.rs` | Add `validate_approve_path`; update `validate_write_path` signature + error message; update tests |
| Modify | `src/fs/mod.rs` | Update `resolve_write_path` to pass session roots |
| Modify | `src/tools/create_file.rs` | Pass session roots to `validate_write_path` |
| Modify | `src/tools/edit_file/mod.rs` | Pass session roots (3 callsites) |
| Modify | `src/tools/symbol/edit_code.rs` | Pass session roots to rename validator |
| Modify | `src/tools/markdown/edit_markdown.rs` | Pass session roots |
| Create | `src/tools/approve_write.rs` | New tool |
| Modify | `src/tools/mod.rs` | `pub mod approve_write;` |
| Modify | `src/server.rs` | Import + register `ApproveWrite`; update `server_registers_all_tools` test |
| Modify | `src/util/path_security.rs` (tests) | Add `"approve_write"` to gate; new gate test; new `validate_approve_path` tests |
| Modify | `src/prompts/server_instructions.md` | One-line mention in Security Profiles section |
| Modify | `src/tools/onboarding.rs` | Bump `ONBOARDING_VERSION` |

---

### Task 1: Add `session_write_roots` to `ActiveProject` and initialize in `activate`

**Files:**
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: Add field to `ActiveProject` struct**

  In `src/agent/mod.rs`, find the `ActiveProject` struct (ends around line 145). Add the new field after `file_lock`:

  ```rust
  pub(crate) file_lock: Arc<std::fs::File>,
  pub(crate) session_write_roots: Arc<std::sync::Mutex<Vec<PathBuf>>>,
  ```

- [ ] **Step 2: Initialize field in `activate`**

  In `activate`, the `ActiveProject { ... }` constructor block (lines ~372–385). Add:

  ```rust
  let active = ActiveProject {
      root: root.clone(),
      config,
      memory,
      private_memory,
      library_registry,
      dirty_files,
      read_only: effective_read_only,
      head_sha,
      has_git_remote: probe_has_git_remote(&root),
      write_lock,
      file_lock,
      session_write_roots: Arc::new(std::sync::Mutex::new(Vec::new())),  // always fresh
  };
  ```

  Note: unlike `dirty_files`, `session_write_roots` is NOT preserved on same-root re-activation — approvals clear when the user re-activates.

- [ ] **Step 3: Run tests to confirm compile + existing tests pass**

  ```
  cargo test
  ```

  Expected: all tests pass (no logic change yet, just a new field).

- [ ] **Step 4: Commit**

  ```bash
  git add src/agent/mod.rs
  git commit -m "feat(agent): add session_write_roots field to ActiveProject"
  ```

---

### Task 2: Add `Agent` methods for session write roots

**Files:**
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: Write failing tests for the two methods**

  In `src/agent/mod.rs`, in the `tests` module, add:

  ```rust
  #[tokio::test]
  async fn session_write_roots_empty_by_default() {
      let dir = tempdir().unwrap();
      let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
      let roots = agent.session_write_roots_snapshot().await;
      assert!(roots.is_empty());
  }

  #[tokio::test]
  async fn add_session_write_root_visible_in_snapshot() {
      let dir = tempdir().unwrap();
      let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
      let extra = dir.path().join("extra");
      agent.add_session_write_root(extra.clone()).await;
      let roots = agent.session_write_roots_snapshot().await;
      assert_eq!(roots, vec![extra]);
  }

  #[tokio::test]
  async fn session_write_roots_cleared_on_reactivation() {
      let dir = tempdir().unwrap();
      let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
      let extra = dir.path().join("extra");
      agent.add_session_write_root(extra.clone()).await;
      // Snapshot shows the root
      let roots = agent.session_write_roots_snapshot().await;
      assert!(!roots.is_empty(), "root should be visible before re-activation");
      // Re-activate same project
      agent.activate(dir.path().to_path_buf(), None).await.unwrap();
      // Snapshot is now empty — re-activation created a fresh ActiveProject
      let roots_after = agent.session_write_roots_snapshot().await;
      assert!(roots_after.is_empty(), "session roots must clear on re-activation");
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  ```
  cargo test session_write_roots
  ```

  Expected: compilation error — methods don't exist yet.

- [ ] **Step 3: Implement the two Agent methods**

  In `src/agent/mod.rs`, near the `mark_file_dirty` method (~line 614), add:

  ```rust
  /// Append a session-approved write root for the current project.
  pub async fn add_session_write_root(&self, path: PathBuf) {
      let inner = self.inner.read().await;
      if let Some(p) = inner.active_project() {
          p.session_write_roots
              .lock()
              .unwrap_or_else(|e| e.into_inner())
              .push(path);
      }
  }

  /// Return a snapshot of the current session-approved write roots.
  pub async fn session_write_roots_snapshot(&self) -> Vec<PathBuf> {
      let inner = self.inner.read().await;
      match inner.active_project() {
          Some(p) => p
              .session_write_roots
              .lock()
              .unwrap_or_else(|e| e.into_inner())
              .clone(),
          None => Vec::new(),
      }
  }
  ```

- [ ] **Step 4: Run tests to verify they pass**

  ```
  cargo test session_write_roots
  ```

  Expected: all 3 new tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add src/agent/mod.rs
  git commit -m "feat(agent): add add_session_write_root and session_write_roots_snapshot"
  ```

---

### Task 3: Add `validate_approve_path` to `path_security.rs`

**Files:**
- Modify: `src/util/path_security.rs`

This new pub function encapsulates the breadth guard + deny-list check used by the `approve_write` tool. Keeping validation logic in the security module keeps it testable and independent of tool plumbing.

- [ ] **Step 1: Write failing tests**

  In `src/util/path_security.rs`, in the `tests` module, add:

  ```rust
  #[test]
  fn validate_approve_path_accepts_normal_directory() {
      let dir = tempdir().unwrap();
      let target = dir.path().join("other");
      std::fs::create_dir_all(&target).unwrap();
      let result = validate_approve_path(
          target.to_str().unwrap(),
          dir.path(),
          &default_config(),
      );
      assert!(result.is_ok(), "normal directory should be approved: {:?}", result);
  }

  #[test]
  fn validate_approve_path_rejects_filesystem_root() {
      let dir = tempdir().unwrap();
      let result = validate_approve_path("/", dir.path(), &default_config());
      assert!(result.is_err());
      assert!(result.unwrap_err().to_string().contains("too broad"));
  }

  #[test]
  fn validate_approve_path_rejects_home_directory() {
      let dir = tempdir().unwrap();
      let home = crate::platform::home_dir().unwrap();
      let result = validate_approve_path(
          home.to_str().unwrap(),
          dir.path(),
          &default_config(),
      );
      assert!(result.is_err());
      assert!(result.unwrap_err().to_string().contains("too broad"));
  }

  #[test]
  fn validate_approve_path_rejects_denied_path() {
      let dir = tempdir().unwrap();
      let home = crate::platform::home_dir().unwrap();
      let ssh = home.join(".ssh");
      let result = validate_approve_path(
          ssh.to_str().unwrap(),
          dir.path(),
          &default_config(),
      );
      assert!(result.is_err());
      assert!(result.unwrap_err().to_string().contains("protected location"));
  }

  #[test]
  fn validate_approve_path_resolves_relative_path() {
      let dir = tempdir().unwrap();
      let result = validate_approve_path("subdir", dir.path(), &default_config());
      // subdir doesn't need to exist — best_effort_canonicalize handles it
      assert!(result.is_ok());
      let resolved = result.unwrap();
      assert!(resolved.ends_with("subdir"));
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  ```
  cargo test validate_approve_path
  ```

  Expected: compilation error — function doesn't exist yet.

- [ ] **Step 3: Implement `validate_approve_path`**

  In `src/util/path_security.rs`, after the `validate_write_path` function (~line 334), add:

  ```rust
  /// Validate a path for **session approval** via the `approve_write` tool.
  ///
  /// Checks:
  /// 1. Rejects the filesystem root (`/`) and `$HOME` — too broad.
  /// 2. Checks the deny-list — protected paths can never be approved.
  ///
  /// Returns the canonicalized path on success.
  pub fn validate_approve_path(
      raw: &str,
      project_root: &Path,
      config: &PathSecurityConfig,
  ) -> Result<PathBuf> {
      if raw.is_empty() {
          bail!("path must not be empty");
      }

      let path = Path::new(raw);
      let resolved = if path.is_absolute() {
          best_effort_canonicalize(path)
      } else {
          best_effort_canonicalize(&project_root.join(raw))
      };

      // Breadth guard: reject / and $HOME
      let is_fs_root = resolved == Path::new("/");
      let is_home = home_dir()
          .map(|h| best_effort_canonicalize(&h) == resolved)
          .unwrap_or(false);
      if is_fs_root || is_home {
          bail!(
              "approve_write: '{}' is too broad — specify a subdirectory",
              resolved.display()
          );
      }

      // Deny-list: protected paths can never be approved
      let denied = denied_read_paths(config);
      if is_denied(&resolved, &denied) {
          bail!(
              "approve_write: '{}' is in a protected location and cannot be approved",
              resolved.display()
          );
      }

      Ok(resolved)
  }
  ```

- [ ] **Step 4: Run tests to verify they pass**

  ```
  cargo test validate_approve_path
  ```

  Expected: all 5 new tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add src/util/path_security.rs
  git commit -m "feat(security): add validate_approve_path for session write approval"
  ```

---

### Task 4: Update `validate_write_path` signature and all callers

**Files:**
- Modify: `src/util/path_security.rs`
- Modify: `src/fs/mod.rs`
- Modify: `src/tools/create_file.rs`
- Modify: `src/tools/edit_file/mod.rs`
- Modify: `src/tools/symbol/edit_code.rs`
- Modify: `src/tools/markdown/edit_markdown.rs`

This is a mechanical signature change. The compiler will flag every caller. There are 6 production callers and ~14 test callsites.

- [ ] **Step 1: Update `validate_write_path` signature and body**

  In `src/util/path_security.rs`, change the signature of `validate_write_path` (line ~248):

  ```rust
  pub fn validate_write_path(
      raw: &str,
      project_root: &Path,
      config: &PathSecurityConfig,
      session_roots: &[PathBuf],
  ) -> Result<PathBuf> {
  ```

  After the block that builds `allowed` (after adding temp dir and CWD), add:

  ```rust
  for root in session_roots {
      allowed.push(best_effort_canonicalize(root));
  }
  ```

  Change the final error message from:
  ```rust
  bail!("write denied: '{}' is outside the project root", raw);
  ```
  to:
  ```rust
  bail!(
      "write denied: '{}' is outside the project root. \
       Call approve_write('<dir>') first to grant write access for this session.",
      raw
  );
  ```

- [ ] **Step 2: Add a `default_session_roots` helper in the test module**

  In `src/util/path_security.rs`, in the `tests` module, add:

  ```rust
  fn default_session_roots() -> Vec<PathBuf> {
      vec![]
  }
  ```

- [ ] **Step 3: Fix test callsites in `path_security.rs`**

  Every `validate_write_path(...)` call in the test module (14 callsites) needs `&default_session_roots()` as the 4th argument. Do a search-and-replace within the `tests` module:

  Find all occurrences of:
  ```rust
  validate_write_path(
  ```
  that have 3 arguments, and add `&default_session_roots()` as the 4th argument.

  Run `cargo test` to confirm tests still pass before moving to production callers.

- [ ] **Step 4: Update `resolve_write_path` in `src/fs/mod.rs`**

  Change the function (lines 70–77) to:

  ```rust
  pub(crate) async fn resolve_write_path(
      agent: &Agent,
      relative_path: &str,
  ) -> anyhow::Result<PathBuf> {
      let root = agent.require_project_root().await?;
      let security = agent.security_config().await;
      let session_roots = agent.session_write_roots_snapshot().await;
      crate::util::path_security::validate_write_path(
          relative_path,
          &root,
          &security,
          &session_roots,
      )
  }
  ```

- [ ] **Step 5: Update `create_file.rs` (1 callsite)**

  In `src/tools/create_file.rs` (around line 49), add `session_roots` before the `validate_write_path` call:

  ```rust
  let root = ctx.agent.require_project_root().await?;
  let security = ctx.agent.security_config().await;
  let session_roots = ctx.agent.session_write_roots_snapshot().await;
  let resolved = crate::util::path_security::validate_write_path(
      path, &root, &security, &session_roots,
  )?;
  ```

- [ ] **Step 6: Update `edit_file/mod.rs` (3 callsites)**

  There are 3 callsites: the `edits` array branch (~line 203), the `insert` branch (~line 268), and `perform_edit` (~line 329). Each one fetches `root` and `security` just before calling `validate_write_path`. Update each to:

  ```rust
  let root = ctx.agent.require_project_root().await?;
  let security = ctx.agent.security_config().await;
  let session_roots = ctx.agent.session_write_roots_snapshot().await;
  let resolved = crate::util::path_security::validate_write_path(
      path, &root, &security, &session_roots,
  )?;
  ```

  For `perform_edit`, `ctx` is passed as a parameter — the same pattern applies.

- [ ] **Step 7: Update `edit_code.rs` (1 callsite in `do_rename`)**

  In `do_rename` (~line 115), `validate_write_path` is called inside a closure that captures `rename_root` and `rename_security`. Add a session roots capture before the closure:

  ```rust
  let rename_root = ctx.agent.require_project_root().await?;
  let rename_security = ctx.agent.security_config().await;
  let rename_session_roots = ctx.agent.session_write_roots_snapshot().await;
  // ... (existing closure setup)
  let plan_path = |path: PathBuf,
                   plain_edits: Vec<lsp_types::TextEdit>,
                   plan: &mut Vec<PlannedWrite>|
   -> anyhow::Result<()> {
      // ...
      crate::util::path_security::validate_write_path(
          path_str,
          &rename_root,
          &rename_security,
          &rename_session_roots,
      )?;
      // ...
  };
  ```

- [ ] **Step 8: Update `edit_markdown.rs` (1 callsite)**

  Around line 359:

  ```rust
  let root = ctx.agent.require_project_root().await?;
  let security = ctx.agent.security_config().await;
  let session_roots = ctx.agent.session_write_roots_snapshot().await;
  let resolved = crate::util::path_security::validate_write_path(
      path, &root, &security, &session_roots,
  )?;
  ```

- [ ] **Step 9: Run all tests**

  ```
  cargo test
  ```

  Expected: all tests pass. Fix any remaining callsites the compiler flags.

- [ ] **Step 10: Add a unit test for the new error message hint**

  In `src/util/path_security.rs` tests:

  ```rust
  #[test]
  fn validate_write_path_outside_root_mentions_approve_write() {
      let dir = tempdir().unwrap();
      let other = tempdir().unwrap();
      let result = validate_write_path(
          other.path().to_str().unwrap(),
          dir.path(),
          &default_config(),
          &default_session_roots(),
      );
      let err = result.unwrap_err().to_string();
      assert!(
          err.contains("approve_write"),
          "error should mention approve_write: {err}"
      );
  }

  #[test]
  fn validate_write_path_allows_session_approved_root() {
      let dir = tempdir().unwrap();
      let other = tempdir().unwrap();
      let session_roots = vec![other.path().to_path_buf()];
      let target = other.path().join("file.txt");
      let result = validate_write_path(
          target.to_str().unwrap(),
          dir.path(),
          &default_config(),
          &session_roots,
      );
      assert!(result.is_ok(), "approved root should allow writes: {:?}", result);
  }

  #[test]
  fn validate_write_path_session_root_still_respects_deny_list() {
      let dir = tempdir().unwrap();
      let home = crate::platform::home_dir().unwrap();
      let ssh = home.join(".ssh");
      // Even if someone manages to sneak ~/.ssh into session_roots, deny-list wins
      let session_roots = vec![ssh.clone()];
      let target = ssh.join("authorized_keys");
      let result = validate_write_path(
          target.to_str().unwrap(),
          dir.path(),
          &default_config(),
          &session_roots,
      );
      assert!(result.is_err(), "deny-list must win over session roots");
  }
  ```

  Run: `cargo test validate_write_path_outside_root` — expected: PASS.

- [ ] **Step 11: Commit**

  ```bash
  git add src/util/path_security.rs src/fs/mod.rs src/tools/create_file.rs \
          src/tools/edit_file/mod.rs src/tools/symbol/edit_code.rs \
          src/tools/markdown/edit_markdown.rs
  git commit -m "feat(security): add session_roots param to validate_write_path; update all callers"
  ```

---

### Task 5: Implement the `approve_write` tool

**Files:**
- Create: `src/tools/approve_write.rs`
- Modify: `src/tools/mod.rs`

- [ ] **Step 1: Add the module declaration to `src/tools/mod.rs`**

  In `src/tools/mod.rs`, add with the other tool modules (alphabetically, near `create_file`):

  ```rust
  pub mod approve_write;
  ```

- [ ] **Step 2: Create `src/tools/approve_write.rs`**

  ```rust
  use std::path::PathBuf;

  use serde_json::{json, Value};

  use crate::tools::{RecoverableError, ToolContext};

  pub struct ApproveWrite;

  #[async_trait::async_trait]
  impl crate::tools::Tool for ApproveWrite {
      fn name(&self) -> &str {
          "approve_write"
      }

      fn description(&self) -> &str {
          "Grant write access to a directory outside the project root for this session. \
           Approval is session-scoped — it is cleared on server restart or project re-activation. \
           Call this before edit_file, create_file, edit_code, or edit_markdown on paths outside \
           the active project root. The deny-list (e.g. ~/.ssh) is always enforced and cannot be \
           approved."
      }

      fn input_schema(&self) -> Value {
          json!({
              "type": "object",
              "properties": {
                  "path": {
                      "type": "string",
                      "description": "Absolute or project-relative path to the directory to approve for writing."
                  }
              },
              "required": ["path"]
          })
      }

      async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
          let raw = super::require_str_param(&input, "path")?;

          let root = ctx.agent.require_project_root().await.map_err(|_| {
              RecoverableError::new(
                  "approve_write: no active project — activate a project first",
              )
          })?;

          let security = ctx.agent.security_config().await;

          if !security.file_write_enabled {
              return Err(RecoverableError::new(
                  "approve_write: file writes are disabled for this project",
              )
              .into());
          }

          let resolved =
              crate::util::path_security::validate_approve_path(raw, &root, &security)
                  .map_err(|e| RecoverableError::new(e.to_string()))?;

          ctx.agent.add_session_write_root(resolved.clone()).await;

          Ok(json!({
              "approved": resolved.to_string_lossy(),
              "scope": "this session only"
          }))
      }
  }
  ```

- [ ] **Step 3: Run tests to confirm compilation**

  ```
  cargo test
  ```

  Expected: compiles and all existing tests pass.

- [ ] **Step 4: Commit**

  ```bash
  git add src/tools/approve_write.rs src/tools/mod.rs
  git commit -m "feat(tools): implement approve_write tool"
  ```

---

### Task 6: Register tool and add security gate

**Files:**
- Modify: `src/server.rs`
- Modify: `src/util/path_security.rs`

- [ ] **Step 1: Write failing tests for registration and gate**

  In `src/server.rs`, update `server_registers_all_tools` — add `"approve_write"` to the `expected_tools` array:

  ```rust
  let expected_tools = [
      "read_file",
      "tree",
      "grep",
      "approve_write",   // ← add here
      "create_file",
      // ... rest unchanged
  ];
  ```

  In `src/util/path_security.rs` tests, add:

  ```rust
  #[test]
  fn file_write_enabled_disabled_blocks_approve_write() {
      let config = PathSecurityConfig {
          file_write_enabled: false,
          ..PathSecurityConfig::default()
      };
      let err = check_tool_access("approve_write", &config).unwrap_err();
      assert!(
          err.to_string().contains("disabled"),
          "should block approve_write when writes disabled: {err}"
      );
  }
  ```

  Run: `cargo test server_registers_all_tools file_write_enabled_disabled_blocks_approve_write`
  Expected: `server_registers_all_tools` fails (tool not registered); gate test fails (not in match arm).

- [ ] **Step 2: Register `ApproveWrite` in `server.rs`**

  In `src/server.rs`, add the import alongside the other tool imports:

  ```rust
  use crate::tools::{
      approve_write::ApproveWrite,
      config::Workspace,
      create_file::CreateFile,
      // ... rest unchanged
  };
  ```

  In `from_parts`, add to the tools vec (in the file-tools section):

  ```rust
  Arc::new(ApproveWrite),
  Arc::new(CreateFile),
  // ... rest
  ```

- [ ] **Step 3: Add `approve_write` to `check_tool_access` gate**

  In `src/util/path_security.rs`, in `check_tool_access`, find the `file_write_enabled` arm:

  ```rust
  "create_file" | "edit_file" | "edit_markdown" | "library" | "edit_code" => {
  ```

  Change to:

  ```rust
  "approve_write" | "create_file" | "edit_file" | "edit_markdown" | "library" | "edit_code" => {
  ```

- [ ] **Step 4: Run the failing tests to verify they now pass**

  ```
  cargo test server_registers_all_tools file_write_enabled_disabled_blocks_approve_write
  ```

  Expected: both pass.

- [ ] **Step 5: Run full test suite**

  ```
  cargo test
  ```

  Expected: all pass.

- [ ] **Step 6: Commit**

  ```bash
  git add src/server.rs src/util/path_security.rs
  git commit -m "feat(server): register approve_write tool and add to file_write gate"
  ```

---

### Task 7: Update prompt surface and bump `ONBOARDING_VERSION`

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/tools/onboarding.rs`

- [ ] **Step 1: Add `approve_write` to `server_instructions.md`**

  In `src/prompts/server_instructions.md`, find the `### Security Profiles` section (line ~215). After the existing profile descriptions, add:

  ```markdown
  - `approve_write(path)` — grant write access to a directory outside the project root for
    this session. Required before `edit_file`/`create_file`/`edit_code`/`edit_markdown` on
    out-of-project paths. Approval is cleared on server restart or re-activation. The
    deny-list (`~/.ssh`, etc.) is always enforced regardless of approval.
  ```

- [ ] **Step 2: Bump `ONBOARDING_VERSION`**

  In `src/tools/onboarding.rs` (line 19), increment the constant:

  ```rust
  pub(crate) const ONBOARDING_VERSION: u32 = 21;
  ```

- [ ] **Step 3: Run the prompt surface consistency test**

  ```
  cargo test prompt_surfaces_reference_only_real_tools
  ```

  Expected: passes (the test checks that tool names in prompts exist in the server's tool registry — `approve_write` is now registered so this should pass).

- [ ] **Step 4: Commit**

  ```bash
  git add src/prompts/server_instructions.md src/tools/onboarding.rs
  git commit -m "docs(prompts): document approve_write; bump ONBOARDING_VERSION to 21"
  ```

---

### Task 8: Final verification

- [ ] **Step 1: `cargo fmt`**

  ```
  cargo fmt
  ```

  Fix any formatting issues, then confirm `cargo fmt -- --check` reports no diff.

- [ ] **Step 2: `cargo clippy`**

  ```
  cargo clippy -- -D warnings
  ```

  Expected: zero warnings.

- [ ] **Step 3: Full test suite**

  ```
  cargo test
  ```

  Expected: all tests pass.

- [ ] **Step 4: Build release binary**

  ```
  cargo build --release
  ```

  Expected: compiles cleanly.

- [ ] **Step 5: Manual verification via MCP**

  Restart the MCP server with `/mcp`. Then verify end-to-end:

  1. Call `approve_write` with a temp path outside the project (e.g. `/tmp/test-outside`):
     - Expected: `{ "approved": "/tmp/test-outside", "scope": "this session only" }`
  2. Call `create_file` targeting a file inside `/tmp/test-outside/`:
     - Expected: succeeds (file written)
  3. Call `create_file` targeting a different path outside the project (not approved):
     - Expected: error mentioning `approve_write`
  4. Call `approve_write` with `~/.ssh`:
     - Expected: error mentioning "protected location"
  5. Call `approve_write` with `/`:
     - Expected: error mentioning "too broad"

- [ ] **Step 6: Final commit (if fmt/clippy caused minor fixes)**

  ```bash
  git add -A
  git commit -m "chore: fmt + clippy fixes for approve_write feature"
  ```

  If no changes needed, skip.
