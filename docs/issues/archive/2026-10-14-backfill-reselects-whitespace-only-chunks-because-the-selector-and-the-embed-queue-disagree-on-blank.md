---
id: ccdbec16bfa4817f
kind: bug
status: fixed
title: 'BUG: backfill re-selects whitespace-only chunks on every run, because the selector and the embed queue disagree on what is blank'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector-gap selection
closed: 2026-10-14
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

Verified and committed on **experiments**.

- **SHA:** `8c02ad779926ec33f51ab436e81fe7f056a7fc9d`
- **patch-id:** `00eabb22f09011ada6735a64186700340a27d39b`

The vector-gap arm skips a chunk whose `trim(c.content, BLANK_CHARS)` is empty. `BLANK_CHARS` (`src/librarian/indexer.rs`) is every character `str::trim` strips, so the selector and `embed_queue_from_chunks` now share one definition of blank. The blank rows themselves are still stored; not storing them is a separate change to `build_chunks`.

## Tests added

- `a_blank_chunk_without_a_vector_does_not_reselect_a_complete_artifact`: observed RED first, the second run reported `(artifacts, embedded, skipped_empty) = (2, 1, 1)` for `(1, 1, 0)`. It uses `''` (2,782 of the live rows) and `"\u{3000}\n"`, which SQLite's one-argument `trim()` keeps.
- `blank_chars_is_every_character_str_trim_strips`: the set equals `{c | c.is_whitespace()}` over all of Unicode.

Applied mutations with `scripts/mutation-probe.sh` in an isolated worktree: one-argument `trim(c.content)` (killed by the first test alone), U+3000 dropped from the set (killed by both tests). The full ordered `./scripts/gate.sh --check` was green on the committed bytes: lean library 4479 passed, default library 6776 passed, 0 failed.

## Workarounds

None needed for correctness; the cost is wasted visits and a misleading `skipped_empty`.

## Resume

Complete. Fixed in `8c02ad77`, patch-id above.

## References

- `src/librarian/indexer.rs` (`embed_queue_from_chunks`, `backfill_chunk_vectors_with`)
- `src/librarian/catalog/chunk.rs` (`build_chunks`, `replace_chunks`)
