---
kind: bug
status: fixed
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-09-26
opened: 2026-09-26
owner: marius
related: []
severity: medium
---

# BUG: a Python-only task under scripts/ published a red Rust gate for three hours

## Summary

5897befe (system1 measurement, Task 4 fix round 1) added `scripts/measure/README.md`
citing a script by its absolute path under a personal home directory.
`tests/committed_paths.rs::no_tracked_script_hardcodes_a_personal_home_path` scans
every TRACKED file under `scripts/`, Markdown included, so from 2026-09-26 12:10
to 15:11 (+0300) the test was red for every session gating in this checkout. The task that
introduced it was Python-only and verified itself with pytest, which never runs
that test.

## Symptom (Effect)

`cargo test --workspace` (both gate lanes) fails in `committed_paths`, naming
`scripts/measure/README.md:20`. Because `cargo test` is fail-fast across
binaries, every integration target ordered after `committed_paths` was hidden
too, so a peer's gate result in that window says nothing about those targets.

## Reproduction

`scripts/with-slot.sh cargo test --test committed_paths` at any commit from
5897befe up to, but not including, ee412408.

## Environment

codescout `experiments`, shared checkout with several concurrent sessions.

## Root cause

The task's verification environment was not the gating environment. The plan's
Python tasks say their tests are the only evidence ("No CI lane runs them") and
run pytest alone. But committing under `scripts/` also changes the input to a
Rust test, which the task never ran. The file that tripped it was prose, not
code, so "a Python task" did not suggest a Rust gate either.

## Evidence

The Task 3 fix round's `./scripts/gate.sh` run (FMT=0 CLIPPY=0 LEAN=101
DEFAULT=101) failed only on this test, and the implementer traced it by blame to
5897befe. After the fix, `committed_paths` reported 6 passed, including its own
controls `the_home_path_scan_discriminates` and
`the_tracked_population_is_not_vacuous`, so the pass is not a vacuous scan.

## Hypotheses tried

None needed. The failure names the file and line.

## Fix

ee412408 — patch-id `1b8484de675e2aff002a9049862f7a5b7e6fb288`. The citation now
names the script without its absolute path. Process fix (SDD ledger, ruling R39):
every remaining task in that plan that commits under `scripts/` also runs
`scripts/with-slot.sh cargo test --test committed_paths` (~10 s) before it
commits.

## Tests added

None. The existing test caught it; the defect was that it was not run.

## Workarounds

None needed after ee412408.

## Resume

Closed.

## References

- `tests/committed_paths.rs` — `no_tracked_script_hardcodes_a_personal_home_path`
- `scripts/measure/README.md`
- `docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md`
