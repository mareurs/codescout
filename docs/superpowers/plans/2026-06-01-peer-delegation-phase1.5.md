# Peer Delegation — Phase 1.5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make peer delegation usable with zero manual launch — the `peer` tool auto-spawns `peer-serve` on demand — and engage the Agent write-guard as a second layer behind the deny-by-default allow-list.

**Architecture:** Mirror the LSP mux's battle-tested spawn-on-demand dance (`LspManager::get_or_start_via_mux`): a `peer`-tool helper acquires the per-workspace flock to decide whether a serve is running; if not, it spawns a detached `codescout peer-serve` child, waits for a `ready\n` line on stdout, then connects with retries. `peer-serve` itself gains the three lifecycle prerequisites it lacks today (single-instance flock, `ready` handshake, idle-timeout self-cleanup). Separately, `build_server_for` flips the served workspace's `ActiveProject.read_only` flag so the write-guard fires on every served call.

**Tech Stack:** Rust, tokio, `fs4` (advisory flock, already a dep), Unix domain sockets, `lsp::transport` Content-Length framing.

---

## Standing constraint — existing-file edits are approval-gated

The user greenlit all `src/` edits for this plan when they chose "Auto-spawn + RO convergence" as the Phase 1.5 scope. No further per-file approval is needed for the files named below. If a task needs to touch a file **not** listed here, stop and ask first.

## Reconnaissance findings (scouted 2026-06-01, this session)

Verified against live code before writing this plan:

1. **`peer::server::run` lacks all three auto-spawn prerequisites** (`src/peer/server.rs:88`):
   - No `ready\n` print → a spawner waiting on stdout would block the full 120 s timeout.
   - No flock → two concurrent `peer(query)` calls would both spawn.
   - `_idle_timeout_secs` is ignored → the process loops `accept_one` forever.
2. **The mux template** is `LspManager::get_or_start_via_mux` (`src/lsp/manager.rs:432`): flock-check → `current_exe()` detached spawn → wait for `ready\n` (120 s) → connect-with-retry (5 × 200 ms). `process::run` writes `b"ready\n"` + flush at `src/lsp/mux/process.rs:228`. `build_mux_args` (`manager.rs:150`) is factored out for unit-testability — mirror that.
3. **The CLI surface already exists**: `Commands::PeerServe` (`src/main.rs:114`) has `--socket` (defaults to derived), `--read-only` (`default_value_t = true`), `--idle-timeout` (`default_value_t = 300`, doc says "reserved; not yet enforced"). No new CLI args needed.
4. **Socket/lock helpers already exist**: `peer_socket_path_for_workspace` and `peer_lock_path_for_workspace` in `src/socket_discovery.rs`.
5. **RO-convergence fits naturally via `with_project_at_mut`, NOT the spec's neutral-home recipe.** `build_workspace` makes `is_home ⇒ read_only=false` (`src/agent/mod.rs:155-164`) and `is_home` defaults to `true` when `home_root` is `None`. The spec's "neutral home + `ensure_resident(ro)` + per-call pin" needs a fake home dir to dodge that invariant — a forced fit. Instead, `Agent::with_project_at_mut(None, |p| { p.read_only = true; Ok(()) })` (`src/agent/mod.rs:711`) sets the default workspace's flag directly. Peer dispatch is unpinned → resolves to the default workspace → the guard covers 100 % of served calls. `project_security_config` (`agent/mod.rs:358`) turns `p.read_only` into `file_write_enabled = false`.

## File structure

- **Modify** `src/peer/server.rs` — `run` (flock + ready + idle-timeout), `build_server_for` (RO-convergence).
- **Create** `src/peer/launch.rs` — `build_peer_serve_args` + `ensure_peer_serve` (the spawn-on-demand client helper).
- **Modify** `src/peer/mod.rs` — `pub mod launch;` + re-export.
- **Modify** `src/tools/peer.rs` — `query`/`knowledge` arms call `ensure_peer_serve` instead of `PeerClient::connect`.
- **Modify** `docs/superpowers/specs/2026-06-01-peer-delegation-protocol-design.md` — §6 + §12 reconciled to the as-built RO-convergence and lifecycle.

---

### Task 1: Harden `peer::server::run` for auto-spawn (flock + ready + idle-timeout)

