---
id: 4c6d164c72f1d747
kind: bug
status: fixed
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

Added the chunk's `start_line` as a third component:
`{project_id}:{path}:{start_line}:{content_hash}` (`src/retrieval/sync.rs`, `chunk_id`).
`start_line` was chosen over a bare loop ordinal because `split_file`'s chunk struct
already carries it (`CodePayload.start_line` already threads it through), and because
the loop that builds ids **skips** empty/whitespace-only chunks via `continue` before
reaching the id call — a raw `enumerate()` index would number *surviving* chunks, not
the chunker's own output, which is one extra thing to get wrong for no benefit over a
value already in hand. Updated both call sites (`stream_index`, `sync_worktree`).

**`drift.rs`'s dirty-set derivation, re-read as this bug's own Resume asked:** it needs
no change. `dirty_paths` keys on the separate `(file_path, content_hash)` fields on
`ChunkRef`/`LocalChunk` — it never touches the `chunk_id` string's internal shape at
all, so widening that shape is invisible to it. (It does have its own, narrower
version of a duplicate-content blind spot — `main_pairs`/`local_pairs` are `HashSet`s,
so two same-content chunks in one file collapse to one set entry there too — but that
affects only the worktree-delta dirty/clean file-level decision, not point identity,
and `dirty_paths` isn't on the `stream_index`/fresh-index path this bug measured. Not
fixed here; noted for whoever next touches worktree delta sync.)

The tradeoff this bug's Fix section flagged going in — an ordinal/position-bearing id
churns points on any insertion above it, where the old content-hash-only id was
idempotent across such moves — is accepted, as reasoned there: a stable id that
silently drops 11% of the corpus is worse than one that re-upserts unchanged content
after an edit shifts line numbers.

**Deferred, not fixed here:** the count-reconciliation warning on `sync_project`
("whatever the id becomes... reconcile its reported count against the store's"). It's
no longer required for correctness — ids don't collide, so `points_count` and the
walk's own count now agree by construction — and it would touch `stream_index`'s
return arity at all 11 call sites plus both downstream consumers
(`src/bin/sync_project.rs`, `src/tools/semantic/index.rs`). Left as a follow-up; it
remains worthwhile as an independent observability improvement, not as a dependency of
this fix.
## Tests added

Three, in `src/retrieval/sync.rs`:

- `chunk_id_disambiguates_duplicate_content_in_the_same_file` — unit-level: two
  `chunk_id()` calls, same project/path/hash, different `start_line`, assert `!=`.
- `stream_index_disambiguates_duplicate_content_chunks_in_one_file` — the load-bearing
  end-to-end case the bug's own template asked for. `.toml` has no tree-sitter grammar
  (`get_ts_language("toml")` is `None`), so `split_file` falls through to the
  size-driven plain-text line splitter — fully controllable: a 3-line block repeated
  back-to-back, with `chunk_target` set to exactly that block's packed byte cost,
  deterministically produces two chunks with byte-identical content (asserted via
  equal `content_hash`). Asserts the two upserted `chunk_id`s are distinct.
  **Mutation-verified**: pinning the id's position component to a constant (undoing
  the fix) reproduces the exact pre-fix failure — `ids.len() == 1` where the assertion
  expects `2`, both entries showing the identical id string.
- `chunk_id_normalizes_native_separators` — updated for the new 4-part id shape.

Full gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--lib` — 3914 passed, 0 failed, 7 ignored.
## Workarounds

None. Treat `points_count` as the real corpus size and the sync's `+N` as an upper bound.

## Resume

Fixed and closed. One deferred follow-up, not required for this fix's correctness:
the count-reconciliation warning on `sync_project` (see § Fix, "Deferred, not fixed
here") — an independent observability improvement, touching `stream_index`'s return
arity at all 11 call sites plus both downstream consumers. And drift.rs's own
narrower duplicate-content blind spot in the worktree-delta dirty/clean decision (see
§ Fix) — file-level, not point-identity, and out of scope for this bug's
reproduction.
## References

- `src/retrieval/sync.rs:77` — `chunk_id`
- `src/retrieval/qdrant.rs:28` — `chunk_id_to_point_id`
- `src/retrieval/code_store.rs:464` — last-wins semantics in the test double
- `docs/trackers/retrieval-benchmark.md` — the corpus this was measured on
