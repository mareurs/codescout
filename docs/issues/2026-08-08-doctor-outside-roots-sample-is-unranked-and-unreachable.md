---
id: f95e6841030e4e9c
kind: bug
status: open
title: 'BUG: doctor''s outside-managed-roots sample is an unranked prefix, and the 281 elided rows cannot be reached by any parameter'
owners:
- marius
tags:
- librarian
- doctor
- progressive-disclosure
- catalog
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

Two independent gaps that only matter together:

- **Unranked.** `scan_artifact_paths` (`src/librarian/tools/doctor.rs:702`) issues
  `SELECT id, abs_path FROM artifact` with no `ORDER BY`, so row order is whatever the
  planner returns. The `retain` that applies the cap keeps the first 10 in that order. Which
  10 appear can change when an index is added or the DB is VACUUMed, with no content change.
- **Unreachable.** `doctor` takes `fix`, `root`/`old_root`/`new_root`, `confirm` — and
  nothing that pages, filters or widens the emitted set.

Contrast the sibling fix in
`docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`: that truncation was
made safe by **ranking before truncating** (`sort_by_key` on `(resolved, severity)`), so
whatever drives the outcome is guaranteed to be inside the shown window. This cap
reintroduced the unranked prefix that fix removed.

Also fails `docs/PROGRESSIVE_DISCOVERABILITY.md` § Pattern 1 — an overflow hint must name at
least one parameter with a real value the caller can use. This hint names none, because
none exists.

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

Both halves, in either order:

1. **Rank before truncating**, mirroring
   `src/librarian/tools/audit_doc_refs/mod.rs` — a stable `sort_by_key` so the emitted
   sample is the most actionable 10 rather than the first 10. Candidate key: rows under a
   root that *used to* exist (a rehome candidate) before rows in a foreign workspace,
   since the former are real drift and the latter are the expected case.
2. **Make the remainder reachable.** Either honour `limit` for `doctor` (cheapest, and it
   is already in the schema), or add `check=<name>` to emit one check unabridged. Whichever
   is chosen, the hint must name it with a real value — that is the
   `docs/PROGRESSIVE_DISCOVERABILITY.md` contract.

Not attempted in `6f261da9`: that commit was closing PR #10 review findings, and this is a
design change to the check's output contract rather than a defect in what shipped.

## Tests added

None yet. The regression test to write with the fix: seed >10 outside-roots rows with a
deterministic ordering signal, assert the emitted sample contains the highest-ranked rows
rather than the first-inserted ones — a test that fails if the `sort_by_key` is removed.
Assert separately that the reachability parameter actually changes `shown`, since a
silently-ignored parameter is precisely what this file measures.

## Workarounds

Query the catalog directly — the rows are in SQLite, only the report is capped:

```sql
SELECT id, abs_path FROM artifact;   -- then filter against your configured [[roots]]
```

## Resume

Start with Fix step 2, because it is the smaller change and it makes step 1 verifiable:
honour `limit` in `doctor`'s arg handling, then re-run
`librarian(action="doctor", limit=400)` and confirm `shown` rises toward 311. Only then add
the ranking, so you can see which rows the ranking promotes. Both changes land in
`src/librarian/tools/doctor.rs` around the `retain` that applies `OUTSIDE_ROOTS_SAMPLE`.

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
