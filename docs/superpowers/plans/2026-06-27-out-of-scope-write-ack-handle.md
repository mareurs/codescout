# Out-of-Scope Write Ack Handle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a write is rejected for being outside the project root, return an `@ack_*` handle that preserves the generated content; acknowledging the handle approves the directory for the session and replays the write — so hundreds of lines of content are never regenerated.

**Architecture:** Mirror `run_command`'s dangerous-command gate. A typed `classify_write_path` distinguishes the approvable "outside root" case from hard denials; the buffer's pending-ack store is generalized to hold writes as well as commands; two shared helpers (`maybe_replay_ack`, `resolve_write_or_capture`) implement replay and capture; each write tool gains ~4 lines.

**Tech Stack:** Rust, `serde_json::Value`, `tokio`, existing `OutputBuffer` LRU store, existing `path_security` module.

**Spec:** `docs/superpowers/specs/2026-06-27-out-of-scope-write-ack-handle-design.md`

## Global Constraints

- Pre-commit gate on every task: `cargo fmt`, `cargo clippy --lib -- -D warnings`, `cargo test --lib` must all pass before the task's commit.
- Source edits use codescout MCP tools (`edit_code`/`edit_file`), never native Write/Edit; run shell via `run_command` (bare, then query `@cmd_*`).
- Branch: `experiments` (master is protected). Do not commit to master.
- Error style: recoverable, agent-correctable failures use `RecoverableError` (→ `ok:false`); never `anyhow::bail!` for these.
- Handle format is `@ack_<8 lowercase hex>` — unchanged; commands and writes share the `pending_acks` store and its `max_pending = 20` LRU.
- `validate_write_path`'s external bail messages must remain byte-identical (other call sites and tests depend on them).
- `edit_code` is **out of scope** for this plan (see "Out of Scope / Follow-up"). Only `create_file`, `edit_file`, `edit_markdown` are wired.

---

### Task 1: `classify_write_path` typed decision

**Files:**
- Modify: `src/util/path_security.rs` (replace `validate_write_path`, add `classify_write_path` + `WritePathDecision`)
- Test: `src/util/path_security.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `pub enum WritePathDecision { Allowed(PathBuf), OutsideRoot { resolved: PathBuf }, Denied(String) }`
  - `pub fn classify_write_path(raw: &str, project_root: &Path, config: &PathSecurityConfig, session_roots: &[PathBuf]) -> WritePathDecision`
  - `pub fn validate_write_path(...) -> Result<PathBuf>` (unchanged signature; now a thin wrapper)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/util/path_security.rs`. Use the module's existing helpers for building a `PathSecurityConfig` (match the pattern already used by other tests in this file — `PathSecurityConfig::default()` or the local constructor).

```rust
#[test]
fn classify_in_project_is_allowed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let cfg = PathSecurityConfig::default();
    let decision = classify_write_path("sub/file.rs", root, &cfg, &[]);
    assert!(matches!(decision, WritePathDecision::Allowed(_)), "got: {decision:?}");
}

#[test]
fn classify_outside_root_is_outsideroot() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let cfg = PathSecurityConfig::default();
    // /var is outside the project root, the temp dir, and cwd.
    let decision = classify_write_path("/var/ce_classify_test/x.rs", root, &cfg, &[]);
    assert!(
        matches!(decision, WritePathDecision::OutsideRoot { .. }),
        "got: {decision:?}"
    );
}

#[test]
fn classify_empty_is_denied() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = PathSecurityConfig::default();
    let decision = classify_write_path("", tmp.path(), &cfg, &[]);
    assert!(matches!(decision, WritePathDecision::Denied(_)), "got: {decision:?}");
}

#[test]
fn validate_write_path_still_bails_outside_with_unchanged_message() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = PathSecurityConfig::default();
    let err = validate_write_path("/var/ce_classify_test/x.rs", tmp.path(), &cfg, &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("is outside the project root"), "got: {err}");
    assert!(err.contains("Call approve_write"), "got: {err}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib path_security::tests::classify 2>&1`
Expected: FAIL — `classify_write_path` / `WritePathDecision` not found.

- [ ] **Step 3: Add `WritePathDecision` and `classify_write_path`, refactor `validate_write_path`**

In `src/util/path_security.rs`, add the enum directly above the current `validate_write_path` (use `edit_code` to insert before the `validate_write_path` symbol):

