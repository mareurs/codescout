---
id: 1da3de9ac6d671d9
kind: bug
status: fixed
title: 'BUG: doc update --body appends a trailing blank line absent from its input, so every round-trip edit of a managed artifact adds one'
owners:
- marius
tags:
- cluster/unclassified
- librarian
- cli
- tool-quirk
topic: librarian body write normalisation
closed: 2026-09-08
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

Two, both observed RED before the fix.

- `writing_back_an_unchanged_body_is_a_fixed_point` — read a body, write it back unchanged,
  assert the **file** is byte-identical. The property this file asked for, and the one
  nothing named before. It fails if **either** end regresses, which a trailing-only
  assertion would not.
- `a_body_that_already_leads_with_a_blank_line_does_not_gain_a_second` — states the head's
  contract directly rather than relying on the shape `get` happens to return, so a later
  change to `get`'s trimming cannot silently retire the coverage.

Both assert on the **file**, not the response: the defect is in what reaches disk, and a
response-level assertion would pass while the file drifted.
## Suggested direction (not a plan — reproduce first)

Normalise the body to exactly one leading and one trailing newline at the write boundary,
rather than appending unconditionally. Assert the round-trip is a fixed point: writing back
a body read unchanged must produce a byte-identical file.

**Done 2026-09-08** — fix SHA `e14faac4` (`experiments`), patch-id
`4757c9489d2273ac87a42a4907033abe51b937dd`.

### The mechanism above was wrong in a way worth keeping

§ *Mechanism (unconfirmed)* guessed *"normalisation at the head and apparently
unconditional appending at the tail."* **There is no head normalisation.** One expression,
inlined at two sites — `format!("\n{body}\n")` at `update.rs:532` and `create.rs:398` —
unconditional at **both** ends. The head only looks well-behaved because the common path
hands it input without a leading newline.

This file's own evidence said so and was read the other way: *"starting the extraction one
line lower repaired a double blank rather than creating one"* is the head doubling. Filed
as an asymmetry; it is one defect with two ends. Reproduced directly — a body arriving with
the separator blank already present yields `---\n\n\n`.

### Extracted, not fixed twice

Both sites now call `frontmatter::render_body`. Two call sites had already inlined the
expression and a third would reintroduce it silently, so the normalisation is stated once.
Same shape as `test-escape-hardening:I-2`, which extracted `escape_like_pattern` for that
reason. CLAUDE.md's *mutate once per guarded SITE* is the same argument from the test side:
fixing `update` alone would have left `create` seeding every new artifact with the blank.

### What the RED run added to the report

`writing_back_an_unchanged_body_is_a_fixed_point` failed showing `some text\n\n\n` after
write 1 and `\n\n\n\n` after write 2. Growth of exactly one newline per write, as filed —
**and already two newlines deep after the first update**, because `create` applies the same
wrap. The accumulation starts one step earlier than this file records.
## Resume

**Closed 2026-09-08** at `e14faac4`. Gate green — fmt, clippy, lean 3560, default 5512 with
only the documented `peer::server` idle-timeout flake, which lives in a directory this
change does not touch and passes in isolation at 1.13s.

No residual. The `low` severity in § *Severity* still stands as written — the cost was
whitespace churn misattributed to an author who did not write it — but the fix was not
low-value: it removes a per-write growth from the **only** sanctioned route for editing a
large managed section.

One datum recorded for its own sake: this file's EOF was `...depends on it.\n\n` when the
fix landed — its own accumulated blank — and the first write through `render_body`
collapsed it to a single newline. The repair is retroactive on next write; no sweep is
owed.
