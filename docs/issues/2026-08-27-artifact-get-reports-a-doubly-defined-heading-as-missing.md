---
id: ad610ff996e932fa
kind: bug
status: open
title: 'BUG: artifact(get) reports a doubly-defined heading as `heading_missing`, in a response whose own heading map lists it twice'
tags:
- librarian
- artifact-get
- error-diagnosis
- headings
closed: null
opened: 2026-08-27
owner: marius
related:
- docs/issues/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md
severity: medium
---

# BUG: artifact(get) reports a doubly-defined heading as `heading_missing`, in a response whose own heading map lists it twice

## Summary

`artifact(action="get", heading=X)` returns `body_meta.heading_missing: true` when `X`
matches **several** headings. "Missing" is the opposite diagnosis: it sends the caller
looking for a heading that is present more than once. The same response's
`preview.headings` array lists both occurrences, so the payload contradicts itself. The
read surface also never received the `occurrence` selector that `164c8bd6` added to the
write surfaces, so there is no way to ask for one of them.

## Symptom (Effect)

```
artifact(action="get", id="01291679a5ee4707", heading="### The ask")

  preview.headings:  … {"level":3,"text":"The ask","line":642} …
                     … {"level":3,"text":"The ask","line":797} …
  body:              ""
  body_meta:         {"line_count":0, "bytes":0,
                      "heading":"### The ask", "heading_missing": true}
```

The call succeeds (`isError: false`, no `error` key). Nothing in the response says
"ambiguous".

## Reproduction

`git rev-parse HEAD` → `550b6bb5` (branch `experiments`), against a binary built from it.

```
artifact(action="get", id="01291679a5ee4707", heading="### The ask")
```

`docs/trackers/capability-proposals.md` defines `### The ask` once per CAP entry
(CAP-7, CAP-8, CAP-10, CAP-11) — correct per-entry template structure, not a defect in
that file. Any artifact with a repeated heading reproduces it.

## Environment

Linux, branch `experiments`, codescout MCP over stdio, project `codescout`.

## Root cause

Three frames, and the information is discarded at the middle one.

1. `resolve_section_range` (`src/tools/file_summary/file_summary.rs`) distinguishes the
   two states correctly: `heading '<X>' found N times (lines …)` for ambiguity, `heading
   '<X>' not found` for a genuine miss. Both are `RecoverableError`.
2. `find_heading_section` (`src/librarian/tools/get.rs:26-30`) calls
   `extract_markdown_section(body, query).ok()`. **`.ok()` collapses every `Err` to
   `None`**, erasing which of the two it was.
3. `get.rs:503-511` maps `None` to `json!({"heading": name, "heading_missing": true})` —
   one label for both states, and it names the one that is false.

The plural form has the same shape: `get.rs:512+` accumulates a `missing` list from the
same `Option`.

This is adjacent to a known, deliberate design choice rather than a violation of one.
`src/usage/db.rs:288-290` already documents that `artifact(get)` "swallows the same miss
into `body_meta.heading_missing` and stays success", unlike `read_markdown` /
`edit_markdown` which raise. Staying success is fine; **labelling ambiguity as absence is
not**, and the two are separable.

## Evidence

### Positive controls — it is duplication, not encoding

The heading contains an em-dash and an apostrophe in the case where it was first noticed
(`### BL-43 — complete BL-41's coverage: …`), so character mismatch was the competing
explanation. Two controls against the same artifact, 2026-08-27:

| query | unique? | em-dash | apostrophe | result |
|---|---|---|---|---|
| `### BL-44 — a params row can drift …` | yes | yes | no | resolves, `bytes` > 0 |
| `### BL-1 — … the overflow hint's own recovery works` | yes | yes | **yes** | resolves, `bytes: 103` |
| `### BL-43 — complete BL-41's coverage: …` | **no, ×2** | yes | yes | `heading_missing: true` |

Encoding is ruled out; duplication is the discriminator.

### Cost, observed

This cost a real detour. Diagnosing the duplicate `### BL-43` in
`docs/trackers/open-issue-work-queue.md` began by trying to read it; `heading_missing`
read as "the heading was renamed or I mistyped it", and the next several calls went
looking for a string that was sitting there twice. The heading map in the *same*
response would have answered it immediately, had anything pointed at it.

## Hypotheses tried

1. **Hypothesis:** the heading string is mistyped, or its em-dash/apostrophe differ from
   the file's. **Test:** two unique-heading controls from the same artifact, one carrying
   an apostrophe. **Verdict:** rejected — both resolve.
2. **Hypothesis:** `artifact(get)` uses `resolve_section_range` and the ambiguity error is
   propagated. **Test:** read `find_heading_section` and the call site.
   **Verdict:** partly — it *does* reach `resolve_section_range` via
   `extract_markdown_section`, but `.ok()` at `get.rs:27` discards the error before the
   caller can distinguish it.

## Fix

*Plan; not implemented.*

1. Stop discarding the distinction. Have `find_heading_section` return the
   `RecoverableError` (or a small enum) rather than `Option`, so `get.rs:503-511` can tell
   the two apart.
2. Emit `heading_ambiguous: true` alongside the occurrence count and their line numbers —
   the data is already in `resolve_section_range`'s error. Keep `heading_missing` for the
   genuine miss. Staying `isError: false` is correct and should not change.
3. Accept `occurrence` on `artifact(get)`'s `heading` (and per-entry in `headings`),
   mirroring what `164c8bd6` added to `edit_markdown` and `body_edits`. The resolver
   already supports it; only this entry point does not pass it. Without step 3 a caller
   can be *told* the heading is ambiguous and still have no way to read either section.

Steps 1–2 fix the false statement; step 3 makes the surface usable. Step 3 without 1–2
would leave a caller guessing that `occurrence` is even relevant.

## Tests added

None yet — not implemented. Owed:

- a heading present twice yields `heading_ambiguous` with both line numbers, and **not**
  `heading_missing` (mutation control: collapsing both to `heading_missing` fails it)
- a heading present zero times still yields `heading_missing` (guards over-correction)
- `heading` + `occurrence: 2` returns the second section's bytes
- the plural `headings` path reports an ambiguous member distinctly from a missing one

## Workarounds

- Read the `preview.headings` array in the same response — it lists every occurrence with
  its line number, and is populated even when `body` comes back empty.
- Read the section by line slice instead: `artifact(action="get", start_line=N,
  end_line=M)`. Note the frame — those line numbers are **body-relative** while
  `grep -n` on the file is file-relative, differing by the frontmatter length.
- To *edit* rather than read, `body_edits` already accepts `occurrence` as of `164c8bd6`.

## Resume

Change `find_heading_section` (`src/librarian/tools/get.rs:26-30`) to propagate the error
instead of `.ok()`, then widen the `None` arm at `get.rs:503-511` into two arms. Anchor on
the existing tests in the same file: `heading_missing_sets_meta_flag` (`get.rs:815`) pins
the genuine-miss behaviour and must keep passing unchanged.

## References

- `src/librarian/tools/get.rs:26-30` — `find_heading_section`, the `.ok()` that erases the distinction
- `src/librarian/tools/get.rs:503-511` — the `None` arm that names it `heading_missing`
- `src/librarian/tools/get.rs:815` — `heading_missing_sets_meta_flag`, the genuine-miss test
- `src/usage/db.rs:288-290` — the comment documenting that `artifact(get)` swallows the miss and stays success
- `docs/issues/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md` — the write-surface half, fixed in `164c8bd6`; this is the read surface it did not reach

