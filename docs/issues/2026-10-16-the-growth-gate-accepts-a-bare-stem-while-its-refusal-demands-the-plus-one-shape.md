---
id: bb2cec400c3f8020
kind: bug
status: open
title: 'BUG: the growth gate accepts a bare stem and a trailing space while its refusal text says it demands `+1: <slug>` and a derivation'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-10-16
owner: marius
related: []
severity: low
---

# BUG: the growth gate accepts a bare stem and a trailing space while its refusal text says it demands `+1: <slug>` and a derivation

## Summary

`CHECK 3` of `scripts/pre-commit-ledger-counts.py` refuses a commit when a defect class gains a
member and its `**Members:**` field does not "name" the member. Two places say what "name" means.
The refusal text (`:906-908`) says *"The ledger's own shape is `+1: `<slug-without-the-date>``
followed by the derivation. This is deliberately NOT `did the line change` -- a trailing space
would satisfy that."* The comment above the check (`:1338-1342`) says *"Whitespace cannot satisfy
that."*

The code (`:1365`) is `if changed and (not stems or any(st in line for st in stems))`. It accepts
any change to the line, provided the dateless stem occurs anywhere in the line as a substring.
No `+1:` marker, no backticks and no derivation are checked. Reproduced 2026-10-16 in a throwaway
repo: a bare stem passes, a stem inside a longer word passes, and one trailing space passes when
the stem is already on the line.

## Symptom (Effect)

Nothing fails. The gate is more permissive than its own message. A filer who follows the stated
shape is accepted, and so is a filer who writes only the stem. The refusal text and the comment
describe a stricter check than the one that runs.

Observed result of the five cases in § Reproduction:

| case | verdict |
|---|---|
| control: line not changed | REFUSED (correct) |
| `+1: `<stem>` — derivation` (the stated shape) | accepted |
| bare stem, no backticks, no `+1`, no derivation | accepted |
| stem as a substring of a longer word | accepted |
| stem already on the line, one trailing space added | accepted |

## Reproduction

Measured 2026-10-16 at `fa7f9439`, n=1 run of each case. A throwaway repo is built per case,
modelled on `newrepo()` in `tests/pre-commit-ledger-divergence.sh`. Two fixture facts matter:
`CHECK 6` refuses unless more than 10 Index rows parse (`len(index_rows) <= 10`), so the repo
needs 11 classes; and slugs must be letters and `-` only (see § Found while reproducing).

```bash
# one target class `alpha` + ten filler classes `fill-b`..`fill-k`; roster rows are `| IC-N | a demo class | `slug` | not yet |`;
# each class file is "# IC-N — demo\n\n**Slug:** `cluster/<slug>`\n\n**Members:** <line>\n"
# copy scripts/pre-commit-ledger-counts.py (and scripts/commit-sequence-tail.txt) into the repo; commit the corpus
# stage a NEW open bug file docs/issues/2026-10-16-a-new-bug.md tagged `cluster/alpha`   (stem: a-new-bug)
# then, per case, replace alpha's **Members:** line, `git add` it, and run:
python3 scripts/pre-commit-ledger-counts.py ; echo $?
#   HEAD line:  "**Members:** none yet."                 new line:  (unchanged)                                       -> exit 1
#   HEAD line:  "**Members:** none yet."                 new line:  "**Members:** none yet. +1: `a-new-bug` — why."  -> exit 0
#   HEAD line:  "**Members:** none yet."                 new line:  "**Members:** none yet. a-new-bug"                -> exit 0
#   HEAD line:  "**Members:** none yet."                 new line:  "**Members:** none yet. See a-new-bug-somewhere." -> exit 0
#   HEAD line:  "**Members:** see a-new-bug already."    new line:  "**Members:** see a-new-bug already. "           -> exit 0
```

The refusal for the control case reads: *"a class gained a member and its `**Members:**` does
not name it: cluster/alpha -- the field is in `docs/trackers/issue-clusters/IC-1-alpha.md`
expected it to change and to contain one of: `a-new-bug`"*. So the check does tell the filer which
stem it looks for. It does not look for the shape.

## Environment

`codescout` at `fa7f9439`, branch `experiments`, Linux, python3 from the pyenv shim. The check
reads the git index (`git show :path`), so the cases above stage both files.

## Root cause

