---
id: bb584800df9f20ba
kind: bug
status: open
title: MSYS_NO_PATHCONV/MSYS2_ARG_CONV_EXCL in shell_command_configured break MSYS-style paths passed to native Windows binaries
tags:
- windows
- run_command
- path-handling
- self-inflicted
- regression
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

## Resume

Not yet fixed — filed at the end of the session that introduced it. The change is
a two-line deletion plus a test; it needs the usual gate, a `cargo rb`, and a
`/mcp` reload. Until then, pass Windows-form paths (`C:/Users/...`) to native
binaries from `run_command`; MSYS-form works fine for `ls`, `grep`, `cat`, etc.

