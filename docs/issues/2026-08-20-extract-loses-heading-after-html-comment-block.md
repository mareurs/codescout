---
id: d407da71bd52fa91
kind: bug
status: open
title: link_scan extract() loses a heading swallowed by a preceding HTML comment block
tags:
- link_scan
- extract
- pulldown-cmark
- citation-graph
- html-block
---

# BUG: `link_scan::extract()` loses a heading swallowed by a preceding HTML comment block

## Summary

`crate::librarian::tools::link_scan::extract` does not report `CAP-4` as a definition in
`docs/trackers/capability-proposals.md`. A `<!-- … -->` HTML comment block immediately
above it causes pulldown_cmark to treat the following `## CAP-4 — …` heading as part of
the HTML block, so no `Event::Start(Heading)` is emitted and `def_re` never sees the line.
An entry that `extract()` cannot define is an entry nothing can cite: every citation of it
counts as **dangling**, and `doctor`'s `entry_without_definition` reports it as missing
from a body that plainly contains it.

## Symptom (Effect)

Measured 2026-08-20 across `docs/**/*.md` (1066 files), comparing `extract()`'s definition
set against `entry_sections()`'s:

```
CORPUS files=1066 sections=1101 disagreeing_files=1 end_line_overshoot_sections=0
  docs/trackers/capability-proposals.md: extract_only=[] (0) sections_only=["CAP-4"] (1)
```

`entry_sections` finds `CAP-4`; `extract()` does not. It is the **only** remaining corpus
disagreement between the two.

## Reproduction

Bisect the file by start line and re-run `extract()`:

- input beginning at **line 1160** → `extract()` finds `CAP-4`
- input beginning at **line 1159** → it does not

Line 1159 of `docs/trackers/capability-proposals.md` opens an HTML comment block:

```
<!-- Insert new CAP-N entries above the "## Anti-goals" heading. …
```

The `## CAP-4 — …` heading is at line 1161.

## Environment

codescout `experiments` at `eb822f86`, Linux. `extract()` is
`src/librarian/tools/link_scan/extract.rs`; the markdown parser is `pulldown_cmark`.

## Root cause

pulldown_cmark follows CommonMark's HTML-block rules: an HTML block opened by `<!--`
continues until its end condition is met, and intervening lines — including ATX headings —
are emitted as `Event::Html`, not as heading events. `extract()` only treats a `def_re`
match as a definition when it arrives inside a heading event, so the swallowed heading is
invisible to it.

*Inferred from the bisect result and from `extract()`'s heading-event gating — the
pulldown_cmark event stream for this input was NOT dumped and read directly. Confirming
the mechanism means printing the events around line 1159 and observing `Event::Html`
covering line 1161.*

Measured 2026-08-20 by the corpus comparison above (Task 1 review + re-review of
`docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md`).

## Evidence

The original review's corpus scan, before the Task 1 fence fix, found two disagreeing
files; the re-review after the fix found one:

| Metric | Before Task 1's fix | After |
|---|---|---|
| sections found | 1073 | 1101 (+28) |
| disagreeing files | 2 | **1** (`CAP-4` only) |
| `end_line` overshoot | 20 | 0 |

The other disagreement — 28 definitions dropped in `docs/trackers/bug-fix-session-log.md`
by a hand-rolled fence toggle — was a defect in the new code and is fixed
(`eb822f86`). This one predates it and lives in `extract()`.

## Hypotheses tried

1. **Hypothesis:** the heading is malformed and legitimately defines nothing.
   **Test:** `entry_sections()`, which takes headings from
   `librarian::preview::headings::parse` (a line-oriented, `FenceState`-backed ATX
   parser), over the same file.
   **Verdict:** rejected — `entry_sections` reports `CAP-4` normally, so the heading is
   well-formed and the difference is in how each side finds headings.

## Fix

Not attempted. Two directions, and the choice is not obvious:

- **Narrow:** have `extract()` also scan `Event::Html` payloads for `def_re` matches at
  line starts. Cheap, but re-implements heading detection inside HTML text and would then
  match headings that really are inside a comment.
- **Structural:** move `extract()` onto the same `librarian::preview::headings::parse`
  that `entry_sections` now uses, so both derive definitions from one line-oriented
  parser. Larger, but it is the direction Task 1's review already pushed the sibling
  function, and it would make the two agree by construction rather than by coincidence.

The structural option deserves its own decision because `extract()` is the citation
graph's front door — `link_scan`, `doctor`'s `corpus_cited_tokens`, and
`scan_undefined_entries` all read through it.

## Tests added

None. The corpus comparison that found this is not committed — see *Resume*.

## Workarounds

Move the `<!-- … -->` block so it does not immediately precede an entry heading, or close
it with a blank line before the heading. That repairs one file and not the class.

## Resume

Two independent next actions:

1. **Confirm the mechanism rather than infer it.** Dump the pulldown_cmark event stream
   for `docs/trackers/capability-proposals.md` around lines 1155-1165 and observe whether
   `Event::Html` covers line 1161. The root cause above is currently a hypothesis wearing
   a conclusion's clothes.
2. **Commit the agreement check.** The comparison of `extract()` against
   `entry_sections()` over `docs/**/*.md` exists only in a review transcript in a
   gitignored workspace. As a test it is a strong regression guard — and with this bug
   open, `CAP-4` is the *entire* expected signal, so the assertion is
   `disagreeing_files == 1 && sections_only == ["CAP-4"]` until this is fixed.

## References

- `src/librarian/tools/link_scan/extract.rs` — `extract`, `def_re`
- `src/librarian/preview/headings.rs` — `parse`, the line-oriented alternative
- `src/util/markdown_fence.rs` — `FenceState`, from
  `docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md`
- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` — Layer 3
  depends on `extract()`'s definition set

