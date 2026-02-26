# Graceful LSP Shutdown Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure all child LSP processes (rust-analyzer, kotlin-language-server, etc.) are gracefully shut down when code-explorer exits, preventing orphaned processes that consume GB of memory.

**Architecture:** Add signal handling (SIGINT/SIGTERM) and structured shutdown to `server::run()`. The `LspManager` is passed into a shutdown hook that calls `shutdown_all()` before the process exits. The `LspClient::Drop` impl is enhanced to store the child process handle and kill it synchronously as a last-resort safety net.

**Tech Stack:** tokio signals (`tokio::signal`), `Arc<LspManager>` (already used)

---

### Task 1: Store child process handle in LspClient for reliable kill-on-drop

**Files:**
- Modify: `src/lsp/client.rs:93-108` (LspClient struct)
- Modify: `src/lsp/client.rs:112-219` (LspClient::start)
- Modify: `src/lsp/client.rs:621-631` (Drop impl)

**Step 1: Add `child_pid` field to `LspClient`**

In the `LspClient` struct (line 93), add a field to store the child process ID:

```rust
pub struct LspClient {
    writer: Mutex<ChildStdin>,
    next_id: AtomicI64,
    pending: Arc<StdMutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>,
    alive: Arc<AtomicBool>,
    reader_handle: StdMutex<Option<JoinHandle<()>>>,
    pub workspace_root: PathBuf,
    pub capabilities: StdMutex<lsp_types::ServerCapabilities>,
    /// PID of the child LSP server process, for kill-on-drop safety net
    child_pid: Option<u32>,
}
```

**Step 2: Capture `child.id()` before moving child into the reader task**

In `LspClient::start` (around line 130), capture the PID before `child` is moved:

```rust
let child_pid = child.id();
```

Add this line right after `let stderr = child.stderr.take(...)` (line 129), before the reader task spawn.

Then set it in the `Self` constructor (around line 205):

```rust
let client = Self {
    writer: Mutex::new(stdin),
    next_id: AtomicI64::new(1),
    pending,
    alive,
    reader_handle: StdMutex::new(Some(reader_handle)),
    workspace_root: config.workspace_root.clone(),
    capabilities: StdMutex::new(lsp_types::ServerCapabilities::default()),
    child_pid,
};
```

**Step 3: Enhance Drop impl to kill child process**

Replace the Drop impl (line 621) with:

```rust
impl Drop for LspClient {
    fn drop(&mut self) {
        // Abort the reader task
        if let Ok(mut guard) = self.reader_handle.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
        // Kill the child process as a safety net.
        // The graceful shutdown path (shutdown_all -> shutdown) sends LSP
        // shutdown/exit first.  This ensures the process dies even if the
        // graceful path was skipped (e.g., panic, abrupt exit).
        if let Some(pid) = self.child_pid {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }
    }
}
```

**Step 4: Add `libc` dependency**

Run: `cargo add libc`

**Step 5: Build and verify it compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles successfully

**Step 6: Run existing tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all existing tests pass

**Step 7: Commit**

```bash
git add src/lsp/client.rs Cargo.toml Cargo.lock
git commit -m "fix: store child PID in LspClient for reliable kill-on-drop"
```

---

### Task 2: Add graceful shutdown to the stdio transport path

**Files:**
- Modify: `src/server.rs:225-251` (run function, stdio branch)

**Step 1: Add signal handling and shutdown to stdio path**

Replace the `"stdio"` match arm in `run()` (lines 238-251) with:

```rust
"stdio" => {
    if auth_token.is_some() {
        tracing::warn!("--auth-token is ignored for stdio transport");
    }
    tracing::info!("code-explorer MCP server ready (stdio)");
    let server = CodeExplorerServer::from_parts(agent, lsp.clone()).await;
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;

    // Wait for service to end OR shutdown signal
    let lsp_shutdown = lsp.clone();
    tokio::select! {
        result = service.waiting() => {
            result.map_err(|e| anyhow::anyhow!("MCP server exited: {}", e))?;
        }
        _ = shutdown_signal() => {
            tracing::info!("Received shutdown signal");
        }
    }

    // Gracefully shut down all LSP servers
    tracing::info!("Shutting down LSP servers...");
    lsp_shutdown.shutdown_all().await;
    tracing::info!("All LSP servers shut down");
    Ok(())
}
```

Note: `lsp` must be changed from `let lsp = Arc::new(...)` to allow cloning. It already is `Arc<LspManager>` so `.clone()` works.

**Step 2: Add the `shutdown_signal` helper function**

Add this function above `run()` (around line 224):

```rust
/// Wait for SIGINT (Ctrl-C) or SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}
```

**Step 3: Build and verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles successfully

**Step 4: Run existing tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all existing tests pass

**Step 5: Commit**

```bash
git add src/server.rs
git commit -m "fix: graceful LSP shutdown on exit for stdio transport"
```

