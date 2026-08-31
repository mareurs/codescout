---
kind: bug
status: open
title: artifact(get) reports body-relative line numbers while grep and link_scan report file-relative ones, so a heading map and a citation finding cannot be composed
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- line-numbers
- coordinate-frame
- composability
opened: 2026-08-31
owner: marius
severity: med
---

## Summary

`artifact(action="get")` numbers lines from the start of the **body** — after frontmatter —
while `grep`, `link_scan` and the file on disk number from the start of the **file**. Every
line number in a heading map, in `body_meta`, and in a `start_line`/`end_line` slice is
therefore short by the frontmatter length, which differs per artifact, and nothing in the
response says which space you are in.

The two families are used together constantly: `link_scan` reports a dangling citation at
line N, and the obvious next call is `artifact(get, start_line=N-9, end_line=N+9)` to read
it. That returns a different part of the file, which reads as a stale catalog rather than as
a units mismatch.

## Symptom (Effect)

Measured on `claude-plugins:docs/issues/2026-08-26-bare-cross-repo-entry-tokens-read-as-dangling.md`,
frontmatter lines 1–16, blank line 17, body starting at line 18:

| heading | `artifact(get)` preview | `grep` / disk | delta |
|---|---|---|---|
| `## Summary` | 1 | 18 | 17 |
| `## The class` | 18 | 35 | 17 |
| `## Fix` | 135 | 152 | 17 |
| `## Upstream` | 356 | 373 | 17 |
| total lines | 363 (`source_line_count`) | 380 (`wc -l`) | 17 |

Constant 17 per file: 16 frontmatter lines plus the blank separator. **`## Summary` reported
at line 1 is the giveaway** — no file with frontmatter has a heading on line 1 — and it is
easy to read past, because a heading map's first entry being 1 looks like the natural base
case.

Two codescout tools, one file, same heading, different answers:

```text
grep(pattern="^## Fix$")          ->  152
artifact(get).preview.headings    ->  135
link_scan finding on this file    ->  line 157, which matches disk 157
```

`link_scan` is file-relative and correct. `audit_doc_refs` reports `path:line` for human
consumption and is presumably file-relative too, though that was not measured here.

## Root cause

The catalog stores the body with frontmatter stripped, and `artifact(get)`'s slicing and
heading extraction both run over that stored body. The line numbers are internally
consistent — they are correct coordinates in the body's own space — they are simply not the
space every other tool, every editor, and every `sed -n` uses.

`body_meta.source_line_count` is the one field that hints at it, and its name works against
the reader: "source" reads as *the source file*, so 363 next to a `wc -l` of 380 looks like
catalog staleness rather than a different denominator.

## Evidence — how it actually bit

While re-reading a bug file this session, `link_scan` reported three dangling entry tokens
at line 157. `artifact(get, start_line=148, end_line=165)` returned prose that lives at disk
lines 165–182, with none of those tokens in it. The conclusion formed from that was *the
catalog's stored body is stale relative to disk* — a wrong diagnosis about a different
subsystem, from two instruments that were both working correctly.

What settled it was reading the disk bytes (`awk 'NR>=150 && NR<=162'`), which showed the
tokens exactly where `link_scan` said, and then the arithmetic: 380 − 363 = 17, and
165 − 148 = 17.

## Fix

Two options; the first is what a reader expects, the second is the minimum.

1. **Report file-relative lines from `artifact(get)`** — add the frontmatter offset when
   emitting heading-map lines, `body_meta.start_line`/`end_line`, and when interpreting
   incoming `start_line`/`end_line`. This makes `get`, `grep`, `link_scan` and the editor
   agree, and makes the composed call above work.
2. **Or keep body-relative and label it**: rename `source_line_count` →
   `body_line_count`, and add `frontmatter_lines` and `body_starts_at_file_line` to
   `body_meta` so a reader can convert. Cheaper and non-breaking, but leaves every
   `link_scan` → `get` hop needing manual arithmetic.

Note that the offset is constant per file and **varies between files**, so no caller-side
constant fixes this.

Prefer option 1; if the incoming-parameter change is judged breaking, ship option 2's
fields alongside it so the space is at least nameable.

## Tests added

None yet. The shape that matters, and the trap to avoid:

Assert that the heading map's line for a known heading equals that heading's line **in the
file**, with the expected value obtained independently — a literal, or a separate read of
the fixture — never by adding the same offset the code under test computes. A test that
derives its expectation from the production offset function passes in the broken world by
construction. This is the defect class named in
`docs/issues/archive/2026-08-27-cross-repo-file-qualified-bucket-never-fires.md`, whose
sibling test hand-built the state production derives and so passed over an inert feature.

The fixture must have **non-empty frontmatter**. With empty frontmatter the offset is 0 and
every assertion holds in both worlds.

## Workarounds

Read line-addressed content with a bounded shell command (`awk 'NR>=A && NR<=B'`) when the
line number came from `link_scan`, `grep`, or a human. Treat `artifact(get)`'s heading-map
lines as ordering information only, not as file coordinates.

Note that `read_markdown` refuses librarian-managed files, so the natural way to notice this
— reading the same file through both markdown tools and comparing — is not available.

## References

- `R-150` — the same session's lesson about instruments read in the wrong space; this is the
  benign twin, where two instruments were both right and the composition was wrong.
- `docs/issues/archive/2026-08-27-cross-repo-file-qualified-bucket-never-fires.md` — the
  can't-fail-test pattern the regression test here has to avoid.
