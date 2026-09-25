---
id: '5d4e9ab75d686fed'
kind: bug
status: open
title: 'Codex: direct test runner skips FreezeMenuGuard regressions'
tags:
- cluster/declared-not-wired
opened: 2026-09-25
owner: marius
severity: low
---

# Direct execution skips the new freeze regressions

**Valid:** dated 2026-09-25

Observed at `805c2a83`, introduced by `da67db02`. `tests/test_stage2_synthetic.py:109` calls unittest.main() before the new module loading and FreezeMenuGuard class at lines 113–141. The default runner exits before defining those five tests.

## Observed verification

`python3 tests/test_stage2_synthetic.py` exits 0 and reports 16 tests. Import-based discovery sees 21 tests. In an isolated in-memory probe, replacing check_menu_positives with a counting function that never rejects under-target rules makes the import-discovered suite run 21 tests with four failures and zero errors: one applied mutation, zero survivors under discovery. The direct-entry path omits those regressions.

This does not dispute the other session's reported 21-test run or its mutation kills; import-based runners reach the tests. It is a runner-dependent regression gap, not evidence the current freeze fix is incorrect.

## Expected correction

Move the existing `if __name__ == '__main__': unittest.main()` after all test definitions. No fix applied in this review. Related: docs/issues/2026-09-25-codex-freeze-positive-count-guard.md. Cluster classification pending.
