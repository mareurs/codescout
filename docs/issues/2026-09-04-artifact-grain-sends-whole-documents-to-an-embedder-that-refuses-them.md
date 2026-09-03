---
kind: bug
status: open
tags:
- librarian
- embedding
- retrieval
- chunk-grain
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-04
owner: marius
related:
- docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md
- docs/trackers/retrieval-benchmark.md
severity: high
---

# BUG: artifact-grain embedding sends whole documents to an embedder that REFUSES oversized input, leaving 32% of this corpus permanently vectorless

## Summary

`ChunkGrain::Artifact` builds one chunk spanning an artifact's entire body and
hands it to `EmbeddingService::embed_artifact`. Nothing in the librarian embed
path truncates. The llama-server embedder answers oversized input with **HTTP
500**, not a truncated vector — so on this corpus **473 of 1,475 artifacts (32%)
fail to embed**, land in `embed_errors`, and are left with a chunk row and no
vector: the absorbing state
`docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md`
describes, which no ordinary reindex escapes.

Reachable today only via `[librarian] chunk_grain = false`, since the default
was flipped to chunk grain on 2026-09-04. **Filed as `high` anyway because the
failure is silent per-artifact and permanent**, and because for one day
(`4f172f70` → the flip) this was the shipped default.

## Symptom (Effect)

```
{"error":{"code":500,"message":"input is too large to process. increase the physical batch size","type":"server_error"}}
```

Surfaced in a reindex as one line of `embed_errors` per artifact and a bumped
`embed_error_count`; the run otherwise reports success. Seen twice in passing
during 2026-09-03 reindexes (counts of 2 and 1) and dismissed both times as
incidental, because at chunk grain only a handful of chunks exceed the limit.

## Reproduction

Measured 2026-09-04 while building a per-file collection to compare grains:

```
for each of 1,475 codescout artifacts:
    body = strip_frontmatter(read(abs_path))
    doc  = f"{title}\n\n{body}"
    POST http://127.0.0.1:48081/v1/embeddings  {"input": doc, ...}

  embedded            : 1475
  OVERSIZE (rejected) : 473      <- 32%
  recovered by cutting: 473      (doc[:2048] succeeds every time)
```

The cut-to-2048 fallback is in the probe, **not** in codescout. Production has no
such fallback.

## Environment

Linux 7.1.9-zen1-2-zen, `experiments` @ post-`81f7f923`. Embedder
`CodeRankEmbed-Q4_K_M.gguf` via llama-server at `127.0.0.1:48081`.

## Root cause

Three facts compose:

1. **Artifact grain emits the whole body as one chunk.**
   `build_single_chunk` (`src/librarian/catalog/chunk.rs`) sets
   `content: body.to_string()` — deliberately, so `matched` reports the
   document's real span.
2. **Nothing truncates before the embedder.** `embed_queue_items`
   (`src/librarian/indexer.rs`) filters empty chunks and prepends an entry token;
   `EmbeddingService::embed_artifact` (`src/librarian/embedding.rs:14-17`)
   formats `"{title}\n\n{body}"` and sends it. A grep of `src/librarian/**` for
   `truncate` / `max_chars` / `MAX_INPUT` returns only slug truncation and
   unrelated `.take()` calls.
3. **The embedder rejects rather than truncates.** A 512-token model *could*
   silently clip; this server returns HTTP 500. So the caller must do the
   clipping and does not.

The result is per-artifact, silent, and permanent: `index_repo_sync` stamps
`file_sha256` when it walks the file, so the next reindex computes
`content_unchanged == true` and never retries. `reembed=true` is the only escape,
and it hits the same rejection.

**Why chunk grain does not have this problem *by this mechanism*:** chunks are built to a
2,048-char budget, so all but a handful sit under the limit by construction.

