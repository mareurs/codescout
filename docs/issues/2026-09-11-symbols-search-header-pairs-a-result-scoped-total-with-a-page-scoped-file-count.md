---
id: '0649a087dabaee59'
kind: bug
status: open
title: symbols search renders a result-scoped match total beside a page-scoped file count, so the same query answers 4 files or 11
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
topic: overflow reporting scope
---

## Summary

`symbols` search renders a header pairing a **result-scoped** match count with a **page-scoped**
file count, in one sentence, with no marker on the second. The same query at two `limit` values
answers `57 matches in 4 files` and `57 matches in 11 files` — the match total is right in both,
the file count is the number of distinct files among the rows that survived the page.

## Symptom (Effect)

```
symbols(name="scan", path="src/librarian/", kind="function", limit=5)
  -> 57 matches in 4 files          <- 4 = distinct files among the 5 SHOWN

symbols(name="scan", path="src/librarian/", kind="function", limit=60)
  -> 57 matches in 11 files         <- 11 = the truth
```

`57` is stable across both, which is what makes the sentence read as internally consistent: one
number is visibly a total, so the number beside it inherits that reading. The overflow line
immediately below says `showing 5 of 57` — it marks the ROWS as paged and says nothing about the
file count, so a reader who correctly registers "these rows are a page" has no reason to suspect
the file figure is one too.

## Reproduction

Any search whose matches span more files than one page can show. The two calls above, run
2026-09-11 at HEAD `7ff820f5`, against a binary containing `94aedcd9`.

## Root cause

`src/tools/symbol/display.rs`, `format_search_symbols`:

```rust
let total = val["total"].as_u64().unwrap_or(symbols.len() as u64) as usize;  // :116  result-scoped
…
let groups = group_by_file(&normalized);                                     // :153  page-scoped
let files = groups.len();                                                    // :154
…
let out = render_grouped(&groups, total, files, noun, render_item);          // :223
```

`normalized` derives from `val["symbols"]`, which is the served page. `total` is read from the
explicit result field; `files` is recomputed from the rows in hand. Two scopes, one sentence.

**The correct value is already in the payload.** `build_by_file`
(`src/tools/symbol/symbols.rs:38-50`) counts over the **full** match set and returns
`(entries_capped_at_15, files_omitted_by_cap)`, so the true count is
`by_file.len() + overflow.by_file_overflow` — both already serialized. The renderer consults
neither.

**A sibling renderer already does this correctly**, which makes it a divergence between two
implementations of one idea rather than an unsolved problem — `src/tools/grep.rs:662`:

```rust
let files_count = val["files_count"].as_u64().unwrap_or(files_arr.len() as u64);
```

Explicit field first, array length only as a fallback.

## Evidence

- The two calls above, differing only in `limit`.
- `display.rs:116` vs `:153-154` — the two scopes, read at the bytes.
- `symbols.rs:38-50` — the producer computes over `matches`, not over a page.
- `grep.rs:662` — the correct pattern, in a neighbouring renderer.

## Why it is a sibling of the bug just fixed, not a duplicate of it

`94aedcd9` (archived as
`docs/issues/archive/2026-09-03-symbols-by-file-overflow-is-an-unrecorded-ic-13-member.md`) made
`$.overflow.by_file_overflow` reach this same renderer, so a run whose file *breakdown* exceeds
`BY_FILE_CAP = 15` now says so. That is a different quantity: it marks the **breakdown list** as
capped. This defect is in the **header count**, fires at any file count, and in the reproduction
above `by_file_overflow` is `0` — 11 files is under the cap, so the new hint cannot fire and the
header is wrong anyway.

The two are worth reading together: after the fix, this renderer emits three numbers, of which
the match total and the breakdown-overflow hint are result-scoped and the file count is not.

## Hypotheses tried

None — filed on first observation. No fix attempted; `src/tools/symbol/` is being actively worked
by another session.

## Workarounds

Do not read the header's file count as a property of the result unless the call returned
everything. `showing N of M` with `N < M` means the file count is a floor.

## References

- `docs/issues/archive/2026-09-03-symbols-by-file-overflow-is-an-unrecorded-ic-13-member.md`
- `docs/issues/archive/2026-09-07-grep-context-mode-total-is-the-shown-count-and-the-floor-flag-misses-this-site.md`
  — the same substitution one tool over, where the *match* total took the shown count.
- `docs/issues/archive/2026-08-17-grep-narrowing-hint-ranks-by-capped-display-count.md`
