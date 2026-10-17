---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
unverified: 'The cross-user case end to end (a directory really owned by another uid, through materialize_into) was not run: no second account, so the owner comparison is driven through a uid seam against a directory we own. The Windows build was not compiled; the cfg(unix) arms are gated and the non-unix arm is a subset. python3 -I is hardening only: a planted typing.py still runs under -I because file-provenance.py:91 re-inserts its own directory on sys.path; the directory checks are what close the hole.'
severity: medium
owner: marius
related: [docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: the attribution helper runs python from `$TMPDIR/codescout-attribution-<hash>`, a directory name anyone can compute and pre-create, so a planted `json.py` runs as the codescout user

## Summary

`materialize_into` (`src/tools/run_command/attribution.rs:247-273`) extracts the embedded scripts into
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
`attribution.rs:247-273`, not run: a cross-user reproduction needs a second account.

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

Two changes in `src/tools/run_command/attribution.rs`, the second being the one that closes the hole:

1. **The directory is trustworthy** (`script_dir`, `verify_private_dir`, `create_private_dir`, `fresh_private_dir`).
   The name is now `codescout-attribution-<euid>-<hash>`; it is created mode 0700 (`DirBuilder`, not
   `create_dir_all`) and RE-VERIFIED ON EVERY USE: `symlink_metadata` says a real directory (so a link,
   even to a good directory, is refused), owned by the effective uid, and no group/other write bit
   (read and execute bits are allowed: the scripts are public). On any refusal it logs a `warn` naming the
   path and the reason and falls back to `fresh_private_dir`: a `mkdtemp` directory (unpredictable name,
   0700 requested explicitly, verified, then deliberately kept for the life of the process).
   `tempfile` creates directories with the umask's view of 0777, so the explicit mode matters: under
   umask 002 the default would be group-writable and fail the check, leaving no fallback.
   Single-user behaviour is unchanged: same cache reuse (no rewrite on the second call), same cost.
2. **The engine is spawned as `python3 -I <script>`** (`engine_std_command`).

**`-I` alone does NOT close the issue, which the issue's own candidate fix assumed.** Measured
2026-10-17 on a scratch copy of all six scripts with `json`, `dataclasses`, `decimal`, `typing`, `sqlite3`
and `functools` planted: plain `python3` runs `json` and `dataclasses`; under `-I` those do not run, but
`typing.py` still does, because `file-provenance.py:91` does `sys.path.insert(0, <its own dir>)` before
`from typing import Iterable` (:95). So `-I` closes the engine entry's own imports, `PYTHON*` and the user
site, and no more. With the directory verified the residual is not reachable by another user. Left as it
is: the insert is how `file-provenance.py` runs standalone.

End to end under `-I` (scratch fixture, identical stdin, plain vs `-I`): both exit 0, stdout byte-identical
(470 bytes, the full three-stage answer naming `src/held.rs` and the coverage gap), stderr empty. The
engine does not need `PYTHON*` env or sibling imports from `sys.path`: it reaches `file-provenance.py` by
`spec_from_file_location` and that file inserts its own directory.

## Tests added

In `attribution.rs::tests` (scaffold-red 8 of 24 before the fix, controls green; then 27 green):

- `a_planted_module_beside_the_entry_is_not_imported`: a `json.py` beside the entry runs under plain
  `python3` (the CONTROL, asserted) and does not run under the shipped command line, which also must
  finish cleanly.
- `dir_trust::*` (unix): fresh directory is exactly 0700 and ours; a second call rewrites nothing (inode
  unchanged); a trustworthy pre-existing directory (0700, 0755, 0750) is reused; a symlinked directory, a
  file squatting on the name, and each of group/other write bits (0777, 0770, 0707, 0720, 0702) are
  refused with nothing written into them; the owner predicate through a uid seam; the verifier names
  what it refused; the fallback is fresh, 0700, re-verified and removed on refusal; the name is per user;
  a missing base is created.
- Not tested: a directory really owned by another uid (needs a second account or root).

Mutation probe (`scripts/mutation-probe.sh`, 15 mutations of the production bytes): 14 KILLED, 1 SURVIVED
(removing the fallback's re-verification), which a uid-seam test then killed on re-run.

Fix commits on branch `worktree-agent-aa185d1f8e0bb222a` (not on experiments yet):
66f2e8e8 patch-id 1ae04f390e7b225d278d792ee1ce347e907df6d1; 0bd53dc9 patch-id
e28950dc04a0cdb8541fce16657bdb648da1932a; 42777d24 patch-id 5593a96d076eb8ead4527c44e25ef5007ea38a6b.

## Workarounds

Use a per-user `TMPDIR` (for example `$XDG_RUNTIME_DIR`).

## Resume

N/A once the branch is merged. If `file-provenance.py` ever stops inserting its own directory on `sys.path`, `-I` becomes a full defence rather than hardening.

## References

- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
