---
id: e1515917e4215c93
kind: bug
status: open
title: The gate's wall time is recorded nowhere, and dev-profile debug info makes 300-582 MB test binaries
owners:
- marius
tags:
- cluster/instrument-omits-the-dimension-that-grows
opened: 2026-10-30
severity: low
unverified: No gate duration has been measured; the binary sizes come from a subagent and were not re-checked.
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

Measure first. Candidates: `debug = "line-tables-only"` or `[profile.dev.package."*"] debug = false`; the lean lane in a second leased slot (check `gate-ordering.md` and `cli_doc`'s `pinned_binary()` first); fewer test binaries or nextest. Long timeout tests on a binary's critical path: `src/lsp/manager.rs:2950-4485`, `src/lsp/client.rs:2699, 3895, 3928`, `src/server.rs:11708`, `tests/rename_symbol.rs:75` (5 s sleep), `src/librarian/tools/update.rs:1103` (5 s busy_timeout by design).

## Tests added

N/A — no code defect; the deliverable is a measurement.

## Workarounds

N/A

## Resume

Run one gate with `--timings` per lane and per-binary test durations; record the totals in a tracker; repeat with `debug = "line-tables-only"`.

## References

- `scripts/gate.sh:170-173`, `scripts/slot-pool.sh:45`
