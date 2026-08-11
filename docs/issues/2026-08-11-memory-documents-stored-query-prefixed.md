---
id: '01529662e7fe6aa5'
kind: bug
status: open
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

Not implemented. The shape of a fix, in rough order of cost:

- Give the write path a document-side method. `DenseEmbedder` has one method,
  `embed`; the memory store needs `embed_document` semantics for writes and
  `embed_query` for recall. That is a trait change with two implementors.
- Or have `CodeDenseAdapter` carry a flag for which side it serves, constructed
  differently at the write and recall sites. Cheaper, uglier, and the kind of
  boolean parameter that gets passed wrong.

Note this interacts with ADR `docs/adrs/2026-07-25-embedding-transport-boundary.md`
contract 3, which already documents three ways root and crate disagree about query
prefixes. A fix here should be designed with that contract rather than beside it.

## Tests added

None. A regression test is straightforward once a fix exists: embed the same text
through the write and recall paths with a non-empty prefix configured, and assert the
stored vector differs from the recall vector — today they are identical, which is the
bug.

## Workarounds

Unset `CODESCOUT_QUERY_PREFIX`. The retrieval benchmark
(`docs/manual/src/concepts/retrieval-stack.md` § Dense embedder) already rates
Q4 **no-prefix** the champion at 37 against 34 with a forced prefix, so on the
project's default model the workaround is also the better configuration.

## Severity note

Not a hard break, which is why it has survived. Recall embeds through the *same*
path, so both sides get the prefix — a symmetric shift into query-space rather than
a mismatch. The costs are (a) recall quality below what the model can do, and (b) a
mixed embedding space in any collection written across a toggle of the env var,
where some vectors carry the prefix and others do not.

## Resume

Decide the seam question first, not the patch: does `DenseEmbedder` gain a
document-side method, or does the memory store stop sharing one adapter between
writes and reads? Read `docs/adrs/2026-07-25-embedding-transport-boundary.md`
contract 3 before choosing — it already owns the query-prefix contract, and a second
mechanism beside it would be the third place this project decides prefix policy.

## References

- ADR: `docs/adrs/2026-07-25-embedding-transport-boundary.md` (contract 3)
- Benchmark: `docs/manual/src/concepts/retrieval-stack.md` § Dense embedder
- Found during: Task 7 review, `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md`

