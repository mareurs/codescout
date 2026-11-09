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

#[cfg(unix)]
#[tokio::test]
async fn native_provenance_fence_is_advertised_on_supported_hosts() {
    let (_dir, server) = make_server().await;
    let capabilities = rmcp::ServerHandler::get_info(&server).capabilities;
    let expected = json!({"version":2,"protocol":"coordinated-checkout-v1"});
    assert_eq!(
        capabilities
            .experimental
            .as_ref()
            .and_then(|map| map.get("codescout/nativeExecutionFence")),
        expected.as_object(),
    );
}

#[cfg(unix)]
#[tokio::test]
async fn native_wire_foreign_workspace_command_is_refused_while_formatting() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    let target = tempfile::tempdir().unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(target.path())
        .status()
        .unwrap()
        .success());
    let path = target.path().join("probe.txt");
    std::fs::write(&path, b"before").unwrap();
    let domain = crate::tools::native_provenance_fence::coordination_root().unwrap();
    let _coordinator = crate::tools::native_provenance_fence::Fence::try_acquire(&domain).unwrap();
    let _checkout =
        crate::tools::native_provenance_fence::Fence::try_acquire(target.path()).unwrap();
    wire.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name":"run_command","arguments":{
            "command":format!("printf changed > '{}'",path.display()),"effect":"read"
        },"_meta":{"sessionId":"native-root","threadId":"native-root"}}}))
        .await;
    let response = wire.recv(2).await;
    assert!(
        response
            .to_string()
            .contains("native provenance coordination refused dispatch"),
        "{response}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"before");
}

/// Stamp the server's pid-keyed rendezvous slot with `session`, the way the companion hook does.
/// Several Codex servers share one ppid, so a slot can name another thread's session; this is
/// that shape. The mtime is forced forward so `Rendezvous::poll` re-reads the slot.
fn native_stamp_rendezvous(dir: &std::path::Path, session: &str) {
    let slot = dir
        .join("servers")
        .join(format!("{}.json", std::process::id()));
    let mut entry: crate::tools::rendezvous::Entry =
        serde_json::from_str(&std::fs::read_to_string(&slot).unwrap()).unwrap();
    entry.session = Some(session.to_string());
    entry.hook_at = Some(chrono::Utc::now());
    std::fs::write(&slot, serde_json::to_string(&entry).unwrap()).unwrap();
    filetime::set_file_mtime(&slot, filetime::FileTime::from_unix_time(2_000_000_000, 0)).unwrap();
}

/// The `cc_session_id` usage.db recorded for the one `run_command` whose input names `marker`.
fn native_recorded_session(dir: &std::path::Path, marker: &str) -> String {
    rusqlite::Connection::open(dir.join(".codescout").join("usage.db"))
        .unwrap()
        .query_row(
            "SELECT cc_session_id FROM tool_calls \
             WHERE tool_name = 'run_command' AND input_json LIKE ?1",
            [format!("%{marker}%")],
            |r| r.get(0),
        )
        .unwrap_or_else(|e| panic!("no single usage row for {marker}: {e}"))
}

/// Docs: ce484db75532c7f0. The commit guard reads `usage.db.cc_session_id`
/// as the writer of a path. A Codex call used to be recorded under whatever the rendezvous
/// slot named, which another Codex thread sharing the ppid can stamp, so the writer came out
/// as the wrong thread. The native root on the request is the per-call identity.
#[tokio::test]
async fn native_codex_root_is_the_recorded_session_not_the_rendezvous_slot() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    wire.call(
        2,
        json!({}),
        json!({"command":"printf warmup","effect":"read"}),
    )
    .await;
    native_stamp_rendezvous(wire.dir.path(), "other-threads-session");
    wire.call(
        3,
        json!({"sessionId":"native-root","threadId":"native-child"}),
        json!({"command":"printf marker-own","effect":"read"}),
    )
    .await;
    // BARE id: the guard's `foreign_writer` compares usage.db's value with a trailer-stripped
    // `codex:<id>`, so a qualified `codex:native-root` here would never match it.
    assert_eq!(
        native_recorded_session(wire.dir.path(), "marker-own"),
        "native-root"
    );
}