```rust
/// Outcome of classifying a write target against the project's write policy.
///
/// `OutsideRoot` is the one *approvable* failure — it can be turned into a
/// pending-ack handle. `Denied` covers the hard failures (empty / null byte /
/// unresolved `..` / deny-listed location) that must never be approved.
#[derive(Debug)]
pub enum WritePathDecision {
    Allowed(PathBuf),
    OutsideRoot { resolved: PathBuf },
    Denied(String),
}

/// Classify a write target without committing to an error type. The pure core
/// of `validate_write_path`; lets the ack layer distinguish the approvable
/// outside-root case from hard denials without matching on bail strings.
pub fn classify_write_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
    session_roots: &[PathBuf],
) -> WritePathDecision {
    if raw.is_empty() {
        return WritePathDecision::Denied("path must not be empty".to_string());
    }
    if raw.contains('\0') {
        return WritePathDecision::Denied("path contains null byte".to_string());
    }

    if config.profile == SecurityProfile::Root {
        let path = Path::new(raw);
        let resolved = if path.is_absolute() {
            PathBuf::from(raw)
        } else {
            project_root.join(raw)
        };
        return WritePathDecision::Allowed(canonicalize_write_target(&resolved));
    }

    let path = Path::new(raw);
    let resolved = if path.is_absolute() {
        PathBuf::from(raw)
    } else {
        project_root.join(raw)
    };
    let resolved = canonicalize_write_target(&resolved);

    if resolved
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return WritePathDecision::Denied(format!(
            "write denied: '{}' contains '..' that could not be resolved",
            raw
        ));
    }

    let project_root = best_effort_canonicalize(project_root);

    let denied = denied_read_paths(config);
    if is_denied(&resolved, &denied) {
        return WritePathDecision::Denied(format!(
            "write denied: '{}' is in a protected location",
            raw
        ));
    }

    let mut allowed = vec![project_root];
    allowed.push(crate::platform::temp_dir());
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = best_effort_canonicalize(&cwd);
        let is_broad = cwd_canon == Path::new("/") || home_dir().is_some_and(|h| cwd_canon == h);
        if !is_broad {
            allowed.push(cwd_canon);
        }
    }
    for extra in &config.extra_write_roots {
        allowed.push(best_effort_canonicalize(extra));
    }
    for root in session_roots {
        allowed.push(best_effort_canonicalize(root));
    }

    let under_allowed_root = allowed.iter().any(|root| resolved.starts_with(root));
    if !under_allowed_root {
        return WritePathDecision::OutsideRoot { resolved };
    }

    WritePathDecision::Allowed(resolved)
}
```

Then replace the body of `validate_write_path` (use `edit_code action=replace` on the `validate_write_path` symbol) so it delegates, preserving the exact messages:

```rust
pub fn validate_write_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
    session_roots: &[PathBuf],
) -> Result<PathBuf> {
    match classify_write_path(raw, project_root, config, session_roots) {
        WritePathDecision::Allowed(p) => Ok(p),
        WritePathDecision::OutsideRoot { .. } => bail!(
            "write denied: '{}' is outside the project root. \
             Call approve_write('<dir>') first to grant write access for this session.",
            raw
        ),
        WritePathDecision::Denied(msg) => bail!("{msg}"),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib path_security 2>&1`
Expected: PASS — new `classify_*` tests plus all pre-existing `path_security` tests (the wrapper preserves behavior).

- [ ] **Step 5: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/util/path_security.rs
git commit -m "feat(path-security): classify_write_path typed decision

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Generalize the buffer pending-ack store to hold writes

**Files:**
- Modify: `src/tools/output_buffer.rs` (add `PendingAck`/`PendingAckWrite`, change map type, update `store_dangerous`/`get_dangerous`, add `store_pending_write`/`get_pending_write`, add `looks_like_ack_handle`)
- Modify: `src/tools/run_command/inner.rs` (remove local `looks_like_ack_handle`)
- Modify: `src/tools/run_command/mod.rs` (import `looks_like_ack_handle` from `output_buffer`)
- Test: `src/tools/output_buffer.rs` (existing `tests` module)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub struct PendingAckWrite { pub tool_name: String, pub input: serde_json::Value, pub approve_dir: PathBuf }`
  - `pub enum PendingAck { Command(PendingAckCommand), Write(PendingAckWrite) }`
  - `OutputBuffer::store_pending_write(&self, tool_name: String, input: Value, approve_dir: PathBuf) -> String`
  - `OutputBuffer::get_pending_write(&self, handle: &str) -> Option<PendingAckWrite>`
  - `pub(crate) fn looks_like_ack_handle(s: &str) -> bool` (relocated here)
  - `get_dangerous` unchanged signature, now returns `None` for write handles.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/tools/output_buffer.rs`:

```rust
#[test]
fn store_pending_write_returns_ack_handle_and_round_trips() {
    let buf = OutputBuffer::new(10);
    let input = serde_json::json!({ "path": "/out/plan.md", "content": "big content" });
    let handle = buf.store_pending_write(
        "create_file".to_string(),
        input.clone(),
        std::path::PathBuf::from("/out"),
    );
    assert!(handle.starts_with("@ack_"), "got: {handle}");
    let got = buf.get_pending_write(&handle).expect("write handle should resolve");
    assert_eq!(got.tool_name, "create_file");
    assert_eq!(got.input, input);
    assert_eq!(got.approve_dir, std::path::PathBuf::from("/out"));
}

#[test]
fn get_dangerous_returns_none_for_write_handle() {
    let buf = OutputBuffer::new(10);
    let handle = buf.store_pending_write(
        "create_file".to_string(),
        serde_json::json!({ "path": "/out/x", "content": "c" }),
        std::path::PathBuf::from("/out"),
    );
    assert!(buf.get_dangerous(&handle).is_none());
}

#[test]
fn get_pending_write_returns_none_for_command_handle() {
    let buf = OutputBuffer::new(10);
    let handle = buf.store_dangerous("rm -rf /x".to_string(), None, 30);
    assert!(buf.get_pending_write(&handle).is_none());
}

#[test]
fn looks_like_ack_handle_recognizes_format() {
    assert!(looks_like_ack_handle("@ack_1a2b3c4d"));
    assert!(!looks_like_ack_handle("@cmd_1a2b3c4d"));
    assert!(!looks_like_ack_handle("@ack_short"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib output_buffer::tests::store_pending_write 2>&1`
