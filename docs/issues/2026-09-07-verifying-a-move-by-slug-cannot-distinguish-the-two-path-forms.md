---
id: b8c0716a0fd4c17d
kind: bug
status: open
title: 'BUG: verifying an archive move by grepping the slug cannot separate fixed citations from broken ones, because the slug is a substring of both path forms'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- trackers
- shared-checkout
- measurement
topic: verifying a citation sweep, and the pattern that cannot answer it
---

## Summary

The archive convention says to sweep citations after `doc(action="move")` and gives a `grep -rn
'docs/issues/<slug>.md'` recipe. The natural way to **verify** the sweep afterwards is to grep the
**slug** — and that pattern cannot answer the question, because the slug is a substring of *both*
path forms:

```
docs/issues/2026-09-01-<slug>.md            <- the dangling form
docs/issues/archive/2026-09-01-<slug>.md    <- the repaired form
```

So a slug grep returns the same hit for a citation that was fixed and one that was not. Which
direction it fails in depends only on how the reader interprets the count, and **both directions
were observed on 2026-09-07 within about ninety seconds of each other**:

- reading the hits as **still broken** → a completed sweep is reported as 5 dangling refs;
- reading the hits as **already fixed** → an unswept corpus is reported as clean.

The discriminating pattern is the full old path with `docs/issues/` as a literal prefix, which
**cannot** match `docs/issues/archive/…`. It is not a better regex, it is a different question.

## Symptom (Effect)

Measured at HEAD after `b5139fc0`, with the control that makes the zero mean anything:

```
$ git grep -n 'docs/issues/2026-09-01-<slug>.md' HEAD -- docs/          # old form only
exit=1  lines=0

$ git grep -n 'docs/issues/archive/2026-09-01-<slug>.md' HEAD -- docs/  # CONTROL
lines=7
```

**The control is load-bearing and is the part that gets skipped.** Without it, `0` is
indistinguishable from a pattern that matches nothing because it is malformed — and a malformed
pattern is exactly what produced two of the instances below.

## Root cause

Not a defect in any tool. It is a property of the **namespace**: an archive move keeps the
filename and changes only the directory, so the slug — the part everyone reaches for, because it
is the memorable half — is invariant under precisely the change being verified.

The convention's own sweep recipe uses the full path, correctly. Nothing tells the reader that the
**verification** step needs the same discipline, and verification is where the shortcut is
tempting because you are checking rather than editing.

## Evidence — five instances, one day, four sessions

| # | session | direction | outcome |
|---|---|---|---|
| 1 | `4eac25ba` | headline vs enumeration | 7-across-5 counted, 6-across-4 listed; `IC-17` in the grep output, absent from the message |
| 2 | `cda3afe5` | read worktree as corpus | reported three correct citations as already-fixed, and a peer's correct list as an error |
| 3 | *(the sweeper)* | — | staged the sweep before the move, briefly leaving the unsafe half |
| 4 | `8dba66b0` | slug read as old-path | post-commit verification reported **5 dangling refs** against a corpus with **0** |
| 5 | `4eac25ba` | slug read as old-path | same error as #4, committed **in the same turn as reading #4's account of it** |

Instance 5 is the one that decides this is a mechanism rather than carelessness: the warning was
in the message being replied to, and knowing the class prevented nothing. What caught it was the
**control**, not the knowledge.

Instances 1 and 2 are a different fault — a **stale snapshot** — and are separated here on purpose:
both readings were correct at their own instant, and the tell that a corpus moved rather than a
reader erred was **line drift** (`:277/:311` against `:278/:340`) between two otherwise-agreeing
readings. Neither party read it as one at the time.


### Sixth instance, 2026-09-08 — four days of residue, and the reason the gate did not stop it

Found by `59112612` while checking that its *own* new citations resolved — not by looking for
this. The `2026-09-04` archive of
`artifact-vector-delete-has-no-production-caller-so-every-archive-strands-its-vectors.md` left
**three** dangling citations, still live four days later at `f8523cc1`:

```
src/librarian/indexer.rs:55
src/librarian/tools/mv.rs:157
src/retrieval/artifact.rs:156
```

Repaired in the same commit as this note, verified in this file's own prescribed form — old
path **0**, archive-form control **3 non-zero**, target present on disk.

