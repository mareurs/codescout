---
status: open
opened: 2026-07-10
closed:
severity: medium
owner: marius
related: []
tags: [monitor, background, buffer-ref, run_command]
kind: bug
---

# BUG: Monitor pipeline errors when a `@bg`/`@cmd_*` background ref is fed to `tail -f`

## Summary
A monitor pipeline built around a background command ref (`@bg` / `@cmd_*`)
errored: the ref does not play well with `tail -f`. Streaming a
still-growing background buffer through `tail -f` inside a Monitor command
fails instead of following the buffer as it grows.

## Symptom (Effect)
*Reported verbatim by the user:*

```
the monitor pipeline errored (the @bg ref doesn't play well with tail -f)
```

Exact error string, exit code, and which layer emitted it (the Monitor
harness vs. codescout's `run_command` gate vs. the shell) not yet captured.

## Reproduction
Not yet reproducible — best lead: start a long-running command with
`run_command(run_in_background=true)` (yielding a `@bg`/`@cmd_*` handle),
then attempt to follow its buffer with a Monitor command such as
`tail -f @cmd_xxx | grep --line-buffered PATTERN`. Capture the exact error
and note whether `tail -f` on a `@`-ref buffer is expected to work at all
(the buffer may be a snapshot handle, not a live-growing file descriptor
that `tail -f` can follow).

## Environment
- Platform: linux, bash
- Project: codescout (`/home/marius/work/claude/codescout`), branch `experiments`
- Transport: MCP (stdio)
- Surfaces involved: Monitor tool + codescout `run_command` background refs

## Root cause
Unknown — under investigation. Leading hypothesis: a `@`-ref buffer handle
is not a live file `tail -f` can `inotify`-follow (it resolves to a
server-side snapshot or a pipe, not an appendable on-disk path), so
`tail -f` either errors on the argument or never receives new lines.

## Evidence
Pending — needs the exact error string and the Monitor command that
produced it.

## Hypotheses tried
*(none yet — captured on notice)*

## Fix
Plan pending root-cause confirmation. Likely one of: (a) document that
`@`-ref buffers are not `tail -f`-followable and give the supported pattern
for streaming a background command, or (b) back background refs with a real
appendable file path so `tail -f` works.

## Tests added
N/A — not yet fixed.

## Workarounds
Avoid `tail -f` on a `@`-ref buffer in a Monitor pipeline. To follow a
long-running command's output live, redirect it to a real file
(`... > /path/run.log 2>&1`) and `tail -f` that file instead.

## Resume
Reproduce per the Reproduction lead, capture the verbatim error and the
emitting layer, then confirm whether `@`-ref buffers back onto an
appendable file. Decide between the "document the limitation" and "back
refs with a real file" fixes based on that.

## References
- Progressive-disclosure `@ref` buffer semantics: `get_guide("progressive-disclosure")` (@cmd_*/@bg buffers are server-side handles)
