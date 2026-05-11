//! codescout-embed — shared embedding primitives.

pub mod chunker;

mod embedder;

#[cfg(feature = "remote-embed")]
pub mod remote;

#[cfg(any(test, feature = "test-mock"))]
pub mod mock;

pub use chunker::{chunk_markdown, split, split_markdown, RawChunk};
pub use embedder::{Embedder, Embedding};

use anyhow::Result;

/// Default per-chunk size in characters when the user does not override via
/// `[embeddings] chunk_size` in project.toml.
///
/// 1600 chars was selected as the sweet spot in the 20-query benchmark
/// (docs/research/2026-04-03-embedding-model-benchmark.md). It keeps methods
/// up to ~40-45 lines whole, avoids "kitchen sink" multi-concept averaging
/// from larger chunks, and preserves enough surface area for multi-keyword
/// queries.
///
/// See spec docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md
/// for the rationale.
pub const DEFAULT_CHUNK_SIZE_CHARS: usize = 1600;

/// Convenience extension for embedding a single query text.
///
/// Uses `embed_query` so model-specific query prefixes are applied automatically.
pub async fn embed_one(embedder: &dyn Embedder, text: &str) -> Result<Embedding> {
    embedder.embed_query(text).await
}

/// Create an embedder using explicit config fields.
///
/// Resolution order:
/// 0. (test/test-mock only) `url` = "mock:DIM" → in-memory MockEmbedder
/// 1. URL set → RemoteEmbedder targeting that URL
/// 2. `model` starts with `ollama:` → Ollama (errors loudly if unreachable)
/// 3. `model` starts with `openai:` → OpenAI API
/// 4. `model` starts with `custom:` → hard error with migration hint
/// 5. Otherwise → hard error pointing user at docker setup docs
pub async fn create_embedder_with_config(
    model: &str,
    url: Option<&str>,
    api_key: Option<String>,
) -> Result<Box<dyn Embedder>> {
    // Suppress unused-variable warning when remote-embed feature is disabled.
    #[cfg(not(feature = "remote-embed"))]
    let _ = &api_key;

    #[cfg(any(test, feature = "test-mock"))]
    if let Some(url) = url {
        if let Some(dims_str) = url.strip_prefix("mock:") {
            let dims: usize = dims_str.parse().map_err(|_| {
                anyhow::anyhow!("mock: URL requires numeric dim suffix, got '{url}'")
            })?;
            return Ok(Box::new(mock::MockEmbedder::new(dims)));
        }
    }

    // URL takes priority — any OpenAI-compatible endpoint
    #[cfg(feature = "remote-embed")]
    if let Some(url) = url {
        // Strip known routing prefixes so "ollama:nomic-embed-text" + url
        // sends "nomic-embed-text" as the model name in the HTTP request.
        let bare_model = model
            .strip_prefix("ollama:")
            .or_else(|| model.strip_prefix("openai:"))
            .unwrap_or(model);
        return Ok(Box::new(remote::RemoteEmbedder::from_url(
            url, bare_model, api_key,
        )?));
    }
    #[cfg(not(feature = "remote-embed"))]
    if url.is_some() {
        anyhow::bail!(
            "Remote embedding requires the 'remote-embed' feature.\n\
             Rebuild with: cargo build --features remote-embed"
        );
    }

    // ollama: prefix — no fallback, errors if unreachable
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("ollama:") {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        if let Err(e) = remote::probe_ollama(&host).await {
            anyhow::bail!(
                "Ollama is not reachable at {host}: {e}\n\
                 Start Ollama or switch to a different embedding backend.\n\n\
                 Options:\n\
                 • url = \"http://your-server:port/v1\"    (any OpenAI-compatible endpoint)"
            );
        }
        return Ok(Box::new(remote::RemoteEmbedder::ollama(model_id)?));
    }

    // openai: prefix
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("openai:") {
        return Ok(Box::new(remote::RemoteEmbedder::openai(model_id, api_key)?));
    }

    // custom: prefix — removed, hard error
    #[cfg(feature = "remote-embed")]
    if model.starts_with("custom:") {
        anyhow::bail!(
            "The custom: prefix has been removed.\n\
             Use the url and model fields in [embeddings] instead.\n\n\
             Example .codescout/project.toml:\n\
             [embeddings]\n\
             model = \"your-model-name\"\n\
             url = \"http://your-server:port/v1\""
        );
    }

    // No URL and no recognised prefix — guide the user to set up a backend.
    anyhow::bail!(
        "Embedding backend not configured.\n\
         \n\
         codescout requires a remote embedding service (Ollama, llama-server, \
         or any OpenAI-compatible endpoint). The local fastembed backend has \
         been removed in v{}.\n\
         \n\
         Set in .codescout/project.toml:\n\
         \n\
         [embeddings]\n\
         model = \"nomic-embed-text\"\n\
         url   = \"http://localhost:11434/v1\"\n\
         \n\
         Suggested docker image: ollama/ollama (https://hub.docker.com/r/ollama/ollama).\n\
         Setup guide: https://github.com/mareurs/codescout/blob/master/docs/embedding-setup.md",
        env!("CARGO_PKG_VERSION"),
    );
}

/// Create an embedder from a model string (legacy interface).
///
/// Delegates to `create_embedder_with_config` with no URL. Existing callers
/// that only have a model string continue to work unchanged.
pub async fn create_embedder(model: &str) -> Result<Box<dyn Embedder>> {
    create_embedder_with_config(model, None, None).await
}

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}

#[cfg(all(test, feature = "test-mock"))]
mod mock_factory_tests {
    use super::*;

    #[tokio::test]
    async fn factory_returns_mock_embedder_when_url_uses_mock_scheme() {
        let e = create_embedder_with_config("ignored", Some("mock:32"), None)
            .await
            .expect("mock factory must succeed");
        assert_eq!(e.dimensions(), 32);
    }
}