Expected: FAIL — `store_pending_write` / `get_pending_write` / `looks_like_ack_handle` not found.

- [ ] **Step 3: Add the types**

In `src/tools/output_buffer.rs`, immediately after the existing `PendingAckCommand` struct, add (use `edit_code action=insert position=after` on `PendingAckCommand`):

```rust
/// A pending out-of-scope write held for acknowledgment. Carries the full
/// original tool input so replay needs no re-sent content.
#[derive(Debug, Clone)]
pub struct PendingAckWrite {
    /// Minting tool name — guards against replaying a handle through a
    /// different write tool than the one that created it.
    pub tool_name: String,
    /// The original tool input, verbatim (includes the large content payload).
    pub input: serde_json::Value,
    /// Directory granted as a session write root when the handle is replayed.
    pub approve_dir: PathBuf,
}

/// A pending acknowledgment — either a dangerous command or an out-of-scope
/// write. Both share the buffer's `pending_acks` LRU store and `@ack_` handles.
#[derive(Debug, Clone)]
pub enum PendingAck {
    Command(PendingAckCommand),
    Write(PendingAckWrite),
}
```

- [ ] **Step 4: Change the store type and update command accessors**

Change the field declaration in `struct BufferInner`:

```rust
    pending_acks: HashMap<String, PendingAck>,
```

`store_dangerous`'s insert becomes (wrap in the enum):

```rust
        inner.pending_acks.insert(
            id.clone(),
            PendingAck::Command(PendingAckCommand {
                command,
                cwd,
                timeout_secs,
            }),
        );
```

`get_dangerous` becomes:

```rust
    pub fn get_dangerous(&self, handle: &str) -> Option<PendingAckCommand> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match inner.pending_acks.get(handle) {
            Some(PendingAck::Command(c)) => Some(c.clone()),
            _ => None,
        }
    }
```

- [ ] **Step 5: Add `store_pending_write` and `get_pending_write`**

Insert directly after `get_dangerous` (use `edit_code action=insert position=after` on `OutputBuffer/get_dangerous`):

```rust
    /// Store an out-of-scope write pending acknowledgment.
    ///
    /// Returns an opaque `@ack_<8hex>` handle. The handle carries the full
    /// tool input (incl. content), so the ack call re-sends nothing.
    pub fn store_pending_write(
        &self,
        tool_name: String,
        input: serde_json::Value,
        approve_dir: PathBuf,
    ) -> String {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        inner.counter = inner.counter.wrapping_add(1);
        let id = format!("@ack_{:08x}", now.wrapping_add(inner.counter) as u32);

        // Evict oldest if at capacity (shared LRU with dangerous commands).
        if inner.pending_acks.len() >= inner.max_pending {
            if let Some(oldest) = inner.pending_order.first().cloned() {
                inner.pending_order.remove(0);
                inner.pending_acks.remove(&oldest);
            }
        }

        inner.pending_acks.insert(
            id.clone(),
            PendingAck::Write(PendingAckWrite {
                tool_name,
                input,
                approve_dir,
            }),
        );
        inner.pending_order.push(id.clone());
        id
    }

    /// Retrieve a stored pending write by handle. Returns `None` if the handle
    /// is unknown, evicted, or refers to a dangerous *command* rather than a
    /// write.
    pub fn get_pending_write(&self, handle: &str) -> Option<PendingAckWrite> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match inner.pending_acks.get(handle) {
            Some(PendingAck::Write(w)) => Some(w.clone()),
            _ => None,
        }
    }
```

- [ ] **Step 6: Relocate `looks_like_ack_handle` into the buffer**

Add this free function at the end of `src/tools/output_buffer.rs`, before the `#[cfg(test)] mod tests` block (use `edit_code action=insert position=before` on the test module, or `edit_file insert=append`-style placement above it):

```rust
/// Returns true when `s` is a bare `@ack_<8hex>` handle.
pub(crate) fn looks_like_ack_handle(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with("@ack_") {
        return false;
    }
    let suffix = &s[5..]; // after "@ack_"
    suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit())
}
```

Remove the duplicate definition from `src/tools/run_command/inner.rs` (use `edit_code action=remove` on `looks_like_ack_handle` in that file).

In `src/tools/run_command/mod.rs`, change the import line:

```rust
use inner::run_command_inner;
use crate::tools::output_buffer::looks_like_ack_handle;
```