The code tests presence of a substring and the text promises a shape. The two were written in the
same commit, `1b3ac36b` (2026-09-02, `refactor(clusters): derive the class counts, stop storing
them`), which replaced the count gate. The comment at `:1334-1342` records why *"did the line
change?"* was rejected: it is `cluster/assertion-satisfiable-by-accident`, because a trailing
space satisfies it. The shipped check narrows that hole for a **new** stem, which must appear. It
does not close it:

- A stem that is already on the line (cited by an earlier entry, or present before a retag)
  needs only *a* change, so one trailing space passes.
- Any occurrence of the stem passes, whatever surrounds it.
- Nothing requires text after the stem, so nothing requires a derivation.

## Class fit

`cluster/guard-narrower-than-its-name`. The guard's name and message promise a derivation of why
the instance belongs to the class. The implementation covers a subset of that property: the stem
occurs. The green result conceals the gap, because a filer who writes only the stem sees the same
exit 0 as a filer who writes the derivation.

Boundary against `cluster/assertion-that-cannot-fail`: this gate **can** fail. The control case
is refused, and so is a new stem that is absent. It is not zero coverage; it is partial coverage
under a stronger name. Boundary against `cluster/assertion-satisfiable-by-accident`: that class
turns on environment-controlled text in the haystack. Here the haystack is the filer's own line.

## Evidence

- The reproduction table above, run against the shipped script.
- The field this gate guards shows the looseness in practice. The `IC-18` `**Members:**` field
  holds at least three entry shapes, which a gate enforcing one shape would not allow: dated
  `**+1, <date>:**` entries, `**+1: `<slug>` (<date>)**` entries, and a `**+5, <date> (...)**`
  batch entry. Read at the head of the field and by counting markers on 2026-10-16.
- **Not established:** how many existing entries lack a derivation. The same-day measurement
  (61 of the class's 69 tagged members are named on the `IC-18` line) shows *naming*, not content.
  I did not classify entries as derived or bare, so the practical cost of the looseness is unknown.

## Found while reproducing

`valid_slugs` (`:365-377`) accepts a slug only if every character is a lowercase ASCII letter or
`-`. A `**Slug:**` declaration whose slug contains a digit is dropped without a message. My first
fixture used `fill-2`, and the hook reported *"only 1 Index row(s) parsed"* for an Index of 11
rows. Checked 2026-10-16: 25 declared slugs, none contains a digit, so this has no live impact.
Not filed separately; recorded here so a future slug such as `ipv6-...` does not meet it blind.

## Hypotheses tried

**H1: the `+1:` shape is advisory and the substring check is the intended rule.** Contradicted by
the text. The refusal says *"deliberately NOT `did the line change`"* and the comment says
*"Whitespace cannot satisfy that"*. An advisory shape would not be defended in those words. Not
refuted for the author's intent, which this record cannot read; refuted as a description of what
the code does.

## Fix

Not started, and it may not be needed. Decide the direction first:
`docs/adrs/2026-10-16-the-reasoning-for-a-class-member-lives-in-the-members-own-bug-file.md`
proposes moving the check off the shared `**Members:**` line into a `## Class fit` section of the
member's own bug file, which replaces `CHECK 3` rather than tightening it. If that ADR is
accepted, tightening the substring test is wasted work. If it is rejected, the smallest repair is
to require the stem in backticks after a `+1` marker, followed by a minimum amount of text. Any
presence test is satisfiable by a placeholder, so whichever form lands needs a mutation test
that an empty derivation is refused.

## Tests added

None. The record is open, and no fix exists. N/A is justified only until a fix lands.

## Workarounds

Follow the stated shape by hand. The gate does not require it, so nothing reminds a filer who
forgets.

## Resume

1. Read the ADR named under § Fix and settle it first.
2. If the ADR is rejected, add the five cases of § Reproduction as a fixture beside
   `tests/pre-commit-ledger-divergence.sh` and make the three accepted-but-weak cases refuse.
3. Re-run § Reproduction against HEAD before acting: the script and this record can both have moved.

## References

- `scripts/pre-commit-ledger-counts.py:906-908` (refusal text), `:1334-1342` (comment), `:1365` (the test), `:365-377` (`valid_slugs`)
- `1b3ac36b` — introduced the substring test and the text together
- `docs/trackers/cluster-promotion-session-log.md` `F-9` — the same session's measurements of the `**Members:**` field
- `docs/issues/archive/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md` — the sibling coupling this gate sits in
