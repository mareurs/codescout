---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-04
fix_branch: experiments
fix_patch_id: de0b0990236e69bf18ef2cbff041cbaa3d565652
fix_sha: 8acec9c76bf519d01cf04c6faf683804be3c5f7f
opened: 2026-09-04
owner: marius
related: []
severity: high
unverified: 'The 7 artifacts are still vectorless on disk: the code no longer produces the failure, but repairing the existing rows needs `cargo rb` plus a reindex, which has not been run. And the second half this file asks for -- a wrapping form for the `**Members:**` line -- is NOT done; it is now a vector-quality concern (a 26 KB line pools to one blurry vector) rather than the data-loss one it was.'
---

# BUG: the chunker's 2,048-char budget is not a bound — an unbreakable single line exceeds it by 12x, and the cluster ledger is falling out of semantic search line by line

## Summary

Chunk grain is described as safe from the oversized-input failure because "chunks are built
to a 2,048-char budget". The budget is a **target, not a bound**: the chunker splits on
structure, so a single unbroken line cannot be split at all. **68 of 35,810 chunks exceed the
budget, and 68 of those 68 are single-line — 100%.** Three exceed the embedder outright and
fail with HTTP 500, landing in the absorbing unembedded state.

All three are `**Members:**` lines in `docs/trackers/issue-clusters/`, the ledger of this
project's own defect classes — and the `ledger-counts` pre-commit gate **requires** appending
to those lines. The convention drives monotonic growth of a single line with no ceiling
anywhere in the loop.

## Symptom (Effect)

`librarian(action="reindex", reembed=true)` on codescout, 2026-09-04:

```
"embedded": 28077,
"vectorless": 10,
"embed_error_count": 3,
"embed_errors": [
  "bdf9cecf-…: embed failed: HTTP 500 from embedding server:
   {\"error\":{\"code\":500,\"message\":\"input is too large to process. increase the physical batch size\"}}",
  … ×3
]
```

The three chunks, and their owning files:

| bytes | newlines | artifact |
|---:|---:|---|
| 24,382 | **0** | `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` |
| 16,451 | **0** | `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md` |
| 15,390 | **0** | `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md` |

24,382 chars is also the longest line in the entire repository.

## Reproduction

```
git rev-parse HEAD              # ae88224f at time of filing
librarian(action="reindex", reembed=true, scope="project")
```

Read `embed_errors`. Then:

```sql
SELECT LENGTH(content), LENGTH(content)-LENGTH(REPLACE(content,char(10),'')) AS newlines
FROM artifact_chunk ORDER BY 1 DESC LIMIT 3;
```

## Environment

codescout `experiments` @ `ae88224f`, chunk grain (the default since `63fae4ea`),
llama-server embedder at `127.0.0.1:48081`, CodeRankEmbed.

## Root cause

**The budget is enforced by splitting, and splitting needs a split point.** The chunker
targets 2,048 chars by breaking on document structure; a line contains no structure to break
on, so a 24 KB line yields a 24 KB chunk. Nothing downstream clips — established by the
sibling record below, whose grep for `truncate` / `max_chars` / `MAX_INPUT` across
`src/librarian/**` returns only slug truncation. The embedder then **rejects rather than
truncates** (HTTP 500), and `index_repo_sync` has already stamped `file_sha256`, so the
artifact is in the absorbing state that no ordinary reindex escapes
(`docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`).

*measured 2026-09-04, whole-corpus SQL rather than a sample:*

| population | n |
|---|---:|
| chunks total | 35,810 |
| over the 2,048 budget | 68 |
| over 4,096 | 20 |
| **over budget AND single-line** | **68 of 68** |

**Every single over-budget chunk in the corpus is an unbreakable line.** That is the finding:
the budget holds everywhere it *can* hold, and the exceptions are not a long tail of
near-misses but a distinct population the mechanism cannot touch at all.

### The gate is the growth driver

`scripts/pre-commit-ledger-counts.py` refuses a commit where a class gained a member unless
that class's `**Members:**` line changes to name it. The line is append-only by design and
carries a derivation per member, so it grows by a paragraph per bug filed, forever, on one
line. The convention is good — the derivations are the valuable half, and the count it
replaced could be satisfied by accident. What is missing is any ceiling.

**So the corpus's own defect taxonomy becomes unfindable in proportion to how much it is
used**, and the most-worked classes go first: `IC-18`, `IC-2` and `IC-11` are three of the
largest. This record's own class would be tagged into a line that is already 16 KB.

## Evidence

### The failing chunk has zero newlines