**Files:**
- Modify: `src/peer/server.rs:88` (`run`)
- Test: `src/peer/server.rs` `tests` module

`run` today (for reference):

```rust
pub async fn run(
    socket_path: &Path,
    workspace: &Path,
    read_only: bool,
    _idle_timeout_secs: u64,
) -> Result<()> {
    let ctx = build_server_for(workspace, read_only).await?;
    let listener = bind_peer_socket(socket_path)?;
    loop {
        if accept_one(&listener, &ctx).await.is_err() {
            break;
        }
    }
    std::fs::remove_file(socket_path).ok();
    Ok(())
}
```

- [ ] **Step 1: Write the failing idle-timeout test**

Add to the `tests` module in `src/peer/server.rs`:

```rust
#[tokio::test]
async fn run_exits_after_idle_timeout_with_no_connections() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    let sock = root.join("peer.sock");
    let lock = root.join("peer.lock");

    // idle_timeout = 1s, no client ever connects → run() must return promptly.
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        run_with_lock(&sock, &lock, &root, true, 1),
    )
    .await;
    assert!(res.is_ok(), "run() did not exit within 10s of a 1s idle timeout");
    assert!(res.unwrap().is_ok(), "run() returned an error");
    assert!(!sock.exists(), "socket file should be cleaned up on exit");
}
```

Note: this test calls a new `run_with_lock(socket, lock, workspace, read_only, idle)` that takes an explicit lock path (so the test controls it). `run` becomes a thin wrapper that derives the lock path. See Step 3.

- [ ] **Step 2: Run it to confirm it fails to compile**

Run: `cargo test --lib peer::server::tests::run_exits_after_idle_timeout -- --nocapture`
Expected: FAIL — `run_with_lock` not found.

- [ ] **Step 3: Implement the hardened run**

Replace `run` with the following (keep the same public signature; add `run_with_lock` as the testable inner):

```rust
/// Run the peer-serve process for `workspace`: acquire the single-instance
/// flock, bind the socket, signal `ready`, and serve connections until the
/// idle timeout elapses with no connection. Mirrors `lsp::mux::process::run`.
pub async fn run(
    socket_path: &Path,
    workspace: &Path,
    read_only: bool,
    idle_timeout_secs: u64,
) -> Result<()> {
    let lock_path = crate::socket_discovery::peer_lock_path_for_workspace(workspace);
    run_with_lock(socket_path, &lock_path, workspace, read_only, idle_timeout_secs).await
}

/// Inner form taking an explicit lock path, for tests that control the lock.
async fn run_with_lock(
    socket_path: &Path,
    lock_path: &Path,
    workspace: &Path,
    read_only: bool,
    idle_timeout_secs: u64,
) -> Result<()> {
    use fs4::fs_std::FileExt;

    // 1. Single-instance flock. If another peer-serve already owns this
    //    workspace, exit 0 quietly — the spawner will connect to the winner.
    let lock_file = {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(lock_path)
            .with_context(|| format!("failed to open peer lock file: {}", lock_path.display()))?
    };
    if lock_file.try_lock_exclusive().is_err() {
        tracing::info!(
            "peer-serve already running for {}, exiting",
            workspace.display()
        );
        return Ok(());
    }
    // lock_file held for the process lifetime; released on drop.

    let ctx = build_server_for(workspace, read_only).await?;
    let listener = bind_peer_socket(socket_path)?;

    // 2. Signal ready to the spawner (mirrors mux process::run), then serve.
    {
        use tokio::io::AsyncWriteExt;
        let mut stdout = tokio::io::stdout();
        stdout.write_all(b"ready\n").await.ok();
        stdout.flush().await.ok();
    }

    // 3. Sequential accept loop with idle-timeout. The timeout wraps ONLY the
    //    accept (the genuine idle wait) — an accepted connection is served
    //    without a deadline so a long query is never cancelled mid-flight.
    let idle = std::time::Duration::from_secs(idle_timeout_secs);
    loop {
        match tokio::time::timeout(idle, listener.accept()).await {
            Ok(Ok((stream, _addr))) => {
                if serve_connection(stream, &ctx).await.is_err() {
                    break;
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("peer-serve accept error: {e}");
                break;
            }
            Err(_elapsed) => {
                tracing::info!(
                    "peer-serve idle timeout reached ({idle_timeout_secs}s), shutting down"
                );
                break;
            }
        }
    }
    std::fs::remove_file(socket_path).ok();
    Ok(())
}
```

