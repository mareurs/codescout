---
id: '6f7b072ff89eb5f1'
kind: bug
status: open
title: 'BUG: replace_chunks re-embeds every chunk below a mid-body insertion, because its reuse key includes the ordinal'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- embeddings
- retrieval-grain
- performance
closed: null
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
  reopens a schema decision taken in the same work
  (`docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` Task 4).

## Evidence

Found in review of Task 5 of the chunk-grain plan, before any caller existed.
Confirmed by construction rather than by a run — no production caller of
`replace_chunks` exists yet, so the defect has never fired in anger.

## Hypotheses tried

1. **Hypothesis:** matching on `content_hash` alone is a cheap correct fix.
   **Test:** reasoned through the duplicate-content case.
   **Verdict:** rejected — two chunks with identical content become ambiguous,
   trading a performance defect for a correctness one.

## Fix

Not applied. Deliberately deferred: it is a performance defect, the cheap fix is
wrong, and the two real options touch a schema decision that should be made once,
with the real call sites visible.

## Tests added

None. The existing `replace_chunks_preserves_ids_for_unchanged_chunks` covers the
ordinal-stable case only. A regression test would insert a heading above an existing
entry and assert the untouched chunks below keep both their `chunk_id` and their
`artifact_vec_v2` row.

## Workarounds

None needed — correctness is unaffected. The cost is redundant embedding work.

## Resume

Decide between the two-phase ordinal bump and dropping the UNIQUE ordinal, with the
Task 6/7/11 call sites in front of you. Whichever is chosen, the regression test
above is what pins it.

