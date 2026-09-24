---
id: b2670c8516368cae
kind: bug
status: open
title: 'BUG: run_command refuses a filter-terminated command when /tmp is full, because its optional tee capture cannot create a temp file'
owners:
- marius
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
related: []
severity: low
---

# BUG: run_command refuses a filter-terminated command when /tmp is full, because its optional tee capture cannot create a temp file

## Summary

For a command whose last pipe stage is a filter (`… | head`, `… | grep`), `run_command` splices in `tee /tmp/codescout-unfiltered-XXXX` so the unfiltered stream can be offered as a bonus `@cmd_*` buffer. It creates that temp file first, and a failure propagates with `?` — so when `/tmp` is full, the **command itself is refused**, not just the bonus buffer. With native `Bash` denied on this machine since 2026-09-20, that leaves a session with no working shell at the moment it most needs one to diagnose the full disk.

## Symptom (Effect)

Measured 2026-09-24 ~15:18Z. `/tmp` is a 63 G tmpfs mounted with `nr_inodes=1048576`, and it was out of space. Most likely it ran out of **inodes**, not bytes (see *Evidence*). Two consecutive calls:

```
run_command("du -sh /tmp/* 2>/dev/null | sort -rh | head -8; ls -la /tmp | grep -c rustc")
→ No space left on device (os error 28) at path "/tmp/codescout-unfiltered-dKXImf"

run_command("df -h /tmp | tail -1; du -xsh /tmp/* 2>/dev/null | sort -rh | head -8")
→ No space left on device (os error 28) at path "/tmp/codescout-unfiltered-Jzqhw0"
```

The error is loud and names the path — this is not silent. What it costs is the command: nothing ran.

## Reproduction

Not reproduced on demand — needs a full `/tmp`. Best lead: point `TMPDIR` at a tiny full filesystem (or a read-only directory) for a test server and call `run_command("echo hi | head -1")`; expect the refusal above. `echo hi` with no filter is the control (see Root cause for why it should succeed).

`experiments` @ `c722204a`.

## Environment

Linux, `/tmp` on tmpfs (63 G), codescout release binary over stdio, native `Bash` in `permissions.deny` for all three profiles.

## Root cause

`inject_tee` (`src/tools/run_command/inner.rs:161-214`) creates the capture file with `tempfile::Builder::new().prefix("codescout-unfiltered-").tempfile()?` (`:179-181`) and `tmppath.keep()?`, and its caller (`:398`) propagates the error, so the whole call fails. The file only exists to feed an optional unfiltered buffer; the command it instruments does not need it.

Scope, from the same read: the temp file is created **only** when `detect_terminal_filter` finds a trailing filter. A command with no terminal filter takes the `else` branch and never touches `/tmp` here — **inferred from `inner.rs:176-213`, not measured** under a full `/tmp`. Whether any other path in `run_command` writes to `/tmp` was not checked.

## Evidence

The two refusals above, and `Monitor` (which runs a plain shell and has no such capture) working throughout the same window.

**What was exhausted is inferred, not measured at the instant.** `ENOSPC` came while bytes were NOT full: `df` from `Monitor` a minute later read `tmpfs 63G 18G 46G`, and a 30-minute watcher alerting on bytes above 50 G never fired. `Glob` counted 365,356 files in `/tmp` at the time. After the operator's cleanup, `df -i` reads 169,283 of 1,048,576 inodes used (17%), with bytes at 13 G. That fits inode exhaustion under `nr_inodes=1048576`. `df -i` was not taken at the failure, so the byte-fill reading this file first gave is withdrawn rather than replaced by a certainty. The defect is the same either way: `tempfile()` fails on either kind of `ENOSPC`.

## Hypotheses tried

N/A — the mechanism is a direct read of `inject_tee`.

## Fix

Not started. Degrade instead of refusing: on a failed `tempfile()` / `keep()`, run the command un-teed and say in the response that the unfiltered buffer is unavailable and why. The same principle `5e51e72f` applied to the principal stamp — a best-effort side channel must never turn into a refused call.

## Tests added

None — this record opens the defect. A fix wants a test that forces the temp-file creation to fail (a `TMPDIR` pointing at a non-writable directory) and asserts the command still runs and the response names the missing buffer. Pair it with the no-filter control, so the test cannot pass by never reaching `inject_tee`.

## Workarounds

While `/tmp` is full, drop the trailing filter (run bare, query the `@cmd_*` buffer — Iron Law 3's own recommendation), or use `Monitor` for one-off shell reads.

## Resume

Implement the degrade path; confirm the `inferred` scope claim above with the no-filter control while doing so.

## References

- `docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md` — the same `inject_tee`, a different defect
