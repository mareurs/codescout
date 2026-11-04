---
id: 87cfa556ce471ef7
kind: bug
status: fixed
title: Attribution directory-swap test intermittently returns no diagnostic in the default gate
owners:
- marius
tags:
- cluster/unclassified
- testing
closed: 2026-11-04
opened: 2026-10-28
severity: medium
unverified: One run on experiments (0.77 s). Not run under load or repeated. The security assertion (the swapped-in script does not run) was not mutation-tested here. Linux only. The research-branch failure itself was not reproduced.
---

# BUG: Attribution directory-swap test intermittently returns no diagnostic in the default gate

## Summary

The default library gate failed its existing directory-swap regression test with a missing fallback diagnostic; the exact test passed once in isolation afterward.

## Class fit

The failing behavior is measured but its mechanism is not established, so cluster/unclassified preserves the uncertainty. Unlike a confirmed security regression, the observed test failure did not report execution of the hostile marker.

## Symptom (Effect)

At src/tools/run_command/attribution.rs:1338, the world-writable variant reported: "the refusal must fall back, not go silent or run it; got None". The assertion that the hostile marker was absent had already passed.

## Reproduction

Run ./scripts/gate.sh --check in the named worktree. One observed default library result was 7880 passed, 2 failed, 14 ignored. This is an intermittent observation, not a deterministic recipe. The named test tools::run_command::attribution::tests::dir_trust::a_directory_swapped_in_after_first_use_is_not_run passed alone in 59.99 seconds; log gate-probe-a_directory_swapped_in_after_first_use_is_not_run.log.

## Environment

Observed 2026-10-28 in research worktree /tmp/codescout-real-task-20261024 at f41b306b4c17bdb9bddc5cecf3e1baead8640ee8. Source/build input hashes were unchanged throughout the gate. The gate used a private short /var/tmp directory and its leased Cargo pool.

## Root cause

The engine this test calls, `wip_author_diagnostic_in`, ran with the default attribution env: it scanned every transcript under `$HOME/.claude*` and `$HOME/.codex*` against a 30 s ceiling, and returned `None` on a timeout. On a busy machine that scan took 16 to 25 s, and under load it crossed the ceiling, so the test's own precondition (the engine answers on a clean directory) or its fallback assertion saw `None`. Established by bug `20927ccfe293c34e` (its `## Root cause` for the attribution tests); this record's own isolated pass of 59.99 s fits a scan of that length.

## Evidence

Private evidence: .codescout/resident-private/independent-transfer-20261028/gate.log, gate-result.json, gate-isolated-probes.json and the named isolated probe log. These are retained locally in the main repository and excluded from Git.

## Hypotheses tried

- Running the exact named test alone with a short private /var/tmp directory: passed once.
- Parallel load and filesystem resource pressure are unproven hypotheses. The same session separately observed /tmp quota exhaustion during experiment preparation, but this does not establish the cause of a /var/tmp test failure.

## Fix

Already on `experiments`, under bug `20927ccfe293c34e`, which this record, written in a research worktree on 2026-10-28, did not know. Commit `b33315316` (2026-10-31) makes four attribution tests, this one included, scan an empty history through `no_history()` in `src/tools/run_command/attribution.rs`: the same engine and the same output, with `FILE_PROVENANCE_ROOTS` pointing at an empty directory. The test's hostile-marker and fallback assertions are unchanged. This record is a duplicate of that work.

## Tests added

None in this change. Measured 2026-11-04 on `experiments`: `tools::run_command::attribution::tests::dir_trust::a_directory_swapped_in_after_first_use_is_not_run` passes in 0.77 s (1 passed, 0 failed), against the 59.99 s this record saw in isolation on the research branch.

## Workarounds

Keep the failed full-gate result visible. An isolated pass does not turn the full gate green.

## Resume

Nothing open on `experiments` for this test. A deadline on a REAL history corpus is the product limit that `20927ccfe293c34e` still tracks.

## Fix provenance

- **SHA:** `b33315316` (`experiments`)
- **patch-id:** `3449c12dd208615c1fcb7a069a59b015c0be17c9`

The commit is an ancestor of `origin/experiments`. I did not run a loaded or 60-iteration sample of this test.

## References

- Bug `20927ccfe293c34e`: the attribution tests and the product limit.
- The existing hardening change and test are in `2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md`. Its security defect is not asserted to have recurred.