**This sharpens § *Tests added*, which currently reads that `audit_doc_refs` "already covers
the outcome". Measured, that is true at the TOOL level and false at the REPORTING level, for
two independent reasons.** I first assumed the tool simply does not scan `.rs` and checked
before writing it down; the assumption was wrong, and the real mechanism is more specific:

- **Severity cap.** Narrowed to the two files, `audit_doc_refs` finds all of them and
  classifies each `verdict: "missing"`, `severity: "med"`, `severity_reason:
  **`code_comment_capped`**`. CI fails on `high`. So an archive-move breakage **in a code
  comment can never red the gate by construction**, while `CLAUDE.md` § *Parsers Over a
  Namespace* records that the same breakage in ordinary markdown backticks gets
  `policy_default` = `high` and does. The residue therefore accumulates specifically in
  `.rs`, silently and without bound — which is why these three survived four days in three
  separate files.
- **Findings cap.** A project-wide run reports `n_refs_broken` **14637** against a
  `findings` array capped at **50**, ordered most-severe-first. Even without the severity
  cap, a specific new breakage is not findable in that output without already knowing the
  path to narrow to — i.e. without already having the answer.

**So the honest correction is not that the reader's confirmation step failed here — it is
that this class has a residue the gate is DESIGNED not to fail on, and nothing counts it.**
The cheap instrument is the one used above: `git grep -F '<old full path>' -- 'src/*'` paired
with the archive-form control, which needs no severity policy at all. Whether
`code_comment_capped` should stay capped is a real question and belongs to whoever owns that
policy — a comment is lower-stakes than prose, so the cap is defensible; what is not
defensible is that the capped population is **unmeasured**.
## Class

Tagged `cluster/selector-narrower-than-its-population` (`IC-18`) as the closest member of the
closed set, **and it is the MIRROR direction**, which the cluster owner should rule on rather than
inherit from this tag. `IC-18`'s claim is a selector *narrower* than the population it names —
members it never saw cannot be counted. Here the selector is **wider**: it sees members it should
not, and the surplus is indistinguishable from the target. The harm is the same shape (a count
that answers a different question than the one asked) and the direction is opposite, so either
`IC-18` widens to *"selector and population disagree"* or this earns a sibling.

## Fix

Not fixed; the remedy is a documented practice, not code.

- **Verify with the full old path, never the slug.** `docs/issues/<name>.md` as a literal prefix
  cannot match the archive form. This is the only pattern that answers *"is anything still
  dangling?"*.
- **Always pair it with the archive-form control.** A bare `0` from a path grep and a `0` from a
  typo are the same bytes. The control must return non-zero or the measurement is void.
- **Stamp the reading.** A citation list is valid only at its instant, exactly as a peer count is
  — `CLAUDE.md` § *Reaching a Peer Session* already states the rule for peers and names the cost:
  ordinary churn presents as a tooling defect and the next step is to go debug a working
  instrument. That is instances 1 and 2, one population over.
- **Emit the list, then count the list — never the corpus.** Instance 1 is a headline derived from
  the tool and an enumeration derived from the eye, with nothing reconciling them.

Where this belongs is `get_guide("tracker-conventions")` § *Bug files*, beside the existing sweep
recipe, since that is the surface a session archiving a bug actually opens.

## Tests added

None, and a test is the wrong instrument: this is a defect in how a human-or-agent verifies, not
in a code path. The mechanical half that *could* be checked already exists —
`librarian(action="audit_doc_refs")` resolves path-shaped tokens against the filesystem and would
catch a genuine dangling ref regardless of which pattern a reader used.

**So the honest statement is that the gate already covers the outcome, and what failed five times
is the reader's own confirmation step** — which is why this is a convention note rather than a
guard.

## Workarounds

`librarian(action="audit_doc_refs")` for the authoritative answer. The greps above for a cheap
local check, with the control.

## References

- `get_guide("tracker-conventions")` § *Bug files* — the sweep recipe, which correctly uses the
  full path.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class, and
  the mirror-direction question this file raises.
- `b5139fc0` — the move whose verification produced instances 4 and 5.
- Instances 2 and 4 reported by sessionIds `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b` and
  `8dba66b0-af4b-4cda-a333-54a0605b318e`, each against themselves. Filed by
  `4eac25ba-b181-4dac-a5a1-ec88502a5bc5`, who committed instance 5.
