---
id: fdf239b77f544684
kind: bug
status: open
title: 'BUG: the lsp fanout tests call std::os::unix::fs::symlink without cfg(unix), so the Windows lib tests do not compile'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- tests
- ci
- windows
- lsp
opened: 2026-11-01
severity: high
unverified: The compile fix is verified only for the windows-gnu target on Linux, without default features; the windows-msvc CI jobs are not yet observed, and the 297/40/40 older Windows test failures it uncovers have no bug file.
---

## Summary

The tests in `src/lsp/watch_fanout.rs` create symlinks with `std::os::unix::fs::symlink` in the helper `links_in` and directly in 2 tests (11 more tests go through the helper), with no `#[cfg(unix)]`. `std::os::unix` does not exist on Windows, so the lib test target does not compile there: all three Windows CI test jobs went from "tests ran" (Oct 30) to "build failed" (Nov 1). The code was added by commit `eee623216` (2026-10-31, session `1a35461f`).

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the local gate runs on Linux only, so a Windows-only compile error cannot show up in it, and CI is the only environment that gates the Windows cfg. It differs from the class's other members in that the divergence is the target platform, not the machine's files or variables, and that it hides a worse state: on Oct 30 the Windows jobs compiled and still had 297, 40 and 40 failing tests, which the compile error now masks.

## Symptom (Effect)

CI run 41322768679, jobs `Test (windows-latest / default | local-embed | no-features)`: `error[E0433]: cannot find 'unix' in 'os'` at `src/lsp/watch_fanout.rs:164, 194, 195, 209`, then `could not compile 'codescout' (lib test) due to 4 previous errors`. No Windows test runs at all.

## Reproduction

On Linux, with the `x86_64-pc-windows-gnu` target and MinGW installed:

`cargo check -p codescout --lib --tests --no-default-features --target x86_64-pc-windows-gnu`

Before the fix: the same 4 errors at the same 4 lines. After: `Finished`. `--no-default-features` is needed only locally, because `ort-sys` ships no prebuilt library for windows-gnu. `watch_fanout` is not feature-gated, so the lean build compiles the same code.

## Environment

codescout `experiments` at `60dc3569e`; rustc 1.97.1; target `x86_64-pc-windows-gnu`.

## Root cause

`eee623216` added the `links_in` helper and 13 tests that create symlinks (11 through the helper, 2 directly: lines 164, 194, 195, 209 call `std::os::unix::fs::symlink`) and put them in an ungated `mod tests`. The other 3 tests in the module (the hint wording) are portable. The repo's convention for symlink tests is `#[cfg(unix)]` (672 uses in 71 files).

## Evidence

CI run 41322768679 log, Windows default job, lines quoted above. The local `cargo check` reproduces the 4 errors exactly (same file, lines and message).

## Hypotheses tried

None needed: the compiler names the lines.

## Fix

`#[cfg(unix)]` on `links_in` and on the 13 tests that use it. No production code changed. Fix SHA is added when the fix is on `experiments`.

## Tests added

None. The regression test is the compile check above (RED with 4 errors, GREEN with `Finished`) and the Windows CI lane. The local gate has no Windows compile check, so a repeat would be caught only by CI.

## Workarounds

None.

## Resume

1. Confirm the three Windows jobs compile on the first CI run containing the fix.
2. Expect them to RED again at the test level: on Oct 30 they had 297, 40 and 40 failing tests (a separate, older state that the compile error hid). That is not part of this bug and has no file yet.
3. Consider adding `cargo check --tests --no-default-features --target x86_64-pc-windows-gnu` to `scripts/gate.sh`: it takes about 25 s with a warm cache and would have caught this before the push.

## References

- Commit `eee623216` (introduced), session `1a35461f`.
- CI runs 41322768679 (compile error) and 41026635953 (Oct 30: compiled, 297/40/40 failing tests).
