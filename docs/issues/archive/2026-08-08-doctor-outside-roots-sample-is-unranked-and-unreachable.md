---
id: '148cf04fedaf5f80'
kind: bug
status: fixed
title: 'BUG: doctor''s outside-managed-roots sample is an unranked prefix, and the 281 elided rows cannot be reached by any parameter'
owners:
- marius
tags:
- librarian
- doctor
- progressive-disclosure
- catalog
- cluster/accepted-parameter-silently-dropped
closed: 2026-08-14
---

## Summary

`librarian(action="doctor")` caps the emitted `abs_path_outside_managed_roots` rows at a
10-row sample. The cap is announced, which is right. But the 10 kept rows are the *first*
10 in query order from a `SELECT` with no `ORDER BY` — not a ranked sample — and `doctor`
exposes no `limit`, `offset`, `paths` or `show_all` that reaches the rest. The hint
instructs the reader to inspect the elided rows, which is not possible.

On this machine that is **281 rows the report names and no call can retrieve**.

## Symptom (Effect)

`librarian(action="doctor")` on the codescout catalog, 2026-08-08, binary built from
`9be2ede4`:

```json
{ "total": 311,
  "shown": 30,
  "by_check": { "abs_path_outside_managed_roots": 291,
                "missing_file": 7,
                "worktree_scoped_row": 13 } }
```

291 outside-roots violations, 10 emitted, **281 elided**. The accompanying hint says:

> `abs_path_outside_managed_roots` fired 291 time(s); showing 10, 281 elided (full count in
> summary.by_check). Rows outside the active project's roots are EXPECTED when the catalog
> spans several workspaces — **confirm a row should be under a managed root before treating
> it as drift.**

That instruction is unexecutable for 281 of the 291 rows.


### Re-reproduced 2026-08-14 on the live server at `141b2cbf`

The defect is unchanged and the numbers have grown:

```json
{ "total": 322,
  "shown": 23,
  "by_check": { "abs_path_outside_managed_roots": 309,
                "missing_file": 11,
                "worktree_scoped_row": 2 } }
```

309 outside-roots violations, 10 emitted, **299 elided**, and the hint still
instructed the reader to confirm rows no call could return.

Incidental, worth carrying elsewhere: `worktree_scoped_row` is now **2**, both
under `/home/marius/work/claude/whatsapp/.claude/worktrees/whatsapp-cli`. The 13
`backend-kotlin` rows that task #46 was opened to protect are gone from the
catalog.
## Reproduction

```
librarian(action="doctor")                # shown: 30
librarian(action="doctor", limit=100)     # shown: 30 — byte-identical result
```

**measured 2026-08-08:** both calls returned an identical 13687-byte payload and the same
`summary`. `limit` is accepted without a `RecoverableError` and silently ignored — it is
declared in the librarian tool schema for `legibility_scan` and `link_scan` only, but
nothing rejects it here.

## Environment

codescout 0.15.0, branch `experiments` at `9be2ede4`, live MCP (stdio). Catalog spans the
`codescout-ecosystem` umbrella, which is why 291 rows legitimately sit outside the active
project's roots.

## Root cause

**Measured 2026-08-14 at `141b2cbf`.** Both halves confirmed at the bytes before
any change; the filing's line numbers had shifted, its mechanism held.

- **Unstable order.** `scan_artifact_paths` issued
  `SELECT id, abs_path FROM artifact` with no `ORDER BY`, so the `retain` that
  applies the cap kept whatever prefix the planner returned. Which 10 appear
  could change after adding an index or running VACUUM, with no content change.
  `scan_worktree_scoped` had the same unordered `SELECT`.
- **Unreachable remainder.** `grep -n '\blimit\b' src/librarian/tools/doctor.rs`
  returned **zero matches** — `doctor` read no paging parameter of any kind. The
  filing's "it is already in the schema" was true only of the shared librarian
  `Args` shape; doctor's handler never looked at it.

