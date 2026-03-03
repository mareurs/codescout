# Fix BGESmallENV15Q CPU Fallback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace `BGESmallENV15Q` with `AllMiniLML6V2Q` as the default CPU fallback everywhere it
is promoted as CPU-safe, while keeping `BGESmallENV15Q` as a named (but correctly-described) option.

**Architecture:** Pure text changes — two Rust source files and three Markdown docs. No new logic,
no new tests. Existing tests keep passing because `BGESmallENV15Q` remains a valid `parse_model`
variant; only its promotion as the CPU default is removed.

**Tech Stack:** Rust (string literals in `src/`), Markdown (`docs/manual/src/`). Use
`mcp__code-explorer__edit_file` for all edits.

---

## Background

`BGESmallENV15Q` maps to `Qdrant/bge-small-en-v1.5-onnx-Q` on HuggingFace. That model was
exported with `optimize_for_gpu: true` and `fp16: true`, meaning its ONNX graph uses GPU-fused
operators with no CPU kernel in ORT 1.20. It fails at runtime on any CPU-only machine.

`AllMiniLML6V2Q` maps to `Xenova/all-MiniLM-L6-v2`. It uses standard `QuantizeLinear` /
`DequantizeLinear` INT8 ops — universal CPU kernels, confirmed working on CPU-only WSL2.

Design doc: `docs/plans/2026-03-03-fix-bge-cpu-fallback-design.md`

---

### Task 1: Fix the Ollama fallback constant in `src/embed/mod.rs`

**Files:**
- Modify: `src/embed/mod.rs` (two changes inside `create_embedder`)

**Step 1: Change the FALLBACK constant**

Use `edit_file` with:

```
old_string: "                const FALLBACK: &str = \"BGESmallENV15Q\";"
new_string: "                const FALLBACK: &str = \"AllMiniLML6V2Q\";"
```

**Step 2: Verify the warning message wording**

The warning uses `{FALLBACK}` interpolation, so it automatically says `AllMiniLML6V2Q`. The
`~20 MB` size claim needs updating to `~22 MB`:

```
old_string: "{e}. Falling back to local:{FALLBACK} (CPU-friendly, ~20 MB). \\"
new_string: "{e}. Falling back to local:{FALLBACK} (CPU-friendly, ~22 MB). \\"
```

**Step 3: Fix the feature-missing error recommendation**

```
old_string:
             • local:BGESmallENV15Q             (384d, quantized, ~20MB, fast)"

new_string:
             • local:AllMiniLML6V2Q             (384d, quantized, ~22MB, CPU-safe)"
```

**Step 4: Run tests**

```bash
cargo test --lib embed
```

Expected: all tests pass. `local_prefix_returns_helpful_error` and
`chunk_size_local_bge_small` are not affected — BGESmallENV15Q is still a valid parse target.

**Step 5: Commit**

```bash
git add src/embed/mod.rs
git commit -m "fix(embed): swap Ollama CPU fallback from BGESmallENV15Q to AllMiniLML6V2Q"
```

---

### Task 2: Fix error messages and doc comments in `src/embed/local.rs` and `src/config/project.rs`

**Files:**
- Modify: `src/embed/local.rs` (the `parse_model` error message)
- Modify: `src/config/project.rs` (a doc comment on the `model` field)

**Step 1: Fix `parse_model` error in `src/embed/local.rs`**

Change BGESmallENV15Q's description from "fast CPU" to a GPU-risk warning:

```
old_string:
             • local:BGESmallENV15Q             (384d, quantized, ~20MB, fast CPU)\n\

new_string:
             • local:BGESmallENV15Q             (384d, GPU-optimized export; may fail on CPU)\n\
```

**Step 2: Fix doc comment in `src/config/project.rs`**

```
old_string:
    ///   "local:BGESmallENV15Q"              → 384d, quantized, ~20MB, fast CPU

new_string:
    ///   "local:BGESmallENV15Q"              → 384d, GPU-optimized export; may fail on CPU
```

**Step 3: Run tests**

```bash
cargo test --lib embed
```

Expected: all tests pass. `parse_model_known_names_return_ok` still passes because the match
arm for `"BGESmallENV15Q"` is unchanged — only the error message changes.

**Step 4: Run clippy**

```bash
cargo clippy -- -D warnings
```

Expected: clean.

**Step 5: Commit**

```bash
git add src/embed/local.rs src/config/project.rs
git commit -m "fix(embed): correct BGESmallENV15Q description — GPU-optimized export, not CPU-safe"
```

---

### Task 3: Fix `docs/manual/src/configuration/embedding-backends.md`

This file has 5 independent locations. Apply them top-to-bottom.

**Files:**
- Modify: `docs/manual/src/configuration/embedding-backends.md`

**Step 1: Recommended Models table — swap the `local:` representative**

```
old_string:
| `local:BGESmallENV15Q`          | fastembed|  384 | Medium (CPU)   | Good         | Air-gapped or no daemon; no GPU needed    |

new_string:
| `local:AllMiniLML6V2Q`          | fastembed|  384 | Medium (CPU)   | Good         | Air-gapped or no daemon; CPU-safe INT8    |
```

**Step 2: Automatic CPU Fallback — update model name in prose and warning block**

```
old_string:
it automatically falls back to `local:BGESmallENV15Q` and emits a warning:

```
Ollama not reachable at http://localhost:11434: …
Falling back to local:BGESmallENV15Q (CPU-friendly, ~20 MB).

