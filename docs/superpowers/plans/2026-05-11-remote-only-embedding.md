# Remote-Only Embedding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `LocalEmbedder` (fastembed + ONNX runtime) from `codescout-embed`. Make `RemoteEmbedder` the sole production implementor of the `Embedder` trait, with a deterministic `MockEmbedder` for unit tests.

**Architecture:** Subtract `local.rs` + `fastembed` dep. Keep the `Embedder` trait. Add a cfg-gated `MockEmbedder`. Replace per-model `chunk_size_for_model` derivation with a fixed 1600-char default + user `[embeddings] chunk_size` override. Preserve AST leaf-symbol boundaries even when they exceed the target.

**Tech Stack:** Rust 2021, `async-trait`, `reqwest`, `anyhow`, `serde`, `tokio`, `rusqlite` (sqlite-vec), tree-sitter (AST chunker).

**Spec:** `docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md`

---

## File Structure

### Files to create

- `crates/codescout-embed/src/mock.rs` — `MockEmbedder` deterministic test double (Phase 1)
- `tests/embedding_integration.rs` — `#[ignore]`-gated integration smoke tests (Phase 2)
- `docs/adrs/2026-05-11-remote-only-embedding.md` — Architecture Decision Record (Phase 5)

### Files to modify

- `crates/codescout-embed/src/lib.rs` — drop `local:` branch in factory; replace `chunk_size_for_model` with `DEFAULT_CHUNK_SIZE_CHARS` constant; remove `mod local`
- `crates/codescout-embed/Cargo.toml` — remove `fastembed` dep + `local-embed` feature
- `crates/codescout-embed/src/embedder.rs` — unchanged (verify only)
- `src/embed/index.rs` — `check_model_mismatch` auto-wipes when stored model has `local:` prefix and configured does not
- `src/config/project.rs` — `EmbeddingsSection::effective_chunk_size` returns `chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE_CHARS)`; drop the model-max cap
- `src/embed/ast_chunker.rs` — `enforce_max_chunk_size` becomes `prefer_chunk_size`; leaf symbols never truncated
- `src/embed/mod.rs` — drop `chunk_size_for_model` re-export, drop its tests
- `src/prompts/server_instructions.md` — remove `local:` examples; document required `url`
- `src/prompts/onboarding_prompt.md` — same
- `src/prompts/builders.rs` (`build_system_prompt_draft`) — same
- `src/tools/onboarding.rs` — bump `ONBOARDING_VERSION`
- `README.md` — embedding setup section
- `docs/ARCHITECTURE.md` — embedding stack section
- `docs/manual/src/concepts/embedding.md` (if present) — same
- `Cargo.toml` (workspace root) — major version bump
- `CHANGELOG.md` — major version entry
- `docs/trackers/retrieval-benchmark.md` — append new `### YYYY-MM-DD` history entry after Phase 4 benchmark run

### Files to delete

- `crates/codescout-embed/src/local.rs` — fastembed wrapper (Phase 3)

---

## Task 1: Phase 0 — Audit (no commit)

**Files:**
- Read only: `src/`, `crates/`, `tests/`

- [ ] **Step 1: Baseline test count**

Run:
```bash
cargo test 2>&1 | tail -20
```
Record `test result: ok. N passed` in a scratch note. This is the baseline. Every later phase must not regress this count (modulo deliberate test deletions).

- [ ] **Step 2: Enumerate `LocalEmbedder` and fastembed usage**

Run:
```bash
grep -rn "LocalEmbedder\|fastembed\|local::\|local:" --include="*.rs" --include="*.toml" src/ crates/ tests/ | grep -v target/
```

Save the output to a scratch file (NOT committed) — used during Phase 2 to migrate test files one by one.

- [ ] **Step 3: Classify each `LocalEmbedder` test reference**

For every test file that constructs `LocalEmbedder`, classify into:

- **Plumbing** — asserts on `chunk.id`, `chunk.start_line`, `len(results)`, presence in DB, factory wiring
- **Quality** — asserts on `result.score`, top-k ranking, semantic equivalence, `results[0].id == "..."`
- **Implementation-coupled** — asserts on fastembed internals or `local:` model names

Record the classification next to each path in the scratch file.

- [ ] **Step 4: No commit for Phase 0**

Audit is informational. Move to Task 2.

---

## Task 2: MockEmbedder — module skeleton + failing test

**Files:**
- Create: `crates/codescout-embed/src/mock.rs`
- Modify: `crates/codescout-embed/src/lib.rs`
- Modify: `crates/codescout-embed/Cargo.toml`

- [ ] **Step 1: Add `test-mock` feature in Cargo.toml**

Edit `crates/codescout-embed/Cargo.toml`. Under `[features]`, add a line below the existing `local-embed = ...` entry:

```toml
# Deterministic in-process embedder for unit tests. No native deps.
test-mock = []
```

- [ ] **Step 2: Create empty `mock.rs` with cfg gate**

Create `crates/codescout-embed/src/mock.rs`:

```rust
//! Deterministic test embedder. Produces orthogonal unit vectors derived
//! from a stable hash of the input text. Intentionally returns vectors
//! that are NEAR-ORTHOGONAL between distinct inputs so that any test
//! asserting on semantic similarity ranking will fail — forcing test
//! authors to assert on plumbing (chunk emission, vector storage, factory
//! wiring) rather than retrieval quality.

use crate::embedder::{Embedder, Embedding};
use anyhow::Result;
use std::hash::{Hash, Hasher};

pub struct MockEmbedder {
    dims: usize,
}

impl MockEmbedder {
    pub fn new(dims: usize) -> Self {
        assert!(dims > 0, "MockEmbedder requires dims > 0");
        Self { dims }
    }
}

#[async_trait::async_trait]
impl Embedder for MockEmbedder {
    fn dimensions(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        Ok(texts.iter().map(|t| vector_for(t, self.dims)).collect())
    }
}

fn vector_for(text: &str, dims: usize) -> Embedding {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let mut state = h.finish();
    let mut v = Vec::with_capacity(dims);
    for _ in 0..dims {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let bits = (state >> 33) as u32;
        let f = (bits as f32 / u32::MAX as f32) * 2.0 - 1.0;
        v.push(f);
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}
```

