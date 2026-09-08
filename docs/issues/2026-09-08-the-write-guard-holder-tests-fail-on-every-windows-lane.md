---
status: open
opened: 2026-09-08
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md
  - docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md
kind: bug
tags:
  - cluster/repro-env-diverges-from-gate-env
---

# BUG: the three write-guard holder tests fail on every Windows lane, and they are now the only thing keeping `experiments` red

## Summary

`agent::write_guard::tests` fails three tests on `windows-latest`, in all three Windows
configurations (`default`, `local-embed`, `no-features`):

```
a_contended_acquire_names_the_holder_not_merely_that_someone_holds_it
releasing_the_lock_clears_the_holder_record
the_refusal_reports_elapsed_hold_and_promises_no_deadline
```

They pass on `ubuntu-latest` and `macos-latest`, and in both local gate lanes.

**These three are now the entire red.** At `2ad4490c` the run is **14 jobs green, 4 red**, and
every red is Windows-family: the three lanes above plus `Windows-gnu cross (MinGW + wine)`.
Every Linux and macOS lane — including `server-stack`, the build `cargo rb` ships — is green for
the first time since 09-06T17:35.

## Symptom (Effect)

`test result: FAILED. 5139 passed; 3 failed` on `windows-latest / default`.

**A second-order effect worth naming:** `cargo test` stops after a failing test binary, so the
lib failure means the Windows lanes **never reach the integration tests at all**. Verified at
`2ad4490c` — `tests/embedder_env_isolation.rs` appears in the ubuntu job log
(`Running tests/embedder_env_isolation.rs`) and **zero** times in the Windows one. So every
`tests/*.rs` guard in this repo is currently unexercised on Windows, and nothing says so.

## Environment

`windows-latest`. Not reproducible on this machine (Linux) — see *cluster*.

## Root cause

**Unknown, and not investigated here.** What is established:

The three tests were introduced by
`docs/issues/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md`
(`7f023c2ec0ae7856`, status `fixed`), whose *Tests added* section names all three and asserts
they are *"all reached in **both** gate lanes (the file is under `src/agent/`, so the lean lane
is not vacuous for it)"*. That claim is true and insufficient in the way this repo has already
documented: **both gate lanes are Linux.** Lane coverage was reasoned about on the feature axis
and not on the platform axis, so a record marked `fixed` shipped three tests that have never
passed on a third of the matrix.

The likely families, none confirmed: holder identity via pid/process metadata, filesystem lock
semantics, or timing in the elapsed-hold assertion. The messages assert on *text* built from
runtime state, so a platform difference in any one input reds all three at once — which is
consistent with the three failing together and nothing else in the module failing.

## Evidence

- `2ad4490c` job matrix: 14 success, 4 failure, all Windows-family.
- Identical three names at `999553cf` (before the embedder fix), where they sat alongside the
  embedder failures. **Pre-existing, not introduced by `ab33f4ff`** — that commit's diff touches
  nothing under `src/agent/`.
- `999553cf` windows/default failed 8+ tests; `2ad4490c` fails 3. The embedder fix cleared the
  Windows embedder failures too.

## Hypotheses tried

- *"The new `tests/embedder_env_isolation.rs` guard broke Windows."* **Refuted** — it does not
  appear in the Windows failure list, and per *Symptom* it never runs there at all.

## Fix

None attempted. Two things a fixer should know before starting:

1. **The lean/default gate cannot see this**, so a Linux-only reproduction attempt will report
   green and prove nothing. The lane to read is CI's `Test (windows-latest / *)`.
2. **Do not delete or `#[cfg]`-skip the three tests to green the branch.** They guard the
   holder-naming remedy their own bug file was opened for, and skipping them on Windows converts
   a visible red into exactly the silent gap that record exists to close. If they must be
   gated, the gate needs its own note saying what is then unguarded on Windows.

## Tests added

None. What is owed alongside a fix is a signal for the second-order effect: nothing currently
reports that the Windows lanes stop before the integration tests, so a `tests/*.rs` guard can be
green in every readable lane and unexercised on Windows indefinitely.

## References

- `src/agent/write_guard.rs` — the three tests
- `docs/issues/2026-09-03-a-held-write-lock-names-no-owner-progress-or-duration.md` — added them
- `docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md` — the prior
  instance of this shape, nine tests, archived `fixed`
- `.github/workflows/ci.yml:218` — `cargo test --workspace ${{ matrix.config.flags }}`
