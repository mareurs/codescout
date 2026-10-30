---
id: '5d9cb8fddcaae130'
kind: bug
status: open
title: memory write embeds and upserts the same content twice (avg 2.9 s)
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-30
severity: low
unverified: The phase split of the 2.9 s is not measured; the double embed is read from code.
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

Embed once and upsert once; fall back to an upsert without anchors if the search fails.

## Tests added

None yet.

## Workarounds

None.

## Resume

Time `guarded_write`, the two embeds, `search_code` and the two upserts separately.

## References

N/A
