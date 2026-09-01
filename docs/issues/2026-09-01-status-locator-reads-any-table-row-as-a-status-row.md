---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-01
owner: marius
related: []
severity: low
---

# BUG: `entry_status_region` reads ANY table row naming an entry as that entry's status row

## Summary

`entry_status_region` (`src/librarian/tools/doctor.rs:3946`) tries the table-row form first
and returns the **first line** matching `^\|\s*`?<ID>`?\s*\|`, with no check that the table
has anything to do with status. A tracker whose only table is an *analysis* table — one
column of numbers, one of prose — therefore has every entry's "status region" resolve to a
row that states no status, and `params_status_drift` reports drift for entries that have no
second representation at all.

That is the case its own doc comment says it excludes: *"An entry with no body region
stating a status is skipped, not reported. It has one representation and cannot drift."*

## Symptom (Effect)

`librarian(action="doctor")`, 2026-09-01:

```
[params_status_drift] docs/trackers/system-retrospective-improvements.md
  2 of 2 `tasks` entries have a `params` status their body region does not state:
  T-11 (params `done`, table row states no enum value);
  T-12 (params `dropped`, table row states no enum value).
```

The locator string `table row` is the tell. That tracker has exactly one table:

```
| task | headline number | reality |
|---|---|---|
| T-11 | `missing field 'patch'` ×11, largest single cause in its family | all 11 predate … |
| T-12 | `artifact_event` 26.1% error rate, worst on the surface | spans **two** fixes … |
```

Three columns, none of them status. It is a measurement-vs-reality table added by the
close-out commit `dcd5ff14`, and it is not the entry index — this tracker has none.

## Reproduction

1. A params-backed ledger whose `params_schema` declares a `status` enum.
2. Its entries are defined by `## <ID> — <title>` headings carrying **no** `**Status:**`
   line — this tracker records closure in a `## History` section instead, which is a
   legitimate convention.
3. Anywhere in the file, a table with a row beginning `| <ID> |` for some reason other than
   status.
4. `librarian(action="doctor")` → `params_status_drift` fires on every such entry.

Live instance: `docs/trackers/system-retrospective-improvements.md` at `dcd5ff14`.

## Environment

codescout `0.15.0`, branch `experiments`, catalog `~/.local/share/librarian/catalog.db`.

## Root cause

Branch order, plus a predicate that under-specifies what it is addressing.

```rust
// 1. Table row. Anchored at line start so a mention inside another row's prose
//    cannot masquerade as this entry's row.
if let Ok(row_re) = regex::Regex::new(&format!(r"^\|\s*`?{esc}`?\s*\|")) {
    if let Some(l) = lines.iter().find(|l| row_re.is_match(l)) {
        return Some(((*l).to_string(), "table row"));
    }
}
// 2. Heading section, then its `Status:` line.
```

Branch 1 runs first and returns unconditionally on a match. For
`system-retrospective-improvements` it succeeds on the analysis row, so branch 2 — which
would have found `## T-11 — …`, found no `Status:` line, and correctly returned `None` —
is never reached. The shadowing is what converts a skip into a finding.

**The author already reasoned about masquerading and closed the adjacent case.** The
comment says the anchor exists so *"a mention inside another row's prose cannot masquerade
as this entry's row"* — a mention in the middle of a row. What is not closed is a row that
legitimately **starts** with the id and is not about status. The line anchor is necessary
and insufficient.

**Why the sensitivity measurement did not surface it.** The scan's doc comment reports 490
of 536 simulated drifts flagged (91.4%). That experiment substitutes a wrong status into
entries that *have* a status region, so it measures false **negatives** over a population
selected to have two representations. This defect is a false **positive** on a population
the experiment excludes by construction — CLAUDE.md § *Testing Discipline*'s recording-filter
law: no member of the sample could have exhibited it.

**`IC-6`, the escape-hatch half.** `| <ID> |` is being used to address "this entry's status
row", and there is no way to write a table row that merely *mentions* an entry without the
locator reading it as a status declaration. Same shape as that class's member where a
documentation example of citation syntax was counted as a real citation. (Tagged on that
reading; a reader who thinks the defect is branch **ordering** rather than an
under-specified address should re-tag rather than inherit this.)

