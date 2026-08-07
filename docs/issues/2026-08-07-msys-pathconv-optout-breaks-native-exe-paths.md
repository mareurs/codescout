---
id: bb584800df9f20ba
kind: bug
status: fixed
title: MSYS_NO_PATHCONV/MSYS2_ARG_CONV_EXCL in shell_command_configured break MSYS-style paths passed to native Windows binaries
tags:
- windows
- run_command
- path-handling
- self-inflicted
- regression
closed: 2026-08-07
---

## Summary

`platform::shell_command_configured` (Windows) sets, as of d564c9bb:

```rust
.env("MSYS_NO_PATHCONV", "1")
.env("MSYS2_ARG_CONV_EXCL", "*")
```

These disable MSYS's automatic translation of Unix-style absolute paths into
Windows form when a **native Windows binary** is invoked. The result is that
`git -C /c/Users/... `, `cargo --manifest-path /c/...`, `python /c/...` and
similar all fail with "No such file or directory", even though the same path
works for every MSYS binary.

This is self-inflicted and the justification in d564c9bb's commit message is
**wrong**.

## Reproduction

```
$ git -C /c/Users/me/work/repo rev-parse --abbrev-ref HEAD
fatal: cannot change to '/c/Users/me/work/repo': No such file or directory

$ env -u MSYS_NO_PATHCONV -u MSYS2_ARG_CONV_EXCL \
    git -C /c/Users/me/work/repo rev-parse --abbrev-ref HEAD
main                                    # works

$ git -C 'C:/Users/me/work/repo' rev-parse --abbrev-ref HEAD
main                                    # works

$ ls -d /c/Users/me/work/repo
/c/Users/me/work/repo                   # MSYS binary: unaffected either way
```

## Why the original justification was wrong

d564c9bb claimed the opt-outs were needed so MSYS "does not rewrite
Unix-looking arguments inside the -c script (which would corrupt `sed 's/a/b/'`,
`find / -name x`)".

`sed`, `find`, `grep` and `awk` in Git Bash are **MSYS binaries**. MSYS argument
conversion applies only when crossing to a *native* Windows executable — it never
touches arguments handed to another MSYS program. So the failure mode the
opt-outs were added to prevent could not occur, while the behaviour they disable
(converting `/c/...` to `C:/...` for native exes) is actively useful and is Git
Bash's default.

The cost is asymmetric and falls on the common case: an agent reading `pwd`
output (`/c/Users/...`) and feeding it to `git`/`cargo`/`python` is the natural
thing to do, and it now breaks.

## Impact

Any `run_command` invocation that passes an MSYS-style absolute path to a native
Windows binary. Discovered when `git -C /c/.../claude-plugins status` failed
while committing tracker updates — i.e. within the same session that introduced
it.

Not caught by the test suite: no test passes an MSYS-form path to a native exe
through `run_command`. Same shape as the other three misses this session — the
check could not reach the behaviour (see W-2 in
`claude-plugins/docs/trackers/windows-shell-env-session-log.md`).

## Fix

Remove both `.env(...)` lines, restoring Git Bash's default conversion. Add a
regression test that runs a native binary with an MSYS-form path argument (e.g.
`git -C <msys-form project root> rev-parse --show-toplevel`) and asserts it
resolves.

If a specific future command genuinely needs conversion suppressed, scope it to
that command rather than to every command the server ever runs.

## Fix

`94a63c32` (experiments) — `src/platform/windows.rs`.

The two `.env(...)` calls are replaced by `.env_remove(...)`, not merely dropped.
Removing them from the child environment means an exported value in the parent
shell cannot change how commands resolve — the same reasoning that pins
`GIT_PAGER=cat`. A shell codescout hands to an agent should behave identically
regardless of what the launching environment happens to export.

The doc comment on `shell_command_configured` was rewritten too. The old one
asserted the opt-outs protected `sed 's/a/b/'` and `find / -name x`; that claim
is false and was the reason the change looked justified in review, so leaving it
in place would have invited the same mistake again.

Also documented in `docs/manual/src/tools/workflow-and-config.md`: MSYS-form
paths work on Windows including as arguments to native binaries.

## Tests added

`src/platform/windows.rs`, `tests::msys_form_path_resolves_for_native_binaries`.

It drives `git -C '<msys-form path>' rev-parse --git-dir` and asserts the stderr
does **not** contain `cannot change to`. `git` may still fail with "not a git
repository" — that is fine and is the point: it proves git resolved and entered
the directory.

The test asserts on the **native** side of the boundary deliberately. A test
driving only MSYS builtins (`ls /c/...`) passes either way, because MSYS programs
resolve MSYS paths themselves and never see the conversion. That is precisely the
green-check-that-cannot-fail which let the regression ship in the first place —
the original commit's own test suite was 134 tests green.

**Verified to discriminate:** the failure was reproduced live against the
previously built binary (`fatal: cannot change to '/c/Users/...': No such file or
directory`) before the fix, and the test passes after it.

## Resume

N/A — fixed. One residual bookkeeping step: record the **master-side** SHA after
cherry-pick. `94a63c32` is an `experiments` SHA and orphans on rebase.
