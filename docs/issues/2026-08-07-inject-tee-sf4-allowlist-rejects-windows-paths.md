---
id: '52710fd548bee569'
kind: bug
status: fixed
title: inject_tee's SF-4 path allowlist rejects every Windows path, making tee injection unreachable on Windows
tags:
- windows
- run_command
- tee
- path-handling
---

## Summary

`inject_tee` (`src/tools/run_command/inner.rs`) validates the generated temp
path against an SF-4 safety allowlist before interpolating it into the command:

```rust
if !tmpfile
    .chars()
    .all(|c| c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
```

A Windows temp path is `C:\Users\...\Temp\codescout-unfiltered-XXXX` — it
contains `:` and `\`, **neither of which is in the allowlist**. So the check
fails for every path `tempfile` can produce on Windows, and the tool returns a
`RecoverableError` instead of running the command.

Net effect: on Windows, any command whose last pipe stage is a detected terminal
filter (`head`, `tail`, `grep`, `sed`, `awk`, …) and which passes the IL3 gate
(bounded LHS) **cannot run at all**.

## Reproduction

Live, against the shipped Windows binary:

```
run_command("dir | head -3")
→ temporary file path contains unexpected characters:
  C:\Users\MAILIN~1.002\AppData\Local\Temp\codescout-unfiltered-KR1bzf
```

## Why it went unnoticed

Two masking effects:

1. **IL3 already blocks the common shape.** Piping an *unbounded* producer
   (`cargo`, `git`, `rg`) into a trimmer is rejected earlier with the IL3
   violation error, so the most frequent commands never reach `inject_tee`.
   Only a bounded LHS (`dir`, `ls`, `cat`) gets far enough to hit this.
2. **`head`/`tail`/`grep` did not exist under `cmd.exe` anyway**, so a user who
   worked around the error found the command failed for an unrelated reason
   (`'head' is not recognized`) and would reasonably attribute it to that.

The allowlist comment ("contains only alphanumeric chars, hyphens, and dots —
no shell metacharacters") describes a POSIX `$TMPDIR` path and was evidently
never re-evaluated against `%TEMP%`.

## Fix

Landed as part of the bash-only shell change:

- The path is now rendered via `platform::shell_path_str`, which emits the
  forward-slash form (`C:/Users/.../Temp/...`) — required regardless, because
  Git Bash executes the command and would treat `\` as an escape.
- `:` added to the allowlist for the drive-letter prefix. It is not a shell
  metacharacter inside a word, so the SF-4 property (no injectable
  metacharacters in an interpolated path) is preserved.

## Tests

Covered indirectly by the `run_command` suite (134 passing). No dedicated
regression test was added for the allowlist itself — see Resume.

## Resume

Add a direct unit test for `inject_tee` asserting a generated temp path passes
the SF-4 check on the host platform. The current coverage would not catch a
future re-tightening of the allowlist that re-breaks Windows, because the
failure only surfaces through an end-to-end command with a terminal filter.