Ensure `use anyhow::Context;` is in scope in the function (the file already imports `anyhow::{anyhow, Context, Result}` at the top — confirm; if `Context` is missing, add it to the existing `use` rather than a new line).

- [ ] **Step 4: Add the flock-contention test**

```rust
#[tokio::test]
async fn run_exits_quietly_when_lock_is_held() {
    use fs4::fs_std::FileExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    let sock = root.join("peer.sock");
    let lock = root.join("peer.lock");

    // Hold the lock from the test, simulating an already-running peer-serve.
    let held = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&lock)
        .unwrap();
    held.try_lock_exclusive().unwrap();

    // run() must return Ok promptly without binding the socket.
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        run_with_lock(&sock, &lock, &root, true, 30),
    )
    .await;
    assert!(res.is_ok(), "run() blocked despite the lock being held");
    assert!(res.unwrap().is_ok());
    assert!(!sock.exists(), "run() must not bind the socket when the lock is held");
}
```

- [ ] **Step 5: Run both tests to verify they pass**

Run: `cargo test --lib peer::server::tests::run_ -- --nocapture`
Expected: PASS (both `run_exits_after_idle_timeout_with_no_connections` and `run_exits_quietly_when_lock_is_held`).

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
git add src/peer/server.rs
git commit -m "feat(peer): harden peer-serve run with flock, ready handshake, idle-timeout

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: RO-convergence — flip the served workspace read-only in `build_server_for`

**Files:**
- Modify: `src/peer/server.rs:50` (`build_server_for`)
- Test: `src/peer/server.rs` `tests` module

`build_server_for` today:

```rust
pub async fn build_server_for(root: &Path, read_only: bool) -> Result<PeerServe> {
    let agent = Agent::new(Some(root.to_path_buf()))
        .await
        .context("failed to construct agent for peer workspace")?;
    let server = Arc::new(CodeScoutServer::new(agent).await);
    let audit_path = Some(root.join(".codescout").join("peer-audit.jsonl"));
    Ok(PeerServe { server, read_only, audit_path })
}
```

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn build_server_for_read_only_disables_writes_on_default_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();

    let ctx = build_server_for(&root, true).await.unwrap();
    // The served (default) workspace must report writes disabled, proving the
    // Agent write-guard engages behind the allow-list.
    let sec = ctx.server.agent_security_config().await;
    assert!(
        !sec.file_write_enabled,
        "read-only peer-serve must disable file writes on the served workspace"
    );
}
```

This needs a thin accessor on `CodeScoutServer` to reach the agent's security config. If `CodeScoutServer` already exposes the agent (e.g. a `pub(crate) agent` field or accessor — check `src/server.rs`), call `ctx.server.<agent>.security_config().await` directly and drop the `agent_security_config` wrapper. **Scout `src/server.rs` for the existing accessor before adding a new one.** If none exists, add:

```rust
// in src/server.rs, impl CodeScoutServer
#[cfg(test)]
pub(crate) async fn agent_security_config(&self) -> crate::util::path_security::PathSecurityConfig {
    self.agent.security_config().await
}
```

Adjust the field/accessor name to match what `src/server.rs` actually exposes (the peer server already calls `ctx.server.project_name()`, `project_root_string()`, `tool_names()` — follow that pattern).

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test --lib peer::server::tests::build_server_for_read_only -- --nocapture`
Expected: FAIL — assertion fails (writes still enabled) because home is read-write by default.

- [ ] **Step 3: Implement the RO-convergence**

Replace `build_server_for` with:

```rust
/// Construct a `CodeScoutServer` for `root` and wrap it with the read-only grant.
///
/// When `read_only`, flip the served (default) workspace's `read_only` flag so the
/// Agent write-guard engages as a second layer behind the `PEER_EXPOSED_TOOLS`
/// allow-list. `Agent::new` makes `root` the home, and home is read-write by
/// default (`build_workspace`'s `is_home ⇒ rw` invariant); peer-serve is a pure
/// reader, so we override that here. Peer dispatch is unpinned and resolves to
/// the default workspace, so this covers every served call.
pub async fn build_server_for(root: &Path, read_only: bool) -> Result<PeerServe> {
    let agent = Agent::new(Some(root.to_path_buf()))
        .await
        .context("failed to construct agent for peer workspace")?;
    if read_only {
        agent
            .with_project_at_mut(None, |p| {
                p.read_only = true;
                Ok(())
            })
            .await
            .context("failed to mark peer workspace read-only")?;
    }
    let server = Arc::new(CodeScoutServer::new(agent).await);
    let audit_path = Some(root.join(".codescout").join("peer-audit.jsonl"));
    Ok(PeerServe { server, read_only, audit_path })
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib peer::server::tests::build_server_for_read_only -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the whole peer module to confirm no regressions**

Run: `cargo test --lib peer:: -- --nocapture`
Expected: PASS (all existing peer tests + the two new ones from Task 1 + this one).

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
git add src/peer/server.rs src/server.rs
git commit -m "feat(peer): engage Agent write-guard on read-only peer-serve

RO-convergence (spec section 12) via with_project_at_mut, not the spec's
neutral-home recipe — see plan reconnaissance finding 5.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: `src/peer/launch.rs` — `build_peer_serve_args` + `ensure_peer_serve`

**Files:**
- Create: `src/peer/launch.rs`
- Modify: `src/peer/mod.rs` (add `pub mod launch;`)
- Test: `src/peer/launch.rs` `tests` module

This is the requester-side spawn-on-demand helper, mirroring `LspManager::get_or_start_via_mux`. Phase 1 always serves read-only, so the spawner does **not** pass `--read-only` (relies on the CLI default `true`) — this sidesteps clap's bool-with-default-value form entirely. The `read_only` parameter is threaded for forward-compat and logged, not yet passed as a flag (documented below).

- [ ] **Step 1: Write the failing `build_peer_serve_args` test**

Create `src/peer/launch.rs` with just the test first:

```rust
//! Requester-side spawn-on-demand for `peer-serve`. Mirrors
//! `lsp::manager::get_or_start_via_mux`: flock-check → spawn detached
//! `codescout peer-serve` → wait for `ready` → connect with retries.
#![cfg(unix)]

use crate::peer::client::PeerClient;
use anyhow::{Context, Result};
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_peer_serve_args_derives_socket_and_workspace() {
        let args = build_peer_serve_args(
            &PathBuf::from("/run/user/1000/codescout-peer-abc.sock"),
            &PathBuf::from("/home/u/proj"),
            300,
        );
        // subcommand first
        assert_eq!(args[0], "peer-serve");
        // socket + workspace + idle-timeout present as flag/value pairs
        let socket_idx = args.iter().position(|a| a == "--socket").unwrap();
        assert_eq!(args[socket_idx + 1], "/run/user/1000/codescout-peer-abc.sock");
        let ws_idx = args.iter().position(|a| a == "--workspace").unwrap();
        assert_eq!(args[ws_idx + 1], "/home/u/proj");
        let idle_idx = args.iter().position(|a| a == "--idle-timeout").unwrap();
        assert_eq!(args[idle_idx + 1], "300");
        // Phase 1: --read-only is NOT passed (CLI default is true).
        assert!(!args.iter().any(|a| a == "--read-only"));
    }
}
```

- [ ] **Step 2: Run it to confirm it fails to compile**

Run: `cargo test --lib peer::launch::tests::build_peer_serve_args -- --nocapture`
Expected: FAIL — `build_peer_serve_args` not found.

- [ ] **Step 3: Implement `build_peer_serve_args` and `ensure_peer_serve`**

Add above the `tests` module in `src/peer/launch.rs`:

```rust
/// Idle timeout (seconds) for auto-spawned peer-serve processes. Matches the
/// LSP mux default; an auto-spawned serve reaps itself after this much idle.
pub const PEER_IDLE_TIMEOUT_SECS: u64 = 300;

