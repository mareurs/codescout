---
status: open
opened: 2026-10-17
closed:
severity: high
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: the write allowlist reads `$HOME` and `$TMPDIR` as given, so an unset, empty, relative or wrong `$HOME` and a relative or `..` `$TMPDIR` switch its guards off or point them at the wrong directory

## Summary

`src/util/path_security.rs` takes two environment values on trust: `$HOME` (deny list, breadth
guards) and `$TMPDIR` (an implicit write root). A `$TMPDIR` of `.` or `a/../b` became a write root
at a directory the operator never named; an unset, empty or relative `$HOME` emptied the `~`
entries of the deny list and switched the `$HOME` breadth guard off; a `$HOME` that names some
other directory left the account's real home unprotected. Each was measured against the real
library (`classify_write_path`, `validate_approve_path`, `validate_read_path`). This file tracks
the port of the fixes from branch `worktree-agent-a40d6fa7287de0e17` (head 1f5443fd) onto
`experiments` at 27efb11d, one commit each; the sections below are filled in as each lands.

## Class fit

Environment input trusted as if it were a validated path: the same trait as the temp-dir and
parent-only canonicalization bugs this file is related to, where a value compared against a
canonical target was never itself checked. It differs in that nothing here is a symlink: the
inputs are missing, empty or spelled in a way `fs::canonicalize` hides.

## Symptom (Effect)

Measured 2026-10-17 with `pathsec_probe`, a throwaway example linking the REAL library of the
tree under test (classification only, nothing written; scenario driver and outputs under the
session scratchpad `port/`). Baseline is `experiments` 27efb11d. `[ALLOWED]` means
`classify_write_path` returned `Allowed`.

| scenario (env, cwd) | target | baseline |
|---|---|---|
| F1d `TMPDIR=/home/marius/.cache/../.claude` | `~/.claude/settings.json` | ALLOWED |
| F5a `TMPDIR=.` cwd `/home` | `/home/otheruser/x` | ALLOWED |
| F5c `TMPDIR=.claude` cwd `~` | `~/.claude/settings.json` | ALLOWED |

## Reproduction

`PROBE_BIN=<probe> FX_DIR=<scratch> run.sh`, scenarios F1d, F5a and F5c. Unit tests below.

## Environment

Linux, rustc 1.97.1, codescout `experiments` 27efb11d, branch `fix/write-allowlist-port-2026-10-17`.

## Root cause

`canonical_temp_write_root` (`src/util/path_security.rs`) handed `$TMPDIR` straight to
`fs::canonicalize`, which resolves a relative path against the process cwd and collapses `..`.
Measured 2026-10-17: the three probe rows above.

## Evidence

See the probe table. Before the fix the unit tests `a_relative_temp_dir_is_not_a_write_root`,
`a_temp_dir_spelled_with_dotdot_is_not_a_write_root` and the two `..admits_no_write_through_classification`
tests failed with `Some(<cwd>)` / `Allowed(..)`.

## Hypotheses tried

None needed.

## Fix

F5 and F1-`..` (commit 1): `canonical_temp_write_root` returns `None` for a path that is not
absolute or that has a `..` component. The empty `$TMPDIR` was already refused (`canonicalize("")`
fails); a test pins it so it stays refused.

Documented residual, NOT closed: a `$TMPDIR` that is or sits under a SYMLINK is followed to its
target (F1a `TMPDIR=link -> /etc` admits `/etc`; F1b/F1c link to `~/.claude`/`~/.config`; F1f/F1g
`TMPDIR=/home` admits `/home/otheruser`). A macOS `/var -> /private/var` link is indistinguishable
from an attacker's link, and whoever controls `$TMPDIR` can already name the directory literally;
`temp_root_admits` also keeps a temp root above `$HOME` on purpose (sandboxes whose `$HOME` is under
`$TMPDIR`). A design decision, none proposed here.

## Tests added

`src/util/path_security.rs`: `a_relative_temp_dir_is_not_a_write_root`,
`a_temp_dir_spelled_with_dotdot_is_not_a_write_root`, `an_empty_temp_dir_is_not_a_write_root`
(green before: pinned), `a_dotdot_temp_dir_admits_no_write_through_classification`,
`a_relative_temp_dir_admits_no_write_through_classification`,
`an_empty_temp_dir_admits_no_write_through_classification` (green before: pinned).

## Workarounds

Do not export a relative or `..` `$TMPDIR`.

## Resume

Later commits on this branch add the `$HOME` fixes (items 2 to 5) and the `extra_write_roots`
check; this file is updated with each.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
