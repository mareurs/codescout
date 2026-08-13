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
            "CODESCOUT_EMBEDDER_MODEL",
            "CODESCOUT_SPARSE_EMBEDDER_URL",
            "CODESCOUT_RERANKER_URL",
            "CODESCOUT_MODEL_DIM",
            "CODESCOUT_RETRIEVAL_PROFILE",
            "EMBED_API_KEY",
        ],
        || {
            let cfg = RetrievalConfig::from_env().expect("defaults");
            assert_eq!(cfg.qdrant_url, "http://127.0.0.1:6334");
            assert_eq!(
                cfg.embedder_url, None,
                "an unset url must mean 'resolve from the model', not 'assume 8081'"
            );
            assert_eq!(cfg.sparse_embedder_url, "http://127.0.0.1:8084");
            assert_eq!(cfg.reranker_url, "http://127.0.0.1:8083");
            assert_eq!(
                cfg.model_dim, None,
                "an unpinned dim must let the model decide"
            );
            assert_eq!(cfg.model, "local:AllMiniLML6V2Q");
            assert_eq!(cfg.api_key, None);
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
            ("CODESCOUT_EMBEDDER_MODEL", Some("local:BGESmallENV15")),
            ("CODESCOUT_SPARSE_EMBEDDER_URL", Some("http://eb-sparse:5")),
            ("CODESCOUT_RERANKER_URL", Some("http://rr:3")),
            ("CODESCOUT_MODEL_DIM", Some("4096")),
            ("CODESCOUT_RETRIEVAL_PROFILE", Some("gpu")),
            ("EMBED_API_KEY", Some("sk-test")),
        ],
        || {
            let cfg = RetrievalConfig::from_env().expect("overrides");
            assert_eq!(cfg.qdrant_url, "http://qd:1");
            assert_eq!(cfg.embedder_url.as_deref(), Some("http://eb:2"));
            assert_eq!(cfg.sparse_embedder_url, "http://eb-sparse:5");
            assert_eq!(cfg.model_dim, Some(4096));
            assert_eq!(cfg.model, "local:BGESmallENV15");
            assert_eq!(cfg.api_key.as_deref(), Some("sk-test"));
            assert_eq!(cfg.profile, "gpu");
        },
    );
}

#[test]
fn config_from_env_and_project_prefers_project_toml_when_env_silent() {
    // Composition-level regression for review round-1 mutation α: the
    // root -> ProjectConfig::load_or_default -> merge chain inside
    // `RetrievalConfig::from_env_and_project` must actually reach the
    // project's [embeddings] values, not just each merge field in isolation.
    //
    // Needs BOTH env-var families neutralized: CODESCOUT_EMBEDDER_* (what
    // RetrievalConfig itself reads) AND CODESCOUT_EMBED_MODEL/_URL (a
    // DIFFERENT, pre-existing family `ProjectConfig::load_or_default` applies
    // internally, unconditionally, before RetrievalConfig ever sees the
    // result) — this machine exports both, and a test that only controlled
    // the first family would silently read the second family's ambient value
    // instead of the project.toml's.
    temp_env::with_vars_unset(
        [
            "CODESCOUT_EMBEDDER_URL",
            "CODESCOUT_EMBEDDER_MODEL",
            "EMBED_API_KEY",
            "CODESCOUT_MODEL_DIM",
            "CODESCOUT_EMBED_MODEL",
            "CODESCOUT_EMBED_URL",
        ],
        || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
            std::fs::write(
                dir.path().join(".codescout/project.toml"),
                "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"local-dir:/weights\"\nurl = \"http://from-toml:9\"\n",
            )
            .unwrap();
            let cfg = RetrievalConfig::from_env_and_project(Some(dir.path())).expect("loads");
            assert_eq!(cfg.model, "local-dir:/weights");
            assert_eq!(cfg.embedder_url.as_deref(), Some("http://from-toml:9"));
        },
    );
}

