# Windows Removal & Build Optimization

**Date:** 2026-05-06  
**Status:** Approved  
**Motivation:** Binary size (56MB → target: best effort), CI speed, code simplicity, release quality

---

## Goals

1. Drop Windows support — Linux + macOS only
2. Remove `local-embed` from default features (ONNX Runtime is 12.3MB of `.text`)
3. Gate `tantivy` behind an optional feature
4. Switch `reqwest` TLS from `aws-lc-rs` to `rustls`
5. Upgrade release profile to `lto = "fat"` + `codegen-units = 1`

**Non-goals:** Remove `local-embed` capability entirely; change any tool behavior; touch non-build code beyond Windows `#[cfg]` cleanup.

---

## Baseline

| Metric | Value |
|--------|-------|
| Binary (stripped) | 56MB |
| Binary (unstripped) | 78MB |
| `.text` section | 35.5MB |
| Largest contributor | `ort_sys` (ONNX Runtime) — 12.3MB / 34.8% of `.text` |
| Second largest | `aws_lc_sys` — 1.3MB |
| `tantivy` | 713KB |

---

## Section 1 — Windows Removal

### Files deleted
- `src/platform/windows.rs`

### Files modified

**`src/platform/mod.rs`**
- Remove `#[cfg(windows)] mod windows;` and `#[cfg(windows)] use windows as imp;`
- Make `#[cfg(unix)] mod unix;` and `use unix as imp;` unconditional

**`src/util/path_security.rs`**
- Remove `#[cfg(windows)]` block (lines ~149–155) for Windows system path denied list
- Keep `#[cfg(target_os = "linux")]` and `#[cfg(target_os = "macos")]` blocks

**`src/tools/run_command/inner.rs`**
- Remove `#[cfg(windows)]` block for `taskkill` fallback (lines ~49–56)
- Remove `#[cfg(windows)]` child output future block (lines ~357+)
- The unconditional `libc::kill` path becomes the only path

**`src/lsp/client.rs`**
- Line ~1883: replace `cfg!(windows)` branch with the Unix URI directly

**`src/embed/index.rs`**
- Line ~3594: replace `cfg!(windows)` test path with Unix path directly

### CI — `.github/workflows/ci.yml`
- Remove `windows-latest` from `matrix.os`; keep `[ubuntu-latest, macos-latest]`
- `fmt`, `clippy`, `msrv`, `tool-docs-sync` jobs unchanged (already Ubuntu-only)

---

## Section 2 — Feature Flag Changes

### Default features

```toml
# Cargo.toml [features]
default = ["remote-embed", "http", "librarian"]
# local-embed and full-text are now opt-in
```

### New `full-text` feature for tantivy

```toml
full-text = ["dep:tantivy"]
```

- Move `tantivy` from unconditional dep to optional: `tantivy = { version = "0.22", optional = true }`
- Gate all tantivy-using code behind `#[cfg(feature = "full-text")]`
- Audit scope: grep `use tantivy` across `src/` to find all call sites before implementing

### reqwest TLS switch

```toml
reqwest = { version = "0.13", features = ["json", "rustls-tls"], default-features = false, optional = true }
```

- Drops `aws_lc_sys` (1.3MB of `.text`)
- `rustls` already present in the binary via `rmcp` — net reduction, no new dep
- **Implementation note:** verify which reqwest default features are actually used before setting `default-features = false`; reqwest 0.13 defaults include `http2`, `charset`, `stream`, `macos-system-roots`. Keep any that are needed; only goal is to drop `default-tls` (which pulls `aws-lc-rs`).
### Expected impact (default build)
- `ort_sys` gone: ~12.3MB off `.text`
- `aws_lc_sys` gone: ~1.3MB off `.text`
- `tantivy` + `bitpacking` + related gone: ~1.2MB off `.text`
- Rough stripped binary estimate: **~35–40MB** before LTO improvements

---

## Section 3 — Release Profile

```toml
[profile.release]
opt-level = 3
lto = "fat"        # was "thin"
codegen-units = 1  # was 16
strip = true
panic = "abort"
```

- **Fat LTO**: full cross-crate dead code elimination; with `ort_sys` out of the default graph, the optimizer has much less noise to work through
- **codegen-units = 1**: single translation unit, maximum LLVM inlining and pruning visibility
- **Compile time impact**: +30–40% on `cargo build --release`; absorbed by dropping the Windows CI runner
- **Additional size reduction**: estimated 10–20% off remaining `.text` → ~2–4MB more off stripped binary

### Final target estimate
- Stripped binary: **~30–35MB** (default features, fat LTO)
- With `--features local-embed`: back to ~50MB+ (ONNX Runtime reintroduced)

---

## Implementation Order

1. Windows removal (platform/mod.rs, windows.rs, cfg guards, CI matrix)
2. `full-text` feature gate for tantivy (audit call sites first)
3. Remove `local-embed` from default features
4. Switch reqwest to `rustls-tls`
5. Release profile upgrade (lto=fat, codegen-units=1)
6. `cargo fmt && cargo clippy -- -D warnings && cargo test`
7. Measure final binary size and update this doc

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| tantivy call sites missed by audit | `cargo check --no-default-features` will catch compile errors |
| reqwest rustls-tls breaks TLS behavior | reqwest rustls-tls is well-tested; rmcp already uses rustls so same root CA stack |
| Fat LTO breaks CI cache efficiency | Rust cache keys by OS+feature set; first run cold, subsequent runs warm |
| macOS CI takes longer with codegen-units=1 | Accept; CI time still net-improves from dropping Windows runner |
