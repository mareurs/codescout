#![cfg(target_os = "linux")]
use codescout::observations::store::*;
use serde_json::{json, Value};
#[test]
fn capture_has_diagnostic_time_without_reusing_a_request_timestamp() {
    let d = tempfile::tempdir().unwrap();
    git2::Repository::init(d.path()).unwrap();
    enable(d.path()).unwrap();
    let StoreAvailability::Enabled(s) =
        ObservationStore::open_if_enabled(d.path(), Limits::default())
    else {
        panic!("store unavailable");
    };
    let event:SanitizedEvent=serde_json::from_value(json!({
        "schema_version":1,"event":"hook-pre","producer_kind":"codex-hook",
        "workspace":d.path(),"caller":{"harness":"codex","principal":"codex:root","thread_id":null,"agent_id":null},
        "tool":"create_file","local_invocation":null,"native_call_id":"call","turn_id":"turn",
        "fingerprints":[],"targets":[],"outcome":"unknown","diagnostics":[]
    })).unwrap();
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let receipt = s.append(event).unwrap();
    let value: Value = serde_json::to_value(receipt.observation).unwrap();
    assert!(value["observed_at_unix_ms"]
        .as_u64()
        .is_some_and(|ms| ms >= before));
}
