use super::contract::{canonical_json, fingerprint, validate_observation};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[test]
fn canonical_vectors() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/operation-observations-v1.json"
    ))
    .unwrap();
    for c in corpus["canonical"].as_array().unwrap() {
        let want = c["canonical"].as_str().unwrap();
        assert_eq!(canonical_json(&c["input"]).unwrap(), want);
        assert_eq!(
            fingerprint(&c["input"]).unwrap().digest,
            hex::encode(Sha256::digest(want.as_bytes()))
        );
    }
    assert!(canonical_json(&json!(1.5)).is_err());
    assert!(canonical_json(&json!(9007199254740992_u64)).is_err());
}

#[test]
fn record_validation_is_strict_and_root_is_not_thread() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/operation-observations-v1.json"
    ))
    .unwrap();
    let record = corpus["record"].clone();
    assert_eq!(
        serde_json::to_value(validate_observation(record.clone()).unwrap()).unwrap(),
        record
    );
    for (key, value) in [
        ("schema_version", json!(2)),
        ("sequence", json!(-1)),
        ("body", json!("canary")),
        ("native_call_id", json!("\0")),
        ("tool", json!("credential-canary")),
    ] {
        let mut bad = record.clone();
        bad[key] = value;
        assert!(validate_observation(bad).is_err());
    }
    let mut unknown = record.clone();
    unknown["caller"]["principal"] = Value::Null;
    unknown["caller"]["thread_id"] = json!("child");
    assert!(validate_observation(unknown.clone()).is_ok());
    unknown["caller"]["principal"] = json!("child");
    assert!(validate_observation(unknown).is_err());
}

#[test]
fn unknown_harness_has_no_principal_or_hook_origin() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/operation-observations-v1.json"
    ))
    .unwrap();
    let mut r = corpus["record"].clone();
    r["caller"] = json!({"harness":"unknown","principal":null,"thread_id":null,"agent_id":null});
    assert!(validate_observation(r.clone()).is_ok());
    r["producer_kind"] = json!("codex-hook");
    assert!(validate_observation(r.clone()).is_err());
    r["producer_kind"] = json!("codescout-server");
    r["caller"]["principal"] = json!("codex:root");
    assert!(validate_observation(r).is_err());
}
#[test]
fn native_cwd_is_optional_bounded_and_separate() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/operation-observations-v1.json"
    ))
    .unwrap();
    for (cwd, valid) in [
        (Value::Null, true),
        (json!("/repo/src/../nested"), true),
        (json!("relative/path"), false),
        (json!("/bad\0cwd"), false),
        (json!(format!("/{}", "a".repeat(4096))), false),
        (json!(42), false),
    ] {
        let mut record = corpus["record"].clone();
        record["native_cwd"] = cwd.clone();
        let decoded = validate_observation(record);
        assert_eq!(decoded.is_ok(), valid, "{cwd}");
        if let Ok(record) = decoded {
            assert_eq!(record.native_cwd, cwd.as_str().map(str::to_owned));
            assert_eq!(
                record.workspace,
                corpus["record"]["workspace"].as_str().unwrap()
            );
        }
    }
}