/// Build the CLI argv for a spawned `codescout peer-serve` child. Factored out
/// for unit-testability (mirrors `lsp::manager::build_mux_args`). Phase 1 always
/// serves read-only, so `--read-only` is omitted (the CLI default is `true`).
pub(crate) fn build_peer_serve_args(
    socket_path: &Path,
    workspace: &Path,
    idle_timeout_secs: u64,
) -> Vec<String> {
    vec![
        "peer-serve".to_string(),
        "--socket".to_string(),
        socket_path.to_string_lossy().to_string(),
        "--workspace".to_string(),
        workspace.to_string_lossy().to_string(),
        "--idle-timeout".to_string(),
        idle_timeout_secs.to_string(),
    ]
}

/// Connect to the peer-serve owning `target`, spawning it on demand if not
/// running. Mirrors `LspManager::get_or_start_via_mux`:
///
/// 1. Derive the per-workspace socket + lock paths.
/// 2. Acquire the flock to decide whether a serve is already running.
/// 3. If we got the lock (none running), drop it and spawn a detached
///    `codescout peer-serve` child; wait for its `ready\n` line (120 s).
/// 4. Connect as a client with retries (5 × 200 ms).
///
/// `read_only` is reserved for Phase 1.5+ RW peers; Phase 1 always spawns
/// read-only, so it is logged but not yet passed as a CLI flag.
pub async fn ensure_peer_serve(target: &Path, read_only: bool) -> Result<PeerClient> {
    use fs4::fs_std::FileExt;

    let socket_path = crate::socket_discovery::peer_socket_path_for_workspace(target);
    let lock_path = crate::socket_discovery::peer_lock_path_for_workspace(target);

    let need_spawn = {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(false);
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let lock_file = opts
            .open(&lock_path)
            .with_context(|| format!("failed to open peer lock file: {}", lock_path.display()))?;
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // No serve running — drop the lock so the child can acquire it.
                drop(lock_file);
                true
            }
            Err(_) => {
                tracing::info!("peer-serve already running for {}", target.display());
                false
            }
        }
    };

    if need_spawn {
        tracing::info!(
            "spawning peer-serve for {} (read_only={read_only})",
            target.display()
        );
        let exe = std::env::current_exe().context("failed to determine codescout binary path")?;
        let args = build_peer_serve_args(&socket_path, target, PEER_IDLE_TIMEOUT_SECS);

        let mut child = tokio::process::Command::new(&exe)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to spawn peer-serve process")?;

        // Wait for the `ready` line on stdout (binds socket + acquires lock first).
        let stdout = child.stdout.take().expect("stdout piped");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        match tokio::time::timeout(
            std::time::Duration::from_secs(120),
            tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line),
        )
        .await
        {
            Ok(Ok(_)) if line.trim().starts_with("ready") => {
                tracing::info!("peer-serve ready for {}", target.display());
            }
            // Empty line / non-ready / EOF: another instance may have won the
            // lock race and this child exited. Fall through to connect-retry,
            // which will reach whichever instance is actually serving.
            Ok(Ok(_)) => {
                tracing::warn!(
                    "peer-serve produced no ready line for {} ({:?}); trying to connect anyway",
                    target.display(),
                    line.trim()
                );
            }
            Ok(Err(e)) => {
                tracing::warn!("peer-serve stdout error for {}: {e}", target.display());
            }
            Err(_) => {
                return Err(crate::tools::RecoverableError::with_hint(
                    format!("peer-serve timed out waiting for ready (120s) for {}", target.display()),
                    "The peer workspace may be slow to index. Retry in a moment, or check \
                     for a stale lock file in the per-user runtime dir.",
                )
                .into());
            }
        }
    }

    // Connect with retries.
    let mut last_err = None;
    for attempt in 0..5u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        match PeerClient::connect(&socket_path).await {
            Ok(client) => return Ok(client),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        anyhow::anyhow!("failed to connect to peer-serve for {}", target.display())
    }))
}
```

- [ ] **Step 4: Register the module**

In `src/peer/mod.rs`, add alongside the other `pub mod` lines:

```rust
#[cfg(unix)]
pub mod launch;
```

(Match the existing module-declaration style in that file; `launch.rs` is `#![cfg(unix)]` at its top, so the `#[cfg(unix)]` on the decl keeps non-unix builds clean.)

- [ ] **Step 5: Run the unit test + build**

