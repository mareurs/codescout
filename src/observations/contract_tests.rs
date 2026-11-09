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