- [ ] **Step 3: Register module in lib.rs**

Edit `crates/codescout-embed/src/lib.rs`. Locate the existing module declarations near the top:

```rust
pub mod chunker;

pub mod embedder;

#[cfg(feature = "local-embed")]
pub mod local;

#[cfg(feature = "remote-embed")]
pub mod remote;
```

Add after the `remote` module declaration:

```rust
#[cfg(any(test, feature = "test-mock"))]
pub mod mock;
```

- [ ] **Step 4: Write the failing self-test in mock.rs**

Append to `crates/codescout-embed/src/mock.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_is_deterministic() {
        let e = MockEmbedder::new(8);
        let a = e.embed(&["hello"]).await.unwrap();
        let b = e.embed(&["hello"]).await.unwrap();
        assert_eq!(a, b, "same input must produce same vector");
    }

    #[tokio::test]
    async fn mock_vectors_are_unit_norm() {
        let e = MockEmbedder::new(16);
        let v = e.embed(&["anything"]).await.unwrap();
        let n: f32 = v[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-5, "vector must be unit norm, got {n}");
    }

    #[tokio::test]
    async fn mock_distinct_inputs_have_low_similarity() {
        let e = MockEmbedder::new(32);
        let a = &e.embed(&["the quick brown fox"]).await.unwrap()[0];
        let b = &e.embed(&["a completely different sentence"]).await.unwrap()[0];
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!(dot.abs() < 0.5, "distinct inputs must be near-orthogonal, got cos={dot}");
    }
}
```

- [ ] **Step 5: Run tests to verify failure (module not visible yet)**

Run:
```bash
cargo test -p codescout-embed --features test-mock mock::tests 2>&1 | tail -20
```
Expected: tests compile and pass (the module is freshly added).

If they fail, fix the module path or feature gate before continuing.

- [ ] **Step 6: Commit**

```bash
git add crates/codescout-embed/Cargo.toml crates/codescout-embed/src/mock.rs crates/codescout-embed/src/lib.rs
git commit -m "feat(embed): add MockEmbedder for deterministic unit tests"
```

---

## Task 3: Factory recognizes `mock:DIM` URL prefix

**Files:**
- Modify: `crates/codescout-embed/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/codescout-embed/src/lib.rs` inside the existing `#[cfg(test)] mod smoke {}` block (around line 223), or add a new test module if none. Use:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```bash
cargo test -p codescout-embed --features test-mock mock_factory_tests 2>&1 | tail -20
```
Expected: FAIL (factory does not yet handle `mock:` scheme — likely tries remote path and errors on the URL shape).

- [ ] **Step 3: Add `mock:` branch to factory**

Edit `crates/codescout-embed/src/lib.rs`. Inside `create_embedder_with_config`, add the new branch **before** the `#[cfg(feature = "remote-embed")] if let Some(url) = url` block (around line 134):

```rust
    #[cfg(any(test, feature = "test-mock"))]
    if let Some(url) = url {
        if let Some(dims_str) = url.strip_prefix("mock:") {
            let dims: usize = dims_str
                .parse()
                .map_err(|_| anyhow::anyhow!("mock: URL requires numeric dim suffix, got '{url}'"))?;
            return Ok(Box::new(mock::MockEmbedder::new(dims)));
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run:
```bash
cargo test -p codescout-embed --features test-mock mock_factory_tests 2>&1 | tail -20
```
Expected: PASS.

- [ ] **Step 5: Run full crate tests, no regressions**

Run:
```bash
cargo test -p codescout-embed --features test-mock 2>&1 | tail -20
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/codescout-embed/src/lib.rs
git commit -m "feat(embed): factory recognizes mock:DIM scheme for tests"
```

---

## Task 4: Phase 2 — Migrate test files to MockEmbedder (loop)

This task is **a loop**, not a single commit. Execute it once per test file identified in Task 1, Step 3.

**Files:** every test file from the Phase 0 audit that constructs `LocalEmbedder`.

- [ ] **Step 1: Pick the next test file from the audit list**

Open the scratch file from Task 1. Take the topmost unmigrated file.

- [ ] **Step 2: Replace `LocalEmbedder` constructions with `MockEmbedder`**

Pattern to replace — find a line shaped like:
```rust
let e = codescout_embed::local::LocalEmbedder::new("AllMiniLML6V2Q").await.unwrap();
```
Or:
```rust
let e = create_embedder_with_config("local:AllMiniLML6V2Q", None, None).await.unwrap();
```

Replace with:
```rust
let e = codescout_embed::mock::MockEmbedder::new(384);
```
Or — when going through the factory (e.g. in indexer tests that feed a `[embeddings]` TOML stanza):
```toml
[embeddings]
model = "mock"
url = "mock:384"
```

Pick the dim that matches what the test originally produced (look up the model on the [fastembed model card](https://github.com/Anush008/fastembed-rs#supported-models) — `AllMiniLML6V2Q` = 384, `BGESmallENV15Q` = 384, `JinaEmbeddingsV2BaseCode` = 768, `NomicEmbedTextV15Q` = 768).

- [ ] **Step 3: Re-run that test file**

Run:
```bash
cargo test --test <test_file_stem> 2>&1 | tail -30
```
Or for inline tests:
```bash
cargo test <module_path> 2>&1 | tail -30
```

Three possible outcomes:

| Outcome | Classification | Action |
|---|---|---|
| Tests pass unchanged | Pure plumbing test | Proceed to Step 5 |
| Tests fail on `score`/ranking assertion | Quality test in disguise | Move to Step 4 |
| Tests fail on dimension mismatch | Wrong mock dim | Adjust dim, re-run |

- [ ] **Step 4: If quality assertion failed — convert to ignored integration test**

Move the test to `tests/embedding_integration.rs` (create the file if it does not exist). Wrap each migrated test with:

```rust
#[tokio::test]
#[ignore = "requires CODESCOUT_TEST_EMBED_URL pointing at a real embedding service"]
async fn original_test_name() {
    let url = match std::env::var("CODESCOUT_TEST_EMBED_URL") {
        Ok(u) => u,
        Err(_) => return,
    };
    // ... original test body, using `url` to construct a RemoteEmbedder
}
```

If a test cannot be meaningfully expressed against any real backend (e.g. it asserts a specific fastembed-internal numeric output), **delete** it. Quality coverage lives in the benchmark, not in unit tests.

- [ ] **Step 5: Commit the migration**

```bash
git add <files-touched>
git commit -m "test(embed): migrate <test_file> from LocalEmbedder to MockEmbedder"
```

- [ ] **Step 6: Mark the file as migrated in the scratch list**

If more files remain, repeat from Step 1. Otherwise advance to Task 5.

- [ ] **Step 7: Final green gate for Phase 2**

After all files migrated:
```bash
cargo test 2>&1 | tail -20
```
Expected: all tests pass. Pass count may be lower than baseline if quality tests were converted to `#[ignore]` or deleted — that is OK; record the delta in your scratch notes.