Run: `cargo test --lib peer::launch::tests -- --nocapture`
Expected: PASS.
Run: `cargo build --lib`
Expected: clean.

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
git add src/peer/launch.rs src/peer/mod.rs
git commit -m "feat(peer): ensure_peer_serve — spawn peer-serve on demand

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Wire the `peer` tool to auto-spawn

**Files:**
- Modify: `src/tools/peer.rs:16` (`impl Tool for PeerTool`, the `query`/`explore` and `knowledge` arms)
- Test: `src/tools/peer.rs` `tests` module

Today both arms call `PeerClient::connect(&entry.socket_path()).await?` directly, which fails if no serve is running. Replace those with `ensure_peer_serve`.

- [ ] **Step 1: Read the current arms**

The `query`/`explore` arm currently does:

```rust
let mut client = PeerClient::connect(&entry.socket_path()).await?;
let _caps = client.hello().await?;
client.call_tool(tool, tool_args).await
```

The `knowledge` arm:

```rust
let mut client = PeerClient::connect(&entry.socket_path()).await?;
client.read_buffer(handle).await
```

- [ ] **Step 2: Replace the connect calls with `ensure_peer_serve`**

In the `query`/`explore` arm, replace the `PeerClient::connect(...)` line with:

```rust
let mut client =
    crate::peer::launch::ensure_peer_serve(&entry.target, entry.default_access.is_read_only())
        .await?;
let _caps = client.hello().await?;
client.call_tool(tool, tool_args).await
```

In the `knowledge` arm, replace the `PeerClient::connect(...)` line with:

```rust
let mut client =
    crate::peer::launch::ensure_peer_serve(&entry.target, entry.default_access.is_read_only())
        .await?;
client.read_buffer(handle).await
```

`PeerEntry` exposes `target: PathBuf` and `default_access` (with `is_read_only()`) — confirmed in `src/peer/registry.rs`. If the `PeerClient` import in `src/tools/peer.rs` becomes unused after this change, remove it (clippy will flag it).

Note: `ensure_peer_serve` is `#[cfg(unix)]`. `src/tools/peer.rs` is compiled on all platforms. If the crate builds on non-unix, guard these call sites or the whole peer tool with `#[cfg(unix)]` consistent with how the tool is registered in `src/server.rs`. **Scout how `PeerTool` is registered** (the registration may already be `#[cfg(unix)]`); match it. If the tool is registered unconditionally, wrap just the `ensure_peer_serve` calls in `#[cfg(unix)]` with a non-unix `bail!` fallback.

- [ ] **Step 3: Build and run the peer tool tests**

