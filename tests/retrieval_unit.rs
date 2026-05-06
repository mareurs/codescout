use codescout::retrieval::config::RetrievalConfig;

#[test]
fn config_from_env_uses_defaults_when_unset() {
    std::env::remove_var("CODESCOUT_QDRANT_URL");
    std::env::remove_var("CODESCOUT_EMBEDDER_URL");
    std::env::remove_var("CODESCOUT_RERANKER_URL");
    std::env::remove_var("CODESCOUT_MODEL_DIM");
    std::env::remove_var("CODESCOUT_RETRIEVAL_PROFILE");

    let cfg = RetrievalConfig::from_env().expect("defaults");
    assert_eq!(cfg.qdrant_url, "http://127.0.0.1:6333");
    assert_eq!(cfg.embedder_url, "http://127.0.0.1:8080");
    assert_eq!(cfg.reranker_url, "http://127.0.0.1:8081");
    assert_eq!(cfg.model_dim, 1024);
    assert_eq!(cfg.profile, "cpu");
}

#[test]
fn config_from_env_reads_overrides() {
    std::env::set_var("CODESCOUT_QDRANT_URL", "http://qd:1");
    std::env::set_var("CODESCOUT_EMBEDDER_URL", "http://eb:2");
    std::env::set_var("CODESCOUT_RERANKER_URL", "http://rr:3");
    std::env::set_var("CODESCOUT_MODEL_DIM", "4096");
    std::env::set_var("CODESCOUT_RETRIEVAL_PROFILE", "gpu");

    let cfg = RetrievalConfig::from_env().expect("overrides");
    assert_eq!(cfg.qdrant_url, "http://qd:1");
    assert_eq!(cfg.model_dim, 4096);
    assert_eq!(cfg.profile, "gpu");

    for k in ["CODESCOUT_QDRANT_URL","CODESCOUT_EMBEDDER_URL","CODESCOUT_RERANKER_URL",
              "CODESCOUT_MODEL_DIM","CODESCOUT_RETRIEVAL_PROFILE"] {
        std::env::remove_var(k);
    }
}

#[test]
fn client_from_env_constructs_when_urls_present() {
    std::env::set_var("CODESCOUT_QDRANT_URL", "http://127.0.0.1:6333");
    std::env::set_var("CODESCOUT_EMBEDDER_URL", "http://127.0.0.1:8080");
    std::env::set_var("CODESCOUT_RERANKER_URL", "http://127.0.0.1:8081");
    let cfg = codescout::retrieval::config::RetrievalConfig::from_env().unwrap();
    let _ = codescout::retrieval::client::RetrievalClient::from_config_only(cfg);
    // doesn't connect — just constructs
    for k in ["CODESCOUT_QDRANT_URL","CODESCOUT_EMBEDDER_URL","CODESCOUT_RERANKER_URL"] {
        std::env::remove_var(k);
    }
}

use codescout::retrieval::drift::{diff_chunks, ChunkRef};

fn cr(id: &str, hash: &str) -> ChunkRef {
    ChunkRef { chunk_id: id.into(), content_hash: hash.into() }
}

#[test]
fn diff_identical_yields_noop() {
    let server = vec![cr("a","h1"), cr("b","h2")];
    let local = vec![cr("a","h1"), cr("b","h2")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_added_chunk_yields_upsert() {
    let server = vec![cr("a","h1")];
    let local = vec![cr("a","h1"), cr("b","h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["b".to_string()]);
    assert!(d.to_delete.is_empty());
}

#[test]
fn diff_deleted_chunk_yields_delete() {
    let server = vec![cr("a","h1"), cr("b","h2")];
    let local = vec![cr("a","h1")];
    let d = diff_chunks(&server, &local);
    assert!(d.to_upsert.is_empty());
    assert_eq!(d.to_delete, vec!["b".to_string()]);
}

#[test]
fn diff_modified_chunk_yields_upsert_for_new_id() {
    let server = vec![cr("a-old","h1")];
    let local = vec![cr("a-new","h2")];
    let d = diff_chunks(&server, &local);
    assert_eq!(d.to_upsert, vec!["a-new".to_string()]);
    assert_eq!(d.to_delete, vec!["a-old".to_string()]);
}
