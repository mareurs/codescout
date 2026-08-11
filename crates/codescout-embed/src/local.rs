//! Local CPU embedding via fastembed-rs (ONNX Runtime).
//!
//! Two ways to get weights:
//! - [`LocalEmbedder::new`] / [`LocalEmbedder::new_blocking`]: model strings
//!   use fastembed's `EmbeddingModel` variant names directly, e.g.
//!   `local:JinaEmbeddingsV2BaseCode` or `local:BGESmallENV15Q`. Models are
//!   downloaded on first use to `~/.cache/huggingface/hub/` — requires
//!   network on that first use.
//! - [`LocalEmbedder::from_dir`]: loads AllMiniLM-L6-v2-Q weights already
//!   present on disk — no hf-hub, no network, works offline. See
//!   [`MODEL_FILE`] for the expected on-disk layout and its model-identity
//!   caveat.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::Embedding;

/// Weights layout expected by [`LocalEmbedder::from_dir`]. Values match
/// fastembed's own registry entry for AllMiniLM-L6-v2 (quantized) — wrong
/// values here yield plausible, silently wrong vectors.
///
/// `from_dir` assumes the directory holds AllMiniLM-L6-v2-Q weights
/// specifically — nothing here checks model identity. A different model
/// whose snapshot directory happens to use the same five filenames (e.g.
/// `BAAI/bge-small-en-v1.5`, also a 384-dim BERT model, but wanting
/// `Pooling::Cls` + `QuantizationMode::Static` instead of `Mean`/`Dynamic`)
/// will load without error and produce silently wrong vectors. There is no
/// cheap runtime discriminator for this — architecture/`hidden_size` checks
/// cannot tell the two models apart. The real guard is
/// `local::tests::from_dir_matches_the_hub_path_for_the_same_model`: it
/// embeds the same text via `from_dir` and via the hub path
/// (`LocalEmbedder::new`, which derives pooling/quantization from
/// fastembed's own registry rather than from these constants) and asserts
/// the vectors match — not a golden vector fixed at implementation time
/// (which would only prove self-consistency), and not a config.json check
/// here.
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

