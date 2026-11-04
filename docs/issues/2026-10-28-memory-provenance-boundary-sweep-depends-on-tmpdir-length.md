---
id: 649a1408a75ca3cc
kind: bug
status: mitigated
title: Memory provenance boundary sweep depends on TMPDIR path length
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- testing
- memory
closed: 2026-11-04
opened: 2026-10-28
severity: medium
unverified: Linux only. The threshold between 430 and 550 bytes was not narrowed. The commit that widened the sweep targeted Windows path spelling, not this bug, so no test was written for the long-TMPDIR premise itself. macOS and Windows runs of this test with a long TMPDIR were not made.
---

# BUG: memory provenance boundary sweep depends on TMPDIR path length

## Summary

The unchanged test a_memory_read_from_the_other_layout_counts_its_provenance_keys_in_the_gate fails with a long TMPDIR because its fixed content-size sweep contains no inline result. With TMPDIR=/var/tmp, the same lean binary passes this exact test.

## Class fit

This is cluster/repro-env-diverges-from-gate-env: an ambient launcher path changes a test's premise while production bytes stay identical. It resembles the archived emission-byte-ceiling fixture defect, but this case has a fixed sweep interval around memory response provenance rather than a guide-output ceiling.

## Symptom (Effect)

With TMPDIR=/var/tmp/codescout-producer-gate-yla5quaz, both full-gate Rust lanes fail at src/tools/memory/tests.rs:3119:

```text
the sweep must cross the edge: 0 inline, 191 buffered
```

The 191 count is the test's observed complete sweep, not a model/task sample.

## Reproduction

At research worktree HEAD 0cb5ca9f48791ff81f769de48b4679d2f78a7bfa, launch ./scripts/gate.sh with the long TMPDIR above. Both lean and default logs retain the failure.

Control: run the same compiled lean binary with TMPDIR=/var/tmp and the exact test filter. Observed 1 passed, 0 failed.

## Environment

Linux; research worktree /tmp/codescout-real-task-20261024. A long private /var/tmp directory was chosen to avoid an unrelated /tmp/.git ancestor marker. No source change separated the failing and passing controls.

## Root cause

src/tools/memory/tests.rs::a_memory_read_from_the_other_layout_counts_its_provenance_keys_in_the_gate sweeps content sizes 9800 through 9990 inclusive and asserts that both inline and buffered branches were observed. The response carries resolved_from and write_target paths. A longer fixture root changes their serialized contribution; in the observed long-path environment, every sampled response is buffered. The test still checks that no returned response exceeds the inline limit, but its branch-crossing premise fails.

This is an environment-dependent fixture interval, not an observed memory-tool response-size regression.

## Evidence

Main checkout .codescout/resident-private/producer-status-pilot-20261028 contains isolated-gate.log, isolated-gate-result.json, short-temp-memory.log and short-temp-probe.json.

## Hypotheses tried

Shortening TMPDIR to /var/tmp makes the exact test pass without changing source or the binary.

## Fix

Not a fix aimed at this bug. Commit `7c468bc04` (2026-11-01, for Windows paths) widened the sweep in `a_memory_read_from_the_other_layout_counts_its_provenance_keys_in_the_gate` from `9_800..=9_990` (191 values) to `9_000..=9_990` (991 values), because a Windows path is longer and its backslashes are doubled in JSON. The same widening lets the test cross the inline/buffered edge with a long `TMPDIR`. The sweep start is still a fixed number, so a long enough root still pushes every sampled response into the buffered branch.

A future repair should derive the interval from the actual serialized path contribution, or build a controlled short root. Do not remove the branch-crossing assertion.

## Tests added

None for this bug. The existing test is the one that fails; it was widened by `7c468bc04`.

Measured 2026-11-04 on `experiments` (`scripts/with-slot.sh cargo test --lib tools::memory::tests::a_memory_read_from_the_other_layout`, `XDG_RUNTIME_DIR` as set, `TMPDIR` of the given length in bytes):

| TMPDIR bytes | result |
|---|---|
| 41, 90, 150, 370, 430 | passes |
| 550, 610, 670, 790, 1150 | fails: `the sweep must cross the edge: 0 inline, 991 buffered` |

So the threshold lies between 430 and 550 bytes; it was not narrowed further. The failing case of the record (41 bytes, at the research worktree HEAD `0cb5ca9f`, sweep `9_800..=9_990`) does not reproduce on `experiments`.

## Workarounds

TMPDIR=/var/tmp passes this exact test on the measured host.

## Resume

The reported case is gone on `experiments`; a residual remains above about 430 to 550 bytes of `TMPDIR`, which no real launcher seen here produces (the 41-byte path of the record was the longest reported). Decide whether to derive the interval or leave the residual. If a repair is made, check that a mutation dropping the provenance keys from the gate's accounting is still killed, as the record's earlier Resume asked.

## Fix provenance

- **SHA:** `7c468bc04` (`experiments`)
- **patch-id:** `cb9ab2c22533fcf3147f0ae27d9c8c630b7d1f17`

The commit is an ancestor of `HEAD` (checked 2026-11-04). It was written for another reason, so its message does not cite this bug.

## References

- The fresh pilot and gate record exists only on the research branches, in commit `1023edc17` (`docs/research/2026-10-28-fresh-producer-status-pilot.md`); it is not on `experiments`. Read it with `git show 1023edc17:docs/research/2026-10-28-fresh-producer-status-pilot.md`.
- The earlier guide emission path-length defect: `docs/issues/archive/2026-09-08-the-emission-byte-ceiling-measures-the-fixtures-tempdir-path-length.md`.
