# Cross-Process Write Serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent write races when multiple codescout instances run against the same project by serializing mutations via an OS-level advisory file lock.

**Architecture:** Each `ActiveProject` holds two cooperating write guards — an in-process `tokio::sync::Mutex<()>` for same-process ordering and a cross-process `fd_lock::RwLock<File>` on `.codescout/write.lock` for inter-process safety. Both are acquired (in that order) by a centralized gate in `CodeScoutServer::call_tool_inner` when the dispatched tool is a write.

**Tech Stack:** Rust, tokio, `fs4` crate (simpler API than `fd-lock` — `try_lock_exclusive(&self)` rather than `&mut self`, so no unsafe lifetime dance), existing `RecoverableError` pattern.

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | Add `fs4 = "0.12"` dependency. |
| `.gitignore` | Ignore `.codescout/write.lock`. |
| `src/config/project.rs` | `SecuritySection::write_lock_timeout_secs` + default. |
| `src/agent.rs` | `ActiveProject` gains `write_lock` + `file_lock` fields; `open_write_lock_file` helper; updated construction at all three sites. |
| `src/agent/write_guard.rs` | **New** — `WriteGuard` struct, `Agent::acquire_write_guard()` helper, deadlock-order docs. |
| `src/server.rs` | `WRITE_TOOLS` const, `is_write_call()` helper, gate in `call_tool_inner`. |
| `tests/cross_process_write_lock.rs` | **New** — spawn two binaries, assert one wins / one recovers. |
| `tests/mcp-smoke-rust.sh` | Add one smoke-test case (`test_write_lock_contention`). |

---

## Task 1: Add `fd-lock` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add the crate to `[dependencies]`**

Insert the following line alphabetically within `[dependencies]`:

```toml
fs4 = "0.12"
```

- [ ] **Step 2: Run `cargo build` to pull the crate**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished ... profile` and no compile errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add fd-lock dependency for write serialization"
```

---

## Task 2: Add `write_lock_timeout_secs` config field

**Files:**
- Modify: `src/config/project.rs` (`SecuritySection` + `default_write_lock_timeout`)

- [ ] **Step 1: Write the failing test**

Append to the existing `tests` module in `src/config/project.rs` (find the `#[cfg(test)] mod tests` block and add inside it):

```rust
    #[test]
    fn security_section_defaults_write_lock_timeout_to_5s() {
        let toml = "";
        let config: SecuritySection = toml::from_str(toml).unwrap();
        assert_eq!(config.write_lock_timeout_secs, 5);
    }

    #[test]
    fn security_section_accepts_custom_write_lock_timeout() {
        let toml = "write_lock_timeout_secs = 10";
        let config: SecuritySection = toml::from_str(toml).unwrap();
        assert_eq!(config.write_lock_timeout_secs, 10);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test security_section_defaults_write_lock_timeout_to_5s 2>&1 | tail -15`
Expected: FAIL with `no field 'write_lock_timeout_secs'` or similar compile error.

- [ ] **Step 3: Add the field + default function**

Add a new field to `SecuritySection` (just before the closing `}`):

```rust
    /// Seconds to wait for the cross-process write lock before returning a
    /// RecoverableError. Default: 5.
    #[serde(default = "default_write_lock_timeout")]
    pub write_lock_timeout_secs: u64,
```

Add a new default function alongside the existing `default_*` functions (`default_shell_mode`, `default_shell_output_limit`, `default_true`):

```rust
fn default_write_lock_timeout() -> u64 {
    5
}
```

Also update `impl Default for SecuritySection` — find the existing `impl Default` block and add `write_lock_timeout_secs: 5,` to the field list.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout config::project 2>&1 | tail -15`
Expected: both new tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/project.rs
git commit -m "feat(config): add security.write_lock_timeout_secs"
```

---

## Task 3: Add `.gitignore` entry for lock file

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Add entry**

Append to `.gitignore`:

```
# Cross-process write lock file (per-project; recreated on activate)
.codescout/write.lock
```

