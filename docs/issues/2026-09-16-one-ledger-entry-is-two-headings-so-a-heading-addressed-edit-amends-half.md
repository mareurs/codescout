---
id: '6cd56693dd5a95db'
kind: bug
status: open
title: 'BUG: one ledger entry is two headings apart, so a heading-addressed edit amends half of it and reports success'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- doc-tool
- trackers
topic: librarian document editing
closed: null
opened: 2026-09-16
severity: medium
---

# BUG: one ledger entry is two headings 16,000 lines apart, so a heading-addressed edit amends half of it and reports success

## Summary

An `F-N` / `W-N` entry is **one logical unit with two text surfaces**: a `## F-N — <title>`
section, and a `| F-N | … |` row in the ledger's `## Index` table. `doc(action="update",
patch={body_edits:[…]})` addresses by **heading**. The two surfaces live under **different**
headings — in `bug-fix-session-log.md` today, `## Index` at line 64 and `## F-168 — …` at line
16,671 — so an edit addressed to the entry's own heading reaches the section, succeeds, and
returns `updated: true` while the row still says whatever it said before.

Nothing in the call, the response, or the tool's contract indicates a second surface exists.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live, by code read.** Both
surfaces still exist (`bug-fix-session-log.md`: `## Index` :64, `| F-168 |` :243, `## F-168 — …` :16675), and
`apply_body_edits` (`src/librarian/tools/update.rs:285-390`) detects neither entry-shaped headings nor Index rows;
the response (`:806-835`) carries no such warning, and `src/librarian` has no index-row warning anywhere. Not run
live: the reproduction is a shared-catalog write and the CLI `doc update` takes no body_edits. **A guide conflict
feeds it:** `librarian.md:144` says "keep the table too if it reads well" while `tracker-conventions.md:807`
says not to hand-maintain an index beside sections.

## Symptom (Effect)

Two sessions verified the same quoted figure against `F-168` and reached **opposite verdicts**,
both correct:

| reader | surface read | result |
|---|---|---|
| the amending session | `## F-168 — …` section | quoted phrasing **0** occurrences |
| the citing session | `| F-168 …` Index row | quoted phrasing **1** occurrence |

The citing session was one step from **retracting a true claim as fabricated**, on the basis
that the phrase it had quoted verbatim did not appear in the entry it cited. A retraction of a
correct statement is worse than the ambiguity that caused it: it removes a true datapoint and
adds a false admission.

## Reproduction

1. Pick any ledger entry with both a section and an Index row.
2. `doc(action="update", id=…, patch={body_edits:[{heading:"## F-N — <title>", action:"edit",
   old_string:…, new_string:…}]})` — amend a figure or a claim in the section.
3. `grep '^| F-N ' <ledger>` — the row is byte-identical.
4. The response said `updated: true` and was correct.

Measured 2026-09-16 on `F-168`: three section edits left the Index row byte-identical to
`c054113b`, verified by `diff` against that commit.

## Root cause

**Not a missing capability — a missing composition.** The tool can reach both surfaces:
`heading="## Index"` edits the row, and this session used exactly that call to write `F-168`'s
row in the first place. What does not compose is the **addressing**: the entry's identity is
`F-168`, and no single heading selects everything that identity names.

So a caller who knows the tool perfectly still needs to know a **layout convention of the
document** — that entries are split, and where the other half lives. The tool's addressable
unit (a heading) is narrower than the artifact's logical unit (an entry), and the gap is filled
by the caller's model rather than by anything checkable.

`get_guide("tracker-conventions")` § *One entry format, never two* argues the general case
against exactly this split. The split exists anyway, in the repo's most-written ledger.

## Evidence

**Correction to this file's own first framing, recorded because the wrong version was published
to two parties before it was checked.** This session initially stated the row was *"structurally
incapable"* of being reached by `doc(update)`, and said so to a peer and to its operator. That is
false: `heading="## Index"` reaches it. The true defect is narrower and sharper — the capability
exists and the addressing does not compose — and the false version would have sent a reader
looking for a tool limitation that is not there. Caught by checking which heading the row
actually lives under, before filing.

## Hypotheses tried

1. **The tool cannot edit index rows.** **Refuted** — `doc(action="update", heading="## Index",
   action="edit")` writes them, and is how every row in that ledger was added.
2. **The citing session misquoted.** **Refuted** — the phrase occurs once, in the row.

## Fix

Not attempted, and the choice is a document-structure question rather than a tool one, which is
why this file does not pick:

- **Make the entry one surface.** What `tracker-conventions` already argues. Largest change;
  four sessions write that ledger concurrently and the Index table is what makes it scannable.
- **Have `append_entry`'s two-surface write be the only writer**, so amendments go through a
  call that knows both halves — it already writes section and row in one `fs::write`.
- **Name the other surface in the response.** Cheapest: when a `body_edits` heading matches an
  entry-shaped heading (`^## [A-Z]+-\d+ —`), have the response note that an Index row for that
  id exists and was not touched. Does not change any document, and closes the *silent* half.

## Tests added

None — no fix chosen. The shape any guard needs: amend a section through `doc(update)` and
assert the response mentions the untouched row; paired with an amendment to an entry that has
**no** row, asserting silence. Without the pair, a response that always warns passes the first
assertion.

## Workarounds

**Treat an entry id as naming two surfaces and check both.** After amending a section, read
`grep '^| F-N ' <ledger>` as a separate step — a section-scoped read cannot see the row, so the
verification must be scoped differently from the edit. When citing an entry in prose, **name the
surface** (*"F-168's Index row reads …"*), which is what made the disagreement above resolvable.

## Classification

`cluster/selector-narrower-than-its-population` (`IC-18`). `heading="## F-168 — …"` names the
entry to its caller and selects a proper subset of it; the excluded surface is never examined,
so the call returns a well-formed `updated: true` that is true of what it did and silent about
what it did not.

## Resume

Decide between the three options in § *Fix*. The third is cheap, changes no document, and
closes the silent half rather than the split — it is the only one that does not need agreement
from the sessions sharing the ledger.

## References

- `docs/trackers/bug-fix-session-log.md` — `F-168`, the entry the incident happened on; its
  section and Index row still differ in phrasing, deliberately (the row is quoted verbatim by
  `F-170`).
- `get_guide("tracker-conventions")` § *One entry format, never two*.
- The row-needs-its-own-check-before-the-next-amendment formulation is sessionId
  `9403d62d-116b-46ea-ac9b-004acff2b1cb`'s.
