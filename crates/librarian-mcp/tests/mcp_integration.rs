use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write one JSON-RPC line to stdin and read back the response with the
/// matching `id`. Lines that parse as notifications (no `id`) are skipped.
fn send_request(
    stdin: &mut ChildStdin,
    reader: &mut BufReader<std::process::ChildStdout>,
    request: &serde_json::Value,
) -> serde_json::Value {
    let id = request.get("id").cloned();
    let mut line = serde_json::to_string(request).unwrap();
    line.push('\n');
    stdin.write_all(line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    // Read lines until we find one with a matching id (skip notifications).
    loop {
        let mut buf = String::new();
        reader.read_line(&mut buf).expect("read_line failed");
        let val: serde_json::Value =
            serde_json::from_str(buf.trim()).expect("response is not valid JSON");

        match &id {
            None => return val,
            Some(expected_id) => {
                if val.get("id") == Some(expected_id) {
                    return val;
                }
                // Skip notifications or responses with a different id.
            }
        }
    }
}

/// Send a notification (no id, no response expected).
fn send_notification(stdin: &mut ChildStdin, notification: &serde_json::Value) {
    let mut line = serde_json::to_string(notification).unwrap();
    line.push('\n');
    stdin.write_all(line.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn mcp_subprocess_integration() {
    // -----------------------------------------------------------------------
    // 1. Set up temp workspace
    // -----------------------------------------------------------------------
    let tmp = tempfile::TempDir::new().unwrap();

    // Point at the real fixture directory (has 3 .md files from Phase 7).
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/repo_a");
    assert!(
        fixture_dir.exists(),
        "fixture dir missing: {}",
        fixture_dir.display()
    );

    let ws_path = tmp.path().join("workspace.toml");
    let db_path = tmp.path().join("catalog.db");

    // Workspace config: one root, one catch-all rule for .md files.
    let ws_toml = format!(
        "[[roots]]\nname = \"repo_a\"\npath = \"{}\"\n\n[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        fixture_dir.display()
    );
    std::fs::write(&ws_path, &ws_toml).unwrap();

    // -----------------------------------------------------------------------
    // 2. Pre-seed catalog via `librarian-mcp reindex`
    // -----------------------------------------------------------------------
    let reindex_status = Command::new(assert_cmd::cargo::cargo_bin("librarian-mcp"))
        .arg("reindex")
        .env("LIBRARIAN_WORKSPACE", &ws_path)
        .env("LIBRARIAN_DB", &db_path)
        .status()
        .expect("failed to spawn librarian-mcp reindex");
    assert!(
        reindex_status.success(),
        "reindex exited with non-zero status"
    );

    // -----------------------------------------------------------------------
    // 3. Spawn librarian-mcp in stdio mode
    // -----------------------------------------------------------------------
    let mut child = Command::new(assert_cmd::cargo::cargo_bin("librarian-mcp"))
        .env("LIBRARIAN_WORKSPACE", &ws_path)
        .env("LIBRARIAN_DB", &db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn librarian-mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Wrap child so it is killed on drop (even on panic).
    let _guard = ChildGuard(child);

    // -----------------------------------------------------------------------
    // 4. JSON-RPC handshake
    // -----------------------------------------------------------------------

    // --- initialize ---
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1" }
        }
    });
    let init_resp = send_request(&mut stdin, &mut reader, &init_req);
    assert_eq!(init_resp["id"], 1, "initialize response id mismatch");
    assert!(
        init_resp.get("result").is_some(),
        "initialize: expected result, got: {init_resp}"
    );

    // --- notifications/initialized ---
    send_notification(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    );

    // --- tools/list ---
    let tools_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let tools_resp = send_request(&mut stdin, &mut reader, &tools_req);
    assert_eq!(tools_resp["id"], 2, "tools/list response id mismatch");
    let tools = tools_resp["result"]["tools"]
        .as_array()
        .expect("tools/list result.tools should be an array");
    assert_eq!(
        tools.len(),
        13,
        "expected 13 tools, got {}: {:?}",
        tools.len(),
        tools.iter().map(|t| &t["name"]).collect::<Vec<_>>()
    );

    // --- tools/call artifact_find ---
    let call_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "artifact_find",
            "arguments": {}
        }
    });
    let call_resp = send_request(&mut stdin, &mut reader, &call_req);
    assert_eq!(call_resp["id"], 3, "tools/call response id mismatch");
    assert!(
        call_resp.get("result").is_some(),
        "artifact_find: expected result, got: {call_resp}"
    );

    // The content array contains a text block with the JSON response payload.
    let content = call_resp["result"]["content"]
        .as_array()
        .expect("result.content should be an array");
    assert!(!content.is_empty(), "artifact_find returned empty content");

    let text = content[0]["text"]
        .as_str()
        .expect("content[0].text should be a string");
    let payload: serde_json::Value =
        serde_json::from_str(text).expect("content[0].text is not valid JSON");

    // count must be >= 1 (we indexed at least the 3 fixture .md files).
    let count = payload["count"]
        .as_u64()
        .or_else(|| payload["count"].as_i64().map(|v| v as u64))
        .expect("artifact_find result should have a numeric 'count' field");
    assert!(
        count >= 1,
        "expected at least 1 artifact from fixture, got count={count}"
    );

    // ChildGuard drop kills the subprocess.
    std::thread::sleep(Duration::from_millis(50));
}
