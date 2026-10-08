---
id: d282e217922937c5
kind: bug
status: fixed
title: 'BUG: twenty-seven tests/test_*.py files are run by no gate, CI job or hook, and three do not collect on this machine'
tags:
- tests
- gate
- python
- cluster/declared-not-wired
closed: 2026-10-08
opened: 2026-10-06
owner: marius
related:
- docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md
severity: low
---

# BUG: twenty-seven `tests/test_*.py` files are run by no gate, CI job or hook, and three of them do not even collect on this machine

## Summary

The repository has 27 Python test files under `tests/`. `scripts/gate.sh` runs none of them. The CI workflow has no Python test lane. A suite that nothing runs cannot fail, so a regression in the scripts it covers is invisible to the gate.


### Re-verified 2026-10-08 — suite result stands; the CI-absence claim was false

Enumerated every file matching `Path("tests").glob("test_*.py")`: 27 files. Ran `PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider --continue-on-collection-errors tests/test_*.py` on the shared checkout: **980 passed, 3 collection errors, 421 subtests passed in 33.76s**, exit 1. The same three modules could not collect: step4 and step5 need `sklearn`, training needs `torch`. No collected test failed. This result does not validate the uncollected modules.

Correction: `.github/workflows/ci.yml` exists, including at the original report's commit `10e935e3` (`git ls-tree` confirmed the path there). The original claim that there was no CI workflow was false. Inspection of the workflow, gate and hook found no pytest invocation; the missing Python runner remains real.

Recommended next step: add an explicit lightweight Python lane and give the three dependency-heavy suites a declared runner/environment. Do not silently call a partial run the whole Python suite. No lane or dependency installation was added in this investigation.

## Symptom (Effect)

Measured 2026-10-06 at `10e935e3`, with `python3 -m pytest -q -p no:cacheprovider --continue-on-collection-errors tests/test_*.py`, run in a scratch worktree with the system Python 3.14 and pytest 9.1.1:

```
ERROR tests/test_phase1b_step4.py
ERROR tests/test_phase1b_step5.py
ERROR tests/test_phase1b_training.py
980 passed, 3 errors, 421 subtests passed in 39.26s
```

The three errors are `ModuleNotFoundError: No module named 'sklearn'` (the first two, through `docs/evals/data/2026-09-24-rule-tell/phase1b/step4.py:39`) and `No module named 'torch'` (the third, through `docs/evals/data/2026-09-24-rule-tell/stage3/train_arm.py:32`).

So the suites that can be collected here pass. The finding is "never run", not "red".

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

1. Run `ls tests/test_*.py | wc -l`. It prints 27.
2. Run `grep -n -i -E "python|pytest" scripts/gate.sh`. It prints nothing.
3. Inspect `.github/workflows/ci.yml`: it exists, but has no pytest invocation. The original report incorrectly claimed the workflows directory was absent (corrected 2026-10-08).
4. Run the pytest command above in a scratch worktree.

## Environment

Linux, Python 3.14, pytest 9.1.1, no `sklearn` or `torch` in the system site-packages. The prompt-engineering venv at `~/work/claude/prompt-engineering/.venv` is what several module headers name (for example `scripts/measure/join.py:7`). It was not checked here.

## Root cause

No lane was ever added for them. The repository says so itself: `tests/python_test_entry_guard.rs:1-20` states "No CI job, gate lane or hook runs `tests/test_*.py`; they are run by hand." That file exists because of it: its guard is a Rust test, so that it runs on every gate.

Measured 2026-10-06 for the gate and the pytest run. The open bug `docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md` also records the count, in a "Correction to the triage and an important follow-up" paragraph, and notes the sweep brief said 29.

## Evidence

The pytest summary above. `tests/python_test_entry_guard.rs` is the only gate-run check on these files, and it reads their text; it does not run them.

## Hypotheses tried

1. **Hypothesis:** the suites have rotted. **Test:** run them. **Verdict:** rejected for the 24 collectable files. 980 tests pass. The three that cannot be collected were not run.

## Fix

Implemented and verified on experiments on 2026-10-08. `scripts/python-tests.py` discovers every top-level `tests/test_*.py` and partitions it into light/heavy/all lanes. New files enter the light lane automatically; the three ML files are explicitly classified and their absence fails every lane. Missing dependencies, empty selection, pytest failures and collection errors fail loudly.

`scripts/gate.sh` runs light Python first and retains the four Rust commands in their original order, with default Cargo last even after a Python failure. Existing CI now has `python-light` and `python-heavy` jobs with declared requirements and CPU Torch. Contributor instructions describe both lanes. The full local gate exited `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

**SHA (experiments):** `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34`.

**patch-id:** `7e1a3a2858f3c27a5b777db567352e50b31b6d44`.

## Tests added

The 18 tests in `tests/test_python_runner.py` exercise light/heavy/all membership, automatic discovery, missing heavy inventory, empty selection, missing dependencies, pytest assertion/collection failures, and propagation of each gate lane's failure while default Cargo remains last. Four applied mutations (membership inversion, missing-inventory bypass, swallowed pytest status, ignored Python gate status) were all killed.

Local light lane: 998 passed and 421 subtests passed over 25 selected files. Heavy lane: 134 passed over three selected files. All lane: 1132 passed and 421 subtests passed over 28 selected files, with none excluded. Heavy/all used the existing Python 3.12.12 interpreter at `/home/marius/work/claude/jevk5/.venv/bin/python`, `PYTEST_DISABLE_PLUGIN_AUTOLOAD=1`, and `PYTHONPATH=/tmp/codescout-python-pytest-deps-oct08` pointing to installed pure-Python pytest dependencies; no profile or dependency installation was changed. Commands were `scripts/python-tests.py --lane heavy` and `--lane all`.

The CI workflow was parsed locally; GitHub execution has not been observed.

## Workarounds

Run `python3 -m pytest tests/test_*.py` by hand before changing anything under `scripts/measure/` or `docs/evals/data/`.

## Resume

N/A — all original 27 files plus the new runner regression suite have execution paths and passed locally. Remote CI execution has not been observed; no master promotion is required for archive.

## Fix provenance

- **SHA:** `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34` (`experiments`)
- **patch-id:** `7e1a3a2858f3c27a5b777db567352e50b31b6d44`

## References

- `tests/python_test_entry_guard.rs` header.
- `docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md` (open): records the same count inside a paragraph about another bug.
- `docs/issues/archive/2026-09-26-a-python-only-task-under-scripts-published-a-red-rust-gate.md`: its plan said "No CI lane runs them".
- Cluster `IC-3`: the suites declare checks that no gate reaches.
