const NATIVE_CONTEXT_ENV_COMMAND: &str =
    "printf '%s|%s|%s' \"${CODEX_SESSION_ID-}\" \"${CODEX_THREAD_ID-}\" \"${CODEX_VERSION-}\"";

struct NativeContextWire {
    dir: tempfile::TempDir,
    client: tokio::io::BufReader<tokio::io::DuplexStream>,
    task: tokio::task::JoinHandle<()>,
    pending: std::collections::BTreeMap<u64, Value>,
}

impl Drop for NativeContextWire {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl NativeContextWire {
    async fn new(client_name: &str) -> Self {
        let (dir, server) = make_server().await;
        let (client, transport) = tokio::io::duplex(65536);
        let task = tokio::spawn(async move {
            let (read, write) = tokio::io::split(transport);
            let running = server.serve((read, write)).await.unwrap();
            let _ = running.waiting().await;
        });
        let mut wire = Self {
            dir,
            client: tokio::io::BufReader::new(client),
            task,
            pending: std::collections::BTreeMap::new(),
        };
        wire.send(json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize",
            "params": {
                "protocolVersion":"2024-11-05",
                "capabilities":{"elicitation":{"form":{}}},
                "clientInfo":{"name":client_name,"version":"0.160.0"}
            }
        }))
        .await;
        let initialized = wire.recv(1).await;
        assert!(initialized.get("error").is_none(), "{initialized}");
        wire.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        wire
    }

    async fn send(&mut self, value: Value) {
        use tokio::io::AsyncWriteExt;
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            self.client
                .write_all(format!("{value}\n").as_bytes())
                .await
                .unwrap();
            self.client.flush().await.unwrap();
        })
        .await
        .unwrap();
    }

    async fn recv(&mut self, id: u64) -> Value {
        if let Some(value) = self.pending.remove(&id) {
            return value;
        }
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            use tokio::io::AsyncBufReadExt;
            loop {
                let mut line = String::new();
                assert!(self.client.read_line(&mut line).await.unwrap() > 0);
                let value: Value = serde_json::from_str(&line).unwrap();
                if value.get("method").and_then(Value::as_str) == Some("elicitation/create") {
                    self.send(json!({"jsonrpc":"2.0","id":value["id"],
                                     "result":{"action":"decline"}}))
                        .await;
                } else if let Some(response_id) = value.get("id").and_then(Value::as_u64) {
                    if response_id == id {
                        return value;
                    }
                    self.pending.insert(response_id, value);
                }
            }
        })
        .await
        .unwrap()
    }

    async fn call(&mut self, id: u64, meta: Value, arguments: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
                         "params":{"name":"run_command","arguments":arguments,"_meta":meta}}))
            .await;
        let response = self.recv(id).await;
        assert!(response.get("error").is_none(), "{response}");
        response
    }
}

