# Fix: BGESmallENV15Q CPU-Safety Claims and Ollama Fallback

**Date:** 2026-03-03
**Status:** Approved

---

## Problem

`BGESmallENV15Q` (backed by `Qdrant/bge-small-en-v1.5-onnx-Q` on HuggingFace) was exported
with `optimize_for_gpu: true` and `fp16: true`. This means:

- Weights are stored as float16, not INT8.
- The ONNX graph uses GPU-fused operators (`SkipLayerNormalization` with FP16 inputs) that have
  no CPU kernel in ORT 1.20.
- The model fails at runtime on any CPU-only machine, regardless of ORT version.

Despite this, the codebase promoted `BGESmallENV15Q` as the CPU-safe default in two ways:

1. **As the automatic Ollama fallback** (`const FALLBACK: &str = "BGESmallENV15Q"` in
   `src/embed/mod.rs`) — triggered whenever Ollama is unreachable on a machine built with
   both `remote-embed` and `local-embed` features.
2. **In all documentation** as "fast on CPU", "no GPU needed", and "recommended for most users".

The correct CPU-safe quantized model is `AllMiniLML6V2Q` (backed by `Xenova/all-MiniLM-L6-v2`),
which uses standard `QuantizeLinear`/`DequantizeLinear` INT8 ops with universal CPU kernels.

---

## Solution: Approach A — Targeted Swap

Replace `BGESmallENV15Q` with `AllMiniLML6V2Q` everywhere it appears as a default or CPU
recommendation. Keep `BGESmallENV15Q` as a supported named option with a corrected description.

---

## Touch Points

### Code (4 locations across 2 files)

| File | Symbol / Line | Change |
|---|---|---|
| `src/embed/mod.rs` | `FALLBACK` constant (~line 153) | `"BGESmallENV15Q"` → `"AllMiniLML6V2Q"` |
| `src/embed/mod.rs` | Feature-missing error message (~line 185–187) | Remove BGESmallENV15Q from "Recommended" list; list AllMiniLML6V2Q first with the "CPU/WSL2" label |
| `src/embed/local.rs` | `parse_model` error message (~line 49) | Change BGESmallENV15Q description from "fast CPU" to note GPU-export risk |
| `src/config/project.rs` | Doc comment (~line 45) | Remove "fast CPU" claim for BGESmallENV15Q |

### Documentation (3 files, 9 locations)

**`docs/manual/src/configuration/embedding-backends.md`**

- Recommended Models table: swap `local:BGESmallENV15Q` → `local:AllMiniLML6V2Q` as the local
  representative; update Notes for BGESmallENV15Q to flag GPU-export risk.
- Automatic CPU Fallback section: update model name and code snippet throughout.
- Local backend configuration example: change to `AllMiniLML6V2Q`.
- Supported Local Models table + prose: fix "recommended for most users" / "fast CPU inference"
  claim; add a caveat for BGESmallENV15Q.
- Choosing a Backend section: fix both fallback mentions.

**`docs/manual/src/semantic-search-guide.md`**

- Backend comparison table local: example: `BGESmallENV15Q` → `AllMiniLML6V2Q`.

**`docs/manual/src/configuration/project-toml.md`**

- Full example config: change `local:BGESmallENV15Q` → `local:AllMiniLML6V2Q`.

---

## What We Are NOT Changing

- `BGESmallENV15Q` remains a valid named option in `parse_model` — users who explicitly
  configure it can still use it (e.g. on a GPU machine).
- No tests need updating — `parse_model_known_names_return_ok` just asserts the name parses,
  which remains true. `chunk_size_local_bge_small` tests chunk size math, not CPU compatibility.
- No new runtime detection or model deny-list — out of scope.
- No WSL2/corporate proxy guide — documented externally; out of scope for this fix.

---

## Acceptance Criteria

- [ ] `cargo test` passes (no regressions).
- [ ] `cargo clippy -- -D warnings` clean.
- [ ] Grepping for `BGESmallENV15Q` in live docs returns only the model's own entry with a
      corrected description (no remaining "recommended", "no GPU needed", "fast CPU" claims).
- [ ] The automatic Ollama fallback warning message names `AllMiniLML6V2Q`.
