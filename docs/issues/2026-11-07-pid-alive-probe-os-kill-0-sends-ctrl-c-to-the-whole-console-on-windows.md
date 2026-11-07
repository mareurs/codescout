---
kind: bug
status: open
title: 'BUG: _pid_alive probes liveness with os.kill(pid, 0), which on Windows sends Ctrl+C to every process on the console and reports dead pids as alive'
owners:
- marius
tags:
- cluster/blast-radius-exceeds-visibility
opened: 2026-11-07
severity: high
unverified: 'Verified on Windows only: the new test, four neighbouring suites (same 14 failures with and without the fix), and one controlled red run on a rebuilt binary (n = 1, no live peer on the helper''s console). Not run on Linux, not run through the full gate. That the old os.kill call is what ended Claude Code sessions remains an association: the Claude Code process is not attached to the console the helper shares, so how it died is not established.'
---

# BUG: _pid_alive probes liveness with os.kill(pid, 0), which on Windows sends Ctrl+C to every process on the console and reports dead pids as alive

## Summary
`scripts/file-provenance.py::_pid_alive` tests whether a process exists with `os.kill(int(pid), 0)`.
That is the POSIX idiom. On Windows `signal.CTRL_C_EVENT` is `0`, so the same call asks the console to
deliver Ctrl+C, and every process attached to the caller's console receives it. The same call also never
raises for a process that is gone, so the function answers `True` for dead pids.
`run_command` runs this helper (through `scripts/attribute-red.py`) on every result with a non-zero exit.

## Class fit
A signal addressed to one pid reaches every process on the console: the effect is wider than the set
the caller named and the caller sees no sign of it, which is the class's claim. It differs from the
founding members in that the audience is processes sharing a console, not peers sharing a checkout,
and in that the same call also returns a wrong answer (alive) for what it was asked about.

## Symptom (Effect)
Claude Code sessions on a Windows host ended repeatedly in one day, most often right after a `run_command`
that finished red (exit 101). Every red result carried
`wip_authors: red attribution is unavailable: its helper exited 1 (SystemError: <built-in function kill> returned a result with an exception set)`.

## Reproduction
Isolated, in a console of its own, signalling only sentinels the probe created (Windows 11, Python 3.11):
1. sentinel in the SAME console, `os.kill(sentinel_pid, 0)` returns `None`; the sentinel logs SIGINT and so does the prober;
2. sentinel in its OWN console: no SIGINT delivered; own process group (`CREATE_NEW_PROCESS_GROUP`): none delivered;
3. the real `_pid_alive`, loaded from `scripts/file-provenance.py`, on a live same-console sentinel: returns `True`, sentinel receives SIGINT;
4. the real `_pid_alive` on a dead pid: returns `True` (expected `False`).

## Environment
Windows 11, Python 3.11. Process tree of a live session: the agent process -> `codescout.exe` -> `cmd.exe` -> `node.exe`
(pyright-langserver). The console that `run_command` children run in holds `codescout.exe`, `cmd.exe`, `node.exe`,
`bash.exe` and `python.exe`, and does not hold the agent process (`GetConsoleProcessList`, read only).

## Root cause
`_pid_alive` is called with the pid taken from `sessions/<pid>.json` file names and with `rec["pid"]`: the pids
of live Claude Code sessions. On Windows `os.kill(pid, 0)` is `GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid)`; the
event goes to the whole console, not to `pid`, and does not fail for a pid that no longer exists.
Not established: the exact route by which a Ctrl+C on that console ends the agent process.

## Evidence
- Probe outputs above (all four cases).
- Per-server heartbeat logs: the last line of each of five servers that stopped in one day shows `op=run_command`;
  four of the five stopped within seconds to minutes of a red run. One red run did not.
- Python on Windows: `signal.CTRL_C_EVENT == 0`.
- A controlled red run on a rebuilt binary (a `printf` of a rustc-style diagnostic naming an uncommitted file, then
  `exit 1`) returned a complete `wip_authors` diagnostic; the `SystemError ... kill` note present on every red of
  the old image did not appear. That is one red with one diagnostic shape (n = 1), and it did not put a live
  process on the helper's console, which is the case that delivered Ctrl+C in the probes.

## Hypotheses tried
- The wake-on-job-end hook ends the session: rejected as the mechanism. It only polls the job record and
  exits 0, or 2 for a deliberate wake; the "Stop hook blocking error" text is how Claude Code labels that wake.

## Fix
`scripts/file-provenance.py`: `_pid_alive` dispatches on `sys.platform`. Windows goes to the new
`_pid_alive_windows`: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` then `GetExitCodeProcess == STILL_ACTIVE`;
access denied counts as alive; a non-numeric or out-of-DWORD-range pid is dead (ctypes truncates silently).
POSIX keeps the previous `os.kill(pid, 0)` body unchanged. Known residual: a process that exited with code 259
reads as alive; no pid-only probe can tell it apart.

## Tests added
`tests/test_pid_alive.py` (8 cases; 9 counting the Windows-only range case on Windows). Every case runs in a child
that owns its console (`CREATE_NEW_CONSOLE`), because the broken call interrupts whatever shares the caller's
console. Red first: on the unfixed code the child died before writing its result, as predicted. Green after the
fix: 8 passed.

Mutations applied by hand to `_pid_alive_windows`, each caught by one named test: exit-code check removed ->
`test_a_process_that_has_exited_is_reported_dead`; range guard removed ->
`test_a_pid_outside_the_dword_range_is_dead`; access denied read as dead ->
`test_a_process_that_exists_but_cannot_be_opened_is_alive`; non-numeric pid read as alive ->
`test_a_value_that_is_not_a_pid_is_dead`. Not mutated: the `GetExitCodeProcess`-failed branch (no way to make it
fail from a test).

Neighbouring suites that load this script (`test_history_cache`, `test_harness_provenance`,
`test_write_targets_cache`, `test_session_context`): 14 failed / 74 passed with the fix, and the same 14 fail
without it (Windows-environment causes such as `WinError 1314` for symlinks).

## Workarounds
Start Claude Code with `CODESCOUT_NO_WIP_ATTRIBUTION=1` in the launching shell's environment (disables the red
attribution; `src/tools/run_command/attribution.rs`, `DISABLE_ENV`). Verify the value reached the server: a
Claude Code settings `env` entry does not override an inherited variable.

## Resume
Not archived: the gate has not run. Still open:
1. Run `tests/test_pid_alive.py` on Linux (CI). The POSIX branch is untouched; the test's POSIX expectations
   (pid 1 alive, exited child dead, no SIGINT from signal 0) were reasoned, not run.
2. Run the full light Python lane and the Rust gate before archiving.
3. Watch further red `run_command` results on a fixed image. Only a run of reds with no session lost, in a session
   that still has live peers on its console, would link the old call to the session losses.

## References
- `scripts/file-provenance.py`, `src/tools/run_command/attribution.rs`
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` (same engine, different harm)
