---
id: f4566e7955af8a5e
kind: bug
status: open
title: 'BUG: backfill re-selects whitespace-only chunks on every run, because the selector and the embed queue disagree on what is blank'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector-gap selection
closed: ''
opened: 2026-10-14
related: []
severity: medium
---

# BUG: backfill re-selects whitespace-only chunks on every run, because the selector and the embed queue disagree on what is blank

## Summary

`embed_queue_from_chunks` drops a chunk whose content is whitespace only (`r.content.trim().is_empty()`), so such a chunk never receives a vector. `62177b8c`'s gap predicate selects an artifact when ANY chunk lacks a vector, so an artifact whose only unvectored chunks are blank is selected on every run, visited, and reported under `artifacts` and `skipped_empty` — which the report defines as "correctly vectorless".

## Symptom (Effect)

Measured 2026-10-14 on the live catalog (read-only), with SQLite `trim(content, <every char::is_whitespace character>)`:

```
catalog-wide: whitespace-only chunk rows=2811 in 2808 artifacts; with a vector=0; artifacts selected ONLY by them=853
project codescout: whitespace-only chunk rows=1351 in 1349 artifacts; artifacts selected ONLY by them=1
```

Their contents: `''` 2,782 rows, `'\n'` 28, `'\n\n'` 1. The commit's tests assert that a second run visits nothing (`again.artifacts == 0`); on this corpus a second run would visit at least those 853.

## Reproduction

Store an artifact whose chunk rows are one non-blank chunk with a vector and one whitespace-only chunk; run backfill twice. Both runs report `artifacts: 1, skipped_empty: 1`.

## Environment

Any backend whose gap arm runs (sqlite-vec).

## Root cause

Two definitions of "embeddable" that are not the same predicate: the embed queue's Rust `str::trim`, and the selector's `NOT EXISTS vector`, which has no blank test at all. A blank test in SQL must use the same character set as `str::trim` — SQLite's one-argument `trim(x)` removes only spaces, which is how this record's first count came out low (2,782 rows instead of 2,811).

## Evidence

Review of `62177b8c~1..6cf6d6e8` (session 1e54b0a7), finding 3.

## Hypotheses tried

N/A — mechanism read from the code and measured.

## Fix

Unknown — under investigation.

## Tests added

N/A — not fixed yet.

## Workarounds

None needed for correctness; the cost is wasted visits and a misleading `skipped_empty`.

## Resume

Planned fix: one shared character set equal to `char::is_whitespace`, pinned by an exhaustive test, used in the selector's SQL `trim(content, set)`.

## References

- `src/librarian/indexer.rs` (`embed_queue_from_chunks`, `backfill_chunk_vectors_with`)
- `src/librarian/catalog/chunk.rs` (`build_chunks`, `replace_chunks`)