#[tokio::test]
async fn native_codex_interleaved_roots_are_recorded_each_under_its_own_id() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    wire.call(
        2,
        json!({}),
        json!({"command":"printf warmup","effect":"read"}),
    )
    .await;
    native_stamp_rendezvous(wire.dir.path(), "other-threads-session");
    for (id, root, marker) in [
        (3, "first-root", "marker-first"),
        (4, "second-root", "marker-second"),
    ] {
        wire.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
            "params":{"name":"run_command",
                      "arguments":{"command":format!("sleep 0.05; printf {marker}"),"effect":"read"},
                      "_meta":{"sessionId":root,"threadId":root}}}))
            .await;
    }
    wire.recv(3).await;
    wire.recv(4).await;
    assert_eq!(
        native_recorded_session(wire.dir.path(), "marker-first"),
        "first-root"
    );
    assert_eq!(
        native_recorded_session(wire.dir.path(), "marker-second"),
        "second-root"
    );
}

#[tokio::test]
async fn native_attribution_is_unchanged_without_a_verified_codex_root() {
    for (index, (client, meta)) in [
        // A foreign client's `sessionId` is not a native Codex root.
        (
            "claude-code",
            json!({"sessionId":"foreign-root","threadId":"foreign-root"}),
        ),
        // A thread alone does not establish the native logical root.
        ("codex-mcp-client", json!({"threadId":"unverified-child"})),
        ("codex-mcp-client", json!({})),
    ]
    .into_iter()
    .enumerate()
    {
        let mut wire = NativeContextWire::new(client).await;
        wire.call(
            2,
            json!({}),
            json!({"command":"printf warmup","effect":"read"}),
        )
        .await;
        native_stamp_rendezvous(wire.dir.path(), "rendezvous-session");
        let marker = format!("marker-unchanged-{index}");
        wire.call(
            3,
            meta,
            json!({"command":format!("printf {marker}"),"effect":"read"}),
        )
        .await;
        assert_eq!(
            native_recorded_session(wire.dir.path(), &marker),
            "rendezvous-session",
            "{client}"
        );
    }
}

