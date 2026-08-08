# Local ONNX Embedder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `semantic_search` and `memory(recall)` embed in-process via ONNX, so both work on a host with no reachable embedding server.

**Architecture:** Introduce a `CodeEmbedder` trait object on `RetrievalClient` (mirroring the existing `Arc<dyn CodeVectorStore>` seam), implement it for the existing `EmbedderHttp` and a new feature-gated `LocalOnnxEmbedder`, and select between them from one env var. Memory recall reaches the same object through a thin `DenseEmbedder` adapter, so it is fixed without a second selection path.

**Tech Stack:** Rust, `fastembed 5.13` (`try_new_from_user_defined`), `ort` via `local-embed-dynamic`, `async-trait`, sqlite-vec.

**Spec:** `docs/superpowers/specs/2026-08-08-local-onnx-embedder-design.md`

## Global Constraints

- Toolchain is pinned to `1.97.1` (`rust-toolchain.toml`). **On the Windows VDI that toolchain cannot link** (no MSVC linker); build and test with `cargo +stable-x86_64-pc-windows-gnu ...` locally. CI uses the pinned toolchain.
- Pre-commit gate for every task: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- Default behaviour must not change. With `CODESCOUT_EMBEDDER_MODEL` unset, every existing code path keeps using `EmbedderHttp`.
- `#[async_trait::async_trait]` on every new trait and impl — native `async fn` in traits is not dyn-compatible, and these are used as `dyn`.
- Error style: operator-fixable failures use `RecoverableError::with_hint`, not `anyhow::bail!`. See `get_guide("error-handling")`.
- Env-mutating tests use `EnvGuard` + `#[serial]` (project convention; see memory `conventions`).
- New ONNX-dependent tests must be `#[cfg(feature = "local-embed-dynamic")]` AND skip cleanly when weights are absent — but must NOT self-skip on the VDI, where they are the only real coverage.
- Model constants are fixed and must not be guessed: `AllMiniLML6V2Q` → **dim 384**, **`Pooling::Mean`**, **`QuantizationMode::Dynamic`**, `output_key: None`, model file `onnx/model_quantized.onnx`.

## File Structure

| File | Responsibility |
|---|---|
| `src/retrieval/embedder.rs` (modify) | `CodeEmbedder` trait, `impl CodeEmbedder for EmbedderHttp`, `CodeDenseAdapter` |
| `src/retrieval/local_onnx.rs` (create) | `LocalOnnxEmbedder` — file loading, session `OnceCell`, `BatchEmbedder`/`CodeEmbedder` impls |
| `src/retrieval/client.rs` (modify) | field type change + embedder selection in `from_env` |
| `src/retrieval/config.rs` (modify) | `embedder_model: Option<String>` from `CODESCOUT_EMBEDDER_MODEL` |
| `src/retrieval/search.rs:96` (modify) | `.embed(` → `.embed_one(` |
| `src/agent/mod.rs:1737-1749` (modify) | memory embedder built from the shared `Arc` |
| `src/retrieval/mod.rs` (modify) | `pub mod local_onnx;` (feature-gated) |
| `crates/codescout-embed/` | **untouched** — that crate serves librarian only |

---

### Task 1: `CodeEmbedder` trait, implemented for `EmbedderHttp`

Pure refactor. No behaviour change, no new dependency. Ends with the whole suite green.

**Files:**
- Modify: `src/retrieval/embedder.rs` (add after the `BatchEmbedder` block, ~line 36)
- Modify: `src/retrieval/client.rs:16` (field type), `:40-46` and `:78-84` (construction)
- Modify: `src/retrieval/search.rs:96`
- Modify: `src/retrieval/sync.rs:773` (test constructor)

**Interfaces:**
- Produces: `trait CodeEmbedder: BatchEmbedder` with `embed_one(&self, &str) -> Result<EmbedOutput>` and `embed_dense_one(&self, &str) -> Result<Vec<f32>>`; `RetrievalClient.embedder: Arc<dyn CodeEmbedder>`.

**Why two methods, not a `DenseEmbedder` supertrait:** `DenseEmbedder::embed` and `EmbedderHttp`'s inherent `embed` have the same name and different return types. Making `CodeEmbedder: DenseEmbedder` forces both onto one type, where inherent-method resolution silently wins. Distinct names avoid it. (This refines §1 of the spec, which proposed the supertrait.)

