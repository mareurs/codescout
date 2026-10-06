---
id: '73064c99e0c6cbb6'
kind: bug
status: open
title: 'BUG: twenty-seven tests/test_*.py files are run by no gate, CI job or hook, and three do not collect on this machine'
tags:
- tests
- gate
- python
- cluster/declared-not-wired
opened: 2026-10-06
owner: marius
related:
- docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md
severity: low
---

# BUG: twenty-seven `tests/test_*.py` files are run by no gate, CI job or hook, and three of them do not even collect on this machine

## Summary

The repository has 27 Python test files under `tests/`. `scripts/gate.sh` runs none of them. There is no CI workflow. A suite that nothing runs cannot fail, so a regression in the scripts it covers is invisible to the gate.

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
3. Run `ls .github`. There is no `workflows/` directory: `copilot-instructions.md`, `ISSUE_TEMPLATE` and `pull_request_template.md` only.
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

Not started. Options, not exclusive:

- Add a lane to `scripts/gate.sh` that runs the 24 collectable suites. About 40 s.
- Decide where the three heavy ones run (a venv with `sklearn` and `torch`), or mark them as hand-run in their module headers.
- Make the lane skip, loudly, when the dependencies are absent, as `CODESCOUT_SKIP_ONNX_TESTS` does for the embed tests.

## Tests added

N/A — not fixed.

## Workarounds

Run `python3 -m pytest tests/test_*.py` by hand before changing anything under `scripts/measure/` or `docs/evals/data/`.

## Resume

Decide whether the Python suites belong in the gate. The lane is cheap for 24 of 27.

## References

- `tests/python_test_entry_guard.rs` header.
- `docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md` (open): records the same count inside a paragraph about another bug.
- `docs/issues/archive/2026-09-26-a-python-only-task-under-scripts-published-a-red-rust-gate.md`: its plan said "No CI lane runs them".
- Cluster `IC-3`: the suites declare checks that no gate reaches.
