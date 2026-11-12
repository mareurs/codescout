---
id: '17fa244491a90b49'
kind: bug
status: mitigated
title: 'BUG: the windows-gnu clippy lane is red at HEAD on unix-only items in the native provenance fence'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-12
opened: 2026-11-09
owner: marius
related:
- docs/issues/archive/2026-10-24-the-windows-clippy-lane-fails-on-dead-code-and-an-unused-mut-that-exist-only-on-windows.md
- docs/issues/archive/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md
severity: medium
unverified: 'CI''s windows-gnu clippy step was not read live on a head carrying the fix. The exit 0 is a local cross-compile from Linux; nothing ran on Windows. Which items are dead because a unix gate is missing, as opposed to code that should be reachable on Windows, was not judged item by item: the allow covers all of them.'
---

## Summary

At HEAD `a37c45d0f`, `scripts/build-windows.sh clippy --all-targets -- -D warnings` exits 101: 19 errors in the lib and 17 in the lib tests. All of them are in code that commit `9cab1095e` (2026-11-07, "feat: validate native provenance with coordinated execution fences") added: `src/tools/native_provenance_fence.rs`, `src/tools/provenance_fence_context.rs`, `src/tools/trusted_formatter.rs`, plus two test imports. CI runs this exact command in its windows-gnu clippy step.

Found while verifying an unrelated change (`local-embed` leaving `default`). The same command on a clean extract of HEAD, with none of that change, gives the same errors, so that change neither causes nor hides them.

## Class fit

Same family as `2026-10-24-the-windows-clippy-lane-fails-on-dead-code-and-an-unused-mut-that-exist-only-on-windows` and `2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix`: code that only the `cfg(unix)` arms use, declared without a `cfg`, so the Windows build sees definitions that nothing reads. It differs in shape: here the unix-only arms sit inside functions that exist on both targets, with a stub under `cfg(not(unix))`, and the count is larger (19 items across three modules).

## Symptom (Effect)

`-D warnings` turns these into errors on the windows-gnu lib build:

- `src/tools/native_provenance_fence.rs`: constant `LOCK_NAME`; fields `file`, `root`, `counter_path` and `generation` of `Fence`; functions `path`, `open`, `counter`, `coordination_root`, `has_unverified_shell`, `mark_unverified_shell`, `canonical_checkout`; associated items `try_acquire`, `try_participate`, `try_mode`, `generation`, `root`, `bump`.
- `src/tools/provenance_fence_context.rs`: unused import `canonical_checkout` (line 3); unused variables `root`, `input`, `meta`, `client` (lines 122 to 127).
- `src/tools/trusted_formatter.rs`: functions `is_foreground_formatter`, `certificate`, `verify`; constant `SCRIPTS`.
- Lib tests: unused imports `make_server_in_root` (`src/server.rs:3671`) and `BufRead`, `BufReader` (`src/tools/native_provenance_fence/tests.rs:2`), and never-read fields.

`scripts/build-windows.sh check` passes with the same warnings (exit 0). Only the `-D warnings` lane fails.

## Reproduction

```bash
scripts/build-windows.sh clippy --all-targets -- -D warnings   # exit 101
```

Control, run 2026-11-09: `git archive HEAD` extracted to an empty directory, then the same command there under `scripts/with-slot.sh`. Result: the same 19 lib errors, the same 17 lib-test errors, exit 101.

## Environment

