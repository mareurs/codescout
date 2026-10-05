//! End-to-end: a codescout binary instance contends with a lock held by the
//! test process itself. One process wins (the test); the other (the binary)
//! returns a RecoverableError with the contention message.
//!
//! This tests the real cross-process flock path without the race-condition
//! fragility of two binary instances completing sub-millisecond edits. The
//! test process pre-acquires the OS-level flock on `.codescout/write.lock`,
//! spawns a codescout binary against the same directory, sends `edit_file`,
//! and asserts the binary times out and surfaces the expected error.
//!
//! Run with: `cargo test --test cross_process_write_lock`

use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Absolute path to the `codescout` binary for this test run.
///
/// `CARGO_BIN_EXE_<name>` is set by Cargo for integration tests and, crucially,
/// **guarantees the binary is built before the test runs** — so there is no
/// missing-binary case, and therefore no skip branch that could report green
/// without executing. It also tracks the active profile and any custom
/// `CARGO_TARGET_DIR`, both of which the previous hand-built
/// `target/debug/codescout` path got wrong.
///
/// docs/issues/archive/2026-08-27-cross-process-write-lock-test-passes-when-it-does-not-run.md
fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_codescout"))
}

/// Write one newline-delimited JSON-RPC message to stdin.
async fn send(stdin: &mut tokio::process::ChildStdin, msg: &serde_json::Value) {
    let mut line = msg.to_string();
    line.push('\n');
    stdin.write_all(line.as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();
}

/// Read newline-delimited messages until we find the one with the given id.
async fn recv_id(reader: &mut BufReader<tokio::process::ChildStdout>, id: u64) -> String {
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .expect("failed to read from server stdout");
        assert!(n > 0, "server stdout closed unexpectedly");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                return trimmed.to_owned();
            }
        }
    }
}

/// Send `initialize`, drain the response, send `notifications/initialized`.
async fn mcp_handshake(
    stdin: &mut tokio::process::ChildStdin,
    stdout: &mut BufReader<tokio::process::ChildStdout>,
) {
    send(
        stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0.1"}
            }
        }),
    )
    .await;

    recv_id(stdout, 1).await;

    send(
        stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    )
    .await;
}

/// Hold the project's OS-level flock from THIS process, spawn a codescout binary
/// against the same project, ask it to `edit_file`, and return its raw JSON-RPC
/// response to that call.
///
/// `holder_record` is the exact content to place in `.codescout/write.lock.holder`
/// before the binary is spawned, or `None` to write no record at all. The test
/// process takes a raw flock rather than going through `write_guard::acquire`, so
/// it writes no record of its own: whichever branch of the binary's refusal a test
/// reaches is decided by this argument and nothing else. `None` reaches the
/// anonymous branch; a well-formed `<epoch_ms>\t<holder>` reaches the named one.
async fn contended_edit_response(holder_record: Option<&str>) -> String {
    let bin = binary_path();

    // Create a temp project.
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    std::fs::write(project.join("target.txt"), "hello world").unwrap();

    // Pre-create the lock file directory and acquire the OS-level exclusive
    // flock from the test process. The binary instance will contend against
    // this lock when it tries to run `edit_file`.
    let lock_dir = project.join(".codescout");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let lock_path = lock_dir.join("write.lock");
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .expect("failed to open lock file");
    use fs4::fs_std::FileExt;
    lock_file
        .try_lock_exclusive()
        .expect("test process should acquire the lock uncontested");

    // The holder-record sidecar, written (or not) AFTER the flock is taken —
    // the same order `write_guard::acquire` uses, and the property that makes the
    // record trustworthy: only the lock's holder can have written it.
    if let Some(record) = holder_record {
        std::fs::write(lock_dir.join("write.lock.holder"), record)
            .expect("failed to write holder record");
    }

    // Spawn the binary against the same project directory.
    //
    // Without env overrides, the spawned binary's `ServerEnv::from_env()` sets
    // `guide_hints_dir: None`, which falls back to the developer's REAL
    // `per_user_state_dir()` — the guide-hint ledger's 35-day file-deleting GC
    // would then run over `~/.local/state/codescout/guide_hints/`, and the
    // child inherits `CLAUDE_CODE_SESSION_ID` too, so it could write into the
    // developer's live session ledger. Redirect both into scratch. `state_dir`
    // is bound to a named local (not a temporary) so the tempdir isn't deleted
    // before the child process is done reading/writing it. See
    // docs/issues/archive/2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md.
    let state_dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(&bin)
        .args(["start", "--project", project.to_str().unwrap()])
        .env("XDG_STATE_HOME", state_dir.path())
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn codescout");

    let mut child_stdin = child.stdin.take().unwrap();
    let mut child_out = BufReader::new(child.stdout.take().unwrap());

    mcp_handshake(&mut child_stdin, &mut child_out).await;

    // Send `edit_file`. The binary will try to acquire the flock, spin-poll
    // for `write_lock_timeout_secs` (default 5 s), and return a RecoverableError.
    send(
        &mut child_stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "edit_file",
                "arguments": {
                    "path": "target.txt",
                    "old_string": "hello world",
                    "new_string": "HELLO WORLD"
                }
            }
        }),
    )
    .await;

    // Allow 15 s — the binary should time out within ~5 s and respond.
    let response = tokio::time::timeout(Duration::from_secs(15), recv_id(&mut child_out, 10))
        .await
        .expect("binary did not respond within 15 s");

    child.kill().await.ok();
    // Release our lock after the binary has responded. Called through the trait: on a
    // toolchain with `File::unlock` (1.89) the method form resolves to the std inherent
    // one, which is newer than this crate's MSRV.
    FileExt::unlock(&lock_file).expect("failed to release test lock");

    response
}

/// The ANONYMOUS branch: the flock is held and no holder record exists, so the
/// refusal can say only that some other instance is writing.
#[tokio::test]
async fn write_lock_contention_produces_recoverable_error() {
    let response = contended_edit_response(None).await;

    assert!(
        response.contains("another codescout instance"),
        "expected contention error in response, got:\n{response}"
    );
    assert!(
        !response.contains("write lock held by"),
        "with no holder record the refusal must not name a holder, got:\n{response}"
    );
}

/// The NAMED branch, end to end: the same contention, but the holder record is
/// present and well-formed, so the refusal must name the holder from the record.
///
/// Until this test existed the named branch of `write_guard::acquire` was covered
/// only in-process (`write_guard::tests`), and the one cross-process test above
/// reached only the anonymous fallback — a regression that made the binary ignore
/// the record, or read it from the wrong path, would have passed every test. The
/// holder string is deliberately one no other test or default uses, so the
/// assertion cannot be satisfied by anything but the record's own bytes.
///
/// See docs/issues/archive/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md.
#[tokio::test]
async fn write_lock_contention_names_the_holder_recorded_in_the_sidecar() {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let record = format!("{now_ms}\tcodescout:sid-alpha reindex");

    let response = contended_edit_response(Some(&record)).await;

    assert!(
        response.contains("write lock held by codescout:sid-alpha reindex"),
        "the refusal must name the holder recorded in write.lock.holder, got:\n{response}"
    );
    assert!(
        !response.contains("another codescout instance"),
        "a readable holder record selects the NAMED branch, not the anonymous one, got:\n{response}"
    );
}
