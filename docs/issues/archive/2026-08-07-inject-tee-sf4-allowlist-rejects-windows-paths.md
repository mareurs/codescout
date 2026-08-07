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

`d564c9bb` (experiments), completed by `20d12b5f` (experiments).

Landed as part of the bash-only shell change:

- The path is now rendered via `platform::shell_path_str`, which emits the
  forward-slash form (`C:/Users/.../Temp/...`) — required regardless, because
  Git Bash executes the command and would treat `\` as an escape.
- `:` added to the allowlist for the drive-letter prefix. It is not a shell
  metacharacter inside a word, so the SF-4 property (no injectable
  metacharacters in an interpolated path) is preserved.
- `~` added in `20d12b5f`. The first fix shipped incomplete: `:` covered the
  drive letter but not the `~` that 8.3 short names put in `%TEMP%`
  (`MAILIN~1.002`), so `inject_tee` stayed unreachable on this host after the
  commit that claimed to fix it.

## Tests

The first fix (d564c9bb) was **incomplete and shipped that way**: it added `:`
for the drive letter but missed `~`, which Windows uses in 8.3 short names.
`%TEMP%` resolves through the short form whenever the account name is long or
contains a dot — here `C:/Users/MAILIN~1.002/AppData/Local/Temp/...` — so tee
injection still failed after the "fix", just with a different rejected character.

It slipped through because the `run_command` suite exercises `inject_tee` only
indirectly, via end-to-end commands, and the buffer-only path (`grep … @cmd_x |
head`) **returns before tee injection entirely**, so the green suite proved
nothing about this check. It was caught by running a real non-buffer-only
filtered command (`ls src/platform | head -3`) against the live rebuilt server.

Follow-up adds the direct unit coverage the Resume section asked for:
`tee_path_is_safe` is now an extracted pure function with tests over the real
platform temp-path shapes (POSIX, Windows long name, Windows 8.3 with `~`) and
over shell metacharacters — including `'`, which is the one character that could
escape the new single-quoting.
## Resume

N/A for master — decided 2026-08-07 that no cherry-pick is planned. `master` is
897 commits behind `experiments` and 0 ahead (its HEAD is the merge base), so
`experiments` is a strict superset and is where this work lives. The SHAs above
are `experiments` SHAs and stay that way; re-open this line only if master is
ever fast-forwarded or a cherry-pick is scheduled.

Otherwise done — `tee_path_is_safe` is extracted and directly unit-tested, so a future
re-tightening of the allowlist fails a fast test instead of only surfacing
through an end-to-end command on a machine whose temp path happens to contain
the offending character.

Remaining nuance worth knowing: the allowlist is now a tripwire, not the primary
defence — the path is single-quoted at the interpolation site. Quoting is safe
here specifically because `inject_tee` runs *after* `is_buffer_only` has been
computed, so it cannot perturb that classification (unlike `resolve_refs`, where
quoting would reclassify buffer-only commands past the dangerous-command gate).
