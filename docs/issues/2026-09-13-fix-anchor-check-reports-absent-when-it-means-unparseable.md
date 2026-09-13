---
id: '76bdf3c09a70816a'
kind: bug
status: open
title: 'BUG: the fix-anchor check reports a section ABSENT when it means UNPARSEABLE'
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doctor
topic: bug-ledger integrity checks
---

# BUG: the fix-anchor check reports a section ABSENT when it means UNPARSEABLE

## Summary

`doctor`'s `terminal_status_without_fix_anchor` says *"no `## Fix provenance` pointer is declared"*.
It fires identically when the section does not exist and when it exists, carries both correct
hashes, and is written as a sentence. The check parses `- **SHA:**` / `- **patch-id:**` as structured
bullet fields; prose under the right heading satisfies nothing, and nothing in the message says so.

## Symptom (Effect)

Measured 2026-09-13 on `docs/issues/2026-09-10-the-references-manual-page-teaches-name_path-a-parameter-the-tool-has-never-accepted.md`.
Three repair attempts, each verified against the check's own count rather than assumed:

| attempt | what was written | `terminal_status_without_fix_anchor` |
|---|---|---|
| 1 | `closed: 2026-09-12` in frontmatter | 1 → 1 |
| 2 | a real `## Fix provenance` section, both hashes, in prose | 1 → 1 |
| 3 | the same facts as `- **SHA:**` / `- **patch-id:**` bullets | 1 → **0** |

After attempt 2 the file contained the heading verbatim, at `## ` level, preceded by a blank line,
with both the SHA and the patch-id — and the report still read "no pointer is declared".

## Reproduction

Take any `status: fixed` bug record and add:

```markdown
## Fix provenance

Fixed on `experiments` — SHA `<40-hex>`, patch-id `<40-hex>`.
```

Run `librarian(action="doctor")`. The finding persists, unchanged, naming the section you just
added. Replace the sentence with two bullets and it clears. A `librarian(action="reindex")` between
attempts changes nothing — this is not a stale-snapshot effect; it was ruled out by running one.

## Environment

codescout `experiments` @ `741412a4`. `librarian(action="doctor")`, project scope.

## Root cause

The parser wants structured fields. The message describes the failure at the wrong level of the
grammar: it reports on the **section's existence**, which is not what it tested. What it tested is
whether two labelled bullets parse.

The phrase *"Record both lines — the SHA, and the patch-id"* is, in hindsight, the specification —
*both lines* is literal and means two bullet lines, not two facts. It reads as prose guidance.

## Evidence

The corpus is unambiguous once looked at: 115 archived records use `## Fix provenance`, and the ones
that satisfy the check use the bullet form, e.g.
`docs/issues/archive/2026-08-07-edit-code-remove-ast-repair-over-deletes.md:256`:

```markdown
## Fix provenance

- **SHA:** `c551f19b`
- **patch-id:** `ce19b400eb39d09f7c9cd4ed9a2ec8220ac5d31f`
```

Nothing states this is required rather than customary, so an author with the facts and no example
open writes the sentence.

## Hypotheses tried

1. **Stale catalog body snapshot.** **Refuted** — `librarian(action="reindex")` reported
   `updated: 2` and the finding survived it.
2. **The frontmatter `closed:` date is the anchor.** **Refuted** by attempt 1; the date is correct
   and necessary for other reasons, and moves this count not at all.
3. **The heading needed a preceding blank line.** **Refuted** — it had one.

## Fix

Not implemented. The cheap and correct repair is entirely in the message: when the heading is
present but unparsed, say so and show the expected shape.

> `## Fix provenance` is present but declares no parsed fields. Expected two bullets:
> `- **SHA:** \`<sha>\`` and `- **patch-id:** \`<id>\``. Found prose.

That is a two-branch message, not a new parser.

## Tests added

None. Note the shape the regression test must take: asserting *the check fires on a file with no
section* is monotone under the defect — it passes whether or not the prose case is handled. The
discriminating fixture is **heading present, prose body**, asserting the message names
unparseability rather than absence.

## Workarounds

Copy the bullet form from any archived record.

## Resume

Implement the two-branch message. While there, check the sibling
`non_terminal_status_with_fix_anchor` — it has the inverse defect (reads any prose patch-id as a
claim), and the two become consistent if a declared anchor is defined structurally in one place and
both checks read that definition.

## References

- Inverse defect, same day, same mechanism:
  `docs/issues/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md`
- CLAUDE.md § *Testing Discipline* — a suite tests a guard's PREDICATE and never its REMEDY TEXT;
  arrival at the right addressee buys nothing if the instruction is unanswerable as written. Here
  the remedy text names the thing the reader has already done.
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
