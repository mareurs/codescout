# Windows Removal & Build Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop Windows support, gate heavy optional deps behind features, and switch to fat LTO — targeting a significantly smaller stripped release binary.

**Architecture:** Four independent cleanup streams committed in order: (1) Windows platform code deleted, (2) feature flags restructured, (3) TLS backend swapped, (4) release profile hardened. Each stream is independently verifiable with `cargo check` / `cargo test`.

**Tech Stack:** Rust, Cargo features, LLVM LTO, GitHub Actions CI matrix

**Spec:** `docs/superpowers/specs/2026-05-06-windows-removal-build-optimization-design.md`

**Baseline binary size (stripped):** 56MB

---

## File Map

| File | Change |
|------|--------|
| `src/platform/windows.rs` | **Delete** |
| `src/platform/mod.rs` | Remove `#[cfg(windows)]` branches; make unix unconditional |
| `src/util/path_security.rs` | Remove `#[cfg(windows)]` system-path block |
| `src/tools/run_command/inner.rs` | Remove two `#[cfg(windows)]` blocks |
| `src/lsp/client.rs` | Remove `cfg!(windows)` URI branch in test |
| `src/embed/index.rs` | Remove `cfg!(windows)` path in test; gate BM25 build call |
| `src/embed/mod.rs` | Gate `pub mod bm25` behind `#[cfg(feature = "full-text")]` |
| `src/embed/bm25.rs` | Add `#![cfg(feature = "full-text")]` inner attribute |
| `src/tools/semantic/semantic_search.rs` | Gate BM25 leg behind `#[cfg(feature = "full-text")]` |
| `Cargo.toml` | New `full-text` feature; change defaults; fat LTO profile |
| `crates/codescout-embed/Cargo.toml` | Switch reqwest to `rustls-tls`, disable default TLS |
| `.github/workflows/ci.yml` | Remove `windows-latest` from test matrix |

---

## Task 1: Drop Windows — platform abstraction layer

**Files:**
- Delete: `src/platform/windows.rs`
- Modify: `src/platform/mod.rs`

- [ ] **Step 1: Delete windows.rs**

```bash
git rm src/platform/windows.rs
```

- [ ] **Step 2: Simplify platform/mod.rs — make unix unconditional**

Replace the entire top of `src/platform/mod.rs` (lines 1–17):

```rust
//! Platform abstraction layer.
//!
//! Provides OS-specific implementations for filesystem paths, shell commands,
//! process management, and security defaults. All platform-specific code should
//! go through this module rather than using `#[cfg]` blocks elsewhere.

use std::path::PathBuf;

mod unix;
use unix as imp;
```

Also update the doc comments below that mention Windows specifically:

- Line ~57: `"/// On Unix this is a no-op wrapper around \`std::fs::rename\`.\n/// On Windows this uses \`MoveFileExW\` with \`MOVEFILE_REPLACE_EXISTING\`."` → `"/// Atomic rename that overwrites the destination."`
- Line ~64: `"/// Platform-aware LSP server binary name.\n/// On Windows, appends \`.cmd\` or \`.exe\` as needed."` → `"/// Return the LSP server binary name for the current platform."`

- [ ] **Step 3: Verify it compiles**

```bash
cargo check
```

Expected: no errors. If the compiler complains about `imp` being unused — that's a logic error in step 2; double-check the `use unix as imp` line is present.

- [ ] **Step 4: Run tests**

```bash
cargo test -p codescout 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/platform/mod.rs
git commit -m "feat: drop Windows platform support — remove windows.rs and make unix unconditional"
```

---

## Task 2: Drop Windows — inline cfg guards

**Files:**
- Modify: `src/util/path_security.rs`
- Modify: `src/tools/run_command/inner.rs`
- Modify: `src/lsp/client.rs`
- Modify: `src/embed/index.rs`

- [ ] **Step 1: Remove Windows system-path block from path_security.rs**

Find and delete the block starting at approximately line 148:

```rust
// Windows-specific system paths
#[cfg(windows)]
{
    if let Ok(sysroot) = std::env::var("SYSTEMROOT") {
        let p = PathBuf::from(&sysroot).join("System32").join("config");
        // ... rest of block
    }
}
```

Delete the entire `#[cfg(windows)] { ... }` block. The linux and macos blocks above it stay.

- [ ] **Step 2: Remove Windows blocks from run_command/inner.rs**

Run to locate exact lines:

```bash
grep -n 'cfg(windows)' src/tools/run_command/inner.rs
```

There are two `#[cfg(windows)]` blocks:

**Block 1** (~line 49): After `libc::kill(pid, SIGKILL)` — a `#[cfg(windows)]` block that calls `taskkill /F /PID`. Delete the entire `#[cfg(windows)] { ... }` block. The `libc::kill` line above it stays.

**Block 2** (~line 357): A `#[cfg(windows)]` child output future that duplicates what the Unix path already does. Delete the entire `#[cfg(windows)] let (child_output_fut, child_pgid) = { ... };` block.

- [ ] **Step 3: Fix cfg!(windows) URI in lsp/client.rs test**