new_string:
it automatically falls back to `local:AllMiniLML6V2Q` and emits a warning:

```
Ollama not reachable at http://localhost:11434: …
Falling back to local:AllMiniLML6V2Q (CPU-friendly, ~22 MB).
```

**Step 3: Automatic CPU Fallback — update the "make permanent" TOML snippet**

```
old_string:
To silence the warning and make the
fallback permanent, set the model explicitly:

```toml
[embeddings]
model = "local:BGESmallENV15Q"
```

new_string:
To silence the warning and make the
fallback permanent, set the model explicitly:

```toml
[embeddings]
model = "local:AllMiniLML6V2Q"
```
```

**Step 4: Local section — configuration example**

```
old_string:
```toml
[embeddings]
model = "local:BGESmallENV15Q"
```

### Supported Local Models

new_string:
```toml
[embeddings]
model = "local:AllMiniLML6V2Q"
```

### Supported Local Models
```

**Step 5: Supported Local Models table row and prose**

Fix the table row for BGESmallENV15Q:

```
old_string:
| `local:BGESmallENV15Q` | 384 | ~20 MB | Quantized, fast on CPU, recommended for most users |

new_string:
| `local:BGESmallENV15Q` | 384 | ~20 MB | GPU-optimized ONNX export; may fail on CPU-only ORT |
```

Fix the prose below the table:

```
old_string:
For most local setups, `BGESmallENV15Q` gives the best tradeoff: small download, fast CPU
inference, and solid retrieval quality. Use `JinaEmbeddingsV2BaseCode` when search quality
on code is the priority and the larger download is acceptable.

new_string:
For most local setups, `AllMiniLML6V2Q` gives the best tradeoff: small download, fast CPU
inference, and solid retrieval quality. Use `JinaEmbeddingsV2BaseCode` when search quality
on code is the priority and the larger download is acceptable. `BGESmallENV15Q` uses a
GPU-optimized ONNX export and may fail on CPU-only machines.
```

**Step 6: Choosing a Backend — update both fallback mentions**

```
old_string:
  `ollama:mxbai-embed-large`. If Ollama is absent, it falls back to `local:BGESmallENV15Q`

new_string:
  `ollama:mxbai-embed-large`. If Ollama is absent, it falls back to `local:AllMiniLML6V2Q`
```

```
old_string:
- **You are on an air-gapped machine or want complete data privacy** → use
  `local:BGESmallENV15Q` (build with `--features local-embed`).

new_string:
- **You are on an air-gapped machine or want complete data privacy** → use
  `local:AllMiniLML6V2Q` (build with `--features local-embed`).
```

**Step 7: Verify no stale claims remain**

```bash
grep -n "BGESmallENV15Q" docs/manual/src/configuration/embedding-backends.md
```

Expected output: only the table row for BGESmallENV15Q itself (which now says
"GPU-optimized ONNX export") and the prose caveat. No occurrences of "no GPU needed",
"fast on CPU", "recommended for most users", or as a default/fallback.

**Step 8: Commit**

```bash
git add docs/manual/src/configuration/embedding-backends.md
git commit -m "docs: fix BGESmallENV15Q CPU-safety claims in embedding-backends guide"
```

---

### Task 4: Fix `semantic-search-guide.md` and `project-toml.md`

**Files:**
- Modify: `docs/manual/src/semantic-search-guide.md`
- Modify: `docs/manual/src/configuration/project-toml.md`

**Step 1: Fix `semantic-search-guide.md` backend table**

```
old_string:
| `local:` | `local:BGESmallENV15Q` | Offline / air-gapped, no daemon required |

new_string:
| `local:` | `local:AllMiniLML6V2Q` | Offline / air-gapped, no daemon required |
```

**Step 2: Fix `project-toml.md` full example config**

```
old_string:
[embeddings]
model = "local:BGESmallENV15Q"
drift_detection_enabled = true

new_string:
[embeddings]
model = "local:AllMiniLML6V2Q"
drift_detection_enabled = true
```

**Step 3: Verify — grep for any remaining false CPU claims**

```bash
grep -rn "BGESmallENV15Q" docs/manual/src/
```

Expected: only occurrences that correctly describe BGESmallENV15Q's GPU-export nature (in
`embedding-backends.md`). Zero occurrences in `semantic-search-guide.md` or `project-toml.md`.

**Step 4: Run full test suite**

```bash
cargo test
```

Expected: all tests pass.

**Step 5: Final clippy check**

```bash
cargo clippy -- -D warnings
```

Expected: clean.

**Step 6: Commit**

```bash
git add docs/manual/src/semantic-search-guide.md docs/manual/src/configuration/project-toml.md
git commit -m "docs: update local model examples to AllMiniLML6V2Q in search guide and project-toml ref"
```

---

## Acceptance Criteria

- [ ] `cargo test` passes.
- [ ] `cargo clippy -- -D warnings` clean.
- [ ] `grep -rn "BGESmallENV15Q" docs/manual/src/` returns only entries describing its
      GPU-export limitation — no "recommended", "no GPU needed", "fast CPU", or default/fallback references.
- [ ] The automatic Ollama fallback in `create_embedder` names `AllMiniLML6V2Q`.
- [ ] The `local:` row in the Recommended Models table shows `AllMiniLML6V2Q`.