> ⚠ **Corrected 2026-09-04 — the sentence above was right about the mechanism and wrong about
> the conclusion, and it was the sentence scoping this record to a non-default grain.** The
> budget is enforced by *splitting*, and splitting needs a split point: a single unbroken line
> cannot be split at all. Measured over the whole corpus the same day — **68 of 35,810 chunks
> exceed the budget, and 68 of those 68 are single-line.** Not a long tail of near-misses; a
> distinct population the budget cannot act on. Three of them exceed the embedder and produce
> this record's identical HTTP 500 **under chunk grain, the default since `63fae4ea`**.
>
> So the failure is not confined to `chunk_grain = false`. Full derivation, and the reason the
> affected files keep growing (a pre-commit gate mandates appending to one line):
> `docs/issues/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md`.
>
> The Fix section below is unaffected and is now the shared one: clipping at the embed
> boundary closes both records, because both fail for the same reason — **the embedder rejects
> rather than truncates, so the caller must bound what it sends.**

## Evidence

### The cost is not evenly spread, which is why the count is large

473/1,475 is 32% of artifacts, and they are the *long* ones — trackers, ledgers,
archived bug files with full evidence sections. That is precisely the population
whose retrieval matters most, so the failure is anti-correlated with value.

### It was visible three times before it was seen

`embed_error_count` was 2 during the 2026-09-03 full re-embed and 1 in two later
reindexes. Each was noted and set aside as incidental. The count is the same
instrument that would have shown 473 at artifact grain — it was correct and
nobody read it as a rate.

## Hypotheses tried

1. **Hypothesis:** the embedder truncates oversized input, so whole-body
   embedding is merely lossy rather than failing.
   **Test:** POST whole documents; count non-200 responses.
   **Verdict:** **rejected** — 473 hard failures, each recovered by cutting to
   2,048 chars.

2. **Hypothesis:** the librarian truncates before sending.
   **Test:** grep `src/librarian/**` for truncation.
   **Verdict:** **rejected** — no truncation exists on that path.

## Fix

*Plan only.*

The narrow fix is to clip the text `embed_artifact` sends to the model's input
budget. Two cautions:

- **Clip at the embed boundary, not in `build_single_chunk`.** The chunk ROW
  should keep the whole body — that is what makes `matched` report a true span,
  and it is the one thing artifact grain currently gets right. Only the text
  handed to the embedder should be cut.
- **A silent clip is its own defect.** Storing a vector that represents 2,048 of
  40,000 characters, with no record that it happened, is how artifact grain came
  to be described as merely "degraded" rather than "represents 5% of the
  document". Whatever clips should count, and the count should reach the reindex
  report next to `embed_error_count`.

The alternative — refuse artifact grain for oversized artifacts and fall back to
chunking them — makes the mode self-repairing but means `chunk_grain = false` no
longer means what it says. Not obviously wrong; not decided here.

SHA: *(not fixed)*
patch-id: *(not fixed)*

## Tests added

None yet. A discriminating test needs an embedder stub that REJECTS input over N
bytes, because a permissive stub makes the bug unrepresentable — the same shape
as the bug itself. Assert on the stored outcome (a vector exists for a long
artifact), not on the call.

## Workarounds

Do not set `[librarian] chunk_grain = false`. The default is chunk grain as of
2026-09-04, which never produces an oversized input.

## Resume

`src/librarian/catalog/chunk.rs` (`build_single_chunk`),
`src/librarian/embedding.rs:14-17` (the unclipped send), and
`docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` for why
the resulting state is absorbing rather than merely missing.

## References

- `docs/trackers/retrieval-benchmark.md` — the 2026-09-04 grain comparison that
  produced the 473 figure as a side effect of building the comparison collection.
- `docs/issues/2026-09-03-artifact-documents-are-embedded-through-the-query-method.md`
  — a second defect on the same three lines of `embed_artifact`.

### Cluster adjudication

Tagged `cluster/gate-keyed-on-unobservable-event` (`IC-2`). The reindex reports
`embed_error_count`, which is the true signal, but every consumer reads the run's
*completion* instead — `vectorless` is reported separately and `embed_note` says
"DEGRADED" in prose that no gate parses. The decision "did this reindex give the
corpus a searchable representation?" is keyed on an event nothing observes, so
success is inferred from the call returning.

Considered `IC-15` (`accepted-parameter-silently-dropped`) and rejected: nothing
is dropped, the input is refused loudly by the embedder and the refusal is
recorded. What fails is that no one reads it.
