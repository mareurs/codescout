---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
unverified: 'STANDING — the cross-user case end to end (a directory really owned by another uid) is not run here: no second account, so the owner comparison is driven through a uid seam. python3 -I is hardening only: eleven stdlib names planted one at a time still run under it on Python 3.14.7 (file-provenance.py:91 re-inserts its own directory). The temp base itself is not checked: a group/world-writable TMPDIR without the sticky bit lets anyone swap the verified directory (a root-owned 1777 /tmp is fine). Windows: compiled and linted by the cross-clippy with no finding in attribution.rs, never run.'
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
`<name>.<pid>.tmp` link is no longer followed. It does not address this. (`b493599d` lives on branch
`worktree-agent-a26646b8b970ff11a` and is NOT an ancestor of the branch carrying this fix; the sentence is true
once the two are merged.)

## Hypotheses tried

None needed.

## Fix

Changes in `src/tools/run_command/attribution.rs`; the directory checks are what close the hole.

1. **The directory is trustworthy** (`script_dir`, `verify_private_dir`, `create_private_dir`,
   `fresh_private_dir`, `fallback_dir`). The name is `codescout-attribution-<euid>-<hash>`; it is created mode
   0700 (`DirBuilder`, not `create_dir_all`) and VERIFIED BEFORE EVERY RUN, not once per process:
   `symlink_metadata` says a real directory (a link, even to a good directory, is refused), owned by the
   effective uid, no group/other write bit (read and execute bits are allowed: the scripts are public).
   The first version of this fix cached the verified path in a `OnceLock`; a reviewer showed that a directory
   removed while the process lives (`tmpreaper`, `systemd-tmpfiles`, `rm -rf /tmp/codescout-*`) and replaced by a
   symlink to a hostile one ran unverified and returned `Some("PWNED")`. The cache is gone; the swap is a
   test (`a_directory_swapped_in_after_first_use_is_not_run`). Cost of verifying per run, measured 2026-10-17
   on a debug build, warm: **437 microseconds per call** (stats plus six file reads), against a ~7 s scan.
   A refusal logs a `warn` naming the path and reason and falls back to a `mkdtemp` directory (unpredictable name,
   0700 requested explicitly, verified, kept for the life of the process and reused while it still verifies, one
   per base). If even that cannot be made (a filesystem that ignores modes, an NFS id mapping, sshfs) a second
   `warn` says so and the hint goes silent: recorded as ceiling 4 in the module doc.
   `tempfile` creates directories with the umask's view of 0777, so the explicit mode matters: under umask 002 the
   default would be group-writable and fail the check, leaving no fallback.
2. **A relative base is refused** (`script_dir`): `std::env::temp_dir()` returns `$TMPDIR` verbatim (`""`
   included), and the spawn resolves a relative script under `current_dir(work_dir)`, the workspace being
   analysed, where a repository could ship a directory of that name. The helper then runs nothing.
3. **The engine is spawned as `python3 -I <script>`** (`engine_std_command`), hardening only.

Single-user behaviour is unchanged: same cache reuse (inode-unchanged test), same cost to within the figure above.

**`-I` alone does NOT close the issue, which the issue's own candidate fix assumed, and the hole it leaves is
wide.** `file-provenance.py:91` does `sys.path.insert(0, <its own dir>)` before importing its siblings. Measured
2026-10-17 on Python 3.14.7, ONE stdlib name planted at a time beside a scratch copy of all six scripts, run as
`python3 -I attribute-red.py --explain`: eleven of them run under `-I`: `typing`, `dataclasses`, `decimal`,
`annotationlib`, `inspect`, `numbers`, `copy`, `ast`, `tokenize`, `linecache`, `weakref`. The set depends on the
Python version. An earlier draft of this record claimed `dataclasses` did not run under `-I`: that came from a
run that planted several modules at once, where the first `ImportError` hid the rest; it was wrong. What `-I`
does close is the engine entry's own imports (`json`, `re`, `os`, `subprocess`, `pathlib`, `sys`,
`importlib.util`), `PYTHON*` variables and the user site. Left as it is: the insert is how `file-provenance.py`
runs standalone.