- [ ] **Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: ignore .codescout/write.lock"
```

---

## Task 4: Create `WriteGuard` module

**Files:**
- Create: `src/agent/write_guard.rs`
- Modify: `src/agent.rs` (add `mod write_guard;`)

This module owns the `WriteGuard` RAII type and the deadlock-order invariant. Putting it in its own file keeps `agent.rs` (already 1000+ lines) focused on `Agent` and `ActiveProject`.

- [ ] **Step 1: Create the new module file**

Create `src/agent/write_guard.rs`:

```rust
//! RAII guard held by the write-tool gate in `CodeScoutServer::call_tool_inner`.
//!
//! Two layers:
//! 1. Async `tokio::sync::Mutex<()>` — serializes writes inside a single
//!    codescout process. Acquired FIRST.
//! 2. `flock` (via `fs4`) on `.codescout/write.lock` — serializes writes
//!    across codescout processes on the same project. Acquired SECOND.
//!
//! Order matters: always inner mutex → outer flock. Releasing happens in
//! reverse order on drop (flock released first, then async mutex).

use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fs4::fs_std::FileExt;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::tools::RecoverableError;

/// Held for the duration of a single write-tool call.
/// Drop order: the async_guard drops last (Rust drops struct fields in
/// declaration order), so we declare the file-lock handle first.
pub struct WriteGuard {
    file: Arc<File>,
    _async_guard: OwnedMutexGuard<()>,
}

impl Drop for WriteGuard {
    fn drop(&mut self) {
        // Release the flock explicitly — documents intent. Closing the fd
        // would also release it, but we keep the File alive in an Arc across
        // calls, so an explicit unlock is required.
        let _ = FileExt::unlock(&*self.file);
    }
}

