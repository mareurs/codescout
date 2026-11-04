---
id: 23fb46486562fd7b
kind: bug
status: fixed
title: Attribution guard tests hit the real-history diagnostic timeout
tags:
- cluster/unclassified
closed: 2026-11-04
opened: 2026-11-01
severity: medium
unverified: One run each on experiments (0.81 s and 0.77 s), not run under load or repeated. The real-history engine timing (30.6 s on 150 files on the research branch) was not re-measured on experiments; the history cache and scan floor came later and were not re-checked here. The research-branch failures were not reproduced. Linux only.
---

# BUG: Attribution guard tests hit the real-history diagnostic timeout

## Summary

The unchanged U branch's full gate failed attribution availability preconditions. The 150-file case also failed in isolation; the directory-swap case passed in isolation. A direct engine run on the same-shaped 150-file fixture returned a complete diagnostic in30.645s, beyond the shipped default30s ceiling. This supports a timeout explanation for that case, not a proved cause for every gate failure.

## Class fit

Unclassified environment-sensitive guard availability failure. Tests ask the real local history engine for a diagnostic and receive None; this is not a model-evaluation outcome or a claim that the attribution security guard is bypassed.

## Symptom (Effect)

`the same engine answers for 150 files`

`world-writable: precondition, the engine answers on a clean directory; got None`

Full gate: PYTHON_LIGHT0, FMT0, CLIPPY0, LEAN101, DEFAULT101. LEAN failed the directory-swap test; DEFAULT failed that test and the150-file diagnostic test.

## Reproduction

Worktree /home/marius/.codex/worktrees/9369/codescout, HEAD e3daa380d87dcf663cc0f32e099afe199eba2bee, no Rust source changes. Run ./scripts/gate.sh --check under a fresh short TMPDIR. Exact targeted names and commands are retained in /home/marius/work/claude/codescout/.codescout/resident-private/fixed-checker-transfer-20261101/gate-targeted-results.json.

## Environment

2026-11-01 local real Claude/Codex history corpus, Python3.14, leased Cargo target. Full gate ran with default test concurrency and default attribution ceiling. Actor model calls were independently executing; latency is not a randomized comparison.

## Root cause

AttributionEnv::default uses30 seconds (src/tools/run_command/attribution.rs). wip_author_diagnostic_in returns None on timeout or child wait error. The tests interpret missing diagnostic as a failed availability precondition. Cause of each exact missing result is not directly instrumented.

Measured2026-11-01: isolated150-file test failed in55.57s after its small control and large case; isolated directory-swap passed in86.14s. Direct python3 -I scripts/attribute-red.py on a150-file fixture exited0 with36278 stdout bytes, no stderr, in30.645098272012547s under a120s observation ceiling. This probe is above the default deadline but is a separate execution, so per-run causation remains an inference.

## Evidence

Private root /home/marius/work/claude/codescout/.codescout/resident-private/fixed-checker-transfer-20261101: gate.log, gate-targeted-many-files.log, gate-targeted-swapped-directory.log, gate-targeted-results.json, attribution-timing-probe/result.json and full stdout/stderr/input. Original failures are retained.

## Hypotheses tried

1. Pure whole-suite contention: insufficient alone, because the150-file exact test also failed in isolation.
2. Real-history engine crosses30s: supported by an actual successful direct probe taking30.645s; not a direct trace of the failed child.
3. Directory-swap guard bypass: not established; failure was its clean-directory precondition and the exact isolated test subsequently passed.

## Fix

Already on `experiments`, under bug `20927ccfe293c34e`, which this record, written in a research worktree on 2026-11-01, did not know. For the tests this record names:

- `a_red_naming_many_dirty_files_carries_a_bounded_wip_authors` (the 150-file test): `421b0f896` (2026-11-01) makes it scan an empty history through `no_history()`, and `handle_successful_output_with` takes an optional `AttributionEnv` so the test can pass it. `4bc4b0960` makes a missing diagnostic FAIL the test when `python3` is present, instead of skipping it.
- `dir_trust::a_directory_swapped_in_after_first_use_is_not_run`: `b33315316` (2026-10-31), same method. Record `87cfa556ce471ef7` covers it.

The product side of this record's root cause, a real-history scan that took 30.6 s against the 30 s default ceiling, is the product limit tracked in `20927ccfe293c34e`, which since then added history caching (`e8b9935c4`) and a scan floor (`14f512875`). Neither this record's operator-knob run (`CODESCOUT_WIP_ATTRIBUTION_TIMEOUT_MS=120000`) nor the 30.6 s probe was repeated here.


## Subsequent verification

The second full gate exited0 after1532s under the explicit process-local four test threads and120000ms attribution timeout. PYTHON_LIGHT, FMT, CLIPPY, LEAN and DEFAULT all returned0. No source/shared-config change or added test-skip flag was used. The original default gate failures remain evidence; the directory-swap default-environment cause remains incompletely established. This is a successful configured verification run, not a production fix.

## Tests added

None in this change. Measured 2026-11-04 on `experiments`: `tools::run_command::tests::a_red_naming_many_dirty_files_carries_a_bounded_wip_authors` passes in 0.81 s (1 passed, 0 failed), and the directory-swap test in 0.77 s. This record saw the 150-file test fail in 55.57 s and the directory-swap test pass in 86.14 s on the research branch.

## Workarounds

Use the explicit machine-appropriate attribution timeout knob for from_env callers. Reduced test concurrency may reduce contention; that cause is not established by the isolated probe.

## Resume

Nothing open on `experiments` for these two tests. The real-history timing of the engine itself is tracked by `20927ccfe293c34e`.

## Fix provenance

- **SHA:** `421b0f896` (`experiments`)
- **patch-id:** `6c0483bc99a64b23305f96606bcaaa662b9d2e55`

The commit is an ancestor of `origin/experiments`. Also relevant: `4bc4b0960`, patch-id `8d5ad2c56ea2e3d8c516bd5f3ae55c3af8ebe6c1`.

## References

- `src/tools/run_command/attribution.rs`, `src/tools/run_command/tests.rs`.
- Bug `20927ccfe293c34e` and duplicate `87cfa556ce471ef7`.