fn native_context_output(response: &Value) -> Value {
    response["result"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|block| {
            block["text"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
        })
        .unwrap_or_else(|| panic!("missing structured output: {response}"))
}

fn native_context_ambient_env() -> String {
    ["CODEX_SESSION_ID", "CODEX_THREAD_ID", "CODEX_VERSION"]
        .iter()
        .map(|key| std::env::var(key).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("|")
}

#[tokio::test]
async fn native_context_wire_keeps_root_and_child_distinct() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    for (id, meta, expected) in [
        (
            2,
            json!({"sessionId":"native-root","threadId":"native-root"}),
            "native-root|native-root|0.160.0",
        ),
        (
            3,
            json!({"sessionId":"native-root","threadId":"native-child"}),
            "native-root|native-child|0.160.0",
        ),
        (
            4,
            json!({"sessionId":"native-root"}),
            "native-root||0.160.0",
        ),
    ] {
        let response = wire
            .call(
                id,
                meta,
                json!({"command":NATIVE_CONTEXT_ENV_COMMAND,"effect":"read"}),
            )
            .await;
        let output = native_context_output(&response);
        assert_eq!(output["exit_code"], 0, "{response}");
        assert_eq!(output["stdout"], expected);
    }
}

#[tokio::test]
async fn native_context_wire_does_not_infer_from_thread_or_foreign_client() {
    let ambient = native_context_ambient_env();
    for (name, meta) in [
        ("codex-mcp-client", json!({"threadId":"unverified-child"})),
        ("codex-mcp-client", json!({})),
        (
            "claude-code",
            json!({"sessionId":"foreign-root","threadId":"foreign-root"}),
        ),
    ] {
        let mut wire = NativeContextWire::new(name).await;
        let response = wire
            .call(
                2,
                meta,
                json!({"command":NATIVE_CONTEXT_ENV_COMMAND,"effect":"read"}),
            )
            .await;
        assert_eq!(native_context_output(&response)["stdout"], ambient);
    }
}

#[tokio::test]
async fn native_context_wire_rejects_malformed_identity_before_spawn() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    for (index, meta) in [
        json!({"sessionId":null,"threadId":"child"}),
        json!({"sessionId":42,"threadId":"child"}),
        json!({"sessionId":"  ","threadId":"child"}),
        json!({"sessionId":"root\0bad","threadId":"child"}),
        json!({"sessionId":"root","threadId":false}),
    ]
    .into_iter()
    .enumerate()
    {
        let response = wire
            .call(
                index as u64 + 2,
                meta,
                json!({"command":"printf executed > native-context-marker","effect":"write"}),
            )
            .await;
        assert!(
            !wire.dir.path().join("native-context-marker").exists(),
            "{response}"
        );
        assert!(
            native_context_output(&response)["stdout"].is_null(),
            "{response}"
        );
    }
}

#[tokio::test]
async fn native_context_wire_keeps_concurrent_requests_and_absence_isolated() {
    let ambient = native_context_ambient_env();
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    for (id, root, child) in [
        (2, "first-root", "first-child"),
        (3, "second-root", "second-child"),
    ] {
        wire.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
            "params":{"name":"run_command",
                      "arguments":{"command":format!("sleep 0.05; {NATIVE_CONTEXT_ENV_COMMAND}"),"effect":"read"},
                      "_meta":{"sessionId":root,"threadId":child}}})).await;
    }
    assert_eq!(
        native_context_output(&wire.recv(2).await)["stdout"],
        "first-root|first-child|0.160.0"
    );
    assert_eq!(
        native_context_output(&wire.recv(3).await)["stdout"],
        "second-root|second-child|0.160.0"
    );
    let absent = wire
        .call(
            4,
            json!({}),
            json!({"command":NATIVE_CONTEXT_ENV_COMMAND,"effect":"read"}),
        )
        .await;
    assert_eq!(native_context_output(&absent)["stdout"], ambient);
    assert_eq!(native_context_ambient_env(), ambient);
}

#[tokio::test]
async fn native_context_wire_reaches_background_shell() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    let response = wire
        .call(
            2,
            json!({"sessionId":"background-root","threadId":"background-child"}),
            json!({"command":NATIVE_CONTEXT_ENV_COMMAND,"run_in_background":true,"effect":"read"}),
        )
        .await;
    let handle = native_context_output(&response)["output_id"]
        .as_str()
        .unwrap()
        .to_string();
    for attempt in 0..100 {
        let response = wire
            .call(
                attempt + 3,
                json!({}),
                json!({"command":format!("tail -10 {handle}"),"effect":"read"}),
            )
            .await;
        let output = native_context_output(&response);
        if let Some(stdout) = output["stdout"].as_str().filter(|s| !s.is_empty()) {
            assert_eq!(stdout, "background-root|background-child|0.160.0");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("background command did not yield observed output");
}

#[tokio::test]
async fn native_context_wire_reaches_interactive_shell() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    let response = wire
        .call(
            2,
            json!({"sessionId":"interactive-root","threadId":"interactive-child"}),
            json!({"command":NATIVE_CONTEXT_ENV_COMMAND,"interactive":true,"effect":"read"}),
        )
        .await;
    assert_eq!(
        native_context_output(&response)["stdout"],
        "interactive-root|interactive-child|0.160.0",
        "{response}"
    );
}