```bash
cargo test -- --ignored --test embedding_integration 2>&1 | tail -20
```
Expected: tests are listed as `ignored` (skipped). They do not need to pass yet — they run only with `CODESCOUT_TEST_EMBED_URL` set.

---

## Task 5: `check_model_mismatch` auto-wipes legacy `local:` indexes

**Files:**
- Modify: `src/embed/index.rs:2358-2368`

- [ ] **Step 1: Write the failing test**

Append to the existing `#[cfg(test)] mod tests` in `src/embed/index.rs` (near the existing `check_model_mismatch_first_run_is_ok` test, around line 3541):

```rust
    #[test]
    fn check_model_mismatch_local_to_remote_wipes_index() {
        let (_dir, conn) = open_test_db();
        // Simulate: previous index built with local:AllMiniLML6V2Q.
        set_meta(&conn, "embed_model", "local:AllMiniLML6V2Q").unwrap();
        set_meta(&conn, "embed_dims", "384").unwrap();

        // User upgrades to remote-only build, configured = ollama:nomic-embed-text.
        let result = check_model_mismatch(&conn, "ollama:nomic-embed-text");
        assert!(result.is_ok(), "local: → non-local: must auto-wipe, got {result:?}");

        // After auto-wipe, the meta keys are gone — first-run state.
        assert!(get_meta(&conn, "embed_model").unwrap().is_none());
        assert!(get_meta(&conn, "embed_dims").unwrap().is_none());
    }

    #[test]
    fn check_model_mismatch_remote_to_remote_still_errors() {
        let (_dir, conn) = open_test_db();
        set_meta(&conn, "embed_model", "ollama:nomic-embed-text").unwrap();
        let err = check_model_mismatch(&conn, "ollama:bge-m3").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Delete .codescout/embeddings.db"),
            "non-local mismatch must keep manual-delete behavior, got: {msg}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run:
```bash
cargo test check_model_mismatch_local_to_remote_wipes_index 2>&1 | tail -20
```
Expected: FAIL — the current `check_model_mismatch` bails on any mismatch.

- [ ] **Step 3: Implement the auto-wipe branch**

Edit `src/embed/index.rs:2358-2368`. Replace the body of `check_model_mismatch` with:

```rust
pub fn check_model_mismatch(conn: &Connection, configured: &str) -> Result<()> {
    match get_meta(conn, "embed_model")? {
        None => Ok(()), // first run
        Some(stored) if stored == configured => Ok(()),
        Some(stored) if stored.starts_with("local:") && !configured.starts_with("local:") => {
            tracing::info!(
                "Embedding index was built with removed local backend '{stored}'. \
                 Auto-wiping and re-indexing under configured model '{configured}'."
            );
            wipe_index_meta(conn)?;
            Ok(())
        }
        Some(stored) => anyhow::bail!(
            "Index was built with model '{stored}'.\n\
             Configured model is '{configured}'.\n\
             Delete .codescout/embeddings.db and re-run `index` to rebuild."
        ),
    }
}

fn wipe_index_meta(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM meta WHERE key IN ('embed_model', 'embed_dims')", [])?;
    conn.execute("DELETE FROM chunks", [])?;
    // Vector table is virtual — drop and recreate happens on next build_index.
    conn.execute("DELETE FROM vec_chunks", []).ok();
    Ok(())
}
```

**Note for the implementer:** verify the actual table names in `src/embed/index.rs` — `chunks` and `vec_chunks` are the conventional names but the codebase may differ. Search for `CREATE TABLE` in the file to confirm. If different, substitute the real names.

- [ ] **Step 4: Run tests to verify they pass**

Run:
```bash
cargo test check_model_mismatch 2>&1 | tail -20
```
Expected: all four `check_model_mismatch_*` tests pass.

- [ ] **Step 5: Full test suite**

Run:
```bash
cargo test 2>&1 | tail -20
```
Expected: no regressions.

- [ ] **Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): auto-wipe legacy local:-prefix indexes on first run"
```

---

## Task 6: Delete `local.rs`, drop fastembed dep, simplify factory

**Files:**
- Delete: `crates/codescout-embed/src/local.rs`
- Modify: `crates/codescout-embed/src/lib.rs`
- Modify: `crates/codescout-embed/Cargo.toml`
- Modify: `src/embed/mod.rs`

- [ ] **Step 1: Remove the `mod local` declaration in lib.rs**

Edit `crates/codescout-embed/src/lib.rs`. Delete these two lines (around lines 7-8):

```rust
#[cfg(feature = "local-embed")]
pub mod local;
```

- [ ] **Step 2: Remove the `local:` branch from the factory**

Inside `create_embedder_with_config` in `lib.rs`, delete:

1. The block under `// 2. local: prefix` (lines ~145-150):
```rust
    #[cfg(feature = "local-embed")]
    if let Some(model_id) = model.strip_prefix("local:") {
        return Ok(Box::new(local::LocalEmbedder::new(model_id).await?));
    }
```

2. The block under `// 6. No prefix — try as local model name` (lines ~190-197):
```rust
    #[cfg(feature = "local-embed")]
    {
        if local::LocalEmbedder::new(model).await.is_ok() {
            return Ok(Box::new(local::LocalEmbedder::new(model).await?));
        }
    }
```

