---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `classify_write_path` canonicalizes only the immediate parent, so a symlink leaf or a symlinked directory with a missing subdirectory below it is classified as inside the root

## Summary

`canonicalize_write_target` (`src/util/path_security.rs`) resolved symlinks in the target's
immediate parent only, then joined the file name. Two shapes therefore kept their literal,
in-root spelling while the OS would resolve them outside the root. Measured at the classifier, and
for the second shape at a write tool (it wrote outside the root). **Shape 2 is fixed (`2c71a473`);
shape 1, a leaf that is itself a symlink, is still open.**

## Class fit

No existing class names a containment check that resolves one path component and trusts the
rest. Filed unclassified because it was found while fixing the temp-dir root
(`2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized`) and is a different fault in a
different function; the root being compared against is not involved, so that bug's class does not
carry over. Revisit if a second containment check shows the same one-component shape.

## Symptom (Effect)

Both are classified `Allowed`, with the in-root spelling as the resolved path:

1. A symlink leaf inside an allowed root pointing at a file outside it: `<root>/leaf -> /elsewhere/victim.txt`;
   target `<root>/leaf`. The parent `<root>` canonicalizes, the name `leaf` is joined unresolved.
2. A symlinked directory inside an allowed root, then a path through a directory that does not exist
   yet: `<root>/esc -> /elsewhere`, target `<root>/esc/new/x.txt`. The parent `<root>/esc/new` does
   not exist, so `best_effort_canonicalize` returns it unchanged, and it starts with `<root>`.

Measured at the write tools on 2026-10-17, with throwaway links under this worktree and victims
under `$HOME/.cache`, all removed afterwards:

- Shape 2 IS an escape. `create_file(path=".esc-link/newdir/x.txt")`, with `.esc-link` a symlink to a
  directory outside the project, returned `ok` and wrote `<outside>/newdir/x.txt` containing the
  content, creating `newdir` outside the root.
- Shape 1 did NOT escape through `edit_file`: the link was replaced by a regular file and the
  victim kept its content, so that tool's atomic write does not follow a leaf link. Other write
  paths (`create_file` with `overwrite`, `edit_code`) were not run against a leaf link.

## Reproduction

Scratch test, not committed. Unix. `temp`, `outside`, `project` are three `tempdir()`s:

- `symlink(outside/victim.txt, temp/leaf)`, then
  `classify_write_path_with_temp(temp/leaf, project, default, [], temp)`
- `symlink(outside, temp/esc)`, then
  `classify_write_path_with_temp(temp/esc/new/x.txt, project, default, [], temp)`

## Environment

Linux, codescout worktree branch cut from `experiments` at 299395e5.

## Root cause

`canonicalize_write_target` canonicalized `path.parent()` best-effort and joined `file_name()`.
It did not walk up to the deepest existing ancestor when the parent was missing (shape 2, fixed), and
it does not resolve the leaf if the leaf is itself a symlink (shape 1, open). The allowed roots were a
separate matter and not "the same function": the project root, the cwd, the extra roots and the session
roots went through `best_effort_canonicalize`, which keeps a missing path literal, so a root that did not
exist yet under a symlinked ancestor never matched a target resolved through its existing ancestors
(fixed in `abdb0b03`: every root, and the deny-list entries, now use
`canonicalize_through_existing_ancestor` too).

## Evidence

Measured 2026-10-17 with the scratch test above:

```
leaf:            Allowed("<temp>/leaf")
nonexist-subdir: Allowed("<temp>/esc/new/x.txt")
```

## Hypotheses tried

None needed for the classification result. The write-tool consequence was measured for shape 2
(escape) and for shape 1 through `edit_file` only (no escape).

## Fix

Shape 2 FIXED in `2c71a473` (a commit on the branch carrying `4915b443`): `canonicalize_write_target` now
resolves the target's parent through its deepest existing ancestor
(`canonicalize_through_existing_ancestor`) and appends the rest verbatim, `..` included. It was
needed by the temp-dir fix too: once the temp root was canonical, a write into a not-yet-created
subdirectory of a symlinked temp dir or project root was refused, which went green -> red against
`299395e5` for `classify_in_project_is_allowed` and `create_text_file_creates_parent_dirs` with a
symlinked TMPDIR.

Shape 1 (a leaf that is itself a symlink) is NOT changed and stays open. Candidate: `symlink_metadata`
the leaf, and refuse or resolve it when it is a symlink. Resolving it changes what the write tools
write to for an in-root link, so it needs its own decision.

## Tests added

Shape 2, all in `src/util/path_security.rs` (unix):
`symlink_out_of_the_temp_dir_is_refused_even_below_a_missing_subdirectory`,
`symlinked_temp_dir_allows_a_write_below_a_missing_subdirectory`,
`project_root_behind_a_symlink_allows_a_write_below_a_missing_subdirectory`.
None for shape 1.

## Workarounds

Shape 1: do not leave symlinks to outside files inside a project root.

## Resume

Shape 1: measure a leaf symlink against `create_file` with `overwrite` and `edit_code` (only
`edit_file` was run, and it did not follow the link). If any follows it, this is an escape and shape
1 needs the leaf check above plus a red test. Otherwise lower the severity and decide whether the
classifier should still name the resolved leaf.

Other follow-ups from the review of this branch, none fixed here:

- `validate_approve_path` still compares the approved directory with `$HOME` by equality
  (`best_effort_canonicalize(&h) == resolved`), so `approve_write` accepts a directory ABOVE the home
  such as `/home` or `/`. The temp and cwd roots now refuse an ancestor of the canonical home;
  `validate_approve_path` does not.
- A path with a DANGLING symlink or an unreadable (EACCES) component is not followed by
  `canonicalize_through_existing_ancestor`: `fs::canonicalize` fails for it, the walk stops above it and
  keeps it as written. Measured for a dangling directory link with `create_dir_all`: it returned
  `AlreadyExists` and created nothing outside, so no escape came of it; the EACCES case was not run.
- Separate, pre-existing and independent of the allowlist: `atomic_write` follows a planted
  `<target>.tmp` symlink. See
  `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
