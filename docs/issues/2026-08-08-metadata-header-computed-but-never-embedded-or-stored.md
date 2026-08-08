---
kind: bug
status: open
title: 'BUG: the AST metadata header is computed and tested for every chunk but reaches neither the embedder nor the payload — ast_kind/ast_header are empty on all 579,311 chunks in the live collection'
tags:
- retrieval
- embedding
- chunker
- dead-feature
- silent-quality-loss
closed: null
opened: 2026-08-08
owner: marius
related:
- docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md
severity: high
---

# BUG: the chunk metadata header is computed for every chunk and consumed by nothing

## Summary

`build_metadata_header` produces a per-chunk identity string — `src/foo.rs :: impl Bar
:: fn baz(&self) -> Result<()>` — for every chunk the AST chunker emits, and nine tests
assert on its exact shape. It is read by exactly one caller: an internal gap-chunk check
inside `coalesce_small_chunks`. It is **not** prepended to the text sent to the embedder,
and it is **not** written to the vector payload.

Measured live: `ast_kind` is the empty string on **579,311 of 579,311** points in the
`code_chunks` collection — every project, every language, every index generation.

## Symptom (Effect)

No error, no warning. The symptom is absence: chunks are embedded from their raw source
text alone, so a chunk whose body never names its own file, container, or signature has
no vector representation of that identity.

```
$ curl -s -X POST http://127.0.0.1:6333/collections/code_chunks/points/count \
    -H 'Content-Type: application/json' \
    -d '{"exact":true,"filter":{"must":[{"key":"ast_kind","match":{"value":""}}]}}'
{"result":{"count":579311},"status":"ok","time":2.219263975}
```

Total points in the collection: 579,311. The two counts are equal.

Stored payloads begin with raw source, with no header line:

```
src/tools/edit_file/tests.rs L704  |first 120 chars| \n// ── ReadFile: source-range hint gate ──…\n\n#[tokio::test]
src/librarian/tools/doctor.rs L970  |first 120 chars|     /// `abs_path` shape the catalog stores.\n    fn dead_root(tag: &str) -> String {…
```

## Reproduction

```
1. curl the count above against any codescout-indexed Qdrant collection.
2. Read src/retrieval/sync.rs:201-202 — ast_kind and ast_header are String::new().
3. Read src/retrieval/sync.rs:96 (flush_pending) — the embed text is p.content only.
```

Commit: `15fa5692` (experiments).

## Environment

codescout `experiments` @ `15fa5692`. Qdrant at `127.0.0.1:6333` (REST) /
`CODESCOUT_QDRANT_URL=http://127.0.0.1:6334`, collection `code_chunks`, 21 project_ids,
579,311 points, dense 768 + sparse.

## Root cause

Two independent drops on the same value.

**1. The embedder never sees it.** `flush_pending` at `src/retrieval/sync.rs:83-100`
builds the embedding input from content alone:

```rust
let texts: Vec<String> = pending.iter().map(|p| p.content.clone()).collect();
let embeds = embedder.embed_batch_dyn(&texts).await?;
```

**2. The payload never receives it.** `stream_index` at `src/retrieval/sync.rs:201-202`
constructs every `CodePayload` with the AST fields hardcoded empty, discarding
`c.metadata` — which `split_file` has just populated:

```rust
pending.push(CodePayload {
    ...
    ast_kind: String::new(),
    ast_header: String::new(),
    content: c.content,
    ...
});
```

`CodePayload` declares both fields (`src/retrieval/payload.rs:16`), maps them in and out
(`:32`, `:69`), and they are never anything but `""`. The slots were designed; nothing
was ever put in them.

*measured 2026-08-08: the count query above, run against the live collection; two sampled
payload bodies read back and confirmed to start with raw source. Mechanism then read at
`src/retrieval/sync.rs:83-100` and `:201-202`, and the consumer census done with
`grep '\.metadata' src/**/*.rs` — 14 hits in `src/embed/ast_chunker.rs`, of which one is a
non-test read (`:605`), and 4 unrelated `std::fs` `entry.metadata()` hits elsewhere.*

## Evidence

### The header is computed and heavily tested

`src/embed/ast_chunker.rs`: `build_metadata_header` at `:503`; written at `:683`, `:743`,
`:813`, `:831`. Nine tests assert its shape — `metadata_header_top_level_rust_fn`,
`metadata_header_rust_method_in_impl`, `metadata_header_struct_no_signature`,
`metadata_header_gap_file_only`, `metadata_header_container_only`,
`metadata_header_kind_without_signature_uses_name`,
`metadata_header_name_only_no_kind_no_sig`, `metadata_header_nested_container`,
`split_file_rust_populates_metadata_headers`.

### It has exactly one live consumer, and it is internal

```
src/embed/ast_chunker.rs:605: let is_gap = chunk.metadata.as_deref() == Some(gap_metadata);
```

That is inside `coalesce_small_chunks` — the header is being used as a sentinel to detect
gap chunks during coalescing. Every other read is in a test in the same file.

