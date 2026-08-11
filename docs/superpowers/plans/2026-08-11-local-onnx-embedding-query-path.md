---
id: b04a5b6276b37ce6
kind: plan
status: draft
title: Local ONNX embedding reaches the query path — implementation plan
owners:
- marius
tags:
- embeddings
- retrieval
- codescout-embed
- low-resource
- offline
- plan
topic: embedding-backend-selection
---

# Local ONNX Embedding Reaches the Query Path — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `semantic_search` and `memory(recall)` embed in-process via a local ONNX AllMiniLM model, selected through the same `[embeddings]` config the librarian already honours, including on a host with no network.

**Architecture:** `src/retrieval` stops constructing `EmbedderHttp` unconditionally. When no embedder url is configured it resolves through `codescout_embed::create_embedder_with_config` and wraps the result in a dense-only adapter implementing the existing `BatchEmbedder` / `CodeEmbedder` traits. `codescout-embed` gains `LocalEmbedder::from_dir`, which loads weights from a directory and never reaches HuggingFace.

**Tech Stack:** Rust, `fastembed 5` (`try_new_from_user_defined`), `ort` via `local-embed` / `local-embed-dynamic`, `async-trait`, `tokio::task::spawn_blocking`, sqlite-vec.

**Spec:** `docs/superpowers/specs/2026-08-11-local-onnx-embedding-query-path-design.md`

## Global Constraints

- **Base branch:** cut from the head of `feat/local-onnx-embedder` (`b9a67d1d`), which already has current `experiments` merged in. Do **not** cherry-pick onto `experiments` — the trait-object commit predates ~50 commits of drift and the merge already resolved it.
- **Pre-commit gate, every task:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- **Toolchain** is pinned to `1.97.1` (`rust-toolchain.toml`). On the Windows VDI that toolchain cannot link (no MSVC linker) — build and test with `cargo +stable-x86_64-pc-windows-gnu ...` locally. CI uses the pinned toolchain.
- **Never gate a new test on `local-embed-dynamic`.** No test lane enables it (`.github/workflows/ci.yml:157` runs `cargo check` only). New tests belong under `local-embed`, which runs on ubuntu, macOS, and windows (`ci.yml:62`). This is the exact mechanism that let PR #13 reach CI with two compile errors.
- **`RecoverableError` is not available in `codescout-embed`.** It is `crate::tools::RecoverableError` in the root crate only. The crate reports *what happened* via `anyhow` with the concrete path/file in the message; the root wraps it with *what to do* via `RecoverableError::with_hint`. Do not add a codescout dependency to the crate to get around this.
- **`#[async_trait::async_trait]`** on every trait and impl used as `dyn` — native `async fn` in traits is not dyn-compatible.
- **Env-mutating tests** use `EnvGuard` + `#[serial]` (project convention; memory `conventions`).
- **Default cargo features do not change.** `local-embed` stays opt-in.
- **Two paths in this plan do not exist in `experiments` and are deliberately
  written without code spans** — `audit_doc_refs` resolves refs against the
  current checkout and would read them as stale links:
  src/retrieval/local_onnx.rs (exists on the base branch; Task 1 deletes it) and
  docs/manual/src/concepts/local-embedding-offline.md (Task 9 creates it).

---

### Task 1: Remove the superseded implementation

Deletes PR #13's `LocalOnnxEmbedder` and the root `fastembed` dependency. Keeps `25c0a175` (the `CodeEmbedder` trait object) and its review fixes. Ends with the one red CI lane green.

**Files:**
- Delete: `src/retrieval/local_onnx.rs`
- Modify: `src/retrieval/mod.rs` (remove the module gate)
- Modify: `Cargo.toml` (remove the root `fastembed` dep and its two feature edges)
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: nothing.
- Produces: a branch where `cargo check --features local-embed-dynamic --all-targets` succeeds.

- [ ] **Step 1: Cut the working branch**

```bash
git fetch origin feat/local-onnx-embedder
git checkout -b feat/local-onnx-query-path origin/feat/local-onnx-embedder
```

- [ ] **Step 2: Reproduce the failure first**

Run: `cargo check --features local-embed-dynamic --all-targets`
Expected: FAIL with `error[E0596]` at `src/retrieval/local_onnx.rs:82` and `error[E0277]` at `:153`.

Do not skip this. The rest of the task is only meaningful against a reproduced red.

- [ ] **Step 3: Delete the module and its gate**

```bash
git rm src/retrieval/local_onnx.rs
```

In `src/retrieval/mod.rs`, remove these two lines:

```rust
#[cfg(feature = "local-embed-dynamic")]
pub mod local_onnx;
```

- [ ] **Step 4: Remove the root fastembed dependency**

In `Cargo.toml`, delete this block (it sits just above the `reqwest` line):

```toml
# In-process local ONNX embedder (src/retrieval/local_onnx.rs), gated by the
# local-embed / local-embed-dynamic features above. default-features off so
# the ONNX backend (download-binaries vs load-dynamic) is selected by those
# features, mirroring codescout-embed's own fastembed dependency.
fastembed = { version = "5", optional = true, default-features = false }
```

Then restore the two feature lines to their `experiments` form:

```toml
local-embed = ["codescout-embed/local-embed"]
local-embed-dynamic = ["codescout-embed/local-embed-dynamic"]
```

- [ ] **Step 5: Verify the lane is green**

Run: `cargo check --features local-embed-dynamic --all-targets`
Expected: PASS, no warnings.

Run: `cargo test --lib`
Expected: PASS. The `CodeEmbedder` trait and the shared-embedder test from `25c0a175` must still be present and passing — if `memory_embedder_is_built_from_the_shared_code_embedder` is missing, you deleted too much.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(retrieval): drop the root-crate LocalOnnxEmbedder

Superseded by docs/superpowers/specs/2026-08-11-local-onnx-embedding-query-path-design.md:
local ONNX ownership belongs in codescout-embed, which already holds the
TextEmbedding session. Keeps the CodeEmbedder trait seam and the shared
memory-recall embedder; removes the root fastembed dependency that
contradicted Cargo.toml:115.

