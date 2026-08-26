---
id: '0b40a6e83053c1d2'
kind: bug
status: open
title: docs/templates/session-log.md cites codescout's own ledger ids bare, so every copy into another repo imports dangling citations — and one that resolves to the wrong entry
tags:
- librarian
- link-scan
- templates
- session-log
- reconnaissance
- cross-repo
opened: 2026-08-26
owner: marius
related: []
severity: medium
unverified: Only one downstream repo was measured (claude-plugins). The wrong-resolution of `R-1` was confirmed there by elimination from link_scan's edge list, not by a direct per-token resolver dump. Other consuming repos were not surveyed.
---

# BUG: session-log template imports its own repo's citations into every copy

## Summary

`docs/templates/session-log.md` cites codescout's own ledger entries as **bare** tokens
(`R-89`, `R-1 + R-7`, `W-4`, `F-2`/`W-3`). Bare tokens resolve against the *copying* repo,
so every fresh copy imports broken citations — and at least one that resolves **to the
wrong entry, silently**, producing a confidently incorrect `cites` edge rather than a
reported break.

A template is the one document whose citations are always read in a different repo than
the one they were written in. This one was authored in-repo, where they resolved.

## Symptom (Effect)

Copied into `claude-plugins` on 2026-08-26 per the reconnaissance skill's Phase 3
instruction, `link_scan` reported four dangling citations from the fresh file before a
single entry had been written. Three were unrelated (`T-35`, a downstream defect); the
rest, plus several ambiguous, came from template prose.

The sharp one: `R-1` bound to `claude-plugins`' *own* `docs/trackers/reconnaissance-patterns.md`
`R-1` — an unrelated entry about spec testing sections. `link_scan` materialised the edge
without complaint. A dangling citation is reported and fixable; a wrong one is neither.

## Reproduction

```
grep -n '\b[A-Z]\{1,3\}-[0-9]\+\b' docs/templates/session-log.md
```

```
29:> codescout's `statement-validity-session-log` starts at `F-2`/`W-3`
30:> rather than `F-1`/`W-1` (see `F-3` there).
105:> promotion working as intended, observed 2026-08-20 when `R-89`'s bullet was
108:> *"(R-1 + R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
116:audit correctly caught its own `W-4` as fired-and-unapplied and named the exact
```

Then, in any repo that has its own `reconnaissance-patterns.md`:

```
cp docs/templates/session-log.md <other-repo>/docs/trackers/<topic>-session-log.md
# in <other-repo>:
librarian(action="reindex")
librarian(action="link_scan")     # read dangling_by_source + the edge list
```

Note line 108 already *names* the right repo in prose — *"in codescout's
`docs/trackers/reconnaissance-patterns.md`"* — while the tokens themselves stay bare. The
human reader is told; the resolver is not.

## Environment

- codescout @ branch `experiments`, `docs/templates/session-log.md`
- Downstream repo measured: `claude-plugins` @ `2d6cdbe`
- Reached via `codescout-companion/skills/reconnaissance/SKILL.md` Phase 3, which instructs
  `cp <codescout-repo>/docs/templates/session-log.md docs/trackers/<topic>-session-log.md`

## Root cause

`link_scan` resolves a bare `PREFIX-N` against the definers present in the **scanned
repo**. The template's tokens were written inside codescout, where `R-89`, `R-1`, `R-7` and
`W-4` all have definers, so they resolved correctly and nothing flagged them. Copied out,
the same tokens are re-resolved against a different definer set:

- absent in the target → **dangling**
- present but unrelated in the target → **wrong edge, silently** (the `R-1` case)
- present in many session logs → **ambiguous** (the `F-2`/`W-3`/`F-1`/`W-1` case, which is
  the F/W-namespace-per-work-stream condition)

