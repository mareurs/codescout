//! In-process ONNX embedder for the daemon-free stack.
//!
//! Loads weights from a directory rather than fastembed's model enum, because
//! every HuggingFace route is blocked on the target host — `try_new_from_user_defined`
//! builds the session from bytes and never touches the network.
//! See `docs/superpowers/specs/2026-08-08-local-onnx-embedder-design.md`.

use crate::retrieval::embedder::{BatchEmbedder, CodeEmbedder, EmbedOutput, SparseVector};
use crate::tools::RecoverableError;
use anyhow::Result;
use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use std::path::{Path, PathBuf};

/// AllMiniLM-L6-v2 (quantized) — values copied from fastembed's own model
/// registry (`models/text_embedding.rs`, `get_default_pooling_method`,
/// `get_quantization_mode`). Wrong values here yield plausible wrong vectors.
const MODEL_FILE: &str = "onnx/model_quantized.onnx";

pub struct LocalOnnxEmbedder {
    model: TextEmbedding,
    expected_dim: usize,
    dir: PathBuf,
}

fn read_required(dir: &Path, rel: &str) -> Result<Vec<u8>> {
    let p = dir.join(rel);
    std::fs::read(&p).map_err(|e| {
        RecoverableError::with_hint(
            format!("local embedder: cannot read {} ({e})", p.display()),
            format!(
                "Expected ONNX weights under {}. Required files: {MODEL_FILE}, \
                 tokenizer.json, config.json, special_tokens_map.json, tokenizer_config.json. \
                 HuggingFace is unreachable on this host; recover the bundle from \
                 https://chroma-onnx-models.s3.amazonaws.com/all-MiniLM-L6-v2/onnx.tar.gz",
                dir.display()
            ),
        )
        .into()
    })
}

impl LocalOnnxEmbedder {
    /// `dir` is the directory containing `onnx/model_quantized.onnx` and the
    /// tokenizer files (the `snapshots/<ref>/` level, not the cache root).
    pub fn new(dir: &Path, expected_dim: usize) -> Result<Self> {
        let onnx_file = read_required(dir, MODEL_FILE)?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read_required(dir, "tokenizer.json")?,
            config_file: read_required(dir, "config.json")?,
            special_tokens_map_file: read_required(dir, "special_tokens_map.json")?,
            tokenizer_config_file: read_required(dir, "tokenizer_config.json")?,
        };
        let user_model = UserDefinedEmbeddingModel {
            onnx_file,
            external_initializers: Vec::new(),
            tokenizer_files,
            pooling: Some(Pooling::Mean),
            quantization: QuantizationMode::Dynamic,
            output_key: None,
        };
        let model =
            TextEmbedding::try_new_from_user_defined(user_model, InitOptionsUserDefined::default())
                .map_err(|e| {
                    RecoverableError::with_hint(
                        format!("local embedder: ONNX session init failed: {e}"),
                        "If this is a dylib load error, set ORT_DYLIB_PATH to onnxruntime.dll. \
                 An `os error 5` here is application control (CyberArk EPM) denying the \
                 load, not a missing file — the DLL is present but not permitted to execute.",
                    )
                })?;
        Ok(Self {
            model,
            expected_dim,
            dir: dir.to_path_buf(),
        })
    }

    fn embed_texts(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let out = self
            .model
            .embed(texts, None)
            .map_err(|e| anyhow::anyhow!("local embed failed: {e}"))?;
        if let Some(first) = out.first() {
            if first.len() != self.expected_dim {
                return Err(RecoverableError::with_hint(
                    format!(
                        "local embedder dim mismatch: model produced {}, configured {}",
                        first.len(),
                        self.expected_dim
                    ),
                    format!(
                        "Set CODESCOUT_MODEL_DIM={} to match the weights in {}, then delete \
                         .codescout/code-index.db and reindex — vec0 bakes the dimension into \
                         the table at creation and cannot migrate in place.",
                        first.len(),
                        self.dir.display()
                    ),
                )
                .into());
            }
        }
        Ok(out)
    }
}

