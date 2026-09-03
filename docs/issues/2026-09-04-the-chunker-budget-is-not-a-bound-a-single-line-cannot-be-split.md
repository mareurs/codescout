---
kind: bug
status: open
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-04
owner: marius
related: []
severity: medium
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

Not attempted. Two independent halves, and both are wanted:

- **Clip at the embed boundary.** The caller must bound what it sends, since the embedder
  rejects rather than truncates. This is the sibling record's fix too, so one change closes
  both — the reason to state them together.
- **Give the ledger convention a wrapping form.** A `**Members:**` field whose members sit on
  continuation lines would chunk normally. Note the constraint that produced the current
  shape: `scripts/pre-commit-ledger-counts.py`'s `members_fields` keys on *the single line
  beginning* `**Members:**`, and `IC-unclassified` documents this explicitly — *"All
  derivations below — **on continuation lines, which the script does not read.**"* So the
  one-line form is load-bearing for the gate as written, and any wrapping change must move
  the parser with it. That is the whole of why this is not a five-minute fix.

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
