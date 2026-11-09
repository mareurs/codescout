use super::server::{observe_call, LocalInvocationId, ObservationContext};
use super::store::{enable, read_store, Limits};
use crate::tools::ClientIdentity;
use rmcp::model::{CallToolResult, Content};
use serde_json::{json, Value};
use std::path::Path;

fn root() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git2::Repository::init(d.path()).unwrap();
    enable(d.path()).unwrap();
    d
}
fn context(root: &Path, session: Option<&str>, thread: &str, call: &str) -> ObservationContext {
    let mut meta = json!({"threadId":thread,"callId":call,"x-codex-turn-metadata":{"thread_id":thread,"turn_id":"turn"}});
    if let Some(session) = session {
        meta["sessionId"] = json!(session);
        meta["x-codex-turn-metadata"]["session_id"] = json!(session);
    }
    ObservationContext::from_meta(
        Some(root.to_path_buf()),
        meta.as_object(),
        Some(&ClientIdentity {
            name: "codex-mcp-client".into(),
            version: Some("0.167.0".into()),
        }),
    )
}
fn returned(v: Value) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(v.to_string())]))
}

#[tokio::test]
async fn writes_are_facts_even_when_return_is_scalar() {
    let d = root();
    let path = d.path().join("one.txt");
    let result = observe_call(
        context(d.path(), Some("root"), "child", "call"),
        LocalInvocationId::fresh("connection"),
        "create_file",
        &json!({"content":"private-body-canary"}),
        async {
            super::server::dispatch_at(d.path());
            crate::util::fs::atomic_write(&path, "private-body-canary").unwrap();
            returned(json!("ok"))
        },
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "private-body-canary"
    );
    let r = read_store(d.path(), Limits::default());
    let committed: Vec<_> = r
        .records
        .iter()
        .filter(|r| r.targets.iter().any(|t| t.phase == "committed"))
        .collect();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].caller.principal.as_deref(), Some("codex:root"));
    assert_eq!(committed[0].caller.thread_id.as_deref(), Some("child"));
    let serialized = serde_json::to_string(&r).unwrap();
    assert!(!serialized.contains("private-body-canary"));
}
#[tokio::test]
async fn replaced_parent_between_attempt_and_commit_is_uncertain() {
    use super::server::{observe_write, WritePhase};
    use std::os::unix::fs::MetadataExt;
    let d = root();
    let parent = d.path().join("parent");
    std::fs::create_dir(&parent).unwrap();
    let path = parent.join("written.txt");
    let bytes = b"supplied bytes";
    let result = observe_call(
        context(d.path(), Some("root"), "child", "call"),
        LocalInvocationId::fresh("connection"),
        "create_file",
        &json!({}),
        async {
            super::server::dispatch_at(d.path());
            observe_write(&path, bytes, WritePhase::Attempted);
            let old = std::fs::metadata(&parent).unwrap();
            std::fs::rename(&parent, d.path().join("retired-parent")).unwrap();
            std::fs::create_dir(&parent).unwrap();
            let new = std::fs::metadata(&parent).unwrap();
            assert_ne!((old.dev(), old.ino()), (new.dev(), new.ino()));
            std::fs::write(&path, bytes).unwrap();
            observe_write(&path, bytes, WritePhase::Committed);
            returned(json!("ok"))
        },
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let records = read_store(d.path(), Limits::default()).records;
    let facts: Vec<_> = records.iter().flat_map(|r| &r.targets).collect();
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].phase, "attempted");
    assert_eq!(facts[1].phase, "uncertain");
    assert!(facts.iter().all(|t| t.phase != "committed"));
    assert!(facts.iter().all(|t| t.path == path.to_str().unwrap()));
    assert!(facts
        .iter()
        .all(|t| t.fingerprint == Some(super::contract::bytes_fingerprint(bytes))));
}
#[tokio::test]
async fn raw_native_turn_cwd_is_separate_from_the_reached_checkout() {
    let d = root();
    std::fs::create_dir(d.path().join("nested")).unwrap();
    let cwd = format!("{}/nested/../nested", d.path().display());
    let meta = json!({"sessionId":"root","threadId":"child","callId":"call","x-codex-turn-metadata":{"session_id":"root","thread_id":"child","turn_id":"turn","cwd":cwd}});
    let context = ObservationContext::from_meta(
        Some(d.path().to_owned()),
        meta.as_object(),
        Some(&ClientIdentity {
            name: "codex-mcp-client".into(),
            version: None,
        }),
    );
    observe_call(
        context,
        LocalInvocationId::fresh("connection"),
        "create_file",
        &json!({}),
        async {
            super::server::dispatch_at(d.path());
            crate::util::fs::atomic_write(&d.path().join("actual.txt"), "bytes").unwrap();
            returned(json!("ok"))
        },
    )
    .await
    .unwrap();
    let records = read_store(d.path(), Limits::default()).records;
    assert!(!records.is_empty());
    assert!(records
        .iter()
        .all(|r| r.native_cwd.as_deref() == Some(cwd.as_str())));
    assert!(records
        .iter()
        .all(|r| r.workspace == d.path().to_str().unwrap()));
}