The two only matter together, and the hint made it worse by naming an action
("confirm a row should be under a managed root") that was impossible for 299 of
309 rows. That fails `docs/PROGRESSIVE_DISCOVERABILITY.md` § Pattern 1: an
overflow hint must name at least one parameter with a real value.
## Evidence

### The count fix is not this fix

`summary.total` now partitions `summary.by_check` (fixed in `6f261da9`), so the report no
longer *understates* the problem — 311 is correct. This bug is the other half: the number
is right and the rows behind it are still unreachable. Filed separately for that reason.

### Why it is not merely cosmetic

`abs_path_outside_managed_roots` is a check whose whole output requires human triage: the
hint itself says rows outside the active roots are EXPECTED on a multi-workspace catalog.
A check that cannot show its evidence is a count with no audit trail — the reader either
trusts 291 or dismisses it, and neither is the intended action.

## Hypotheses tried

1. **Hypothesis:** `limit` is honoured and 30 is coincidental.
   **Test:** `librarian(action="doctor", limit=100)`. **Verdict:** rejected — identical
   13687-byte payload, `shown` unchanged at 30.
2. **Hypothesis:** the elided rows are reachable through the emitted `audit_issues`-style
   tracker. **Test:** `doctor` writes no tracker; it returns JSON only (see the tool
   description: "JSON violation-count report"). **Verdict:** rejected.
3. **Hypothesis:** the cap is stable because SQLite returns rowid order in practice.
   **Verdict:** deferred — plausible for a simple table scan today, but it is not a
   guarantee, and it is orthogonal to reachability. Ranking is the fix either way.

## Fix

Implemented in `27309362` (`experiments`). `master` is a strict ancestor, so
promotion is a fast-forward and this SHA already is the master-side SHA — there
is no second SHA to record.

Three changes in `src/librarian/tools/doctor.rs`, plus the schema:

1. **Deterministic order.** `ORDER BY abs_path` on both artifact `SELECT`s and
   `ORDER BY git_root` on the commits scan. This is what makes a window
   comparable between runs — and it is a precondition for `offset`, which would
   otherwise page through a set that reshuffles.
2. **Reachability.** `limit` (default 10) sizes the outside-roots window;
   `offset` pages through the rest. Both documented on the librarian schema.
3. **`catalog_health.outside_roots_by_project`.** A per-project count over
   **all** outside-roots rows, computed before the truncation.

The hint now reads, with real values substituted:

> … showing 10 from offset 0, 299 elided. Rows are ordered by abs_path, so the
> window is stable across calls: `librarian(action="doctor", limit=309)` returns
> all of them, or `limit=10, offset=10` for the next page.
> `catalog_health.outside_roots_by_project` counts every row, elided ones
> included. …

### Why (3) is part of the fix and not scope creep

Ordering by `abs_path` makes the default window **deterministic but clustered** —
all 10 rows now come from whichever project sorts first, where the old arbitrary
order happened to spread across workspaces. Determinism alone would therefore
have traded one flaw for another. The aggregate is what keeps the window honest:
the reader sees the whole distribution and knows which project to page into. The
test `outside_roots_by_project_counts_elided_rows_too` asserts the clustering
explicitly, so the coupling cannot be silently broken.

### Deliberately NOT done — the actionability ranking

The filing's Fix step 1 proposed ranking rows so the sample is the *most
actionable* 10 — "rows under a root that used to exist (a rehome candidate)
before rows in a foreign workspace". That is not implemented, for two reasons
found while measuring:

- **Not cheaply computable as described.** "Used to exist" is the dead-root /
  move-candidate derivation, which already runs as its own check and reports
  separately; folding it into this sample's sort key would couple two checks.
- **No evidence for the ordering.** The check's own message says a foreign
  workspace is the *expected* case, but nothing in the catalog distinguishes
  "foreign workspace, fine" from "project that should be a managed root and
  isn't". The post-v6 `artifact` table has only `abs_path` — the legacy `repo`
  column was dropped — so any grouping is a path heuristic, and picking which
  heuristic means "actionable" is a product judgment, not a defect fix.

