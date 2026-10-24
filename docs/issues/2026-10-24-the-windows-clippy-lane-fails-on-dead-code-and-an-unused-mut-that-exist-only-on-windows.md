---
status: open
opened: 2026-10-24
closed:
severity: low
owner: marius
related: [docs/issues/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md]
tags:
- cluster/unclassified
kind: bug
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

Not applied. For each item either gate the definition with `#[cfg(unix)]` or add the Windows reader. For the `mut`, gate the binding on `cfg(unix)` or use `#[cfg_attr(not(unix), allow(unused_mut))]`.

## Tests added

N/A: filed, not fixed.

## Workarounds

None known. The errors do not affect Unix builds.

## Resume

Read each item's use sites (`references`), decide gate-versus-reader per item, and check with `scripts/build-windows.sh clippy --all-targets -- -D warnings` together with the sibling bug's fix.

## References

- `docs/issues/2026-10-23-the-windows-lib-test-build-fails-because-a-mux-test-reads-constants-that-exist-only-on-unix.md`
- `src/lsp/client.rs:38`, `src/lsp/manager.rs:109`, `src/memory/write_transaction.rs:161`
