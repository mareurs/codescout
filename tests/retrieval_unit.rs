use codescout::retrieval::config::RetrievalConfig;

#[test]
fn config_from_env_uses_defaults_when_unset() {
    // Use temp_env::with_vars_unset to isolate this test's env-var assumptions
    // from concurrent tests in other binaries. #[serial_test::serial] only
    // serializes within a single test binary's parallel scheduler, so it does
    // not prevent cross-binary CODESCOUT_* writes that occur during a full
    // `cargo test` run (BUG: 2026-05-24-ci-retrieval-env-test-cross-binary-flake).
    temp_env::with_vars_unset(
        [
            "CODESCOUT_QDRANT_URL",
            "CODESCOUT_EMBEDDER_URL",
            "CODESCOUT_SPARSE_EMBEDDER_URL",
            "CODESCOUT_RERANKER_URL",
            "CODESCOUT_MODEL_DIM",
            "CODESCOUT_RETRIEVAL_PROFILE",
        ],
        || {
            let cfg = RetrievalConfig::from_env().expect("defaults");
            assert_eq!(cfg.qdrant_url, "http://127.0.0.1:6334");
            assert_eq!(cfg.embedder_url, "http://127.0.0.1:8081");
            assert_eq!(cfg.sparse_embedder_url, "http://127.0.0.1:8084");
            assert_eq!(cfg.reranker_url, "http://127.0.0.1:8083");
            assert_eq!(cfg.model_dim, 768);
            assert_eq!(cfg.profile, "cpu");
        },
    );
}

#[test]
fn config_from_env_reads_overrides() {
    // See config_from_env_uses_defaults_when_unset above for the cross-binary
    // serial_test inadequacy rationale.
    temp_env::with_vars(
        [
            ("CODESCOUT_QDRANT_URL", Some("http://qd:1")),
            ("CODESCOUT_EMBEDDER_URL", Some("http://eb:2")),
            ("CODESCOUT_SPARSE_EMBEDDER_URL", Some("http://eb-sparse:5")),
            ("CODESCOUT_RERANKER_URL", Some("http://rr:3")),
            ("CODESCOUT_MODEL_DIM", Some("4096")),
            ("CODESCOUT_RETRIEVAL_PROFILE", Some("gpu")),
        ],
        || {
            let cfg = RetrievalConfig::from_env().expect("overrides");
            assert_eq!(cfg.qdrant_url, "http://qd:1");
            assert_eq!(cfg.sparse_embedder_url, "http://eb-sparse:5");
            assert_eq!(cfg.model_dim, 4096);
            assert_eq!(cfg.profile, "gpu");
        },
    );
}

#[test]

fn client_from_env_constructs_when_urls_present() {
    std::env::set_var("CODESCOUT_QDRANT_URL", "http://127.0.0.1:6334");
    std::env::set_var("CODESCOUT_EMBEDDER_URL", "http://127.0.0.1:8081");
    std::env::set_var("CODESCOUT_SPARSE_EMBEDDER_URL", "http://127.0.0.1:8084");
    std::env::set_var("CODESCOUT_RERANKER_URL", "http://127.0.0.1:8083");
    let cfg = codescout::retrieval::config::RetrievalConfig::from_env().unwrap();
    let _ = codescout::retrieval::client::RetrievalClient::from_config_only(cfg);
    // doesn't connect — just constructs
    for k in [
        "CODESCOUT_QDRANT_URL",
        "CODESCOUT_EMBEDDER_URL",
        "CODESCOUT_SPARSE_EMBEDDER_URL",
        "CODESCOUT_RERANKER_URL",
    ] {
        std::env::remove_var(k);
    }
}

use codescout::retrieval::drift::{diff_chunks, ChunkRef};

fn cr(id: &str, hash: &str) -> ChunkRef {
    ChunkRef {
        chunk_id: id.into(),
        content_hash: hash.into(),
    }
}

#[test]
fn diff_identical_yields_noop() {
    let server = vec![cr("a", "h1"), cr("b", "h2")];
    let local = vec![cr("a", "h1"), cr("b", "h2")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_added_chunk_yields_upsert() {
    let server = vec![cr("a", "h1")];
    let local = vec![cr("a", "h1"), cr("b", "h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["b".to_string()]);
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_deleted_chunk_yields_delete() {
    let server = vec![cr("a", "h1"), cr("b", "h2")];
    let local = vec![cr("a", "h1")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert_eq!(d.to_delete, vec!["b".to_string()]);
}

#[test]
fn diff_modified_chunk_yields_upsert_for_new_id() {
    let server = vec![cr("a-old", "h1")];
    let local = vec![cr("a-new", "h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["a-new".to_string()]);
    assert_eq!(d.to_delete, vec!["a-old".to_string()]);
}

use codescout::retrieval::payload::{map_to_payload, payload_to_map, CodePayload};

#[test]
fn payload_roundtrip_preserves_fields() {
    let p = CodePayload {
        project_id: "codescout".into(),
        file_path: "src/lib.rs".into(),
        language: "rust".into(),
        start_line: 10,
        end_line: 42,
        ast_kind: "fn".into(),
        ast_header: "fn main()".into(),
        content: "fn main() {}".into(),
        content_hash: "h1".into(),
        last_indexed_commit: "abc".into(),
        chunk_id: "id1".into(),
    };
    let map = payload_to_map(&p);
    let back = map_to_payload(&map).expect("decode");
    assert_eq!(back.project_id, p.project_id);
    assert_eq!(back.start_line, p.start_line);
    assert_eq!(back.content_hash, p.content_hash);
    assert_eq!(back.file_path, p.file_path);
}