(Adjust to keep `run_command_inner` imported as before; only `looks_like_ack_handle` moves source.)

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --lib output_buffer 2>&1`
Expected: PASS — new tests plus all existing `output_buffer` tests (LRU, dangerous round-trip).

Run: `cargo test --lib run_command 2>&1`
Expected: PASS — `run_command`'s `@ack_` flow still works with the relocated recognizer and enum-wrapped store.

- [ ] **Step 8: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/tools/output_buffer.rs src/tools/run_command/inner.rs src/tools/run_command/mod.rs
git commit -m "feat(output-buffer): generalize pending-ack store to hold writes

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Shared capture + replay helpers (`write_ack.rs`)

**Files:**
- Create: `src/tools/core/write_ack.rs`
- Modify: `src/tools/core/mod.rs` (declare + re-export the module)
- Test: `src/tools/core/write_ack.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `classify_write_path`, `WritePathDecision`, `validate_approve_path` (Task 1 + existing); `OutputBuffer::{store_pending_write, get_pending_write, looks_like_ack_handle}` (Task 2); `Agent::{require_project_root_for, security_config_for, session_write_roots_snapshot_for, add_session_write_root_for}`; `ToolContext`.
- Produces:
  - `pub enum WriteOutcome { Write(PathBuf), Pending(Value) }`
  - `pub async fn maybe_replay_ack(ctx: &ToolContext, input: Value, tool_name: &str) -> anyhow::Result<Value>`
  - `pub async fn resolve_write_or_capture(ctx: &ToolContext, tool_name: &str, input: &Value, raw_path: &str) -> anyhow::Result<WriteOutcome>`

- [ ] **Step 1: Declare the module**

In `src/tools/core/mod.rs`, add the module and re-export (use `edit_file`):

```rust
pub mod guards;
pub mod params;
pub mod types;
pub mod write_ack;

pub use guards::*;
pub use params::*;
pub use types::*;
pub use write_ack::*;
```

- [ ] **Step 2: Write the failing tests**

Create `src/tools/core/write_ack.rs` with the test module first (the impl goes in Step 3). Use a `ctx`-builder mirroring `src/tools/edit_file/tests.rs::project_ctx` — a tempdir project + `ToolContext`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use serde_json::json;

    async fn ctx_with_project() -> (tempfile::TempDir, crate::tools::ToolContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = crate::tools::ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };
        (dir, ctx)
    }

    #[tokio::test]
    async fn capture_outside_path_returns_pending_handle() {
        let (_dir, ctx) = ctx_with_project().await;
        let input = json!({ "path": "/var/ce_ack_test/plan.md", "content": "BIG" });
        let outcome = resolve_write_or_capture(&ctx, "create_file", &input, "/var/ce_ack_test/plan.md")
            .await
            .unwrap();
        match outcome {
            WriteOutcome::Pending(env) => {
                let handle = env["pending_ack"].as_str().expect("pending_ack");
                assert!(handle.starts_with("@ack_"), "got: {env}");
                assert!(env["reason"].as_str().unwrap().contains("outside the project root"));
                // Content preserved verbatim in the buffer.
                let stored = ctx.output_buffer.get_pending_write(handle).unwrap();
                assert_eq!(stored.input, input);
                assert_eq!(stored.tool_name, "create_file");
            }
            other => panic!("expected Pending, got Write: {other:?}"),
        }
    }

    #[tokio::test]
    async fn in_project_path_resolves_to_write() {
        let (dir, ctx) = ctx_with_project().await;
        let p = dir.path().join("plan.md");
        let input = json!({ "path": p.to_str().unwrap(), "content": "x" });
        let outcome = resolve_write_or_capture(&ctx, "create_file", &input, p.to_str().unwrap())
            .await
            .unwrap();
        assert!(matches!(outcome, WriteOutcome::Write(_)), "got: {outcome:?}");
    }

    #[tokio::test]
    async fn replay_approves_dir_and_returns_stored_input() {
        let (_dir, ctx) = ctx_with_project().await;
        // approve_dir must be approvable: a fresh tempdir (under system temp,
        // not /, $HOME, or a denied path).
        let ext = tempfile::tempdir().unwrap();
        let stored_input = json!({
            "path": ext.path().join("plan.md").to_str().unwrap(),
            "content": "preserved content"
        });
        let handle = ctx.output_buffer.store_pending_write(
            "create_file".to_string(),
            stored_input.clone(),
            ext.path().to_path_buf(),
        );

        let replayed = maybe_replay_ack(&ctx, json!({ "path": handle }), "create_file")
            .await
            .unwrap();
        assert_eq!(replayed, stored_input, "replay returns original input");

        let roots = ctx.agent.session_write_roots_snapshot().await;
        assert!(
            roots.iter().any(|r| r == ext.path()),
            "approve_dir should be a session write root now: {roots:?}"
        );
    }

    #[tokio::test]
    async fn replay_unknown_handle_errors() {
        let (_dir, ctx) = ctx_with_project().await;
        let err = maybe_replay_ack(&ctx, json!({ "path": "@ack_deadbeef" }), "create_file")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("expired or unknown"), "got: {err}");
    }

    #[tokio::test]
    async fn replay_cross_tool_handle_rejected() {
        let (_dir, ctx) = ctx_with_project().await;
        let ext = tempfile::tempdir().unwrap();
        let handle = ctx.output_buffer.store_pending_write(
            "create_file".to_string(),
            json!({ "path": ext.path().join("p").to_str().unwrap(), "content": "c" }),
            ext.path().to_path_buf(),
        );
        let err = maybe_replay_ack(&ctx, json!({ "path": handle }), "edit_markdown")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("minted by"), "got: {err}");
    }

    #[tokio::test]
    async fn non_handle_path_passes_through_unchanged() {
        let (_dir, ctx) = ctx_with_project().await;
        let input = json!({ "path": "src/main.rs", "content": "x" });
        let out = maybe_replay_ack(&ctx, input.clone(), "create_file").await.unwrap();
        assert_eq!(out, input);
    }
}
```

- [ ] **Step 3: Write the implementation (above the test module)**

Prepend this to `src/tools/core/write_ack.rs`:

```rust
//! Shared capture + replay helpers for out-of-scope writes.
//!
//! When a write tool targets a path outside the project root, `resolve_write_or_capture`
//! stashes the full tool input and returns a `@ack_*` handle instead of failing.
//! Re-invoking the tool with that handle in `path` hits `maybe_replay_ack`, which
//! approves the directory for the session and returns the original input so the
//! tool can replay the write without re-sending content. Mirrors `run_command`'s
//! dangerous-command gate.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::tools::output_buffer::looks_like_ack_handle;
use crate::util::path_security::{classify_write_path, validate_approve_path, WritePathDecision};

