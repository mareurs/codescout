---
id: e817931ef9d51dd0
kind: bug
status: open
title: 'BUG: two Windows CI tests flake on wall-clock/race assumptions — one is skip-listed on wine but gates on MSVC'
tags:
- windows
- ci
- flake
- test-portability
- timing
closed: ''
opened: 2026-08-07
owner: marius
related:
- docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md
severity: medium
---

# BUG: two Windows CI tests flake on wall-clock/race assumptions

## Summary

Two single-test failures turned a 15-job CI run red with no code defect, at the exact moment a
440-commit promotion was waiting on that gate. Re-running only the two failed jobs, with zero
code change, produced 15/15. Both tests assert on real elapsed time or on a background
process's output being ready, and one of them is already `--skip`-listed on the wine job while
still gating on `windows-latest`.

## Symptom (Effect)

Run `31151333288` on `2d824f15`, first attempt — 13 success / 2 failure, one test each:

```
Windows-gnu cross (MinGW + wine)
---- lsp::budget_tests::cold_start_over_budget_returns_none_but_keeps_warming stdout ----
panicked at src/lsp/mod.rs:133:9:
must not wait out the cold start
test result: FAILED. 3287 passed; 1 failed; 11 ignored; 12 filtered out; finished in 28.81s

Test (windows-latest / default)
---- tools::run_command::tests::background_command_with_quotes_captures_output stdout ----
panicked at src\tools\run_command\tests.rs:3763:5:
background command output not captured
test result: FAILED. 3299 passed; 1 failed; 11 ignored; 0 filtered out; finished in 138.34s
```

Both jobs reported `steps=12`, so they executed — this is not the outage attrition that
produced three other red runs the same day.

## Reproduction

Not reliably reproducible by construction — that is the bug. Observed once on
`gh run 31151333288` attempt 1, then **passed on attempt 2** via
`gh run rerun 31151333288 --failed`, no code change, same commit. Both tests also passed on
all four earlier 15/15 runs (`6348dfad`, `db4b1968`, `de4f7ccd`, `382c3344`) and pass locally
on Linux across all three feature configs.

## Environment

GitHub-hosted runners. `windows-latest` (MSVC) and `ubuntu-latest` running
`x86_64-pc-windows-gnu` under wine. Note the day's Actions major outage had ended and Actions
reported All Systems Operational before this run, so runner scarcity is not the explanation.

## Root cause

**1. `cold_start_over_budget_returns_none_but_keeps_warming`** — `src/lsp/mod.rs:133`.

The test builds a `SlowStart` provider with a 200 ms delay, calls `client_within_budget` with a
**50 ms** budget, and asserts:

```rust
assert!(
    t0.elapsed() < Duration::from_millis(150),
    "must not wait out the cold start"
);
```

The tolerance is therefore **100 ms of scheduler slack** (150 ms ceiling minus the 50 ms
budget) measured in **real** time. Under wine, on a shared runner, 100 ms is not a safe
margin. The test is `#[tokio::test]` with no `start_paused`, so tokio's timers run against the
real clock even though every sleep involved is a tokio timer.

**2. `background_command_with_quotes_captures_output`** — `src/tools/run_command/tests.rs:3763`,
message `background command output not captured`. A race between the spawned background process
writing its output and the assertion reading it.

**The inconsistency that made this bite.** `.github/workflows/ci.yml`'s wine step `--skip`s
`background_command_with_quotes_captures_output` by name, as one of the 20 pre-existing wine
failures catalogued in WIN-27. The `windows-latest` job does not skip it. So the same test is
exempted on one Windows job and load-bearing on the other.

## Evidence

### WIN-27 named a different test as the only real-Windows case

`docs/trackers/windows-platform-support.md` (WIN-27, 2026-07-02) inventories 20 wine failures
and states: *"`validate_prune_request_gates` is the one exception, also red on real Windows
(windows-latest MSVC)."* This run makes
`background_command_with_quotes_captures_output` the **second** exception — new information,
not a known state. Full inventory and un-skip protocol:
`docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`.

