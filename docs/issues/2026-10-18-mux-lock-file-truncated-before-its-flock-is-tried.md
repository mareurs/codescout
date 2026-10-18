---
kind: bug
status: open
title: 'BUG: a second mux start truncates the lock file before its flock is tried, erasing the PID line of the live mux that holds it'
tags:
- cluster/unclassified
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
severity: low
unverified: 'CLEARED 2026-10-18. Was: the OS behaviour was measured with a python script, not through the Rust code. Now measured through acquire_mux_lock (review F7), see Reproduction.'
---

# BUG: a second mux start truncates the lock file before its flock is tried, erasing the PID line of the live mux that holds it

## Summary

Step 1 of the mux (`acquire_mux_lock` in `src/lsp/mux/process.rs`, formerly inline in `run`) creates the lock file with `std::fs::File::create`, which opens with truncation, and only then tries the flock. A mux that loses the lock (another mux is live) has already emptied the file in which the holder wrote its PID for diagnostics.

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: a refused attempt that still changes the state of the thing it was refused, so the loser's failed start damages the winner's record.

## Symptom (Effect)

Not observed in a run of the product. What the PID line is for: `writeln!(&lock_file, "{}", std::process::id())` in `run` is labelled "Write PID for diagnostics"; after a refused second start the file is empty, so that diagnostic names no holder.

## Reproduction

At the OS level, measured 2026-10-18 with python (`fcntl.flock` as the holder, then an `open(path, 'w')`, which is what `File::create` does):

```
before='12345\n' second-flock=refused after=''
```

Through the Rust code, measured 2026-10-18 by the independent review of `fix/mux-connect-2026-10-18`: a lock holder that took the flock with `OpenOptions` (no truncate) and wrote `12345\n`, then `acquire_mux_lock` on the same path:

```
refused=true before="12345\n" after=""
```

I reproduced the same line through the Rust path myself (a throwaway test, removed): `MUX1018-F7 refused=true before="12345\n" after=""`.

Nothing in the codebase reads the PID line: holder-gone detection uses the flock (`claim_mux_lock`), not the file's content, so the connect-path fixes of `2026-10-17-mux-connect-path-lock-races.md` are unaffected. The line is a human diagnostic only.

## Environment

Linux, branch `fix/mux-connect-2026-10-18`.

## Root cause

inferred from `acquire_mux_lock` (`std::fs::File::create(lock_path)` before `try_lock_exclusive`) and measured both at the OS level and through the Rust function as above. The flock is advisory and does not stop the truncation.

## Evidence

The python line above and the Rust-path line.

## Hypotheses tried

None beyond the measurement.

## Fix

Unknown, not implemented. Option: open with `OpenOptions::new().create(true).write(true).truncate(false)` (as `claim_mux_lock` in `src/lsp/manager.rs` does), try the flock, and truncate and write the PID only after it succeeds.

## Tests added

N/A: open.

## Workarounds

None needed; the PID line is a diagnostic only.

## Resume

1. Extend the give-up test to read the lock file and see it empty (red). 2. Move the truncation after the flock.

## References

- `src/lsp/mux/process.rs` (`acquire_mux_lock`, `run`)
- `docs/issues/2026-10-17-mux-connect-path-lock-races.md`
