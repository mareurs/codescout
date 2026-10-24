---
id: '70f4cad102afb762'
kind: bug
status: fixed
title: friction-probe.py counts only outcome='error' and misses every recoverable_error, so it sees 8 of 2,038 errors
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-24
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

Implemented on the isolated research/checked-real-task-20261024 branch: exact ERROR_OUTCOMES membership at every Python report site, the matching SQL IN predicate, and corrected help text. Success/other rows remain in sequences and denominators. No hard-only flag or arbitrary warning threshold was needed for the correction. This branch is not merged or published; the required full repository gate passes, and the post-review Python lane passes with all 14 focused cases included. Delivery evidence is recorded in its real-task report.

## Tests added

Thirteen frozen SQLite/subprocess tests were written before implementation: unchanged source failed 11; a separate full-population control passed 13; five partial-fix controls failed. The single low-effort candidate passed all 13 independently. Review actually applied 16 site-specific mutations: 15 were assertion-killed and one survived. After preserving and reproducing that survivor (unknown next outcome sharing its predecessor's signature), one additional test failed against the mutant and passed against unchanged candidate source. The strengthened 14-test suite kills the same 16 mutations with no survivors. Original 13-test evidence is preserved separately.

## Workarounds

Query the intended population directly with outcome IN ('error', 'recoverable_error'); outcome != 'success' would also select unknown values. A frozen retained-corpus backup on 2026-10-24 held 80,924 calls, 19 hard errors and 3,053 recoverable errors. The candidate and independent SQL both count 3,072 errors; original source counts 19. These are retained rows in that snapshot, not all-time counts.

## Resume

Implementation and acceptance are ready in /tmp/codescout-real-task-20261024 on research/checked-real-task-20261024. See docs/research/2026-10-24-checked-completion-real-task.md for the real-corpus calibration, exact accounting and verification status. Retain the worktree; merge/publication is outside this trial.

## References

- `docs/issues/archive/2026-09-02-recoverable-error-outcome-is-unreachable-in-production.md`
- `docs/PROBES.md` (row "How often do agents err")
- codescout memory `infra/friction-measurement` (repeats the stale docstring claim)
