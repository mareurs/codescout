---
status: open
opened: 2026-09-15
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/unclassified
kind: bug
---

# BUG: `doc(action="move")` reports slug-form and path-form citations in one field named for paths, and only one of the two needs re-pointing

## Summary

`doc(action="move")` returns `inbound_path_citations` so a caller can re-point them **in the
same commit** — that is the field's whole purpose. Since `c57e9f7272b36f3f` fixed its
false-negative half it also matches **slug-form** citations, which is correct detection and
the right fix. What was not added is the discriminator: both forms arrive in one array under
a name that states one of them, and they take **opposite** actions.

- a **path** citation (`docs/issues/<dated-stem>.md`) is now dead → re-point it.
- a **slug** citation (the bare dateless stem, the house form of every `**Members:**` line in
  `docs/trackers/issue-clusters/`) is **unaffected by the move** → touching it is wrong.

So the caller must re-derive, per entry, which kind it is — by running the grep the field
just ran. The field's own output cannot answer the question it exists to answer.

## Symptom (Effect)

Measured 2026-09-15 across three consecutive archive-moves in one session, plus one the day
before. **Every IC-file entry the field reported was slug-only; no move produced a path-form
IC citation at all.**

| move | reported | genuinely dead | slug-only (no edit owed) |
|---|---|---|---|
| `2a97a4faddc0eace` → archive | 1 | 0 | `IC-3-declared-not-wired.md` |
| `fc08bf52ac6b478d` → archive | 5 | 4 (`src/tools/grep.rs`, `output_buffer.rs`, `read_file.rs`, `tests/buffer_stream_policy.rs`) | `IC-6-addressing-without-an-escape-hatch.md` |
| `6c821e05be227a40` → archive | 1 | 0 | `IC-5-repro-env-diverges-from-gate-env.md` |
| (2026-09-14, `IC-13`) | — | — | same shape, recorded in `bug-fix-session-log` |

The `fc08bf52ac6b478d` row is the one that matters: **one call mixed four real path citations
with one slug citation**, so "this field is now all slugs" is not available as a shortcut
either. The forms interleave.

## Reproduction

Archive any bug file whose slug is cited by an `IC-N` members file:

```
doc(action="move", id=<bug id>,
    new_rel_path="docs/issues/archive/<same stem>.md")
```

Read `inbound_path_citations`. Then, for each entry:

```bash
grep -o "docs/issues/[a-z0-9/-]*<stem>[a-z0-9.-]*" <entry>   # path form — dead, re-point
grep -c "<dateless-slug>" <entry>                            # slug form — untouched, leave
```

The second command is the one the caller should not have had to write.

## Root cause

Not read. The field is named and typed for one form and now carries two; the response has no
per-entry kind. Adjacent code is `doc(action="move")`'s citation scan — the same scan
`c57e9f7272b36f3f` widened.

## Cost, and why it is worse than make-work

Re-deriving three entries by hand is cheap. The expensive branch is a caller who trusts the
field name and **edits** the IC file — converting a slug citation into a path citation. That
is the exact form `c57e9f7272b36f3f` records the scan as having been blind to, so the edit
would silently re-open the bug that fix closed, in the ledger that fix names as the whole
affected population: *"Every `**Members:**` line in `docs/trackers/issue-clusters/` cites its
members by slug — that is the ledger's house form."*

A field whose remedy is "re-point these" and whose contents are half un-re-pointable is a
guard whose REMEDY TEXT is wrong for half its output, which this repo already tracks as a
class of its own (`CLAUDE.md` § *Testing Discipline*, the remedy-text law).

## Fix

Not attempted. The shape that would close it is a per-entry kind rather than a second array —
a second array re-splits a population the caller then has to re-union for the "did I get them
all" check. Something like `[{"path": …, "form": "path"|"slug"}]`, with the existing field
kept as the union so no caller breaks.

## Classification

Filed under `cluster/unclassified` after weighing three, none of which fit without forcing:

- **`IC-18` `selector-narrower-than-its-population`** — inverted. There a selector sees less
  than its name claims; here it sees *more*, and correctly. The zero-reads-as-absent tell does
  not apply: nothing is missing from this output.
- **`IC-20` `floor-published-under-the-name-of-a-total`** — nearest on the remedy axis (rename
  the quantity, or refuse to print it), and still wrong on the claim: IC-20's true value is
  **unknowable** because a walk stopped. Here every value is present and exactly right; only
  the *kind* is unpublished.
- **`IC-21` `instrument-omits-the-dimension-that-grows`** — the omitted dimension here is
  **form**, not magnitude, and the decision does not turn on how many there are.

**If a second instance appears, the candidate class is "a widened detector publishes its new
population under the old one's name, so the caller cannot tell which remedy applies."** The
blind party is the author of the widening fix, for whom both forms *are* citations — which is
the correct reading for detection and the wrong one for repair.