#[tokio::test]
async fn non_utf8_target_never_fabricates_a_replacement_character_path() {
    use std::os::unix::ffi::OsStringExt;
    let d = root();
    let path = d
        .path()
        .join(std::ffi::OsString::from_vec(b"non-utf8-\xff.rs".to_vec()));
    observe_call(
        context(d.path(), Some("root"), "child", "non-utf8"),
        LocalInvocationId::fresh("connection"),
        "create_file",
        &json!({}),
        async {
            crate::util::fs::atomic_write(&path, "actual bytes").unwrap();
            returned(json!("ok"))
        },
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"actual bytes");
    let read = read_store(d.path(), Limits::default());
    assert!(
        read.records.iter().all(|r| r.targets.is_empty()),
        "a lossy pathname is not an actual target fact"
    );
    assert!(read
        .records
        .iter()
        .any(|r| r.diagnostics.iter().any(|d| d == "incomplete-input")));
}

#[tokio::test]
async fn thread_only_and_refusal_do_not_manufacture_success_or_root() {
    let d = root();
    observe_call(
        context(d.path(), None, "child", "call"),
        LocalInvocationId::fresh("connection"),
        "create_file",
        &json!({}),
        async {
            super::server::dispatch_at(d.path());
            returned(json!({"ok":false,"error":"private-refusal-canary"}))
        },
    )
    .await
    .unwrap();
    let r = read_store(d.path(), Limits::default());
    assert!(r.records.iter().all(|r| r.caller.principal.is_none()));
    assert!(r
        .records
        .iter()
        .any(|r| r.event == "result-observed" && r.outcome == "refused"));
    assert!(!serde_json::to_string(&r)
        .unwrap()
        .contains("private-refusal-canary"));
}
#[tokio::test]
async fn cancellation_after_effect_retains_the_write_and_missing_terminal() {
    let d = root();
    let path = d.path().join("partial.txt");
    let arguments = json!({"effect":"read","command":"private-command-canary"});
    let call = observe_call(
        context(d.path(), Some("root"), "child", "call"),
        LocalInvocationId::fresh("connection"),
        "run_command",
        &arguments,
        async {
            super::server::dispatch_at(d.path());
            crate::util::fs::atomic_write(&path, "partial").unwrap();
            std::future::pending::<Result<CallToolResult, rmcp::ErrorData>>().await
        },
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), call)
            .await
            .is_err()
    );
    let r = read_store(d.path(), Limits::default());
    assert!(r
        .records
        .iter()
        .any(|r| r.targets.iter().any(|t| t.phase == "committed")));
    assert!(r.records.iter().any(|r| r.outcome == "missing-terminal"));
    assert!(r
        .records
        .iter()
        .any(|r| r.diagnostics.iter().any(|d| d == "opaque-effects")));
    assert!(!serde_json::to_string(&r)
        .unwrap()
        .contains("private-command-canary"));
}
#[tokio::test]
async fn observer_failure_does_not_replace_completed_success() {
    let d = root();
    let path = d.path().join("written.txt");
    let result = observe_call(
        context(d.path(), Some("root"), "child", "call"),
        LocalInvocationId::fresh("connection"),
        "create_file",
        &json!({}),
        async {
            super::server::dispatch_at(d.path());
            crate::util::fs::atomic_write(&path, "written").unwrap();
            // Make result capture unavailable after the already committed user effect.
            std::fs::remove_file(d.path().join(".codescout/observations/v1/quota.lock")).unwrap();
            returned(json!("ok"))
        },
    )
    .await
    .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "written");
}
#[tokio::test]
async fn overlapping_dispatches_keep_context_connection_and_targets_separate() {
    let a = root();
    let b = root();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let run = |dir: std::path::PathBuf,
               owner: &str,
               connection: &str,
               barrier: std::sync::Arc<tokio::sync::Barrier>| {
        let owner = owner.to_owned();
        let connection = connection.to_owned();
        async move {
            observe_call(
                context(&dir, Some(&owner), "child", "same-native-call"),
                LocalInvocationId::new(&connection, 0),
                "create_file",
                &json!({}),
                async {
                    super::server::dispatch_at(&dir);
                    barrier.wait().await;
                    crate::util::fs::atomic_write(&dir.join("pinned.txt"), &owner).unwrap();
                    barrier.wait().await;
                    returned(json!("ok"))
                },
            )
            .await
            .unwrap()
        }
    };
    let _ = tokio::join!(
        run(
            a.path().to_path_buf(),
            "root-a",
            "connection-a",
            barrier.clone()
        ),
        run(b.path().to_path_buf(), "root-b", "connection-b", barrier)
    );
    let mut invocation_ids = std::collections::HashSet::new();
    for (dir, owner) in [(&a, "codex:root-a"), (&b, "codex:root-b")] {
        let r = read_store(dir.path(), Limits::default());
        assert!(r
            .records
            .iter()
            .all(|r| r.caller.principal.as_deref() == Some(owner)));
        assert!(r
            .records
            .iter()
            .all(|r| r.workspace == dir.path().to_str().unwrap()));
        for record in r.records {
            invocation_ids.insert(record.local_invocation);
        }
    }
    assert_eq!(invocation_ids.len(), 2);
}