3. The "helpful error for local: prefix without the feature" block (lines ~199-205):
```rust
    if model.starts_with("local:") {
        anyhow::bail!(
            "Local embedding requires the 'local-embed' feature.\n\
             Rebuild with: cargo build --features local-embed\n\n\
             Recommended: local:AllMiniLML6V2Q (384d, quantized, 22MB)"
        );
    }
```

4. The `.or_else(|| model.strip_prefix("local:"))` call when stripping prefixes for the URL branch (line ~138). The final form is:
```rust
        let bare_model = model
            .strip_prefix("ollama:")
            .or_else(|| model.strip_prefix("openai:"))
            .unwrap_or(model);
```

- [ ] **Step 3: Replace the final fallback error**

Find the trailing `anyhow::bail!("Unknown model ...")` block at the end of `create_embedder_with_config` and replace it with the new "URL required" error:

```rust
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
```

Update the doc comment above the function:

```rust
/// Create an embedder using explicit config fields.
///
/// Resolution order:
/// 1. URL with `mock:DIM` prefix (test feature only) → MockEmbedder
/// 2. URL set → RemoteEmbedder targeting that URL
/// 3. `model` starts with `ollama:` → Ollama (errors loudly if unreachable)
/// 4. `model` starts with `openai:` → OpenAI API
/// 5. `model` starts with `custom:` → hard error with migration hint
/// 6. Otherwise → hard error pointing user at docker setup docs
```

- [ ] **Step 4: Drop `local-embed` feature and `fastembed` dep from Cargo.toml**

Edit `crates/codescout-embed/Cargo.toml`. Remove:

```toml
# Local CPU embedding via fastembed-rs (ONNX Runtime + HuggingFace model hub).
# First use downloads the chosen model (~20-300MB) to ~/.cache/huggingface/hub/.
local-embed = ["dep:fastembed"]
```

And:

```toml
# Local CPU embeddings via ONNX Runtime (fastembed model hub)
fastembed = { version = "5", optional = true }
```

- [ ] **Step 5: Delete local.rs**

Run:
```bash
rm crates/codescout-embed/src/local.rs
```

- [ ] **Step 6: Update the workspace lockfile**

Run:
```bash
cargo update -p fastembed --precise '' 2>&1 | head -5 || true
cargo build 2>&1 | tail -30
```

Expected: build succeeds. Any remaining references to `fastembed`, `local::`, or `LocalEmbedder` surface as compile errors. Fix each by deletion (every reference is now dead).

- [ ] **Step 7: Update `src/embed/mod.rs` re-exports**

Delete the `chunk_size_for_model` from the re-export — it stays in `lib.rs` until Task 7 collapses it, but the function name remains so this is a no-op for now. Verify the file still compiles. If you find a `#[cfg(feature = "local-embed")]` somewhere in `src/embed/mod.rs` or sibling files, delete the gate (the feature is gone).

Run:
```bash
grep -rn "local-embed\|LocalEmbedder\|fastembed\|local::" --include="*.rs" --include="*.toml" src/ crates/ 2>&1 | grep -v target/
```

Expected: only matches are in `docs/` and the `local:` branch handling left in `effective_chunk_size` (handled in Task 7). No code references remain.

- [ ] **Step 8: Run full test suite**

Run:
```bash
cargo test 2>&1 | tail -20
cargo clippy --workspace -- -D warnings 2>&1 | tail -20
```

Expected: all green.

- [ ] **Step 9: Record binary size delta**

Run:
```bash
cargo build --release 2>&1 | tail -5
ls -la target/release/codescout
```

Note the byte size. Compare to a fresh checkout of the previous commit for the delta. Record in the commit message body.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(embed)!: remove LocalEmbedder + fastembed dep

Dropped the local fastembed/ONNX backend entirely. RemoteEmbedder is now
the sole production implementor of the Embedder trait.

Binary size delta: <RECORDED BYTES> smaller.
Lockfile shrinks by the fastembed dependency tree (ONNX runtime,
tokenizers, model-download deps).

Breaking change: [embeddings] url is now required in project.toml.
Legacy local:-prefix indexes auto-wipe on first run (see Task 5).
"
```

---

## Task 7: Replace `chunk_size_for_model` with `DEFAULT_CHUNK_SIZE_CHARS`

**Files:**
- Modify: `crates/codescout-embed/src/lib.rs:18-97`
- Modify: `src/config/project.rs:346-363`
- Modify: `src/embed/mod.rs:29-33` (re-export)
- Modify: `src/embed/mod.rs:76-220` (delete chunk_size_for_model tests)

- [ ] **Step 1: Write the failing test for the new default**

Add to `src/config/project.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn effective_chunk_size_returns_default_when_unset() {
        let sec = EmbeddingsSection {
            model: "ollama:nomic-embed-text".into(),
            url: Some("http://localhost:11434/v1".into()),
            api_key: None,
            chunk_size: None,
            ..Default::default()
        };
        assert_eq!(sec.effective_chunk_size(), 1600);
    }

    #[test]
    fn effective_chunk_size_honors_user_override() {
        let sec = EmbeddingsSection {
            model: "ollama:nomic-embed-text".into(),
            url: Some("http://localhost:11434/v1".into()),
            api_key: None,
            chunk_size: Some(2400),
            ..Default::default()
        };
        assert_eq!(sec.effective_chunk_size(), 2400);
    }
