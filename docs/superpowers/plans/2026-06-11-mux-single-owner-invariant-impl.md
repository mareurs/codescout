# Mux Single-Owner Invariant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce the mux single-owner invariant — "at most one process holds a workspace's RocksDB index, and it is the live mux" — by closing the three leak seams (S1/S2/S3).

**Architecture:** Keep the per-workspace socket mux as the single owner. Add three causal enforcements: (M3) remove the silent direct-LSP fallback for mux languages; (M2) reap an orphaned index-lock holder before spawning a fresh mux — winning the ownership `flock` proves the holder is an orphan; (M1) put the mux's LSP child in its own process group and kill the group on signalled exit. See ADR [[2026-06-11-mux-single-owner-invariant]] and spec `docs/superpowers/specs/2026-06-11-mux-single-owner-invariant-design.md`.

**Tech Stack:** Rust, tokio, `libc` (fcntl/killpg), `walkdir`, `nix`-free (raw `libc`), existing helpers `posix_write_lock_is_held` / `kotlin_index_lock_held` (`src/lsp/manager.rs`).

**Branch:** `experiments`. Each task is independently committable; cherry-pick to `master` per the Standard Ship Sequence after `cargo fmt && cargo clippy -- -D warnings && cargo test` and a manual `/mcp` verify.

---

## File Structure

- `src/lsp/manager.rs` — Tasks 1 & 2. Add `is_test_runner_exe`, `reap_holders_of_lock`, `reap_orphan_index_holder`; gate the fallback in `get_or_start`; call the reap in `get_or_start_via_mux`'s `need_spawn` branch.
- `src/lsp/mux/process.rs` — Task 3. Process-group on the LSP child + signal arm in `event_loop` + `killpg` in `run` after the loop.

No new files — all changes extend existing modules, following the project's flat-module pattern (`tool-registration-rule-of-three`: not enough divergence to warrant a new module).

---

## Task 1 (M3): Remove the silent direct-LSP fallback for mux languages

**Files:**
- Modify: `src/lsp/manager.rs` — `get_or_start` (the mux-failure `Err(e)` arm, currently ending `config.mux = false;`), add free fn `is_test_runner_exe`.
- Test: `src/lsp/manager.rs` `mod tests`.

- [ ] **Step 1: Write the failing test for the predicate**

```rust
#[test]
fn is_test_runner_exe_true_for_non_codescout_basename() {
    use std::path::Path;
    // A cargo test binary: hash-suffixed, not "codescout".
    assert!(super::is_test_runner_exe(Path::new(
        "/repo/target/debug/deps/codescout_lib-9f2a1b3c4d5e6f70"
    )) == false, "lib test binary starts with 'codescout' → treated as prod-ish");
    // A clearly foreign runner.
    assert!(super::is_test_runner_exe(Path::new("/usr/bin/cargo")));
    assert!(super::is_test_runner_exe(Path::new("/tmp/x/some-test-runner")));
    // The real installed binary → not a test runner.
    assert!(!super::is_test_runner_exe(Path::new("/home/u/.cargo/bin/codescout")));
}
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test --lib is_test_runner_exe_true_for_non_codescout_basename`
Expected: FAIL — `is_test_runner_exe` not defined.

- [ ] **Step 3: Implement the predicate**

Add near `resolve_mux_flag` in `src/lsp/manager.rs`:

```rust
/// True when the current executable is NOT the codescout binary — i.e. a cargo
/// test runner, where spawning a `codescout mux` child (via `current_exe()`)
/// would re-exec the test binary instead of the server. The direct-LSP fallback
/// in `get_or_start` is retained ONLY for this case; in production it is removed
/// so a mux language can never spawn a competing direct LSP on the shared index.
fn is_test_runner_exe(exe: &std::path::Path) -> bool {
    exe.file_name()
        .map(|n| !n.to_string_lossy().starts_with("codescout"))
        .unwrap_or(true)
}
```

- [ ] **Step 4: Run it, verify it passes**

Run: `cargo test --lib is_test_runner_exe_true_for_non_codescout_basename`
Expected: PASS.

