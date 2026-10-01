---
id: 9502666fed5b53a2
kind: bug
status: archived
title: 'Codex: direct test runner skips FreezeMenuGuard regressions'
tags:
- cluster/declared-not-wired
closed: 2026-09-25
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

Move the existing `if __name__ == '__main__': unittest.main()` after all test definitions. No fix applied in this review. Related: docs/issues/archive/2026-09-25-codex-freeze-positive-count-guard.md. Cluster classification pending.

## Fix

**Fixed in `f0125e0e`, patch-id `f57b7cf16581abab25e3ae878ee25dc8cd7fe146`.** That commit also carries the review file and a pre-registration correction, so the patch-id hashes more than the test move.

- The `if __name__ == "__main__": unittest.main()` guard moved below the last `TestCase`, with a comment saying why it must stay last.
- **Observed before:** `python3 tests/test_stage2_synthetic.py` gave `Ran 16 tests ... OK`.
- **Observed after:** the direct run gives `Ran 21 tests ... OK`, and `python3 -m pytest tests/test_stage2_synthetic.py -q` gives `21 passed`.
- Class: IC-3, `declared-not-wired`, whose members line names this file.

**Guard added 2026-10-01** in `5605ff50f4fa46e3ebec5e00104bf9bbeeb076b7` (`tests/python_test_entry_guard.rs`), as a Rust test so that it runs in the gate, which no Python test does. It checks the mechanism (nothing at column 0 below the first entry guard) and not the count disagreement this note first named; that file's module header states what it does not reach.

## Fix provenance

- **SHA:** `f0125e0e` (`experiments`)
- **patch-id:** `f57b7cf16581abab25e3ae878ee25dc8cd7fe146`

**Recorded 2026-09-28** by session `82cff72e`; the patch-id was re-derived from `f0125e0e` and matches. The direct run still reaches every test: `python3 -m pytest tests/test_stage2_synthetic.py` reported `21 passed` at HEAD (`fd0b4181`). The regression guard is the second pair below. It was observed red on the real defect (a second guard re-created above `FreezeMenuGuard` in this file made the corpus test name this file at the class line), and each of its ten guarded sites was killed by a mutation.

- **SHA:** `5605ff50f4fa46e3ebec5e00104bf9bbeeb076b7` (`experiments`), the regression guard
- **patch-id:** `d176a55a57ee1c133eac5caddab11cec83f94b16`
