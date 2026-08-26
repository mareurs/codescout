---
status: open
opened: 2026-08-26
closed:
severity: low
owner: marius
related: [docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md]
tags: [embeddings, docs, onnx, retrieval]
unverified: 'both reported blockers are working-as-designed with documented remedies — the only defect here is the undocumented migration cost. The CodeRankEmbed-on-ONNX request is a feature, not a bug, and is recorded here only because it has no tracker home yet.'
kind: bug
---

# BUG: the ONNX local-embed path is recommended as an escape from #15 without documenting that it forces a model change, a full reindex, and dense-only

## Summary

A host running CodeRankEmbed over llama-server was advised to move to the
in-process ONNX path to escape the slot-context drops of
`docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`. The
reporter audited the code and found that move is not vector-compatible: the ONNX
path cannot load CodeRankEmbed at all, and it cannot run the hybrid sparse leg.
Both behaviors are deliberate and well-guarded — **the defect is that neither
cost is written down anywhere**, so the only way to discover it is by reading
`crates/codescout-embed/src/local.rs`. The reporter did exactly that.

Imported from GitHub issue #16 (reporter: mic-urs, 2026-08-20). Both claims
verified at `d5ed4d6f` on 2026-08-26 and both are accurate.

## Symptom (Effect)

No error message and no crash — the reporter never got as far as running
anything. The symptom is guidance that reads as cheap and is not:

> an existing CodeRankEmbed (768-dim) deployment switching to ONNX today must
> change the embedding model entirely → all stored vectors become incompatible →
> **full reindex of every code index and every memory**, plus an unmeasured
> retrieval-quality regression.

## Reproduction

Documentation absence, not a runtime failure. To confirm the two constraints:

1. `grep -rn 'CodeRankEmbed' crates/codescout-embed/src/` → no registry entry.
2. Set a local backend with the sparse leg enabled and `CODESCOUT_DISABLE_SPARSE`
   unset → `guard_sparse` refuses with a config-conflict error.
3. `grep -rn 'ONNX\|local:' docs/` for a statement of the migration cost → absent.

## Environment

- Reporter: codescout `experiments` (build 2026-08-19), sqlite-vec backend,
  dense = CodeRankEmbed-Q4_K_M via llama-server, sparse + reranker active.
- Re-verified at `d5ed4d6f`, 2026-08-26.

## Root cause

Both constraints are real, intentional, and correctly implemented. Neither is a
code defect; the gap is that they are invisible outside the source.

### Constraint 1 — the ONNX path has no CodeRankEmbed, by design

Two sub-paths, both closed:

- **`local:<name>`** resolves through `parse_model`
  (`crates/codescout-embed/src/local.rs:234-255`), an explicit `match` allowlist:
  `NomicEmbedTextV15`, `NomicEmbedTextV15Q`, `JinaEmbeddingsV2BaseCode`,
  `BGESmallENV15Q`, `AllMiniLML6V2Q`, `BGESmallENV15`, `AllMiniLML6V2`.
  CodeRankEmbed is absent. The fallback arm `anyhow::bail!`s with a well-written
  list of every supported variant, its dimension and its size — so the failure
  is loud and self-documenting *at the point of use*.
- **`local-dir:<path>`** goes through `LocalEmbedder::from_dir_blocking`
  (`:192-231`), which applies the module-level `LOCAL_MODEL_POOLING` and
  `LOCAL_MODEL_QUANTIZATION` constants (`:68-69`) — AllMiniLM-L6-v2-Q's
  parameters — regardless of what the directory actually contains. The in-code
  comment already documents this and names the hazard: another model's weights
  in that directory yield "plausible but silently wrong vectors".

So `local-dir:` is not a generic loader; it is AllMiniLM with a configurable
path. The reporter's read is exact.

`inferred from crates/codescout-embed/src/local.rs:68-69,192-231,234-255 — read
2026-08-26.`

### Constraint 2 — `guard_sparse` refusing local backends is deliberate

`src/retrieval/client.rs:170-182`. Its own doc comment is the rationale:

```rust
/// A local backend emits no sparse vector. Silently dropping to dense would
/// show up as degraded recall and never as a failure, so it is an error —
/// but an expected, operator-fixable one (a config conflict, not a bug),
/// so `RecoverableError` (isError: false, sibling parallel calls survive)
/// rather than `anyhow::bail!` (isError: true, aborts siblings).
```

It carries a documented remedy: `CODESCOUT_DISABLE_SPARSE=1` for dense-only, or
an embedder URL serving both legs. This is the correct behavior and should not
change — a silent drop to dense-only is exactly the failure mode codescout
treats as unacceptable elsewhere.

## Evidence

### `parse_model`'s allowlist (`symbols include_body`, 2026-08-26)