### The deleted legacy path DID embed it, and had a test pinning the contract

`66db4c70` ("delete legacy embed::index and bm25/drift/sqlite-vec/tantivy deps") removed
`src/embed/index.rs`, which contained:

```
:490   // Migrate: add metadata column (searchable header prepended before embedding)
:1641  flat_texts.push(match &chunk.metadata { … })
:2762  // Build embed texts: metadata header + content (mirrors embed_producer)
:4842  fn embed_text_format_includes_metadata_prefix() {
:4844  // "{metadata}\n{content}" — not just content.
```

So the behaviour existed, was deliberate, and was regression-tested. The test died with
the module it covered, which is why nothing failed when the surviving path turned out not
to implement it.

**Not yet measured:** whether the Qdrant path ever prepended the header, i.e. whether this
is a regression introduced when the legacy path was retired, or a feature the Qdrant path
never had while the legacy path carried it. `CodePayload` declaring the two fields
suggests the latter — the slots were designed in and never filled. Someone should read
the Qdrant path's history before this file calls it a regression.

## Hypotheses tried

1. **Hypothesis:** codescout's index is stale and predates the AST chunker, which is why
   `ast_kind` is empty.
   **Test:** `index(action="status")` → `last_indexed_commit: 1a1f2c82`, well after the
   chunker work; then counted `ast_kind == ""` across the whole collection, not just
   codescout.
   **Verdict:** rejected. 579,311 of 579,311 across 21 projects — not a stale-index
   artifact, a write-path constant.

2. **Hypothesis:** the header is smuggled into `content`, so the payload fields are merely
   redundant.
   **Test:** read two stored payload bodies (§ Symptom) and `flush_pending`.
   **Verdict:** rejected. Stored content starts at raw source, and the embed text is
   `p.content` with no concatenation. Note the stored-content check alone would NOT have
   settled this — the legacy path stored raw content while embedding metadata+content, so
   only reading `flush_pending` is decisive.

## Fix

Not yet implemented. The change is small; the validation is not.

1. **Carry the value.** Add `metadata` to what `stream_index` reads off the chunk and
   write it into `ast_header` (and derive `ast_kind` from the chunk's node kind, or drop
   `ast_kind` if nothing will ever fill it — a permanently-empty payload field is worse
   than no field).
2. **Prepend for embedding.** In `flush_pending`, build the text as
   `{ast_header}\n{content}` when the header is non-empty, matching the contract the
   deleted `embed_text_format_includes_metadata_prefix` test pinned.
3. **Re-embed.** This changes embedding inputs for every chunk, so it invalidates every
   vector — an `index --force` per project. Measured cost on codescout 2026-08-07: 8.06
   min, not the "~2h" that has been quoted around this area.

Validate against `docs/research/2026-05-06-retrieval-stack-benchmark.md` before landing.
Prepending a header to every chunk is not obviously free — it adds tokens to short chunks
and could dilute the body signal. That is precisely why it needs measurement rather than
assumption, and the same benchmark gate the chunk-size floor was supposed to pass.

## Tests added

None yet. Two are needed, and the second is the one that would have caught this:

- Unit: `flush_pending` sends `{header}\n{content}` when the header is present, plain
  content when it is not. This is the deleted test, restored against the surviving path.
- Integration: after an index pass over a fixture, assert the stored payload's
  `ast_header` is non-empty for at least one known symbol chunk. A unit test on the
  chunker cannot catch a value being dropped two modules downstream — the entire failure
  here is that the producer's tests all passed.

## Workarounds

None available to a user. Retrieval quality is affected but nothing is broken enough to
route around; queries that name a symbol still match its body text.

## Resume

Decide first whether `ast_kind` should exist at all. `ast_header` has a clear producer
(`build_metadata_header`) and a clear use; `ast_kind` has neither, and filling it means
inventing a value. Read `CodePayload` at `src/retrieval/payload.rs:10-40` and check
whether anything reads `ast_kind` back out (`map_to_payload` at `:69` deserializes it —
find out if any caller uses the result). If nothing does, delete the field rather than
populate it.

Then check the Qdrant path's history for whether the header was ever embedded there, so
this file can state regression-or-never-implemented as fact rather than as the open
question in § Root cause.

## References

- `src/retrieval/sync.rs:83-100` — `flush_pending`, embeds content only
- `src/retrieval/sync.rs:201-202` — `stream_index`, hardcodes both AST fields empty
- `src/retrieval/payload.rs:16,32,69` — the two fields, declared and round-tripped
- `src/embed/ast_chunker.rs:503` — `build_metadata_header`, the orphaned producer
- `src/embed/ast_chunker.rs:605` — its only live consumer, an internal gap-chunk check
- `66db4c70` — deleted `src/embed/index.rs`, the path that embedded the header, and its test
- `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` — the bug whose
  "tiny chunks carry almost no retrievable signal" argument this defect compounds
