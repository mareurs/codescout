---
id: '5d9cb8fddcaae130'
kind: bug
status: fixed
title: memory write embeds and upserts the same content twice (avg 2.9 s)
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
severity: low
unverified: 'The latency was not re-measured; the unexplained ~1 s of the 2.9 s (hypothesis: anchor seeding inside guarded_write) was not investigated. Gate 2026-10-30 green on a clean worktree.'
---

# BUG: memory write embeds and upserts the same content twice (avg 2.9 s)

## Summary

`cross_embed_memory` (`src/tools/memory/mod.rs:283-285`) and `create_semantic_anchors` (`:365-368`) each embed the same content and each upsert it, one after the other (`:1122`, `:1150`). The anchor step also runs `search_code`.

## Class fit

`cluster/unclassified`. What the case shows: two features added at different times that each compute the same derived value from one input.

## Symptom (Effect)

usage.db: `memory action=write` 12 calls, avg 2,898 ms, max 4,538 ms.

## Reproduction

`memory(action="write", topic=..., content=...)` on codescout.

## Environment

Embedder at `127.0.0.1:48081`; Qdrant 1.17.0.

## Root cause

inferred from `src/tools/memory/mod.rs` — not measured. About 1 s of the 2.9 s is unexplained; the hypothesis is anchor seeding inside `guarded_write`.

## Evidence

Code read 2026-10-30 by the librarian explorer of this session.

## Hypotheses tried

None.

## Fix

Done 2026-10-30, SHA f12eb6b3e on branch worktree-agent-a3aee1a8919a0949f (patch-id `ba13f6189dab68acd8e45f8296b1584cf94fa97d`). `cross_embed_memory` and `create_semantic_anchors` are one function now (src/tools/memory/mod.rs): it embeds the content once, searches for anchors through `find_semantic_anchors`, and upserts once. A failed anchor search files the memory without anchors and returns a `semantic anchor creation failed` warning; a failed embed or upsert is still `cross-embed failed`. Verified first at the bytes: the unfixed code embedded in both functions (`left: 2, right: 1` below).

## Tests added

In `single_embed_and_upsert` (src/tools/memory/tests.rs), through `Memory::call(write)` with a counting embedder and a store that records each upsert's anchors. Both red first with `left: 2, right: 1` (embed count), green after:
- `a_write_embeds_once_and_upserts_once_with_its_anchors` (also asserts the one upsert carries the hit's anchor)
- `a_failed_anchor_search_still_files_the_memory_once_without_anchors`
Not measured: the 2.9 s average was not re-timed, so the claim is two fewer embeds and one fewer upsert per write, not a latency figure.

## Workarounds

None.

## Resume

Time `guarded_write`, the two embeds, `search_code` and the two upserts separately.

## References

N/A