/// Acquire both locks.  Returns `RecoverableError` on cross-process timeout
/// so the caller can surface it as `isError: false`.
pub async fn acquire(
    async_mutex: Arc<AsyncMutex<()>>,
    file: Arc<File>,
    timeout: Duration,
) -> Result<WriteGuard, RecoverableError> {
    let async_guard = async_mutex.lock_owned().await;

    let file_clone = file.clone();
    let acquired = tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        loop {
            match FileExt::try_lock_exclusive(&*file_clone) {
                Ok(true) => return true,
                Ok(false) | Err(_) => {
                    if start.elapsed() >= timeout {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    })
    .await
    .unwrap_or(false);

    if !acquired {
        return Err(RecoverableError::with_hint(
            "another codescout instance is writing to this project",
            "Retry in a moment — the holder should release shortly.",
        ));
    }

    Ok(WriteGuard {
        file,
        _async_guard: async_guard,
    })
}

/// Open (or create) the lock file at `.codescout/write.lock` under `root`.
/// Idempotent; safe to call on an existing file. Returns an `Arc<File>` so
/// the descriptor can be shared by every tool call without re-opening.
pub fn open_lock_file(root: &Path) -> std::io::Result<Arc<File>> {
    let dir = root.join(".codescout");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("write.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    Ok(Arc::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn acquire_returns_guard_when_uncontended() {
        let dir = tempdir().unwrap();
        let fd = open_lock_file(dir.path()).unwrap();
        let m = Arc::new(AsyncMutex::new(()));
        let g = acquire(m, fd, Duration::from_secs(1)).await.unwrap();
        drop(g); // released
    }

    #[tokio::test]
    async fn acquire_times_out_on_cross_process_contention() {
        // Emulate a second process by opening a SEPARATE File handle on the
        // same path (flock is per-open-file-description, not per-fd).
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();
        // Sanity: they must be different File handles.
        assert!(!Arc::ptr_eq(&fd_a, &fd_b));

        let m_a = Arc::new(AsyncMutex::new(()));
        let m_b = Arc::new(AsyncMutex::new(()));

        let _held = acquire(m_a, fd_a, Duration::from_secs(1)).await.unwrap();

        let r = acquire(m_b, fd_b, Duration::from_millis(200)).await;
        assert!(r.is_err(), "second process should time out");
    }

    #[tokio::test]
    async fn guard_drop_releases_lock() {
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();

        {
            let _g = acquire(
                Arc::new(AsyncMutex::new(())),
                fd_a,
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        } // guard drops here → flock released

        let r = acquire(
            Arc::new(AsyncMutex::new(())),
            fd_b,
            Duration::from_millis(500),
        )
        .await;
        assert!(r.is_ok(), "second acquire should succeed after first drops");
    }

    #[tokio::test]
    async fn open_lock_file_creates_codescout_dir() {
        let dir = tempdir().unwrap();
        let _ = open_lock_file(dir.path()).unwrap();
        assert!(dir.path().join(".codescout/write.lock").exists());
    }
}
```

**Note on `fs4` versioning:** if `fs4 = "0.12"` doesn't expose `fs4::fs_std::FileExt`, adjust to whatever path the version re-exports (`fs4::FileExt` on older versions). Run `cargo doc --package fs4 --open` if uncertain.

- [ ] **Step 2: Register the module**

In `src/agent.rs`, add to the top-level module declarations (near other `mod` or `use` lines at the top of the file):

```rust
mod write_guard;
pub(crate) use write_guard::{acquire as acquire_write_guard, open_lock_file, WriteGuard};
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test agent::write_guard 2>&1 | tail -20`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/agent/write_guard.rs src/agent.rs
git commit -m "feat(agent): add WriteGuard RAII for cross-process write lock"
```

---

## Task 5: Add lock fields to `ActiveProject`

**Files:**
- Modify: `src/agent.rs` (`ActiveProject` struct + three construction sites at lines 144, 246, 430)

- [ ] **Step 1: Add the two fields to the struct**

Find `pub struct ActiveProject` (around line 89) and add these fields before the closing brace:

```rust
    /// Async mutex serializing writes within this process.
    /// Acquired FIRST in the write-lock order (see agent::write_guard).
    pub(crate) write_lock: Arc<tokio::sync::Mutex<()>>,
    /// Shared file descriptor for the cross-process advisory lock at
    /// `.codescout/write.lock`. The flock is per-open-file-description, so a
    /// single File handle shared by every tool call in this process (via Arc)
    /// is sufficient — in-process ordering is handled by `write_lock` above.
    pub(crate) file_lock: Arc<std::fs::File>,
```

- [ ] **Step 2: Update all three construction sites**

For each of the three sites (lines ~144, ~246, ~430 in `src/agent.rs` — grep with `grep -n "ActiveProject {" src/agent.rs`), add the two fields at the same indentation as `head_sha`:

```rust
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
            file_lock: write_guard::open_lock_file(&root)
                .with_context(|| format!("failed to open write.lock for {}", root.display()))?,
```

For the third site (around line 430), the root variable is named `abs_root` — substitute accordingly. All three enclosing functions already return `anyhow::Result` (or compatible), so `?` propagation works. Ensure `use anyhow::Context;` is imported at the top of `src/agent.rs` — check with `grep -n "use anyhow" src/agent.rs` and add if absent.

- [ ] **Step 3: Build to verify**

Run: `cargo build 2>&1 | tail -10`
Expected: clean build, no errors.

- [ ] **Step 4: Run existing agent tests**

Run: `cargo test agent:: 2>&1 | tail -15`
Expected: all existing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src/agent.rs
git commit -m "feat(agent): wire write_lock and file_lock into ActiveProject"
```

---

## Task 6: Add `WRITE_TOOLS` and `is_write_call` helper in server.rs

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Write the failing test**

Append this test to the existing `#[cfg(test)] mod tests` block in `src/server.rs`:

```rust
    #[test]
    fn is_write_call_classifies_plain_writes() {
        use serde_json::json;
        assert!(is_write_call("edit_file", &json!({})));
        assert!(is_write_call("create_file", &json!({})));
        assert!(is_write_call("replace_symbol", &json!({})));
        assert!(is_write_call("insert_code", &json!({})));
        assert!(is_write_call("remove_symbol", &json!({})));
        assert!(is_write_call("rename_symbol", &json!({})));
        assert!(!is_write_call("read_file", &json!({})));
        assert!(!is_write_call("find_symbol", &json!({})));
    }

    #[test]
    fn is_write_call_memory_depends_on_action() {
        use serde_json::json;
        assert!(is_write_call("memory", &json!({"action": "write"})));
        assert!(is_write_call("memory", &json!({"action": "remember"})));
        assert!(is_write_call("memory", &json!({"action": "forget"})));
        assert!(is_write_call("memory", &json!({"action": "delete"})));
        assert!(is_write_call("memory", &json!({"action": "refresh_anchors"})));
        assert!(!is_write_call("memory", &json!({"action": "read"})));
        assert!(!is_write_call("memory", &json!({"action": "list"})));
        assert!(!is_write_call("memory", &json!({"action": "recall"})));
        assert!(!is_write_call("memory", &json!({})));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test is_write_call 2>&1 | tail -10`
Expected: FAIL — `is_write_call` undefined.

- [ ] **Step 3: Add the constant and helper**

Add at module level in `src/server.rs` (near the top, above `impl CodeScoutServer`):

```rust
/// Tools whose successful execution mutates project state. See the design spec
/// at docs/superpowers/specs/2026-04-17-cross-process-write-serialization-design.md.
const WRITE_TOOLS: &[&str] = &[
    "create_file",
    "edit_file",
    "edit_markdown",
    "replace_symbol",
    "insert_code",
    "remove_symbol",
    "rename_symbol",
];

/// Memory actions that write to the memory store. `memory` with any other
/// action (read, list, recall) is treated as a read and bypasses the lock.
const MEMORY_WRITE_ACTIONS: &[&str] = &["write", "remember", "forget", "delete", "refresh_anchors"];

/// Returns true if the tool call will mutate project state and therefore
/// must acquire the write lock.
fn is_write_call(tool_name: &str, input: &serde_json::Value) -> bool {
    if WRITE_TOOLS.contains(&tool_name) {
        return true;
    }
    if tool_name == "memory" {
        if let Some(action) = input.get("action").and_then(|v| v.as_str()) {
            return MEMORY_WRITE_ACTIONS.contains(&action);
        }
    }
    false
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test is_write_call 2>&1 | tail -10`
Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/server.rs
git commit -m "feat(server): add is_write_call helper and WRITE_TOOLS registry"
```

---

## Task 7: Gate write dispatch on the write guard

**Files:**
- Modify: `src/server.rs` (`call_tool_inner` method, around lines 223-383)

- [ ] **Step 1: Find the dispatch point**

Locate this block in `call_tool_inner`:

```rust
        let tool_call_fut = recorder.record_content(&req.name, &input_for_record, || {
            tool.call_content(input, &ctx)
        });
```

- [ ] **Step 2: Wrap the dispatch with the guard acquisition**

Replace the block above with:

```rust
        // Acquire the write guard if this is a mutating call. Read calls skip
        // the lock entirely. The guard is held for the full duration of the
        // tool future; it drops when the future completes or is cancelled.
        let write_guard = if is_write_call(&req.name, &input_for_record) {
            let (mutex, fd_lock, timeout_secs) = self
                .agent
                .with_project(|p| {
                    Ok((
                        p.write_lock.clone(),
                        p.file_lock.clone(),
                        p.config.security.write_lock_timeout_secs,
                    ))
                })
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("write gate: {}", e), None)
                })?;
            match crate::agent::acquire_write_guard(
                mutex,
                fd_lock,
                std::time::Duration::from_secs(timeout_secs),
            )
            .await
            {
                Ok(g) => Some(g),
                Err(rec_err) => {
                    // Route to isError: false so sibling calls survive.
                    return Ok(route_tool_error(rec_err.into()));
                }
            }
        } else {
            None
        };

        let tool_call_fut = recorder.record_content(&req.name, &input_for_record, || {
            tool.call_content(input, &ctx)
        });
