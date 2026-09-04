---
id: '0aaeae47b7869a43'
kind: bug
status: open
title: 'BUG: the librarian embeds stored artifacts through the query seam, and its own constructor path reaches the QueryPrefix default a project ruling forbids'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- librarian
- embedding
- retrieval-quality
topic: librarian artifact embedding seam
opened: 2026-09-04
related:
- docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md
- docs/issues/2026-09-04-artifact-grain-sends-whole-documents-to-an-embedder-that-refuses-them.md
severity: high
unverified: The prefix is established by reading every link in the chain, not by observing a live request. The RED test named in the Verification-owed section is still owed, and is what would turn this from a sound derivation into an observation.
---

## Summary

`EmbeddingService::embed_artifact` embeds **stored artifact bodies** by calling
`Embedder::embed_query` — the query seam. On this deployment that seam prepends
CodeRankEmbed's query instruction, so every librarian artifact vector is stored in
query-space. This is the defect
`docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md` closed for
memories, reproduced in the librarian's artifact path.

It takes **two** independent wrongs, which is why neither alone was caught:

1. **The wrong seam.** `embed_query` is for queries; documents belong on `embed`.
2. **The wrong default.** The librarian constructs its embedder through the *crate's*
   `create_embedder_with_config`, which takes no `QueryPrefix` and therefore uses the
   crate's `#[default] = Derive` — bypassing the project's unset→`Suppressed` mapping,
   which lives only in `EmbedderHttp::remote_dense`.

Either one alone is harmless. With the right seam the prefix never applies; with
`Suppressed` the wrong seam never prefixes. Both together produce the defect.

## Reproduction — verified link by link at the bytes, not inferred

Laptop (`archlinux`), `experiments` @ `93560a37`, 2026-09-04. `CODESCOUT_QUERY_PREFIX`
**unset**.

| # | link | evidence |
|---|---|---|
| 1 | config selects an asymmetric model over HTTP | `.codescout/project.toml:18-19` — `model = "CodeRankEmbed"`, `url = "http://127.0.0.1:48081/v1"` |
| 2 | the librarian builds its embedder via the **crate** constructor | `src/librarian/mod.rs:106` and `:395` — `codescout_embed::create_embedder_with_config(model, url, api_key)`, no `QueryPrefix` argument |
| 3 | so `QueryPrefix` is the crate default | `crates/codescout-embed/src/remote.rs:126` — `#[default] Derive` |
| 4 | which derives a prefix for this model | `remote.rs:155-161` — `derive_for` matches `"coderank"` → `Some("Represent this query for searching relevant code: ")` |
| 5 | and the artifact path asks for the query seam | `src/librarian/embedding.rs:17` — `self.embedder.embed_query(&text)` |
| 6 | which prepends the prefix | `remote.rs:596-609` — `QueryPrefix::resolve` → `format!("{prefix}{text}")` |

The vector from step 6 is what gets stored as the artifact's *document* embedding.

## Mechanism

The controlling ruling is stated on `QueryPrefix` itself (`remote.rs:110-120`):

> Decided 2026-08-30 (`resume-embedding-transport-stages-1-3:ET-9` D1), upholding
> `docs/adrs/2026-07-25-embedding-transport-boundary.md` § *The three contracts*: an
> unset `CODESCOUT_QUERY_PREFIX` maps to `QueryPrefix::Suppressed`, **never** to
> `QueryPrefix::Derive`. Deriving there would silently cost ~3 benchmark points on the
> default deployment.

That mapping is implemented in exactly one place — `src/retrieval/embedder.rs:456-459`.
The librarian never goes through it. So the ruling holds for the retrieval stack and is
silently absent for the artifact stack, which is the `guard-narrower-than-its-name`
shape: the sentence everyone reasons with says *"the default deployment suppresses"*, and
its actual coverage is one builder.

The seam half carries its own warning, written on the function being misused
(`remote.rs:596-601`):

> The document side (`Embedder::embed`) never prefixes — that asymmetry is the entire
> point of an asymmetric model, and getting it backwards **strands stored vectors in
> query-space**.

## Why this is invisible to the party best placed to catch it