/// Result of resolving a write target with capture awareness.
#[derive(Debug)]
pub enum WriteOutcome {
    /// Proceed: write to this resolved path.
    Write(PathBuf),
    /// Return this `pending_ack` envelope to the caller verbatim.
    Pending(Value),
}

/// Phase A — if `input["path"]` is an `@ack_*` write handle, approve its
/// directory for the session and return the original stored input. Otherwise
/// return `input` unchanged. MUST run before any path-shape gate in the tool.
pub async fn maybe_replay_ack(
    ctx: &ToolContext,
    input: Value,
    tool_name: &str,
) -> anyhow::Result<Value> {
    let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if !looks_like_ack_handle(path) {
        return Ok(input);
    }
    let stored = ctx.output_buffer.get_pending_write(path).ok_or_else(|| {
        RecoverableError::with_hint(
            "ack handle expired or unknown",
            "Regenerate the write to get a fresh handle.",
        )
    })?;
    if stored.tool_name != tool_name {
        return Err(RecoverableError::with_hint(
            format!(
                "ack handle was minted by '{}', not '{}'",
                stored.tool_name, tool_name
            ),
            format!("Re-invoke {}(path=\"{}\") instead.", stored.tool_name, path),
        )
        .into());
    }
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    // Re-validate approvability (deny-list / breadth) before granting.
    validate_approve_path(&stored.approve_dir.to_string_lossy(), &root, &security)
        .map_err(|e| RecoverableError::new(e.to_string()))?;
    ctx.agent
        .add_session_write_root_for(ctx.workspace_override.as_deref(), stored.approve_dir.clone())
        .await;
    Ok(stored.input)
}

