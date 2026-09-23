---
id: f629b54178a330d6
kind: bug
status: fixed
title: 'BUG: the fix-anchor check reports a section ABSENT when it means UNPARSEABLE'
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doctor
topic: bug-ledger integrity checks
closed: 2026-09-15
---

# BUG: the fix-anchor check reports a section ABSENT when it means UNPARSEABLE

## Summary

`doctor`'s `terminal_status_without_fix_anchor` says *"no `## Fix provenance` pointer is declared"*.
It fires identically when the section does not exist and when it exists, carries both correct
hashes, and is written as a sentence. The check parses `- **SHA:**` / `- **patch-id:**` as structured
bullet fields; prose under the right heading satisfies nothing, and nothing in the message says so.

## Symptom (Effect)

Measured 2026-09-13 on `docs/issues/archive/2026-09-10-the-references-manual-page-teaches-name_path-a-parameter-the-tool-has-never-accepted.md`.
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

**IMPLEMENTED 2026-09-15 (`da5c1f8c`).** Two branches, not a new parser — the shape this
section prescribed.

`declares_fix_provenance_heading` answers the question `structured_fix_pointers` never asks.
That helper sweeps the whole file for the two bullet forms and **never looks at headings**, so
"no section was written" and "a section was written and nothing in it parses" were different
states producing one identical sentence. Any heading LEVEL counts: level is irrelevant to
whether the pointers parse, so refusing a `### Fix provenance` would send its author to fix the
one thing that was never wrong.

The new message names what is actually checked — two labelled bullets, outside any fence — and
says plainly that prose naming the same two hashes satisfies nothing.

**The decoy sentence is deliberately EXCLUDED from the new branch, and that is correctness
rather than brevity.** It asserts the hashes are probably the commit the bug was OBSERVED at:
measured, and right for a file with no section. In the prose case the hashes inside a provenance
section are most likely the **real fix**, merely unparsed — so attaching that explanation names
a cause that does not apply, which is the confident wrong answer this module refuses elsewhere.
The mutation run below shows the old message doing exactly that to a real fix pair.
## Tests added

`terminal_status_without_fix_anchor_distinguishes_unparseable_from_absent`
(`src/librarian/tools/doctor.rs`), added 2026-09-15 with `da5c1f8c`.

**It asserts on MESSAGE CONTENT, not on whether the check fires — this section's earlier note
called that exactly right.** The check fires on the prose fixture in BOTH worlds, before and
after the fix, because prose still parses to nothing and the record still owes an anchor. A test
counting findings is therefore monotone under the defect and green either way. What changed is
*which of the two states the message names*, so that is what the assertions read.

The no-section record is the control: without it, a message that said "unparseable"
unconditionally would satisfy every assertion in the test.

**Mutation-verified.** Making the heading match impossible returns `KILLED` (rc=101, **7 tests
ran**, 6 passed / 1 failed — the count rules out a compile failure wearing the verdict). The
panic prints the pre-fix message verbatim, including the decoy sentence misattributing
`344aff6e2e28` and `2ab7a09a05ea` as *"the commit the bug was OBSERVED at"* — those two are the
real fix pair for a different bug closed the same night, which is the excluded-decoy argument
demonstrating itself.

**Lean-lane absence is correct here and must not be read as coverage.** The test is librarian
code, and `--no-default-features` switches the librarian off; `CLAUDE.md` § *Development
Commands* states that lane is **vacuous** for librarian work, so its `LEAN_RC=0` is a suite that
never compiled the code under test. Verified the way that section prescribes — by reading this
test's own name out of the DEFAULT lane (line 5796, against `LEAN_RC` at 4139 and `DEFAULT_RC`
at 10381), never a lane total. Gate otherwise green: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, 0 failed.
## Workarounds

Copy the bullet form from any archived record.

## Resume

**Nothing on this defect — fixed, mutation-verified, closed.**

**The sibling advice this section used to carry is WITHDRAWN, and the correction is worth more
than the original.** It read: *"check the sibling `non_terminal_status_with_fix_anchor` — it has
the inverse defect (reads any prose patch-id as a claim), and the two become consistent if a
declared anchor is defined structurally in one place and both checks read that definition."*
Checked at the bytes 2026-09-15:

- It is **not a defect.** That check uses `declared_patch_ids`, which is *deliberately* looser,
  and its doc comment carries the measurement: **2026-09-02, 0 of the live `docs/issues/*.md`
  corpus used the structured form.** Every live record wrote prose, so a check keyed on bullets
  alone would be "precise and reachable by no in-tree author" — decoration, by this repo's own
  loudness law.
- The proposed unification would therefore **break** it. Defining a declared anchor structurally
  in one place and pointing both checks at it makes the sibling unreachable, which is the exact
  outcome its looseness was written to avoid. Not free, and it needs that 2026-09-02 figure
  re-derived before anyone tries.

**What is real is the finding underneath the wrong prescription, and it is this record's own
cause.** One corpus carries **two grammars for one concept**: the same prose provenance section
reads as *a declared patch-id* to `non_terminal_status_with_fix_anchor` and as *no pointer
declared* to `terminal_status_without_fix_anchor`. Both checks are right about themselves. That
disagreement is what cost the original author three repair attempts, and it survives this fix —
the message is now honest about which grammar IT wants, which is as far as a message can go.
Whoever takes the unification owes the re-derivation above first.
## Fix provenance

- **SHA:** `da5c1f8c` (experiments) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `f360413843941ed468be06e3089762b188808f4e` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id.
## References

- Inverse defect, same day, same mechanism:
  `docs/issues/archive/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md`
- CLAUDE.md § *Testing Discipline* — a suite tests a guard's PREDICATE and never its REMEDY TEXT;
  arrival at the right addressee buys nothing if the instruction is unanswerable as written. Here
  the remedy text names the thing the reader has already done.
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
