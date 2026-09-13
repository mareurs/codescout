---
id: '44ebfc3bd48ceb53'
kind: bug
status: investigating
title: 'BUG: doc(action=move)''s inbound_path_citations cannot see a slug-form citation, and reports [] rather than the form it searched'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: medium
---

## Summary

`doc(action="move")` returns `inbound_path_citations` — the citations it believes point at
the artifact being moved, so a caller can re-point them in the same commit. It resolves
them by grepping for the artifact's **dated file stem** as a literal substring, so a
citation of the **dateless** slug is never matched, and the response reports `[]` rather
than naming the form it searched.

**Read § Root cause, not this paragraph's first draft.** The mechanism was originally given
here as "path form only" and that was refuted by `5ea1f2ad`; the discriminator is the date
prefix, not the path wrapper.

`IC-18` exactly: *"a zero reads as 'not present' rather than 'not looked at'."*

Slug-form citations are not an edge case here. **Every `**Members:**` line in
`docs/trackers/issue-clusters/` cites its members by slug** — that is the ledger's house
form — so the population this field cannot see includes the entire cluster corpus.

## Symptom (Effect)

Two instances, 2026-09-13, both real moves:

| # | session | result | was the zero right? |
|---|---|---|---|
| 1 | `f3c594ce` | `inbound_path_citations: []` while `IC-6`'s Members line cited the file by slug | **No.** Caught only because the slug had been grepped BEFORE the move, for an unrelated reason. |
| 2 | `8bd791df` | `[]`, and no citation was broken | **Yes — by ordering, not by the field.** They wrote the `IC-2` Members entry *after* moving, so it already named the new slug. |

**Instance 2 is the more dangerous of the two and is the reason this is filed.** A broken
instrument that happens to return the correct answer produces no prompt to look again. Had
the ordering been reversed — Members line first, as in instance 1 — the same `[]` would
have been false and nothing would have marked it.

## Reproduction

Move an artifact that is cited by slug rather than by path:

```
doc(action="move", id="<id>", new_rel_path="docs/issues/archive/<same-name>.md")
-> inbound_path_citations: []      # while `grep -F '<slug>'` returns the citing line
```

Confirmed against `70e16d65`, where the citing line was `IC-6`'s `**Members:**`.

## Environment

`experiments`, 2026-09-13. Independent of the archive/rename distinction — instance 1 was
a move that also renamed, instance 2 a move that did not.

## Root cause

**ESTABLISHED 2026-09-13, and it is neither of the two mechanisms proposed so far.**

`files_mentioning` (`src/librarian/tools/mv.rs`) runs
`git grep --untracked -l -F -e <stem>`, where `stem` is the artifact's **file stem**. Its
own doc comment states the assumption that fails: *"Stems in this corpus are **dated
slugs** and effectively unique; a short or generic stem would over-report, which is the
safe direction."*

So the selector is a literal substring search for the **dated** stem:

```
stem searched     2026-09-12-audit-doc-refs-reads-a-backtick-inside-a-fence-as-syntax
IC-6 cites        `audit-doc-refs-reads-a-backtick-inside-a-fence-as-syntax` (2026-09-12)
                   ^^ dateless, with the date OUTSIDE the backticks
```

A `-F` search for the dated stem cannot match a dateless citation. **The discriminator is
the date prefix, not the path wrapper** — a citation of the bare dated stem with no
directory at all IS found, which is exactly what `5ea1f2ad` pins.

**This corpus has two citation conventions and the scan knows one.** Paths and prose cite
the dated form; **every `**Members:**` line under `docs/trackers/issue-clusters/` cites the
dateless slug**, because a cluster member's identity has to survive an archive move. The
repo already knows this: `scripts/pre-commit-ledger-counts.py`'s `_stem()` strips the date
for precisely that reason, and its comment says so — *"the dateless slug — a bug file's
identity ACROSS a rename"*. Two components of one system disagree about what a slug is, and
only one of them is documented as making a choice.

**Correcting this file's own first answer, which was wrong in the same way the bug it came
from was.** § *Root cause* originally read *"the resolver's selector is the path form ...
the direction does not depend on"* the implementation. It does. `eba3d2c6` refuted it with
`5ea1f2ad` and moved this file to `investigating` rather than accepting it, which was the
right call: the symptom was real and the named cause was not. That is the third time in one
day on this chain — the fenced-block bug named a backtick, this file named a path wrapper,
and both times § *Root cause* hedged on the implementation while the § *Summary* asserted
the direction as fact.

