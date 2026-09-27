---
id: '0614080cf4efbb1f'
kind: bug
status: fixed
title: 'Codex: Phase1b shell runners return success after child failure'
owners:
- codex
tags:
- phase1b
- eval-integrity
- cluster/record-asserts-an-unchecked-completion
closed: 2026-09-27
opened: 2026-09-27
severity: medium
unverified: Working-tree fix verified with focused shell regression tests and applied mutations; integration and a fresh whole-repository gate are not claimed.
---

## Summary

The Stage-2 shell wrappers lose child exit failures. `lanes.sh` ends each background lane with a successful echo, uses bare `wait`, and ends with another echo. `step4.sh` and `step5.sh` record intermediate failures but end with successful echoes; their final common-menu/summary child failures do not even set `fail=1`.

## Reproduction

At HEAD a5372e000a525f2f8e59d416cf690781598ac02c, run `python3 -m unittest discover -s tests -p test_phase1b_stage2_shell.py -v`. Tests run disposable copies of the actual wrappers with only machine-local path bindings changed and a fake Python child. No model calls or live run writes. Observed: 8 tests, 7 failing assertions; success controls for all three scripts pass. Every injected child exits 23; every wrapper incorrectly exits 0. Common/summary failures additionally print `DONE fail=0`.

## Impact and historical result

Automation observing shell status can announce completion or proceed after a failed run. This does not explain the recorded classifier failure: the actual six training statuses, eighteen score/calibration statuses, common status, nine gate-execution statuses, and summary status in the saved run logs are all zero. A completed gate execution is distinct from a classifier passing that gate.

## Fix and verification

Fixed in the working tree: preserve per-lane failures through explicit PID waits; propagate every score/calibration/gate and final common-menu/summary failure to the wrapper exit status. Independent runs still finish. A completed classifier gate whose scientific verdict is negative remains a successful execution; its result JSON carries that verdict.

`tests/test_phase1b_stage2_shell.py`: 8 tests pass. The historical control flow at a5372e00 produces 7 failures; all three success controls pass. Nine applied mutations in disposable wrapper copies: 9 killed, 0 survived, 0 test errors. Bash syntax and Python compilation checks pass.

Evidence and reproduction: `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-stop-review/shell-verification.json` and `verify_shell_fixes.py`. Review: `docs/research/2026-09-27-codex-phase1b-stop-review.md`.

No models, training jobs, held-out evaluations, or full Rust gate rerun. Changes are uncommitted; no fix SHA or patch-id exists yet. The historical outputs are unchanged. Archive after integration and the applicable repository gate.

## References

- `docs/evals/data/2026-09-24-rule-tell/phase1b/stage2/lanes.sh`
- `docs/evals/data/2026-09-24-rule-tell/phase1b/stage2/step4.sh`
- `docs/evals/data/2026-09-24-rule-tell/phase1b/stage2/step5.sh`
- `tests/test_phase1b_stage2_shell.py`