Find (~line 1883):

```rust
let uri: Uri = if cfg!(windows) {
    "file:///C:/temp/test.rb".parse().unwrap()
} else {
    "file:///tmp/test.rb".parse().unwrap()
};
```

Replace with:

```rust
let uri: Uri = "file:///tmp/test.rb".parse().unwrap();
```

- [ ] **Step 4: Fix cfg!(windows) path in embed/index.rs test**

Find (~line 3594):

```rust
let root = PathBuf::from(if cfg!(windows) {
    "C:\\project"
} else {
    "/project"
});
```

Replace with:

```rust
let root = PathBuf::from("/project");
```

- [ ] **Step 5: Verify**

```bash
cargo check && cargo test 2>&1 | tail -5
```

Expected: no errors, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/util/path_security.rs src/tools/run_command/inner.rs src/lsp/client.rs src/embed/index.rs
git commit -m "feat: remove remaining #[cfg(windows)] guards"
```

---

## Task 3: Drop Windows — CI matrix

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Remove windows-latest from test matrix**

Find in `.github/workflows/ci.yml`:

```yaml
matrix:
  os: [ubuntu-latest, macos-latest, windows-latest]
```

Replace with:

```yaml
matrix:
  os: [ubuntu-latest, macos-latest]
```

- [ ] **Step 2: Verify yml is valid**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo "valid"
```

Expected: `valid`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: drop windows-latest from test matrix"
```

---

## Task 4: Gate tantivy behind `full-text` feature

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/embed/bm25.rs`
- Modify: `src/embed/mod.rs`
- Modify: `src/embed/index.rs`
- Modify: `src/tools/semantic/semantic_search.rs`

- [ ] **Step 1: Verify tests pass with current code (baseline)**

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 2: Add full-text feature and make tantivy optional in Cargo.toml**

In `Cargo.toml`, find:

```toml
tantivy = "0.22"
```

Replace with:

```toml
tantivy = { version = "0.22", optional = true }
```

In the `[features]` section, add the new feature (place it near the other feature definitions):

```toml
# Full-text BM25 search via tantivy (increases binary size significantly)
full-text = ["dep:tantivy"]
```

Also add `full-text` to the default features for now (we keep it on by default initially — Task 5 removes it):

```toml
default = ["remote-embed", "local-embed", "http", "librarian", "full-text"]
```

- [ ] **Step 3: Gate bm25.rs entirely behind the feature**

Add as the very first line of `src/embed/bm25.rs`:

```rust
#![cfg(feature = "full-text")]
```

- [ ] **Step 4: Gate bm25 module export in embed/mod.rs**

Find in `src/embed/mod.rs` line 14:

```rust
pub mod bm25;
```

Replace with:

```rust
#[cfg(feature = "full-text")]
pub mod bm25;
```

- [ ] **Step 5: Gate BM25 index build in embed/index.rs**

Find (~line 2112):

```rust
    // BM25 index: full rebuild from the freshly-written chunks table.
    // conn was moved into db_writer — re-open for the BM25 pass.
    {
        let bm25_conn = open_db(project_root)?;
        crate::embed::bm25::BM25Index::build(project_root, &bm25_conn)?;
    }
```

Replace with:

```rust
    // BM25 index: full rebuild from the freshly-written chunks table.
    // conn was moved into db_writer — re-open for the BM25 pass.
    #[cfg(feature = "full-text")]
    {
        let bm25_conn = open_db(project_root)?;
        crate::embed::bm25::BM25Index::build(project_root, &bm25_conn)?;
    }
```

Also find the test `bm25_index_built_after_build_index` (~line 4298) and add `#[cfg(feature = "full-text")]` above its `#[test]` attribute:

```rust
    #[cfg(feature = "full-text")]
    #[test]
    fn bm25_index_built_after_build_index() {
```

- [ ] **Step 6: Gate BM25 leg in semantic_search.rs**

Find (~line 207):

```rust
            // BM25 leg — project scope only; other scopes fall back to pure vector
            let bm25_results = if matches!(scope2, crate::library::scope::Scope::Project) {
                match crate::embed::bm25::BM25Index::open(&root2)? {
                    Some(idx) => idx.search(&query2, search_limit).unwrap_or_else(|e| {
                        tracing::warn!(
                            "BM25 search failed, falling back to pure vector: {e}"
                        );
                        vec![]
                    }),
                    None => vec![],
                }
            } else {
                vec![]
            };
```

Replace with:

```rust
            // BM25 leg — project scope only; other scopes fall back to pure vector
            #[cfg(feature = "full-text")]
            let bm25_results = if matches!(scope2, crate::library::scope::Scope::Project) {
                match crate::embed::bm25::BM25Index::open(&root2)? {
                    Some(idx) => idx.search(&query2, search_limit).unwrap_or_else(|e| {
                        tracing::warn!(
                            "BM25 search failed, falling back to pure vector: {e}"
                        );
                        vec![]
                    }),
                    None => vec![],
                }
            } else {
                vec![]
            };
            #[cfg(not(feature = "full-text"))]
            let bm25_results: Vec<crate::embed::fusion::BM25Result> = vec![];
```

