---
kind: bug
status: fixed
title: 'BUG: a second mux start truncates the lock file before its flock is tried, erasing the PID line of the live mux that holds it'
tags:
- cluster/unclassified
closed: 2026-10-27
fix_patch_id: a6eab46886c8122b71f3123e8153234222eb2ffa
fix_sha: a99be50a5 (experiments)
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

Fixed on `experiments` in `a99be50a5` (patch-id `a6eab46886c8122b71f3123e8153234222eb2ffa`). `acquire_mux_lock` (`src/lsp/mux/process.rs`) now opens the lock file with `OpenOptions::new().create(true).truncate(false).write(true)`, tries the flock, and calls `set_len(0)` only once the lock is held. A losing start never touches the file. A winner empties it first, so a short PID written over an older longer record leaves no tail. This is the option the bug proposed.

## Tests added

- `a_mux_start_that_loses_the_lock_leaves_the_holders_pid_line_alone`: a holder takes the flock without truncating and writes `12345\n`, a second `acquire_mux_lock` gives up, and the file must still read `12345\n`. RED was `left: ""`, `right: "12345\n"`.
- `a_mux_start_that_wins_the_lock_leaves_exactly_its_own_pid`: the guard for the other half of the change. A file holding an older, longer record ends up holding exactly the winner's PID. It passed before the fix and must keep passing.

The 22 `lsp::mux::process::tests` and the manager's `a_waiting_clients_lock_probe_never_makes_a_starting_mux_give_up` pass. The full `./scripts/gate.sh` was green (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, `PYTHON_LIGHT=0`), run in an isolated worktree holding `HEAD` plus this patch and the logging fix.

## Workarounds

None needed; the PID line is a diagnostic only.

## Resume

1. Extend the give-up test to read the lock file and see it empty (red). 2. Move the truncation after the flock.

## References

- `src/lsp/mux/process.rs` (`acquire_mux_lock`, `run`)
- `docs/issues/2026-10-17-mux-connect-path-lock-races.md`
