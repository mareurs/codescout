---
id: '95eb4d1e8f19009d'
kind: bug
status: open
title: chunk_id omits the chunk index, so duplicate-content chunks in one file collapse to a single point
tags:
- retrieval
- indexing
- qdrant
- silent-loss
---

## Summary

`chunk_id` is `{project_id}:{file_path}:{content_hash}` with no chunk index, and the
Qdrant point id is derived from it. Two chunks in the same file with identical content
therefore produce the same point id, and the second silently overwrites the first.
Measured on a fresh index: **19,216 chunks produced, 17,108 points stored — 2,108 lost,
10.97%**. `sync_project` reports all 19,216 as added.

## Symptom (Effect)

```
$ ./target/release/sync_project .worktrees/bench
done: +19216 -0 ~0 chunks in 323862ms

$ curl -s localhost:6333/collections/bench_coderank_code_chunks
points_count: 17108      indexed_vectors_count: 17108      status: green
```

A second, no-op sync agrees nothing is missing:

```
done: +0 -0 ~0 chunks in 1748ms
```

So the loss is invisible from both ends: the writer reports success for every chunk, and
the drift check afterwards reports nothing to do.

## Reproduction

Index any repo with repeated identical chunk content and compare the reported chunk count
against the collection's `points_count`. Observed 2026-08-16 on the pinned bench corpus
(851 files at `ede25e69`), `desktop-threadripper`.

## Environment

codescout `experiments` @ `148aabe6`, Qdrant stack, CodeRankEmbed-Q4_K_M dense +
Splade_PP sparse. `host: desktop-threadripper`.

## Root cause

`src/retrieval/sync.rs:77`:

```rust
pub fn chunk_id(project_id: &str, rel_path: &Path, content_hash: &str) -> String {
    format!("{project_id}:{}:{content_hash}", to_forward_slash(rel_path))
}
```

The tuple is `(project, path, content_hash)` — **no ordinal**. Two chunks from the same
file whose text hashes identically are the same id by construction.

The point id is that string, hashed: `src/retrieval/qdrant.rs:28`

```rust
/// Qdrant point IDs must be u64 or UUID — hash the chunk_id string to u64.
fn chunk_id_to_point_id(s: &str) -> u64 {
    let hash = sha2::Sha256::digest(s.as_bytes());
    u64::from_le_bytes(hash[..8].try_into().unwrap())
}
```

Upsert is last-wins on that id, so the collision is a silent overwrite rather than an
error. The in-memory test double encodes the same semantics explicitly
(`src/retrieval/code_store.rs:464`): `store.retain(|existing| existing.chunk_id != p.chunk_id)`
then push.

*Measured 2026-08-16 by the counts above; mechanism read at the two cited lines.*

## Evidence

**The 64-bit truncation is NOT the cause, and ruling it out is what points at `chunk_id`.**
For ~19.2k items in a 2^64 space the expected number of birthday collisions is on the order
of 1e-11 — it cannot produce 2,108. The collapse therefore happens upstream, in `chunk_id`
itself, where duplicate `(path, content_hash)` pairs are genuinely equal strings.

## Hypotheses tried

1. **Hypothesis:** the points are still being written and Qdrant's optimizer is lagging.
   **Test:** re-read `points_count` after the sync returned and again minutes later;
   `status: green`, `indexed_vectors_count == points_count == 17108` both times.
   **Verdict:** rejected — the number is stable, not settling.
2. **Hypothesis:** truncating SHA-256 to u64 collides at this corpus size.
   **Verdict:** rejected on arithmetic (see Evidence).

## Fix

Not implemented. Add the chunk ordinal (or the start line) to the id:
`{project_id}:{path}:{ordinal}:{content_hash}`. Note the tradeoff the current shape buys:
a content-hash-only id makes re-indexing idempotent when a chunk moves within a file
unchanged, whereas an ordinal-bearing id churns points on any insertion above it. That is
probably the right price — a stable id that silently drops 11% of the corpus is worse than
one that re-upserts unchanged content — but it deserves a deliberate decision, and
`drift.rs`'s dirty-set derivation should be re-read before changing the shape.

Whatever the id becomes, `sync_project` should reconcile its reported count against the
store's and warn when they disagree. The current silence is what let this sit unmeasured.

## Tests added

None yet. The natural regression test: index a fixture file containing the same chunk body
twice, assert `points_count == chunks_produced`. It fails today.

## Workarounds

None. Treat `points_count` as the real corpus size and the sync's `+N` as an upper bound.

## Resume

Decide the id shape (ordinal vs start-line vs content-hash-plus-ordinal) against
`src/retrieval/drift.rs`'s dirty-set logic, then add the count-reconciliation warning to
`sync_project` regardless of which id shape wins.

## References

- `src/retrieval/sync.rs:77` — `chunk_id`
- `src/retrieval/qdrant.rs:28` — `chunk_id_to_point_id`
- `src/retrieval/code_store.rs:464` — last-wins semantics in the test double
- `docs/trackers/retrieval-benchmark.md` — the corpus this was measured on

