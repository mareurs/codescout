//! codescout-embed — shared embedding primitives.

pub mod chunker;

mod embedder;

#[cfg(feature = "local-embed")]
pub mod local;

#[cfg(feature = "remote-embed")]
pub mod remote;

#[cfg(any(test, feature = "test-mock"))]
pub mod mock;

pub use chunker::{chunk_markdown, split, split_markdown, RawChunk};
pub use embedder::{Embedder, Embedding};

use anyhow::Result;

/// Returns the chunk size in characters appropriate for the given model spec.
///
/// Derived from each model's documented maximum sequence length using a
/// conservative formula: `max_tokens × 0.85 × 3 chars/token`.
///
/// - The 0.85 factor leaves 15 % headroom for tokenisation variance and
///   control tokens (BOS/EOS).
/// - Code tokenises at roughly 3–4 chars/token; 3 is the conservative lower
///   bound, ensuring chunks stay within the context window even for files with
///   many short identifiers and operators.
///
/// Unknown or custom models fall back to 512 tokens (the most common context
/// window among small embedding models). This is intentionally conservative —
/// chunks will be smaller than necessary but will never be truncated.
///
/// This value is not user-configurable. It is derived from the model spec
/// so that users cannot accidentally misconfigure it.
pub fn chunk_size_for_model(model_spec: &str) -> usize {
    // 85 % of context × 3 chars/token.
    fn from_tokens(n: usize) -> usize {
        (n as f64 * 0.85 * 3.0) as usize
    }

    // Map well-known model name substrings to their published max sequence
    // lengths. Matching is done on the bare model name (prefix stripped) so
    // that "ollama:nomic-embed-text" and "openai:nomic-embed-text" both match.
    fn tokens_for_bare(name: &str) -> usize {
        let l = name.to_lowercase();
        // 8 192-token models
        if l.contains("nomic-embed") || l.contains("jina") || l.contains("bge-m3") {
            return 8192;
        }
        // OpenAI text-embedding-3-* and text-embedding-ada-002
        if l.starts_with("text-embedding-") {
            return 8191;
        }
        // mxbai-embed-large (MixedBread)
        if l.contains("mxbai") {
            return 512;
        }
        // BGE Small variants
        if l.contains("bge-small") || l.starts_with("bge_small") {
            return 512;
        }
        // all-MiniLM-L6-v2
        if l.contains("all-minilm") || l.contains("minilm-l6") {
            return 256;
        }
        // Unknown — conservative fallback
        512
    }

    // Local fastembed models use their documented sequence lengths.
    // These are listed here rather than in local.rs to avoid a feature-gate
    // dependency (local.rs is #[cfg(feature = "local-embed")]).
    if let Some(local_name) = model_spec.strip_prefix("local:") {
        let max_tokens = match local_name.to_lowercase().as_str() {
            "nomicembedtextv15" | "nomicembedtextv15q" => 8192,
            "jinaembeddingsv2basecode" => 8192,
            "bgesmallenv15q" | "bgesmallenv15" => 512,
            "allminilml6v2q" | "allminilml6v2" => 256,
            _ => 512,
        };
        return from_tokens(max_tokens);
    }

    // Strip backend prefix to get the bare model name.
    let bare = model_spec
        .strip_prefix("ollama:")
        .or_else(|| model_spec.strip_prefix("openai:"))
        .or_else(|| {
            // "custom:model-name@base_url" — extract only the model-name part
            model_spec
                .strip_prefix("custom:")
                .map(|rest| rest.split('@').next().unwrap_or(rest))
        })
        .unwrap_or(model_spec);

    from_tokens(tokens_for_bare(bare))
}

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
/// 1. `url` set → RemoteEmbedder targeting that URL
/// 2. `model` starts with `local:` → local ONNX via fastembed
/// 3. `model` starts with `ollama:` → Ollama (errors loudly if unreachable)
/// 4. `model` starts with `openai:` → OpenAI API
/// 5. `model` starts with `custom:` → hard error with migration hint
/// 6. No url, no prefix → default to local:AllMiniLML6V2Q
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
            let dims: usize = dims_str
                .parse()
                .map_err(|_| anyhow::anyhow!("mock: URL requires numeric dim suffix, got '{url}'"))?;
            return Ok(Box::new(mock::MockEmbedder::new(dims)));
        }
    }

    // 1. URL takes priority — any OpenAI-compatible endpoint
    #[cfg(feature = "remote-embed")]
    if let Some(url) = url {
        // Strip known routing prefixes so "ollama:nomic-embed-text" + url
        // sends "nomic-embed-text" as the model name in the HTTP request.
        let bare_model = model
            .strip_prefix("ollama:")
            .or_else(|| model.strip_prefix("openai:"))
            .or_else(|| model.strip_prefix("local:"))
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

    // 2. local: prefix
    #[cfg(feature = "local-embed")]
    if let Some(model_id) = model.strip_prefix("local:") {
        return Ok(Box::new(local::LocalEmbedder::new(model_id).await?));
    }

    // 3. ollama: prefix — no fallback, errors if unreachable
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("ollama:") {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        if let Err(e) = remote::probe_ollama(&host).await {
            anyhow::bail!(
                "Ollama is not reachable at {host}: {e}\n\
                 Start Ollama or switch to a different embedding backend.\n\n\
                 Options:\n\
                 • url = \"http://your-server:port/v1\"    (any OpenAI-compatible endpoint)\n\
                 • model = \"local:AllMiniLML6V2Q\"        (bundled ONNX, 22MB, no server needed)"
            );
        }
        return Ok(Box::new(remote::RemoteEmbedder::ollama(model_id)?));
    }

    // 4. openai: prefix
    #[cfg(feature = "remote-embed")]
    if let Some(model_id) = model.strip_prefix("openai:") {
        return Ok(Box::new(remote::RemoteEmbedder::openai(model_id, api_key)?));
    }

    // 5. custom: prefix — removed, hard error
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

    // 6. No prefix — try as local model name
    #[cfg(feature = "local-embed")]
    {
        // Try parsing as a local model name directly
        if local::LocalEmbedder::new(model).await.is_ok() {
            return Ok(Box::new(local::LocalEmbedder::new(model).await?));
        }
    }

    // Helpful error for local: prefix without the feature
    if model.starts_with("local:") {
        anyhow::bail!(
            "Local embedding requires the 'local-embed' feature.\n\
             Rebuild with: cargo build --features local-embed\n\n\
             Recommended: local:AllMiniLML6V2Q (384d, quantized, 22MB)"
        );
    }

    anyhow::bail!(
        "Unknown model '{}'. Options:\n\
         • Set url in [embeddings] to point at any OpenAI-compatible server\n\
         • Use local:AllMiniLML6V2Q for bundled ONNX (384d, 22MB, no server needed)\n\
         • Use local:JinaEmbeddingsV2BaseCode for code-specialized ONNX",
        model
    )
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
