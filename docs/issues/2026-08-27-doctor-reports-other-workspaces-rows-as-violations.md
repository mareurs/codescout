---
id: d6984b7e960f79db
kind: bug
status: open
title: 'BUG: doctor''s report is 78% other repos'' rows — Ruling 17 applied to the entry-validity family but not to abs_path_outside_managed_roots'
tags:
- librarian
- doctor
- scope
- reporting
closed: null
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md
severity: medium
---

# BUG: `doctor`'s report is 78% other repos' rows — Ruling 17 was applied to the entry-validity family but not to `abs_path_outside_managed_roots`

## Summary
`librarian(action="doctor")` returns 516 violations in this repo, **401 of them
(77.7%) `abs_path_outside_managed_roots`** — rows belonging to other workspaces,
which the check's own hint calls "EXPECTED when the catalog spans several
workspaces". The same report scopes **102** entry-validity rows *out* for exactly
that reason. One tool, two opposite answers to the same question.

## Symptom (Effect)
Live run, 2026-08-27, active project `/home/marius/work/claude/codescout`:

```
"summary": {
  "total": 516,
  "shown": 118,
  "by_check": {
    "abs_path_outside_managed_roots": 401,
    "cited_prefix_with_no_definer": 47,
    "terminal_status_with_caveat": 42,
    "entry_dated_stale": 8,
    "entry_cited_from_outside_but_undeclared": 6,
    ...
  }
}
```

The report's own hint, verbatim:

```
abs_path_outside_managed_roots fired 401 time(s); showing 3 from offset 0, 398 elided.
... Rows outside the active project's roots are EXPECTED when the catalog spans
several workspaces — confirm a row should be under a managed root before treating
it as drift. 102 entry-validity row(s) (entry_conditional_past_due /
entry_dated_stale / entry_cited_from_outside_but_undeclared / validity_unparseable)
scoped out of this report because they belong to 7 other project root(s) ...
Exposure itself stays cross-repo (entry_indegree is not scoped); only the reported
worklist is limited to the active project, so a developer here is not handed other
repos' work.
```

`catalog_health.outside_roots_by_project` names the foreign owners directly —
`claude-plugins/buddy/tests/advisor-projection-eval`, `explore-project-eval`, etc.

## Reproduction
```
workspace(action="activate", path="/home/marius/work/claude/codescout")
librarian(action="doctor", limit=3)
# read $.summary.by_check and $.catalog_health.hint
```
Requires a catalog spanning more than one workspace root (7 other project roots
were present here). `git rev-parse HEAD` at observation: branch `experiments`,
tip `2dc8cadb`.

## Environment
Linux, codescout MCP over stdio, branch `experiments`, catalog shared across
13 registered project roots (`~/work`, `~/agents`).

## Root cause
Not a defect in the check's logic — a defect in **which population it reports**.

`src/librarian/tools/doctor.rs:229-232` states the governing principle
("Ruling 17") in a comment, and applies it to the validity-decay family only:

> Stays GLOBAL/unscoped — Ruling 17 — even though the three checks below now
> scope their REPORTED population to the active project: narrowing the metric
> itself would understate real cross-repo exposure and manufacture false
> negatives.

`scan_conditional_past_due`, `scan_dated_stale` and `scan_cited_but_undeclared`
each return `(violations, scoped_count)` and drop out-of-project rows from the
first while keeping `entry_indegree` global (`doctor.rs:233-250`).

`scan_artifact_paths` (`doctor.rs:1056`) never received the same treatment. Its
own doc comment already concedes the finding is not evidence of corruption
(`doctor.rs:1114-1118`): *"A firing row is not necessarily corrupt: a catalog
spanning several workspaces legitimately holds rows outside the active project's
roots."* So the check knowingly emits a population it has classified as expected.

measured 2026-08-27: `librarian(action="doctor", limit=3)` → `summary.by_check
.abs_path_outside_managed_roots = 401` of `total = 516`, alongside
`entry_validity_scoped_by_project` covering 102 rows across 7 roots.

## Evidence

### The split, in one report
See Symptom. 401 reported-and-expected vs 102 scoped-out, same run, same catalog.

