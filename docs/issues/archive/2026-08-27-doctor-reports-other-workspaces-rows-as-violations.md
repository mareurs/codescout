---
id: b176ff103ec56c09
kind: bug
status: fixed
title: 'BUG: doctor''s report is 78% other repos'' rows — Ruling 17 applied to the entry-validity family but not to abs_path_outside_managed_roots'
tags:
- librarian
- doctor
- scope
- reporting
closed: 2026-08-27
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

### The measured partition — the discriminator the plan lacked

Against the live catalog (`~/.local/share/librarian/catalog.db`, 4,265 artifacts,
29 distinct `commits.git_root`), with the session's real managed roots
reconstructed from `~/.config/librarian/workspace.toml`:

| bucket | rows |
|---|---|
| firing total | 402 |
| under an **umbrella member** of the active project's umbrella | 359 (89%) |
| under another repo the catalog holds **commits** for | 33 |
| under **neither** — nothing on this machine claims it | **10** (2.5%) |

This is the distinction `check_outside_managed_roots`'s doc comment had always
described in prose — *"expected if it belongs to another workspace; a defect if it
should be under one of ..."* — and never computed.
## Hypotheses tried
1. **Hypothesis:** the 401 rows are genuine catalog drift (stale paths).
   **Test:** read `catalog_health.outside_roots_by_project`; every listed owner is
   a live sibling workspace with files on disk.
   **Verdict:** rejected — they are other repos' healthy rows.
3. **Hypothesis:** the 401 rows can be partitioned by which *managed root* they
   belong to, so the fix is to drop rows under a root other than the active one.
   **This was this file's own `## Fix` plan.**
   **Test:** read `managed_roots` (`src/librarian/tools/mod.rs:215`) and
   `check_outside_managed_roots`.
   **Verdict:** **rejected — the case does not exist.** `managed_roots` returns the
   active project's `git_root` and `abs_path` plus the legacy `workspace.roots`
   entries, and never another repo; the check fires only when `containing_root`
   matches *nothing*. Every firing row is under NO managed root, so the prescribed
   partition was over the empty set — it would have shipped, passed a test written
   from the same wrong model, and changed the report by zero rows. See
   `bug-fix-session-log:F-74`.
4. **Hypothesis:** a real discriminator exists in data already on hand.
   **Test:** partitioned the firing rows against (a) members of the active
   project's umbrella and (b) distinct `commits.git_root`, over the live catalog.
   **Verdict:** confirmed — 402 rows split 359 / 33 / 10. See Evidence.
   **Test:** `doctor.rs:229` dates the ruling to the validity-decay family (Tasks
   5-7); `scan_artifact_paths` is older and carries its own "not necessarily
   corrupt" caveat instead.
   **Verdict:** confirmed — the caveat is the older, prose-only form of the same
   remedy.

## Fix

**Shipped in `442d8b7c` on `experiments`**, but NOT as this section originally
prescribed — see Hypothesis 3. The original plan partitioned an empty set.

What shipped instead:

- **`known_workspace_roots(ctx, conn)`** (new, `src/librarian/tools/doctor.rs`) —
  roots this machine knows about but is not managing this session: members of the
  active project's umbrella, plus every distinct `commits.git_root` in the
  catalog.
- **`scan_artifact_paths`** takes them and returns
  `(violations, scoped_by_project)`. A firing row under a known root is counted,
  not reported.
- **The metric stays global.** `outside_roots_by_project` is now built from the
  violations *and* the scoped map, so it counts exactly what it counted before —
  the Ruling 17 requirement, and the one thing a naive "drop them" fix would have
  broken, since that aggregate is derived from `all_violations`.
- **`catalog_health.outside_roots_scoped_by_project`** plus a hint naming the
  drop, mirroring `entry_validity_scoped_by_project`.

Effect: the report goes 516 → ~124 findings, this check 401 → 10, and the 392
scoped-out rows remain in the unscoped metric.