Fixes the red 'Feature check (opt-in build configs)' lane."
```

---

### Task 2: `LocalEmbedder::from_dir`

Directory-loaded weights in the crate that already owns the ONNX session, with the cache-root repair folded in.

**Files:**
- Modify: `crates/codescout-embed/src/local.rs`
- Test: same file, `mod tests`

**Interfaces:**
- Consumes: the existing `LocalEmbedder { model: Arc<Mutex<fastembed::TextEmbedding>>, dims: usize }` and its `new_blocking` probe pattern.
- Produces:
  - `pub async fn LocalEmbedder::from_dir(dir: &std::path::Path) -> anyhow::Result<Self>`
  - `pub fn resolve_weights_dir(dir: &std::path::Path) -> std::path::PathBuf`
  - `pub const MODEL_FILE: &str = "onnx/model_quantized.onnx"`
  - `pub const REQUIRED_TOKENIZER_FILES: [&str; 4]`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/codescout-embed/src/local.rs`:

```rust
#[test]
fn from_dir_missing_names_the_path_and_the_model_file() {
    let dir = std::path::Path::new("does/not/exist");
    let err = futures::executor::block_on(LocalEmbedder::from_dir(dir))
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
```

Add the dev-dependencies if absent, in `crates/codescout-embed/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
futures = "0.3"
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p codescout-embed --features local-embed local::tests`
Expected: FAIL to compile — `cannot find function from_dir`, `cannot find value MODEL_FILE`.

- [ ] **Step 3: Implement**

Add near the top of `crates/codescout-embed/src/local.rs`, after the existing `use` lines:

```rust
use std::path::{Path, PathBuf};

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
    std::fs::read(&p)
        .map_err(|e| anyhow::anyhow!("cannot read {} ({e})", p.display()))
}
```

Add to `impl LocalEmbedder`:

```rust
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
```

Note `let mut model` — `TextEmbedding::embed` takes `&mut self` in fastembed 5. This is the exact constraint PR #13's plan got wrong; the surrounding `Arc<Mutex<..>>` is what lets the `&self` trait methods work afterwards.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p codescout-embed --features local-embed local::tests`
Expected: PASS, 4 tests.

- [ ] **Step 5: Gate**

Run: `cargo fmt && cargo clippy --all-targets --features local-embed -- -D warnings && cargo test --features local-embed`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/codescout-embed/
git commit -m "feat(embed): load local ONNX weights from a directory

