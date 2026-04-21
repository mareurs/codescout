//! Local CPU embedding via fastembed-rs (ONNX Runtime).
//!
//! Model strings use fastembed's `EmbeddingModel` variant names directly,
//! e.g. `local:JinaEmbeddingsV2BaseCode` or `local:BGESmallENV15Q`.
//! Models are downloaded on first use to `~/.cache/huggingface/hub/`.

use anyhow::Result;
use std::sync::{Arc, Mutex};

use crate::Embedding;

pub struct LocalEmbedder {
    model: Arc<Mutex<fastembed::TextEmbedding>>,
    dims: usize,
}

impl LocalEmbedder {
    /// Create a new local embedder.  The heavy ONNX session creation runs on
    /// `spawn_blocking` to keep the async executor responsive.
    pub async fn new(model_name: &str) -> Result<Self> {
        let model_name = model_name.to_string();
        tokio::task::spawn_blocking(move || Self::new_blocking(&model_name))
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking join error: {e}"))?
    }

    fn new_blocking(model_name: &str) -> Result<Self> {
        let embedding_model = parse_model(model_name)?;
        let mut opts = fastembed::InitOptions::new(embedding_model);
        opts.show_download_progress = false;
        let mut model = fastembed::TextEmbedding::try_new(opts)?;
        // Derive actual dims by embedding a probe string.
        let probe = model.embed(vec!["probe".to_string()], None)?;
        let dims = probe
            .first()
            .map(|v| v.len())
            .filter(|&d| d > 0)
            .ok_or_else(|| {
                anyhow::anyhow!("fastembed probe returned empty embedding — model may be corrupt")
            })?;
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            dims,
        })
    }
}

fn parse_model(name: &str) -> Result<fastembed::EmbeddingModel> {
    match name {
        "NomicEmbedTextV15" => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15),
        "NomicEmbedTextV15Q" => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15Q),
        "JinaEmbeddingsV2BaseCode" => Ok(fastembed::EmbeddingModel::JinaEmbeddingsV2BaseCode),
        "BGESmallENV15Q" => Ok(fastembed::EmbeddingModel::BGESmallENV15Q),
        "AllMiniLML6V2Q" => Ok(fastembed::EmbeddingModel::AllMiniLML6V2Q),
        // Non-quantized variants for users who want full f32 precision
        "BGESmallENV15" => Ok(fastembed::EmbeddingModel::BGESmallENV15),
        "AllMiniLML6V2" => Ok(fastembed::EmbeddingModel::AllMiniLML6V2),
        other => anyhow::bail!(
            "Unknown local model '{other}'. Supported variants:\n\
             • local:AllMiniLML6V2Q               (384d, quantized, ~22MB, recommended default)\n\
             • local:NomicEmbedTextV15Q           (768d, quantized, ~158MB, higher quality)\n\
             • local:NomicEmbedTextV15            (768d, full precision, ~547MB)\n\
             • local:JinaEmbeddingsV2BaseCode     (768d, code-specific, ~300MB)\n\
             • local:AllMiniLML6V2                (384d, full precision)\n\
             • local:BGESmallENV15Q               (384d, deprecated — GPU-only, crashes on CPU)\n\
             • local:BGESmallENV15                (384d, full precision)"
        ),
    }
}

#[async_trait::async_trait]
impl crate::Embedder for LocalEmbedder {
    fn dimensions(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let owned: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let model = Arc::clone(&self.model);
        tokio::task::spawn_blocking(move || {
            // fastembed 5 changed embed() to &mut self — Mutex serializes access across spawn_blocking tasks
            model
                .lock()
                .map_err(|e| anyhow::anyhow!("fastembed model lock poisoned: {e}"))?
                .embed(owned, None)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_unknown_name_returns_error() {
        let err = parse_model("NotARealModel").unwrap_err().to_string();
        assert!(err.contains("NotARealModel"));
        assert!(
            err.contains("JinaEmbeddingsV2BaseCode"),
            "error should list supported models"
        );
    }

    #[test]
    fn parse_model_known_names_return_ok() {
        assert!(parse_model("NomicEmbedTextV15").is_ok());
        assert!(parse_model("NomicEmbedTextV15Q").is_ok());
        assert!(parse_model("JinaEmbeddingsV2BaseCode").is_ok());
        assert!(parse_model("BGESmallENV15Q").is_ok());
        assert!(parse_model("AllMiniLML6V2Q").is_ok());
        assert!(parse_model("BGESmallENV15").is_ok());
        assert!(parse_model("AllMiniLML6V2").is_ok());
    }

    #[test]
    fn parse_model_nomic_v15_variants() {
        assert!(parse_model("NomicEmbedTextV15").is_ok());
        assert!(parse_model("NomicEmbedTextV15Q").is_ok());
    }
}
