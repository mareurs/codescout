---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-24
owner: marius
related:
- docs/issues/archive/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md
severity: low
unverified: 'STANDING no Windows runner here: the lib and its tests compile and lint for x86_64-pc-windows-gnu (verified 2026-10-24) but were not run on Windows. The allow(dead_code) attributes on non-Unix builds hide real dead code in the files they cover.'
---

# BUG: the Windows clippy lane fails on dead code and an unused `mut` that exist only on Windows

## Summary

`scripts/build-windows.sh clippy --all-targets -- -D warnings` fails for the Windows target even before the lib-test errors in the sibling bug. Three lint errors come from code that is compiled on Windows but only used on Unix: `MUX_INIT_READ_TIMEOUT` and `MuxInitTimeout` (`src/lsp/client.rs:38`, `:43`), the field `LspManager::mux_wedged_at` (`src/lsp/manager.rs:109`), and an unused `mut` on a `DirBuilder` (`src/memory/write_transaction.rs:161`).

## Class fit

Same family as the sibling bug: code gated on `cfg(unix)` at its use sites but not at its definition, so the Windows build sees a definition with no reader.

## Symptom (Effect)

The Windows clippy lane cannot go green with `-D warnings`. Because the lane compiles the lib first, these errors also hide anything else the lane would find.

## Reproduction

Run 2026-10-24 on branch `experiments` with uncommitted changes from other work in the tree:

```
scripts/build-windows.sh clippy --all-targets -- -D warnings
```

(expands to `cargo clippy --target x86_64-pc-windows-gnu --no-default-features --features remote-embed,http,librarian,local-embed-dynamic --all-targets -- -D warnings`). Lib errors, besides the sibling's four `E0425`:

```
error: variable does not need to be mutable   --> src/memory/write_transaction.rs:161:17
error: constant `MUX_INIT_READ_TIMEOUT` is never used   --> src/lsp/client.rs:38:18
error: struct `MuxInitTimeout` is never constructed   --> src/lsp/client.rs:43:19
error: field `mux_wedged_at` is never read   --> src/lsp/manager.rs:109:5
```

## Environment

Linux host cross-compiling to `x86_64-pc-windows-gnu`. Not run on a Windows machine.

## Root cause

Not read to the bottom. `git status` shows `src/lsp/client.rs` and `src/memory/write_transaction.rs` unmodified, so the errors are not from the working-tree edits of this session. I did not run a baseline on a clean checkout, so "present before this session" is an inference. The likely cause, not verified: the readers of the three mux items are under `cfg(unix)` while the items are not, and the `mut` is needed only on a Unix branch (`DirBuilderExt::mode`).

## Evidence

See the Reproduction output.

## Hypotheses tried

None.

## Fix

Fixed 2026-10-24 in `40f33bad6` (experiments).

- `src/lsp/client.rs`: `#[cfg(unix)]` on `MUX_INIT_READ_TIMEOUT`, `MuxInitTimeout` and its `Display` and `Error` impls. Their only reader, `LspClient::connect`, is `#[cfg(unix)]`.
- `src/lsp/manager.rs`: `#[cfg(unix)]` on the field `mux_wedged_at` and its initializer. Its readers are in the unix-only `get_or_start_via_mux_waiting`.
- `src/memory/write_transaction.rs`: `#[cfg_attr(not(unix), allow(unused_mut))]` on the `DirBuilder`, which only the Unix branch mutates.

The cause named in Root cause held for all four lib errors: a definition compiled on Windows whose only readers are Unix-only. The root-cause note that this was not checked against a baseline stays true; the fix does not depend on it.

Beyond the four lib errors, 52 dead-code errors appeared once the lib tests compiled (see the sibling bug about the `E0425` lib-test build). They were fixed in the same commit with `cfg_attr(not(unix), allow(dead_code))`: a module-level attribute in `src/tools/run_command/tests.rs` for 49 helpers, and one each on two test-seam fields in `src/lsp/manager.rs`, on `make_debug_server` and on `SECRET`. This hides real dead code on Windows in those files.

## Tests added

No new test: the failure was a lint error, and the regression check is the lane itself. `scripts/build-windows.sh clippy --all-targets -- -D warnings` failed with 4 lib errors before `40f33bad6` and finishes with exit 0 after it. `./scripts/gate.sh` on a clean detached worktree of `40f33bad6` exited 0 (lean 5253 passed, default 7895 passed, Python 1274 passed). No test was run on Windows.

## Workarounds

None known. The errors do not affect Unix builds.

## Resume

Closed. Nothing to resume.


## Fix provenance

- **SHA:** `40f33bad6b52764f7dd1a70d9bfb48517c33f28b` (`experiments`)
- **patch-id:** `442c8136824ec7eb4fb27b69bb544aa9ef52765b`

## References

- `docs/issues/archive/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md`
- `src/lsp/client.rs:38`, `src/lsp/manager.rs:109`, `src/memory/write_transaction.rs:161`
