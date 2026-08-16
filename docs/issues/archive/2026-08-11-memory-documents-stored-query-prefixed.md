---
id: '022a6e2ed108a196'
kind: bug
status: fixed
title: 'Memory documents are stored query-prefixed: CodeDenseAdapter routes writes through the query seam'
owners:
- marius
tags:
- retrieval
- memory
- embeddings
- query-prefix
- asymmetric-embedding
- pre-existing
closed: 2026-08-14
---

# BUG: memory documents are embedded with the query prefix, because the write path runs through the query seam

## Summary

`CODESCOUT_QUERY_PREFIX` is meant to be applied **query-side only** — that is the
whole point of an asymmetric embedding model. But the memory *write* path reaches
the embedder through `CodeDenseAdapter`, which calls the query-side method, so
stored memory documents get the query prefix prepended before embedding.

Pre-existing on `master`; not introduced by the local-ONNX work. Found while
reviewing Task 7 of `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md`.

## Symptom (Effect)

No error. Memories are written and recalled successfully. The damage is silent:
every stored memory vector sits in query-space rather than document-space.

## Reproduction

Live on this machine's own configuration — no special setup:

1. `.codescout/project.toml` sets `[embeddings] url = "http://127.0.0.1:48081/v1"`,
   so `RetrievalConfig.embedder_url` is `Some` and the client builds `EmbedderHttp`.
2. `CODESCOUT_QUERY_PREFIX="Represent this query for searching relevant code: "`
   is exported in the shell environment. (`.env` has it commented out; the shell
   exports it anyway — worth noting, since reading `.env` alone would suggest it is
   inactive.) `EmbedderHttp::new` reads it at `src/retrieval/embedder.rs:252`.
3. Call `memory(action="write", …)` — or any path reaching `cross_embed_memory`
   (`src/tools/memory/mod.rs:353`) or `create_semantic_anchors` (`:396`).
4. Those push **document** content through `Agent::memory_embedder()` →
   `CodeDenseAdapter::embed` (`src/retrieval/embedder.rs:64-66`) →
   `EmbedderHttp::embed_dense_one` → `dense_query` (`:403-411`), which prepends
   `query_prefix` unconditionally.

## Environment

Linux, branch `feat/local-onnx-query-path` and equally `master`. Requires
`CODESCOUT_QUERY_PREFIX` non-empty, which is the configured state on this host.

## Root cause

A naming/contract mismatch that was harmless while one implementation existed.

`CodeEmbedder` is documented as the *"Query-side embedding seam for code search and
memory recall"* (`src/retrieval/embedder.rs:30`). `CodeDenseAdapter` bridges it into
the `DenseEmbedder` seam the memory path holds — and the memory path uses that seam
for **both** recall (a query) and write (a document). The adapter has no way to
distinguish them, so it calls the query method for both.

`dense_query` (`src/retrieval/embedder.rs:403-411`) prepends `query_prefix`
unconditionally, by design — it is the query method. The defect is the caller, not
the callee.

Measured 2026-08-11 by tracing the call chain in the review of commit `9a62a4a6`;
the prefix's presence in the live process env was confirmed by the reviewer, not
inferred from `.env`.


### Re-verified 2026-08-14 — mechanism confirmed, scope wider than filed

Every premise held at `65f1aa12`: `CODESCOUT_QUERY_PREFIX` is live on this host
(`Represent this query for searching relevant code: `), `CodeDenseAdapter::embed`
routed to `embed_dense_one` → `dense_query`, which prefixed unconditionally.

Two corrections to the filing, both found by reading further:

**1. § Severity note is right, and it is the load-bearing sentence.** `memory_embedder()`
is a `OnceCell`, so write and recall share one cached adapter — a symmetric shift
into query-space, not a mismatch. Any reading of this bug as "recall is broken"
is wrong, and the file says so. What it understates is *which surface pays*:
`embed_batch`'s own doc comment notes the doc-side batch path is prefix-free, so
**code search does asymmetry correctly** (prefixed query, unprefixed chunks).
Memory was the only surface prefixing both sides — the only one discarding the
asymmetry the model provides and code search exploits.