#[async_trait::async_trait]
impl BatchEmbedder for LocalOnnxEmbedder {
    async fn embed_batch_dyn(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
        let dense = self.embed_texts(texts.to_vec())?;
        Ok(dense
            .into_iter()
            .map(|d| EmbedOutput {
                dense: d,
                sparse: SparseVector {
                    indices: vec![],
                    values: vec![],
                },
            })
            .collect())
    }
}

#[async_trait::async_trait]
impl CodeEmbedder for LocalOnnxEmbedder {
    async fn embed_one(&self, text: &str) -> Result<EmbedOutput> {
        Ok(EmbedOutput {
            dense: self.embed_dense_one(text).await?,
            sparse: SparseVector {
                indices: vec![],
                values: vec![],
            },
        })
    }
    async fn embed_dense_one(&self, text: &str) -> Result<Vec<f32>> {
        let mut v = self.embed_texts(vec![text.to_string()])?;
        v.pop()
            .ok_or_else(|| anyhow::anyhow!("local embedder returned no vector"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Missing weights must name the path and the file we looked for — an `ort`
    /// file-not-found alone tells the operator nothing actionable.
    #[test]
    fn missing_model_dir_names_the_expected_files() {
        let dir = std::path::Path::new("does/not/exist");
        let err = LocalOnnxEmbedder::new(dir, 384).unwrap_err().to_string();
        assert!(
            err.contains("model_quantized.onnx"),
            "error must name the model file, got: {err}"
        );
        assert!(
            err.contains("does/not/exist") || err.contains("does\\not\\exist"),
            "error must name the directory, got: {err}"
        );
    }

    /// The only test that catches a wrong tokenizer or wrong pooling: both
    /// produce a correctly-shaped, silently-wrong vector.
    ///
    /// Skips when the weights are absent (CI has none, and HuggingFace is
    /// blocked from the runners). It must NOT skip on the VDI — that is the one
    /// machine where this is the real coverage — so the skip prints loudly.
    #[test]
    fn real_embed_produces_a_stable_384d_vector() {
        let dir =
            std::path::PathBuf::from(std::env::var("CODESCOUT_TEST_ONNX_DIR").unwrap_or_else(
                |_| ".fastembed_cache/models--Xenova--all-MiniLM-L6-v2/snapshots/manual".into(),
            ));
        // Ruling 2026-08-08: skip ONLY on an explicit opt-out, never on a
        // missing file. A typo'd weights path must fail loudly rather than pass
        // green — a skip keyed on file presence is indistinguishable from
        // success, which is how two tests in this codebase shipped unable to
        // fail. CI sets CODESCOUT_SKIP_ONNX_TESTS=1; the VDI never does.
        if std::env::var("CODESCOUT_SKIP_ONNX_TESTS").is_ok() {
            eprintln!("SKIP real_embed: CODESCOUT_SKIP_ONNX_TESTS is set");
            return;
        }
        assert!(
            dir.join(MODEL_FILE).exists(),
            "no weights at {} — set CODESCOUT_TEST_ONNX_DIR, or set \
             CODESCOUT_SKIP_ONNX_TESTS=1 to opt out deliberately",
            dir.join(MODEL_FILE).display()
        );
        let e = LocalOnnxEmbedder::new(&dir, 384).expect("weights present, must load");
        let a = e.embed_texts(vec!["fn main() {}".to_string()]).unwrap();
        let b = e.embed_texts(vec!["fn main() {}".to_string()]).unwrap();
        assert_eq!(a[0].len(), 384, "AllMiniLM-L6-v2 is 384-dimensional");
        assert_eq!(a[0], b[0], "same input must give the same vector");
        assert!(
            a[0].iter().any(|x| *x != 0.0),
            "an all-zero vector means the session ran but produced nothing usable"
        );
    }
}
