---
id: b1d6d162b2cd08da
kind: bug
status: fixed
title: 'BUG: replace_chunks re-embeds every chunk below a mid-body insertion, because its reuse key includes the ordinal'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- embeddings
- retrieval-grain
- performance
closed: 2026-09-06
opened: 2026-09-02
owner: marius
severity: med
---

# BUG: `replace_chunks` re-embeds every chunk below a mid-body insertion, because its reuse key includes the ordinal

## Summary

`replace_chunks` (`src/librarian/catalog/chunk.rs`) preserves a chunk's `chunk_id` —
and therefore its `artifact_vec_v2` embedding — when the chunk is unchanged. Its
doc comment states the purpose: *"stops a re-index re-embedding an untouched 766 KB
tracker."*

The reuse key is `(chunk_ix, content_hash)`. `chunk_ix` is a positional ordinal, so
**any insertion above a chunk shifts its ordinal and defeats the match**, even though
the chunk's content is byte-identical. Each such chunk takes the delete-and-insert
branch, gets a fresh uuid, and loses its vector to the
`artifact_vec_v2_cascade_delete` trigger.

So the guarantee holds for end-appends and in-place content edits, and fails for
mid-body insertion.

## Symptom (Effect)

No error, no wrong data. Retrieval stays correct; the work is simply redone.

The bill lands on the librarian's own append path: `append_entry` with
`anchor_heading` writes `## <ID> — <title>` **before** an existing heading, which is
mid-body by construction. On `docs/trackers/bug-fix-session-log.md` (498 chunks at
the 2,048-char budget) an append near the top re-embeds essentially the whole file.

## Root cause

```rust
// src/librarian/catalog/chunk.rs — the reuse lookup
existing.iter().find(|e| e.chunk_ix == row.chunk_ix && e.content_hash == row.content_hash)
```

`content_hash` is `sha256(content)` and correctly identifies unchanged content.
`chunk_ix` is position. ANDing them makes the selector narrower than the population
it is meant to cover: *chunks whose content did not change* is the population;
*chunks whose content did not change **and** whose ordinal also did not move* is what
the key selects.

## Why the obvious fix is wrong

> **SUPERSEDED 2026-09-06 by the fix at `71077fe9`.** The analysis below is right
> that matching on `content_hash` alone is ambiguous, and wrong that the only ways
> out were the two it lists. A third existed inside the same function: match in
> **two passes over a multimap**, claiming each existing row at most once —
> exact-ordinal first, then same-bytes-at-a-different-ordinal. That resolves the
> ambiguity deterministically and needs no schema decision at all. The two-phase
> ordinal bump *was* still required and is what shipped, at four statements;
> dropping the UNIQUE ordinal was not required and did not happen.

Dropping `chunk_ix` and matching on `content_hash` alone was evaluated and
**rejected**: two chunks with byte-identical content become ambiguous, so it trades
a performance defect for a correctness one.

The remaining options both cost more than a local edit:

- **Two-phase ordinal bump.** `UNIQUE (artifact_id, chunk_ix)` means a surviving
  row's ordinal cannot be `UPDATE`d into a slot a doomed row still holds, so a shift
  needs an intermediate offset pass. It cannot be done by delete-then-reinsert: the
  `AFTER DELETE ON artifact_chunk` trigger would destroy the very vectors being
  preserved.
- **Drop the UNIQUE ordinal**, ordering chunks by `start_line` instead. This
  reopens a schema decision taken in the same work.
## Evidence

Found in review of Task 5 of the chunk-grain plan, before any caller existed, and
confirmed **by construction** rather than by a run.

*(Corrected 2026-09-06.)* This section used to add that no production caller of
`replace_chunks` existed, so the defect had never fired in anger. Both halves are
now false. `index_repo_sync` (`:467`, `:505`) and `backfill_chunk_vectors`
(`:1150`) all call it, so the defect was live on every reindex; and it has now
been reproduced by an executed test rather than reasoned about — the regression
test below failed with W-1's `chunk_id` changed across a pure ordinal move.
## Hypotheses tried

