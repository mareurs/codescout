---
kind: bug
status: fixed
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-09-15
opened: 2026-09-15
owner: marius
related: []
severity: medium
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

**SHIPPED `e79fa902`**, patch-id `d63380f9840374e5291d2ca0704d461a933f434c`.

The response grows two fields, and the information needed no new work — `mv.rs` already
scanned the dated stem and the dateless slug separately, then `extend`ed and `dedup`ed. The
fix only stops discarding which scan matched. No extra `git` invocation, no match text, no
second cap.

| field | meaning |
|---|---|
| `inbound_citations_cleared` | the subset of the **same capped entries** that cannot cite the old path |
| `citation_stem_preserved` | `false` ⇒ nothing is cleared, and why |

**The flag is an OBSERVATION, not a verdict, and that is the design.** `files_mentioning`
deliberately over-reports — its own doc comment says so — so it cannot honestly answer *"must
this be re-pointed?"*. It can answer *"did the dated stem appear in this file"*, and that is
sound in exactly one direction: every citation of `docs/issues/<dated>.md` contains that
substring, so a file not mentioning it **cannot** hold one. The complement stays ambiguous on
purpose — a bare dated stem survives a move untouched, since a move changes only the
directory.

### Two things this file got wrong, both found by scouting before implementing

**The plan said *"per-entry kind: path vs slug"*. That is not the distinction.** A bare dated
stem carries no path wrapper and still survives the move, so "form of the citation" does not
decide the remedy. The honest datum is which **needle** matched.

**The plan rejected a second array for the wrong reason.** It argued the caller would have to
re-union the two lists. The real hazard is narrower and worse: two independently
`.take(CITATION_SAMPLE)`-ed lists cannot be subtracted **at all** — that is `IC-13` built into
the seam rather than merely risked. Fixed by capping **once** and projecting both renderings
from the same vector, so they correspond by construction.

### A fourth condition, found by naming the claim rather than by reading code

*"Unaffected by this move"* holds only because the slug survives it — and `new_rel_path` is
arbitrary, so a move that **renames** the stem breaks slug citations exactly as hard as path
ones. Without `citation_stem_preserved` the field would clear an `IC-N` Members line that a
rename had just killed: a confident wrong answer in the direction that **loses data**, which
is strictly worse than the make-work this bug was filed about.

### NOT LIVE IN THE SESSION THAT SHIPPED IT

The running MCP binary was built 2026-09-15 07:04:55; this commit landed 08:27:23. So the
archive move for **this very file** returned the OLD response shape, with neither new field —
recorded because it is the obvious thing to claim and it would have been false. `cargo rb`
plus `/mcp` is what makes it live; CI tests it regardless of any session's binary.

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