- [ ] **Step 1: Write the failing test**

In `src/retrieval/embedder.rs`, inside `mod tests`:

```rust
/// The trait object must expose both query shapes. If `EmbedderHttp` ever stops
/// satisfying `CodeEmbedder`, this fails to compile — which is the point.
#[tokio::test]
async fn embedder_http_is_usable_as_a_code_embedder_trait_object() {
    let e: std::sync::Arc<dyn CodeEmbedder> = std::sync::Arc::new(
        EmbedderHttp::with_config("http://127.0.0.1:1", "http://127.0.0.1:1", 768, "m", ""),
    );
    // Connect refusal is expected; we assert the call is dispatchable and that
    // the error names the URL, not that it succeeds.
    let err = e.embed_one("hello").await.unwrap_err().to_string();
    assert!(
        err.contains("127.0.0.1:1"),
        "error should name the dense URL, got: {err}"
    );
    let err2 = e.embed_dense_one("hello").await.unwrap_err().to_string();
    assert!(
        err2.contains("127.0.0.1:1"),
        "dense-one error should name the dense URL, got: {err2}"
    );
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib retrieval::embedder::tests::embedder_http_is_usable_as_a_code_embedder_trait_object`
Expected: FAIL — `cannot find trait CodeEmbedder in this scope`.

- [ ] **Step 3: Add the trait and impl**

In `src/retrieval/embedder.rs`, after the `impl BatchEmbedder for EmbedderHttp` block:

```rust
/// Query-side embedding seam for code search and memory recall.
///
/// `BatchEmbedder` covers the indexing path; this adds the two query shapes the
/// search and memory paths need. Held as `Arc<dyn CodeEmbedder>` on
/// [`crate::retrieval::client::RetrievalClient`], mirroring the
/// `Arc<dyn CodeVectorStore>` seam that made the lite stack possible.
///
/// Deliberately does NOT have `DenseEmbedder` as a supertrait: that trait's
/// `embed` collides by name with `EmbedderHttp`'s inherent `embed`, where
/// inherent resolution would silently win. Use [`CodeDenseAdapter`] to bridge.
#[async_trait::async_trait]
pub trait CodeEmbedder: BatchEmbedder {
    /// Full query embedding (dense + sparse when the impl has sparse).
    async fn embed_one(&self, text: &str) -> anyhow::Result<EmbedOutput>;
    /// Dense-only query embedding, for consumers that never rank on sparse.
    async fn embed_dense_one(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

#[async_trait::async_trait]
impl CodeEmbedder for EmbedderHttp {
    async fn embed_one(&self, text: &str) -> anyhow::Result<EmbedOutput> {
        self.embed(text).await
    }
    async fn embed_dense_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.dense_query(text).await
    }
}

/// Bridges an `Arc<dyn CodeEmbedder>` into the `DenseEmbedder` seam the memory
/// path holds, so memory recall and code search share one embedder instance.
pub struct CodeDenseAdapter(pub std::sync::Arc<dyn CodeEmbedder>);

#[async_trait::async_trait]
impl DenseEmbedder for CodeDenseAdapter {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.0.embed_dense_one(text).await
    }
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib retrieval::embedder::tests::embedder_http_is_usable_as_a_code_embedder_trait_object`
Expected: PASS.

- [ ] **Step 5: Widen the field and fix the call sites**

`src/retrieval/client.rs` — imports and field:

```rust
use crate::retrieval::embedder::{CodeEmbedder, EmbedderHttp};
```
```rust
    pub embedder: std::sync::Arc<dyn CodeEmbedder>,
```

In `from_env`, wrap the constructed value (keep every existing builder call unchanged):

```rust
        let embedder: std::sync::Arc<dyn CodeEmbedder> = std::sync::Arc::new(
            EmbedderHttp::new(
                &config.embedder_url,
                &config.sparse_embedder_url,
                config.model_dim,
            )
            .dense_only(dense_only),
        );
```

Same wrap in `from_config_only` (no `.dense_only`, matching today):

```rust
        let embedder: std::sync::Arc<dyn CodeEmbedder> = std::sync::Arc::new(EmbedderHttp::new(
            &config.embedder_url,
            &config.sparse_embedder_url,
            config.model_dim,
        ));
```

`src/retrieval/search.rs:96`:

