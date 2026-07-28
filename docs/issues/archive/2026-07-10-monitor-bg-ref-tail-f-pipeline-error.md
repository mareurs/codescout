---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- monitor
- background
- buffer-ref
- run_command
topic: null
time_scope: null
closed: '2026-07-13'
opened: '2026-07-10'
owner: marius
related: []
severity: medium
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

**Shipped on `experiments` in `b33ad329`** (`fix(run_command): resolve @bg refs to the live log so tail -f works`). Archive after cherry-pick to `master`.

Root cause (confirmed): `OutputBuffer::resolve_refs`'s `@bg_*` branch read the log into a fresh read-only `NamedTempFile` snapshot and substituted THAT path. A snapshot never grows, so `tail -f @bg_xxx` followed a dead inode and blocked until timeout. Fix: substitute the live `log_path` directly, record it in `temp_path_strings` (buffer-only classification preserved) but NOT in `temp_paths` (the live job log must not be deleted after the command). Missing log → clear `RecoverableError`.
## Tests added

`resolve_refs_bg_substitutes_live_log_not_snapshot` (`src/tools/output_buffer.rs`): asserts a `@bg_` ref resolves to the live log path, that path is not queued for temp cleanup, and the command is still classified buffer-only. RED before (resolved to a `/tmp/.tmpXXXX` snapshot); GREEN after. Existing `resolve_refs_bg_rejects_err_suffix` + is_buffer_only tests still pass.
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
