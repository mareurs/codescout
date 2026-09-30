---
id: 49c319ed82f5dee9
kind: bug
status: fixed
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

**Classification checked 2026-09-25 — stays `cluster/unclassified`, deliberately.** The shape is *an
optional side channel's failure aborts the primary operation*, and none of the 24 IC claims states it:
`IC-7` (warm-up billed to the first caller) and `IC-15` (a parameter silently dropped) are the nearest and
both are about a different failure direction. Recorded so a census reads this as judged, not unexamined.
Checked by sessionId `e4fbc7ef-27b7-4707-8469-ccdffa8e4e92`.

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

Fixed in `c410a275831f2da3f9125e2340a35e9120a66e58` (`git show <sha> | git patch-id --stable`). `inject_tee` now returns a `TeeInjection { command, capture, skipped }`. On a failed `tempfile_in` or `keep()` it returns the command un-teed with `skipped` set, and `run_command_inner` attaches that as `unfiltered_output_skipped` on the response, naming the directory and the missing buffer. The `tee_path_is_safe` tripwire stays a refusal, since it guards a path the shell must not be handed. The capture directory is now an argument (`inject_tee_in`), with a `#[cfg(test)]` per-thread override for tests, and not process env (`docs/conventions/test-env-isolation.md`).

## Tests added

`tools::run_command::tests::a_tee_capture_that_cannot_be_created_degrades_instead_of_refusing` (the helper, with a no-filter control and a usable-directory positive so it cannot pass by always degrading or by never reaching the tee branch) and `a_full_tmp_does_not_refuse_a_filtered_command_and_the_response_says_so` (through `RunCommand.call`, covering the response key, with a healthy-capture control on the same call). **What they force:** the creation-failure branch, by pointing the capture at a directory that does not exist. That is the same `tempfile_in` error path a full `/tmp` takes, but NOT ENOSPC itself, which cannot be produced portably in a unit test.

Mutations, one per guarded site, run with `scripts/mutation-probe.sh`: the creation-failure branch restored to `Err(e.into())` was KILLED, and both tests failed. The response key renamed was KILLED by the tool-level test alone, and the helper test stayed green, so the two cover different things. **The `keep()` failure branch SURVIVED** (its `skip` replaced by an error): a test cannot make `keep()` fail without another seam, so that one branch is unreached by any test.

## Workarounds

While `/tmp` is full, drop the trailing filter (run bare, query the `@cmd_*` buffer — Iron Law 3's own recommendation), or use `Monitor` for one-off shell reads.

## Resume

Closed by `c410a275`. **Scope, resolved by reading:** the *Root cause* section's scope claim held for the Unix foreground path. Lines `inner.rs:412-455` at HEAD capture through pipes with no temp files, so the tee file was the only `/tmp` dependency there. The `#[cfg(windows)]` branch is different: it creates `codescout-cmd-out-` and `codescout-cmd-err-` capture files with `?`, so a full temp dir would still refuse EVERY command on Windows, filtered or not. That exposure is unfixed and untested here, and is noted for a separate bug if it matters. Still not measured under an actually full `/tmp`.

## Fix provenance

- **SHA:** `c410a275831f2da3f9125e2340a35e9120a66e58` (`experiments`)
- **patch-id:** `9ba99ef1c130be5e9adc96378724ec38670e16e0`

## References

- `docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md` — the same `inject_tee`, a different defect