```rust
        let q = self.embedder.embed_one(query).await?;
```

`src/retrieval/sync.rs:773` (test helper):

```rust
        embedder: std::sync::Arc::new(EmbedderHttp::new(
            "http://unused.invalid",
            "http://unused.invalid",
            3,
        )),
```

- [ ] **Step 6: Full gate**

Run: `cargo +stable-x86_64-pc-windows-gnu fmt && cargo +stable-x86_64-pc-windows-gnu clippy --all-targets -- -D warnings && cargo +stable-x86_64-pc-windows-gnu test --lib`
Expected: PASS, 3350+ tests, 0 failed. `sync.rs` needs no signature change — it already takes `&dyn BatchEmbedder`, and `Arc<dyn CodeEmbedder>` derefs to it.

- [ ] **Step 7: Commit**

```bash
git add src/retrieval/embedder.rs src/retrieval/client.rs src/retrieval/search.rs src/retrieval/sync.rs
git commit -m "refactor(retrieval): put the code embedder behind a CodeEmbedder trait object"
```

---

### Task 2: Memory recall shares the code embedder

Do this before the ONNX impl: it is small, and it means Task 3's work fixes both consumers the moment it lands.

**Files:**
- Modify: `src/agent/mod.rs:1737-1749`

**Interfaces:**
- Consumes: `CodeDenseAdapter` and `RetrievalClient.embedder` from Task 1.

- [ ] **Step 1: Write the failing test**

In `src/agent/mod.rs` tests (or the nearest existing test module):

```rust
/// Memory recall must ride the same embedder instance code search uses. If this
/// regresses, memory silently keeps its own HTTP embedder and a local model
/// configured for code search would not reach memory at all.
#[tokio::test]
async fn memory_embedder_is_built_from_the_shared_code_embedder() {
    use crate::retrieval::embedder::{CodeDenseAdapter, CodeEmbedder, EmbedderHttp};
    let shared: std::sync::Arc<dyn CodeEmbedder> = std::sync::Arc::new(
        EmbedderHttp::with_config("http://127.0.0.1:1", "http://127.0.0.1:1", 384, "m", ""),
    );
    let adapter = CodeDenseAdapter(shared.clone());
    // Two Arc handles to ONE embedder.
    assert_eq!(std::sync::Arc::strong_count(&shared), 2);
    let err = crate::retrieval::embedder::DenseEmbedder::embed(&adapter, "x")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("127.0.0.1:1"), "got: {err}");
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib memory_embedder_is_built_from_the_shared_code_embedder`
Expected: FAIL — `CodeDenseAdapter` unresolved if Task 1 is incomplete; otherwise it compiles and passes, which means only Step 3 remains.

- [ ] **Step 3: Rewire `memory_embedder`**

`src/agent/mod.rs`, replacing the `HttpDenseEmbedder` construction:

```rust
            .get_or_try_init(|| async {
                let client = crate::retrieval::client::RetrievalClient::from_env().await?;
                // Share the code-search embedder rather than building a second
                // one: whatever backend code search selected (HTTP or local
                // ONNX), memory recall now uses the same instance.
                let emb = crate::retrieval::embedder::CodeDenseAdapter(client.embedder.clone());
                anyhow::Ok(Arc::new(emb) as Arc<dyn crate::retrieval::embedder::DenseEmbedder>)
            })
```

Update the doc comment above `memory_embedder` — it currently says "wraps the resulting `EmbedderHttp` in `HttpDenseEmbedder`", which becomes false.

- [ ] **Step 4: Run the gate**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib && cargo +stable-x86_64-pc-windows-gnu clippy --all-targets -- -D warnings`
Expected: PASS. `HttpDenseEmbedder` may now be unused in production — if clippy flags it, keep the type (it is public API and used in tests) and add `#[allow(dead_code)]` only if the lint actually fires.

- [ ] **Step 5: Commit**

```bash
git add src/agent/mod.rs
git commit -m "refactor(memory): recall shares the code-search embedder instance"
```

---

### Task 3: `LocalOnnxEmbedder`

**Files:**
- Create: `src/retrieval/local_onnx.rs`
- Modify: `src/retrieval/mod.rs` (add the module, feature-gated)
- Modify: `Cargo.toml` (the `local-embed-dynamic` feature must reach the main crate)

