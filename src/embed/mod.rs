//! Embedding engine: semantic code search via local or remote embeddings.
//!
//! Inspired by cocoindex-code (../cocoindex-code/) but implemented natively
//! in Rust with sqlite-vec for zero-dependency vector storage.
//!
//! Architecture:
//!   chunker → Embedder trait → sqlite-vec index
//!
//! Embedder backend:
//!   - RemoteEmbedder (reqwest, feature "remote-embed")   — OpenAI-compatible API (Ollama, llama-server, OpenAI, etc.)

pub mod ast_chunker;
pub mod bm25;
pub mod fusion;

pub use codescout_embed::chunker;
pub use codescout_embed::{chunk_markdown, split, split_markdown, RawChunk};
pub mod drift;
pub mod index;
pub mod preflight;
pub mod schema;

#[cfg(feature = "remote-embed")]
pub use codescout_embed::remote;

pub use codescout_embed::{
    create_embedder, create_embedder_with_config, embed_one, DEFAULT_CHUNK_SIZE_CHARS,
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
            err.to_string().contains("Embedding backend not configured"),
            "unexpected error: {}",
            err
        );
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
