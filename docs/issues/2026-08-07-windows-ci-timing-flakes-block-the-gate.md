---
id: e817931ef9d51dd0
kind: bug
status: open
title: 'BUG: two Windows CI tests flake on wall-clock/race assumptions — one is skip-listed on wine but gates on MSVC'
tags:
- cluster/repro-env-diverges-from-gate-env
- windows
- ci
- flake
- test-portability
- timing
closed: ''
last_observed: 2026-08-07
last_verified: 2026-08-26
opened: 2026-08-07
owner: marius
related:
- docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md
severity: medium
---

# BUG: two Windows CI tests flake on wall-clock/race assumptions

## Summary
> **Status: `zombie` as of 2026-08-07, by maintainer decision.** Not observed since, root cause not
> confirmed, and there is no work available: both flakes resolve only by recurring. Kept open in the
> ledger rather than archived, exactly what the `zombie` status exists for
> (`docs/issues/_TEMPLATE.md`). This was the last thing blocking the `experiments` -> `master`
> promotion, and "work harder" could not have closed it — a trigger-gated bug is not reachable by
> effort.
>
> **Re-open trigger — any ONE of these, and this goes straight back to `open`:**
>
> 1. A `windows-latest` job fails on either named test again. Route it per the file's own § Fix
>    rather than re-diagnosing: the wine skip is PERMANENT (no Python launcher under wine, see
>    the `.github/workflows/ci.yml` comment), so an MSVC-side failure is a genuinely different
>    signal from the skip-listed one.
> 2. The same wall-clock assumption appears in a NEW test. The class, not the instance, is what
>    matters — and W-14 (`docs/trackers/release-promotion-session-log.md`) is the general form:
>    a first measurement after idle is a warm-up artifact, and a test that asserts on one is
>    timing-dependent by construction.
> 3. Any test in this repo starts using `std::time::Instant` under `#[tokio::test(start_paused =
>    true)]`. That combination is the trap F-11 found here: `tokio::time::Instant` is virtualised
>    and `std::time::Instant` is not, so virtual-time tests built on the latter are wall-clock
>    tests wearing a deterministic costume.
>
> Nothing about the analysis below is retracted — it stays as the starting point for whoever picks
> this up when it fires.
>
> **Verified 2026-08-26 — stays `zombie`; no trigger has fired.** Checked against CI run
> `32740102144` (`047dd433`, 2026-08-24), the most recent run on `experiments`:
>
> 1. **Not fired.** That run's `windows-latest / default` lane failed **46** tests, and
>    *neither* named test is among them — not `cold_start_over_budget_returns_none_but_keeps_warming`,
>    not `background_command_with_quotes_captures_output`. The lane is red for an unrelated
>    reason (31 of the 46 are `librarian::tools::doctor::tests`, a scoping cluster), which is
>    precisely the discrimination this trigger exists to make: *a red Windows lane is not this
>    bug* unless it names one of these two tests.
> 2. **Not fired.** No new test asserts on wall-clock elapsed time.
> 3. **Not fired.** All seven `#[tokio::test(start_paused = true)]` sites in the crate
>    (`src/lsp/mod.rs`, `src/tools/progress.rs` ×2, `src/server.rs` ×3 plus the `last_activity`
>    field) use `tokio::time::Instant`; `grep std::time::Instant src/tools/progress.rs` returns
>    zero. The virtual-time trap is closed everywhere it could apply.
>
> This is the first re-check since the status was set, and it is the point of the status: a
> `zombie` is a claim that *nothing has been observed*, and that claim decays silently unless
> someone runs the trigger. Three months of green `windows-latest` cannot be the archive
> criterion while the lane is red for other reasons — re-check by trigger, not by lane colour.

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

**2. `background_command_with_quotes_captures_output`** — `src/tools/run_command/tests.rs`,
message `background command output not captured`.

**Corrected 2026-08-07, after reading the test.** This section first called it "a race between
the spawned background process writing its output and the assertion reading it" and prescribed
replacing a fixed wait with a bounded poll. The test **already polled** — 50 iterations ×
100 ms, a 5 s bound — so that fix would have been a no-op that closed the bug and left the flake
in place. Both claims came from the CI log line, not from the source.

The real defect was in the poll's shape: `if let Ok(v) = out { … }` **discarded the `Err` arm**,
so a command that never ran and a command still flushing both ended the loop with
`found == false` and the same contentless message. That is why the CI red carried no information.
The underlying MSVC intermittency remains unconfirmed — `py` demonstrably works on
`windows-latest` (the same job passed 3299 other tests and passed this one on an unchanged
re-run), so a genuine timing effect is still the leading explanation.