Linux host, `x86_64-pc-windows-gnu` via MinGW, rustc 1.99.1, HEAD `a37c45d0f34ed061b5b02ce40791d40c585811f1`. Feature set: `remote-embed,http,librarian,local-embed-dynamic` (the script's default).

## Root cause

Partly established. `src/tools/mod.rs` declares `native_provenance_fence` (line 22), `provenance_fence_context` (line 23) and `trusted_formatter` (line 64) with no `cfg`. Their functions branch on `#[cfg(unix)]` and `#[cfg(not(unix))]` (`native_provenance_fence.rs` lines 31, 65, 72, 159, 210, 218, 283, 291; `provenance_fence_context.rs` lines 136, 138). On Windows the `not(unix)` arms are stubs, so the helpers that only the unix arms call have no reader.

Verified for one chain: `LOCK_NAME` is used only by `path()`, and `path()` is itself reported unused. Not verified item by item for the other 18.

## Evidence

- With a local-embed change applied: exit 101, 19 lib errors and 17 lib-test errors.
- HEAD control from a clean extract: exit 101, the same counts and the same items.
- `git status` is clean for the three modules; `git log` shows `9cab1095e` as the only commit touching them.

## Hypotheses tried

- Caused by the `local-embed` change: refuted by the HEAD control, which has none of it.
- Caused by the feature set the script passes: not supported. The errors are about `cfg(unix)`, not about any Cargo feature.

## Re-measured at HEAD aec654c7a, 2026-11-11

Clean `git archive HEAD` of `aec654c7a0fd1784bb1d0a4ed32a10a5ccb24d59` (207 commits after `a37c45d0f`) extracted to an empty directory, then `scripts/with-slot.sh ./scripts/build-windows.sh clippy --all-targets -- -D warnings` there (`x86_64-pc-windows-gnu`, mingw present, same feature set as the script's default).

- Result: exit 101. `could not compile codescout (lib) due to 20 previous errors` and `(lib test) due to 18 previous errors`. The bug measured 19 and 17 at `a37c45d0f`, so the lane is still red and the count grew by one in each target.
- Only three lint kinds appear in the output: `unused-imports`, `unused-variables` and `dead-code`, each implied by `-D warnings`. No `clippy::` lint is among them.
- The Symptom list has drifted. Four commits touched these files since `a37c45d0f` (`4eceee5e7`, `0d47f5898`, `1ccf33e7b`, `193ec23d0`), and the surface is no longer only the three modules:
  - `is_foreground_formatter` and `certificate` are now reported in `src/tools/provenance_fence_context.rs` (lines 20 and 33), not in `trusted_formatter.rs`. `verify` (line 95) and `SCRIPTS` (line 7) stay in `trusted_formatter.rs`.
  - New: fields `call_id` and `turn_id` never read, `src/tools/invocation_context.rs:89`. `git log -S"call_id"` on that file names `c7b38be29` (feat(observations): capture stock Codex operations), so `9cab1095e` is no longer the only commit behind the red lane.
  - Moved: unused import `make_server_in_root` is now at `src/server.rs:3800` (was `:3671`); `provenance_fence_context.rs` unused variables are still at lines 122 to 127.
  - Lib test target: `fields file and root never read` and `method root is never used` (`native_provenance_fence.rs:12` and `:279`), plus the unused imports.

Probe of completeness, in the extracted copy only (nothing in this checkout was edited): adding `#![cfg_attr(not(unix), allow(dead_code, unused_imports, unused_variables))]` at the top of `src/lib.rs` and re-running the same command gives exit 0 and zero errors (`Finished dev profile in 55.02s`). So the 38 errors are exactly these three lint kinds under `not(unix)`, and no further lint is hidden behind them. This is a probe of the error set, not the recommended fix: a crate-level allow hides real dead code everywhere, which is the caveat the Fix section already names for option 2.

This also upgrades the Root cause: it was verified for one item chain; now the whole set is shown to disappear when exactly dead-on-`not(unix)` code is allowed. Which items are dead because of a missing `cfg(unix)` gate, as opposed to code that should be reachable on Windows, was not judged item by item.

## Fix

Option 2 for the three fence modules, option 1 where a cfg is cheap. `0d98dcef2`:

- `native_provenance_fence.rs`, `provenance_fence_context.rs`, `trusted_formatter.rs` carry `#![cfg_attr(not(unix), allow(dead_code))]`, scoped to the module and to `not(unix)`. Option 1 on each definition was not chosen: the tests read `SCRIPTS` and `Fence::root`, so a cfg on the definition would cascade into them.
- `provenance_fence_context.rs`: `canonical_checkout` is imported on Unix only; `scope_root` consumes `root`, `input`, `meta` and `client` on its `not(unix)` stub arm.
- `invocation_context.rs`: `call_id` and `turn_id` of `NativeInvocation::Codex` carry `#[cfg_attr(not(target_os = "linux"), allow(dead_code))]`. `observations::server` reads them and builds on Linux only (`src/observations/mod.rs:2`). This is the item the 2026-11-11 re-measure added; `c7b38be29` is its commit.
- `server.rs` and `native_provenance_fence/tests.rs`: the imports that only a `cfg(unix)` test reads are `cfg(unix)`.

The cost of option 2 is the one the entry named: inside those three modules, dead code on Windows is no longer an error. Dead code on Unix still is.

Measured 2026-11-12 on a tree carrying the change: `scripts/with-slot.sh ./scripts/build-windows.sh clippy --all-targets -- -D warnings` exits 0 (before: exit 101, 20 lib and 18 lib-test errors). `./scripts/gate.sh`: clippy lean and default exit 0; Python 1836 passed; DEFAULT 9756 of 9757 passed.

## Fix provenance

- **SHA:** `0d98dcef2a0815f95336a0cf5cda2ebedf8ed2ce` (`experiments`)
- **patch-id:** `3fe7a8df1b65868cb5bded6ac50c98f5594933a6`

## Tests added

No new test. The regression check is the lane itself: `scripts/build-windows.sh clippy --all-targets -- -D warnings` must exit 0.

## Workarounds

None for the lane. `scripts/build-windows.sh check` passes.

## Resume

Mitigated by `0d98dcef2`. Left to do: read CI's windows-gnu clippy step on a head that carries it, which was not checked. The one test failure in the gate lanes (`tools::onboarding::tests::the_block_names_the_scope_it_scanned_even_when_nothing_was_found`) is in `src/tools/onboarding.rs`, which another session has uncommitted; it is not part of this bug.

## References

- Commit `9cab1095e` — the feature that added the three modules.
- `docs/issues/archive/2026-10-24-the-windows-clippy-lane-fails-on-dead-code-and-an-unused-mut-that-exist-only-on-windows.md` — the sibling, fixed in `40f33bad6`.
- `docs/issues/archive/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md` — the other sibling.
- `.github/workflows/ci.yml`, step "Cross-clippy (default features)", which runs `scripts/build-windows.sh clippy --all-targets -- -D warnings`.
