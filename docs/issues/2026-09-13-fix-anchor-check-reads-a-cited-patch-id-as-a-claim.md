---
id: '8713b680435c878a'
kind: bug
status: open
title: 'BUG: the fix-anchor check reads a patch-id a record MENTIONS as one it CLAIMS'
tags:
- cluster/addressing-without-an-escape-hatch
- librarian
- doctor
topic: bug-ledger integrity checks
---

# BUG: the fix-anchor check reads a patch-id a record MENTIONS as one it CLAIMS

## Summary

`librarian(action="doctor")`'s `non_terminal_status_with_fix_anchor` flags a non-terminal bug whose
body "declares" a patch-id, on the reasoning that a patch-id is produced only by
`git show <sha> | git patch-id --stable`, i.e. by closing the bug. It has no way to distinguish a
patch-id the record claims as **its own fix anchor** from one it **cites as another bug's**. Both are
a 40-hex token in prose.

## Symptom (Effect)

Measured 2026-09-13. `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
is reported as:

> status is `open` but the body declares 1 patch-id(s) — `0e7feedf232c5ed9e22fd975c6fe36baa109e1d2` —
> which is produced only by [...] closing this bug. The queryable field and the prose disagree and only
> the prose is true, so every triage query hands this out as unstarted work.

Every clause of that is false for this record. The record is correctly `open`.

## Reproduction

Read the cited line rather than the finding:

```
grep -n '0e7feedf' docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md
```

Line 305-306 reads, in full context:

> **Both of the guards they proposed were then MEASURED AND REJECTED, and what shipped is neither.**
> Superseded by `docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md`
> (`d5af3d3ceff1d08c`), fixed at `74b9cc67`, patch-id `0e7feedf232c5ed9e22fd975c6fe36baa109e1d2`.

The patch-id belongs to `d5af3d3ceff1d08c`, a different bug that supersedes *part* of this one. This
record's own `## Resume` holds a live, undecided choice between two fix directions with a third
explicitly falsified — it is open in the fullest sense.

## Environment

codescout `experiments` @ `d3a2c24f`. `librarian(action="doctor")`, project scope.

## Root cause

The check scans the body for patch-id-shaped tokens and treats presence as a claim. A patch-id has no
syntax distinguishing *use* from *mention*, and the surrounding prose — which does carry the
distinction, unambiguously, for a human — is not consulted.

This is `cluster/addressing-without-an-escape-hatch` exactly: a scheme that interprets every token in
its namespace, with no way to write one as a mention. The defect is not any input it accepts; it is
that "cite a neighbour's fix anchor" is unrepresentable.

## Evidence

The check's own remedy text concedes a *different* limitation and, by naming only that one, implies
the rest are handled:

> This is a worklist, not a verdict: the check cannot tell a complete fix from a landed half.

It cannot tell a complete fix from a landed half — true, and stated. It also cannot tell **whose fix
it is reading**, which is not stated, and which is the failure that actually occurred. A reader who
takes the caveat at face value concludes the only risk is over-eager closure of a partial fix, and
flips the status.

## Hypotheses tried

1. **The record is genuinely a landed half** (the stated caveat). **Refuted** — the patch-id resolves
   to another artifact id named on the same line, and `74b9cc67` is that bug's fix commit, not this
   one's.
2. **`unverified:` is the remedy** — the check names it as the discharge for a legitimately-open
   record. **Rejected as dishonest here**: `unverified:` means *the fix is partial*. There is no fix.
   Using it would silence a false positive by asserting something untrue, and would make the record
   read as half-fixed to every future reader.

## Fix

Not yet implemented. Two directions, neither costed:

1. **Require an anchor to be declared structurally**, as `terminal_status_without_fix_anchor` already
   does — a `## Fix provenance` section with `- **SHA:**` / `- **patch-id:**` bullets. A patch-id in
   running prose would then never be read as a claim, because claims have a shape. This is the escape
   the class asks for, and the parser for it already exists in the sibling check.
2. **Consult the sentence.** Cheaper to state, worse to build, and it re-creates the same problem one
   level down.

Direction 1 is preferred and has a pleasing property: the two checks become inverses over one
grammar rather than two parsers disagreeing about what a patch-id means.

## Tests added

None. A regression test wants a fixture whose body cites a foreign patch-id and asserts **no**
finding — note that is an absence assertion, monotone under removal of the check, so it must be
paired with a positive fixture that does claim its own anchor and must fire.

## Workarounds

Read the cited line before acting on this finding. The class it belongs to is the tell: if the hash
sits next to another artifact id or an "superseded by" phrase, it is a mention.

## Resume

Decide between direction 1 and 2 above. Direction 1 additionally requires a migration question
answered: 115 archived records already use the structured form, so the corpus is mostly there
already — count how many non-terminal records carry a prose patch-id before deciding whether the
stricter read is a breaking change or a no-op.

## References

- **`docs/issues/archive/2026-09-02-declared-patch-ids-per-line-scan-misses-a-wrapped-value.md` —
  the SAME function, failing in the opposite direction, and missed on this file's first pass.** It
  names the implementation this bug is about and which this file did not cite:
  `declared_patch_ids`, `src/librarian/tools/doctor.rs:5245-5278`. There, the scan finds `patch-id`
  and then searches **only the remainder of that same line** for the opening backtick, so a
  declaration whose 40-hex value wraps to the next line yields no match — and
  `non_terminal_status_with_fix_anchor` reports a correct-looking **zero** for a record that did
  declare its provenance. **That is a false NEGATIVE; this file is the false POSITIVE**, and the two
  bound one function from opposite sides: it misses declarations that wrap, and counts mentions that
  do not. Anyone fixing either should read both, because a widening that catches the wrapped value
  also catches more mentions. Surfaced by `git grep -il non_terminal_status_with_fix_anchor --
  'docs/issues/archive/*.md'` — one command, not run before this file was opened.
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
- Sibling defect found the same day, same mechanism, opposite direction:
  `docs/issues/2026-09-13-fix-anchor-check-reports-absent-when-it-means-unparseable.md`
- CLAUDE.md § *Parsers Over a Namespace* — "owe an escape and a disambiguator"
- CLAUDE.md § *Testing Discipline* — a document that names one failure mode implies by omission that
  the rest are handled
