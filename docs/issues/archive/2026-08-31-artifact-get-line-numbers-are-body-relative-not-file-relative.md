---
kind: bug
status: fixed
title: artifact(get) reports body-relative line numbers while grep and link_scan report file-relative ones, so a heading map and a citation finding cannot be composed
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- line-numbers
- coordinate-frame
- composability
closed: 2026-09-09
opened: 2026-08-31
owner: marius
related:
- bug-fix-session-log:F-128
severity: med
unverified: start_line/end_line composability with grep/link_scan remains unfixed (deliberately, per F-128) — only headings/occurrences are file-relative now
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

Shipped a scoped version of Option 1, not the full recommendation above.

**Fixed:** `preview.headings[*].line`, `preview.last_heading.line`, and the ambiguous-heading
`body_meta.occurrences` array are now file-relative — offset by
`frontmatter::body_line_offset()` (already built for the analogous chunk/embedding line-range
bug, `docs/issues/archive/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`,
but never wired into `doc(get)` until now). This fixes exactly the composed-call failure in
§ Evidence: a `link_scan` finding's line now lines up with `doc(get)`'s heading map.

**Deliberately NOT fixed:** `start_line`/`end_line` stay body-relative, both incoming and in
the `body_meta` echo. Reconnaissance before editing (bug-fix-session-log:F-128) found two
existing regression tests — `line_slice_returns_requested_range` and
`line_slice_start_line_1_returns_first_visible_content_line` — that pin the current
body-relative contract as correct for every existing caller of `doc(get, start_line=…)`.
Changing that interpretation is a real breaking change to a tested, documented contract, not
just a fix to this bug's failure mode, and deciding to break it is bigger than one bug-fix
session should do unilaterally.

**Shipped Option 2's fallback instead, for the part Option 1 couldn't safely reach:** a new
top-level `frontmatter_lines` field on every `doc(get)` response (0 when there's no
frontmatter), so a caller still using `start_line`/`end_line` can convert to file-relative
itself. `source_line_count` was left alone — it wasn't part of the demonstrated failure and
changing its meaning would affect the soft-cap/pagination logic that reads it.

Fix SHA: `d26d3cd636b2d74fb25ff93e24e3c675d0dfdd72` (experiments)
Patch-id: `c9c089987a516572054fb1c81292b47b3427e50b`
## Tests added

Four, in `src/librarian/tools/get.rs`'s `tests` module, all following the trap-avoidance rule
this section originally specified (expected line/offset derived independently from the raw
fixture text via `.lines().position()`/`.enumerate()`, never by re-deriving the production
offset):

- `heading_map_lines_are_file_relative_not_body_relative`
- `ambiguous_heading_occurrences_are_file_relative` — the literal-line sibling of
  `duplicate_heading_reports_ambiguous_not_missing`, which had asserted document order only
  and said explicitly why ("the frontmatter-stripping frame is exactly what this bug's
  sibling is about").
- `response_reports_frontmatter_line_count_for_offset_conversion`
- `response_reports_zero_frontmatter_lines_when_there_is_no_frontmatter`

All four watched RED against the pre-fix code (body-relative values / missing field) before
the fix landed. The two pre-existing `start_line`/`end_line` regression tests
(`line_slice_returns_requested_range`, `line_slice_start_line_1_returns_first_visible_content_line`)
were run and left unchanged — confirming the scoped fix doesn't touch that contract.
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