**The inconsistency that made this bite.** `.github/workflows/ci.yml`'s wine step `--skip`s
`background_command_with_quotes_captures_output` by name (`.github/workflows/ci.yml:117`), as one
of the 20 pre-existing wine failures catalogued in WIN-27. The `windows-latest` job does not skip
it. So the same test is exempted on one Windows job and load-bearing on the other.

**That split is justified, and now for a stated reason** — see *Evidence → The wine failure is a
missing interpreter*. It was recorded here as an unexplained inconsistency because nobody had
run the test under wine and read why it failed.

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

### The wine failure is a missing interpreter, not a race

Run locally under wine at `ea0340b0` with the improved diagnostic in place
(`scripts/build-windows.sh test --lib background_command_with_quotes`):

```
thread 'tools::run_command::tests::background_command_with_quotes_captures_output' panicked at
src/tools/run_command/tests.rs:3779:5:
background command output not captured within 15s (read errors: 0); last stdout: "Can't
recognize 'py -c \"print('bg-ok', 2+2)\"' as an internal or external command, or batch
script.\r\n"; last error: ""
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3319 filtered out; finished in 32.81s
```

Three readings, none of which the old contentless assertion could have supported:

1. **wine has no Python launcher.** The failure is environmental, so the wine `--skip` is correct
   and permanent — not a deferred investigation.
2. **The background plumbing works under wine.** The buffer captured cmd's own error text, which
   means spawn, capture, and the `type @bg_*` read path all function there. Only `py` is absent.
   This retires one member of WIN-27's twelve-test "root cause unknown" wine cluster: it was never
   a wine path-handling mystery.
3. **`read errors: 0`** — every poll iteration returned `Ok`, so the discarded `Err` arm was not
   itself the mechanism here; it was what made the *message* useless.

The reason for the skip is now recorded in `.github/workflows/ci.yml` next to the flag, including
an explicit instruction not to make the test self-skip on a missing `py`: a probe-gated early
return would pass vacuously on `windows-latest` if the probe ever misfired, silently disarming the
MSVC-CRT quote-mangling regression guard — the only place the test has value.

### Re-confirmed live 2026-09-02 — `zombie` → `open`, measured at job grain

The `zombie` status asked *"has this come back?"*. It has. Four consecutive `experiments` runs,
read from `gh run view <id> --json jobs` rather than from the run-level conclusion — a red run
says nothing about **which** job failed, and this file's whole subject is which:

| run | sha | win/no-features | win/default | win/local-embed | wine |
|---|---|---|---|---|---|
| 33570342471 | `a82026d7` | fail | fail | fail | fail |
| 33574961971 | `6d89a69b` | fail | fail | fail | fail |
| 33577436407 | `2d04c6ad` | fail | fail | fail | **success** |
| 33600281053 | `62d7fa4b` | success | success | success | success |

**Three of four runs, all three MSVC lanes together.** They fail and recover as a unit, which is
what a shared wall-clock assumption looks like and is consistent with this file's root cause.

**The wine lane decoupled on `2d04c6ad`** — passed while all three MSVC lanes failed. That is the
evidence that `05b157e0c38b765a` is a *separate* fault rather than the same one seen through a
different toolchain, and it is why these stay two bug files. Recorded here because the pair had no
measurement separating them before today; the split was assumed.

**The 2026-09-02 green run is not evidence of a fix, and the status change says so.** Nothing in
these lanes changed between `2d04c6ad` and `62d7fa4b` — same code, different dice. A passing flaky
test gives exactly the output a fixed one gives, which is why `open` is the honest status and why a
future reader meeting a green run should not read it as lapsed. **The predictive error is worth
keeping too:** on the strength of three consecutive failures this was forecast to fail again, and
all four lanes passed. A base rate is not a forecast.

**Not re-diagnosed — only re-observed.** No failure log from these four runs was read; the claim
here is *the lanes still fail intermittently*, not *they fail for the reason § Root cause states*.
Re-establishing the mechanism needs the actual assertion output from a failing run, and that is the
next step rather than something this pass did.
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

Item 1 **fixed and verified**; item 2 **mitigated**, root cause unconfirmed by design.