- [ ] **Step 7: Verify full-text feature compiles**

```bash
cargo check --features full-text
```

Expected: no errors.

- [ ] **Step 8: Verify no-default-features compiles (no tantivy)**

```bash
cargo check --no-default-features
```

Expected: no errors. If the compiler complains about `BM25Result` type not found, there's a missing `#[cfg]` gate — find it via the error location and add the appropriate gate.

- [ ] **Step 9: Run tests with full-text on**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass (full-text is still in defaults at this point).

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml Cargo.lock src/embed/bm25.rs src/embed/mod.rs src/embed/index.rs src/tools/semantic/semantic_search.rs
git commit -m "feat: gate tantivy/BM25 behind full-text feature flag"
```

---

## Task 5: Remove `local-embed` and `full-text` from default features

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Update default features**

In `Cargo.toml` `[features]` section, change:

```toml
default = ["remote-embed", "local-embed", "http", "librarian", "full-text"]
```

to:

```toml
default = ["remote-embed", "http", "librarian"]
```

- [ ] **Step 2: Verify default build compiles**

```bash
cargo check
```

Expected: no errors. `ort_sys` (ONNX Runtime) and `tantivy` are no longer compiled.

- [ ] **Step 3: Verify local-embed still works as opt-in**

```bash
cargo check --features local-embed
```

Expected: no errors.

- [ ] **Step 4: Verify full-text still works as opt-in**

```bash
cargo check --features full-text
```

Expected: no errors.

- [ ] **Step 5: Run tests (default features)**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass. BM25-specific tests are skipped (feature off).

- [ ] **Step 6: Run tests with all non-local features**

```bash
cargo test --features full-text 2>&1 | tail -10
```

Expected: all tests pass including BM25 tests.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: remove local-embed and full-text from default features"
```

---

## Task 6: Switch reqwest TLS from aws-lc to rustls

**Files:**
- Modify: `crates/codescout-embed/Cargo.toml`

- [ ] **Step 1: Audit reqwest feature usage**

Run to see what reqwest features are currently active:

```bash
cargo tree -e features -p reqwest 2>&1 | head -30
```

Note which default features are pulled in (typically: `charset`, `http2`, `macos-system-roots`, `stream`). The goal is to keep those and only swap out the TLS backend.

- [ ] **Step 2: Update reqwest dep in crates/codescout-embed/Cargo.toml**

Find:

```toml
reqwest = { version = "0.13", features = ["json"], optional = true }
```

Replace with:

```toml
reqwest = { version = "0.13", features = ["json", "rustls-tls", "charset", "http2", "stream"], default-features = false, optional = true }
```

Note: `macos-system-roots` is macOS-only and only relevant when using the native TLS backend — skip it. `rustls-tls` bundles its own root store via `rustls-native-certs`.

- [ ] **Step 3: Verify both platforms compile**

```bash
cargo check --features remote-embed
cargo check --features remote-embed,local-embed
```

Expected: no errors on both.

- [ ] **Step 4: Run tests**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 5: Confirm aws_lc_sys is gone from the dep tree**

```bash
cargo tree 2>&1 | grep aws_lc_sys
```

Expected: no output (aws-lc-rs no longer pulled in).

- [ ] **Step 6: Commit**

```bash
git add crates/codescout-embed/Cargo.toml Cargo.lock
git commit -m "feat: switch reqwest TLS from aws-lc-rs to rustls — drops aws_lc_sys (1.3MB)"
```

---

## Task 7: Release profile — fat LTO + codegen-units=1

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Update release profile**

In `Cargo.toml`, find:

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 16
strip = true
panic = "abort"
```

Replace with:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

- [ ] **Step 2: Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: compiles successfully. This will take longer than usual (~3–5 min extra) due to fat LTO.

- [ ] **Step 3: Measure final binary size**

```bash
ls -lh target/release/codescout
```

Record the result. Update the spec with the actual final size:

```bash
# In docs/superpowers/specs/2026-05-06-windows-removal-build-optimization-design.md
# Update the "Final target estimate" section with the actual measured size
```

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: switch release profile to fat LTO + codegen-units=1"
```

---

## Task 8: Final verification

- [ ] **Step 1: Format**

```bash
cargo fmt
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | tail -20
```

Expected: no warnings or errors. Fix any that appear before continuing.

- [ ] **Step 3: Full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 4: Test opt-in features**

```bash
cargo test --features full-text 2>&1 | tail -10
cargo check --features local-embed 2>&1 | tail -5
```

Expected: both pass.

- [ ] **Step 5: Final binary size report**

```bash
cargo build --release 2>/dev/null; ls -lh target/release/codescout
```

- [ ] **Step 6: Amend spec with actual result**

Open `docs/superpowers/specs/2026-05-06-windows-removal-build-optimization-design.md` and add actual final size to the "Final target estimate" row.

- [ ] **Step 7: Commit any fmt/clippy fixes**

```bash
git add -A
git commit -m "chore: fmt + clippy cleanup after build optimization"
```