```

And at the bottom of the method (before the final `Ok(call_result)`), add:

```rust
        drop(write_guard);
```

This is documentary — the guard would drop anyway at the end of scope, but an explicit drop marks the release point.

- [ ] **Step 3: Verify `RecoverableError` → `anyhow::Error` conversion exists**

Run: `grep -n "impl From<RecoverableError>\|impl Into<anyhow" src/tools/mod.rs src/server.rs`
If no conversion exists, add one in `src/tools/mod.rs` after the `impl std::error::Error for RecoverableError` block:

```rust
impl From<RecoverableError> for anyhow::Error {
    fn from(e: RecoverableError) -> Self {
        anyhow::Error::new(e)
    }
}
```

(If the conversion already exists, skip this substep.)

- [ ] **Step 4: Build and fix compile errors**

Run: `cargo build 2>&1 | tail -20`
Expected: clean build. If `route_tool_error` takes a different arg shape, adjust accordingly — inspect with `grep -n "fn route_tool_error" src/server.rs`.

- [ ] **Step 5: Run existing server tests**

Run: `cargo test server:: 2>&1 | tail -20`
Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src/server.rs src/tools/mod.rs
git commit -m "feat(server): gate write-tool dispatch on cross-process write lock"
```

---

## Task 8: Integration test — two processes contend

