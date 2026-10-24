---
id: '70f4cad102afb762'
kind: bug
status: open
title: friction-probe.py counts only outcome='error' and misses every recoverable_error, so it sees 8 of 2,038 errors
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-10-24
owner: marius
severity: medium
---

# BUG: friction-probe.py counts only `outcome='error'` and misses every `recoverable_error`

## Summary

`scripts/friction-probe.py` selects errors with `outcome == 'error'` (`coverage_by_tool`, `immediate_repeat`, `calls_to_recovery`, `error_runs`, `null_detail`, `compare_split`, `main`). Since `docs/issues/archive/2026-09-02-recoverable-error-outcome-is-unreachable-in-production.md` was fixed, codescout writes `recoverable_error` for every `RecoverableError` (`classify_content_result`, `src/usage/mod.rs`). The probe therefore sees only hard errors. On the codescout `usage.db`, `--days 21` reported `52756 calls, 8 errors (0.02%)`, while the same window holds 2,030 `recoverable_error` rows and 8 `error` rows. `docs/PROBES.md` names this probe as THE instrument for "How often do agents err or repeat a call, and how do they recover?".

## Class fit

The probe's selector (`outcome = 'error'`) is narrower than the population its name and docstring claim ("error" = every error, "codescout's RecoverableErrors ARE counted here"). The members it never saw (99.6% of errors) cannot be counted from its own output; it differs from other IC-18 members because the population grew under it when a sibling fix made a third outcome value reachable.

## Symptom (Effect)

`python3 -I scripts/friction-probe.py --days 21 --calibrate` on 2026-10-24 printed `corpus: 52756 calls, 8 errors (0.02%), 6 unclassified` and an empty family table. The calibration section printed `—` for every TU-7 family. Nothing in the output says the selector excludes a population.

## Reproduction

1. `sqlite3 .codescout/usage.db "SELECT outcome, COUNT(*) FROM tool_calls WHERE called_at>='2026-10-03' GROUP BY 1"` → `error 8`, `recoverable_error 2030`.
2. `python3 -I scripts/friction-probe.py --days 21` → `8 errors`.

## Environment

codescout `experiments` at `30f3e728d`; usage.db retained corpus 2026-09-24 .. 2026-10-24.

## Root cause

The probe was written when `recoverable_error` was unreachable in production, so `error` was the only error value. The 2026-09-02 fix made the documented third value real; the probe and its docstring were not updated. Grep: `scripts/friction-probe.py` lines 17, 99, 136, 146, 189, 194, 234, 264, 331, 486 all test `outcome` against `'error'`.

## Evidence

- Probe docstring line 17: `* "error"      = \`outcome = 'error'\`. codescout's RecoverableErrors ARE counted here;` — false since 2026-09-02.
- Daily split in the codescout DB: every day since 2026-09-24 has 0-6 `error` and 8-429 `recoverable_error` rows.
- `src/usage/db.rs` `query_stats` and `recent_errors` already use `outcome IN ('error', 'recoverable_error')`, so the in-binary consumers were updated and only the script was missed.

## Hypotheses tried

N/A — confirmed directly by the two queries above.

## Fix

Not started. Proposed: one predicate `is_error(row) = row["outcome"] in ("error", "recoverable_error")` used everywhere; keep a `--hard-only` flag for the old view; fix the docstring; add a self-check that warns when the selected error count is below 10% of `outcome != 'success'`.

## Tests added

None yet.

## Workarounds

Query `usage.db` directly with `outcome != 'success'`. The 2026-10-24 usage sweep did this (`docs/usage-reports/2026-10-24-usage-analysis.md`).

## Resume

Start at `load_rows` and the eleven `outcome` tests listed above; then re-run `--calibrate` and compare with TU-7.

## References

- `docs/issues/archive/2026-09-02-recoverable-error-outcome-is-unreachable-in-production.md`
- `docs/PROBES.md` (row "How often do agents err")
- codescout memory `infra/friction-measurement` (repeats the stale docstring claim)
