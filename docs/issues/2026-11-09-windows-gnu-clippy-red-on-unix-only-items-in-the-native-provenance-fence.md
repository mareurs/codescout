---
id: '17fa244491a90b49'
kind: bug
status: open
title: 'BUG: the windows-gnu clippy lane is red at HEAD on unix-only items in the native provenance fence'
owners:
- marius
tags:
- cluster/unclassified
closed: null
opened: 2026-11-09
owner: marius
related:
- docs/issues/archive/2026-10-24-the-windows-clippy-lane-fails-on-dead-code-and-an-unused-mut-that-exist-only-on-windows.md
- docs/issues/archive/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md
severity: medium
unverified: CI's windows-gnu clippy step was not read live. Red-at-HEAD is established only by running the CI command locally on a clean extract of HEAD (a37c45d0f). Root cause is verified for one item chain (LOCK_NAME -> path), not for all 19. Nothing was run on Windows.
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

## Fix

Not fixed. Two options, for whoever owns the fence work:

1. Gate the unix-only helpers, fields and imports with `#[cfg(unix)]` at their definitions.
2. `#[cfg_attr(not(unix), allow(dead_code))]`, as `40f33bad6` did for the 2026-10-24 instance. That fix's own caveat applies: allow attributes hide real dead code in the files they cover.

Not attempted here: the files are a recent feature of another work stream and outside the change that found this.

## Tests added

N/A — not fixed. The regression check is the lane itself: `scripts/build-windows.sh clippy --all-targets -- -D warnings` must exit 0.

## Workarounds

None for the lane. `scripts/build-windows.sh check` passes.

## Resume

1. Choose option 1 or 2 above and apply it.
2. Run `scripts/build-windows.sh clippy --all-targets -- -D warnings` under `scripts/with-slot.sh`; expect exit 0.
3. Read CI's windows-gnu clippy step on the current head, to confirm it is red there too (not checked, see `unverified`).

## References

- Commit `9cab1095e` — the feature that added the three modules.
- `docs/issues/archive/2026-10-24-the-windows-clippy-lane-fails-on-dead-code-and-an-unused-mut-that-exist-only-on-windows.md` — the sibling, fixed in `40f33bad6`.
- `docs/issues/archive/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md` — the other sibling.
- `.github/workflows/ci.yml`, step "Cross-clippy (default features)", which runs `scripts/build-windows.sh clippy --all-targets -- -D warnings`.