With (1)-(3) in place the ranking is a **ranking of a fully reachable, fully
accounted set** — an ergonomics improvement, no longer a correctness gap. Left as
an open decision rather than guessed at.
## Tests added

Seven new in `src/librarian/tools/doctor.rs`, module `tests`:

| test | what it pins |
|---|---|
| `outside_managed_roots_limit_reaches_every_elided_row` | `limit` returns every row the summary counts; no elision hint when nothing is elided |
| `outside_managed_roots_offset_pages_without_gaps_or_repeats` | three pages reconstruct the full set **in order**, and no row appears twice |
| `outside_managed_roots_sample_is_ordered_and_repeatable` | repeated calls return the same window, and it is in `abs_path` order |
| `outside_roots_by_project_counts_elided_rows_too` | the aggregate partitions the **true** total (25), not the shown 10 — and asserts the window really is clustered, which is why the aggregate exists |
| `outside_managed_roots_hint_names_a_parameter_that_reaches_the_rest` | the Pattern 1 contract: hint contains `limit=25`, `offset=10`, `outside_roots_by_project`, and still announces `15 elided` |
| `outside_roots_group_uses_the_project_prefix_before_docs` | only the *first* `docs` component splits, so a nested `docs/manual/docs/` does not fragment one project into several groups |
| `outside_roots_group_falls_back_to_the_parent_without_a_docs_component` | the no-`docs` fallback |

The pre-existing `outside_managed_roots_caps_the_list_but_not_the_count` and
`summary_total_partitions_by_check` both still pass unchanged — the default
behaviour (10-row window, true counts in `summary`) is preserved, and only the
reachability around it is new.

### Gate

`cargo test --workspace` → **3781 passed / 0 failed / 50 ignored**;
`cargo clippy --workspace --all-targets -- -D warnings` clean.

One guard caught a real mistake on the way: `every_tool_description_under_cap`
(`src/server.rs:2128`) rejected the change twice while the `librarian` tool
description exceeded its 1800-char budget (1960, then 1846). The description was
already at ~1790/1800 — **99.4% of budget before this change** — so the
windowing note was moved out of the action list and into the `limit`/`offset`
parameter descriptions, which are not capped, plus the runtime hint. That is the
right home for parameter detail anyway. The budget pressure itself is worth
knowing about: the next action added to `librarian` will trip the same gate.
## Workarounds

N/A — fixed. On a pre-fix binary there was genuinely no way to reach the elided
rows through the tool; the only recourse was querying the catalog DB directly.
## Resume

N/A for the reported defect — both halves fixed and gated.

One deliberate non-decision is left open, and it is now an ergonomics choice
rather than a correctness gap: whether the default 10-row window should be
*ranked* by actionability instead of `abs_path` order, and what "actionable"
means for a check whose own message says the foreign-workspace case is expected.
See § Fix → *Deliberately NOT done*. Anyone picking it up starts from
`outside_roots_group` and the `outside_roots_by_project` distribution, which now
show what the real data looks like.
## References

- `src/librarian/tools/doctor.rs:702` — the unordered `SELECT`
- `src/librarian/tools/doctor.rs` — `OUTSIDE_ROOTS_SAMPLE`, the `retain`, and the hint
- `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md` — same shape, fixed
  by ranking before truncating
- `docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md` — precedent for a
  schema-declared parameter silently ignored by its tool
- `docs/PROGRESSIVE_DISCOVERABILITY.md` § Pattern 1 — hints must name a usable parameter
- PR #10 review, 2026-08-08 — surfaced by the librarian-correctness reviewer; confirmed
  here against live data rather than the 25-row test fixture

## Fix provenance

- **SHA:** `6f261da9` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `4323669256772174afffff114fc62c7ff0b21e4a` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 432366925677 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