Run: `cargo test --lib tools::peer:: -- --nocapture`
Expected: PASS (existing `status`-action tests don't touch the socket, so they remain green).
Run: `cargo build --lib`
Expected: clean.

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/peer.rs
git commit -m "feat(peer): peer tool auto-spawns peer-serve on demand

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: End-to-end auto-spawn test + spec reconciliation + manual verification

**Files:**
- Test: `src/peer/launch.rs` `tests` module (a gated integration test) OR `tests/` if it must spawn the real binary
- Modify: `docs/superpowers/specs/2026-06-01-peer-delegation-protocol-design.md` (§6, §12)

- [ ] **Step 1: Write an integration test for the in-process auto-spawn dance**

A true end-to-end test would spawn the compiled `codescout` binary, which `cargo test` cannot reach reliably (the test runner is `current_exe()`, not codescout — the same reason `get_or_start_via_mux` falls back to direct mode in tests, see `manager.rs:314`). So test the **dance** without a real child: spawn `run_with_lock` as a tokio task (the "serve" side), then call `PeerClient::connect` + `hello` + a `tree` call (the "requester" side), asserting the exposed read tool succeeds and a write is denied.

```rust
// in src/peer/server.rs tests (run_with_lock is module-private there)
#[tokio::test]
async fn end_to_end_served_read_tool_and_write_denied() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    let sock = root.join("peer.sock");
    let lock = root.join("peer.lock");

    let (sr, sk, lk) = (root.clone(), sock.clone(), lock.clone());
    let serve = tokio::spawn(async move {
        // long idle so the serve stays up for the duration of the test
        let _ = run_with_lock(&sk, &lk, &sr, true, 30).await;
    });

    // Wait for the socket to appear (the serve binds before it would print ready).
    let client = {
        let mut c = None;
        for _ in 0..50 {
            if let Ok(client) = crate::peer::client::PeerClient::connect(&sock).await {
                c = Some(client);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        c.expect("could not connect to served socket")
    };
    let mut client = client;
    let _caps = client.hello().await.unwrap();

    // Exposed read tool succeeds.
    let ok = client
        .call_tool("tree", serde_json::json!({ "path": "." }))
        .await;
    assert!(ok.is_ok(), "exposed read tool should succeed: {ok:?}");

    // Non-exposed write tool is denied by the allow-list (RecoverableError).
    let denied = client
        .call_tool("create_file", serde_json::json!({ "path": "x.txt", "content": "no" }))
        .await;
    assert!(denied.is_err(), "create_file must be rejected");
    assert!(
        !root.join("x.txt").exists(),
        "denied write must have no side effect"
    );

    serve.abort();
}
```

- [ ] **Step 2: Run the e2e test**

Run: `cargo test --lib peer::server::tests::end_to_end_served -- --nocapture`
Expected: PASS.

- [ ] **Step 3: Reconcile the spec**

Update `docs/superpowers/specs/2026-06-01-peer-delegation-protocol-design.md`:
- §12 "Workspace-pinning RO convergence (Phase 1.5)": change from "deferred/open" to **done**, and replace the neutral-home recipe with the as-built note: *"Implemented in `build_server_for` via `Agent::with_project_at_mut(None, |p| p.read_only = true)` — the served workspace (= home) is flipped read-only post-construction. The spec's earlier neutral-home + per-call-pin recipe was rejected as a forced fit against the `is_home ⇒ rw` invariant (see Phase 1.5 plan reconnaissance finding 5)."*
- §6 "Lifecycle": note that the lock file, `ready` handshake, and 300 s idle-timeout are now implemented in `peer::server::run`, and auto-spawn lives in `peer::launch::ensure_peer_serve`.

Use `edit_markdown` (heading-addressed) for both edits.

- [ ] **Step 4: Full suite + clippy**

```bash
cargo test --lib
cargo clippy --lib -- -D warnings
```
Expected: all green, no warnings.

- [ ] **Step 5: Manual MCP verification (per CLAUDE.md)**

```bash
cargo build --release
ln -sf "$(pwd)/target/release/codescout" ~/.cargo/bin/codescout
```
Then restart the MCP server (`/mcp`) and, with NO peer-serve pre-launched, run:
- `peer(action="query", peer="codescout-main", tool="symbols", args={"path":"src/peer/launch.rs"})` — must auto-spawn peer-serve and return symbols.
- Confirm a peer-serve process appeared (`pgrep -x codescout` + per-PID `/proc/<pid>/cmdline` grep for `peer-serve` — NOT `pkill -f`, which self-matches).
- Confirm a write through the peer is still denied with no side effect.

- [ ] **Step 6: Commit the spec reconciliation**

```bash
git add docs/superpowers/specs/2026-06-01-peer-delegation-protocol-design.md
git commit -m "docs(peer): reconcile spec to as-built Phase 1.5 (auto-spawn + RO convergence)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Manual end-to-end verification (after Task 5)

Covered by Task 5 Step 5. Do not cherry-pick to master until the live MCP auto-spawn check passes (frog summon gate per CLAUDE.md applies before any master merge).

## Self-review

- **Spec coverage:** auto-spawn (Tasks 1, 3, 4), idle-timeout self-cleanup (Task 1), single-instance flock (Task 1), `ready` handshake (Task 1), RO-convergence (Task 2). All Phase 1.5 in-scope items have a task.
- **Type consistency:** `run_with_lock` signature is stable across Tasks 1 + 5; `build_peer_serve_args(socket, workspace, idle)` arity matches its test and `ensure_peer_serve` caller; `entry.target` / `entry.default_access.is_read_only()` match `registry.rs`.
- **No placeholders:** every code step has complete code. Two scout-then-adapt points are explicit (the `CodeScoutServer` agent accessor in Task 2; the `PeerTool` cfg-gating in Task 4) — these are "confirm the existing shape, match it," not deferred work.
- **Out of scope (deferred, noted for the user):** per-call request-id tagging (observability), the librarian-CWD cosmetic.
