---
kind: bug
status: fixed
title: 'BUG: the AST metadata header is computed and tested for every chunk but reaches neither the embedder nor the payload — ast_kind/ast_header are empty on all 579,311 chunks in the live collection'
tags:
- retrieval
- embedding
- chunker
- dead-feature
- silent-quality-loss
closed: 2026-08-08
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

Implemented in **`2bc0f9f0`** (`experiments`, 2026-08-08). Four changes, all in the
retrieval path. Promotion to `master` is by fast-forward, so this SHA *is* the master
SHA — there is no second one to record later.

1. **`embed_text` is the named home for the decision** (`src/retrieval/payload.rs`).
   It returns `{ast_header}\n{content}` when the header is present and bare content
   when it is not. `flush_pending` calls it instead of reading `p.content` inline, so
   "what does a chunk look like to the embedder" is now one function with one test
   rather than the residue of a struct literal in another module.
2. **`stream_index` carries the value** — `ast_header: c.metadata.unwrap_or_default()`
   in place of `String::new()`.
3. **`ast_kind` is deleted**, not populated. It had no producer anywhere in the tree,
   so filling it meant inventing a value. Removed from the struct, from
   `payload_to_map`, and from `map_to_payload`; points written before today still
   carry the key and it is simply not read.
4. **The header is now checkout-independent.** `stream_index` handed `split_file` the
   *absolute* path (`entry.path()`), so prepending the header as-was would have
   embedded `/home/<user>/...` into every vector and made the same code embed
   differently per checkout location. It now passes the forward-slashed relative path
   — the same string the payload stores. Found during implementation: all 31 of the
   chunker's own `split_file` call sites already pass relative paths; the single
   production caller was the outlier, and the divergence was invisible because the
   header it produced was unconsumed.

**Gate:** `fmt`, `clippy --all-targets -D warnings`, `cargo test` (3580 passed),
`check --no-default-features --all-targets`, `test --no-default-features` (2643),
`test --features local-embed --no-default-features` (2644) — all green.

**Benchmark: run 2026-08-08, null result.** 26/75 → 25/75, five single-point moves in
both directions. Kept by maintainer decision on the strength of the restored contract,
not on retrieval grounds — see § Resume and the benchmark doc. The paragraph below is
preserved as written *before* the measurement, because it is the standard the change
was held to and it was met.

**Written pre-measurement:** the retrieval benchmark has NOT run. Prepending a
header to every chunk is not free — it adds tokens to short chunks and may dilute
body signal. Validate against `docs/research/2026-05-06-retrieval-stack-benchmark.md`
before this is promoted. Nothing here should be read as evidence the change improves
retrieval; it restores a declared contract that the code had silently stopped
honouring.
## Tests added

Both mutation-verified — a test that has never been seen to fail is a test whose
failure mode is unknown.

- `stream_index_embeds_the_ast_header_ahead_of_content`
  (`src/retrieval/sync.rs`, in the existing `mod tests`). Drives a real index pass
  over a temp file and asserts on **what the embedder received**, via a new `seen`
  recorder on `FakeEmbedder`. Also asserts the body survives the prepend, and that no
  absolute path leaks into the embedding input.
  *Mutation:* restoring `ast_header: String::new()` fails it with
  `no embedded text carried an AST header; got ["fn assemble_widget(n: usize) …"]` —
  and **the other seven tests in that module still pass**, which is the measured proof
  that the pre-existing suite was blind to this defect.
- `embed_text_prepends_the_ast_header_and_omits_it_when_absent`
  (`tests/retrieval_unit.rs`). The deleted `embed_text_format_includes_metadata_prefix`
  contract, restored against the surviving path. Covers the empty-header branch too,
  so a markdown chunk cannot start embedding with a leading blank line.
  *Mutation:* making `embed_text` return `p.content.clone()` unconditionally fails it.

Deliberately **not** gated behind `server-stack` — that feature is in neither
`default` nor any CI lane, so a test carrying the gate is never compiled and cannot
fail. Filed separately as
`docs/issues/2026-08-08-server-stack-gated-tests-never-compiled-by-any-lane.md`.

Asserting on the stored payload would not have caught the original defect: the legacy
path stored raw content while embedding header+content, so stored content reads as raw
in both the working and the broken world. Only the embedder's input discriminates.
## Workarounds

None available to a user. Retrieval quality is affected but nothing is broken enough to
route around; queries that name a symbol still match its body text.

## Resume

N/A — closed 2026-08-08.

The benchmark gate that this file held itself open for has run, and it came back
**null**: 26/75 before, 25/75 after, on the same 25-TC suite against the same corpus
with only the stored vectors differing. Five cases moved, two up and three down, every
one by a single point. No measured benefit and no measured harm. Full A/B, including
why the result is neither a doc-blindness artifact nor a coverage failure, is recorded
in `docs/research/2026-05-06-retrieval-stack-benchmark.md` § *AST metadata header A/B
(2026-08-08) — NULL RESULT*.

**Maintainer decision 2026-08-08: keep the header.** It restores a contract the field's
own doc comment declares, costs nothing further (codescout's corpus is rebuilt; other
projects pick it up only on their next `--force`), and makes `ast_header` a real
populated field — which is what made this defect findable in the first place. Reverting
would buy a measured zero for another full re-embed.

**Do not let a later session cite this fix as a retrieval improvement.** It is not one,
and the measurement saying so is one document away. The one open question is
chunk-level discrimination, which the file-level suite is structurally unable to see;
that needs a new instrument, not a re-run.
## References

- `src/retrieval/sync.rs:83-100` — `flush_pending`, embeds content only
- `src/retrieval/sync.rs:201-202` — `stream_index`, hardcodes both AST fields empty
- `src/retrieval/payload.rs:16,32,69` — the two fields, declared and round-tripped
- `src/embed/ast_chunker.rs:503` — `build_metadata_header`, the orphaned producer
- `src/embed/ast_chunker.rs:605` — its only live consumer, an internal gap-chunk check
- `66db4c70` — deleted `src/embed/index.rs`, the path that embedded the header, and its test
- `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` — the bug whose
  "tiny chunks carry almost no retrievable signal" argument this defect compounds

## Fix provenance

- **SHA:** `15fa5692` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `de7015d05c220fa33c9ac400d65f5ca22190e9a4` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep de7015d05c22 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
