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

/// Absolute path to the debug binary built by `cargo build`.
fn binary_path() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    std::path::PathBuf::from(manifest_dir).join("target/debug/codescout")
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

#[tokio::test]
async fn write_lock_contention_produces_recoverable_error() {
    let bin = binary_path();
    if !bin.exists() {
        eprintln!(
            "SKIP: binary not found at {} — run `cargo build` first",
            bin.display()
        );
        return;
    }

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

    // Spawn the binary against the same project directory.
    let mut child = Command::new(&bin)
        .args(["start", "--project", project.to_str().unwrap()])
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
    // Release our lock after the binary has responded.
    lock_file.unlock().expect("failed to release test lock");

    assert!(
        response.contains("another codescout instance"),
        "expected contention error in response, got:\n{response}"
    );
}