```

If `EmbeddingsSection` does not implement `Default`, construct the struct explicitly with all fields the codebase requires — check the struct definition at `src/config/project.rs:44`.

- [ ] **Step 2: Run test to verify it fails**

Run:
```bash
cargo test effective_chunk_size_returns_default_when_unset 2>&1 | tail -20
```
Expected: FAIL — current default for `nomic-embed-text` is 20889 (or capped at 4096 by `DEFAULT_CAP`).

- [ ] **Step 3: Replace `chunk_size_for_model` with a constant in lib.rs**

Edit `crates/codescout-embed/src/lib.rs`. Delete the entire `chunk_size_for_model` function (lines 18-97 in the doc comment + body). Replace with:

```rust
/// Default per-chunk size in characters when the user does not override via
/// `[embeddings] chunk_size` in project.toml.
///
/// 1600 chars was selected as the sweet spot in the 20-query benchmark
/// (docs/research/2026-04-03-embedding-model-benchmark.md). It keeps methods
/// up to ~40-45 lines whole, avoids "kitchen sink" multi-concept averaging
/// from larger chunks, and preserves enough surface area for multi-keyword
/// queries. See spec docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md
/// for the rationale.
pub const DEFAULT_CHUNK_SIZE_CHARS: usize = 1600;
```

- [ ] **Step 4: Update `effective_chunk_size` in project.rs**

Edit `src/config/project.rs:346-363`. Replace the method body with:

```rust
    /// Resolve the chunk size in characters.
    ///
    /// Returns the user-set `chunk_size` if present, otherwise the project-
    /// wide default. There is no model-specific cap: the AST chunker is
    /// responsible for never truncating a leaf symbol (see
    /// `prefer_chunk_size` in `src/embed/ast_chunker.rs`).
    pub fn effective_chunk_size(&self) -> usize {
        self.chunk_size
            .filter(|&n| n > 0)
            .unwrap_or(codescout_embed::DEFAULT_CHUNK_SIZE_CHARS)
    }
```

- [ ] **Step 5: Update `src/embed/mod.rs` re-exports**

Edit `src/embed/mod.rs:29-33`:

```rust
pub use codescout_embed::{
    create_embedder, create_embedder_with_config, embed_one, DEFAULT_CHUNK_SIZE_CHARS,
};
pub use codescout_embed::{Embedder, Embedding};
```

- [ ] **Step 6: Delete the `chunk_size_for_model` tests in `src/embed/mod.rs`**

Open `src/embed/mod.rs`. Delete every test function whose name starts with `chunk_size_` in the `#[cfg(test)] mod tests` block (lines ~76 through ~220 — the audit grep in Task 1 located 9 of them). Also delete any helper imports that become unused.

- [ ] **Step 7: Fix the second `chunk_size_for_model` test call site**

Edit `src/config/project.rs:854`. The line:
```rust
let model_max = codescout_embed::chunk_size_for_model("local:AllMiniLML6V2Q");
```

Belongs to a test that asserted the old capping behavior. Delete that entire test function. If the function name is `effective_chunk_size_caps_at_model_max` or similar, both the function and its `model_max` variable are dead.

- [ ] **Step 8: Run tests**

Run:
```bash
cargo test effective_chunk_size 2>&1 | tail -20
cargo test 2>&1 | tail -20
cargo clippy --workspace -- -D warnings 2>&1 | tail -20
```
Expected: all green. The two new tests pass. No compile errors from removed `chunk_size_for_model` references.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(embed)!: replace per-model chunk_size derivation with 1600-char default

DEFAULT_CHUNK_SIZE_CHARS = 1600 per benchmark sweet spot
(docs/research/2026-04-03-embedding-model-benchmark.md). User can override
via [embeddings] chunk_size in project.toml.

Removed chunk_size_for_model and its model-name substring tables — the
sweet spot is empirical, not derived from model context length.
"
```

---

## Task 8: AST chunker — preserve leaf-symbol boundaries

**Files:**
- Modify: `src/embed/ast_chunker.rs:828`, `849`, `885`, `905`

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` in `src/embed/ast_chunker.rs`:

```rust
    /// A leaf-symbol chunk that exceeds the soft target must be emitted whole,
    /// not truncated. Splitting a function body mid-statement strips the
    /// return/error path that often carries the answer for retrieval queries.
    #[test]
    fn prefer_chunk_size_preserves_leaf_symbols_above_target() {
        let oversized_body: String = "    let _ = 1;\n".repeat(300); // ~4500 chars
        let chunk = RawChunk {
            content: format!("fn huge() {{\n{oversized_body}}}\n"),
            start_line: 1,
            // Add other fields as needed by the actual RawChunk struct.
            ..Default::default()
        };
        let out = prefer_chunk_size(vec![chunk.clone()], 1600);
        assert_eq!(out.len(), 1, "leaf symbol must not be split");
        assert_eq!(out[0].content, chunk.content);
    }
```

The test uses `Default` for `RawChunk` — if not implemented, instantiate every field explicitly. Check the struct at `crates/codescout-embed/src/chunker.rs:9-18`.

- [ ] **Step 2: Run test to verify it fails**

Run:
```bash
cargo test prefer_chunk_size_preserves_leaf_symbols 2>&1 | tail -20
```
Expected: FAIL — `prefer_chunk_size` does not exist yet; or fails because `enforce_max_chunk_size` truncates the oversized chunk.

- [ ] **Step 3: Rename `enforce_max_chunk_size` and change semantics**

Edit `src/embed/ast_chunker.rs:905`. Use `mcp__codescout__edit_code` with action `rename`:

Run via tool:
```
edit_code(
  path="src/embed/ast_chunker.rs",
  symbol="enforce_max_chunk_size",
  action="rename",
  new_name="prefer_chunk_size",
)
```

This sweeps every call site automatically.

- [ ] **Step 4: Change the body to never truncate leaf symbols**

Open `src/embed/ast_chunker.rs`. Replace the body of `prefer_chunk_size` (formerly `enforce_max_chunk_size`):

```rust
/// Aspirational chunk-size target used by the AST splitter to decide whether
/// to descend into sub-boundaries (impl block → methods, module → functions).
/// Does NOT truncate leaf symbols: a 4000-char method is emitted whole rather
/// than cut mid-body, because a truncated body loses the return path and
/// error handling that often carries the retrieval signal.
fn prefer_chunk_size(chunks: Vec<RawChunk>, _target: usize) -> Vec<RawChunk> {
    // Leaf symbols pass through unchanged. The caller already decomposed
    // composite nodes (impl, mod) into per-method/per-function chunks when
    // they exceeded the target — that decomposition is where the soft target
    // is enforced. By this point, every chunk is a leaf and must remain whole.
    chunks
}
```

- [ ] **Step 5: Update the caller to descend at composite boundaries**

