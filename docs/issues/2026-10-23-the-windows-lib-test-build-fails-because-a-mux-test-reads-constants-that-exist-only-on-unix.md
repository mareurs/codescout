---
id: '99b56275f0439c09'
kind: bug
status: open
title: the Windows lib-test build fails because a mux test reads constants that exist only on Unix
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
closed: null
opened: 2026-10-23
severity: medium
unverified: The cfg of three of the four constants was not read, and no Windows build was run here.
---

# BUG: the Windows lib-test build fails because a mux test reads constants that exist only on Unix

## Summary
`cargo test` on the Windows runners stops at compile time with four `E0425` errors in `src/lsp/manager.rs`. No lib test runs on Windows, so the tests added with the IL-3 capture have never run there. The failure is already in the baseline CI run for `b1fde55c0` and is unchanged on `5018dd327` and `2acc553ad`.

## Class fit
The test builds on the developer machine and on Linux and macOS, where the constants it names exist. It fails to build on the Windows gating runners, where they are compiled out. The reproduction environment diverged from the gate environment.

## Symptom (Effect)
From `gh run view 39915517190 --job 120288473909 --log`, job `Test (windows-latest / no-features)`:

```
error[E0425]: cannot find value `MUX_INIT_READ_MIN` in this scope
error[E0425]: cannot find value `MUX_SPAWNED_CONNECT_WINDOW` in this scope
error[E0425]: cannot find value `MUX_CONNECT_RETRY_INTERVAL` in this scope
error[E0425]: cannot find value `MUX_CONNECT_ROUNDS` in this scope
error: could not compile `codescout` (lib test) due to 4 previous errors
```

The three Windows test jobs (`default`, `no-features`, `local-embed`) fail the same way. Baseline run 39574468685 shows 4 `E0425` errors in the same job; I counted them and did not read their names there.

## Reproduction
Build the lib tests for a Windows target, for example `cargo test --lib --no-run --target x86_64-pc-windows-gnu`. Not run locally this session: CI logs only.

## Environment
GitHub runners `windows-latest`. Branch `experiments`.

## Root cause
Read 2026-10-23: `src/lsp/manager.rs:603` declares `MUX_INIT_READ_MIN` under `#[cfg(unix)]`, and the four names in the errors are the four constants that `the_mux_connect_constants_have_the_values_the_docs_quote` (about `src/lsp/manager.rs:3168`) compares by literal at lines 3174 to 3177. That test carries a plain `#[test]` with no `#[cfg(unix)]`. I read the `cfg` only at line 603; the other three constants' attributes were not read. Not built for Windows here.

## Evidence
`gh run view 39915517190 --log` for job 120288473909, quoted above.

## Hypotheses tried
1. **Hypothesis:** the capture work caused it. **Test:** the baseline run for `b1fde55c0` predates the capture and shows 4 `E0425` errors in the same job. **Verdict:** rejected.

## Fix
Not started. One candidate: mark the test `#[cfg(unix)]`, like the constants. The owner of the mux code decides.

## Tests added
N/A until the fix exists.

## Workarounds
None. Windows CI cannot gate lib tests until it is fixed.

## Resume
Open `src/lsp/manager.rs` around lines 584-603 and 3168-3178. Check the `cfg` on all four constants, then gate the test the same way. Verify with a Windows lib-test build or the CI job `Test (windows-latest / no-features)`.

## References
CI runs 39915517190 and 39574468685. Related capture-test bug: `docs/issues/archive/2026-10-23-capture-feature-tests-assume-a-bash-sh-and-gnu-seq-and-fail-on-ubuntu-and-macos-ci.md`. Mux issues that quote these constants: `docs/issues/2026-10-17-connect-to-a-mux-that-never-sends-its-init-message-hangs-forever.md`.