1. **Hypothesis:** matching on `content_hash` alone is a cheap correct fix.
   **Test:** reasoned through the duplicate-content case.
   **Verdict:** rejected — two chunks with identical content become ambiguous,
   trading a performance defect for a correctness one.

## Fix

Applied at `71077fe9` (`experiments`), patch-id
`db84c43896130ca473956ca35f9ae72018dd8cf5`.

The reuse key is now `content_hash` alone, matched in **two passes over a
multimap** so that each existing row is claimed at most once:

1. exact `chunk_ix` match — preferred, so a body whose ordinals did not move
   writes exactly what it wrote before this scheme existed;
2. any still-unclaimed row with the same `content_hash`, taken in ordinal order —
   the chunk only moved.

Landing the survivors is the two-phase ordinal bump this file costed as expensive.
It is four statements: DELETE every unclaimed row, park each mover on the sentinel
ordinal `-(new_ix + 1)`, INSERT the genuinely new rows, then land the movers and
re-sync positions in one UPDATE. No schema change — the UNIQUE ordinal stays.

One further correction rides along: DELETE is now keyed on `chunk_id`, never
`chunk_ix`. A reused row can be sitting on an ordinal that another row is about to
take, and an ordinal-keyed DELETE would take that reused row down with it — losing
exactly the vector the function exists to keep.

**The deferral rationale had decayed, and that is the transferable part.** It read
*"no production caller of `replace_chunks` exists yet"* and deferred the decision
until the real call sites were visible. True on 2026-09-02; false by 2026-09-06.
Nothing re-opened the file in between, because a deferral's whole function is to
stop people looking — the claim was dated but carried no decay class, so it read as
a settled decision rather than as a fact about an instant.
## Tests added

`a_chunk_that_only_moved_ordinal_keeps_its_id_and_its_vector`, in
`src/librarian/catalog/chunk.rs`. Written first and **watched red**.

Two fixture preconditions are load-bearing, and are asserted rather than assumed:
W-1's `content_hash` must be **unchanged** by the insertion, and its `chunk_ix`
must have **moved**. Drop the first and it becomes a content-change test; drop the
second and it duplicates
`an_unchanged_chunk_gets_its_line_range_resynced_when_content_above_it_shifts`.
Either way it would still pass against the bug.

It also asserts that the persisted ordinals come back a dense `0..n` run. That is
what catches a two-phase shift whose second pass is skipped — that failure leaves
the negative sentinels on disk, and an id-only assertion cannot see it.

**Two mutations, one per guarded site**, because a kill at one site says nothing
about the other:

| mutation on the production path | result |
|---|---|
| pass 2 removed (the reuse key) | RED — W-1's id changed: the original defect |
| parking phase disabled (`if false &&`) | RED — `UNIQUE constraint failed: artifact_chunk.artifact_id, artifact_chunk.chunk_ix`, in **two** tests |

The second mutation also reds the pre-existing
`growing_an_entry_corrects_the_of_n_on_siblings_that_did_not_change`, which turns
out to exercise the ordinal-move path as well. It was already covering this shape
and could not report on it, because the old code never attempted the move at all —
the delete-and-insert branch it took is monotone under the defect.
## Workarounds

None needed — correctness is unaffected. The cost is redundant embedding work.

## Resume

> **CLOSED 2026-09-06 — fixed at `71077fe9`, patch-id
> `db84c43896130ca473956ca35f9ae72018dd8cf5`, four-command gate green on
> `experiments`.** Nothing outstanding. The schema decision this file was waiting
> on turned out not to be needed: the UNIQUE ordinal stays.

One defect noticed while fixing this one, filed separately rather than folded in:
the embedded text is not `content` alone — `embed_queue_items` prepends the
chunk's entry token and that entry's **title** for a mid-entry chunk — while
`content_hash` hashes `content` only. So the reuse key omits two inputs the vector
actually depends on. That is orthogonal to the ordinal defect and predates it.