Locate the function in `src/embed/ast_chunker.rs` that produces the initial `Vec<RawChunk>` (it cap-clamps to `AST_CHUNK_TARGET` at line 849). The relevant fragment is:

```rust
let target = chunk_size.min(AST_CHUNK_TARGET);
```

The AST chunker's recursive descent should use `target` as the trigger for descending into a composite node. **If the chunker today already descends when a composite exceeds `target`, no change is needed** — `AST_CHUNK_TARGET = 3000` was the descent trigger and `enforce_max_chunk_size` was the post-hoc truncator. Removing the truncator (Step 4) leaves the descent intact.

Read the surrounding code at `src/embed/ast_chunker.rs` lines 820-890. If the descent uses `AST_CHUNK_TARGET` directly (not `target`), no further action. If it uses `target`, confirm `target = chunk_size.min(AST_CHUNK_TARGET)` still gives the right behavior (`chunk_size = 1600` from `effective_chunk_size` → `target = 1600` since `1600 < 3000`). That is the intended soft target.

- [ ] **Step 6: Update the existing `enforce_max_chunk_size_is_noop_for_small_chunks` test**

That test (around line 1204) was authored against the old truncation semantics. Rename it to `prefer_chunk_size_is_identity` and update its assertion: the function should now return the input unchanged for **any** size, not just small chunks.

```rust
    #[test]
    fn prefer_chunk_size_is_identity() {
        let chunks = vec![
            RawChunk { content: "a".repeat(500), start_line: 1, ..Default::default() },
            RawChunk { content: "b".repeat(5000), start_line: 10, ..Default::default() },
        ];
        let out = prefer_chunk_size(chunks.clone(), 1000);
        assert_eq!(out, chunks);
    }
```

- [ ] **Step 7: Run tests**

Run:
```bash
cargo test prefer_chunk_size 2>&1 | tail -20
cargo test --lib embed::ast_chunker 2>&1 | tail -30
cargo test 2>&1 | tail -20
cargo clippy --workspace -- -D warnings 2>&1 | tail -20
```
Expected: all green. Both new tests pass. Any previously-passing AST chunker tests that asserted truncation may fail — fix them by removing the truncation expectation, since leaf symbols are now preserved.

- [ ] **Step 8: Commit**

```bash
git add src/embed/ast_chunker.rs
git commit -m "refactor(embed): prefer_chunk_size preserves leaf symbols above target

AST_CHUNK_TARGET (3000) and the user-configured chunk_size remain triggers
for descending into composite nodes (impl → methods). But once a leaf
symbol is reached, it is emitted whole — truncation strips the return
path that often answers retrieval queries.

Renamed enforce_max_chunk_size → prefer_chunk_size to make the soft-target
semantics explicit.
"
```

---

## Task 9: Re-run retrieval benchmark and record in tracker

**Files:**
- Modify: `docs/trackers/retrieval-benchmark.md`

- [ ] **Step 1: Build release with current changes**

Run:
```bash
cargo build --release
```
Expected: clean build.

- [ ] **Step 2: Wipe and rebuild the index against the local docker embed service**

Set:
```bash
export CODESCOUT_EMBED_URL="http://localhost:43300/v1"  # or wherever your service runs
```

Update `.codescout/project.toml` to:
```toml
[embeddings]
model = "CodeRankEmbed"   # or the model used in the 2026-04-03 benchmark
url = "http://localhost:43300/v1"
```

Delete and rebuild the index:
```bash
rm -rf .codescout/embeddings.db
# Restart MCP server (cargo build --release + /mcp restart in Claude Code)
# Trigger an index build via the index tool.
```

- [ ] **Step 3: Run the 20-query benchmark**

Follow the benchmark procedure in `docs/research/2026-04-03-embedding-model-benchmark.md`. For each of the 20 TCs, score 0-3 based on the top-10 results.

Total max: 60.

- [ ] **Step 4: Compare against ship gate**

The success criterion from spec `76b7e842b04bdc3c` is **≥ 30/60**.

- Total ≥ 30 → ship.
- Total 25–29 → modest but worthwhile; ship and iterate.
- Total < 25 → STOP. Open a follow-up issue. Do not merge Phase 4 until the cause is understood.

- [ ] **Step 5: Append the run to the retrieval-benchmark tracker**

Edit `docs/trackers/retrieval-benchmark.md`. Under `## History`, append a new dated section:

```markdown
### 2026-MM-DD — remote-only + 1600 default chunk size

Model: <model>, URL: <url>, chunk_size: 1600 (default), index commit: <git sha>.

| Tier | Score | Max |
|---|---|---|
| 1 (exact) | <n> | 15 |
| 2 (impl) | <n> | 21 |
| 3 (arch) | <n> | 15 |
| 4 (docs) | <n> | 9 |
| **Total** | **<n>** | **60** |

Notes: <one paragraph on which queries moved up/down vs the 2026-04-03 baseline,
whether leaf-symbol preservation surfaced new files into top-10, any regressions>.
```

- [ ] **Step 6: Commit**

```bash
git add docs/trackers/retrieval-benchmark.md
git commit -m "docs(benchmark): record post-removal retrieval scores"
```

---

## Task 10: Update three prompt surfaces

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/prompts/builders.rs` (`build_system_prompt_draft`)
- Modify: `src/tools/onboarding.rs` (bump `ONBOARDING_VERSION`)

- [ ] **Step 1: Grep for `local:` and fastembed mentions in prompt surfaces**

Run:
```bash
grep -n "local:\|fastembed\|local-embed\|AllMiniLM\|BGESmall\|JinaEmbed" \
  src/prompts/server_instructions.md \
  src/prompts/onboarding_prompt.md \
  src/prompts/builders.rs