/// An asserted principal (the companion's stamp) or `_meta` conversation is per-call too, and
/// the only source that separates a subagent from its parent, so it outranks the native Codex
/// root. A wrong fix `native_root.or(asserted_session)` records the bare root instead.
#[tokio::test]
async fn native_codex_root_does_not_outrank_an_asserted_principal_or_conversation() {
    for (index, (asserted, expected)) in [
        (
            json!({"dev.codescout.mcp/agentId":"principal-session/agent-a"}),
            "principal-session/agent-a",
        ),
        (
            json!({"_meta":{"dev.codescout.mcp/conversationId":"asserted-conversation"}}),
            "asserted-conversation",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut wire = NativeContextWire::new("codex-mcp-client").await;
        wire.call(
            2,
            json!({}),
            json!({"command":"printf warmup","effect":"read"}),
        )
        .await;
        native_stamp_rendezvous(wire.dir.path(), "rendezvous-session");
        let marker = format!("marker-asserted-{index}");
        let mut meta = json!({"sessionId":"native-root","threadId":"native-child"});
        let mut arguments = json!({"command":format!("printf {marker}"),"effect":"read"});
        match asserted {
            Value::Object(map) if map.contains_key("_meta") => {
                meta.as_object_mut()
                    .unwrap()
                    .extend(map["_meta"].as_object().unwrap().clone());
            }
            Value::Object(map) => arguments.as_object_mut().unwrap().extend(map),
            _ => unreachable!(),
        }
        wire.call(3, meta, arguments).await;
        assert_eq!(
            native_recorded_session(wire.dir.path(), &marker),
            expected,
            "{expected}"
        );
    }
}

/// A Codex call whose native id does not parse yields `NativeInvocation::Invalid`, which is
/// not a root: the recorder falls through to the order it always had.
#[tokio::test]
async fn native_codex_invalid_root_falls_through_to_the_rendezvous_slot() {
    for (index, meta) in [
        json!({"sessionId":"","threadId":"native-child"}),
        json!({"sessionId":42,"threadId":"native-child"}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut wire = NativeContextWire::new("codex-mcp-client").await;
        wire.call(
            2,
            json!({}),
            json!({"command":"printf warmup","effect":"read"}),
        )
        .await;
        native_stamp_rendezvous(wire.dir.path(), "rendezvous-session");
        let marker = format!("marker-invalid-{index}");
        wire.call(
            3,
            meta,
            json!({"command":format!("printf {marker}"),"effect":"read"}),
        )
        .await;
        assert_eq!(
            native_recorded_session(wire.dir.path(), &marker),
            "rendezvous-session",
            "{index}"
        );
    }
}

/// A refused `workspace=` pin is recorded too (`refuse_pin`), and must rank the native Codex
/// root like any other row: otherwise a refused Codex row carries the foreign rendezvous
/// session while its siblings carry the root.
#[tokio::test]
async fn native_codex_root_is_recorded_for_a_refused_pin() {
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    wire.call(
        2,
        json!({}),
        json!({"command":"printf warmup","effect":"read"}),
    )
    .await;
    native_stamp_rendezvous(wire.dir.path(), "other-threads-session");
    let missing = wire.dir.path().join("no-such-workspace");
    let response = wire
        .call(
            3,
            json!({"sessionId":"native-root","threadId":"native-child"}),
            json!({"command":"printf marker-refused","effect":"read",
                   "workspace":missing.to_string_lossy()}),
        )
        .await;
    assert!(
        response.to_string().contains("no-such-workspace"),
        "the pin must have been refused: {response}"
    );
    assert_eq!(
        native_recorded_session(wire.dir.path(), "marker-refused"),
        "native-root"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn stock_observer_wire_records_pinned_effects_and_rootless_api_calls() {
    use crate::observations::store::{disable, enable, read_store, Limits};
    let mut wire = NativeContextWire::new("codex-mcp-client").await;
    // Observe an actual Git checkout, not the server's startup/default project.
    let pinned = tempfile::tempdir().unwrap();
    git2::Repository::init(pinned.path()).unwrap();
    enable(pinned.path()).unwrap();
    for (id, path, meta) in [
        (
            2,
            "rooted.txt",
            json!({"sessionId":"native-root","threadId":"native-child","callId":"rooted","x-codex-turn-metadata":{"session_id":"native-root","thread_id":"native-child","turn_id":"turn"}}),
        ),
        (3, "api.txt", json!({"threadId":"api-child","callId":"api"})),
    ] {
        wire.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{
            "name":"create_file","arguments":{"workspace":pinned.path(),"path":path,"content":"wire-private-canary"},"_meta":meta
        }})).await;
        let result = wire.recv(id).await;
        assert!(result.get("error").is_none(), "{result}");
        assert_eq!(
            std::fs::read_to_string(pinned.path().join(path)).unwrap(),
            "wire-private-canary"
        );
    }
    let r = read_store(pinned.path(), Limits::default());
    let committed: Vec<_> = r
        .records
        .iter()
        .filter(|r| r.targets.iter().any(|t| t.phase == "committed"))
        .collect();
    // Activating a new project also writes its .gitignore through atomic_write.
    let paths: std::collections::BTreeSet<_> = committed
        .iter()
        .flat_map(|r| {
            r.targets
                .iter()
                .filter(|t| t.phase == "committed")
                .map(|t| {
                    std::path::Path::new(&t.path)
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                })
        })
        .collect();
    assert_eq!(
        paths,
        std::collections::BTreeSet::from([".gitignore", "rooted.txt", "api.txt"])
    );
    assert_eq!(committed.len(), 3);
    let rooted = committed
        .iter()
        .find(|r| r.native_call_id.as_deref() == Some("rooted"))
        .unwrap();
    assert_eq!(
        rooted.caller.principal.as_deref(),
        Some("codex:native-root")
    );
    assert_eq!(rooted.caller.thread_id.as_deref(), Some("native-child"));
    let api = committed
        .iter()
        .find(|r| r.native_call_id.as_deref() == Some("api"))
        .unwrap();
    assert_eq!(api.caller.principal, None);
    assert_eq!(api.caller.thread_id.as_deref(), Some("api-child"));
    assert!(r
        .records
        .iter()
        .all(|r| r.workspace == pinned.path().to_str().unwrap()));
    assert!(!serde_json::to_string(&r)
        .unwrap()
        .contains("wire-private-canary"));

    wire.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{
        "name":"create_file","arguments":{"workspace":pinned.path(),"path":"rooted.txt","content":"refusal-private-canary"},
        "_meta":{"sessionId":"native-root","threadId":"native-child","callId":"refused"}
    }})).await;
    let refused = wire.recv(4).await;
    assert!(refused.get("error").is_none(), "{refused}");
    assert_eq!(refused["result"]["isError"], false);
    let r = read_store(pinned.path(), Limits::default());
    assert!(r
        .records
        .iter()
        .any(|r| r.native_call_id.as_deref() == Some("refused")
            && r.event == "result-observed"
            && r.outcome == "refused"));
    assert!(!r
        .records
        .iter()
        .any(|r| r.native_call_id.as_deref() == Some("refused")
            && r.targets.iter().any(|t| t.phase == "committed")));

    let count = r.records.len();
    disable(pinned.path()).unwrap();
    wire.send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{
        "name":"create_file","arguments":{"workspace":pinned.path(),"path":"disabled.txt","content":"still works"},
        "_meta":{"sessionId":"native-root","callId":"disabled"}
    }})).await;
    let disabled = wire.recv(5).await;
    assert!(disabled.get("error").is_none(), "{disabled}");
    assert_eq!(
        std::fs::read_to_string(pinned.path().join("disabled.txt")).unwrap(),
        "still works"
    );
    assert_eq!(
        read_store(pinned.path(), Limits::default()).records.len(),
        count
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn stock_observer_records_both_cancel_arms_before_connection_close() {
    use crate::observations::{
        server::{observe_call, LocalInvocationId, ObservationContext},
        store::{enable, read_store, Limits},
    };
    for timeout in [None, Some(30)] {
        let root = tempfile::tempdir().unwrap();
        git2::Repository::init(root.path()).unwrap();
        enable(root.path()).unwrap();
        let token = tokio_util::sync::CancellationToken::new();
        let cancel = token.clone();
        let path = root.path().to_path_buf();
        let (started, ready) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            observe_call(
                ObservationContext::from_meta(Some(path), None, None),
                LocalInvocationId::new("cancel-connection", 0),
                "run_command",
                &json!({}),
                async {
                    CodeScoutServer::race_against_cancel(
                        async {
                            started.send(()).unwrap();
                            std::future::pending::<Result<Vec<Content>, anyhow::Error>>().await
                        },
                        token,
                        timeout,
                        "run_command",
                        (),
                    )
                    .await
                    .map(CallToolResult::success)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))
                },
            )
            .await
        });
        ready.await.unwrap();
        cancel.cancel();
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert!(
            !task.is_finished(),
            "the legacy cancellation response suppression must remain unchanged"
        );
        let before_close = read_store(root.path(), Limits::default());
        assert!(
            before_close
                .records
                .iter()
                .any(|r| r.outcome == "missing-terminal"),
            "timeout={timeout:?}: cancellation diagnostics must not wait for connection teardown"
        );
        task.abort();
        let _ = task.await;
        assert_eq!(
            read_store(root.path(), Limits::default())
                .records
                .iter()
                .filter(|r| r.outcome == "missing-terminal")
                .count(),
            1
        );
    }
}
