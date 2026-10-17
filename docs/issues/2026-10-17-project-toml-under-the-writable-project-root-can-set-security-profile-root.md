---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `.codescout/project.toml` sits under the always-writable project root, and `profile = "root"` in it turns every write gate off

## Summary

The project root is always a write root, so the write tools can edit
`<project>/.codescout/project.toml`. That file holds `[security] profile`, and
`classify_write_path_in` returns `Allowed` for every path when the profile is `Root`
(`src/util/path_security.rs`). The security config is re-read when the file's mtime moves
(`Agent::security_config_for_call`, `config_is_stale` then `reload_config_from_disk`), so the change
takes effect on the next tool call, not on the next activation. Measured end to end on 2026-10-17:
a `create_file` of that file with `profile = "root"` was accepted, and the next `create_file` to a
path outside the root, which had asked for an approval before, was written without one.

## Class fit

No existing class names a policy file that the policy it configures lets you write. Filed
unclassified; it is a design question, not an implementation slip.

## Symptom (Effect)

A session that can write inside the project root can write `profile = "root"` (or add
`extra_write_roots`, or change `file_write_enabled` / `shell_command_mode`) and from the next call
write anywhere the deny list does not name.

## Reproduction

Measured 2026-10-17 on branch `fix/write-allowlist-port-2026-10-17` (the tool path is unchanged
from `experiments` 27efb11d: no diff under `src/tools/`, and under `src/agent/` only a test). A
throwaway test, not committed (patch kept in the session scratchpad `port/toml-probe.patch`), built
the same `ToolContext` the `edit_file` tests use over a fresh project and called the real
`CreateFile` tool three times. The outside target was a file under `~/.cache`, removed afterwards.

```
project.toml exists before: false
1 create_file(outside) under the default profile -> Ok({"pending_ack": "@ack_...", "reason": "'/home/marius/.cache/cs-probe-root-profile.txt' is outside the project root", "hint": "create_file(path=\"@ack_...\") to write it and approve /home/marius/.cache for this session"})
  outside exists: false
2 create_file(.codescout/project.toml, "[project]...[security]\nprofile = \"root\"\n") -> Ok("ok")
3 create_file(outside) after the edit -> Ok("ok")
  outside exists: true
cleaned up; exists now: false
```

The classifier half, with the real library (`pathsec_probe` F9c/F9d, `experiments` 27efb11d, from the
comparison run in the session scratchpad `cmp-pathsec/probe-exp-c.txt`, not re-run on the branch):
`<project>/.codescout/project.toml` is `[ALLOWED]` under the default profile, and with
`profile = Root` every probe target, `~/.bashrc`, `~/.claude/settings.json`,
`~/.config/codescout/.env`, `/etc/probe-x`, `/home/otheruser/x` and `/probe-x` included, is
`[ALLOWED]`; only `Root` also admits `~/.ssh/authorized_keys`.

## Environment

Linux, rustc 1.97.1, codescout `experiments` 27efb11d, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

The security configuration is a file inside the sandbox it configures, and it is trusted on reload.
Measured 2026-10-17: the three-step run above, with the project root as `allowed[0]`.

## Evidence

The run above. Not measured: whether any later check refuses a profile change made through the
write tools in a real server session (the test used the in-process tool context, not an MCP
client); the search for a protection on `.codescout/project.toml` was by keyword in `src/`, not
exhaustive.

## Hypotheses tried

None.

## Fix

Not applied, and no change is proposed: whether the model-facing write tools should be able to
edit the file that sets their own sandbox is a design decision.

## Tests added

N/A. Not fixed.

## Workarounds

None needed for a trusted session; for an untrusted one, the profile is a property of the project
directory the session is pointed at.

## Resume

A design decision is needed before any fix.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