**Not checked, and why that is acceptable.** (a) The BASE (`$TMPDIR`) is not verified: a group/world-writable base
WITHOUT the sticky bit lets anyone rename a verified directory away and put their own at the name between the check
and the spawn; a root-owned sticky `/tmp` (1777) does not. (b) File modes inside a verified directory are not
checked: the directory is 0700 and ours so nobody else can add or replace a file; a pre-existing directory of ours at
0750/0755 is accepted, which leaves existing file modes to whoever made them (under umask 002 the scripts are
0664). (c) There is a window between verify and spawn; closing it needs an fd-based exec, and the attacker would first
have to replace a directory in a sticky `/tmp` that we own.

End to end under `-I` (scratch fixture, identical stdin, plain vs `-I`): both exit 0, stdout byte-identical (470
bytes, the full three-stage answer naming `src/held.rs` and the coverage gap), stderr empty.

Windows: `scripts/build-windows.sh clippy --all-targets -- -D warnings` (x86_64-pc-windows-gnu is installed locally)
found an `unused_mut` in `create_private_dir` (`mut` is only needed under `cfg(unix)`), fixed with
`#[cfg_attr(not(unix), allow(unused_mut))]`. What still fails that lane is not this change: the same `unused_mut`
at `src/memory/write_transaction.rs:161` and 49 dead-code errors in `src/tools/run_command/tests.rs`; no finding
names `attribution.rs`. The non-unix arm is a subset of the unix one for types, not for lints. Nothing was run on
Windows.

## Tests added

In `attribution.rs::tests`:

- `a_planted_module_beside_the_entry_is_not_imported` (constructor level, with a CONTROL showing the plant fires
  under a plain `python3`) and `a_module_planted_beside_the_entry_does_not_run_through_the_shipped_path` (through
  `wip_author_diagnostic_in`, the function that spawns, so a call site building its own `Command` fails it).
- `the_engine_ignores_pythonpath` (with a control): kills `-I` replaced by `-P`, which a planted-module test alone
  cannot (`-P` also drops the script directory but honours `PYTHONPATH`).
- `a_relative_base_is_refused`.
- `dir_trust::*` (unix): fresh directory exactly 0700 and ours; second call rewrites nothing; a trustworthy
  pre-existing directory (0700, 0755, 0750) is reused; a symlinked directory, a file squatting on the name and each
  of group/other write bits (0777, 0770, 0707, 0720, 0702) are refused with nothing written into them; the owner
  predicate through a uid seam; the verifier names what it refused; the fallback is 0700, re-verified and removed on
  refusal, reused while valid, replaced when not, never reused across bases; the name is per user; a missing base is
  created; and `a_directory_swapped_in_after_first_use_is_not_run` (a symlink and a world-writable directory swapped
  in after a first successful run must be neither executed nor silent).
- Not tested: a directory really owned by another uid (needs a second account or root).

Red then green: the swap test and the relative-base test were red before the fix (the swap returned the hostile
script's marker; the relative base returned `attribution-relative-base-probe/codescout-attribution-1000-<hash>/attribute-red.py`);
the shipped-path and PYTHONPATH tests were green before the fix and are guards for mutants (below).

Mutation probe (`scripts/mutation-probe.sh`, 22 mutations of the production bytes, run against `48f80865`): 22
KILLED, 0 SURVIVED. Including: spawn bypassing `engine_std_command`, `-I` replaced by `-P`, `-I` removed, relative base
allowed, a per-process path cache restored, a cached fallback trusted unverified, a fallback that ignores its base,
the fallback never tried, each write bit, owner, `is_dir`, `symlink_metadata`, the two 0700s, the refusal ignored, the
fallback re-verification, the name without the uid, an existing directory treated as an error, no base creation, and
the wrong uid verified.

Fix commits on branch `worktree-agent-aa185d1f8e0bb222a` (not on experiments yet): `66f2e8e8` (patch-id
1ae04f390e7b225d278d792ee1ce347e907df6d1) first version; `64d25b1b` (patch-id 1be954f8cad4adeec53a407b03fbb9fe0a2c9305),
`c18a0dbe` (patch-id 96d739812fb89a2ea730576f15a9a1e66826b4a4), `48f80865` (patch-id
881667d13406bb2b9f5d6068cc29a247c9a0a508): per-run verification, relative base, shipped-path tests, fallback keyed by base.

## Workarounds

Use a per-user `TMPDIR` (for example `$XDG_RUNTIME_DIR`).

## Resume

N/A once the branch is merged. If `file-provenance.py` ever stops inserting its own directory on `sys.path`, `-I` becomes a full defence rather than hardening.

## References

- `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`