#[test]
fn config_from_env_and_project_env_wins_over_project_toml() {
    // Same composition seam as above, but with CODESCOUT_EMBEDDER_* populated
    // too — a precedence inversion that only manifests once the real
    // `load_or_default` call is wired in (rather than in a pure
    // merge-function-level test alone) would slip past `merge_tests` in
    // src/retrieval/config.rs but not this one. No need to neutralize
    // CODESCOUT_EMBED_MODEL/_URL here: env must win regardless of what that
    // other layer resolves the project side to.
    temp_env::with_vars(
        [
            ("CODESCOUT_EMBEDDER_URL", Some("http://from-env:8")),
            ("CODESCOUT_EMBEDDER_MODEL", Some("local:BGESmallENV15")),
        ],
        || {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
            std::fs::write(
                dir.path().join(".codescout/project.toml"),
                "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"local-dir:/weights\"\nurl = \"http://from-toml:9\"\n",
            )
            .unwrap();
            let cfg = RetrievalConfig::from_env_and_project(Some(dir.path())).expect("loads");
            assert_eq!(cfg.model, "local:BGESmallENV15");
            assert_eq!(cfg.embedder_url.as_deref(), Some("http://from-env:8"));
        },
    );
}

#[cfg(feature = "server-stack")]
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
        // diff_chunks only reads chunk_id; file_path is irrelevant to these tests.
        file_path: String::new(),
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

#[cfg(feature = "server-stack")]
use codescout::retrieval::payload::{map_to_payload, payload_to_map, CodePayload};

#[cfg(feature = "server-stack")]
#[test]
fn payload_roundtrip_preserves_fields() {
    let p = CodePayload {
        project_id: "codescout".into(),
        file_path: "src/lib.rs".into(),
        language: "rust".into(),
        start_line: 10,
        end_line: 42,
        ast_header: "src/lib.rs :: fn main()".into(),
        content: "fn main() {}".into(),
        content_hash: "h1".into(),
        last_indexed_commit: "abc".into(),
        chunk_id: "id1".into(),
    };
    let map = payload_to_map(&p);
    let back = map_to_payload(&map).expect("decode");

    // Every field, not a sample. This test asserted 4 of 11 while its name claimed
    // all of them, so `ast_header` could round-trip as garbage — or not at all —
    // without failing anything.
    assert_eq!(back.project_id, p.project_id);
    assert_eq!(back.file_path, p.file_path);
    assert_eq!(back.language, p.language);
    assert_eq!(back.start_line, p.start_line);
    assert_eq!(back.end_line, p.end_line);
    assert_eq!(back.ast_header, p.ast_header);
    assert_eq!(back.content, p.content);
    assert_eq!(back.content_hash, p.content_hash);
    assert_eq!(back.last_indexed_commit, p.last_indexed_commit);
    assert_eq!(back.chunk_id, p.chunk_id);
}

/// `embed_text` is the single home for "what does a chunk look like to the embedder".
/// Before it existed, that decision was the residue of a struct literal in another
/// module, which is how the AST header stopped being embedded without a failing test.
///
/// Deliberately NOT gated behind `server-stack`. That feature is in neither `default`
/// nor any CI workflow, so every test carrying the gate — including
/// `payload_roundtrip_preserves_fields` right above — is never compiled and cannot
/// fail. `embed_text` and `CodePayload` are both ungated, so this one runs.
#[test]
fn embed_text_prepends_the_ast_header_and_omits_it_when_absent() {
    use codescout::retrieval::payload::{embed_text, CodePayload};

    let mut p = CodePayload {
        project_id: "codescout".into(),
        file_path: "src/lib.rs".into(),
        language: "rust".into(),
        start_line: 1,
        end_line: 1,
        ast_header: "src/lib.rs :: fn main()".into(),
        content: "fn main() {}".into(),
        content_hash: "h1".into(),
        last_indexed_commit: "abc".into(),
        chunk_id: "id1".into(),
    };

    assert_eq!(
        embed_text(&p),
        "src/lib.rs :: fn main()\nfn main() {}",
        "the header is a newline-separated prefix, and the body is preserved verbatim"
    );

    // Markdown and plain text have no AST header; they must embed as bare content
    // rather than as a leading blank line.
    p.ast_header = String::new();
    assert_eq!(embed_text(&p), "fn main() {}");
}