### The budget test is not on any skip list

`cold_start_over_budget_returns_none_but_keeps_warming` does not appear in the wine `--skip`
list, so its wine failure is new relative to the WIN-27 baseline rather than pre-existing.

### Re-run is the discriminator

Attempt 1: 13 success / 2 failure. Attempt 2 (failed jobs only, same commit): **15/15**. A
deterministic defect cannot pass an unchanged re-run.

## Hypotheses tried

1. **Hypothesis:** the failures were caused by the day's code changes (audit_doc_refs gitignore
   cap, bug-status default, kotlin NMT flag, clippy fix).
   **Test:** check which files each failing test exercises against the diff.
   **Verdict:** rejected — neither test touches any changed file, and both passed on the
   unchanged re-run.
2. **Hypothesis:** outage attrition, like the three other red runs that day.
   **Test:** read each job's `steps` count. **Verdict:** rejected — both failing jobs report
   `steps=12`, meaning they executed; outage-attrition jobs reported `steps=0`.
3. **Hypothesis:** the budget test's margin is generous and something really did block.
   **Test:** read the test. **Verdict:** rejected — the margin is 100 ms of real time, which is
   within normal emulated-runner jitter.

## Fix

Not yet implemented.

**For the budget test — use tokio's virtual clock.** Both the mock's delay
(`SlowStart::get_or_start` → `tokio::time::sleep(self.delay)`) and the test's own wait
(`tokio::time::sleep(Duration::from_millis(250))`) are tokio timers, so
`#[tokio::test(start_paused = true)]` makes the whole test deterministic. Auto-advance moves
the clock to the **next** scheduled timer, which is the 50 ms budget rather than the 200 ms
cold start, so the `None` return is still exercised for the right reason. `t0.elapsed()` uses
`std::time::Instant`, which tokio does **not** virtualise — so the elapsed-time assertion keeps
measuring real time and becomes trivially satisfied, which is exactly the robustness wanted:
it still proves the call did not block, without depending on scheduler luck.

**For the background-output test —** replace the fixed wait with a bounded poll on the captured
output (deadline plus small sleep, fail on deadline). The assertion should be "output arrived
within N seconds", not "output arrived by the time we looked".

**Decide the skip-list inconsistency deliberately.** Either skip
`background_command_with_quotes_captures_output` on `windows-latest` too — and record it as a
second real-Windows failure against WIN-27's inventory — or fix it and un-skip it on wine. The
current split is the worst option: exempted where it was noticed, gating where it was not.

## Tests added

None yet — status is `open`. When the fix lands, the regression guard is that the tests pass
deterministically: for the budget test, that no assertion depends on wall-clock slack; for the
background test, that the wait is a bounded poll rather than a fixed sleep.

## Workarounds

`gh run rerun <run-id> --failed` re-runs only the failed jobs and preserves the passing ones,
which is what produced 15/15 here. Cheap, but it means the gate is not trustworthy on first
read — a red run has to be triaged by job `steps` count and test name before it means anything.

## Resume

Apply the `start_paused = true` change to
`cold_start_over_budget_returns_none_but_keeps_warming` in `src/lsp/mod.rs` and confirm the
test still fails when `client_within_budget`'s budget logic is mutated (otherwise virtual time
has made it vacuous). Then convert
`background_command_with_quotes_captures_output` in `src/tools/run_command/tests.rs` to a
bounded poll, and resolve the wine-vs-MSVC skip-list split in `.github/workflows/ci.yml`.

## References

- `src/lsp/mod.rs` — `budget_tests`, `SlowStart`
- `src/tools/run_command/tests.rs` — `background_command_with_quotes_captures_output`
- `.github/workflows/ci.yml` — the wine job's `--skip` list
- `docs/trackers/windows-platform-support.md` — WIN-27 baseline, and WIN-30 for this entry
- `docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md` — the 20-failure inventory