---

### Task 3: Add graceful shutdown to the HTTP transport path

**Files:**
- Modify: `src/server.rs:252-310` (run function, http branch)

**Step 1: Add signal handling to HTTP path**

Replace the HTTP connection accept loop (lines 290-310) with:

```rust
let mut sse_server = rmcp::transport::sse_server::SseServer::serve(addr)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to start SSE server: {}", e))?;

let lsp_shutdown = lsp.clone();

// Accept connections until shutdown signal
loop {
    tokio::select! {
        transport = sse_server.next_transport() => {
            match transport {
                Some(transport) => {
                    let agent = agent.clone();
                    let lsp = lsp.clone();
                    tokio::spawn(async move {
                        let handler = CodeExplorerServer::from_parts(agent, lsp).await;
                        match handler.serve(transport).await {
                            Ok(service) => {
                                if let Err(e) = service.waiting().await {
                                    tracing::debug!("SSE session ended: {}", e);
                                }
                            }
                            Err(e) => tracing::warn!("SSE session failed to start: {}", e),
                        }
                    });
                }
                None => break,
            }
        }
        _ = shutdown_signal() => {
            tracing::info!("Received shutdown signal");
            break;
        }
    }
}

// Gracefully shut down all LSP servers
tracing::info!("Shutting down LSP servers...");
lsp_shutdown.shutdown_all().await;
tracing::info!("All LSP servers shut down");
Ok(())
```

**Step 2: Build and verify**

Run: `cargo build 2>&1 | tail -5`
Expected: compiles successfully

**Step 3: Run existing tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all existing tests pass

**Step 4: Commit**

```bash
git add src/server.rs
git commit -m "fix: graceful LSP shutdown on exit for HTTP transport"
```

---

### Task 4: Write tests for the shutdown behavior

**Files:**
- Modify: `src/lsp/client.rs` (tests module)
- Modify: `src/lsp/manager.rs` (tests module)

**Step 1: Add test that LspClient::drop kills child process**

Add to the tests module in `src/lsp/client.rs`:

```rust
#[tokio::test]
async fn drop_kills_child_process() {
    if !rust_analyzer_available() {
        return;
    }
    let dir = create_test_cargo_project();
    let config = LspServerConfig {
        command: "rust-analyzer".into(),
        args: vec![],
        workspace_root: dir.path().to_path_buf(),
    };
    let client = LspClient::start(config).await.unwrap();
    let pid = client.child_pid.unwrap();

    // Verify child is alive
    let alive = unsafe { libc::kill(pid as i32, 0) };
    assert_eq!(alive, 0, "child should be alive before drop");

    // Drop the client
    drop(client);

    // Give the process a moment to die
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify child is dead
    let dead = unsafe { libc::kill(pid as i32, 0) };
    assert_ne!(dead, 0, "child should be dead after drop");
}
```

**Step 2: Add test that shutdown_all gracefully stops all servers**

Add to the tests module in `src/lsp/manager.rs`:

```rust
#[tokio::test]
async fn shutdown_all_stops_running_servers() {
    use std::process::Command as StdCommand;

    // Check if rust-analyzer is available
    if StdCommand::new("rust-analyzer").arg("--version").output().is_err() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    // Create minimal Cargo project
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}").unwrap();

    let mgr = LspManager::new();
    let client = mgr
        .get_or_start("rust", dir.path())
        .await
        .unwrap();
    assert!(client.is_alive());

    mgr.shutdown_all().await;

    // After shutdown, the client should be dead
    assert!(!client.is_alive());
    assert!(mgr.active_languages().await.is_empty());
}
```

**Step 3: Run the new tests**

Run: `cargo test drop_kills_child_process -- --nocapture 2>&1 | tail -20`
Run: `cargo test shutdown_all_stops_running -- --nocapture 2>&1 | tail -20`
Expected: both pass

**Step 4: Run full test suite**

Run: `cargo test 2>&1 | tail -5`
Expected: all tests pass

**Step 5: Commit**

```bash
git add src/lsp/client.rs src/lsp/manager.rs
git commit -m "test: add tests for LSP shutdown and kill-on-drop"
```

---

### Task 5: Final verification

**Step 1: Run clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -10`
Expected: no warnings

**Step 2: Run fmt**

Run: `cargo fmt`

**Step 3: Run full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: all tests pass

**Step 4: Build release**

Run: `cargo build --release 2>&1 | tail -5`
Expected: successful build

**Step 5: Manual smoke test**

Start the server, verify it exits cleanly:
```bash
timeout 3 ./target/release/code-explorer start --project . 2>&1 || true
# Check no orphaned rust-analyzer processes
ps aux | grep rust-analyzer | grep -v grep
```

**Step 6: Final commit if any fmt changes**

```bash
git add -A && git diff --cached --quiet || git commit -m "chore: fmt"
```
