---
id: '510f27fe15220816'
kind: bug
status: open
title: 'BUG: run_command reads files on the read deny list (~/.ssh and the rest), because only the file tools consult it and the shell gates do not'
owners:
- marius
tags:
- cluster/unclassified
- security
- run_command
opened: 2026-10-27
related:
- docs/issues/archive/2026-09-28-run-command-returns-an-environment-dump-verbatim-so-a-credential-reaches-model-context.md
- docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md
severity: medium
unverified: 'Whether the shell is MEANT to be outside the read deny list is not established: no document found that says so, and the search was a regex over four docs directories, not a read of them. Only Linux was tested.'
---

# BUG: `run_command` reads files on the read deny list (`~/.ssh` and the rest), because only the file tools consult it and the shell gates do not

## Summary

`read_file`, `edit_file` and `create_file` refuse a path on the read deny list (`~/.ssh`, `~/.aws`, `~/.gnupg`, `/etc/shadow` and the others in `denied_read_prefixes` and `DEFAULT_DENIED_EXACT`), and they refuse a symbolic link that points into it. `run_command` does not. A `cat` of the same file, by its absolute path or through an in-project link, returns the contents. The output goes to the model's context and into the session transcript.

## Class fit

No class is established for this record. It carries the `cluster/unclassified` escape hatch on purpose: the roster was not read class by class, and forcing a fit would corrupt the counts promotion reads. A reader who knows the roster may retag it.

## Symptom (Effect)

With a canary file inside `~/.ssh`, every file tool refused it and `run_command` returned it:

| Route | Result |
|---|---|
| `read_file` on an in-project link to the canary | `access denied: ... is in a protected location` |
| `read_file` on the canary's absolute path | `access denied` |
| `edit_file` with the marker as `old_string`, on the link | `write denied: ... symbolic link to ... protected location` |
| `create_file` overwrite with `force`, on the link | `write denied` |
| `run_command` `cat` on the link | the marker, `exit_code` 0 |
| `run_command` `cat` on the absolute path | the marker, `exit_code` 0 |

The link adds nothing: the shell reads the file directly.

## Reproduction

Run 2026-10-27 on a rebuilt server (PID 4055182, binary built 13:16, after commit `018abc731`, built from a working tree that also held peers' uncommitted test edits). The canary holds a marker, never a key.

1. Take a manifest of `~/.ssh` first (names, sizes, mtimes, SHA-256 of the 28 regular files).
2. Create `~/.ssh/codescout-probe-e0ddd77d-canary` with no-clobber (`set -C`), mode 600, content `CODESCOUT-PROBE-CANARY-DO-NOT-DISCLOSE-7f3a9c`.
3. `run_command("cat /home/marius/.ssh/codescout-probe-e0ddd77d-canary")`: returns the marker.
4. Remove the canary, restore the directory mtime, and diff the manifest. The 28 file hashes were identical after the run.

## Environment

Linux, codescout 0.15.0 on `experiments`. macOS and Windows not tested.

## Root cause

Read from the code, not run:

- `is_dangerous_command` (`src/util/path_security.rs`) matches `DEFAULT_DANGEROUS_PATTERNS` (destructive commands) and the user's `shell_dangerous_patterns`. It has no deny-list lookup.
- `check_source_file_access` (same file) blocks a file-reading command that names a source-file extension. Its body contains no mention of the deny list (zero matches for `denied`, `deny_list` and `protected`). Its own doc says it "is not a security boundary".
- `denied_paths_with` has call sites in `validate_read_path_in`, the write classifier and `validate_approve_path_in`. None is in the `run_command` path.

So the deny list is enforced by the tools that take a path argument, and the shell takes a command string. Whether that is a design decision or an omission is not established: see Resume.

## Evidence

- The six-row table above, from live calls.
- `~/.ssh` after the probe: 28 file hashes identical to the baseline, the canary gone.
- Related, not the same: the archived record `2026-09-28-run-command-returns-an-environment-dump-verbatim-so-a-credential-reaches-model-context.md` redacts four token shapes (`gh[pousr]_`, `github_pat_`, `sk-`, `AKIA`/`ASIA`) from `run_command` output. By that record's own list, the text of a private key is not among the shapes. That reading was not run.

## Hypotheses tried

None. This record states a gap, not a malfunction.

## Fix

Not fixed. The choices, none costed:
- Make the shell gates ask the deny list for every path-like word, as the file tools do. Variable expansion, `bash -c`, `eval` and command substitution defeat a parser, so this narrows the gap and does not close it.
- Redact private-key blocks by shape in the output, as the archived record does for tokens.
- Declare the shell outside the deny list, and say so in the guide and the prompt surfaces, so the file tools' refusals are not read as a boundary.

## Tests added

N/A: the bug is open and nothing was changed.

## Workarounds

None inside codescout. A user who does not want a session to read a path can deny it in the harness's own permissions.

## Resume

Decision for Marius: is the shell meant to be exempt from the read deny list? If yes, the fix is a sentence in the guide, a test that pins the exemption, and no code. If no, pick a choice under Fix. A reproduction needs a canary inside the denied directory, never a real key.

## References

- `src/util/path_security.rs`: `denied_paths_with`, `is_dangerous_command`, `check_source_file_access`.
- `src/platform/unix.rs`: `denied_read_prefixes`.
- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`: the file tools' side, where these probes were first run.