**Interfaces:**
- Consumes: `CodeEmbedder`, `BatchEmbedder`, `EmbedOutput`, `SparseVector` from Task 1.
- Produces: `LocalOnnxEmbedder::new(dir: &Path, expected_dim: usize) -> Result<Self>`.

**Fixed constants — do not substitute:** `Pooling::Mean`, `QuantizationMode::Dynamic`, dim 384, file `onnx/model_quantized.onnx`. `UserDefinedEmbeddingModel::new` defaults to `pooling: None, quantization: QuantizationMode::None`; taking those defaults produces wrong-but-plausible vectors that no dimension assertion catches.

- [ ] **Step 1: Write the failing test**

Create `src/retrieval/local_onnx.rs` with the test module first:

```rust
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
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo +stable-x86_64-pc-windows-gnu test --features local-embed-dynamic --lib retrieval::local_onnx`
Expected: FAIL — module/type not found.

- [ ] **Step 3: Implement**

`src/retrieval/local_onnx.rs`:

```rust
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
        let model = TextEmbedding::try_new_from_user_defined(
            user_model,
            InitOptionsUserDefined::default(),
        )
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
```

`src/retrieval/mod.rs`:

```rust
#[cfg(feature = "local-embed-dynamic")]
pub mod local_onnx;
```

`Cargo.toml` — add the passthrough features to the main crate:

```toml
local-embed = ["codescout-embed/local-embed", "dep:fastembed", "fastembed/ort-download-binaries-native-tls", "fastembed/hf-hub-native-tls"]
local-embed-dynamic = ["codescout-embed/local-embed-dynamic", "dep:fastembed", "fastembed/ort-load-dynamic", "fastembed/hf-hub-native-tls"]
```

and under `[dependencies]`:

```toml
fastembed = { version = "5", optional = true, default-features = false }
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cargo +stable-x86_64-pc-windows-gnu test --features local-embed-dynamic --lib retrieval::local_onnx`
Expected: PASS.

- [ ] **Step 5: Add the real-embed test**

Append to `mod tests` in `src/retrieval/local_onnx.rs`:

```rust
    /// The only test that catches a wrong tokenizer or wrong pooling: both
    /// produce a correctly-shaped, silently-wrong vector.
    ///
    /// Skips when the weights are absent (CI has none, and HuggingFace is
    /// blocked from the runners). It must NOT skip on the VDI — that is the one
    /// machine where this is the real coverage — so the skip prints loudly.
    #[test]
    fn real_embed_produces_a_stable_384d_vector() {
        let dir = std::path::PathBuf::from(
            std::env::var("CODESCOUT_TEST_ONNX_DIR").unwrap_or_else(|_| {
                ".fastembed_cache/models--Xenova--all-MiniLM-L6-v2/snapshots/manual".into()
            }),
        );
        if !dir.join(MODEL_FILE).exists() {
            eprintln!("SKIP real_embed: no weights at {}", dir.display());
            return;
        }
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
```

- [ ] **Step 6: Run it on the VDI (weights present)**

Run: `cargo +stable-x86_64-pc-windows-gnu test --features local-embed-dynamic --lib retrieval::local_onnx -- --nocapture`
Expected: PASS with no SKIP line. Requires `ORT_DYLIB_PATH=<repo>/.onnxruntime/onnxruntime.dll`. If it prints SKIP, the weights path is wrong — fix that before continuing, or the coverage is imaginary.

- [ ] **Step 7: Commit**

```bash
git add src/retrieval/local_onnx.rs src/retrieval/mod.rs Cargo.toml
git commit -m "feat(retrieval): in-process ONNX embedder loaded from local weights"
```

---

### Task 4: Selection via `CODESCOUT_EMBEDDER_MODEL`

**Files:**
- Modify: `src/retrieval/config.rs` (field + `from_env`)
- Modify: `src/retrieval/client.rs` (`from_env` branch)

**Interfaces:**
- Consumes: `LocalOnnxEmbedder::new` (Task 3), `Arc<dyn CodeEmbedder>` (Task 1).
- Produces: `RetrievalConfig.embedder_model: Option<String>`.

- [ ] **Step 1: Write the failing tests**

In `src/retrieval/config.rs`, a new `mod selection_tests`:

