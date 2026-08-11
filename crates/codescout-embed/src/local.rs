//! Local CPU embedding via fastembed-rs (ONNX Runtime).
//!
//! Model strings use fastembed's `EmbeddingModel` variant names directly,
//! e.g. `local:JinaEmbeddingsV2BaseCode` or `local:BGESmallENV15Q`.
//! Models are downloaded on first use to `~/.cache/huggingface/hub/`.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::Embedding;

/// Weights layout expected by [`LocalEmbedder::from_dir`]. Values match
/// fastembed's own registry entry for AllMiniLM-L6-v2 (quantized) — wrong
/// values here yield plausible, silently wrong vectors.
pub const MODEL_FILE: &str = "onnx/model_quantized.onnx";
pub const REQUIRED_TOKENIZER_FILES: [&str; 4] = [
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

/// True when `dir` directly holds the five expected weight files.
fn holds_weights(dir: &Path) -> bool {
    dir.join(MODEL_FILE).exists()
        && REQUIRED_TOKENIZER_FILES
            .iter()
            .all(|f| dir.join(f).exists())
}

/// Repair-and-continue for the mistake this layout invites: pointing at the
/// HuggingFace cache root rather than the snapshot directory four levels down.
/// Descends only when there is EXACTLY ONE snapshot holding the expected files
/// — two candidates is not one correct reading, so it is left alone and the
/// caller gets the ordinary missing-file error naming the directory it was given.
pub fn resolve_weights_dir(dir: &Path) -> PathBuf {
    if holds_weights(dir) {
        return dir.to_path_buf();
    }
    let snapshots = dir.join("snapshots");
    let Ok(entries) = std::fs::read_dir(&snapshots) else {
        return dir.to_path_buf();
    };
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| holds_weights(p))
        .collect();
    if candidates.len() == 1 {
        let found = candidates.remove(0);
        tracing::info!(
            given = %dir.display(),
            resolved = %found.display(),
            "local weights: descended into the sole snapshot directory"
        );
        return found;
    }
    dir.to_path_buf()
}

fn read_required(dir: &Path, rel: &str) -> Result<Vec<u8>> {
    let p = dir.join(rel);
    std::fs::read(&p).map_err(|e| anyhow::anyhow!("cannot read {} ({e})", p.display()))
}

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

    /// Load weights from a directory rather than fastembed's model enum.
    ///
    /// `dir` holds `onnx/model_quantized.onnx` plus the four tokenizer files —
    /// the `snapshots/<ref>/` level, not the cache root (a cache root is
    /// repaired; see [`resolve_weights_dir`]). Never touches the network:
    /// `try_new_from_user_defined` builds the session from bytes read here.
    pub async fn from_dir(dir: &Path) -> Result<Self> {
        let dir = dir.to_path_buf();
        tokio::task::spawn_blocking(move || Self::from_dir_blocking(&dir))
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking join error: {e}"))?
    }

    fn from_dir_blocking(dir: &Path) -> Result<Self> {
        let dir = resolve_weights_dir(dir);
        let onnx_file = read_required(&dir, MODEL_FILE)?;
        let tokenizer_files = fastembed::TokenizerFiles {
            tokenizer_file: read_required(&dir, REQUIRED_TOKENIZER_FILES[0])?,
            config_file: read_required(&dir, REQUIRED_TOKENIZER_FILES[1])?,
            special_tokens_map_file: read_required(&dir, REQUIRED_TOKENIZER_FILES[2])?,
            tokenizer_config_file: read_required(&dir, REQUIRED_TOKENIZER_FILES[3])?,
        };
        let user_model = fastembed::UserDefinedEmbeddingModel {
            onnx_file,
            external_initializers: Vec::new(),
            tokenizer_files,
            // Copied from fastembed's registry for AllMiniLM-L6-v2-Q. A wrong
            // pooling or quantization here produces a correctly-shaped, wrong
            // vector — which only the real-embed test in Task 3 can catch.
            pooling: Some(fastembed::Pooling::Mean),
            quantization: fastembed::QuantizationMode::Dynamic,
            output_key: None,
        };
        let mut model = fastembed::TextEmbedding::try_new_from_user_defined(
            user_model,
            fastembed::InitOptionsUserDefined::default(),
        )
        .map_err(|e| anyhow::anyhow!("ONNX session init failed for {}: {e}", dir.display()))?;
        // Same probe the hub path uses — the model is the source of truth for
        // dimensionality, never the caller's configuration.
        let probe = model.embed(vec!["probe".to_string()], None)?;
        let dims = probe
            .first()
            .map(|v| v.len())
            .filter(|&d| d > 0)
            .ok_or_else(|| {
                anyhow::anyhow!("probe returned an empty embedding — weights may be corrupt")
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

    #[tokio::test]
    async fn from_dir_missing_names_the_path_and_the_model_file() {
        let dir = std::path::Path::new("does/not/exist");
        let err = LocalEmbedder::from_dir(dir)
            .await
            .err()
            .expect("missing weights must error")
            .to_string();
        assert!(
            err.contains("model_quantized.onnx"),
            "error must name the model file, got: {err}"
        );
        assert!(
            err.contains("does/not/exist") || err.contains("does\\not\\exist"),
            "error must name the directory, got: {err}"
        );
    }

    #[test]
    fn resolve_weights_dir_descends_into_a_lone_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("snapshots").join("deadbeef");
        std::fs::create_dir_all(snap.join("onnx")).unwrap();
        std::fs::write(snap.join(MODEL_FILE), b"not-a-real-model").unwrap();
        for f in REQUIRED_TOKENIZER_FILES {
            std::fs::write(snap.join(f), b"{}").unwrap();
        }
        assert_eq!(resolve_weights_dir(tmp.path()), snap);
    }

    #[test]
    fn resolve_weights_dir_leaves_a_direct_dir_alone() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("onnx")).unwrap();
        std::fs::write(tmp.path().join(MODEL_FILE), b"not-a-real-model").unwrap();
        for f in REQUIRED_TOKENIZER_FILES {
            std::fs::write(tmp.path().join(f), b"{}").unwrap();
        }
        assert_eq!(resolve_weights_dir(tmp.path()), tmp.path());
    }

    #[test]
    fn resolve_weights_dir_leaves_ambiguous_snapshots_alone() {
        let tmp = tempfile::tempdir().unwrap();
        for h in ["aaa", "bbb"] {
            let snap = tmp.path().join("snapshots").join(h);
            std::fs::create_dir_all(snap.join("onnx")).unwrap();
            std::fs::write(snap.join(MODEL_FILE), b"x").unwrap();
            for f in REQUIRED_TOKENIZER_FILES {
                std::fs::write(snap.join(f), b"{}").unwrap();
            }
        }
        // Two candidates is not "exactly one correct reading" — do not guess.
        assert_eq!(resolve_weights_dir(tmp.path()), tmp.path());
    }
}
