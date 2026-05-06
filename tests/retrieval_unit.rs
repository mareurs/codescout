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