**2. There were FOUR write routes, not two (or three).** The filing lists
`cross_embed_memory`, `create_semantic_anchors`, and a branch-specific
`CodeEmbedderAdapter`. It misses `HttpMigrationEmbedder::embed`
(`src/migrate/memories.rs`), which called `embed_one` — the query seam. **The
migration tool re-embedded documents through the query path.** That inverts the
sequencing this file's § Resume proposes: a re-embed built on the existing
migration would have faithfully reproduced the defect it exists to repair, so the
seam had to land *before* the migration, not after.
## Evidence

### The write path reaches the query method

`cross_embed_memory` (`src/tools/memory/mod.rs:353`) and `create_semantic_anchors`
(`:396`) → `memory_embedder()` → `CodeDenseAdapter::embed`
(`src/retrieval/embedder.rs:64-66`) → `embed_dense_one` → `dense_query`.

### A second route exists on this branch

`CodeEmbedderAdapter::embed_dense_one` (`src/retrieval/embedder.rs:1734-1740`) calls
`codescout_embed::Embedder::embed_query`, which `RemoteEmbedder` overrides with the
CodeRank prefix (`crates/codescout-embed/src/remote.rs:338-352`). `LocalEmbedder`
does **not** override it, so the local backend this plan adds is inert here. The
branch adds a route to the defect; it does not create it.

## Hypotheses tried

1. **Hypothesis:** the hazard is latent, only reachable once a prefixing embedder is
   wired through the new adapter. **Test:** trace the live configuration rather than
   the new code path. **Verdict:** rejected — the `EmbedderHttp` url branch already
   does it, on the configuration this repo actually runs.

## Fix

Implemented in two commits on `experiments`. `master` is a strict ancestor, so
these are already the master-side SHAs — no second SHA to record.

### `66678f34` — the seam

- `EmbedderHttp::dense_document` — the same single-item dense call as
  `dense_query`, with no prefix. `dense_query` now delegates to it after
  prefixing, so **the prefix asymmetry is expressed in exactly one place**.
- `CodeEmbedder::embed_document_one` and `DenseEmbedder::embed_document`, neither
  with a default implementation. Inheriting the query method *is* the bug, and a
  default would let a future backend reintroduce it with no failing test — the
  same reasoning `known_dim`'s doc comment already gives for having no default.
  The compiler then enumerated all five implementors.
- All four writers routed to the document side. Recall stays on the query side.

This answers the § Resume seam question as: **`DenseEmbedder` gains a
document-side method**, not a flag on the adapter. It also adds **no new prefix
policy**, which is what ADR `docs/adrs/2026-07-25-embedding-transport-boundary.md`
contract 3 warns against — it adds the missing query-vs-document *side*
distinction that both existing policies already assume.

### `428a7e77` — the repair

`reembed_memories_in_place` + `codescout migrate-memories --in-place`. Re-derives
every vector from the content the store already holds, via
`SemanticMemoryStore::list` (backend-agnostic) and the new document seam.
`conflicts_with = "db_path"` since it imports nothing. Needed because fixing the
write path only fixes *future* writes, and nothing in the tree could repair
already-stored vectors: `reembed` existed for artifacts and code chunks, and
nowhere for memories.

### The workaround this file recommended was a trap

§ Workarounds advised unsetting `CODESCOUT_QUERY_PREFIX`, correctly noting the
benchmark prefers no prefix. But on a host that had already written memories
*with* the prefix, unsetting it flips queries into document-space while stored
vectors stay in query-space — converting the harmless symmetric shift into a
real mismatch, with no tool able to repair it. That is now safe, because
`--in-place` exists.
## Tests added