```rust
#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::test_support::EnvGuard;
    use serial_test::serial;

    #[test]
    #[serial]
    fn embedder_model_absent_by_default() {
        let _g = EnvGuard::unset("CODESCOUT_EMBEDDER_MODEL");
        assert!(RetrievalConfig::from_env().unwrap().embedder_model.is_none());
    }

    #[test]
    #[serial]
    fn local_prefix_is_parsed_with_its_directory() {
        let _g = EnvGuard::set("CODESCOUT_EMBEDDER_MODEL", "local:/weights/minilm");
        let cfg = RetrievalConfig::from_env().unwrap();
        assert_eq!(cfg.embedder_model.as_deref(), Some("local:/weights/minilm"));
    }

    /// Precedence: a configured local model beats a set embedder URL. The URL
    /// default is always populated, so without this rule local could never win.
    #[test]
    #[serial]
    fn local_model_wins_over_a_set_embedder_url() {
        let _a = EnvGuard::set("CODESCOUT_EMBEDDER_MODEL", "local:/weights/minilm");
        let _b = EnvGuard::set("CODESCOUT_EMBEDDER_URL", "http://example.invalid:9999");
        let cfg = RetrievalConfig::from_env().unwrap();
        assert!(cfg.local_model_dir().is_some(), "local must win");
        assert_eq!(
            cfg.local_model_dir().unwrap().to_string_lossy(),
            "/weights/minilm"
        );
    }

    /// A bare `local:` is an error, not a guess at the cache location — silently
    /// picking a directory is how the wrong weights load unnoticed.
    #[test]
    #[serial]
    fn bare_local_prefix_is_rejected() {
        let _g = EnvGuard::set("CODESCOUT_EMBEDDER_MODEL", "local:");
        assert!(RetrievalConfig::from_env().is_err());
    }
}
```

Use the project's existing `EnvGuard` helper; if its constructor names differ, match them rather than inventing new ones.

- [ ] **Step 2: Run and confirm failure**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib retrieval::config::selection_tests`
Expected: FAIL — no `embedder_model` field, no `local_model_dir`.

- [ ] **Step 3: Implement**

`src/retrieval/config.rs` — add to the struct:

```rust
    /// `local:<dir>` selects the in-process ONNX embedder; `None` keeps the HTTP one.
    pub embedder_model: Option<String>,
```

in `from_env`, inside the `Ok(Self { ... })`:

```rust
            embedder_model: {
                let raw = std::env::var("CODESCOUT_EMBEDDER_MODEL").ok();
                if let Some(v) = raw.as_deref() {
                    if v == "local:" || v.trim().is_empty() {
                        anyhow::bail!(
                            "CODESCOUT_EMBEDDER_MODEL='local:' has no directory. \
                             Use local:<dir> where <dir> contains onnx/model_quantized.onnx \
                             and tokenizer.json."
                        );
                    }
                }
                raw
            },
```

and an accessor:

```rust
impl RetrievalConfig {
    /// The weights directory when a local embedder is configured. A relative
    /// path resolves against the project root, not the process CWD — the MCP
    /// server's CWD is not the project's.
    pub fn local_model_dir(&self) -> Option<std::path::PathBuf> {
        let raw = self.embedder_model.as_deref()?.strip_prefix("local:")?;
        Some(std::path::PathBuf::from(raw))
    }
}
```

- [ ] **Step 4: Run and confirm pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib retrieval::config::selection_tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Branch in `client.rs::from_env`**

Replace the embedder construction from Task 1 Step 5 with:

```rust
        let embedder: std::sync::Arc<dyn CodeEmbedder> = match config.local_model_dir() {
            #[cfg(feature = "local-embed-dynamic")]
            Some(dir) => {
                let dir = if dir.is_absolute() {
                    dir
                } else {
                    crate::util::project_root_or_cwd().join(dir)
                };
                std::sync::Arc::new(crate::retrieval::local_onnx::LocalOnnxEmbedder::new(
                    &dir,
                    config.model_dim,
                )?)
            }
            #[cfg(not(feature = "local-embed-dynamic"))]
            Some(_) => {
                return Err(crate::tools::RecoverableError::with_hint(
                    "CODESCOUT_EMBEDDER_MODEL requests a local embedder, but this binary has none compiled in",
                    "Rebuild with --features local-embed-dynamic (windows-gnu) or --features local-embed (msvc/linux/macos).",
                )
                .into())
            }
            None => std::sync::Arc::new(
                EmbedderHttp::new(
                    &config.embedder_url,
                    &config.sparse_embedder_url,
                    config.model_dim,
                )
                .dense_only(dense_only),
            ),
        };