## Evidence

Read at the bytes 2026-09-01:

- `entry_status_region`, `src/librarian/tools/doctor.rs:3946` — branch 1 returns before
  branch 2 is considered.
- `docs/trackers/system-retrospective-improvements.md` — the only `^|` block is the
  `| task | headline number | reality |` table; `grep -n '^|'` returns 4 lines, the header,
  the separator, and the T-11/T-12 rows.
- `## T-11 — Fix the bare missing-param error…` carries `**Why:** / **Shape:** /
  **Acceptance:**` and no `Status:` line. Same for `## T-12`. So branch 2 would return
  `None`, which the scan documents as *not a finding*.

**Scope: the population is 1, measured rather than deferred.** Both current
`params_status_drift` findings were partitioned by the locator string their own message
carries:

```
system-retrospective-improvements.md  -> table row      <- this defect
eduplanner-ui/open-bugs-worklist.md   -> Status: line   <- branch 2, a real region
```

So exactly one finding in the corpus comes from branch 1, and the other is the check
working. That is small enough that the honest options narrow to the header-column fix or a
documented limitation at the refusal site — and it is the count `R-117` asks for before
designing a partition, taken before rather than after.

## Hypotheses tried

1. **Hypothesis:** the check conflates *"body states a different status"* with *"body states
   no status"*, and reports both under one name.
   **Test:** read `scan_params_status_drift`'s doc comment and body — `entry_status_region`
   returning `None` hits `continue`, so the silent case is genuinely skipped.
   **Verdict:** **rejected.** This was the hypothesis the investigation started from and it
   was wrong; the defect is upstream, in which region gets found.

2. **Hypothesis:** the peer's close-out commit `dcd5ff14` forgot to update a status column.
   **Test:** `grep -n '^|'` on the tracker — there is no status column anywhere in the file,
   and no entry has ever had one.
   **Verdict:** rejected. Nothing was forgotten; the convention is `## History`.

3. **Hypothesis:** branch 1 matches a row that is not a status row and shadows branch 2.
   **Test:** the reported locator is `table row`; the only table is the analysis table; the
   heading sections carry no `Status:` line.
   **Verdict:** **confirmed.**

## Fix

Options, unranked. Count the population first — see Evidence.

- **Require the table to look like a status table.** Before accepting a row, check the
  header row above it for a `status` column, and return the cell under it rather than the
  whole line. Most precise; costs a header scan and a column index.
- **Accept a row only if it contains some enum token**, else fall through to branch 2. Cheap
  and self-limiting, but it makes a *genuinely drifted* row (whose status was blanked)
  invisible — trading a false positive for a false negative in the direction the check
  exists to catch. Probably wrong for that reason; recorded so it is not re-proposed.
- **Try branch 2 first.** A `## <ID> — …` heading with a `Status:` line is the stronger
  signal; a table row is the fallback. Smallest diff, but it does not fix a ledger that has
  a non-status table and no `Status:` lines — the exact case here — so it is not a fix at
  all for this instance. Recorded to be ruled out rather than re-derived.

**Deliberately not proposed:** adding a `Status:` line to `system-retrospective-improvements`
to silence the finding. That would edit a tracker to fit an instrument, and the tracker's
convention is fine.

## Tests added

None yet. The test must assert the check is **silent** on a ledger with a non-status table —
and note that assertion is monotone under "the check does nothing", so it needs a sibling
that stays red: a real drift in the *same* fixture that must still be reported. One test
alone here is a control, not a discriminator.

## Workarounds

None needed — the check reports only, has no `fix=`, and its message already says it is a
heuristic that never claims the entry is wrong. The cost is one false finding in the doctor
report, which is triage noise rather than a wrong write.

## Resume

Count the population first. If it is this one tracker, the honest options are the
header-column fix or documenting the limitation at the refusal site.

## References

- `src/librarian/tools/doctor.rs` — `entry_status_region`, `scan_params_status_drift`.
- `docs/trackers/system-retrospective-improvements.md` — the live instance.
- `docs/trackers/open-issue-work-queue.md` — `BL-44`, the conditional this check partially
  discharges.
- `docs/trackers/issue-clusters.md` — `IC-6`.