```rust
fn parse_model(name: &str) -> Result<fastembed::EmbeddingModel> {
    match name {
        "NomicEmbedTextV15"        => Ok(…NomicEmbedTextV15),
        "NomicEmbedTextV15Q"       => Ok(…NomicEmbedTextV15Q),
        "JinaEmbeddingsV2BaseCode" => Ok(…JinaEmbeddingsV2BaseCode),
        "BGESmallENV15Q"           => Ok(…BGESmallENV15Q),
        "AllMiniLML6V2Q"           => Ok(…AllMiniLML6V2Q),
        "BGESmallENV15"            => Ok(…BGESmallENV15),
        "AllMiniLML6V2"            => Ok(…AllMiniLML6V2),
        other => anyhow::bail!("Unknown local model '{other}'. Supported variants: …"),
    }
}
```

### `local-dir:` is model-fixed (`symbols`, 2026-08-26)

`crates/codescout-embed/src/local.rs` exposes `LOCAL_MODEL_POOLING` and
`LOCAL_MODEL_QUANTIZATION` as module **constants** at `:68-69`, consumed by
`from_dir_blocking` at `:192-231`. Its tests pin this deliberately:
`from_dir_produces_a_stable_384d_vector` (`:335`),
`from_dir_matches_the_hub_path_for_the_same_model` (`:387`),
`from_dir_pooling_and_quantization_match_the_hub_registry` (`:454`).

## Hypotheses tried

1. **Hypothesis:** `local-dir:` is a generic ONNX loader and CodeRankEmbed
   weights would just work.
   **Test:** read `from_dir_blocking` and the pooling/quantization constants.
   **Verdict:** rejected. Fixed AllMiniLM parameters; wrong-model weights give
   silently wrong vectors, as the in-code comment warns.
2. **Hypothesis:** `guard_sparse` is an over-strict guard worth relaxing.
   **Test:** read its doc comment and error hint.
   **Verdict:** rejected. It is the intended design with a documented remedy,
   and the alternative (silent dense-only) is the worse failure.

## Fix

The in-scope, actionable fix is documentation. Split explicitly:

1. **[bug — do this] Document the migration cost** where the ONNX/local path is
   described (retrieval-stack docs). One short note, in the reporter's own
   words: *"the ONNX local path currently implies a model change, a full reindex
   of every code index and every memory, and dense-only retrieval."* That is
   what would have saved an audit.
2. **[bug — do this] Stop recommending ONNX as the remedy for
   `…-dense-embedder-slot-context-drops-large-embeds.md`** until (3) exists. The
   segmentation fix in that bug is the real remedy and is model-independent.
3. **[feature — not this file] Generic user-defined-model loading on the ONNX
   path**: load ONNX weights from a directory with *explicit* pooling and
   quantization parameters rather than compiled-in constants. The reporter's
   observation that CodeRankEmbed derives from nomic-embed-text-v1.5 — already
   in fastembed's registry as `NomicEmbedTextV15` — makes this plausibly cheap
   and would make the migration vector-compatible: same model, same dimension,
   no reindex. Per CLAUDE.md this belongs in `docs/trackers/` or `docs/plans/`,
   not `docs/issues/`; it is recorded here only so the request is not lost, and
   should be moved when a home exists.
4. **[answer the reporter] Is dense-only the intended end-state for the local
   path, or is hybrid-with-local-backend on the roadmap?** A direct question in
   the issue, currently unanswered. It is a roadmap decision, not a code change.

No SHA, no patch-id.

## Tests added

`N/A` — the fix is a documentation note plus a roadmap answer; there is no
behavior change to regress. If item 3 is ever built, it needs its own coverage
(a `from_dir` variant asserting explicit pooling/quantization round-trip, and a
dimension-compatibility assertion against an index built by the HTTP path).

## Workarounds

Stay on the llama-server container. It is the only path that serves
CodeRankEmbed *and* the sparse leg, so it remains correct for this deployment.
The slot-context ceiling that motivated the move is better addressed by the
segmentation fix in the related bug, which does not require changing models.

## Resume

Find where the ONNX / `local:` / `local-dir:` backends are described for
operators — start with `grep -rn 'local-dir:\|local:' docs/ README.md` and the
`scripts/retrieval-stack.sh` header comments — and add the three-clause cost
note from Fix item 1 at whichever surface an operator actually reads before
switching. Then decide item 4 and answer GitHub #16, since the reporter is
blocked on a roadmap answer rather than on code.

## References

- GitHub issue #16 — <https://github.com/mareurs/codescout/issues/16>
- `crates/codescout-embed/src/local.rs:68-69` (fixed pooling/quantization),
  `:192-231` (`from_dir_blocking`), `:234-255` (`parse_model` allowlist)
- `src/retrieval/client.rs:170-182` (`guard_sparse` and its rationale)
- `docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` —
  the bug this was proposed as an escape from