```
16451|0|**Members:** `filter={"tags": {"contains": "cluster/gate-keyed-on-unobservable-event"}}` *
```

`LENGTH(content) - LENGTH(REPLACE(content, char(10), '')) == 0`. Not "few newlines" — none.

### Six of 22 cluster files are already over 8 KB on one line

```
24382  IC-18-selector-narrower-than-its-population.md
16451  IC-2-gate-keyed-on-unobservable-event.md
15390  IC-11-doc-contradicted-by-code.md
11219  IC-6-addressing-without-an-escape-hatch.md
10468  IC-17-shared-resource-carries-no-owner.md
 9827  IC-14-guard-narrower-than-its-name.md
```

Three are over the embedder's limit today. The other three are on the same trajectory, and
nothing reports the distance to it.

### It corrects a claim in the sibling record

`docs/issues/2026-09-04-artifact-grain-sends-whole-documents-to-an-embedder-that-refuses-them.md`
states: *"**Why chunk grain does not have this problem:** chunks are built to a 2,048-char
budget, so all but a handful sit under the limit by construction."*

That is right about the mechanism and wrong about the conclusion, and it matters because it
is the sentence scoping that bug to a non-default grain. Chunk grain — **the default since
`63fae4ea`** — reaches the identical HTTP 500 by a different route. Corrected in the same
commit rather than left for a reader to hit.

## Hypotheses tried

1. **Hypothesis:** the 3 failures are the artifact-grain bug already filed.
   **Test:** read the failing chunks' newline counts and sizes; check the configured grain.
   **Verdict:** rejected — grain is chunk (default since `63fae4ea`), and these are chunks of
   a multi-chunk artifact, not whole bodies. Same symptom, different mechanism.
2. **Hypothesis:** it is a long tail — the budget mostly holds and a few chunks drift over.
   **Test:** count over-budget chunks and their newline counts across all 35,810.
   **Verdict:** rejected, and this is the useful one. 68 of 68 have zero newlines. There is no
   tail; there are two populations, and the mechanism cannot act on the second at all.
3. **Hypothesis:** raising the embedder's `--ubatch` fixes it.
   **Verdict:** deferred, and it is a ceiling-raise not a fix. The line grows without bound
   by construction, so any fixed limit is reached again. Named because it is the obvious move
   and it buys time rather than correctness.

## Fix

Fixed on `experiments` at `8acec9c7` — patch-id `de0b0990236e69bf18ef2cbff041cbaa3d565652`.

**Segmented, not clipped**, which is better than this section asked for and cost less.
`src/embed/document.rs` already had `segment_for_budget` + `mean_pool_normalized`, built
for this identical failure on the memory-migration path, and `segment_for_budget` already
hard-splits a line longer than the budget — the whole of this defect — with the comment
*"no boundary can help, and dropping it would be the silent loss this exists to stop."*
So `EmbeddingService` gained `budget_chars` from `chunk_size_for_model(&model_spec)` at
both production sites, and `embed_artifact` routes through those two helpers. Clipping
would have discarded the tail of every oversized line; pooling keeps it.

`new` retains today's behaviour via `usize::MAX`, matching `HttpMigrationEmbedder::new`'s
opt-out convention, so the ~20 existing callers — all tests with ceiling-less mocks — are
untouched.

**This record is the third instance of one shape**, which is the part worth carrying
forward: an embed path lacking the segmentation a sibling path already has. The tool path
had it; the migration path lacked it and was fixed 2026-08-26
(`docs/issues/archive/2026-08-26-migration-embedder-lacks-the-segmentation-the-tool-path-has.md`);
the librarian's artifact path lacked it until now. Three paths, one helper, adopted one at
a time by whoever next hit the failure.

### Three corrections to this file's own analysis

1. **The population is 7, not 3 — and this file predicted it.** § *Evidence* listed six
   cluster files over 8 KB and said the other three were *"on the same trajectory, and
   nothing reports the distance to it."* All six now fail, plus `IC-13`. Three of the
   original chunks also grew 1.5–2.2 KB since filing (IC-18 24,382→25,859; IC-2
   16,451→18,432; IC-11 15,390→17,595). **`IC-17` is byte-identical at 10,468 and now
   fails anyway**, so growth alone does not explain 3→7 — see correction 2.
2. **The binding limit is `n_ctx = 2048`, not the 4096 physical batch, and Hypothesis 3
   is wrong about which failures it would address.** This file only ever observed HTTP 500
   `too large to process`, because at filing time only chunks above 4096 existed. Four of
   today's seven fail at **HTTP 400 `exceed_context_size_error`**, with the server
   reporting `"n_ctx":2048` in its own payload. So raising `--ubatch-size` would have
   fixed 3 of 7 and merely changed the other 4 from a 500 into a 400. The server's error
   message names a remedy that is not the fix — and it is the remedy this file recorded,
   on the server's authority.
