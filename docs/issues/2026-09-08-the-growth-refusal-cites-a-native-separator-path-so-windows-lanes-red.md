---
id: '74eb09ca48756b9d'
kind: bug
status: open
title: 'BUG: the growth refusal cites a native-separator path, so both Windows CI lanes red on a message a Windows reader cannot paste'
tags:
- cluster/repro-env-diverges-from-gate-env
- clusters
- ci
- windows
- pre-commit
opened: 2026-09-08
owner: marius
severity: med
---

## Summary

`9852c474` — a fix that made the cluster growth refusal name the *file* holding the
`**Members:**` field — renders that path with the **native separator**, so on Windows the
refusal cites `docs\trackers\issue-clusters\IC-7-….md`. Both Windows CI lanes are red on
`579a2085`, and every POSIX lane is green.

`class_files()` has three sources and they disagree. `head` and `index` go through git,
which emits forward slashes on every platform. `worktree` globs the directory and rendered
each `Path` with `str(p)` — native. Nothing reconciled them, so a single refusal message
printed both forms at once:

```
cluster/lazy-warmup-bills-the-first-caller -- the field is in `docs\trackers\issue-clusters\IC-7-...md`
cluster/unclassified                       -- the field is in `docs/trackers/issue-clusters.md`
```

The second row survives because `cluster/unclassified` falls back to `LEDGER`, a literal
string constant. The asymmetry is the tell.

## Symptom (Effect)

`tests/issue_clusters.rs:821` asserts the POSIX form literally, so it is **green on Linux
and macOS and red on Windows by construction**. CI run `34221968837` on `579a2085`: 13
success, 2 failure, 3 cancelled — both failures this one test.

Two costs, and the second is why this is not cosmetic:

- **A backslash path is not usable as a citation.** The refusal is documentation-shaped —
  its entire point is handing the reader a path to grep, paste into `git`, or feed to
  another tool. `audit_doc_refs` keys on backticked path-shaped tokens and cannot resolve
  one either. A Windows reader gets a message whose one actionable element does not work.
- **`class_file_for` splits on `"/"` to take a basename.** A backslash path does not split
  at all, leaving `stem` as the entire path. That routing survived only on the
  `stem.endswith(f"-{slug}")` fallback — by luck, not design. This is a second latent
  defect the reporter did not name, and it is closed by the same fix.

## Reproduction

Platform-independent, from Linux:

```
$ python3 -c "
import pathlib
p = pathlib.PureWindowsPath('docs/trackers/issue-clusters') / 'IC-7-lazy-warmup-bills-the-first-caller.md'
print(str(p)); print(p.as_posix()); print(str(p).rsplit('/',1)[-1])"
docs\trackers\issue-clusters\IC-7-lazy-warmup-bills-the-first-caller.md
docs/trackers/issue-clusters/IC-7-lazy-warmup-bills-the-first-caller.md
docs\trackers\issue-clusters\IC-7-lazy-warmup-bills-the-first-caller.md   <- rsplit did not split
```

## Root cause

Not the interpolation site. `path` at `pre-commit-ledger-counts.py:464` is a `str`, so the
reporter's suggested `{path.as_posix()}` would raise `AttributeError` — the diagnosis was
directionally right and the prescribed edit would have failed. The native separator enters
one layer earlier, in `class_files()`'s worktree branch, which is also the only place that
can fix it for **every** consumer rather than for this one message.

## Fix

`_repo_path(p) -> str` returning `p.as_posix()`, used by the worktree branch. Callers pass
the `Path` object, never `str(p)`: the object knows its own flavour, and `as_posix()` is
what converts.

**The test problem is the interesting half, and it is why this file exists.** The defect is
**invisible on POSIX by construction** — `str(p)` and `p.as_posix()` are byte-identical
here — so no assertion over real `class_files` output can discriminate on the gate the
author runs. Such a test is monotone under the mutation and passes with the bug restored.
Measured: under the mutation, the pre-existing
`the_growth_refusal_names_the_file_holding_the_members_field` **passed** (`1 passed; 1
failed`) while the new test caught it.

`--fixture-repo-path` closes the gap by pushing a `PureWindowsPath` through the same
renderer production calls — **the foreign flavour without the foreign OS**. New test:
`a_repo_path_renders_posix_even_when_the_platform_flavour_is_windows`, paired with a
positive assertion because "contains no backslash" is satisfied by empty output, a crashed
fixture, and a renderer that drops the path entirely.

Mutation verified: `as_posix()` → `str(p)` reds on Linux with
`docs\trackers\issue-clusters\IC-7-….md` in the failure text; restored byte-identical.

## Severity

Med. No data loss and the routing still resolved by luck, but two Windows CI lanes are red
on published `experiments`, and the user-facing artefact — a path a reader is told to act
on — is unusable on that platform.

## Resume

Fixed and gated. The `class_file_for` basename split is closed by the same change; if that
function ever reads a path from a source outside `class_files`, it needs its own
normalisation.

## References

- CI run `34221968837` on `579a2085`, both Windows lanes.
- Reported by sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`, who read the failure text
  at the bytes and explicitly declined to edit another session's in-flight work.
- `9852c474` — the commit that introduced it, itself the fix for
  `docs/issues/archive/2026-09-08-the-cluster-refusal-names-a-field-not-a-file-and-the-index-confirms-it.md`.
