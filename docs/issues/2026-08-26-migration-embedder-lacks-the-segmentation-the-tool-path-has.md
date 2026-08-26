---
status: open
opened: 2026-08-26
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
tags: [memory, migration, embedder, segmentation]
kind: bug
---

# BUG: `migrate-memories` embeds raw, so it cannot repair a memory larger than the embedder's per-request ceiling

## Summary

`26feb1aa` fixed oversized memory embeds by segmenting and mean-pooling — but only in the
**tool** layer (`cross_embed_memory`, `create_semantic_anchors`). `HttpMigrationEmbedder`
calls the embedder raw, so `migrate-memories --in-place` and `embed_missing_memories`
inherit the original defect. Measured 2026-08-26: the repair recovered 7 of the 8
memories missing from this repo's store and failed on the 8th, `eval-design` (31 596 B),
with the embedder's per-request ceiling error.

## Symptom (Effect)

`./target/release/codescout migrate-memories --in-place`, 2026-08-26 at `6f6d2023`+:

```
WARN codescout::migrate::memories: embed-missing: embed failed for eval-design:
  dense openai status 500: {"error":{"code":500,
  "message":"input is too large to process. increase the physical batch size",
  "type":"server_error"}}
INFO codescout: migrate-memories --in-place: 19 re-derived, 6 newly embedded from disk
{"read":26,"upserted":25,"skipped":1,"anchors_attached":44,"dry_run":false,
 "mode":"in-place-reembed"}
```

That is the same llama.cpp `n_batch` wording pinned in `5f8c42ec` and recorded in
`bug-fix-session-log:W-57` — not the `n_ctx`-per-slot wording the original issue quoted.

Post-run state, measured against the store directly:

```
on disk 23   in store(structured) 24
still missing: ['eval-design']
orphans      : ['prompt-tdd-skill-eval-confounds', 'zz-probe-delete-me']
```

**The failure is non-destructive and reported** — `skipped: 1`, the existing corpus
untouched. That contract held; the gap is that the memory cannot be recovered at all.

## Reproduction

```
./target/release/codescout migrate-memories --in-place --dry-run   # skipped: 0 — see below
./target/release/codescout migrate-memories --in-place             # the real answer
```

**`--dry-run` cannot surface this.** It increments `upserted` and `continue`s *before*
calling the embedder, so `skipped: 0` on a dry run says nothing about embeddability — it
is structurally incapable of reporting this class. A caller who dry-runs first and trusts
the zero learns nothing.

## Environment

- Linux, branch `experiments`, HEAD `6f6d2023`+ (release build)
- Embedder: llama-server / CodeRankEmbed over the OpenAI-compatible route
- Backend: Qdrant, `memories` collection

## Root cause

Two embed paths, one segmented and one not.

- **Tool layer, segmented.** `embed_document_pooled` → `segment_for_budget` +
  `mean_pool_normalized`, using `chunk_size_for_model(&model_spec)`
  (`src/tools/memory/mod.rs`, added in `26feb1aa`). Both `cross_embed_memory` and
  `create_semantic_anchors` go through it.
- **Migration layer, raw.** `HttpMigrationEmbedder::embed`
  (`src/migrate/memories.rs:53-62`) delegates straight to the underlying
  `CodeEmbedder`. No budget, no segmentation, no pooling.

So `26feb1aa` fixed every *future* tool-mediated write and left the migration path — the
one whose entire job is repairing what the old defect lost — on the pre-fix code.

Note the overlap, but do not overstate it: the two blind spots are the same population
(oversized content). Empirically only 1 of the 8 missing memories here was oversized; the
other 7 were simply never written through the tool since cross-embedding landed. So this
is a real gap with a narrow measured blast radius, not a systematic bias that would have
made the repair useless.

## Evidence

### The two paths, side by side

`src/migrate/memories.rs:53-62` — raw:

```rust
impl MigrationEmbedder for HttpMigrationEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> { /* delegates to inner */ }
}
```

`src/tools/memory/mod.rs` — segmented, and its own comment says why:

> Segmented on the same budget as `cross_embed_memory`, and for the same reason: this call
> re-upserts the SAME point id, so leaving it unsegmented would let the anchor pass
> overwrite a correctly-pooled vector with a truncated or missing one.

### Sizes, measured

`eval-design` is 31 596 B. The next largest memory on disk is `test-design-discipline` at
19 401 B, which already **has** a point (it was re-derived successfully in the same run) —
so the ceiling sits between those two for this model and this `n_batch`, consistent with
the ≈2048-token figure measured for CodeRankEmbed in
`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`.

## Hypotheses tried

1. **Hypothesis:** the dry run would have predicted the failure.
   **Test:** ran `--in-place --dry-run` first; it reported `skipped: 0`.
   **Verdict:** rejected, and the reason is structural — the dry-run branch returns before
   the embed call. Recorded in Reproduction because a caller could reasonably trust it.

## Fix

*Not yet implemented.*

Lift the segmentation helpers out of the tool layer and use them in
`HttpMigrationEmbedder`. `segment_for_budget`, `mean_pool_normalized` and
`embed_document_pooled` currently live in `src/tools/memory/mod.rs`; the migration crate
cannot reach a private tool-module item, so they need a shared home — `src/embed/` already
hosts `chunk_size_for_model`, which they depend on, so that is the natural one.

Do **not** fix this by raising the server's `--batch-size`: that is a deployment change
that fixes one operator's box and leaves every other stack broken, and the ceiling is the
model's, not the config's — the point established in
`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`
§ *"The ceiling is the model's, not the config's — this inverts the fix"*.

While in there, consider making `--dry-run` embed-but-not-upsert, so its `skipped` count
means something. That is the difference between a dry run that predicts the real run and
one that only counts rows.

## Tests added

None yet. The regression test embeds a document larger than the model's budget through
`HttpMigrationEmbedder` and asserts a unit-norm vector comes back rather than an error —
the same shape as `tools::memory::tests::under_budget_makes_one_call_and_over_budget_pools_to_unit_norm`,
which already pins this for the tool path and is the test the migration path lacks.

## Workarounds

Re-write the memory through the tool path, which *is* segmented:
`memory(action="write", topic="eval-design", content=<current disk content>)`. For a human
with the file open this is fine. **For an agent it is not** — 31 596 B exceeds the inline
result budget, so the content comes back as a buffer handle that cannot be re-emitted as a
tool argument. That asymmetry is exactly why `embed_missing_memories` exists, and it is
why this bug blocks the last memory rather than merely inconveniencing it.

## Resume

Read `src/tools/memory/mod.rs`'s `embed_document_pooled` and decide its new home
(`src/embed/` is the candidate — `chunk_size_for_model` already lives there and is its
dependency). Then `HttpMigrationEmbedder::embed` wraps it. Verify with
`./target/release/codescout migrate-memories --in-place` and confirm `skipped: 0` and
`eval-design` present in the `structured` bucket — currently the one memory still missing
of this repo's 23.

## References

- `src/migrate/memories.rs:53-62` (`HttpMigrationEmbedder::embed`), `:292-370`
  (`embed_missing_memories`)
- `src/tools/memory/mod.rs` — `embed_document_pooled`, `segment_for_budget`,
  `mean_pool_normalized`
- `26feb1aa` — the segmentation fix that covered only the tool layer
- `bug-fix-session-log:W-57` — the `n_batch` vs `n_ctx` wording distinction
- `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` — the
  parent bug; this is the gap its own repair path left