/// Phase B — resolve `raw_path` for writing. On an outside-root rejection,
/// stash the full input and return a `pending_ack` envelope instead of failing.
pub async fn resolve_write_or_capture(
    ctx: &ToolContext,
    tool_name: &str,
    input: &Value,
    raw_path: &str,
) -> anyhow::Result<WriteOutcome> {
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    let session_roots = ctx
        .agent
        .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
        .await;

    match classify_write_path(raw_path, &root, &security, &session_roots) {
        WritePathDecision::Allowed(p) => Ok(WriteOutcome::Write(p)),
        WritePathDecision::Denied(msg) => Err(RecoverableError::new(msg).into()),
        WritePathDecision::OutsideRoot { resolved } => {
            let dir = resolved
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| resolved.clone());
            // Pre-validate so a minted handle is guaranteed replayable.
            if let Err(e) = validate_approve_path(&dir.to_string_lossy(), &root, &security) {
                return Err(RecoverableError::new(e.to_string()).into());
            }
            let handle =
                ctx.output_buffer
                    .store_pending_write(tool_name.to_string(), input.clone(), dir.clone());
            Ok(WriteOutcome::Pending(json!({
                "pending_ack": handle,
                "reason": format!("'{}' is outside the project root", raw_path),
                "hint": format!(
                    "{}(path=\"{}\") to write it and approve {} for this session",
                    tool_name,
                    handle,
                    dir.display()
                ),
            })))
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib write_ack 2>&1`
Expected: PASS — all six tests.

- [ ] **Step 5: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/tools/core/write_ack.rs src/tools/core/mod.rs
git commit -m "feat(tools): shared out-of-scope write capture/replay helpers

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Wire `create_file` + end-to-end tests

**Files:**
- Modify: `src/tools/create_file.rs` (`CreateFile::call`)
- Test: `src/tools/edit_file/tests.rs` (where `create_file` tests already live; uses `project_ctx`)

**Interfaces:**
- Consumes: `maybe_replay_ack`, `resolve_write_or_capture`, `WriteOutcome` (Task 3, in scope as `super::*`).

- [ ] **Step 1: Write/adjust the failing tests**

In `src/tools/edit_file/tests.rs`, replace the body of `create_file_outside_project_rejected` to assert the new shape, and add the capture+replay tests:

```rust
#[tokio::test]
async fn create_file_outside_project_returns_pending_ack() {
    let (_dir, ctx) = project_ctx().await;
    let result = CreateFile
        .call(
            json!({ "path": "/var/outside_ce_test/evil.rs", "content": "evil code" }),
            &ctx,
        )
        .await
        .expect("out-of-scope write should return Ok(pending_ack), not Err");
    let handle = result["pending_ack"].as_str().expect("pending_ack handle");
    assert!(handle.starts_with("@ack_"), "got: {result}");
    // Content preserved server-side — not discarded.
    let stored = ctx.output_buffer.get_pending_write(handle).unwrap();
    assert_eq!(stored.input["content"], json!("evil code"));
    // Nothing was written.
    assert!(!std::path::Path::new("/var/outside_ce_test/evil.rs").exists());
}

#[tokio::test]
async fn create_file_ack_replay_writes_and_approves_dir() {
    let (_dir, ctx) = project_ctx().await;
    // A real, writable directory that is approvable. Mint the handle directly
    // (a path under the system temp dir is already allowed, so it would not
    // trigger capture — minting directly exercises the replay path).
    let ext = tempfile::tempdir().unwrap();
    let target = ext.path().join("plan.md");
    let handle = ctx.output_buffer.store_pending_write(
        "create_file".to_string(),
        json!({ "path": target.to_str().unwrap(), "content": "300 lines of plan" }),
        ext.path().to_path_buf(),
    );

    let result = CreateFile
        .call(json!({ "path": handle }), &ctx)
        .await
        .expect("replay should succeed");
    assert_eq!(result, json!("ok"));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "300 lines of plan");

    let roots = ctx.agent.session_write_roots_snapshot().await;
    assert!(roots.iter().any(|r| r == ext.path()), "dir approved: {roots:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib create_file_outside_project_returns_pending_ack create_file_ack_replay 2>&1`
Expected: FAIL — `create_file` still bails on the outside path / doesn't recognize the handle.

- [ ] **Step 3: Wire `CreateFile::call`**

Replace the top of `CreateFile::call` (use `edit_code action=replace` on `CreateFile/call`). Phase A runs first; `path`/`content` are extracted from the possibly-swapped input; the resolve block is replaced by Phase B:

```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let input = super::maybe_replay_ack(ctx, input, "create_file").await?;
        let path = super::require_str_param(&input, "path")?;
        let content = super::require_str_param(&input, "content")?;
        let overwrite = super::parse_bool_param(&input["overwrite"]);
        let resolved = match super::resolve_write_or_capture(ctx, "create_file", &input, path).await? {
            super::WriteOutcome::Write(p) => p,
            super::WriteOutcome::Pending(env) => return Ok(env),
        };
        if !overwrite && resolved.exists() {
            return Err(super::RecoverableError::with_hint(
                format!("file already exists: {}", resolved.display()),
                "Use edit_file to modify, or pass overwrite: true to replace. \
                 create_file is for new files only.",
            )
            .into());
        }
        crate::util::fs::write_utf8(&resolved, content)?;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent
            .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &resolved)
            .await;
        ctx.agent
            .mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved)
            .await;
        Ok(json!("ok"))
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib create_file 2>&1`
Expected: PASS — capture, replay, and the existing `create_file_within_project_works`.

- [ ] **Step 5: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/tools/create_file.rs src/tools/edit_file/tests.rs
git commit -m "feat(create_file): out-of-scope write returns ack handle, preserves content

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Wire `edit_markdown`

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs` (`EditMarkdown::call`)
- Test: `src/tools/markdown/tests.rs`

**Interfaces:**
- Consumes: `maybe_replay_ack`, `resolve_write_or_capture`, `WriteOutcome` via `crate::tools::*`.

- [ ] **Step 1: Write the failing test**

Add to `src/tools/markdown/tests.rs` (match its existing ctx-builder; if it has none, copy the `ctx_with_project` shape from Task 3 Step 2):

```rust
#[tokio::test]
async fn edit_markdown_outside_project_returns_pending_ack() {
    let (_dir, ctx) = test_ctx().await; // existing helper in this module
    let result = EditMarkdown
        .call(
            json!({
                "path": "/var/outside_ce_md/notes.md",
                "heading": "## Notes",
                "action": "replace",
                "content": "new body"
            }),
            &ctx,
        )
        .await
        .expect("out-of-scope edit should return Ok(pending_ack)");
    let handle = result["pending_ack"].as_str().expect("pending_ack handle");
    assert!(handle.starts_with("@ack_"), "got: {result}");
    let stored = ctx.output_buffer.get_pending_write(handle).unwrap();
    assert_eq!(stored.tool_name, "edit_markdown");
    assert_eq!(stored.input["content"], json!("new body"));
}
```

(If the test module's ctx helper has a different name, use it; the assertions are unchanged.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib edit_markdown_outside_project_returns_pending_ack 2>&1`
Expected: FAIL — currently bails (and the `.md` gate would reject an `@ack_` path on replay).

- [ ] **Step 3: Wire `EditMarkdown::call`**

At the top of `EditMarkdown::call`, insert Phase A **before** the `.md` gate, and replace the resolve block with Phase B. Use `edit_file` to make these two edits.

Replace:
```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        crate::tools::guard_worktree_write(ctx).await?;
        let path = crate::tools::require_str_param(&input, "path")?;
```
with:
```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        crate::tools::guard_worktree_write(ctx).await?;
        let input = crate::tools::maybe_replay_ack(ctx, input, "edit_markdown").await?;
        let path = crate::tools::require_str_param(&input, "path")?;
```

Replace the resolve block:
```rust
        let resolved = crate::util::path_security::validate_write_path(
            path,
            &root,
            &security,
            &session_roots,
        )?;
```
with:
```rust
        let resolved = match crate::tools::resolve_write_or_capture(ctx, "edit_markdown", &input, path).await? {
            crate::tools::WriteOutcome::Write(p) => p,
            crate::tools::WriteOutcome::Pending(env) => return Ok(env),
        };
```

Then delete the now-unused `root` / `security` / `session_roots` bindings that preceded the old resolve block (they are recomputed inside the helper). Verify with clippy's `unused_variables`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib markdown 2>&1`
Expected: PASS — new test plus all existing markdown tests.

- [ ] **Step 5: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/tools/markdown/edit_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(edit_markdown): out-of-scope write returns ack handle

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Wire `edit_file` (three resolve sites)

**Files:**
- Modify: `src/tools/edit_file/mod.rs` (`EditFile::call` two sites + `perform_edit` one site)
- Test: `src/tools/edit_file/tests.rs`

**Interfaces:**
- Consumes: `maybe_replay_ack`, `resolve_write_or_capture`, `WriteOutcome` via `super::*`.

- [ ] **Step 1: Write the failing test**

Add to `src/tools/edit_file/tests.rs`:

```rust
#[tokio::test]
async fn edit_file_outside_project_returns_pending_ack() {
    let (_dir, ctx) = project_ctx().await;
    let result = EditFile
        .call(
            json!({
                "path": "/var/outside_ce_ef/x.txt",
                "old_string": "a",
                "new_string": "b"
            }),
            &ctx,
        )
        .await
        .expect("out-of-scope edit should return Ok(pending_ack)");
    let handle = result["pending_ack"].as_str().expect("pending_ack handle");
    assert!(handle.starts_with("@ack_"), "got: {result}");
    let stored = ctx.output_buffer.get_pending_write(handle).unwrap();
    assert_eq!(stored.tool_name, "edit_file");
    assert_eq!(stored.input["new_string"], json!("b"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib edit_file_outside_project_returns_pending_ack 2>&1`
Expected: FAIL — currently bails.

- [ ] **Step 3: Phase A at the top of `EditFile::call`**

The `.md` redirect gate reads `path`, so Phase A must precede it. Replace:
```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let path = super::require_str_param(&input, "path")?;
        let new_string = input["new_string"].as_str().unwrap_or("");
```
with:
```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let input = super::maybe_replay_ack(ctx, input, "edit_file").await?;
        let path = super::require_str_param(&input, "path")?;
        let new_string = input["new_string"].as_str().unwrap_or("");
```

- [ ] **Step 4: Replace the batch-mode resolve site (in `call`)**

Replace the batch-branch block:
```rust
            let root = ctx
                .agent
                .require_project_root_for(ctx.workspace_override.as_deref())
                .await?;
            let security = ctx
                .agent
                .security_config_for(ctx.workspace_override.as_deref())
                .await;
            let session_roots = ctx
                .agent
                .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
                .await;
            let resolved = crate::util::path_security::validate_write_path(
                path,
                &root,
                &security,
                &session_roots,
            )?;
```
with:
```rust
            let resolved = match super::resolve_write_or_capture(ctx, "edit_file", &input, path).await? {
                super::WriteOutcome::Write(p) => p,
                super::WriteOutcome::Pending(env) => return Ok(env),
            };
```

- [ ] **Step 5: Replace the second resolve site (in `call`)**

Apply the identical replacement to the other `let root … session_roots … validate_write_path` block inside `call` (the single-edit/insert branch around the former line 497-507). The replacement text is the same as Step 4.

- [ ] **Step 6: Replace the `perform_edit` resolve site**

`perform_edit` is a free function with signature `perform_edit(... path: &str, ... ctx: &ToolContext ...)`. It already receives `path` and `ctx`. Replace its resolve block:
```rust
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    let session_roots = ctx
        .agent
        .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
        .await;
    let resolved =
        crate::util::path_security::validate_write_path(path, &root, &security, &session_roots)?;
```
with:
```rust
    let resolved = match crate::tools::resolve_write_or_capture(ctx, "edit_file", input, path).await? {
        crate::tools::WriteOutcome::Write(p) => p,
        crate::tools::WriteOutcome::Pending(env) => return Ok(env),
    };
```

`perform_edit` is the **sole** resolver for the single `old_string`/`new_string`
edit (the most common mode and a mutually-exclusive branch), so it MUST capture
with the full input — not just `path`. Add an `input: &Value` parameter (before
`ctx`):
```rust
async fn perform_edit(
    path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    input: &Value,
    ctx: &ToolContext,
) -> Result<Value> {
```
and update the single call site at the end of `call`:
```rust
        perform_edit(path, old_string, new_string, replace_all, &input, ctx).await
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test --lib edit_file 2>&1`
Expected: PASS — new test plus all existing `edit_file` tests.

- [ ] **Step 8: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/tools/edit_file/mod.rs src/tools/edit_file/tests.rs
git commit -m "feat(edit_file): out-of-scope write returns ack handle at all resolve sites

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Prompt surfaces

**Files:**
- Modify: `src/tools/create_file.rs` (description), `src/tools/edit_file/mod.rs` (description), `src/tools/markdown/edit_markdown.rs` (description)
- Modify: `src/prompts/guides/progressive-disclosure.md` (the `@ack_*` handle-kinds note)
- Test: existing `prompt_surfaces_reference_only_real_tools` suite

**Interfaces:** none (documentation strings).

- [ ] **Step 1: Append an ack note to each of the three tool descriptions**

Append this sentence to the `description()` string of `create_file`, `edit_file`, and `edit_markdown` (keep each tool's existing wording; add at the end):

```
 Writing outside the project root returns an @ack_* handle instead of failing; re-invoke with path="@ack_..." to write it (approves the directory for the session) without re-sending content.
```

- [ ] **Step 2: Update the progressive-disclosure guide**

In `src/prompts/guides/progressive-disclosure.md`, change the sibling-kinds line so `@ack_*` reflects both uses. Replace:
```
`@file_*` and `@ack_*` are sibling handle kinds — same mechanics.
```
with:
```
`@file_*` and `@ack_*` are sibling handle kinds — same mechanics. `@ack_*`
covers both dangerous commands and out-of-scope writes: re-invoke the tool with
the handle to acknowledge and proceed.
```

- [ ] **Step 3: Run the prompt-consistency + guide tests**

Run: `cargo test --lib prompt 2>&1`
Expected: PASS — `prompt_surfaces_reference_only_real_tools` and related guide-snapshot tests. No `ONBOARDING_VERSION` bump is needed (the `onboarding_prompt` slice is unchanged).

If a guide snapshot test fails because the guide text is fixture-checked, regenerate per the repo convention (`UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib <test>` if that env switch is used by the failing test) and re-run.

- [ ] **Step 4: Gate + commit**

Run: `cargo fmt && cargo clippy --lib -- -D warnings 2>&1`
Expected: exit 0.

```bash
git add src/tools/create_file.rs src/tools/edit_file/mod.rs src/tools/markdown/edit_markdown.rs src/prompts/guides/progressive-disclosure.md
git commit -m "docs(prompts): document out-of-scope write ack flow on write tools

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: Full-suite verification + live MCP check

**Files:** none (verification only).

- [ ] **Step 1: Full lib suite**

Run: `cargo test --lib 2>&1`
Expected: PASS (no regressions). Note: dashboard tests are feature-gated and shown as filtered — that is expected.

- [ ] **Step 2: Live build + reconnect (manual)**

Run: `cargo rb` then reconnect with `/mcp`. Manually verify: `create_file(path="/var/ce_live/plan.md", content="...")` returns a `pending_ack`; `create_file(path="@ack_<id>")` writes and reports the directory approved.

- [ ] **Step 3: Final commit (if any snapshot/regen changes)**

```bash
git add -A
git commit -m "test: verify out-of-scope write ack flow end-to-end

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Out of Scope / Follow-up

**`edit_code` is intentionally not wired in this plan.** Unlike the other three
tools, `edit_code` resolves the user path via the *unpinned* `resolve_write_path`
(`src/fs/mod.rs`) at the start of each operation and then re-validates each
LSP-planned write in a loop (`src/tools/symbol/edit_code.rs`); outside-project
files are also treated as read-only libraries that lack writable LSP symbols, so
an out-of-scope `edit_code` write is largely non-functional regardless. Wiring it
would require capturing at the first `resolve_write_path(rel_path)` site plus
auditing the LSP-planned-writes second validation — a separate, lower-value
effort. Track as a follow-up if a real `edit_code`-outside-project case appears.

**Other deferrals (from the spec §7):** configurable allow-patterns for sibling
directories; persisting session write roots across restarts. Both unchanged.

## Self-Review

- **Spec coverage:** §1 goal → Tasks 4-6; §3 ADR-1 → Task 3; ADR-2 → Task 1;
  ADR-3 → Task 2; §4.1 capture → Task 3 `resolve_write_or_capture` + Tasks 4-6;
  §4.2 replay → Task 3 `maybe_replay_ack`; §4.3 integration → Tasks 4-6; §5
  error/security → Tasks 1 (Denied) + 3 (pre-validate, cross-tool, expiry); §6
  testing → tests in Tasks 1-6; prompt surfaces → Task 7. `edit_code` (spec
  named all four) is consciously deferred with rationale above — surfaced to the
  user at handoff.
- **Placeholder scan:** none — every code step shows full code; every run step
  shows command + expected result.
- **Type consistency:** `WritePathDecision` (Task 1) consumed in Task 3;
  `WriteOutcome::{Write,Pending}` defined in Task 3 and matched identically in
  Tasks 4-6; `store_pending_write`/`get_pending_write` signatures (Task 2)
  match their callers (Task 3); `maybe_replay_ack`/`resolve_write_or_capture`
  signatures match all call sites.
