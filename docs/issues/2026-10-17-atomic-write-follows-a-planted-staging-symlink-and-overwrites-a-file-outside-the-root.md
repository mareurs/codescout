---
status: open
opened: 2026-10-17
closed:
severity: high
owner: marius
related: [docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md, docs/issues/archive/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `atomic_write` writes its staging file through a planted `<name>.tmp` symlink, so a write to `x.txt` overwrites a file outside the project root

## Summary

`atomic_write` (`src/util/fs.rs`) stages content at `<name>.tmp` next to the target with
`std::fs::write`, which follows a symlink, then renames the staging path onto the target. If
`<root>/x.txt.tmp` is a symlink to `/outside/victim`, a write to `<root>/x.txt` overwrites the victim and
leaves a symlink where `x.txt` should be. The write allowlist classifies `<root>/x.txt` as inside the root
correctly; the escape is below it.

## Class fit

No existing class names a write that follows a link in a path the caller never named: the staging path
is derived inside the writer. It is not the allowlist class (`write-allowlist-canonicalizes-only-the-parent...`),
since the classifier answers correctly for the path it is given. Filed unclassified for lack of a second
instance of "an internally derived sibling path is followed through a link".

## Symptom (Effect)

A file outside the project root is overwritten with the content of an in-root write, the in-root target
becomes a symlink to it, and the tool reports success. `git` can carry a symlink, so a repository can ship
`x.txt.tmp -> /path/to/target` and wait for any codescout write to `x.txt`.

## Reproduction

Measured 2026-10-17 on branch `worktree-agent-a5bf8d6383eb7a8b1` by calling the real
`crate::util::fs::atomic_write` from a throwaway unit test (since removed), all under a `tempdir()` in the
scratch tree: `root/` and `outside/victim.txt` containing `ORIGINAL`, `symlink(outside/victim.txt,
root/x.txt.tmp)`, then `atomic_write(root/x.txt, "WRITTEN-VIA-ATOMIC-WRITE")`. Output:
`victim="WRITTEN-VIA-ATOMIC-WRITE" x.txt_is_symlink=true`. The reviewer reproduced the same with a verbatim
replica (`scratchpad/review-a5bf/staging.rs`) before this.

## Environment

Linux, `299395e5` plus the branch's path-security changes; `src/util/fs.rs` is unchanged by them.

## Root cause

`atomic_write` -> `std::fs::write(&tmp, content)` with `tmp = staging_path(path)`. `fs::write` is
`File::create`, which follows a symlink at `tmp`. The following `fs::rename(&tmp, path)` then moves the
link itself onto `path`. Nothing checks that `tmp` is not a link, and `O_EXCL`/`O_NOFOLLOW` are not used.

## Evidence

`atomic_write` callers (from `references`): `src/tools/edit_file/mod.rs` (3 sites), `src/tools/symbol/edit_code.rs`,
`src/tools/markdown/edit_markdown.rs`, `src/symbol/edit.rs`, `src/library/registry.rs`, `src/memory/anchors.rs`,
`src/memory/mod.rs`, `src/operator_rules/mod.rs`, and `write_utf8` in `src/util/fs.rs`. `create_file` and the
edit tools reach it through those; this entry ran the function directly, not each tool.

## Hypotheses tried

None needed for the function-level result. Whether each tool reaches the sink with the planted name intact
was read from the callers, not run.

## Fix

Not applied (the reviewer scoped it out of the path-security branch). Candidate: create the staging file
with `OpenOptions::new().write(true).create_new(true)` (and `O_NOFOLLOW` where available) under a unique
name, so an existing link at the staging path is an error rather than a destination; the unique suffix also
covers the concurrent-writer race the archived stem-collision issue left open.

## Tests added

N/A.

## Workarounds

Do not open a repository that ships `*.tmp` symlinks with write tools enabled.

## Resume

Add a red test that plants `<target>.tmp -> victim` and asserts the victim is untouched, then fix
`atomic_write`. Run the same plant against `create_file`, `edit_file`, `edit_code` and `edit_markdown` to
confirm each routes through it.

## References

- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
- `docs/issues/archive/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md`