**`5ea1f2ad`'s test is correct and under-covers, in the shape `IC-23` was opened for.** It
pins that a citation of the dated stem with no path wrapper is found — true, and never the
failing case. The dimension that decides the outcome is the **date prefix**, and the fixture
holds it constant. A guard that varies the wrapper while fixing the prefix cannot fail in
the direction the bug reports, which is this repo's monotone-direction law arriving at a
test written to refute a bug rather than to catch one.
## Investigation update, 2026-09-13 — the reported form does not reproduce

**Ran the reproduction before reading the fix plan, per this repo's own rule
(`bug-fix-session-log:W-32`), and it changed the picture.** `files_mentioning`
(`src/librarian/tools/mv.rs`) greps for `old_full.file_stem()` as a plain `-F` substring
— and for a dated-slug filename, the file stem **is** the slug. A new test,
`the_citation_scan_sees_a_bare_slug_with_no_path_wrapper`, seeds a citer whose only
mention of the target is a bare slug with no path syntax anywhere (`"**Members:** foo\n"`)
and asserts it is found. **It passes against current source**, unmodified — shipped as
`5ea1f2ad` on `experiments`, no production code touched.

This root cause — called "not established in code; the behaviour is black-box so far" by
this file's own Root cause section — is now partially established, and in the direction
opposite to what was assumed: the resolver is **not** path-form-only. A slug-only
citation with zero path decoration is already visible to it.

**This does not explain instances 7 and 8.** Both are real, observed `[]` results on
real moves where a slug citation existed. What it rules out is the specific mechanism
this file names ("the selector is the path form"). Candidate directions not yet checked:
cross-session timing (the citing file's on-disk content at scan time vs. when a reader
later confirmed the citation existed), the `exclude` parameter matching more broadly than
intended, or a git-grep environment difference (`--untracked` scope, `.gitignore`
interaction) between this reproduction's fresh tempdir and the real checkout's actual
git state at the time. **Left `investigating`, not `fixed` — the symptom is real, the
named mechanism for it is refuted, and the true mechanism is still open.**

## Fix

Not implemented. Two halves, and only the first is code:

1. **Widen or report.** Either resolve slug-form citations too, or name the form that was
   searched so `[]` carries its scope — `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
   is this repo's own ruling on exactly this, and the field predates or ignores it.
2. **The ordering rule, which needs no code and is already recorded.** Move BEFORE writing
   the citation; a Members line written afterwards already names the new path. Contributed
   by `8bd791df` from instance 2 and now in
   `docs/issues/archive/2026-09-07-verifying-a-move-by-slug-cannot-distinguish-the-two-path-forms.md`
   § Fix. It is a **sequence rather than a check**, so it does not require anyone to
   remember the field is unreliable.

**This is testable, which the sibling file's own § Tests added denies of its class.** A
fixture holding one slug-form and one path-form citation, asserting both come back, reds
today. That sibling concluded *"a defect in how a human-or-agent verifies, not in a code
path"* — true of its six reader-side instances, and false here.

## Tests added

None; not fixed. The fixture is named above so whoever picks it up does not have to
rediscover it.

## Resume

Found while archiving the fenced-line-attribution bug, and filed **separately from**
`b8c0716a0fd4c17d` on that ledger's explicit rule: *"if a finding satisfies a second
class's claim, it is a second bug file. The one-tag rule is what makes `n` a partition
rather than a tally."*

It first went in as a paragraph inside that file (`663f7efd`), which was the wrong unit
and carried a wrong conclusion with it — that `IC-18` should widen to cover both
directions. It should not: `docs/trackers/issue-clusters.md` § *The entry shape* already
rules that `IC-6` and `IC-18` are separated **by direction**, `IC-6` matching too much and
`IC-18` too little, and widening would collapse the discriminator. The host file's defect
is the over-match (two path forms that collide under one slug, `IC-6`); this one is the
under-match (`IC-18`). Same namespace ambiguity, two classes, by design.

## References

- `docs/issues/archive/2026-09-07-verifying-a-move-by-slug-cannot-distinguish-the-two-path-forms.md`
  — the mirror direction, and where the ordering rule lives.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the ruling half (1) above
  would satisfy.
- `70e16d65` — the move that produced instance 1.
