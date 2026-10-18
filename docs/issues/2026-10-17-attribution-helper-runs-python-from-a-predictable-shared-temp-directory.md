---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: the attribution helper runs python from `$TMPDIR/codescout-attribution-<hash>`, a directory name anyone can compute and pre-create, so a planted `json.py` runs as the codescout user

## Summary

`materialize_into` (`src/tools/run_command/attribution.rs:247-271`) extracts the embedded scripts into
`$TMPDIR/codescout-attribution-<DefaultHasher of the contents>`. `DefaultHasher::new()` has fixed keys, so the name
is computable; `create_dir_all` accepts an existing directory (or a symlink to one) whatever its owner; a script
whose bytes already match is skipped; and the engine is started as `python3 <script>`
(`attribution.rs:404-405`), which puts the script's directory first on `sys.path`. `scripts/attribute-red.py`
imports `json`, `re`, `subprocess`, `pathlib` (lines 51-57), so a `json.py` planted in that directory runs.

## Class fit

Not the staging-link defect, although it was found while fixing it: this is trust placed in a directory in a
shared location, not a write that follows a link. Filed unclassified; the closest relation is the
`atomic_write` issue, which fixed only this function's STAGING file.

## Symptom (Effect)

On a multi-user host with a shared `/tmp`, another local user who creates the directory first (and puts `json.py`
in it, alongside the scripts, which are public) gets their code run as the codescout user the next time a
failure-shaped command triggers the attribution engine.

## Reproduction

The Python half was run 2026-10-17 in a scratch directory (no real `/tmp` directory was touched): a copy of
`scripts/attribute-red.py` and a `json.py` that writes a marker file and raises, then
`python3 <dir>/attribute-red.py --explain`:

```
MARKER-json-py-ran     # the planted json.py executed
ImportError: planted   # from File ".../dir/json.py", line 3, imported at attribute-red.py line 52 (import json)
```

The Rust half (an attacker-created or foreign-owned directory being reused, no owner or mode check) is READ from
`attribution.rs:247-271`, not run: a cross-user reproduction needs a second account.

## Environment

Linux, `python3` on PATH, `std::env::temp_dir()` (a shared `/tmp` is the exposure; a per-user `TMPDIR` is not).

## Root cause

`materialize_into`: deterministic directory name, `create_dir_all` without an owner/mode/symlink check, skip when
content matches, and nothing removes foreign files in the directory; plus `python3 <script>` without `-I`/`-s`/`-P`
so the script directory is importable. Python half measured; Rust half inferred from the code.

## Evidence

Output above. The `atomic_write` fix (`b493599d`) changed only the staging file in this function, so a planted
`<name>.<pid>.tmp` link is no longer followed. It does not address this.

## Hypotheses tried

None needed.

## Fix

Not applied. Candidates: run the engine with `python3 -I` (isolated mode drops the script directory from
`sys.path`), which closes the import hijack whatever the directory holds (measured 2026-10-17 on the scratch copy: with `-I` the planted `json.py` did not run and `--explain` exited 0; the sibling-module imports of a full engine run under `-I` were not re-checked); and/or use a per-user directory
(`per_user_state_dir`) and check owner, mode and not-a-symlink before trusting it.

## Tests added

N/A: filed, not fixed.

## Workarounds

Use a per-user `TMPDIR` (for example `$XDG_RUNTIME_DIR`).

## Resume

Add `-I`, add a test that a planted `json.py` beside the entry is not imported, then decide on the directory checks.

## References

- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
