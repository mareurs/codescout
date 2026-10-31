---
id: e1515917e4215c93
kind: bug
status: mitigated
title: The gate's wall time is recorded nowhere, and dev-profile debug info makes 300-582 MB test binaries
owners:
- marius
tags:
- cluster/instrument-omits-the-dimension-that-grows
opened: 2026-10-30
closed: 2026-10-30
severity: low
unverified: 'Gate wall time is now printed per lane but only one full run is recorded; the debug-info change was measured, not applied (operator decision).'
---

# BUG: The gate's wall time is recorded nowhere, and dev-profile debug info makes 300-582 MB test binaries

## Summary

No full-gate wall time is recorded in any tracker or doc; the only figure is the lean lane at "~20s incremental" (`docs/conventions/gate-ordering.md:214`). `[profile.dev] debug = 1` (`Cargo.toml:238-240`) produces test binaries of 300-582 MB each (48 in one slot, 23 GB per slot). Filed at the operator's request (2026-10-30) so the gate's time can be measured and tracked.

## Class fit

`cluster/instrument-omits-the-dimension-that-grows`: the gate reports pass/fail per lane but not its duration, which is the dimension that grows with the test count. It differs from other members because the missing dimension is the gate's own cost.

## Symptom (Effect)

No recorded duration. Slot sizes per the retrieval explorer of this session (not re-checked by the controller).

## Reproduction

`./scripts/gate.sh` with `time`, plus `cargo build --timings` per lane.

## Environment

64 cores, 125 GB RAM, mold linker (`.cargo/config.toml`), sccache with 1 hit of 260 requests (incremental builds are not cacheable).

## Root cause

N/A — this is a missing measurement plus candidate changes, not a defect with a mechanism.

## Evidence

`Cargo.toml:238-240` read 2026-10-30: `opt-level = 0`, `debug = 1`.

## Hypotheses tried

None.

## Fix

Done on branch `worktree-agent-ac59e41db7b0d1eb3`, SHA `59d870c9d` (patch-id `902b9b617a5970f593b4138330fdaf5be0d302e6`): `scripts/gate.sh` now prints `GATE SECONDS -> TOTAL=.. PYTHON=.. FMT=.. CLIPPY=.. LEAN=.. DEFAULT=..` after the tree-size lines. Exit-code handling and lane order are unchanged (`;` throughout).

Gate wall time, first measurements (2026-10-30/31, this worktree, slot partly warm, shared machine, other sessions running): `GATE SECONDS -> TOTAL=1065 PYTHON=167 FMT=5 CLIPPY=81 LEAN=272 DEFAULT=540` (red run) and `TOTAL=968 PYTHON=117 FMT=5 CLIPPY=83 LEAN=280 DEFAULT=483` (green run). The default lane is the largest share; the lib test binary alone ran 426 s in the first run. Later commit `299c5e78b` fixed the two reds the grep change caused.

Measured 2026-10-30, cold `cargo test --workspace --no-run` in a fresh target dir, 64 cores, mold, machine shared with other sessions (timings are noisy), the profile switched with `--config` so `Cargo.toml` was not touched:

| `profile.dev.debug` | build | tree | 51 test exes total | largest exe | `target/debug/codescout` |
|---|---|---|---|---|---|
| `1` (current) | 192 s | 13 G | 7.96 GB | 584 MB | 459 MB |
| `"line-tables-only"` | 190 s | 9.3 G | 4.71 GB | 401 MB | 326 MB |

So `line-tables-only` cuts disk by 29-41 % and does not change the cold build time (190 vs 192 s, within noise). The earlier "300-582 MB, 48 binaries" figure is confirmed in order of magnitude (51 exes, 584 MB max). `Cargo.toml` is NOT changed: the benefit is disk only, and it costs variable-level debug info (breakpoints and backtraces keep line numbers, `dbg!`-style inspection in a debugger loses locals); that is the operator's call. `[profile.dev.package."*"] debug = false`, the lean lane in a second slot, nextest and the slow-test list were not measured.

## Tests added

N/A — no code defect; the deliverable is a measurement.

## Workarounds

N/A

## Resume

Run one gate with `--timings` per lane and per-binary test durations; record the totals in a tracker; repeat with `debug = "line-tables-only"`.

## References

- `scripts/gate.sh:170-173`, `scripts/slot-pool.sh:45`