It deliberately does not consult the filesystem: a row is scoped out for
belonging somewhere known, not for existing. `check_missing_file` owns
disappearance, and conflating them would make one defect wear two names — the
rule `scan_artifact_paths` already applies to the relative-path case.

**Fix commit — record both, they fail differently:**

- SHA `442d8b7cef5263e87eca7b5ea96781d5204b1393` on **`experiments`**
- patch-id `696dc8e4344e3c21bf60bd6cdbc5a042ef4e9d26`
## Tests added

Two, both in `src/librarian/tools/doctor.rs`:

- **`a_row_under_a_known_workspace_root_is_scoped_out_but_still_counted`** — the
  discriminating pair: two rows, both outside every managed root, only one of them
  anyone's work. Asserts the sibling leaves `violations` **and** lands in the
  scoped map, so the worklist narrows without the metric doing so.
  **Mutation-verified, not merely green:** disabling the partition failed it with
  `got ["/tmp/cs-orphan/docs/c.md", "/tmp/cs-sibling/docs/b.md"]`.
- **`empty_known_elsewhere_reports_every_outside_row_as_before`** — pins the
  fallback, so a caller that cannot compute the known-roots set (no umbrella, no
  commits rows) degrades to the old reporting rather than silently losing the
  check.

Paths are built from `temp_dir()` so both run platform-native on Windows and
unix; the path-form normalisation they would otherwise depend on belongs to
`containing_root` (WIN-30) and is tested there.

Gate at fix time: 4600 passed / 0 failed / 51 ignored (baseline 4598, +2 = exactly
the tests added); fmt clean; clippy clean.
## Workarounds
Read `summary.by_check` rather than `total`, and treat
`abs_path_outside_managed_roots` as a separate axis. The genuinely local checks
are the other ~115 findings. `catalog_health.outside_roots_by_project` already
attributes every foreign row to its owning project, so triage is possible today —
it just is not the default reading.

## Resume

N/A — fixed, live-verified, archived.

**Live verification, 2026-08-27**, after `cargo rb` + `/mcp`. All three parts, because any one alone is consistent with a broken fix:

| check | before | after |
|---|---|---|
| `summary.total` | 516 | **129** |
| `by_check.abs_path_outside_managed_roots` | 401 | **10** |
| `catalog_health.outside_roots_by_project` | 401 rows | **unchanged** |

Part 3 is the one a naive fix fails silently, so it was checked structurally
rather than by eye. `outside_roots_by_project` enumerates **112** project roots;
`outside_roots_scoped_by_project` enumerates **107**. The five keys present only
in the metric — `/home/marius/Documents/PFA` (3), `/home/marius/work/claude/pi`
(1), the `whatsapp` worktree (2), `.../personal/home/terasa` (2),
`.../personal/misc/avatar` (2) — sum to **exactly 10**, which is the violation
count. Every shared key carries an identical value in both maps
(`claude-plugins` 98, `prompt-engineering` 45, `researcher` 19,
`agents/llm-proxy` 15), so the fold-back preserved the metric rather than
recomputing it.

One artefact worth not misreading: `grep -c abs_path_outside_managed_roots` over
the response returns **5**, not 10. That is `limit=3` capping the EMITTED sample
(3 violation rows + the `summary.by_check` line + the hint line). The
authoritative count is `summary.by_check`, which reports the true total — the
cap-but-count behaviour established by
`docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md`.
## References
- `src/librarian/tools/doctor.rs:229-232` — Ruling 17, as applied to the validity family
- `src/librarian/tools/doctor.rs:401-405` — the "EXPECTED" hint on this check
- `src/librarian/tools/doctor.rs:413-417` — the scoped-out hint this fix should mirror
- `src/librarian/tools/doctor.rs:1056` — `scan_artifact_paths`
- `src/librarian/tools/doctor.rs:1114-1118` — the check's own "not necessarily corrupt" caveat
- `src/librarian/tools/doctor.rs:1900-1906` — Ruling 17 restated as the tool's principle
- `docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md` — prior, non-overlapping fix to the same check
- `scripts/probe_librarian_scope.py` — the probe whose `machine_wide` bucket surfaced this