#[tokio::test]
async fn pending_ack_and_false_ok_are_refusals_without_is_error() {
    let d = root();
    for (call, value) in [
        ("false-ok", json!({"ok":false})),
        ("ack", json!({"pending_ack":{"token":"private-ack-canary"}})),
    ] {
        observe_call(
            context(d.path(), Some("root"), "child", call),
            LocalInvocationId::fresh("connection"),
            "edit_file",
            &json!({}),
            async {
                super::server::dispatch_at(d.path());
                returned(value)
            },
        )
        .await
        .unwrap();
    }
    let r = read_store(d.path(), Limits::default());
    let results: Vec<_> = r
        .records
        .iter()
        .filter(|r| r.event == "result-observed")
        .collect();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.outcome == "refused"));
    assert!(!serde_json::to_string(&r)
        .unwrap()
        .contains("private-ack-canary"));
}
#[tokio::test]
async fn refusal_after_a_write_is_possible_partial_effect_not_pre_effect_refusal() {
    let d = root();
    observe_call(
        context(d.path(), Some("root"), "child", "partial-refusal"),
        LocalInvocationId::fresh("connection"),
        "edit_file",
        &json!({}),
        async {
            super::server::dispatch_at(d.path());
            crate::util::fs::atomic_write(&d.path().join("partial.txt"), "partial").unwrap();
            returned(json!({"ok":false,"error":"refused after partial work"}))
        },
    )
    .await
    .unwrap();
    let r = read_store(d.path(), Limits::default());
    assert!(r
        .records
        .iter()
        .any(|r| r.targets.iter().any(|t| t.phase == "committed")));
    assert!(r
        .records
        .iter()
        .any(|r| r.event == "result-observed" && r.outcome == "error-possible-effects"));
}
