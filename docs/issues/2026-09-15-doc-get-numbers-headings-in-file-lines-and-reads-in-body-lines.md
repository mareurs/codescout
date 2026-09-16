---
id: '0135747b8b9664a3'
kind: bug
status: open
title: 'BUG: doc(get) numbers headings in FILE lines and reads in BODY lines, so a heading''s own line reads back empty'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doc-get
- coordinates
topic: artifact addressing
---

## Summary

`doc(action="get")`'s `preview` reports heading positions in **file** lines while its own
`line_count` is in **body** lines, and `start_line` / `end_line` address **body** lines. So
the number a reader lifts from `preview.headings[].line` is in a coordinate system
`start_line` does not accept, and feeding it straight back returns an empty body — with no
error and no indication that a conversion was required.

The offset is `frontmatter_lines`, which ships in the same response. The information needed
to convert is therefore present; what is absent is anything saying a conversion exists.

## Symptom (Effect)

A reader maps a document, sees the section it wants at line N, asks for lines around N, and
gets back either a *different* section's content or `body: ""` with `line_count: 0`. Neither
is an error, and the natural reading of an empty result is **"that section is not there"** —
which is the one conclusion that is false.

## Reproduction

Measured 2026-09-15 against the release binary built 14:54:32. Artifact
`bc284a781fc9f5f2` (`docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`)
is a clean minimal case: 30 file lines, `frontmatter_lines: 13`, 17 body lines, exactly one
heading.

```
doc(action="get", id="bc284a781fc9f5f2")
  -> preview.headings[0] = { level: 2, text: "IC-6 — …", line: 14 }
     preview.line_count  = 17

doc(action="get", id="bc284a781fc9f5f2", start_line=14, end_line=14)
  -> body: "", body_meta.line_count: 0            # empty. no error.

doc(action="get", id="bc284a781fc9f5f2", start_line=1, end_line=1)
  -> body: "## IC-6 — an addressing scheme with no escape hatch and no disambiguator"
```

`30 = 13 + 17` and `14 = 13 + 1`: the heading is **body line 1**, published as **file line
14**.

**Both systems appear inside ONE object, which is the sharpest form of it.**
`preview.line_count` is `17` — the body's length — sitting beside a heading at `line: 14`. A
reader taking both at face value places that heading near the *end* of a 17-line space. It
is the first line.

## Evidence

Met unprompted before it was understood, and the first presentation was the more dangerous
one. Reading `bug-fix-session-log` (`frontmatter_lines: 16`), the preview put
`## Category conventions` at 354; a request for 346–355 returned the category table's rows —
content from *inside* that section but not its heading. That reads as an ordinary
off-by-a-few in a long file rather than as a systematic offset, so the first hypothesis it
invites is "I misread the map", not "these are two coordinate systems". The empty-body form
above is louder; the plausible-but-wrong-content form is what actually shipped the confusion.

## Environment

codescout MCP, `.claude-sdd` profile. Reproduced after two rebuilds the same day, so it is
not an artefact of a stale binary.

## Root cause

Not read.

## Fix

Not attempted. The cheapest remedy is to make the response self-describing rather than to
change either number — both are defensible in isolation, and what is missing is the label.
Smallest first:

1. **Name the space in the field.** `preview.headings[].file_line` (or `body_line`) makes
   carrying the wrong one impossible, and costs a rename.
2. **Accept both on input.** `start_line` already has `frontmatter_lines` in hand, so a
   `line_base: "file" | "body"` parameter is one field.
3. **Refuse rather than return empty.** A `start_line` past `source_line_count` should say so
   and name the total — `docs/adrs/2026-08-27-negative-results-name-their-scope.md` exactly:
   the zero is suspicious, so it owes its scope. **Worth having even if 1 and 2 are
   declined**, because it is what converts a silent wrong answer into a self-explaining one.

## Tests added

None. Note the shape a guard needs, because neither number's own tests can express it: assert
that a line taken from `preview.headings[].line` **round-trips** — feed it to
`doc(get, start_line=…)` and require the heading text it named to come back. A test written
against either coordinate system alone passes today, since each is internally consistent; only
a test that crosses the two can fail.

## Workarounds

Subtract `frontmatter_lines` from any `preview.headings[].line` before passing it to
`start_line`. Better, avoid the arithmetic entirely: address by `heading=` / `headings=`,
which take the text and are unaffected.

## Classification

`cluster/addressing-without-an-escape-hatch` (`IC-6`), the **no-disambiguator** half: one
document, two line spaces, and no field in the response says which one a number belongs to.

**This is the second instance of that half found in this corpus in one day, in an unrelated
subsystem**, which is the part worth noting rather than the instance itself. The other is
`docs/issues/archive/2026-09-15-grep-and-read-file-number-one-buffer-handle-differently.md`
(fixed `57758be9`) — `grep` expanded escaped newlines before matching while `read_file` did
not, so one `@tool_*` handle denoted two line spaces. Same shape, different surfaces,
neither reachable from the other's tests: there, two *tools* disagreed about one handle;
here, two *fields of one response* disagree about one document. A remedy for either does
nothing for the other.

## Resume

Take fix 3 first — it is small, independent of the naming decision, and it is the difference
between a reader who is told and a reader who concludes the section does not exist.
