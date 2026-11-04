---
id: f9abe7185324eec2
kind: bug
status: open
title: grep rewrites a backslash in a file name to a slash, so the listed path names a file that does not exist
tags:
- cluster/addressing-without-an-escape-hatch
opened: 2026-11-04
---

# BUG: grep rewrites a backslash in a file name to a slash, so the listed path names a file that does not exist

## Summary

`grep` (files mode, and the default content mode) reports every matching file through `to_forward_slash(entry.path())`. On Linux a backslash is an ordinary file-name character, so a file named `a\b.txt` is listed as `a/b.txt`: a different path that does not exist. A caller that feeds the listed path to `read_file` or `edit_file` addresses the wrong file. Observed incidentally in Task 4 of the harness-delivered-hints server slice; re-verified below.

## Class fit

`cluster/addressing-without-an-escape-hatch`. The path field is a name the caller feeds back as an address, and the display form turns a legal file-name character into the separator. A name containing a backslash can no longer be written down, so the file cannot be addressed from the tool's own output.

## Symptom (Effect)

Listed path differs from the real path:

```
grep(mode="files", pattern="needle", path=<dir>)  ->  <dir>/a/b.txt
grep(pattern="needle", path=<dir>)                ->  <dir>/a/b.txt (1)   1: needle
```

The file on disk is `<dir>/a\b.txt`.

## Reproduction

Run 2026-11-05 on branch `hints/server-slice` (source identical to the running server's at this site), Linux:

```
mkdir -p $SCRATCH/bs-check && printf 'needle\n' > "$SCRATCH/bs-check/a\\b.txt"
ls -b $SCRATCH/bs-check          # a\\b.txt   (one literal backslash, ls -b escapes it)
grep(mode="files", pattern="needle", path="$SCRATCH/bs-check")
grep(pattern="needle", path="$SCRATCH/bs-check")
```

Both greps printed `$SCRATCH/bs-check/a/b.txt`; no output named `a\b.txt`. `$SCRATCH` was the session scratchpad directory outside the project.

## Environment

Linux 7.2, codescout 0.15.0, branch `hints/server-slice`.

## Root cause

measured 2026-11-05: the repro above printed `a/b.txt` for a file whose name is `a\b.txt`. The mechanism is read from `src/tools/grep.rs`, not otherwise instrumented: the per-file count key (`counts.entry(to_forward_slash(entry.path()))`, ~L289), and the `"file": to_forward_slash(entry.path())` of each content match (~L352, ~L396, ~L421) pass the real path through `crate::util::fs::to_forward_slash`, which replaces every `\` with `/` regardless of platform. Intended for Windows separators, it also rewrites a backslash that is part of a Unix file name. Its own doc comment (`src/util/fs.rs`, `to_forward_slash`) records this as a caveat and bounds it to "string matching against the catalog — not file IO"; grep returns the rewritten string in its MCP response, outside that bound, so the caveat does not cover this use (read from the doc comment, not measured).

## Evidence

The two grep results quoted in Symptom, against `ls -b` showing the single literal backslash.

## Hypotheses tried

1. **Hypothesis** the rewrite is Windows-only. **Test** the repro on Linux. **Verdict** rejected: it happens on Linux.

## Fix

Not started. Direction: apply `to_forward_slash` only where a path separator can be `\` (cfg(windows)), or emit the path unchanged on Unix. A regression test would create a file whose name contains a backslash and assert the listed `file` equals the real path.

## Tests added

N/A: bug only filed; no fix yet.

## Workarounds

Avoid backslashes in file names; or locate such a file with `tree` or `run_command ls`.

## Resume

Open. First step: check `to_forward_slash` callers outside `grep.rs` for the same rewrite, then fix and add the regression test.

## References

- `src/tools/grep.rs` (`to_forward_slash(entry.path())`), `src/util/fs.rs` (`to_forward_slash`)