3. **The mechanism is exact rather than statistical.** All 7 failures are `chunk_ix = 2`,
   `entry_part = 2` of N, with `start_line == end_line`. IC-18's parts 1 and 3–7 are
   205–1889 bytes; part 2 is 25,859 and is one line. So it is not "chunks that happen to
   be large" but *always the part the splitter dumps the unbreakable line into*, which is
   a sharper statement of this file's own 68-of-68 finding.

### Found while reproducing, filed separately, deliberately NOT fixed here

`embed_artifact` reaches the embedder through `embed_query` — the **query** seam — for
stored content, and the librarian's constructor path lands on `QueryPrefix::Derive`, the
state ET-9 D1 rules out. Every artifact vector on a CodeRankEmbed deployment therefore
carries the query prefix.
`docs/issues/2026-09-04-librarian-embeds-stored-artifacts-through-the-query-seam.md`.

It is one line from the fix above and was left alone on purpose: correcting the seam
invalidates every vector already stored, so it and a full `reembed` are a single
operation, and shipping it alone would split the collection across two incompatible
spaces.

### The ledger half is still open

§ *Fix* asked for two things and one shipped. The `**Members:**` wrapping form is not
done, and `scripts/pre-commit-ledger-counts.py`'s single-line requirement is still why.
What changed is its severity: an unbounded line is no longer *data loss* (the artifact
gets a vector now) but *vector quality* — a 26 KB line pools to one blurry vector, so the
class becomes progressively less findable rather than abruptly unfindable. Filing this
very record required appending ~1 KB to `IC-14`'s line, which the `ledger-counts` gate
correctly refused to let me skip.

## Tests added

Five, in `src/librarian/embedding.rs`, against a `CeilingEmbedder` double that **refuses**
oversized input rather than truncating — annotated as load-bearing, because a truncating
double would let all five pass against the unsegmented code.

The fixture is the shape this file said was needed: *"the fixture must contain a single
line longer than the limit."* It asserts on that, too — a newline in the fixture gives the
splitter a boundary and silently stops the test discriminating.

Mutation-tested, 4 rounds, **6 observed REDs**:

| mutation | reds |
|---|---|
| drop the budget check (`<= usize::MAX`) | 2 — segmentation + unit-norm |
| hand-roll the mean instead of `mean_pool_normalized` | exactly 1 — at norm 0.582 against the 0.577 the assertion predicts |
| revert one production site to `new` | exactly 1 — the source-level wiring guard, while the other four stay green |

That last row is the one worth reading. Every other test constructs the service directly,
so all four pass with the real callers reverted — the `declared-not-wired` shape. The guard
is a source-level pin on `src/librarian/mod.rs`, paired with a positive assertion because
the primary one is an absence and therefore monotone under removal: deleting both
construction sites would satisfy it while embedding nothing.

And read those names out of the **default** lane. `LEAN exit=0` is vacuous here — measured
on this run, **0** `librarian::` tests before the lean marker against 1056 after it.
## Tests added

None — not fixed. The regression guard should assert **no chunk exceeds the embedder's input
limit**, over the real corpus, not that the chunker targets a budget. The budget is already
honoured; it is the unbreakable input the assertion has to name, and a fixture built from
well-formed markdown cannot express it — the fixture must contain a single line longer than
the limit.

## Workarounds

Wrap the offending `**Members:**` lines by hand and re-run `reindex(reembed=true)`. This
un-sticks the artifact, and the gate keeps re-growing the line.

## Resume

Decide whether the clip lands at `EmbeddingService::embed_artifact` or in
`embed_queue_items` — the sibling record's Fix section frames the same choice, so settle it
once for both. Then take the ledger half separately: read `members_fields`
(`scripts/pre-commit-ledger-counts.py:367`) before proposing a wrapping form, because the
parser is why the line is one line.

## References

- `docs/issues/2026-09-04-artifact-grain-sends-whole-documents-to-an-embedder-that-refuses-them.md`
  — same HTTP 500, other grain; its chunk-grain exemption is corrected by this record
- `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` — why the failure
  is permanent rather than retried
- `scripts/pre-commit-ledger-counts.py:367` (`members_fields`) — the single-line requirement
- `docs/trackers/issue-clusters.md` § Index — the class list whose files are affected