```

Record every match — these are the lines to update.

- [ ] **Step 2: Update `server_instructions.md`**

For every `local:<ModelName>` mention, replace with a remote example. Typical replacements:

| Before | After |
|---|---|
| `local:AllMiniLML6V2Q` | `ollama:nomic-embed-text` |
| `local:JinaEmbeddingsV2BaseCode` | `nomic-embed-code` (with `url = "http://localhost:43300/v1"`) |
| "bundled ONNX" / "no server needed" | remove — no longer true |
| Examples without `url =` | add `url = "http://localhost:11434/v1"` |

Use `edit_markdown` for heading-based section edits. Do not freelance the writing style — match the existing tone of the file (terse, imperative, no marketing).

- [ ] **Step 3: Update `onboarding_prompt.md`**

Same replacements as Step 2. Pay extra attention to the embedding-setup walkthrough section (if present) — it should now read like:

> Embedding setup requires an external service. Recommended: Ollama
> (`docker run -d -p 11434:11434 ollama/ollama`) or llama-server. Set in
> `.codescout/project.toml`:
> ```toml
> [embeddings]
> model = "nomic-embed-text"
> url   = "http://localhost:11434/v1"
> ```

- [ ] **Step 4: Update `build_system_prompt_draft` in `src/prompts/builders.rs`**

Edit any string literals that mention `local:` or fastembed. This is Rust source — use `mcp__codescout__edit_code` for the function body if the change is structural, or `edit_file` for inline string edits.

- [ ] **Step 5: Bump `ONBOARDING_VERSION`**

Edit `src/tools/onboarding.rs`. Find the line:
```rust
pub const ONBOARDING_VERSION: u32 = <N>;
```
Increment to `<N+1>`. CLAUDE.md explains: this triggers automatic system-prompt regeneration for all projects on the previous version.

- [ ] **Step 6: Run the prompt-surface tripwire test**

Run:
```bash
cargo test prompt_surfaces_reference_only_real_tools 2>&1 | tail -20
```
Expected: PASS. If it fails because a removed tool/model name is still mentioned, fix the offending file.

- [ ] **Step 7: Full test suite**

Run:
```bash
cargo test 2>&1 | tail -20
cargo clippy --workspace -- -D warnings 2>&1 | tail -20
```
Expected: green.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "docs(prompts): update three surfaces for remote-only embedding

Removed all local:<Model> examples. Added required url= in every
[embeddings] sample. Bumped ONBOARDING_VERSION to trigger system-prompt
regeneration for existing projects.
"
```

---

## Task 11: Write the ADR

**Files:**
- Create: `docs/adrs/2026-05-11-remote-only-embedding.md`

- [ ] **Step 1: Create the ADR file**

Create `docs/adrs/2026-05-11-remote-only-embedding.md` with this content:

```markdown
---
title: ADR — Remote-Only Embedding Architecture
date: 2026-05-11
status: accepted
related:
  - docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md
  - docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md
  - docs/trackers/archive/embedding-chunk-size-2026-04.md
  - docs/trackers/retrieval-benchmark.md
---

# ADR — Remote-Only Embedding Architecture

## Status

Accepted — 2026-05-11.

## Context

`codescout-embed` shipped two implementors of the `Embedder` trait:

- `LocalEmbedder` — fastembed + ONNX Runtime, in-process, models downloaded on
  first use.
- `RemoteEmbedder` — HTTP client targeting any OpenAI-compatible embedding
  service (Ollama, llama-server, OpenAI itself).

Local was the original default. Remote was added once external services
became reliable in our deployment topology.

## Decision

Remove `LocalEmbedder` entirely. `RemoteEmbedder` becomes the sole production
implementor. `[embeddings] url` in `.codescout/project.toml` becomes required.
No fallback, no auto-discovery, no bundled service. The `Embedder` trait
survives — it earns its keep at the test boundary (`MockEmbedder`) and as
future-proofing for provider swaps.

Major version bump on the next release. Existing `local:`-prefix indexes
auto-wipe on first run via a narrow `check_model_mismatch` special case.

## Drivers

1. **Compile cost and binary size.** The ONNX runtime, `tokenizers`, and
   model-download machinery dominate cold compile time and add measurable
   bytes to every release artifact.
2. **Maintenance burden.** Two backends meant two failure modes, two
   model-naming conventions (`local:BGESmallENV15Q` vs `ollama:bge-m3`), and
   two integration-test surfaces.
3. **Deployment topology.** A docker-hosted embedding service is now always
   available in every codescout deployment shape we ship. Local was redundant.
4. **Platform reach.** ONNX Runtime requires per-platform native libraries
   (`.so`/`.dylib`/`.dll`). Windows support has been blocked by the
   complexity of bundling and distributing them. Pure-Rust + `reqwest` is
   cross-platform free.

## Alternatives considered

- **Auto-spawn docker container on first index.** Rejected: codescout becomes
  a process supervisor. Lifecycle bugs, container-cleanup edge cases, and
  unclear ownership of the embedding image's update cadence.
- **Bundle docker-compose + degrade gracefully.** Rejected: silent
  partial-feature mode hides misconfiguration. A hard error is easier to
  debug than `semantic_search` returning empty results.
- **Default localhost probe (Ollama on `:11434`).** Rejected: implicit
  dependencies break in production. One line of explicit config is cheaper
  than a probe and friendlier on first failure.
- **Deprecation window with warning logs.** Rejected: pre-1.0 project, users
  expected to follow breaking changes. A window costs an extra release of
  double-backend maintenance for marginal kindness.

## Consequences

**Positive:**
- Smaller release binary, faster CI compile, no per-platform native lib
  shipping concerns.
- Windows support unblocked.
- Single error surface for embedding failures — easier to debug.
- No model-id schema sprawl in `[embeddings] model`.

**Negative:**
- Docker becomes a hard install dependency for any user wanting semantic
  search. First-run requires both the codescout binary AND a running
  embedding service.
- Mitigated by an actionable `RecoverableError` message that names the
  config key, suggests a docker image, and links to setup docs.

**Neutral:**
- Retrieval quality unchanged (`MockEmbedder` protects plumbing; the 20-query
  benchmark protects quality, recorded in
  `docs/trackers/retrieval-benchmark.md`).
- Legacy `local:`-prefix indexes auto-wiped on upgrade via narrow special case
  in `check_model_mismatch`. No user action required.

## When to revisit

- A pure-Rust embedding library (e.g. `candle`, `burn`) reaches
  production-grade with stable, Windows-clean model coverage and no native
  binding requirements.
- GitHub issues tagged `embedding-setup` accumulate beyond ~5 per quarter,
  indicating the docker-setup friction is a meaningful abandonment point.
- The embedding-service space consolidates such that auto-spawn becomes a
  one-line concern (e.g. a stable, opinionated default container image).

## References

- Spec: `docs/superpowers/specs/2026-05-11-remote-only-embedding-design.md`
- Related spec: `docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md`
- Archived tracker: `docs/trackers/archive/embedding-chunk-size-2026-04.md`
- Living tracker: `docs/trackers/retrieval-benchmark.md`
```