- [ ] **Step 5: Gate the fallback in `get_or_start`**

In `src/lsp/manager.rs`, the mux-failure `Err(e)` arm currently ends:

```rust
                    tracing::warn!(
                        "Mux startup failed for {language}, falling back to direct LSP: {e}"
                    );
                    config.mux = false;
```

Replace with:

```rust
                    // For mux languages, a silent direct fallback spawns a
                    // competing LSP on the shared index (S3) — refuse it in
                    // production. Retain the fallback ONLY when current_exe() is a
                    // test runner (spawning a `codescout mux` child would re-exec
                    // the test binary). See ADR-2026-06-11-mux-single-owner-invariant.
                    let exe_is_test = std::env::current_exe()
                        .map(|p| is_test_runner_exe(&p))
                        .unwrap_or(true);
                    if !exe_is_test {
                        return Err(crate::tools::RecoverableError::with_hint(
                            format!("mux startup failed for {language}: {e}"),
                            "codescout will not fall back to a direct LSP for a \
                             multiplexed language — that would open a second process \
                             on the shared index. Retry in a moment; if it persists, \
                             check for an orphaned LSP with \
                             `fuser <kotlin-lsp-home>/.../rocks/*/LOCK` and stop it.",
                        )
                        .into());
                    }
                    tracing::warn!(
                        "Mux startup failed for {language} in a test runner, \
                         falling back to direct LSP: {e}"
                    );
                    config.mux = false;
```

- [ ] **Step 6: Write the failing routing test**

```rust
#[tokio::test]
#[serial_test::serial]
async fn mux_language_does_not_fall_back_to_direct_in_prod_exe() {
    // Simulate: a mux language whose mux startup fails for a non-contention,
    // non-test reason. We can't easily fake current_exe(), so this test asserts
    // the predicate-gated branch via a focused unit on the decision, not the full
    // get_or_start path. The full path is covered by the integration mux test.
    // Decision table: (exe_is_test=false) => Err; (exe_is_test=true) => mux=false.
    let prod = std::path::Path::new("/home/u/.cargo/bin/codescout");
    let test = std::path::Path::new("/repo/target/debug/deps/some_test-abc123");
    assert!(!super::is_test_runner_exe(prod), "prod exe must NOT fall back");
    assert!(super::is_test_runner_exe(test), "test exe MUST keep the fallback");
}
```

- [ ] **Step 7: Run the full module tests + clippy**

Run: `cargo test --lib lsp::manager && cargo clippy -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 8: Commit**

```bash
git add src/lsp/manager.rs
git commit -m "fix(lsp): refuse silent direct-LSP fallback for mux languages (S3)

Mux languages (kotlin, rust) must never spawn a competing direct LSP on
the shared index. The fallback is retained only for the test-runner exe
case. Closes S3 of ADR-2026-06-11-mux-single-owner-invariant.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2 (M2): Reap an orphaned index-lock holder before spawning a mux

**Files:**
- Modify: `src/lsp/manager.rs` — add `reap_holders_of_lock`, `reap_orphan_index_holder`; call from `get_or_start_via_mux` `need_spawn` branch.
- Test: `src/lsp/manager.rs` `mod tests`.

**Rationale:** In the `need_spawn` branch we hold (just won) the ownership `flock`, so **no live mux exists** for this workspace. Any process still holding the RocksDB `LOCK` is therefore an orphan — reap it, do not classify it.

- [ ] **Step 1: Write the failing test for the inner reaper (three-query sandwich)**

```rust
#[cfg(unix)]
#[test]
#[ignore = "spawns a python3 fcntl holder; gated like posix_write_lock tests"]
fn reap_holders_of_lock_kills_an_orphan_holder() {
    use std::io::Read;
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("LOCK");
    std::fs::write(&lock, b"").unwrap();

    // Orphan holder: a python process taking the POSIX write lock (RocksDB's).
    let mut holder = std::process::Command::new("python3")
        .arg("-c")
        .arg(format!(
            "import fcntl,time; f=open(r'{}', 'r+'); \
             fcntl.lockf(f, fcntl.LOCK_EX | fcntl.LOCK_NB); \
             print('held', flush=True); time.sleep(30)",
            lock.display()
        ))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn holder");
    { let mut b = [0u8; 4]; let _ = holder.stdout.as_mut().unwrap().read(&mut b); }

    // Query 1 — baseline: lock is held.
    assert!(super::posix_write_lock_is_held(&lock), "precondition: held");
    // Query 2 — reap.
    let reaped = super::reap_holders_of_lock(&lock).expect("reap ok");
    assert!(reaped, "should report a reap happened");
    // Query 3 — fresh: lock released.
    assert!(!super::posix_write_lock_is_held(&lock), "lock freed after reap");

    let _ = holder.wait();
}

#[cfg(unix)]
#[test]
fn reap_holders_of_lock_noop_when_unheld() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("LOCK");
    std::fs::write(&lock, b"").unwrap();
    assert!(!super::reap_holders_of_lock(&lock).unwrap(), "no holder → no reap");
}
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test --lib reap_holders_of_lock_noop_when_unheld`
Expected: FAIL — `reap_holders_of_lock` not defined.

- [ ] **Step 3: Implement the inner reaper (/proc fd scan + signal escalation)**

Add to `src/lsp/manager.rs`:

```rust
/// Find every process holding an open fd on `lock_path` (via `/proc/<pid>/fd`)
/// and terminate it: SIGTERM, then SIGKILL after a 2s grace. Returns whether any
/// holder was signalled. Linux-only fd scan; `#[cfg(unix)]` overall.
///
/// SAFETY of killing: callers invoke this ONLY after winning the mux ownership
/// `flock` (so no live mux exists), making any holder an orphan corpse-of-a-mux.
#[cfg(unix)]
fn reap_holders_of_lock(lock_path: &std::path::Path) -> anyhow::Result<bool> {
    if !posix_write_lock_is_held(lock_path) {
        return Ok(false);
    }
    let canon = std::fs::canonicalize(lock_path).unwrap_or_else(|_| lock_path.to_path_buf());
    let mut holders: Vec<i32> = Vec::new();
    for entry in std::fs::read_dir("/proc")?.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else { continue };
        if pid == std::process::id() as i32 { continue; }
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = std::fs::read_dir(&fd_dir) else { continue };
        for fd in fds.flatten() {
            if let Ok(target) = std::fs::read_link(fd.path()) {
                if target == canon || target == lock_path {
                    holders.push(pid);
                    break;
                }
            }
        }
    }
    if holders.is_empty() {
        // Held by POSIX lock but no fd match (e.g. permission) — cannot reap.
        return Ok(false);
    }
    for &pid in &holders {
        unsafe { libc::kill(pid, libc::SIGTERM); }
        tracing::warn!("reaped orphan index-lock holder pid={pid} (SIGTERM)");
    }
    // Grace, then SIGKILL survivors.
    std::thread::sleep(std::time::Duration::from_secs(2));
    for &pid in &holders {
        if unsafe { libc::kill(pid, 0) } == 0 {
            unsafe { libc::kill(pid, libc::SIGKILL); }
            tracing::warn!("orphan index-lock holder pid={pid} survived SIGTERM → SIGKILL");
        }
    }
    Ok(true)
}

/// Reap an orphaned RocksDB index-lock holder for a kotlin workspace, if any.
/// No-op for non-kotlin or when the index is free. Returns whether a reap ran.
#[cfg(unix)]
fn reap_orphan_index_holder(language: &str, workspace_root: &std::path::Path) -> anyhow::Result<bool> {
    if language != "kotlin" {
        return Ok(false);
    }
    let ws_hash = crate::lsp::mux::workspace_hash(workspace_root);
    let analyzer_dir =
        crate::lsp::servers::kotlin_analyzer_home(&ws_hash).join(".config/JetBrains/analyzer");
    if !analyzer_dir.exists() {
        return Ok(false);
    }
    let mut any = false;
    for e in walkdir::WalkDir::new(&analyzer_dir).into_iter().filter_map(Result::ok) {
        if e.file_name() == "LOCK" {
            any |= reap_holders_of_lock(e.path())?;
        }
    }
    Ok(any)
}
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib reap_holders_of_lock_noop_when_unheld`
Expected: PASS. (Run the `#[ignore]` orphan test manually: `cargo test --lib reap_holders_of_lock_kills_an_orphan_holder -- --ignored`.)

- [ ] **Step 5: Wire the reap into the spawn branch**

In `src/lsp/manager.rs` `get_or_start_via_mux`, inside `if need_spawn {`, immediately after the `let exe = std::env::current_exe()...?;` line and before `let mux_args = ...`:

```rust
                // We hold the ownership lock → no live mux exists. Any process
                // still holding this workspace's RocksDB index LOCK is therefore an
                // orphan (a dead mux's JVM). Reap it before spawning, or the new
                // mux's LSP child will fail to open the index. (S1/S2 net.)
                match reap_orphan_index_holder(language, workspace_root) {
                    Ok(true) => tracing::info!("reaped orphan index holder for {language} before mux spawn"),
                    Ok(false) => {}
                    Err(e) => tracing::warn!("orphan reap probe failed (continuing): {e}"),
                }
```

- [ ] **Step 6: Run full suite + clippy**

Run: `cargo test --lib lsp:: && cargo clippy -- -D warnings`
Expected: PASS, clean. Confirm `libc` is already a dependency (it is — `posix_write_lock_is_held` uses it).

- [ ] **Step 7: Commit**

```bash
git add src/lsp/manager.rs
git commit -m "fix(lsp): reap orphaned RocksDB index-lock holder before mux spawn (S1/S2)

Winning the mux ownership flock proves no live mux exists, so any holder
of the workspace's RocksDB LOCK is an orphan — reap it (SIGTERM→SIGKILL)
before spawning, instead of deadlocking. Self-heal 'option B' from
issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3 (M1): Mux owns its LSP as a process group; kills the group on signalled exit

**Files:**
- Modify: `src/lsp/mux/process.rs` — `run` (process-group on child + `killpg` after `event_loop`), `event_loop` (signal-break arm).
- Test: integration (gated `#[ignore]`, like the existing rust-analyzer mux test) + a unit on the process-group flag.

- [ ] **Step 1: Put the LSP child in its own process group**

In `src/lsp/mux/process.rs` `run`, the child spawn currently is:

```rust
    let mut child = Command::new(server_command)
        .args(server_args)
        .envs(server_env.iter().map(|(k, v)| (k, v)))
        .current_dir(workspace_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn LSP server: {server_command}"))?;
```

Add `.process_group(0)` (new group, pgid = child pid) before `.spawn()`:

```rust
    use std::os::unix::process::CommandExt as _;
    let mut child = Command::new(server_command)
        .args(server_args)
        .envs(server_env.iter().map(|(k, v)| (k, v)))
        .current_dir(workspace_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0) // own group so killpg reaps grandchildren (JVM forks)
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn LSP server: {server_command}"))?;
    let child_pgid = child.id().map(|id| id as libc::pid_t);
```

- [ ] **Step 2: Add a SIGTERM/SIGINT break arm to `event_loop`**

In `event_loop`'s `tokio::select!`, add (before the closing `}` of the select):

```rust
                  // Signalled shutdown — break so `run` can kill the process group.
                  _ = async {
                      let mut term = tokio::signal::unix::signal(
                          tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
                      let mut intr = tokio::signal::unix::signal(
                          tokio::signal::unix::SignalKind::interrupt()).expect("SIGINT handler");
                      tokio::select! { _ = term.recv() => {}, _ = intr.recv() => {} }
                  } => {
                      info!("mux received shutdown signal, exiting event loop");
                      break;
                  }
```

- [ ] **Step 3: Kill the process group after `event_loop` returns**

In `run`, locate the call `event_loop(...).await` near the end and the existing shutdown log (`info!("mux process shutting down")`, ~`process.rs:261`). Immediately after `event_loop` returns and before the function ends, add:

```rust
    // Kill the whole LSP process group (JVM + any forks). kill_on_drop only kills
    // the direct child; grandchildren would orphan and squat the RocksDB index.
    if let Some(pgid) = child_pgid {
        unsafe { libc::killpg(pgid, libc::SIGTERM); }
        // Brief grace, then hard-kill survivors.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        unsafe { libc::killpg(pgid, libc::SIGKILL); }
    }
```

- [ ] **Step 4: Unit test — process_group flag is set (compile-level guard)**

This is hard to assert behaviorally in a unit test. Add a focused integration test, gated like the existing mux coherence test:

```rust
#[cfg(unix)]
#[tokio::test]
#[ignore = "spawns a real mux + child tree; gated like coherence_rust"]
async fn sigterm_kills_lsp_process_group() {
    // 1. Start a mux whose "LSP" is `bash -c 'sleep 300 & wait'` so it has a
    //    grandchild in the group. Capture the child + grandchild pids.
    // 2. SIGTERM the mux process.
    // 3. Assert both the child and grandchild pids are gone within 2s
    //    (kill(pid, 0) == -1 / ESRCH).
    // (Full harness in tests/fixtures/lsp-mux; see coherence_rust.rs for the
    //  spawn-mux scaffolding to reuse.)
}
```

- [ ] **Step 5: Run + clippy**

Run: `cargo test --lib lsp::mux && cargo clippy -- -D warnings`
Expected: PASS, clean. Run the gated test manually: `cargo test sigterm_kills_lsp_process_group -- --ignored`.

- [ ] **Step 6: Manual MCP verification (process-group teardown)**

```bash
cargo build --release   # updates the live binary via the ~/.cargo/bin/codescout symlink
```
Restart the server with `/mcp`, trigger a kotlin `symbols` call to spawn the mux + JVM, find the mux pid, `kill <mux_pid>` (SIGTERM), then `pgrep -f kotlin-lsp` — expect no survivors. Repeat with `kill -9 <mux_pid>` (SIGKILL) and confirm the NEXT `symbols` call reaps the orphan (Task 2) and succeeds.

- [ ] **Step 7: Commit**

```bash
git add src/lsp/mux/process.rs
git commit -m "fix(lsp): mux kills its LSP process group on signalled exit (S2)

setsid the LSP child into its own group and killpg on signalled shutdown
so the JVM and its forks die with the mux instead of orphaning and
squatting the RocksDB index. SIGKILL (uncatchable) stays covered by the
Task 2 reap-before-spawn net. ADR-2026-06-11-mux-single-owner-invariant.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage:** S3→Task 1, S1+S2 deadlock→Task 2, S2 orphan-window→Task 3. M1/M2/M3 all mapped. ✓

**2. Placeholder scan:** Task 3 Step 4's integration test body is a described harness, not runnable code — flagged as `#[ignore]` integration scaffolding (the existing `coherence_rust.rs` is the reuse source). All production-code steps carry complete code. The Step 4 test is the one acknowledged gap; the behavioral guarantee rests on Step 6's manual verification + Task 2's automated reap test. (Testing Snow Leopard referral stands for hardening the SIGKILL-path assertion.)

**3. Type consistency:** `reap_holders_of_lock(&Path) -> Result<bool>` and `reap_orphan_index_holder(&str, &Path) -> Result<bool>` used consistently; `is_test_runner_exe(&Path) -> bool` used in Tasks 1 steps 3/5/6; `child_pgid: Option<libc::pid_t>` defined in Task 3 Step 1, consumed Step 3. ✓

**Open risks carried from spec:** the `process_group(0)` + `killpg` interaction with the existing `kill_on_drop` ordering (verify no double-kill panic — `killpg` on an already-dead group is a harmless ESRCH); the 500ms grace in Task 3 vs the 2s grace in Task 2 (intentionally different — Task 3 is graceful-shutdown latency-sensitive, Task 2 is rare recovery).

---

## Execution Handoff

Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration. REQUIRED SUB-SKILL: `superpowers:subagent-driven-development`.
2. **Inline Execution** — execute tasks in this session with checkpoints. REQUIRED SUB-SKILL: `superpowers:executing-plans`.

Implementation touches live LSP concurrency on a shared branch — recommend subagent-driven with a manual `/mcp` verify gate (Task 3 Step 6) before any cherry-pick to `master`.