**Files:**
- Create: `tests/cross_process_write_lock.rs`

- [ ] **Step 1: Write the test**

Create `tests/cross_process_write_lock.rs`:

```rust
//! End-to-end: two codescout processes against the same project. One write
//! wins; the other returns a RecoverableError with the contention message.

use std::process::Stdio;
use std::time::Duration;

use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn binary_path() -> std::path::PathBuf {
    // `cargo test` puts the test binary in target/<profile>/deps/.
    // The server binary lives at target/<profile>/codescout.
    let mut path = std::env::current_exe().unwrap();
    path.pop();  // deps
    path.pop();  // profile dir
    path.push("codescout");
    path
}

async fn send_activate(stdin: &mut tokio::process::ChildStdin, project_root: &str, id: u64) {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "activate_project",
            "arguments": { "path": project_root, "read_only": false }
        }
    });
    stdin.write_all(format!("{}\n", req).as_bytes()).await.unwrap();
}

async fn send_edit(stdin: &mut tokio::process::ChildStdin, file: &str, id: u64) {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": "edit_file",
            "arguments": {
                "path": file,
                "old_string": "hello",
                "new_string": "HELLO"
            }
        }
    });
    stdin.write_all(format!("{}\n", req).as_bytes()).await.unwrap();
}

#[tokio::test]
async fn two_instances_contending_produces_one_winner_one_recoverable() {
    let bin = binary_path();
    if !bin.exists() {
        eprintln!("skipping: binary not built at {}", bin.display());
        return;
    }

    let dir = tempdir().unwrap();
    let project = dir.path();
    std::fs::create_dir_all(project.join(".codescout")).unwrap();
    std::fs::write(project.join("target.txt"), "hello").unwrap();

    // Spawn two server instances with stdio transport.
    let mut a = Command::new(&bin)
        .args(["start", "--transport", "stdio", "--project", project.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut b = Command::new(&bin)
        .args(["start", "--transport", "stdio", "--project", project.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut a_stdin = a.stdin.take().unwrap();
    let mut b_stdin = b.stdin.take().unwrap();
    let mut a_out = BufReader::new(a.stdout.take().unwrap());
    let mut b_out = BufReader::new(b.stdout.take().unwrap());

    // Fire edit_file at roughly the same moment.
    send_edit(&mut a_stdin, "target.txt", 1).await;
    send_edit(&mut b_stdin, "target.txt", 1).await;

    let mut a_line = String::new();
    let mut b_line = String::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        a_out.read_line(&mut a_line).await.unwrap();
        b_out.read_line(&mut b_line).await.unwrap();
    })
    .await
    .expect("both replies should arrive within 10s");

    // Exactly one should indicate success; the other should carry the
    // contention message. We don't care which is which.
    let a_contended = a_line.contains("another codescout instance");
    let b_contended = b_line.contains("another codescout instance");
    assert_ne!(
        a_contended, b_contended,
        "expected exactly one contention recovery. a={} b={}",
        a_line, b_line
    );

    a.kill().await.ok();
    b.kill().await.ok();
}
```

