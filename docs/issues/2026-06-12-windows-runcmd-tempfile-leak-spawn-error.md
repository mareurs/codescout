---
status: fixed
opened: 2026-06-12
closed: 2026-06-12
severity: low
owner: marius
related: []
tags: [windows, run_command, process-spawn, tempfile]
kind: bug
---

# BUG: Windows run_command foreground arm leaks temp files on the spawn-error path

## Summary
On Windows, `run_command_inner`'s foreground branch captures child stdout/stderr to
two `tempfile`s persisted via `.keep()`. The `TmpfileGuard`s that delete them were
constructed *inside* the result future, so if `cmd.spawn()?` returned `Err`, the
future was never polled and both `.keep()`d temp files leaked into `%TEMP%`. Low
severity (error path only, zero-byte files), but unbounded over repeated spawn failures.

## Symptom (Effect)
Orphaned `codescout-cmd-out-*` / `codescout-cmd-err-*` zero-byte files accumulate in
`%TEMP%` after any foreground `run_command` whose child process fails to spawn on
Windows. No error is surfaced to the caller — the leak is silent.

## Reproduction
1. Windows host, `vdi-windows` at or before `5c9ba0dd`.
2. Invoke a foreground `run_command` whose shell spawn fails (e.g. an exhausted handle
   table, or inject a spawn error before `cmd.spawn()`).
3. Observe two new zero-byte `codescout-cmd-{out,err}-*` files in `%TEMP%` with no
   corresponding cleanup.

Found by code inspection during the 2026-06-12 Linux-side review, not a field report.

## Environment
Windows only — the `#[cfg(windows)]` foreground arm of `run_command_inner`. Not
reachable on Unix (the Unix arm uses piped stdio, no temp files).

## Root cause
`src/tools/run_command/inner.rs`, Windows `#[cfg(windows)]` foreground block: the temp
files are created and persisted with `out_tmp.keep()` / `err_tmp.keep()` *before* the
`let mut child = cmd.spawn()?;` line, but the `TmpfileGuard` RAII deleters were bound
*inside* the returned `Box::pin(async move { … })`. The guards therefore only come into
existence when the future is first polled. On the `spawn()?` early return, the future is
never constructed/polled, so the kept files have no deleter — they leak.

## Evidence
Pre-fix structure (abbreviated):
```rust
let (out_file, out_path) = out_tmp.keep()?;
let (err_file, err_path) = err_tmp.keep()?;
let mut cmd = crate::platform::shell_command_configured(&effective_command);
cmd.current_dir(&work_dir).stdout(Stdio::from(out_file))/* … */;
let mut child = cmd.spawn()?;     // early Err here orphans out_path/err_path
let fut = Box::pin(async move {
    let _out_guard = TmpfileGuard(out_path.to_string_lossy().into_owned()); // only here
    let _err_guard = TmpfileGuard(err_path.to_string_lossy().into_owned());
    /* … */
});
```

## Hypotheses tried
N/A — root cause clear from inspection; no dead-end branches walked.

## Fix
Create the two `TmpfileGuard`s immediately after `.keep()` (before `spawn()`), then
`move` them into the future. An early `?` from `spawn()` now drops the guards at function
scope and deletes the files; on success they move into the future and still drop on normal
completion and on future-drop (timeout / cancellation).

Implemented in `src/tools/run_command/inner.rs` (Windows foreground arm of
`run_command_inner`). Commit: `vdi-windows 9cba50cb` (branch-side; update to the
master-side SHA at graduation per CLAUDE.md § "After cherry-pick").

## Tests added
None. The leak is on the Windows spawn-*failure* path, which is not deterministically
reproducible in the suite (requires inducing a spawn error), and the whole block is
`cfg(windows)` so it is excluded from the Linux gate. Verified by reasoning (RAII guard
moved before the fallible `?`); compiler-gated on the VDI build. A future Windows-only
test could inject a spawn failure and assert `%TEMP%` has no residual `codescout-cmd-*`
files.

## Workarounds
Periodically clear `codescout-cmd-out-*` / `codescout-cmd-err-*` from `%TEMP%`. Only
accumulates on spawn failures, which are rare.

## Resume
N/A — fixed in `vdi-windows 9cba50cb`. If graduating: confirm `git branch --contains`
shows the master-side SHA, update the Fix section to cite it, then move this file to
`docs/issues/archive/`.

## References
- `src/tools/run_command/inner.rs` — Windows foreground arm of `run_command_inner`.
- `docs/trackers/windows-platform-support.md` — WIN-20.
- 2026-06-12 Linux review session.