**Both sides are prefixed, so nothing looks broken.** `doc(action="find", semantic=…)`
embeds its query through the same seam (`src/librarian/tools/find.rs:639`), so queries
and documents land in the *same* space. Retrieval returns plausible, ranked, non-empty
results. The loss is quality — an asymmetric model used symmetrically — and it has no
error, no empty result, and no observable signature at the call site.

- A reviewer of `embedding.rs` sees a four-line method that reads correctly in isolation.
- A reviewer of `remote.rs` sees the mapping implemented and tested.
- The gap exists only in the *composition*, and no file contains both halves.
- `dense_document_omits_the_query_prefix_that_dense_query_applies`
  (`src/retrieval/embedder.rs:1194`) already guards this property — for the retrieval
  path. The librarian has no equivalent, so the property is asserted where it holds and
  unasserted where it does not.

## Consequence — and a sequencing constraint

Fixing the seam **invalidates every artifact vector currently stored**, because they are
all prefixed and new ones would not be. Applying the code fix without a full re-embed
leaves the store mixed: only changed artifacts get unprefixed vectors, so the collection
drifts into two incompatible spaces, which is worse than either consistent state.

So the seam fix and `librarian(action="reindex", reembed=true)` are one operation. Note
that re-embed currently exceeds the 1800 s MCP idle timeout on this corpus, which couples
this to the reindex-progress work.

## Suggested direction (not a plan — reproduce first)

**The segmentation half has shipped** — `8acec9c7`, patch-id
`de0b0990236e69bf18ef2cbff041cbaa3d565652`, closing
`docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md`.
That commit routes `embed_artifact` through `segment_for_budget` +
`mean_pool_normalized` but deliberately keeps `embed_query`, so this record's defect is
untouched and the code now carries a comment saying so.

What remains is one line: call the **document** side instead. The sanctioned route is
`crate::embed::document::embed_document_pooled`, as `HttpMigrationEmbedder::embed` does
(`src/migrate/memories.rs:87`) — it takes `&dyn DenseEmbedder` and calls `embed_document`,
never `embed`, and its doc comment states this is precisely to avoid re-creating the
query-prefix defect. Reaching it needs `CodeDenseAdapter`, the sanctioned bridge, because
`CodeEmbedder` deliberately does not have `DenseEmbedder` as a supertrait.

Fixing the **default** as well as the seam is the belt-and-braces version: have the
librarian construct through a builder that applies the ET-9 D1 mapping rather than calling
`create_embedder_with_config` directly, so a future caller of the wrong seam is harmless.
One of the two is sufficient to stop the prefix; both are wanted, because the class here
is that either alone leaves the other still loaded.
## Verification owed

The chain above is verified by reading every link, which is strong but is not an observed
request. The regression guard should be the observation: a test asserting
`embed_artifact` does **not** prefix, against a recording embedder, mirroring
`dense_document_omits_the_query_prefix_that_dense_query_applies`. It must RED on today's
code — that RED is the live confirmation, and it is owed before the fix is called done.

## Resume

Not started. Take the segmentation half of the sibling bug first; it is independent and
unblocks 7 artifacts. Before touching the seam, settle with the operator whether the
full `reembed` runs in the same session, because the mixed-space state between the two is
the one outcome to avoid.

## References

- `docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md`
  — the sibling, now fixed at `8acec9c7`; that commit is where this defect was found
- `docs/issues/2026-09-04-artifact-grain-sends-whole-documents-to-an-embedder-that-refuses-them.md`
  — the other grain, still open
- `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md` — the same
  defect, closed for memories
- `docs/issues/archive/2026-08-26-migration-embedder-lacks-the-segmentation-the-tool-path-has.md`
  — the same *shape*: an embed path lacking what a sibling path has
- `docs/adrs/2026-07-25-embedding-transport-boundary.md` § *The three contracts*, and
  `resume-embedding-transport-stages-1-3:ET-9` D1 — the ruling this path does not reach
- `crates/codescout-embed/src/remote.rs:1276`
  (`derive_is_still_the_constructor_default_and_still_prefixes_coderank`) — a pre-existing
  test that independently corroborates links 3 and 4 of the chain above