- [ ] **Step 2: Commit**

```bash
git add docs/adrs/2026-05-11-remote-only-embedding.md
git commit -m "docs(adr): remote-only embedding architecture"
```

---

## Task 12: README, ARCHITECTURE, manual, CHANGELOG, version bump

**Files:**
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/manual/src/concepts/embedding.md` (if it exists)
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Update README**

Grep:
```bash
grep -n "fastembed\|local:\|local-embed\|AllMiniLM\|BGESmall" README.md
```

For every match, rewrite the section to reflect remote-only. The "Embedding setup" or "Getting started" section should now read like:

> codescout requires an external embedding service for semantic search.
> Recommended quick start with Ollama:
> ```bash
> docker run -d --name ollama -p 11434:11434 ollama/ollama
> docker exec ollama ollama pull nomic-embed-text
> ```
> Then in `.codescout/project.toml`:
> ```toml
> [embeddings]
> model = "nomic-embed-text"
> url   = "http://localhost:11434/v1"
> ```

- [ ] **Step 2: Update `docs/ARCHITECTURE.md`**

Find the embedding section. Replace any "two backends (local + remote)" framing with the new container view from spec Section 2.

- [ ] **Step 3: Update the manual page (if present)**

Check:
```bash
ls docs/manual/src/concepts/embedding* 2>/dev/null
```

If a page exists, update it with the same patterns as README. If no page exists, do not create one — the README is sufficient.

- [ ] **Step 4: Write the CHANGELOG entry**

Edit `CHANGELOG.md`. Add at the top:

```markdown
## vX.0.0 — 2026-MM-DD

### Breaking changes

- **Removed local fastembed/ONNX embedding backend.** `[embeddings] url` is
  now required in `.codescout/project.toml`. Configure an external embedding
  service (Ollama, llama-server, or any OpenAI-compatible endpoint). See
  README for setup. Legacy `local:`-prefix indexes auto-wipe on first run.

### Changed

- Default chunk size is now 1600 characters (was per-model-derived, capped at
  4096). User can override via `[embeddings] chunk_size`.
- AST chunker no longer truncates leaf symbols above the target size — a
  4000-char method is emitted whole, preserving the return path that often
  carries the retrieval signal.

### Removed

- `local:<Model>` prefix in `[embeddings] model`.
- `fastembed` dependency and `local-embed` Cargo feature in `codescout-embed`.
- `chunk_size_for_model` function (replaced by `DEFAULT_CHUNK_SIZE_CHARS`
  constant).

### Internal

- Added `MockEmbedder` for unit tests (orthogonal-vector deterministic
  double; forces test authors to assert on plumbing, not ranking).
- Added scheduled CI job running ignored integration tests against a live
  embedding service (`CODESCOUT_TEST_EMBED_URL`).
```

- [ ] **Step 5: Major version bump**

Edit the root `Cargo.toml`. Bump the `[package] version` (and any workspace-pinned versions) to the next major. Example:

```toml
[package]
name = "codescout"
version = "X.0.0"
```

Run:
```bash
cargo build 2>&1 | tail -5
```
Expected: builds; lockfile updates.

- [ ] **Step 6: Final full test + clippy + release build**

Run:
```bash
cargo test 2>&1 | tail -20
cargo clippy --workspace -- -D warnings 2>&1 | tail -20
cargo build --release 2>&1 | tail -5
```
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "chore: bump version to X.0.0 — remote-only embedding (breaking)

See CHANGELOG.md and docs/adrs/2026-05-11-remote-only-embedding.md for the
full migration story.
"
```

---

## Task 13: End-to-end smoke against live docker

**Files:** none (manual verification step).

- [ ] **Step 1: Restart MCP server with the release binary**

In Claude Code: `/mcp` to disconnect, then reconnect. The release binary now serves the new code.

- [ ] **Step 2: Verify the missing-URL error path**

Temporarily edit a test project's `.codescout/project.toml` to remove the `url = ...` line under `[embeddings]`. Trigger `index(action="build")` or `semantic_search(query="anything")`. Expected: `RecoverableError` containing the config key name, docker image suggestion, and GitHub docs URL — in that order.

Restore the `url`.

- [ ] **Step 3: Verify legacy `local:`-prefix wipe**

On a project that was indexed under the previous release: confirm that the
first `index(action="build")` after upgrade emits the `tracing::info!`
auto-wipe message and rebuilds against the new model. The end-to-end run
should not require manual deletion of `.codescout/embeddings.db`.

- [ ] **Step 4: Verify a semantic_search round-trip**

Run a known-good query that previously returned strong results (e.g. `OutputGuard cap_items`). The top result should still be relevant.

- [ ] **Step 5: No commit — manual verification only**

If any of the above failed, open a follow-up issue rather than ad-hoc fixing in the same commit. The plan is otherwise complete.

---

## Final ship sequence

After Task 13 passes, follow the CLAUDE.md release cycle:

```bash
# Already on experiments. Verify clean tree:
git status

# Cherry-pick (or merge — invoke Docs Lotus Frog first per CLAUDE.md):
git checkout master
git cherry-pick <range>     # or appropriate strategy

# Publish:
cargo build --release
cargo test
cargo clippy -- -D warnings

git tag vX.0.0
CARGO_REGISTRY_TOKEN=$(grep CARGO_REGISTRY_TOKEN .env | cut -d= -f2) cargo publish
git push && git push --tags
gh release create vX.0.0 --title "vX.0.0" --notes-file CHANGELOG.md

# Rebase experiments:
git checkout experiments && git rebase master
```