/// Repair-and-continue for the common mistake of pointing `from_dir` at a
/// HuggingFace-style model-repo directory (`models--<org>--<name>/`) rather
/// than the `snapshots/<hash>/` directory one level below it, where the
/// actual weight files live. Descends exactly one `snapshots/<hash>` level
/// from the directory it is given — it does NOT walk further down from a
/// true HuggingFace cache root (`~/.cache/huggingface/hub/`, where
/// `<root>/snapshots` does not exist); the caller must already point at the
/// specific model-repo directory.
///
/// Descends only when there is EXACTLY ONE snapshot holding the expected
/// files — two candidates is not one correct reading, so the given
/// directory is left alone and the caller gets the ordinary missing-file
/// error naming the directory it was given.
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
    match candidates.len() {
        1 => {
            let found = candidates.remove(0);
            tracing::info!(
                given = %dir.display(),
                resolved = %found.display(),
                "local weights: descended into the sole snapshot directory"
            );
            found
        }
        0 => dir.to_path_buf(),
        n => {
            tracing::warn!(
                given = %dir.display(),
                candidate_count = n,
                candidates = ?candidates,
                "local weights: multiple snapshot directories hold the expected files — refusing to guess, using the given directory as-is"
            );
            dir.to_path_buf()
        }
    }
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
    ///
    /// Assumes AllMiniLM-L6-v2-Q specifically — see [`MODEL_FILE`] for why a
    /// different model with the same on-disk file layout would load without
    /// error and silently produce wrong vectors; nothing here checks model
    /// identity.
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

    use crate::Embedder;

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

    /// The only test that catches a wrong tokenizer or wrong pooling: both produce
    /// a correctly-shaped, silently-wrong vector.
    ///
    /// Skips ONLY on an explicit opt-out, never on a missing file. A skip keyed on
    /// file presence is indistinguishable from a pass — which is how two tests in
    /// this repo shipped unable to fail.
    #[tokio::test]
    async fn from_dir_produces_a_stable_384d_vector() {
        if std::env::var("CODESCOUT_SKIP_ONNX_TESTS").is_ok() {
            eprintln!("SKIP from_dir_produces_a_stable_384d_vector: opt-out is set");
            return;
        }
        let dir = std::path::PathBuf::from(
            std::env::var("CODESCOUT_TEST_ONNX_DIR")
                .expect("set CODESCOUT_TEST_ONNX_DIR, or CODESCOUT_SKIP_ONNX_TESTS=1 to opt out"),
        );
        assert!(
            dir.join(MODEL_FILE).exists() || dir.join("snapshots").exists(),
            "no weights at {} — the CI seeding step did not run",
            dir.display()
        );
        let e = LocalEmbedder::from_dir(&dir)
            .await
            .expect("weights must load");
        let a = e.embed(&["fn main() {}"]).await.unwrap();
        let b = e.embed(&["fn main() {}"]).await.unwrap();
        assert_eq!(a[0].len(), 384, "AllMiniLM-L6-v2 is 384-dimensional");
        assert_eq!(
            e.dimensions(),
            384,
            "probe-derived dims must match the vector"
        );
        assert_eq!(a[0], b[0], "same input must give the same vector");
        assert!(
            a[0].iter().any(|x| *x != 0.0),
            "an all-zero vector means the session ran but produced nothing usable"
        );
    }

    /// Hub-vs-dir parity: `from_dir_blocking` hardcodes `Pooling::Mean` and
    /// `QuantizationMode::Dynamic`, copied by hand from fastembed's own
    /// registry entry for AllMiniLM-L6-v2-Q (see `MODEL_FILE`'s doc comment).
    /// Nothing at runtime checks those two constants are right — a wrong value
    /// yields a correctly-shaped, plausible, silently wrong vector, and every
    /// other test above would still pass.
    ///
    /// The hub path (`LocalEmbedder::new`) derives pooling and quantization
    /// from fastembed's own registry, not from our constants — confirmed by
    /// reading fastembed's `get_default_pooling_method` /
    /// `get_quantization_mode` (both keyed on `EmbeddingModel::AllMiniLML6V2Q`,
    /// resolving to `Pooling::Mean` / `QuantizationMode::Dynamic`) and its
    /// registry `model_code` for that variant (`Xenova/all-MiniLM-L6-v2`,
    /// `onnx/model_quantized.onnx` — the exact repo and file this test's
    /// `CODESCOUT_TEST_ONNX_DIR` is seeded from). So comparing hub output to
    /// `from_dir` output proves our hardcoded constants equal what the
    /// registry independently picks. A golden vector produced by our own
    /// implementation would be circular here — it would only prove the
    /// constants are consistent with themselves, not that they are correct.
    ///
    /// Skips ONLY on the explicit opt-out, never on a missing file — same
    /// contract as `from_dir_produces_a_stable_384d_vector` above. Needs
    /// network (hub path) AND the seeded dir, so it belongs in the same lane.
    #[tokio::test]
    async fn from_dir_matches_the_hub_path_for_the_same_model() {
        if std::env::var("CODESCOUT_SKIP_ONNX_TESTS").is_ok() {
            eprintln!("SKIP from_dir_matches_the_hub_path_for_the_same_model: opt-out is set");
            return;
        }
        let dir = std::path::PathBuf::from(
            std::env::var("CODESCOUT_TEST_ONNX_DIR")
                .expect("set CODESCOUT_TEST_ONNX_DIR, or CODESCOUT_SKIP_ONNX_TESTS=1 to opt out"),
        );
        assert!(
            dir.join(MODEL_FILE).exists() || dir.join("snapshots").exists(),
            "no weights at {} — the CI seeding step did not run",
            dir.display()
        );

        let text = "fn main() {}";
        let hub = LocalEmbedder::new("AllMiniLML6V2Q")
            .await
            .expect("hub model must load");
        let dir_embedder = LocalEmbedder::from_dir(&dir)
            .await
            .expect("dir weights must load");

        let from_hub = hub.embed(&[text]).await.unwrap();
        let from_dir = dir_embedder.embed(&[text]).await.unwrap();

        assert_eq!(
            from_hub[0], from_dir[0],
            "from_dir's hardcoded Pooling::Mean + QuantizationMode::Dynamic must \
             match what fastembed's own registry picks for the hub path — a \
             mismatch means one of those two constants is wrong"
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

    #[test]
    fn resolve_weights_dir_prefers_direct_weights_over_a_populated_snapshot() {
        // The direct dir holds valid weights AND has a snapshot subdirectory
        // that ALSO independently qualifies. If the early `holds_weights(dir)`
        // return were deleted or negated, this would wrongly descend into the
        // snapshot instead of stopping at `dir` — this is the discriminator a
        // bare "no snapshots/ present" case (below) cannot provide, since that
        // case returns the same `dir` value via the read_dir-error fallback
        // regardless of whether the early return exists.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("onnx")).unwrap();
        std::fs::write(tmp.path().join(MODEL_FILE), b"direct").unwrap();
        for f in REQUIRED_TOKENIZER_FILES {
            std::fs::write(tmp.path().join(f), b"{}").unwrap();
        }
        let snap = tmp.path().join("snapshots").join("deadbeef");
        std::fs::create_dir_all(snap.join("onnx")).unwrap();
        std::fs::write(snap.join(MODEL_FILE), b"nested").unwrap();
        for f in REQUIRED_TOKENIZER_FILES {
            std::fs::write(snap.join(f), b"{}").unwrap();
        }
        assert_eq!(resolve_weights_dir(tmp.path()), tmp.path());
    }

    #[test]
    fn resolve_weights_dir_leaves_an_empty_snapshots_dir_alone() {
        // `dir` does not hold weights directly, but `snapshots/` exists and
        // has zero valid candidates inside. This must fall through to the
        // ordinary "return dir unchanged" path without panicking — a
        // `candidates.len() == 1` -> `<= 1` mutation would instead try
        // `candidates.remove(0)` on an empty Vec here and panic.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("snapshots").join("empty")).unwrap();
        assert_eq!(resolve_weights_dir(tmp.path()), tmp.path());
    }
}