**Seam — `src/retrieval/embedder.rs` (3):**

- `dense_document_omits_the_query_prefix_that_dense_query_applies` — asserted at
  the wire via two disjoint mockito mocks, and exercises the **trait routing** as
  well as the inherent method. Covering only the inherent method would let an
  impl wired to `dense_query` pass, which is exactly where the defect lived.
- `without_a_prefix_query_and_document_send_identical_requests` — the prefix is
  the only asymmetry; guards a future `dense_document` diverging in some other way.
- `document_embedding_uses_embed_not_the_query_path` — the mirror of the
  pre-existing `query_embedding_uses_embed_query_not_the_document_path`. Either
  alone would pass while the other direction was wrong, which is how memory
  writes came to run through the query seam in the first place.

**Repair — `src/migrate/memories.rs` (5):** `reembed_in_place_replaces_the_vector_and_preserves_everything_else`,
`…leaves_the_existing_vector_when_embedding_fails`,
`…dry_run_neither_embeds_nor_writes`, `…ignores_other_projects`,
`…on_an_empty_store_is_a_no_op`. The first asserts through `search` rather than
reaching into the store: a query equal to the fresh vector scores >0.99, one
equal to the orthogonal seed <0.01.

**Mutation-verified, both directions.** Wiring `embed_document_one` to
`dense_query` fails the wire test with mockito reporting the *query* mock hit
twice instead of once; routing the adapter to `embed_query` fails the mirror with
`left: [9.0, 9.0]`.

### Gate

`cargo test --workspace` → **3789 passed / 0 failed / 50 ignored**;
`cargo clippy --workspace --all-targets -- -D warnings` clean.
## Workarounds

N/A — fixed. **Do not use the workaround this file previously recommended**
(unsetting `CODESCOUT_QUERY_PREFIX`) on a pre-fix binary that has already written
memories: it strands every stored vector in query-space while queries move to
document-space, and no pre-fix build can repair it. On a current build, run
`codescout migrate-memories --in-place` after any change of prefix policy, model,
or dimension.
## Severity note

Upheld on re-verification: not a hard break, because recall embedded through the
same path and both sides shifted together. The costs named here — recall below
what the model can do, and a mixed space across a toggle of the env var — were
both real, and the second one is what made the file's own workaround unsafe.

Raised from the original filing in one respect: `HttpMigrationEmbedder` meant the
*repair mechanism itself* was on the wrong side of the seam, so the defect was
self-perpetuating rather than merely dormant.
## Resume

N/A — fixed, gated, and the repair has been run on this host: `15/15 upserted, 0
skipped, 37 anchors preserved`.

One verification note for whoever reads this next, because it wasted a step here:
`cargo run --bin codescout` builds with **default features**, which exclude
`server-stack`, so `migrate-memories --in-place` resolves the sqlite-vec lite
store and reports `read: 0` — which looks exactly like "this project has no
semantic memories". `memory(action="recall")` refutes it in one call. Use
`cargo rb` (`--features server-stack,local-embed`) and run
`./target/release/codescout` for anything that touches the live memory store.

An identical recall query before and after the repair, for the record — same ids,
same count, vectors demonstrably different, and the intuitively-correct memory
promoted:

| memory | before | after |
|---|---|---|
| `architecture` | 0.22 (3rd) | **0.24 (1st)** |
| `research/sakana-fugu-integration` | 0.23 (1st) | 0.24 (2nd) |
| `research/loadbearing-mcp-guidance` | 0.22 (2nd) | 0.23 (3rd) |

One query, not a benchmark — cited as evidence the vectors changed, not as a
measured recall improvement.
## References

- ADR: `docs/adrs/2026-07-25-embedding-transport-boundary.md` (contract 3)
- Benchmark: `docs/manual/src/concepts/retrieval-stack.md` § Dense embedder
- Found during: Task 7 review, `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md`
