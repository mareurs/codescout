---
id: '2b61de99742ee1d3'
kind: bug
status: open
title: 'BUG: doc update --body appends a trailing blank line absent from its input, so every round-trip edit of a managed artifact adds one'
owners:
- marius
tags:
- cluster/unclassified
- librarian
- cli
- tool-quirk
topic: librarian body write normalisation
---

## Summary

`codescout doc update <id> --body @<file>` writes a trailing blank line that is not in the
input file. Because editing a librarian-managed artifact means round-tripping — read the
current body, modify, write it back — that blank becomes part of the next input and another
is appended. **One blank line accumulates at EOF per write.**

Cosmetic in isolation. It matters because the round-trip is the *only* sanctioned way to
make a targeted edit to a managed ledger whose section is too large to replace through
`body_edits`, so the writes are not rare.

## Reproduction — measured, two consecutive writes

`docs/trackers/reconnaissance-patterns.md`, artifact `5696563f06b2c222`, 2026-09-05.

Input file's last three lines (`tail -3 … | cat -A`):

```
$
  Why this block carries all of it: R-99. A convention documented anyw…
  than the thing authors copy is not a convention. -->$
```

The file the writer produced:

```
  Why this block carries all of it: R-99. A convention documented anyw…
  than the thing authors copy is not a convention. -->$
$
```

Input ends `-->\n`; output ends `-->\n\n`. The `git diff` for that write carried a
`+` empty line at EOF alongside the intended change, on **both** of two consecutive
writes — the second having inherited the first's blank through the round-trip.

## Mechanism (unconfirmed)

Not investigated. The shape suggests the writer appends `\n` to a body it has already
newline-terminated, rather than normalising to exactly one trailing newline. The leading
edge behaves differently and correctly: a body handed over *without* the blank line that
separates frontmatter from the first heading gets exactly one inserted, which is why
starting the extraction one line lower repaired a double blank rather than creating one.
So there is normalisation at the head and apparently unconditional appending at the tail.

## Why it is easy to miss

The `+` blank line sits at the very end of a `git diff` whose interesting hunk is
thousands of lines earlier, and it is one line in a file of ~8,400. Both times it was
caught only because the write was verified by predicting the insertion count first
(`+16 / -1`) and reconciling the actual (`+17 / -1`) — the arithmetic is what surfaced
it, not reading the diff.

## Severity

Low. Trailing blank lines do not affect markdown rendering, the catalog, chunking, or
citation resolution. The cost is unexplained whitespace churn in commits, attributed to
an author who did not write it — which is why it is worth a record rather than a silent
tolerance.

## Workaround

Strip trailing blank lines from the body file before passing it, and expect the writer to
re-add exactly one.

## Suggested direction (not a plan — reproduce first)

Normalise the body to exactly one trailing newline at the write boundary rather than
appending unconditionally, matching what the head already does. A regression guard should
assert the round-trip is a fixed point: writing a body read back unchanged must produce a
byte-identical file, which is the property actually wanted and the one no current test
names.

## Resume

Not started. Filed on notice while using the route for an unrelated correction; nothing
depends on it.