LocalEmbedder::from_dir reads the five weight files directly and builds the
session via try_new_from_user_defined — no hf-hub, no network. A cache-root
path is repaired to its sole snapshot dir; two candidates are left alone
rather than guessed."
```

---

### Task 3: Prove the vectors are real, in CI

`from_dir` returning correctly-shaped garbage passes every test in Task 2. This is the only test that discriminates.

**Files:**
- Modify: `crates/codescout-embed/src/local.rs` (`mod tests`)
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `LocalEmbedder::from_dir`, `MODEL_FILE` (Task 2).
- Produces: env contract `CODESCOUT_TEST_ONNX_DIR` (weights location) and `CODESCOUT_SKIP_ONNX_TESTS` (explicit opt-out).

- [ ] **Step 1: Write the failing test**

```rust
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
    let e = LocalEmbedder::from_dir(&dir).await.expect("weights must load");
    let a = e.embed(&["fn main() {}"]).await.unwrap();
    let b = e.embed(&["fn main() {}"]).await.unwrap();
    assert_eq!(a[0].len(), 384, "AllMiniLM-L6-v2 is 384-dimensional");
    assert_eq!(e.dimensions(), 384, "probe-derived dims must match the vector");
    assert_eq!(a[0], b[0], "same input must give the same vector");
    assert!(
        a[0].iter().any(|x| *x != 0.0),
        "an all-zero vector means the session ran but produced nothing usable"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p codescout-embed --features local-embed from_dir_produces`
Expected: FAIL — panics on the missing `CODESCOUT_TEST_ONNX_DIR`. That failure *is* the contract: absent weights must be loud.

- [ ] **Step 3: Seed the weights in CI**

In `.github/workflows/ci.yml`, inside the test job, add a step **before** the test step, guarded to the `local-embed` config:

```yaml
      - name: Seed ONNX weights (local-embed lane only)
        if: matrix.config.name == 'local-embed'
        shell: bash
        run: |
          set -euo pipefail
          DIR="$HOME/.cache/codescout-onnx/all-MiniLM-L6-v2"
          mkdir -p "$DIR/onnx"
          BASE="https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main"
          curl -fsSL "$BASE/onnx/model_quantized.onnx" -o "$DIR/onnx/model_quantized.onnx"
          for f in tokenizer.json config.json special_tokens_map.json tokenizer_config.json; do
            curl -fsSL "$BASE/$f" -o "$DIR/$f"
          done
          echo "CODESCOUT_TEST_ONNX_DIR=$DIR" >> "$GITHUB_ENV"

      - name: Cache ONNX weights
        if: matrix.config.name == 'local-embed'
        uses: actions/cache@v4
        with:
          path: ~/.cache/codescout-onnx
          key: onnx-all-minilm-l6-v2-q-v1
```

For every config that is **not** `local-embed`, set the opt-out so the test is deliberately skipped rather than accidentally failing:

```yaml
      - name: Opt out of ONNX tests (non-local-embed lanes)
        if: matrix.config.name != 'local-embed'
        shell: bash
        run: echo "CODESCOUT_SKIP_ONNX_TESTS=1" >> "$GITHUB_ENV"
```

- [ ] **Step 4: Verify locally**

```bash
DIR="$HOME/.cache/codescout-onnx/all-MiniLM-L6-v2"
mkdir -p "$DIR/onnx"
BASE="https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main"
curl -fsSL "$BASE/onnx/model_quantized.onnx" -o "$DIR/onnx/model_quantized.onnx"
for f in tokenizer.json config.json special_tokens_map.json tokenizer_config.json; do
  curl -fsSL "$BASE/$f" -o "$DIR/$f"
done
CODESCOUT_TEST_ONNX_DIR="$DIR" cargo test -p codescout-embed --features local-embed from_dir_produces
```

Expected: PASS.

- [ ] **Step 5: Mutation-verify**

Change `pooling: Some(fastembed::Pooling::Mean)` to `None` in `from_dir_blocking`. Re-run the test.
Expected: FAIL (different vector, or a load error).
Record which other tests stayed green — that set is what the suite is blind to. Then revert.

- [ ] **Step 6: Commit**

```bash
git add crates/codescout-embed/src/local.rs .github/workflows/ci.yml
git commit -m "test(embed): prove from_dir vectors on three platforms

The local-embed lane seeds the 22MB AllMiniLM weights and runs the real
embed test; other lanes set the explicit opt-out. Skips only on opt-out,
never on a missing file — a presence-keyed skip is indistinguishable from
a pass."
```

---

### Task 4: `local-dir:` in the model grammar

**Files:**
- Modify: `crates/codescout-embed/src/lib.rs`
- Test: same file

**Interfaces:**
- Consumes: `LocalEmbedder::from_dir` (Task 2).
- Produces: model string `local-dir:<path>` accepted by `create_embedder_with_config`.

**Added 2026-08-11 during execution (Task 4 review finding F2).** `chunk_size_for_model`
(same file, `lib.rs:45-107`) is the grammar's OTHER consumer and has a `local:` arm with no
`local-dir:` arm — so a `local-dir:` spec falls through to substring-matching on the
filesystem PATH. Measured: `local-dir:/opt/weights` → 1305 chars and a HuggingFace cache
path naming nomic → 20889 chars, where 652 is the only correct answer because `from_dir` is
hardcoded to AllMiniLM-L6-v2-Q. The consumer caps at 4096, so that last case ships 4096-char
chunks to a model truncating at 512 tokens — two-thirds of every chunk silently discarded.
This task must add the `local-dir:` arm. Same hub-vs-dir parity class Task 3 closed.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
#[tokio::test]
async fn local_dir_prefix_reports_the_directory_on_failure() {
    let err = create_embedder_with_config("local-dir:/no/such/weights", None, None)
        .await
        .err()
        .expect("missing weights must error")
        .to_string();
    assert!(
        err.contains("/no/such/weights") || err.contains("\\no\\such\\weights"),
        "error must name the directory it was given, got: {err}"
    );
}

#[tokio::test]
async fn unknown_model_error_advertises_local_dir() {
    let err = create_embedder_with_config("banana", None, None)
        .await
        .err()
        .expect("unknown model must error")
        .to_string();
    assert!(
        err.contains("local-dir:"),
        "the unknown-model error must advertise the offline form, got: {err}"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p codescout-embed --features local-embed local_dir`
Expected: FAIL — the `local-dir:` string is treated as an unknown model, and the unknown-model text does not mention it.

- [ ] **Step 3: Implement**

In `create_embedder_with_config`, immediately **before** the existing `// 2. local: prefix` block (it must win, because `local-dir:` also starts with `local`):

```rust
    // 2a. local-dir: prefix — weights from a directory, never the network.
    //     (Corrected 2026-08-11: an earlier draft of this plan claimed the `local:`
    //     arm would otherwise swallow it. That is FALSE and was disproved by
    //     mutation during Task 4's review — `"local-dir:/x".strip_prefix("local:")`
    //     is None, because byte 5 is `-`, not `:`. The arm is needed because
    //     without it `local-dir:` dead-ends in the bare-name fallback and the
    //     catch-all bail, never reaching `from_dir`. Write THAT as the comment.)
    #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
    if let Some(path) = model.strip_prefix("local-dir:") {
        return Ok(Box::new(
            local::LocalEmbedder::from_dir(std::path::Path::new(path)).await?,
        ));
    }
```

Extend the feature-missing bail so it covers both prefixes:

```rust
    if model.starts_with("local:") || model.starts_with("local-dir:") {
        anyhow::bail!(
            "Local embedding requires the 'local-embed' feature.\n\
             Rebuild with: cargo build --features local-embed\n\n\
             Recommended: local:AllMiniLML6V2Q (384d, quantized, 22MB)\n\
             Offline hosts: local-dir:/path/to/weights (no network at all)"
        );
    }
```

And the unknown-model bail gains one line:

```rust
             • Use local-dir:/path/to/weights for an offline host (no network)\n\
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p codescout-embed --features local-embed local_dir unknown_model`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/codescout-embed/src/lib.rs
git commit -m "feat(embed): accept local-dir:<path> in the model grammar

Matched before local: so the shorter prefix does not swallow it. The
feature-missing and unknown-model errors now advertise the offline form."
```

---

### Task 5: The dense-only adapter

**Files:**
- Modify: `src/retrieval/embedder.rs`
- Test: same file

**Interfaces:**
- Consumes: `codescout_embed::Embedder` (`dimensions()`, `embed(&self, &[&str])`), and the `BatchEmbedder` / `CodeEmbedder` traits from `25c0a175`.
- Produces: `pub struct CodeEmbedderAdapter` with `pub fn new(inner: Box<dyn codescout_embed::Embedder>, expected_dim: Option<usize>) -> Self`.

`expected_dim` is `Option` deliberately: `None` means "the model is the authority" (config did not pin one), `Some(n)` means the operator pinned it and a mismatch is an error.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod adapter_tests {
    use super::*;

    struct FakeEmbedder(usize);

    #[async_trait::async_trait]
    impl codescout_embed::Embedder for FakeEmbedder {
        fn dimensions(&self) -> usize {
            self.0
        }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<codescout_embed::Embedding>> {
            Ok(texts.iter().map(|_| vec![0.5_f32; self.0]).collect())
        }
    }

    #[tokio::test]
    async fn adapter_yields_dense_with_empty_sparse() {
        let a = CodeEmbedderAdapter::new(Box::new(FakeEmbedder(384)), None);
        let out = a.embed_one("hello").await.unwrap();
        assert_eq!(out.dense.len(), 384);
        assert!(out.sparse.indices.is_empty(), "local backends have no sparse");
        assert!(out.sparse.values.is_empty());
    }

    #[tokio::test]
    async fn adapter_errors_when_pinned_dim_disagrees_with_the_model() {
        let a = CodeEmbedderAdapter::new(Box::new(FakeEmbedder(384)), Some(768));
        let err = a.embed_one("hello").await.unwrap_err().to_string();
        assert!(err.contains("384"), "must name the produced dim, got: {err}");
        assert!(err.contains("768"), "must name the configured dim, got: {err}");
        assert!(
            err.contains("reindex") || err.contains("rebuild"),
            "must tell the operator the index cannot migrate, got: {err}"
        );
    }

    #[tokio::test]
    async fn adapter_batches_preserve_order_and_arity() {
        let a = CodeEmbedderAdapter::new(Box::new(FakeEmbedder(3)), None);
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = a.embed_batch_dyn(&texts).await.unwrap();
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|o| o.dense.len() == 3));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --features local-embed adapter_tests`
Expected: FAIL to compile — `cannot find type CodeEmbedderAdapter`.

- [ ] **Step 3: Implement**

Append to `src/retrieval/embedder.rs`:

```rust
/// Bridges a `codescout_embed::Embedder` into the retrieval traits.
///
/// Dense-only by construction: local ONNX backends produce no sparse vector,
/// so `sparse` is always empty and callers must run with `dense_only` set.
///
/// The crate reports *what happened* (`anyhow`, naming the path or file); this
/// adapter adds *what to do* (`RecoverableError` with the operator remedy).
/// `RecoverableError` lives in the root crate only — do not push it down.
pub struct CodeEmbedderAdapter {
    inner: Box<dyn codescout_embed::Embedder>,
    /// `None` — the model is the authority. `Some(n)` — the operator pinned a
    /// dimension and a mismatch is an error rather than a discovery.
    expected_dim: Option<usize>,
}

impl CodeEmbedderAdapter {
    pub fn new(inner: Box<dyn codescout_embed::Embedder>, expected_dim: Option<usize>) -> Self {
        Self {
            inner,
            expected_dim,
        }
    }

    /// The dimension downstream callers should build collections with.
    pub fn dimensions(&self) -> usize {
        self.expected_dim.unwrap_or_else(|| self.inner.dimensions())
    }

    fn check_dim(&self, produced: usize) -> anyhow::Result<()> {
        let Some(expected) = self.expected_dim else {
            return Ok(());
        };
        if produced == expected {
            return Ok(());
        }
        Err(crate::tools::RecoverableError::with_hint(
            format!(
                "local embedder dim mismatch: model produced {produced}, configured {expected}"
            ),
            format!(
                "Set CODESCOUT_MODEL_DIM={produced} (or remove it and let the model decide), \
                 then delete the code index and reindex — the vector table bakes the dimension \
                 in at creation and cannot migrate in place."
            ),
        )
        .into())
    }

    fn wrap(&self, dense: Vec<f32>) -> EmbedOutput {
        EmbedOutput {
            dense,
            sparse: SparseVector {
                indices: vec![],
                values: vec![],
            },
        }
    }
}

#[async_trait::async_trait]
impl BatchEmbedder for CodeEmbedderAdapter {
    async fn embed_batch_dyn(&self, texts: &[String]) -> anyhow::Result<Vec<EmbedOutput>> {
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let dense = self.inner.embed(&refs).await?;
        if let Some(first) = dense.first() {
            self.check_dim(first.len())?;
        }
        Ok(dense.into_iter().map(|d| self.wrap(d)).collect())
    }
}

#[async_trait::async_trait]
impl CodeEmbedder for CodeEmbedderAdapter {
    async fn embed_one(&self, text: &str) -> anyhow::Result<EmbedOutput> {
        let dense = self.embed_dense_one(text).await?;
        Ok(self.wrap(dense))
    }

    async fn embed_dense_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut out = self.inner.embed(&[text]).await?;
        let v = out
            .pop()
            .ok_or_else(|| anyhow::anyhow!("local embedder returned no vector"))?;
        self.check_dim(v.len())?;
        Ok(v)
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --features local-embed adapter_tests`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add src/retrieval/embedder.rs
git commit -m "feat(retrieval): dense-only adapter over codescout_embed::Embedder

Wraps the crate's Embedder into BatchEmbedder + CodeEmbedder with an empty
sparse vector. Dim validation stays here per the 2026-07-25 ADR: the crate
discovers, the root validates."
```

---

### Task 6: Config merge and the `from_env` signature

The blast-radius task. `from_env` takes a project root so the compiler forces every call site to decide — the alternative (an optional second constructor) is how the index-state sidecar shipped dead in its primary path.

**Files:**
- Modify: `src/retrieval/config.rs`
- Modify: `src/retrieval/client.rs`
- Modify: `src/agent/mod.rs:1581`, `:1742`
- Modify: `src/tools/config/mod.rs:330`, `:432`
- Modify: `src/bin/sync_project.rs:17`
- Modify: `src/dashboard/api/index.rs:14`
- Modify: `src/tools/memory/mod.rs:402`
- Test: `src/retrieval/config.rs`

**Interfaces:**
- Consumes: `EmbeddingsSection { model: String, url: Option<String>, api_key: Option<SensitiveString> }` (`src/config/project.rs:44`), `ProjectConfig::load_or_default(&Path)`.
- Produces:
  - `RetrievalConfig.embedder_url: Option<String>` (was `String`)
  - `RetrievalConfig.model: String`, `RetrievalConfig.api_key: Option<String>`
  - `RetrievalConfig::from_env_and_project(root: Option<&Path>) -> Result<Self>`
  - `RetrievalClient::from_env(root: Option<&Path>) -> Result<Self>`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod merge_tests {
    use super::*;
    use crate::test_support::EnvGuard;
    use serial_test::serial;

    #[test]
    #[serial]
    fn unset_url_no_longer_fabricates_8081() {
        let _g = EnvGuard::unset("CODESCOUT_EMBEDDER_URL");
        let cfg = RetrievalConfig::from_env_and_project(None).unwrap();
        assert_eq!(
            cfg.embedder_url, None,
            "an unset url must mean 'resolve from the model', not 'assume 8081'"
        );
    }

    #[test]
    #[serial]
    fn env_url_overrides_project_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            "[embeddings]\nmodel = \"local:AllMiniLML6V2Q\"\nurl = \"http://from-toml:9/v1\"\n",
        )
        .unwrap();
        let _g = EnvGuard::set("CODESCOUT_EMBEDDER_URL", "http://from-env:8/v1");
        let cfg = RetrievalConfig::from_env_and_project(Some(dir.path())).unwrap();
        assert_eq!(cfg.embedder_url.as_deref(), Some("http://from-env:8/v1"));
    }

    #[test]
    #[serial]
    fn project_model_reaches_retrieval_when_env_is_silent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            "[embeddings]\nmodel = \"local-dir:/weights\"\n",
        )
        .unwrap();
        let _g = EnvGuard::unset("CODESCOUT_EMBEDDER_URL");
        let cfg = RetrievalConfig::from_env_and_project(Some(dir.path())).unwrap();
        assert_eq!(cfg.model, "local-dir:/weights");
        assert_eq!(cfg.embedder_url, None);
    }

    #[test]
    #[serial]
    fn unset_model_dim_is_none_not_768() {
        let _g = EnvGuard::unset("CODESCOUT_MODEL_DIM");
        let cfg = RetrievalConfig::from_env_and_project(None).unwrap();
        assert_eq!(
            cfg.model_dim, None,
            "an unpinned dim must let the model decide"
        );
    }
}
```

If `EnvGuard::unset` does not exist, add it beside the existing `EnvGuard::set` in the test-support module, following the same restore-on-drop shape.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib merge_tests`
Expected: FAIL to compile — `from_env_and_project` not found; `embedder_url` is `String` not `Option`.

- [ ] **Step 3: Change the config struct**

In `src/retrieval/config.rs`, change three fields and add two:

```rust
    /// `None` means "no url configured" — resolve the backend from `model`.
    /// Previously defaulted to `http://127.0.0.1:8081`, which fabricated a
    /// server that may never have existed. An explicit env value is untouched.
    pub embedder_url: Option<String>,
    /// `None` means "the model is the authority". `Some(n)` is an operator pin.
    pub model_dim: Option<usize>,
    /// Model identifier in codescout-embed's grammar (`local:`, `local-dir:`,
    /// `ollama:`, `openai:`, or a bare name sent to `embedder_url`).
    pub model: String,
    /// Embedding API key, used only when `embedder_url` is set.
    pub api_key: Option<String>,
```

Replace `from_env` with:

```rust
    /// Env-only construction. Equivalent to `from_env_and_project(None)`.
    pub fn from_env() -> Result<Self> {
        Self::from_env_and_project(None)
    }

    /// `[embeddings]` in the project's config is the base; `CODESCOUT_*` env
    /// vars override it. Benchmark matrix cells set env, so they are unaffected.
    pub fn from_env_and_project(root: Option<&std::path::Path>) -> Result<Self> {
        let embeddings = root
            .and_then(|r| crate::config::project::ProjectConfig::load_or_default(r).ok())
            .map(|c| c.embeddings);
        let (cfg_model, cfg_url, cfg_key) = match embeddings {
            Some(e) => (
                Some(e.model),
                e.url,
                e.api_key.map(|k| k.as_str().to_string()),
            ),
            None => (None, None, None),
        };
        Ok(Self {
            qdrant_url: std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".into()),
            embedder_url: std::env::var("CODESCOUT_EMBEDDER_URL").ok().or(cfg_url),
            model: std::env::var("CODESCOUT_EMBEDDER_MODEL")
                .ok()
                .or(cfg_model)
                .unwrap_or_else(crate::config::project::default_embed_model),
            api_key: std::env::var("EMBED_API_KEY").ok().or(cfg_key),
            model_dim: std::env::var("CODESCOUT_MODEL_DIM")
                .ok()
                .and_then(|s| s.parse().ok()),
            // ... every remaining field unchanged from the current from_env body ...
        })
    }
```

Copy the remaining fields (`sparse_embedder_url`, `reranker_url`, `profile`, `bm25_boost`, `disable_sparse`, `rerank`, `collection_prefix`) verbatim from the existing `from_env`, including their comments.

`SensitiveString::as_str()` (`src/config/sensitive.rs:27`) is the read accessor. Do not add another.

- [ ] **Step 4: Fix every call site**

`RetrievalClient::from_env` becomes `from_env(root: Option<&Path>)` and forwards to `from_env_and_project`. Then update all seven — the compiler will list them:

| Site | Pass |
|---|---|
| `src/agent/mod.rs:1581` | `self.project_root().await.as_deref()` |
| `src/agent/mod.rs:1742` | `self.project_root().await.as_deref()` |
| `src/tools/config/mod.rs:330` | the project root already in scope for the status call |
| `src/tools/config/mod.rs:432` | same |
| `src/bin/sync_project.rs:17` | `Some(&root)` — the root already loaded at `:23` |
| `src/dashboard/api/index.rs:14` | `Some(&state.project_root)` |
| `src/tools/memory/mod.rs:402` | the project root in scope for the memory tool |

**Pass `None` nowhere.** If a site genuinely has no root, that is a finding — stop and report it rather than papering over it with `None`, because `None` means "local embedding cannot work through this path".

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib merge_tests && cargo build --all-targets`
Expected: PASS and a clean build with zero call sites left.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(retrieval): [embeddings] config reaches the query path

RetrievalConfig gains model/api_key and makes embedder_url + model_dim
Option. Project config is the base, CODESCOUT_* env overrides. from_env
now takes a project root so the compiler forces all seven call sites to
decide rather than silently inheriting env-only behaviour.

Removes the fabricated http://127.0.0.1:8081 default: an unset url now
means 'resolve from the model'."
```

---

### Task 7: The selection branch

**Files:**
- Modify: `src/retrieval/client.rs`
- Test: same file

**Interfaces:**
- Consumes: `CodeEmbedderAdapter::new` (Task 5), `RetrievalConfig` fields (Task 6), `codescout_embed::create_embedder_with_config` (Task 4).
- Produces: `RetrievalClient.embedder` populated from either backend; `RetrievalClient::backend_is_local() -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::test_support::EnvGuard;
    use serial_test::serial;

    fn cfg_with(url: Option<&str>, model: &str) -> RetrievalConfig {
        let mut c = RetrievalConfig::from_env_and_project(None).unwrap();
        c.embedder_url = url.map(|s| s.to_string());
        c.model = model.to_string();
        c
    }

    #[test]
    fn explicit_url_selects_the_http_backend_regardless_of_model() {
        let c = cfg_with(Some("http://127.0.0.1:8081/v1"), "local:AllMiniLML6V2Q");
        assert!(!RetrievalClient::backend_is_local(&c));
    }

    #[test]
    fn no_url_with_a_local_model_selects_the_local_backend() {
        let c = cfg_with(None, "local-dir:/weights");
        assert!(RetrievalClient::backend_is_local(&c));
    }

    #[test]
    #[serial]
    async fn local_backend_with_sparse_expected_is_an_error() {
        let _g = EnvGuard::unset("CODESCOUT_DISABLE_SPARSE");
        let mut c = cfg_with(None, "local-dir:/weights");
        c.disable_sparse = false;
        let err = RetrievalClient::guard_sparse(&c, /* lite */ false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("sparse"),
            "must name sparse as the conflict, got: {err}"
        );
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib selection_tests`
Expected: FAIL to compile — `backend_is_local` and `guard_sparse` not found.

- [ ] **Step 3: Implement**

In `src/retrieval/client.rs`, add two associated functions and rewrite the embedder construction inside `from_env`:

```rust
    /// A local backend is selected when no url is configured and the model
    /// names one. Keep this the single source of truth — `dense_only` and the
    /// sparse guard both read it.
    pub(crate) fn backend_is_local(config: &RetrievalConfig) -> bool {
        config.embedder_url.is_none()
            && (config.model.starts_with("local:") || config.model.starts_with("local-dir:"))
    }

    /// A local backend emits no sparse vector. Silently dropping to dense would
    /// show up as degraded recall and never as a failure, so it is an error.
    pub(crate) fn guard_sparse(config: &RetrievalConfig, lite: bool) -> Result<()> {
        if Self::backend_is_local(config) && !lite && !config.disable_sparse {
            anyhow::bail!(
                "the local embedding backend produces no sparse vector, but the hybrid \
                 sparse leg is enabled.\n\
                 Either set CODESCOUT_DISABLE_SPARSE=1 to run dense-only, or configure \
                 an embedder url that serves both dense and sparse."
            );
        }
        Ok(())
    }
```

Replace the embedder block in `from_env`:

```rust
        Self::guard_sparse(&config, lite)?;
        let backend_is_local = Self::backend_is_local(&config);
        // Local backends are dense-only by construction; so is the lite stack,
        // and so is an explicit sparse opt-out.
        let dense_only = lite || config.disable_sparse || backend_is_local;
        let embedder: Arc<dyn CodeEmbedder> = if let Some(url) = config.embedder_url.as_deref() {
            Arc::new(
                EmbedderHttp::new(url, &config.sparse_embedder_url, config.model_dim.unwrap_or(768))
                    .dense_only(dense_only),
            )
        } else {
            let inner = codescout_embed::create_embedder_with_config(
                &config.model,
                None,
                config.api_key.clone(),
            )
            .await
            .map_err(|e| {
                crate::tools::RecoverableError::with_hint(
                    format!("could not build the '{}' embedder: {e}", config.model),
                    "Set [embeddings].url (or CODESCOUT_EMBEDDER_URL) to an \
                     OpenAI-compatible endpoint, or rebuild with --features local-embed \
                     for in-process ONNX. For a host with no network, point \
                     [embeddings].model at local-dir:/path/to/weights. \
                     If this is a dylib load error, set ORT_DYLIB_PATH to onnxruntime.dll \
                     — and note that an `os error 5` here is application control (e.g. \
                     CyberArk EPM) denying the load, not a missing file: the DLL is \
                     present but not permitted to execute.",
                )
            })?;
            Arc::new(crate::retrieval::embedder::CodeEmbedderAdapter::new(
                inner,
                config.model_dim,
            ))
        };
```

Apply the same `embedder_url` / `model_dim` `Option` handling to `from_config_only`, which is `#[cfg(feature = "server-stack")]` and always the Qdrant shape — it keeps `EmbedderHttp` unconditionally, using `config.embedder_url.as_deref().unwrap_or("http://127.0.0.1:8081")` so the test/validation constructor keeps its old behaviour explicitly rather than by accident.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib selection_tests`
Expected: PASS.

- [ ] **Step 5: Full gate**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/retrieval/client.rs
git commit -m "feat(retrieval): select the local backend when no url is configured

url present -> EmbedderHttp, unchanged. url absent -> the crate resolver,
wrapped in the dense-only adapter. Local implies dense_only; a local
backend with the sparse leg still enabled is an error rather than a
silent downgrade."
```

---

### Task 8: Surface the stored index dimension

`SqliteVecCodeStore::ensure_vec_table` (`src/retrieval/sqlite_code_store.rs:92`) **already** reads the stored dim back via `length(embedding)/4` and bails on mismatch. This task exposes that through the trait and makes the failure legible earlier — it does **not** build a second guard beside it.

**Two corrections to the spec, found while writing this task. Both are load-bearing:**

1. `SqliteVecCodeStore::conn_for` takes a **`project_id`**, not a collection (`:70`) — the store is per-project. So the trait method needs both identifiers, matching `chunk_refs(&self, collection, project_id)`.
2. Because `project_id` is not known at `RetrievalClient::from_env`, the spec's *startup* guard is not implementable for the sqlite backend. The guard fires instead at the first operation that has a `project_id` — the entry to `sync_project` and to `search_in`. Same failure, same message, one call later.

**Files:**
- Modify: `src/retrieval/code_store.rs` (trait + `QdrantWrap` impl + `InMemoryCodeStore` impl at `:238`)
- Modify: `src/retrieval/sqlite_code_store.rs`
- Modify: `src/retrieval/sync.rs` (`RecordingStore:433`, `SlowEnsureStore:842`, plus the guard call)
- Modify: `src/retrieval/search.rs` (guard call)
- Test: `src/retrieval/sqlite_code_store.rs`

**Interfaces:**
- Consumes: the existing test helper `payload(id, project, file, lang, hash) -> CodePayload` (`src/retrieval/sqlite_code_store.rs:363`).
- Produces:
  - `async fn collection_dim(&self, collection: &str, project_id: &str) -> Result<Option<u64>>` on `CodeVectorStore`
  - `RetrievalClient::guard_index_dim(&self, collection: &str, project_id: &str) -> Result<()>`

- [ ] **Step 1: Write the failing test**

Add to the test module in `src/retrieval/sqlite_code_store.rs`, alongside the existing `real_vec0_*` tests:

```rust
#[tokio::test]
async fn collection_dim_reports_none_then_the_baked_dim() {
    let store = /* same constructor the neighbouring real_vec0_* tests use */;
    assert_eq!(
        store.collection_dim("code_chunks", "proj").await.unwrap(),
        None,
        "no table yet must be None, not an error"
    );
    let p = payload("c1", "proj", "a.rs", "rust", "h1");
    let e = EmbedOutput {
        dense: vec![0.1, 0.2, 0.3],
        sparse: SparseVector { indices: vec![], values: vec![] },
    };
    store.upsert_chunks("code_chunks", &[(p, e)]).await.unwrap();
    assert_eq!(
        store.collection_dim("code_chunks", "proj").await.unwrap(),
        Some(3),
        "vec0 bakes the dim at creation — report what it baked"
    );
}
```

Use the exact store constructor the neighbouring `real_vec0_upsert_query_orders_by_distance` test uses (`:396`); it already handles temp-dir setup.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib collection_dim`
Expected: FAIL to compile — no `collection_dim` on the trait.

- [ ] **Step 3: Add the trait method, with no default**

In `src/retrieval/code_store.rs`, inside `pub trait CodeVectorStore`:

```rust
    /// Dense dimension this project's collection was created with, or `None`
    /// when it does not exist yet.
    ///
    /// Takes `project_id` as well as `collection` because the sqlite-vec store
    /// is per-project — `conn_for` keys on the project, not the collection.
    ///
    /// Deliberately has **no default implementation**: a backend that silently
    /// inherited `Ok(None)` would disable the dim guard with no diagnostic.
    /// Every implementor answers explicitly, so a new backend fails to compile
    /// rather than failing quietly.
    async fn collection_dim(&self, collection: &str, project_id: &str) -> Result<Option<u64>>;
```

- [ ] **Step 4: Implement for all five**

`SqliteVecCodeStore` — reuse the probe shape already in `ensure_vec_table`:

```rust
    async fn collection_dim(&self, _collection: &str, project_id: &str) -> Result<Option<u64>> {
        use rusqlite::OptionalExtension;
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        let present: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_vec'",
                [],
                |_| Ok(true),
            )
            .optional()
            .context("probe code_vec existence")?
            .unwrap_or(false);
        if !present {
            return Ok(None);
        }
        let blob_len: Option<i64> = conn
            .query_row("SELECT length(embedding) FROM code_vec LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()
            .context("read existing code_vec dim")?;
        Ok(blob_len.map(|n| (n / 4) as u64))
    }
```

Match the lock/deref idiom the other methods in this impl use for `conn_for`'s `Arc<Mutex<Connection>>` — copy it from `chunk_refs`, do not invent one.

`QdrantWrap` (`src/retrieval/code_store.rs:131`, `#[cfg(feature = "server-stack")]`) — ask the server for the collection's configured vector size, mapping a missing collection to `Ok(None)`:

```rust
    async fn collection_dim(&self, collection: &str, _project_id: &str) -> Result<Option<u64>> {
        // Qdrant collections are shared across projects, so project_id is unused
        // here — the dimension is a property of the collection itself.
        match self.client.collection_info(collection).await {
            Ok(info) => Ok(dense_vector_size(&info)),
            // A missing collection is "nothing indexed yet", not an error.
            Err(_) => Ok(None),
        }
    }
```

Write `dense_vector_size` as a small private helper in the same file that walks the `CollectionInfoResponse` down to the dense vector's `size`. **Confirm the field path against the vendored qdrant-client 1.13 source before writing it** — the nesting differs between minor versions, and a guess here compiles only if you happen to be right. If the path proves awkward, returning `Ok(None)` with a `tracing::debug!` is an acceptable first implementation: Qdrant already rejects a wrong-dimension upsert server-side, so this backend loses less by abstaining than sqlite does.

The three test doubles answer explicitly:

```rust
    async fn collection_dim(&self, _collection: &str, _project_id: &str) -> Result<Option<u64>> {
        Ok(None)
    }
```

- [ ] **Step 5: Wire the guard where `project_id` exists**

Add to `impl RetrievalClient` in `src/retrieval/client.rs`:

```rust
    /// Fail legibly when the configured embedder disagrees with what the index
    /// already holds. Called at the entry to indexing and to search, which is
    /// the first point `project_id` is known — client construction does not
    /// have one (the sqlite store is per-project).
    pub(crate) async fn guard_index_dim(&self, collection: &str, project_id: &str) -> Result<()> {
        let Some(index_dim) = self.code_store.collection_dim(collection, project_id).await? else {
            return Ok(());
        };
        let model_dim = self.config.model_dim.unwrap_or(index_dim as usize);
        if model_dim as u64 == index_dim {
            return Ok(());
        }
        Err(crate::tools::RecoverableError::with_hint(
            format!(
                "code index was built at {index_dim} dimensions; the configured \
                 embedder produces {model_dim}"
            ),
            "Delete the code index and reindex — the vector table bakes the dimension \
             in at creation and cannot migrate in place. Or set [embeddings].model back \
             to the model the index was built with.",
        )
        .into())
    }
```

Call it once at the top of `sync_project` (`src/retrieval/sync.rs`) and once at the top of `search_in` (`src/retrieval/search.rs`), passing the same collection name each already computes — including `config.collection_prefix` if that path applies it.

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib collection_dim && cargo test`
Expected: PASS.

- [ ] **Step 7: Mutation-verify**

Change the guard's comparison to `>=`. Re-run.
Expected: the dim test fails. Record which siblings stayed green, then revert.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(retrieval): report an index's stored dimension and guard against a model switch

Exposes what ensure_vec_table already computed, so a model switch fails at
the entry to index/search with both numbers and a remedy, instead of mid-
index with a vector-length error. No default trait method: a new backend
must answer explicitly."
```
---

### Task 9: Report the truth, and document the path

**Files:**
- Modify: `src/tools/config/mod.rs` (`ProjectStatus`)
- Create: docs/manual/src/concepts/local-embedding-offline.md (does not exist yet, so un-spanned)
- Modify: `docs/manual/src/SUMMARY.md` — add as a child of the existing `- [Semantic Search](concepts/semantic-search.md)` group (line 27), beside `lite-stack.md` and `retrieval-stack.md`
- Test: `src/tools/config/tests.rs`

**Interfaces:**
- Consumes: `RetrievalConfig.model`, `RetrievalConfig.embedder_url`, `RetrievalClient::backend_is_local` (Task 7).
- Produces: an `embedding` block in `workspace(action="status")` naming the live backend and the compiled-in ones.

- [ ] **Step 1: Write the failing test**

`ProjectStatus` returns **flat** config fields — `project_status_compact_shape`
(`src/tools/config/tests.rs:238`) asserts *"config blob must be removed"* and
`result["embeddings_model"].is_string()`. Extend the flat shape beside that
field; a nested `embedding` object would violate a convention this file pins
with a test.

```rust
#[tokio::test]
async fn status_reports_the_live_backend_and_what_is_compiled_in() {
    // Build `ctx` with the same ToolContext literal `project_status_compact_shape`
    // uses at src/tools/config/tests.rs:206-238. Copy it verbatim rather than
    // reconstructing it — the struct has required fields (lsp, output_buffer,
    // progress, peer, section_coverage, guide_hints_emitted, workspace_override)
    // that a partial literal will not compile without.
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    assert!(
        result["embeddings_model"].is_string(),
        "the existing flat field must survive"
    );
    assert!(
        result["embedding_backend"].is_string(),
        "must name the live backend"
    );
    assert!(
        result["embedding_compiled_in"].is_array(),
        "must name which backends this binary can actually use"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib status_reports_the_live_backend`
Expected: FAIL — no `embedding` key.

- [ ] **Step 3: Implement**

In `ProjectStatus::call`, add to the response:

```rust
    let mut compiled_in = Vec::new();
    if cfg!(feature = "remote-embed") {
        compiled_in.push("remote");
    }
    if cfg!(any(feature = "local-embed", feature = "local-embed-dynamic")) {
        compiled_in.push("local-onnx");
    }
    let backend = if retrieval_config.embedder_url.is_some() {
        "remote-http"
    } else if compiled_in.contains(&"local-onnx") {
        "local-onnx"
    } else {
        // The default config names a local model this binary cannot load.
        // Saying so here is the whole point of this block.
        "unavailable"
    };
```

Then insert them as **flat keys** into the response object the function already
builds, beside the existing `embeddings_model`:

```rust
    obj.insert("embedding_backend".into(), json!(backend));
    obj.insert("embedding_compiled_in".into(), json!(compiled_in));
    if backend == "unavailable" {
        obj.insert(
            "embedding_hint".into(),
            json!(
                "This binary has no local embedding backend compiled in, but the \
                 configured model names one. Rebuild with --features local-embed, \
                 or set [embeddings].url to an OpenAI-compatible endpoint."
            ),
        );
    }
```

Use whatever the surrounding code already does to assemble the response — if it
builds a `serde_json::Map` insert into that; if it builds one `json!({...})`
literal, add the three keys inside it. Do not restructure the response.

- [ ] **Step 4: Write the manual page**

Create docs/manual/src/concepts/local-embedding-offline.md. The manual has no
`guide/` directory — every prose page lives under `concepts/`, and this one
belongs beside `concepts/lite-stack.md` and `concepts/retrieval-stack.md`,
which already cover the two existing retrieval configurations.

````markdown
# Embedding without a server

codescout can embed in-process with a small ONNX model — no GPU, no embedding
server, no daemon. This is the recommended setup for a laptop, a CI box, or any
host that cannot reach an embedding endpoint.

## Build with the local backend

Local ONNX is opt-in at compile time:

```bash
cargo build --release --features local-embed
```

On windows-gnu (MinGW), `ort` ships no prebuilt runtime — use
`--features local-embed-dynamic` and supply `onnxruntime.dll` at runtime via
`ORT_DYLIB_PATH`.

Check what your binary actually has:

```
workspace(action="status")   →   embedding.compiled_in
```

If that reads `["remote"]`, the binary cannot embed locally no matter what the
config says.

## Configure the model

```toml
# .codescout/project.toml
[embeddings]
model = "local:AllMiniLML6V2Q"   # 384d, INT8-quantized, ~22MB
```

On first use this fetches the weights to the HuggingFace cache. Nothing else is
needed — leave `url` unset, and both `semantic_search` and `memory(recall)`
will embed in-process.

## Hosts with no network

Download the five files on a machine that has network, copy the directory over,
and point the config at it:

```
<dir>/onnx/model_quantized.onnx
<dir>/tokenizer.json
<dir>/config.json
<dir>/special_tokens_map.json
<dir>/tokenizer_config.json
```

```toml
[embeddings]
model = "local-dir:/opt/codescout/weights/all-MiniLM-L6-v2"
```

`local-dir:` never contacts HuggingFace. If you point it at a HuggingFace cache
root by mistake and it contains exactly one snapshot, codescout descends into it
and logs what it resolved.

## Switching models on an existing index

The vector table bakes its dimension in at creation. Changing to a model with a
different dimension requires deleting the index and reindexing — codescout
refuses at startup with both numbers rather than failing later.
````

Add it to `docs/manual/src/SUMMARY.md` as a child of the Semantic Search group,
matching the two-space indent its siblings use:

```
  - [Local Embedding (offline)](concepts/local-embedding-offline.md)
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib status_reports_the_live_backend`
Expected: PASS.

- [ ] **Step 6: Full gate and doc audit**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
Run: `librarian(action="audit_doc_refs")` — the new manual page must introduce no unresolved refs.
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(status): report the live embedding backend + document the low-resource path

workspace(status) now names the configured model, the live backend, and
which backends the binary was compiled with — so a lean build stops
silently disagreeing with a config that names a local model."
```

---

## Verification (whole branch)

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy --all-targets --features local-embed -- -D warnings`
- `cargo test`
- `cargo check --features local-embed-dynamic --all-targets` — the lane PR #13 broke
- `cargo test --features local-embed` with `CODESCOUT_TEST_ONNX_DIR` set — the real-embed test must run, not skip
- Live MCP check: `cargo rb`, reconnect, then `semantic_search(query="resolve the git bash executable")` and `memory(action="recall", query="windows shell")` both answer with no embedding server running

## Deviations from the spec — both need a ruling

**1. No `corrections` note on the cache-root repair.** The spec calls for one.
`corrections` rides on MCP tool responses, but the repair happens during
embedder construction, which is not a tool-response boundary — threading it out
would mean plumbing an advisory from `RetrievalClient::from_env` through every
caller. Task 2 implements the repair with a `tracing::info!` instead. If the
note matters, the honest place for it is a follow-up that gives client
construction a way to return advisories, not an ad-hoc channel.

**2. The dim guard fires at first use, not at startup.** The spec placed it at
client construction. `SqliteVecCodeStore::conn_for` keys on `project_id`
(`src/retrieval/sqlite_code_store.rs:70`), which `RetrievalClient::from_env`
does not have — the store is per-project and the client is not. Task 8 therefore
calls `guard_index_dim` at the entry to `sync_project` and `search_in`, the
first points where `project_id` exists. Same check, same message, one call
later. Nothing in the spec's intent is lost, but the spec text should be
corrected rather than left describing something the substrate cannot do.