`get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified* already
specifies the remedy and even states the property that makes it safe: *"A qualifier naming
no file in this repo is still a cross-repo reference (`codescout:A-11`): reported, never
turned into an edge."* The template does not use it.

## Evidence

### The resolver's own rule, unapplied

From `get_guide("tracker-conventions")`:

> Cite **qualified by file stem** when several files share a prefix. […]
> A qualifier naming no file in this repo is still a cross-repo reference
> (`codescout:A-11`): reported, never turned into an edge, because edges cannot span
> workspaces.

`codescout:R-89` in the template would be reported as cross-repo in every consuming repo —
correct and inert. `R-89` is dangling in every consuming repo and correct only here.

### The wrong resolution

In `claude-plugins`, `docs/trackers/reconnaissance-patterns.md` defines `R-1`, `R-2`, `R-3`
locally. `link_scan` emitted an edge `roster-audit-session-log.md → reconnaissance-patterns.md`
from the freshly-copied template. `R-7` and `R-89` have no definer there and were reported
dangling; `R-1` is the only token in the copied file that can produce that edge. See
`unverified:` for the limits of this determination.

## Hypotheses tried

1. **Hypothesis:** the dangling citations came from the entries written into the new log,
   not from template boilerplate.
   **Test:** `grep -o '\b[A-Z]\{1,3\}-[0-9]\+\b'` over the copy, cross-referenced against
   the line numbers of the authored entries vs. the template prose.
   **Verdict:** rejected — the `R-*`/`W-4` tokens all sit in template prose (lines 29–131
   of the copy), above the first authored entry.
   **Evidence link:** § Reproduction.

## Fix

Qualify every id in `docs/templates/session-log.md` with the repo prefix:

| Line | Now | Should be |
|---|---|---|
| 29–30 | `` `F-2`/`W-3` ``, `` `F-1`/`W-1` ``, `` `F-3` `` | `codescout:statement-validity-session-log:F-2`, … |
| 105 | `` `R-89` `` | `codescout:R-89` |
| 108 | `R-1 + R-7` | `codescout:R-1 + codescout:R-7` |
| 116 | `` `W-4` `` | `codescout:W-4` |

Lines 29–30 need the double qualification (`<repo>:<file-stem>:<ID>`) because `F-N`/`W-N`
are namespaced per work stream and the prose already names
`statement-validity-session-log` as the owner.

**Then sweep the siblings.** This is a class, not an instance — the same reasoning applies
to any template this repo ships that quotes a local ledger. Check at minimum
`docs/templates/` in full and the reconnaissance/tracker-hygiene ledger templates
distributed from `claude-plugins`.

Fix commit SHA + `git patch-id --stable`: not yet applied.

## Tests added

None yet. The durable regression is cheap and directly targets the gap: copy each
`docs/templates/*.md` into a scratch repo with no ledgers, run `link_scan`, and assert zero
dangling and zero materialised edges. A template that resolves to *anything* in a foreign
repo has a bug by construction.

This is the third defect of the shape *"reconnaissance boilerplate that does not survive
being copied into another repo"* — see § References — which is the argument for the test
rather than a fourth manual fix.

## Workarounds

Downstream: after copying the template, qualify or delete the imported citations before the
first `link_scan(write=true)`. Nothing warns you to.

## Resume

Apply the § Fix table to `docs/templates/session-log.md`, then run
`librarian(action="link_scan")` in `claude-plugins` and confirm
`dangling_by_source["docs/trackers/roster-audit-session-log.md"]` drops to the `T-35`
residue only. Then grep `docs/templates/` for any remaining `\b[A-Z]{1,3}-[0-9]+\b` that
is not repo-qualified.

## References

- `docs/templates/session-log.md` lines 29, 30, 105, 108, 116 — the bare citations
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified* — the unapplied rule
- `claude-plugins:roster-audit-session-log:F-5` — the reconnaissance entry this issue is filed from
- `claude-plugins:roster-audit-session-log:F-3` — sibling defect: the recon skill's `**Valid:**` exemplar is rejected by `append_entry`
- `claude-plugins:repo-hygiene-session-log:F-2` — sibling defect: the template's own example index row burned the id it displayed
- `claude-plugins` `docs/issues/2026-08-20-reconnaissance-skill-prescribes-hand-allocated-edit-markdown-appends.md` — the same class, fixed at source 2026-08-20