### Ruling 17 is stated as general, not check-specific
`doctor.rs:1903-1906`, in `entry_indegree`'s doc comment, restates it as the
tool's principle rather than one family's exception: *"only the reported worklist
is scoped while the metric stays global."*

### The prior bug on this same check tuned paging, not population
`docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md`
made the elided rows reachable (stable ordering + `limit`/`offset`). That was the
right fix for the symptom it addressed and does not overlap this one: it made 401
rows *paginable*, not *fewer*.

## Hypotheses tried
1. **Hypothesis:** the 401 rows are genuine catalog drift (stale paths).
   **Test:** read `catalog_health.outside_roots_by_project`; every listed owner is
   a live sibling workspace with files on disk.
   **Verdict:** rejected — they are other repos' healthy rows.
2. **Hypothesis:** the check predates Ruling 17 and simply has not been revisited.
   **Test:** `doctor.rs:229` dates the ruling to the validity-decay family (Tasks
   5-7); `scan_artifact_paths` is older and carries its own "not necessarily
   corrupt" caveat instead.
   **Verdict:** confirmed — the caveat is the older, prose-only form of the same
   remedy.

## Fix
*Plan.* Apply Ruling 17 to `scan_artifact_paths`, matching the validity family's
shape exactly:

- Keep counting every foreign row in `catalog_health.outside_roots_by_project`
  (the metric — unchanged, still cross-repo).
- Drop rows whose `abs_path` resolves under a *different* managed root from
  `violations`, and report the count as `outside_roots_scoped_by_project`,
  mirroring `entry_validity_scoped_by_project`.
- Keep reporting a row that resolves under **no** managed root at all — that is
  the genuine drift the check was built for, and it is the case
  `artifact(move)`/`artifact(delete)` enforce via `containing_root`.
- Widening `scope` should restore them, so the cross-repo view stays reachable.

The distinction that makes this safe: "belongs to another workspace" and
"belongs to nowhere" are already distinguishable — `doctor.rs:1116-1118` says the
detail names the roots that were tried precisely so the two are separable at a
glance. Only the first should leave the worklist.

Not started. No SHA, no patch-id yet.

## Tests added
None yet. A regression test should assert that with two managed roots present,
a row under root B does not appear in `violations` when root A is active, while
`catalog_health.outside_roots_by_project["B"]` still counts it — i.e. the metric
and the worklist disagree on purpose.

## Workarounds
Read `summary.by_check` rather than `total`, and treat
`abs_path_outside_managed_roots` as a separate axis. The genuinely local checks
are the other ~115 findings. `catalog_health.outside_roots_by_project` already
attributes every foreign row to its owning project, so triage is possible today —
it just is not the default reading.

## Resume
Edit `scan_artifact_paths` in `src/librarian/tools/doctor.rs:1056`: thread the
managed-root list through so a row can be attributed to a *non-active* root,
partition on that, and return `(violations, scoped_count)` like
`scan_conditional_past_due` (`doctor.rs:234`) does. Then extend the
`catalog_health` hint at `doctor.rs:401-405` to name the scoped-out count, in the
same sentence shape as `doctor.rs:413-417`. Re-run
`librarian(action="doctor", limit=3)` and confirm `total` drops to roughly 115
with `outside_roots_by_project` unchanged at 401.

## References
- `src/librarian/tools/doctor.rs:229-232` — Ruling 17, as applied to the validity family
- `src/librarian/tools/doctor.rs:401-405` — the "EXPECTED" hint on this check
- `src/librarian/tools/doctor.rs:413-417` — the scoped-out hint this fix should mirror
- `src/librarian/tools/doctor.rs:1056` — `scan_artifact_paths`
- `src/librarian/tools/doctor.rs:1114-1118` — the check's own "not necessarily corrupt" caveat
- `src/librarian/tools/doctor.rs:1900-1906` — Ruling 17 restated as the tool's principle
- `docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md` — prior, non-overlapping fix to the same check
- `scripts/probe_librarian_scope.py` — the probe whose `machine_wide` bucket surfaced this

