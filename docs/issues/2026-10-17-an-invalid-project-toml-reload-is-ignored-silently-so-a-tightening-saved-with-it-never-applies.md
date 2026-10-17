---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md, docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: a project.toml edit that fails to load is ignored in a running session, so a tightening saved with an invalid entry never takes effect

## Summary

`ActiveProject::reload_config_from_disk` (`src/agent/mod.rs`) keeps the last good config when the
re-read fails, advances the file stamp, and says nothing. A user who saves `file_write_enabled =
false` in the same edit as any entry the loader refuses keeps writes ENABLED, silently, until the
file is fixed or the server restarted. The loader gained new refusals on branch
`fix/write-allowlist-port-2026-10-17` (a relative, empty, `~` or `..` `extra_write_roots` entry), so
a class of edits that used to load now takes this path. The reload policy itself is older and is
not changed here.

## Class fit

A failure that has no reader: the config is rejected and the only trace is the absence of the
change the user asked for, on the one setting (a security tightening) where a missed change is
unsafe. Filed unclassified; it is the same silent-failure shape as the other "drop the whole
input" paths, not a path-containment fault.

## Symptom (Effect)

Real binary (`codescout start --project <p>`, decoy `HOME`, scrubbed environment; driver
`port/mcp_r.py` scenario B4 in the session scratchpad), measured 2026-10-17 on the branch before the
follow-up commits:

```
B4a create_file({"path": "a.txt", "content": "a\n"}) isError=False   {"status": "ok", ...}
# project.toml rewritten: [security] file_write_enabled = false, extra_write_roots = ["~/scratch"]
B4b create_file({"path": "b.txt", "content": "b\n"}) isError=False   {"status": "ok", ...}
  b.txt exists: True
```

The base (`experiments` 27efb11d) does not show it for this edit because `["~/scratch"]` loaded there, so
the tightening applied (reported by the reviewers; the run above is the branch). By reading, not
measured: an edit that carries an entry the base ALSO refuses, an `extra_write_roots` list over 128
entries, takes the same silent path on the base.

## Reproduction

`python3 mcp_r.py <codescout binary> <workdir>`; scenario B4 is the last block. Unit-level, the
same decision is `reload_config_from_disk`'s `if let Ok(fresh) = ... { self.config = fresh; }`.

## Environment

Linux, rustc 1.97.1, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

`reload_config_from_disk` swallows the `Err` from `ProjectConfig::load_or_default`. Its doc comment
states why: keeping the last good config means a malformed file cannot relax a gate by falling
back to permissive defaults, and advancing the stamp anyway means a file that stays broken is
parsed once instead of on every gated call. Both reasons stand; neither requires saying nothing.
Measured: the run above.

## Evidence

The run above. The doc comment cited is `src/agent/mod.rs:473-479` in the tree measured.

## Hypotheses tried

None.

## Fix

Not applied, by instruction: the reload policy is not changed here. Documented instead in
`docs/manual/src/configuration/project-toml.md` (a running session ignores an edit that fails to
load, including a tightening saved with it). Candidate directions, none chosen: surface the load
error on the next tool response, or apply a valid `file_write_enabled = false` even when another
field fails (per-field tolerance), or refuse writes until the file loads.

## Tests added

N/A. Not fixed.

## Workarounds

Restart the server after editing security settings; fix the entry the loader names first.

## Resume

Decide the reload policy for an edit that fails to load, with security tightenings as the case
that must not be lost.

## References

- `docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`
- `docs/issues/2026-10-17-write-allowlist-home-and-tmpdir-gaps.md`
