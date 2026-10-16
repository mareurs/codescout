---
id: '098fca009ecbdd78'
kind: bug
status: fixed
title: 'BUG: interactive run_command drops a finished command''s output, or elicits for a child that already exited'
tags:
- cluster/unclassified
closed: 2026-10-15
opened: 2026-10-13
owner: marius
related: []
severity: medium
---

# BUG: interactive run_command drops a finished command's output, or elicits for a child that already exited

## Summary
`run_command(interactive: true)` can return `exit_code: 0` with an empty `stdout` for a command that printed output. It can also ask the user for input on a child that has already exited. Both come from `drain_with_settle` and the single `try_wait()` after it. The failure rate is about 1% on an idle machine and about 50% under CPU load. The test `native_context_wire_reaches_interactive_shell` is the reproducer, and it made the full default `cargo test --workspace` stage red in 2 of 2 runs on 2026-10-13.

## Symptom (Effect)
The test panics at `src/server/native_context_tests.rs:279`. It shows one of two response shapes.

Shape A — the code asked for input on a child that had finished:

```
{"ok": false, "error": "User declined the elicitation request",
 "hint": "Re-issue the call with a more specific argument to avoid the disambiguation prompt"}
  left: Null
 right: "interactive-root|interactive-child|0.160.0"
```

Shape B — the command ran and exited 0, but its output is gone:

```
{"exit_code": 0, "stdout": "", "interactive_rounds": 1, ...}
  left: String("")
 right: "interactive-root|interactive-child|0.160.0"
```

The test wire declines every `elicitation/create`, so shape A shows as an error here. A real client would show the user a prompt for a process that is already finished.

## Reproduction
HEAD `802a706f` (working tree clean for this path). Build the lib test binary in a leased slot, then run the one test many times:

```
scripts/with-slot.sh cargo test --lib --no-run      # prints the executable path
<exe> server::guide_hint_tests::native_context_wire_reaches_interactive_shell --exact
```

Measured 2026-10-13 on a 64-core Linux 7.2.8-zen host, 200 runs per row:

| condition | pass | fail |
|---|---|---|
| idle | 198 | 2 |
| 128 busy-loop processes (2 per core) | 103 | 97 |

Under load, the first three captured failures were: iteration 4 shape A, iteration 5 shape A, iteration 6 shape B. The same test passed 3 of 3 alone and 2 of 2 in the whole `server::` module earlier the same day. It failed in 2 of 2 full default runs, and it passed in the later full gate run on `b336e43f`. So a single green run is not evidence that the race is gone.

## Environment
Linux 7.2.8-zen, 64 cores, branch `experiments`. The test came in with `18a007aa`. The code under test in `src/tools/run_command/interactive.rs` is older than that commit.

## Root cause

Measured by ablation on 2026-10-14, 200 runs per row, 128 busy-loop processes on a 64-core host. Each of two changes removed exactly one shape.

| build | idle fail | loaded shape A | loaded shape B | loaded pass |
|---|---|---|---|---|
| unmodified | 2 of 200 (A) | 89 | 10 | 101 |
| drain fix only | 0 of 200 | 81 | 0 | rest |
| drain fix plus bounded wait (prototype) | 0 of 200 | 0 | 0 | 200 |

1. **Shape B.** `drain_with_settle` selected over a stdout read and a stderr read and ended the whole drain on the first EOF or zero-byte read of either pipe. `tokio::select!` picks a ready arm at random. When the stderr arm won, the drain ended while stdout bytes were unread. The drain fix removed this shape (10 failures to 0).
2. **Shape A.** The caller called `child.try_wait()` once after the drain. A process closes its file descriptors during exit, before it is waitable, so `try_wait()` could return `Ok(None)` after EOF. The code then reached `ctx.elicit` for an exited child. The bounded wait removed this shape (81 failures to 0).
3. The post-exit and final drains used the same function, so they had the same early-break behaviour.

## Evidence
- `/home/marius/.cache/cs-review/flake-repro.log` — the two-row table above (session scratch, not in the repo).
- `/home/marius/.cache/cs-review/flake-final.log` — the committed fix (`eac0acb7`), binary built from the final code: idle 200 of 200 pass, loaded (128 busy loops) 200 of 200 pass, shape A 0, shape B 0.
- `/home/marius/.cache/cs-review/flake-failures.log` — the three captured failures: shapes A, A, B.
- `/home/marius/.cache/cs-review/default-rerun.log` lines 3388 and 6812-6817 — the full-suite failure, shape A.

## Hypotheses tried

1. **Hypothesis** — the failure is a load-dependent race in `interactive.rs`, not a fault in the test wire. **Test** — 200 runs idle and 200 under load, plus captured failures. **Verdict** — confirmed. **Evidence** — the table in Reproduction.
2. **Hypothesis** — only the elicitation path is affected (shape A). **Test** — captured failure outputs under load. **Verdict** — rejected: shape B also occurs, with no elicitation involved.
3. **Hypothesis** — shape B comes from the drain ending on one stream's EOF, and shape A from `try_wait()` right after EOF. **Test** — ablation: drain fix alone, then drain fix plus bounded wait (table in Root cause). **Verdict** — confirmed. Each change removed exactly one shape.

## Fix

`drain_with_settle` is now a module-level function over any `AsyncRead` pair. It retires each stream at its own EOF or read error and loops until both are done or the settle window passes. It returns `(output, both_eof)`.

A new function `child_exit(child, both_eof, bound)` decides whether the child exited. When both pipes are closed it awaits `child.wait()` for at most `EXIT_WAIT_BOUND` (2 s) and returns `Ok(None)` on timeout. Otherwise it calls `try_wait()`. The interactive loop uses it in place of the bare `try_wait()`.

Fix commit: `eac0acb7` (branch `experiments`, local at the time of writing). Patch-id: `fb932c79995d5fe4cafa4c676bea5985cd1b7d82` (`git show eac0acb7 | git patch-id --stable`).

## Tests added

Three deterministic tests in `src/tools/run_command/tests.rs`:

- `the_interactive_drain_keeps_one_streams_bytes_when_the_other_is_already_at_eof` — 64 passes, both orientations.
- `the_interactive_drain_ends_on_the_settle_window_while_a_pipe_is_open_and_quiet` — a quiet open pipe still ends on the settle window and reports `both_eof == false`.
- `child_exit_waits_once_both_pipes_are_closed_and_only_that_long` (unix) — a child that closes its fds and sleeps 0.3 s is waited for; a child that sleeps 5 s hits the bound.

Mutation checks: changing both EOF arms to `break` failed only the drain-bytes test. Changing `if both_eof` to `if both_eof && false` failed only the `child_exit` test. The gate (`--check`) on the fix was green: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Workarounds
Use `run_command` without `interactive: true` for commands that do not need stdin.

## Resume

Done. Open item: this file is untracked because the commit hook requires its slug on the `cluster/unclassified` `**Members:**` line of `docs/trackers/issue-clusters.md`, and a peer holds an unstaged edit on that line.

## References
- `src/tools/run_command/interactive.rs:102-132`, `:156`, `:162`, `:166`, `:193`
- `src/server/native_context_tests.rs:270-284`
- Commit `18a007aa` introduced the test.
