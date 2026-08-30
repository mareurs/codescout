//! codescout-embed — shared embedding primitives.

pub mod chunker;

mod embedder;

// The two ONNX backends are mutually exclusive: `local-embed` statically links a
// prebuilt ONNX Runtime; `local-embed-dynamic` dlopens onnxruntime.dll at runtime
// (windows-gnu). Enabling both hands `ort` conflicting backend features
// (ort-download-binaries + ort-load-dynamic) → a cryptic link error. Fail loud.
#[cfg(all(feature = "local-embed", feature = "local-embed-dynamic"))]
compile_error!(
    "features `local-embed` and `local-embed-dynamic` are mutually exclusive — \
     pick exactly one ONNX backend (static `local-embed` vs runtime-loaded `local-embed-dynamic`)"
);

#[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
pub mod local;

#[cfg(feature = "remote-embed")]
pub mod remote;

pub use chunker::{chunk_markdown, split, split_markdown, RawChunk};
pub use embedder::{EmbedError, Embedder, Embedding, CONNECT_FAILED_MARKER};

use anyhow::Result;

/// Strip a trailing `/`, then a trailing `/v1/embeddings` or `/v1`, from an
/// embedder base url — the one piece of URL-shape recognition shared by every
/// caller that accepts either a bare host (`http://host:port`) or an
/// already-`/v1`-suffixed API base (`http://host:port/v1`).
///
/// Deliberately unconditional (no `remote-embed` cfg gate): the root crate's
/// `RetrievalConfig::normalize_embedder_url` calls this unconditionally too,
/// and duplicating the branch logic in both crates is exactly what let them
/// drift in the past — see `RemoteEmbedder::from_url`, which derives its own
/// `/v1/embeddings`-suffixed endpoint from this same stripped base rather
/// than repeating the suffix checks.
pub fn normalize_embeddings_base(url: &str) -> &str {
    let trimmed = url.trim_end_matches('/');
    if let Some(base) = trimmed.strip_suffix("/v1/embeddings") {
        base
    } else if let Some(base) = trimmed.strip_suffix("/v1") {
        base
    } else {
        trimmed
    }
}

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
/// fastembed's `DEFAULT_MAX_LENGTH` — the token ceiling every `local:` model is
/// actually tokenized at, irrespective of its advertised context window. See the
/// clamp in [`chunk_size_for_model`]'s `local:` arm for why this is uniform.
const FASTEMBED_DEFAULT_MAX_TOKENS: usize = 512;

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
        // CodeRankEmbed. MEASURED against a live llama-server 2026-08-26 by binary
        // search on input length: 8000 chars of repeated 'x' embedded, 8250 failed with
        // HTTP 500 `input is too large to process` — i.e. ~2048 tokens, the model's
        // n_ctx_train.
        //
        // It derives from nomic-embed-text-v1.5 but does NOT inherit that model's
        // 8192-token window, and the string "CodeRankEmbed" contains no "nomic"
        // substring — so without this arm it fell to the 512 fallback below, and a
        // consumer sizing work off this function would segment ~4x more finely than the
        // model requires.
        if l.contains("coderank") {
            return 2048;
        }
        // Unknown — conservative fallback
        512
    }

    // local-dir: always loads AllMiniLM-L6-v2-Q — from_dir (local.rs) is
    // hardcoded to that model's tokenizer/pooling/quantization regardless
    // of what the directory path contains, so the chunk size is fixed,
    // never derived by substring-matching the path string (a path like
    // ".../models--nomic-ai--nomic-embed-text-v1.5" would otherwise match
    // the 8192-token branch below and massively over-chunk). Delegates to
    // the `local:` arm below rather than restating its literal, so the two
    // cannot silently disagree if that table entry ever changes. Cannot
    // recurse: the delegated string takes the `local:` arm on the next
    // call, which returns before this check is reached again.
    if model_spec.starts_with("local-dir:") {
        return chunk_size_for_model("local:AllMiniLML6V2Q");
    }

    // Local fastembed models use their documented sequence lengths.
    // These are listed here rather than in local.rs to avoid a feature-gate
    // dependency (local.rs is #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]).
    if let Some(local_name) = model_spec.strip_prefix("local:") {
        let max_tokens = match local_name.to_lowercase().as_str() {
            "nomicembedtextv15" | "nomicembedtextv15q" => 8192,
            "jinaembeddingsv2basecode" => 8192,
            "bgesmallenv15q" | "bgesmallenv15" => 512,
            "allminilml6v2q" | "allminilml6v2" => 256,
            _ => 512,
        };
        // A local model's advertised context is NOT what it gets. fastembed
        // truncates at `min(DEFAULT_MAX_LENGTH, tokenizer_config.model_max_length)`
        // (`fastembed/src/common.rs:91,106`) and `DEFAULT_MAX_LENGTH` is 512 for
        // *every* model — `impl HasMaxLength for EmbeddingModel` hands back the same
        // constant regardless of which model it is
        // (`fastembed/src/text_embedding/init.rs:15-17`, `mod.rs:6`). local.rs never
        // calls `with_max_length`, so 512 is what we actually run with.
        //
        // Without this clamp the 8192-token entries above return 20 889 chars, and
        // everything past ~512 tokens of that is silently dropped by the tokenizer.
        // Measured 2026-08-14; see
        // docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md.
        //
        // Raising `with_max_length` in local.rs is the prerequisite for lifting this,
        // not a reason to skip it: this function must describe what the embedder does,
        // not what the model could do.
        return from_tokens(max_tokens.min(FASTEMBED_DEFAULT_MAX_TOKENS));
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
/// 1. `url` set → RemoteEmbedder targeting that URL
/// 2. `model` starts with `local-dir:` → local ONNX loaded from a directory, no network;
///    or starts with `local:` → local ONNX via fastembed
/// 3. `model` starts with `ollama:` → Ollama (errors loudly if unreachable)
/// 4. `model` starts with `openai:` → OpenAI API
/// 5. `model` starts with `custom:` → hard error with migration hint
/// 6. No url, no known prefix → try `model` as a bare local model name; else
///    the unknown-model error (there is no silent default)
pub async fn create_embedder_with_config(
    model: &str,
    url: Option<&str>,
    api_key: Option<String>,
) -> Result<Box<dyn Embedder>> {
    // Suppress unused-variable warning when remote-embed feature is disabled.
    #[cfg(not(feature = "remote-embed"))]
    let _ = &api_key;

    // 1. URL takes priority — any OpenAI-compatible endpoint
    #[cfg(feature = "remote-embed")]
    if let Some(url) = url {
        // local-dir: forces an offline, in-process ONNX embedder; url forces
        // a network client. The two are contradictory, not a precedence
        // question — without this check the model string is sent verbatim
        // to the server as a model name and fails later as an opaque
        // server-side rejection instead of here, with a clear reason.
        if model.starts_with("local-dir:") {
            anyhow::bail!(
                "Cannot combine url with a local-dir: model — url selects a \
                     network client while local-dir: forces an offline, \
                     in-process embedder.\n\
                     Remove url to use local-dir:<path>, or drop local-dir: to use url."
            );
        }
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

    // 2a. local-dir: prefix — weights from a directory, never the network.
    //     Must be its own arm: "local-dir:/x".strip_prefix("local:") is None
    //     (byte 5 is `-`, not `:`), so `local:` below does NOT capture it —
    //     confirmed by deleting this arm and observing arm 2 miss it. Without
    //     this arm (verified by actually deleting it and running the missing-
    //     weights case with local-embed on, no remote-embed): the string is
    //     caught by the un-cfg-gated "Local embedding requires the
    //     'local-embed' feature" bail below (its `local-dir:` check has no
    //     #[cfg], so it fires whether or not the feature is compiled) —
    //     never reaching the unknown-model bail, and never reaching `from_dir`.
    #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
    if let Some(path) = model.strip_prefix("local-dir:") {
        return Ok(Box::new(
            local::LocalEmbedder::from_dir(std::path::Path::new(path)).await?,
        ));
    }

    // 2. local: prefix
    #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
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
    #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
    {
        // Try parsing as a local model name directly
        if local::LocalEmbedder::new(model).await.is_ok() {
            return Ok(Box::new(local::LocalEmbedder::new(model).await?));
        }
    }

    // Helpful error for local: / local-dir: prefixes without the feature
    if model.starts_with("local:") || model.starts_with("local-dir:") {
        anyhow::bail!(
            "Local embedding requires the 'local-embed' feature.\n\
             Rebuild with: cargo build --features local-embed\n\n\
             Recommended: local:AllMiniLML6V2Q (384d, quantized, 22MB)\n\
             Offline hosts: local-dir:/path/to/weights (no network at all)"
        );
    }

    anyhow::bail!(
        "Unknown model '{}'. Options:\n\
         • Set url in [embeddings] to point at any OpenAI-compatible server\n\
         • Use local:AllMiniLML6V2Q for bundled ONNX (384d, 22MB, no server needed)\n\
         • Use local:JinaEmbeddingsV2BaseCode for code-specialized ONNX\n\
         • Use local-dir:/path/to/weights for an offline host (no network)",
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

    #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
    #[tokio::test]
    async fn local_dir_prefix_reports_the_directory_on_failure() {
        let err = crate::create_embedder_with_config("local-dir:/no/such/weights", None, None)
            .await
            .err()
            .expect("missing weights must error")
            .to_string();
        assert!(
            err.contains("/no/such/weights") || err.contains("\\no\\such\\weights"),
            "error must name the directory it was given, got: {err}"
        );
        assert!(
            err.contains("model_quantized.onnx"),
            "error must come from the directory loader, not a fallthrough, got: {err}"
        );
    }

    #[cfg(feature = "remote-embed")]
    #[tokio::test]
    async fn url_and_local_dir_together_is_a_hard_error() {
        let err = crate::create_embedder_with_config(
            "local-dir:/opt/weights",
            Some("http://localhost:1234/v1"),
            None,
        )
        .await
        .err()
        .expect("combining url with local-dir: must be a hard error")
        .to_string();
        assert!(
            err.contains("url") && err.contains("local-dir:"),
            "error must name both contradictory settings, got: {err}"
        );
    }

    #[test]
    fn chunk_size_pins_local_dir_to_the_all_minilm_budget() {
        // from_dir is hardcoded to AllMiniLM-L6-v2-Q (see local.rs::MODEL_FILE),
        // so this is the only correct chunk size for ANY local-dir: path —
        // regardless of what the path string itself contains.
        let expected = crate::chunk_size_for_model("local:AllMiniLML6V2Q");
        assert_eq!(expected, 652, "sanity: the known-good baseline moved");
        assert_eq!(
            crate::chunk_size_for_model("local-dir:/opt/weights"),
            expected,
            "local-dir: must use the same budget as the hub AllMiniLM path, \
             not fall through to substring-matching the filesystem path"
        );
        assert_eq!(
            crate::chunk_size_for_model(
                "local-dir:/root/.cache/huggingface/hub/models--nomic-ai--nomic-embed-text-v1.5"
            ),
            expected,
            "a path that happens to contain another model's name must not \
             change the chunk size — from_dir always loads AllMiniLM"
        );
    }

    /// A `local:` model's advertised context is not what it gets. fastembed tokenizes
    /// every model at `DEFAULT_MAX_LENGTH = 512` because
    /// `impl HasMaxLength for EmbeddingModel` returns that same constant for all of
    /// them, and `local.rs` never calls `with_max_length`. Chunks sized to an 8192-token
    /// window would have most of their content silently dropped by the tokenizer.
    ///
    /// Measured 2026-08-14 against `fastembed-5.13.4`; see
    /// `docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md`.
    #[test]
    fn local_models_are_clamped_to_fastembeds_actual_token_ceiling() {
        let ceiling = crate::chunk_size_for_model("local:BGESmallENV15Q");
        assert_eq!(ceiling, 1305, "sanity: 512 tokens x 0.85 x 3 chars moved");

        for big in [
            "local:NomicEmbedTextV15Q",
            "local:NomicEmbedTextV15",
            "local:JinaEmbeddingsV2BaseCode",
        ] {
            assert_eq!(
                crate::chunk_size_for_model(big),
                ceiling,
                "{big} advertises 8192 tokens but fastembed truncates it at 512, so its \
                 chunk budget must not exceed the 512-token budget"
            );
        }

        // Models already under the ceiling keep their own smaller budget — the clamp is
        // a maximum, not a floor that inflates small-context models.
        assert_eq!(
            crate::chunk_size_for_model("local:AllMiniLML6V2Q"),
            652,
            "a 256-token model must stay at its own budget, not be raised to 512's"
        );

        // Remote arms are unaffected: no fastembed tokenizer sits in that path, so an
        // 8192-token endpoint really does accept 8192 tokens.
        assert!(
            crate::chunk_size_for_model("ollama:nomic-embed-text") > ceiling,
            "the clamp must apply to the local: arm only"
        );
    }

    #[tokio::test]
    async fn unknown_model_error_advertises_local_dir() {
        let err = crate::create_embedder_with_config("banana", None, None)
            .await
            .err()
            .expect("unknown model must error")
            .to_string();
        assert!(
            err.contains("local-dir:"),
            "the unknown-model error must advertise the offline form, got: {err}"
        );
    }
}
