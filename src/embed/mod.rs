//! Embedding engine: semantic code search via local or remote embeddings.
//!
//! Inspired by cocoindex-code (../cocoindex-code/) but implemented natively
//! in Rust with sqlite-vec for zero-dependency vector storage.
//!
//! Architecture:
//!   chunker → Embedder trait → sqlite-vec index
//!
//! Two Embedder backends:
//!   - LocalEmbedder  (fastembed/ONNX, feature "local-embed") — fully offline, CPU/WSL2-friendly
//!   - RemoteEmbedder (reqwest, feature "remote-embed")   — OpenAI-compatible API

pub mod ast_chunker;
pub use codescout_embed::chunker;
pub use codescout_embed::{chunk_markdown, split, split_markdown, RawChunk};
pub mod drift;
pub mod index;
pub mod preflight;
pub mod schema;

#[cfg(feature = "remote-embed")]
pub use codescout_embed::remote;

#[cfg(feature = "local-embed")]
pub use codescout_embed::local;

pub use codescout_embed::{
    chunk_size_for_model, create_embedder, create_embedder_with_config, embed_one,
};
pub use codescout_embed::{Embedder, Embedding};

#[cfg(test)]
mod tests {

    #[test]
    fn unknown_prefix_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("bogus:model"));
        let err = result.err().expect("expected an error");
        assert!(
            err.to_string().contains("Unknown model"),
            "unexpected error: {}",
            err
        );
    }

    #[cfg(not(feature = "local-embed"))]
    #[test]
    fn local_prefix_returns_helpful_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("local:anything"));
        let err = result.err().expect("expected an error");
        assert!(err.to_string().contains("local-embed"));
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn custom_prefix_missing_at_sign_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("custom:no-at-sign"));
        let err = result.err().expect("expected an error");
        // custom: prefix is now removed — error should be the migration hint
        assert!(
            err.to_string().contains("removed"),
            "unexpected error: {}",
            err
        );
        assert!(
            err.to_string().contains("url"),
            "error should mention url field: {}",
            err
        );
    }

    // ---------- chunk_size_for_model ----------

    #[test]
    fn chunk_size_mxbai_embed_large() {
        // Default model: 512-token context. Formula: 512 × 0.85 × 3 = 1305.
        let sz = super::chunk_size_for_model("ollama:mxbai-embed-large");
        assert_eq!(sz, 1305);
    }

    #[test]
    fn chunk_size_nomic_embed_text() {
        // 8 192-token context. Formula: 8192 × 0.85 × 3 = 20 889.
        let sz = super::chunk_size_for_model("ollama:nomic-embed-text");
        assert_eq!(sz, 20889);
    }

    #[test]
    fn chunk_size_bge_m3() {
        // bge-m3 has 8192-token context. Formula: 8192 × 0.85 × 3 = 20889.
        let sz = super::chunk_size_for_model("ollama:bge-m3");
        assert_eq!(sz, 20889);
    }

    #[test]
    fn chunk_size_openai_text_embedding_3_small() {
        let sz = super::chunk_size_for_model("openai:text-embedding-3-small");
        assert_eq!(sz, 20887); // 8191 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_jina() {
        let sz = super::chunk_size_for_model("local:JinaEmbeddingsV2BaseCode");
        assert_eq!(sz, 20889); // 8192 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_bge_small() {
        let sz = super::chunk_size_for_model("local:BGESmallENV15Q");
        assert_eq!(sz, 1305); // 512 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_all_minilm() {
        let sz = super::chunk_size_for_model("local:AllMiniLML6V2Q");
        assert_eq!(sz, 652); // 256 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_nomic_v15() {
        let sz = super::chunk_size_for_model("local:NomicEmbedTextV15Q");
        assert_eq!(sz, 20889); // 8192 × 0.85 × 3
    }

    #[test]
    fn chunk_size_local_nomic_v15_full() {
        let sz = super::chunk_size_for_model("local:NomicEmbedTextV15");
        assert_eq!(sz, 20889); // 8192 × 0.85 × 3
    }

    #[test]
    fn chunk_size_custom_model() {
        // custom: prefix with @url — model name extracted before @
        let sz = super::chunk_size_for_model("custom:mxbai-embed-large@http://localhost:1234");
        assert_eq!(sz, 1305);
    }

    #[test]
    fn chunk_size_unknown_model_falls_back_to_512_tokens() {
        let sz = super::chunk_size_for_model("ollama:some-unknown-model");
        assert_eq!(sz, 1305); // 512 × 0.85 × 3
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn create_embedder_with_url_uses_remote() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // We can't actually connect, but we can verify it doesn't error on creation
        // by checking that the url path is taken (not the prefix path).
        // Use a model name with no prefix — if url is respected, it won't hit
        // the "Unknown model prefix" error.
        let result = rt.block_on(super::create_embedder_with_config(
            "nomic-embed-text-v1.5",
            Some("http://127.0.0.1:99999"),
            None,
        ));
        // Should succeed (RemoteEmbedder created) — it only fails when we try to embed
        assert!(
            result.is_ok(),
            "url should create RemoteEmbedder without prefix: {:?}",
            result.err()
        );
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn custom_prefix_returns_migration_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("custom:model@http://localhost:1234"));
        let err = result.err().expect("custom: should error");
        assert!(
            err.to_string().contains("removed"),
            "error should say prefix is removed: {}",
            err
        );
        assert!(
            err.to_string().contains("url"),
            "error should mention url field: {}",
            err
        );
    }

    #[test]
    fn create_embedder_no_url_no_prefix_defaults_to_local_allminilm() {
        // A bare model name with no url should be accepted as a local model
        // when the local-embed feature is available.
        #[cfg(feature = "local-embed")]
        {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(super::create_embedder("AllMiniLML6V2Q"));
            assert!(result.is_ok(), "AllMiniLML6V2Q should load as local model");
        }
    }

    #[test]
    fn chunk_size_bare_nomic_model_name() {
        // When url is set, model has no prefix — just the bare name.
        // This test documents that bare model names work correctly.
        let sz = super::chunk_size_for_model("nomic-embed-text-v1.5");
        assert_eq!(sz, 20889); // 8192 × 0.85 × 3
    }

    #[test]
    fn chunk_size_bare_unknown_model() {
        // When url is set, custom model names with no prefix fall back to
        // the conservative 512-token default.
        let sz = super::chunk_size_for_model("some-custom-model");
        assert_eq!(sz, 1305); // 512 × 0.85 × 3 (conservative fallback)
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn ollama_prefix_without_server_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        // Use a port that's definitely not running Ollama
        unsafe { std::env::set_var("OLLAMA_HOST", "http://127.0.0.1:19999") };
        let result = rt.block_on(super::create_embedder_with_config(
            "ollama:nomic-embed-text",
            None,
            None,
        ));
        unsafe { std::env::remove_var("OLLAMA_HOST") };
        assert!(result.is_err(), "should error when Ollama is unreachable");
        let err = result.err().unwrap().to_string();
        assert!(
            !err.contains("Falling back"),
            "should NOT mention fallback: {err}"
        );
        assert!(
            err.contains("not reachable") || err.contains("Ollama"),
            "should mention Ollama is unreachable: {err}"
        );
    }

    #[test]
    fn url_with_ollama_prefix_strips_prefix() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder_with_config(
            "ollama:nomic-embed-text",
            Some("http://localhost:11434/v1"),
            None,
        ));
        // Should succeed via URL path, not the ollama: branch
        assert!(result.is_ok(), "url+ollama: prefix should use URL path");
    }
}
