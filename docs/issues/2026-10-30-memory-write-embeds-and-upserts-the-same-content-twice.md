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

Done 2026-10-30, SHA `f12eb6b3e` on `experiments` (patch-id `ba13f6189dab68acd8e45f8296b1584cf94fa97d`; the branch it was written on is deleted). `cross_embed_memory` and `create_semantic_anchors` are one function now (src/tools/memory/mod.rs): it embeds the content once, searches for anchors through `find_semantic_anchors`, and upserts once. A failed anchor search files the memory without anchors and returns a `semantic anchor creation failed` warning; a failed embed or upsert is still `cross-embed failed`. Verified first at the bytes: the unfixed code embedded in both functions (`left: 2, right: 1` below).

## Tests added

In `single_embed_and_upsert` (src/tools/memory/tests.rs), through `Memory::call(write)` with a counting embedder and a store that records each upsert's anchors. Both red first with `left: 2, right: 1` (embed count), green after:
- `a_write_embeds_once_and_upserts_once_with_its_anchors` (also asserts the one upsert carries the hit's anchor)
- `a_failed_anchor_search_still_files_the_memory_once_without_anchors`
- `a_failed_embed_stores_nothing_and_warns_once` (added in `24a378e55` on `experiments`: no upsert, `cross-embed failed` exactly once, one embed attempt). Checked by mutation: dropping the warning in `cross_embed_memory` makes it fail, at its status assertion. The once-versus-twice count was not mutation-checked.
Not measured: the 2.9 s average was not re-timed, so the claim is two fewer embeds and one fewer upsert per write, not a latency figure.

## Workarounds

None.

## Resume

Time `guarded_write`, the two embeds, `search_code` and the two upserts separately.

A live measurement reported by another session on 2026-10-31, from `usage.db` and not re-checked by me: a private memory write about 40 ms, a shared one about 1,724 ms, and the anchor search with rerank alone about 1,166 ms. If that holds, the reranked anchor search is the larger part of the 2.9 s and the double embed the smaller, so this fix removed an embed and an upsert but did not change the latency by much. The "about 1 s unexplained" in `unverified` then has a competing explanation (the reranked search) to the `guarded_write` hypothesis. Neither is measured here.

## References

N/A