- [ ] **Step 2: Build the binary so the test can spawn it**

Run: `cargo build 2>&1 | tail -5`
Expected: clean build.

- [ ] **Step 3: Run the integration test**

Run: `cargo test --test cross_process_write_lock 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/cross_process_write_lock.rs
git commit -m "test: cross-process write lock contention integration test"
```

---

## Task 9: Add smoke test case

**Files:**
- Modify: `tests/mcp-smoke-rust.sh`

- [ ] **Step 1: Inspect how existing tests are shaped**

Run: `grep -n "^test_" tests/mcp-smoke-rust.sh | tail -10`
Confirm the pattern: each test is a bash function starting with `test_`.

- [ ] **Step 2: Add a smoke test**

Open `tests/mcp-smoke-rust.sh` and append before the final summary block:

```bash
test_write_lock_gate_accepts_single_write() {
    call edit_file '{"path": "README.md", "old_string": "nonexistent_sentinel_XYZ", "new_string": "nonexistent_sentinel_XYZ"}'
    if assert_contains "error"; then
        pass 1 "edit_file returns an error for unmatched old_string (lock gate did not block path)"
    else
        fail 1 "edit_file with bad old_string" "expected an error response"
    fi
}
```

Then register the call at the bottom where other tests run: find the section that invokes each `test_*` function and add `test_write_lock_gate_accepts_single_write`.

(Full cross-instance testing is already covered by the Rust integration test in Task 8; this smoke test just confirms the gate does not break normal writes.)

- [ ] **Step 3: Run smoke tests**

Run: `bash tests/mcp-smoke-rust.sh 2>&1 | tail -20`
Expected: all tests pass including the new one.

- [ ] **Step 4: Commit**

```bash
git add tests/mcp-smoke-rust.sh
git commit -m "test: smoke-test write lock gate on single-instance edit_file"
```

---

## Task 10: Full verification pass

**Files:** (no edits — verification only)

- [ ] **Step 1: `cargo fmt`**

Run: `cargo fmt`
Expected: no output.

- [ ] **Step 2: `cargo clippy`**

Run: `cargo clippy -- -D warnings 2>&1 | tail -15`
Expected: clean.

- [ ] **Step 3: Full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: all tests pass including the new integration test.

- [ ] **Step 4: Release build for MCP-server validation**

Run: `cargo build --release 2>&1 | tail -5`
Expected: `Finished release profile`.

- [ ] **Step 5: Manual: restart MCP and try a write**

Ask the user to run `/mcp`, then perform any write (`edit_file` on a scratch file) and confirm no regression.

- [ ] **Step 6: Commit if needed, or mark complete**

If any formatting or clippy fixes were needed:

```bash
git add -u
git commit -m "chore: fmt + clippy fixes after write-lock feature"
```

Otherwise, feature is complete.

---

## Summary of Commits

Expected commit sequence (10 commits, each green on build + clippy + tests):

1. `chore: add fd-lock dependency for write serialization`
2. `feat(config): add security.write_lock_timeout_secs`
3. `chore: ignore .codescout/write.lock`
4. `feat(agent): add WriteGuard RAII for cross-process write lock`
5. `feat(agent): wire write_lock and file_lock into ActiveProject`
6. `feat(server): add is_write_call helper and WRITE_TOOLS registry`
7. `feat(server): gate write-tool dispatch on cross-process write lock`
8. `test: cross-process write lock contention integration test`
9. `test: smoke-test write lock gate on single-instance edit_file`
10. `chore: fmt + clippy fixes after write-lock feature` (if needed)