**1. The budget test — virtual clock.** `src/lsp/mod.rs`, `budget_tests`:
`#[tokio::test]` → `#[tokio::test(start_paused = true)]`, and `std::time::Instant` →
`tokio::time::Instant`. Every wait in the test is a tokio timer (`SlowStart`'s `delay`,
`client_within_budget`'s `timeout`, the warm-up `sleep`), so the whole schedule is now
deterministic. Auto-advance jumps to the next deadline — the 50 ms budget, not the 200 ms cold
start — so `None` is still returned for the right reason.

The second substitution matters as much as the first. Under `start_paused`, `std::time::Instant`
is **not** virtualised, so leaving it would have made the ceiling assertion trivially true (real
elapsed ≈ 0 ms) — non-flaky but vacuous. `tokio::time::Instant` **is** virtualised, so the 150 ms
ceiling became an exact statement about the schedule: it now fails if the call ever waits the
cold start out. Verified by mutation — tightening the ceiling to 40 ms fails with elapsed at the
50 ms budget, which proves both that the clock is virtual and that the assertion is live.

Wall-clock dependency is gone: the test reports `finished in 0.00s` and passed 25/25 consecutive
runs.

**2. The background-output test — make the failure name its own cause.** The prescribed "replace
the fixed wait with a bounded poll" was already implemented; see the correction in *Root cause*.
What landed instead:

- the poll's `Err` arm is no longer discarded — the last error, the last stdout, and an error
  count are kept and interpolated into the assertion message;
- the bound went from 50 × 100 ms (5 s) to 150 × 100 ms (15 s), proportionate to a suite that
  takes 138 s on that runner.

This does not claim to fix the MSVC intermittency — it makes the next occurrence diagnose itself.
Its first outing already paid for itself by explaining the wine failure (see Evidence).

**3. The skip-list split — resolved with evidence, unchanged in effect.** Keep
`background_command_with_quotes_captures_output` skipped on wine (no interpreter there) and
load-bearing on `windows-latest` (interpreter present, test normally green). The reasoning and an
explicit do-not-self-skip warning are recorded in `.github/workflows/ci.yml` beside the flag, so
the split reads as a decision rather than an oversight.

SHAs: to be recorded on commit; `experiments`-side until cherry-picked.
## Tests added

No new test cases — both changes harden existing tests, which is the point.

- `cold_start_over_budget_returns_none_but_keeps_warming` (`src/lsp/mod.rs`, `budget_tests`) —
  same three assertions, now on a virtual clock. Strictly **stronger** than before: the elapsed
  ceiling went from a 100 ms jitter tolerance to an exact schedule assertion, confirmed by the
  40 ms mutation. Determinism confirmed at 25/25 runs, each `0.00s`.
- `background_command_with_quotes_captures_output` (`src/tools/run_command/tests.rs`) — same
  success condition; the failure path now reports last stdout, last error, and an error count.
  Verified by running it under wine, where it failed and named its own cause on the first try.
  Type-checks for `x86_64-pc-windows-gnu` (`scripts/build-windows.sh check --lib --tests`, 0
  errors).

A regression test for the MSVC intermittency is intentionally absent: the symptom has never been
reproduced, on CI or locally, and a test for an unreproduced timing effect would only re-encode
the guess. The diagnostic is the substitute — it converts the next occurrence into evidence.
## Workarounds

`gh run rerun <run-id> --failed` re-runs only the failed jobs and preserves the passing ones,
which is what produced 15/15 here. Cheap, but it means the gate is not trustworthy on first
read — a red run has to be triaged by job `steps` count and test name before it means anything.

## Resume

Status stays `open` deliberately, matching the precedent set by
`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`: the harm is
neutralised and the next occurrence is self-diagnosing, but the MSVC symptom was never reproduced,
so nothing here has actually been proven fixed.

**Next action is to wait, not to investigate.** On the next `windows-latest` red for
`background_command_with_quotes_captures_output`, read the assertion message and route on it:

- `last stdout` empty and `read errors: 0` → the background process produced nothing in 15 s.
  Look at spawn/flush in the `run_in_background` path, not at the test.
- `last stdout` holds a partial line → a genuine flush race; the poll should key on process exit
  rather than on buffer content.
- `last stdout` holds a cmd error naming `py` → the runner image lost the Python launcher; the
  test needs a different interpreter, not a longer poll.
- `read errors` > 0 → the `type @bg_*` read path itself is failing; `last error` names how.

Archive when either that message identifies a fix that lands with a regression test, or three
months of green `windows-latest` runs justify closing it as `zombie` with the re-open trigger
above. Item 1 needs nothing further.
## References

- `src/lsp/mod.rs` — `budget_tests`, `SlowStart`
- `src/tools/run_command/tests.rs` — `background_command_with_quotes_captures_output`
- `.github/workflows/ci.yml` — the wine job's `--skip` list
- `docs/trackers/windows-platform-support.md` — WIN-27 baseline, and WIN-30 for this entry
- `docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md` — the 20-failure inventory