```

If `crate::util::project_root_or_cwd()` does not exist, use the project-root resolution already used by `SqliteVecCodeStore::from_env` and match it exactly — do not introduce a second notion of "project root".

- [ ] **Step 6: Translate the store-side dimension mismatch**

Task 3's check catches *model vs config*. It does not catch *config vs the existing
table*: `code_vec` is `FLOAT[768]` and `vec0` bakes that in at creation, so a 384-d
vector meets a raw arity error from SQLite deep in an insert or query. The spec promised
an actionable message, so translate it at the call boundary in
`src/retrieval/sqlite_code_store.rs` — wrap the `vec0` error from both `upsert_chunks`
and `query`:

```rust
fn explain_dim_mismatch(e: anyhow::Error, configured: usize) -> anyhow::Error {
    let s = e.to_string();
    // sqlite-vec reports dimension errors as an arity/length complaint on the
    // vector argument; it never names the fix.
    if s.contains("dimension") || s.contains("expected") && s.contains("vector") {
        return crate::tools::RecoverableError::with_hint(
            format!("code index dimension mismatch (configured {configured}): {s}"),
            "The vec0 table's dimension is fixed at creation and cannot be migrated. \
             Delete .codescout/code-index.db and run index(action=\"build\") to rebuild \
             at the new dimension.",
        )
        .into();
    }
    e
}
```

Write the test first, against the real store: create a `SqliteVecCodeStore` in a temp
dir, upsert one 3-d vector, then attempt a 4-d upsert and assert the error names
`code-index.db`. A positive control in the same test — a second 3-d upsert succeeding —
keeps "always errors" from passing as success.

- [ ] **Step 7: Gate**

Run both configurations, because the `#[cfg]` branches must each compile:

```
cargo +stable-x86_64-pc-windows-gnu clippy --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu clippy --all-targets --features local-embed-dynamic -- -D warnings
cargo +stable-x86_64-pc-windows-gnu test --lib
```
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add src/retrieval/config.rs src/retrieval/client.rs src/retrieval/sqlite_code_store.rs
git commit -m "feat(retrieval): select the local ONNX embedder via CODESCOUT_EMBEDDER_MODEL"
```

---

### Task 5: Honest `workspace(status)` reporting

`embeddings_model` currently echoes `[embeddings].model` from `project.toml`, which selects nothing. It misled two readers in one session; shipping this feature beside it would leave the field lying in a new way.

**Files:**
- Modify: the `workspace` status builder (find with `grep(pattern="embeddings_model", glob="src/**/*.rs")`)

- [ ] **Step 1: Write the failing test**

```rust
/// The status field must describe what code search will actually use, not what
/// project.toml claims. `[embeddings].model` feeds chunk sizing only.
#[test]
#[serial]
fn status_reports_the_effective_code_search_embedder() {
    use crate::test_support::EnvGuard;
    let _a = EnvGuard::set("CODESCOUT_EMBEDDER_MODEL", "local:/weights/minilm");
    let s = effective_embedder_label();
    assert!(
        s.contains("local:/weights/minilm"),
        "status must name the local model, got: {s}"
    );

    let _b = EnvGuard::unset("CODESCOUT_EMBEDDER_MODEL");
    let _c = EnvGuard::set("CODESCOUT_EMBEDDER_URL", "http://example.invalid:7777");
    let s2 = effective_embedder_label();
    assert!(
        s2.contains("example.invalid:7777"),
        "status must name the HTTP endpoint when no local model is set, got: {s2}"
    );
}
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib status_reports_the_effective_code_search_embedder`
Expected: FAIL — `effective_embedder_label` not defined.

- [ ] **Step 3: Implement**

```rust
/// What the code-retrieval path will actually use, for `workspace(status)`.
pub fn effective_embedder_label() -> String {
    let cfg = match crate::retrieval::config::RetrievalConfig::from_env() {
        Ok(c) => c,
        Err(e) => return format!("misconfigured: {e}"),
    };
    match cfg.embedder_model.as_deref() {
        Some(m) => m.to_string(),
        None => format!("http:{}", cfg.embedder_url),
    }
}
```

Then set the status field from it, and **rename the key** to `code_embedder` so it cannot be confused with the librarian's `[embeddings].model`. Keep reporting the project.toml value under a separate, clearly-named key if it is still wanted for chunk sizing.

- [ ] **Step 4: Run and confirm pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test --lib status_reports_the_effective_code_search_embedder`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git commit -am "fix(workspace): status reports the effective code embedder, not a config echo"
```

---

### Task 6: Migration runbook + tracker

**Files:**
- Modify: `docs/manual/src/configuration/embeddings-edr-windows.md`
- Modify: `docs/trackers/windows-platform-support.md` (via the librarian — `artifact(action="append_entry")`, never a hand edit)

- [ ] **Step 1: Write the runbook section**

Add a section covering, in order: build with `--features local-embed-dynamic`; set `ORT_DYLIB_PATH`; set `CODESCOUT_EMBEDDER_MODEL=local:<dir>` and `CODESCOUT_MODEL_DIM=384`; **delete `.codescout/code-index.db`**; run `index(action="build")`; verify with `semantic_search` and `memory(action="recall")`. State plainly that the delete is destructive and that reindexing back to the old 768-d vectors requires the remote endpoint.

Record the measured egress facts so the next person does not re-derive them: HuggingFace hub/CDN/xethub and `hf-mirror.com` are Zscaler-blocked; the Chroma S3 bundle and `raw.githubusercontent.com` are reachable.

- [ ] **Step 2: Append the tracker entry**

```
artifact(action="append_entry", id="42dfdfc8b1522192", entry_collection="issues",
  id_prefix="WIN",
  entry={
    area: "retrieval-stack",
    status: "fixed",
    summary: "semantic_search and memory(recall) both dialled the default embedder URL 127.0.0.1:8081 with nothing listening, so both were dead on the VDI: the lite stack removed the Qdrant daemon but kept remote embeddings as a hard requirement. Now selectable in-process via CODESCOUT_EMBEDDER_MODEL=local:<dir>, loading side-loaded AllMiniLM-L6-v2 weights through fastembed's try_new_from_user_defined (builds the ONNX session from bytes, so the Zscaler block on every HuggingFace route is irrelevant at runtime). RetrievalClient.embedder went from a concrete EmbedderHttp to Arc<dyn CodeEmbedder>, mirroring the Arc<dyn CodeVectorStore> seam from lite-stack Phase 1; memory recall reaches the same instance through CodeDenseAdapter, so it is fixed by construction rather than a second selection path. Supersedes WIN-22's 'drop local-embed*, use a remote endpoint' mitigation FOR THIS HOST ONLY, on measured evidence: onnxruntime.dll has sat unquarantined since 2026-07-25 and ctypes.CDLL loads it, so neither CrowdStrike nor CyberArk EPM blocks it today. Costs a full reindex — AllMiniLM is 384-d against the existing 768-d table, and vec0 bakes the dimension in at creation",
    ref: "docs/superpowers/specs/2026-08-08-local-onnx-embedder-design.md",
    since: "2026-08-08"
  })
```

Also update WIN-26's Stack B row: embeddings change from "remote OpenAI-compatible only" to "remote OR in-process ONNX", since that row is the sentence this work falsifies.

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs(retrieval): local ONNX runbook and the WIN-22 supersession"
```

---

## Verification

After Task 6, on the VDI:

```
cargo +stable-x86_64-pc-windows-gnu rb --features local-embed-dynamic   # or the lean build
```

then `/mcp`, then confirm both consumers:

- `semantic_search(query="resolve the git bash executable")` returns hits
- `memory(action="recall", query="windows shell")` returns memories

Both were failing with the `127.0.0.1:8081` connect error before this work; both must now answer without any server running. Note `target/release/codescout.exe` is the live MCP binary — rename it aside before building or the link step fails with `os error 5`.

## Known risks carried from the spec

- **Quality.** 384-d general-purpose AllMiniLM on a dense-only stack may retrieve worse than the previous 768-d vectors, worst for exact identifier matches. Benchmark with `scripts/run-tc-benchmark.sh` before treating this as the permanent default.
- **Destructive migration.** The 312 MB index is deleted, and rebuilding the old vectors needs the currently-unreachable endpoint.
- **EDR is a moving target.** WIN-18/22/35 are all cases where policy shifted under the project. Task 3's error text is what keeps a future re-block legible instead of mysterious.
