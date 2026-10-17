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
shape 1, a leaf that is itself a symlink, is classified in-root. The write tools that reach `atomic_write`
(`edit_file`, `create_file`, `edit_code`) do not write through it. The librarian's writes DO, and `edit_file`
on a link to a deny-listed path discloses that file; both measured 2026-10-17 and filed as their own issues
(see Resume). The issue stays open, at medium.**

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
  victim kept its content, so that tool's atomic write does not follow a leaf link.
- Shape 1, measured 2026-10-17 on branch `worktree-agent-a26646b8b970ff11a` (see Fix): `create_file` with
  `overwrite` and `edit_code` (replace) against an in-project leaf link to an outside file ALSO do not
  escape. Both reach `atomic_write`, whose rename replaces the link with a regular file; the victim keeps its
  bytes. A DANGLING leaf link did not make `create_file` create the file it names. So shape 1 is not an
  out-of-root WRITE through any tool that routes through `atomic_write`. It is not safe overall: the librarian
  writes with plain `std::fs::write` and the deny list is applied to the spelling, not the resolved path. See
  the three issues under Resume, each measured the same day.

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
(escape) and, for shape 1, through `edit_file`, `create_file`, `edit_code` (no write-through), `doc(update)`
(escape), `doc(create)` on a dangling link (escape) and `edit_file` onto a deny-listed target (disclosure).

## Fix

Shape 2 FIXED in `2c71a473` (a commit on the branch carrying `4915b443`): `canonicalize_write_target` now
resolves the target's parent through its deepest existing ancestor
(`canonicalize_through_existing_ancestor`) and appends the rest verbatim, `..` included. It was
needed by the temp-dir fix too: once the temp root was canonical, a write into a not-yet-created
subdirectory of a symlinked temp dir or project root was refused, which went green -> red against
`299395e5` for `classify_in_project_is_allowed` and `create_text_file_creates_parent_dirs` with a
symlinked TMPDIR.

Shape 1 (a leaf that is itself a symlink): classifier NOT changed. For the tools that reach `atomic_write`,
measured instead of fixed: they do not write through the leaf (see Symptom), because `atomic_write` renames over
the link. Decision, written into
`atomic_write`'s doc and pinned by tests (`b493599d`, patch-id `4c9425528fab3aae53dac1ce079719843750e93c`, which
also fixes `2026-10-17-atomic-write-follows-a-planted-staging-symlink...`): a target that is a symlink is
replaced, never written through. For those tools the classifier answering `Allowed(<root>/leaf)` is safe only
because of that property of the writer, not because the classifier resolved the link: a writer that opens the
target directly (`OpenOptions::open(path)`, `fs::write(path)`) reopens it, and the librarian is exactly that
(next paragraph). The candidate `symlink_metadata` check in the classifier remains open and is now needed for the
deny-list case below.

Not every writer routes through `atomic_write`. Measured 2026-10-17 (throwaway tests, removed): `doc(update)` on a
catalogued in-root symlink rewrote the outside file; `doc(create)` on a dangling link created the file outside
the root; `edit_file` on a link to a deny-listed path read it (error text contained the file's lines) and could
copy it into the project. Each is filed separately and NOT fixed here.
`MemoryStore` writes use `tempfile::NamedTempFile::persist` (`src/memory/write_transaction.rs`), not
`atomic_write`; not run against a leaf link.

## Tests added

Shape 2, all in `src/util/path_security.rs` (unix):
`symlink_out_of_the_temp_dir_is_refused_even_below_a_missing_subdirectory`,
`symlinked_temp_dir_allows_a_write_below_a_missing_subdirectory`,
`project_root_behind_a_symlink_allows_a_write_below_a_missing_subdirectory`.
Shape 1, characterization (green before and after the atomic_write fix; they pin the property the safety rests on):
`a_target_that_is_a_symlink_is_replaced_not_written_through` (`src/util/fs.rs`, also a dangling leaf),
`create_file_overwrite_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim` (`src/tools/edit_file/tests.rs`,
also a dangling leaf), `edit_code_replace_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`
(`src/tools/symbol/tests.rs`). A mutation replacing `rename` with `copy` in `atomic_write` (which writes through
the link) killed all three (mutation-probe, 7 tests red in total).

## Workarounds

Shape 1: do not keep symlinks to files outside the project root, or to credential files, inside a project root;
and do not keep `.md` links that point outside the root in a repository that uses the librarian.

## Resume

Shape 1: measured 2026-10-17. `edit_file`, `create_file` (overwrite) and `edit_code` do not write through a leaf
link (they reach `atomic_write`); severity stays medium because three other behaviours did escape and are filed:

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md` (high)
- `docs/issues/2026-10-17-doc-create-on-a-dangling-symlink-creates-a-file-outside-the-root.md` (medium)
- `docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md` (high)

Still open here: whether the classifier should resolve the leaf (the deny-list case wants it), and the other
tools that write (`edit_markdown`, memory writes) were not run against a leaf link; `edit_markdown` reaches
`atomic_write` (read from the caller, not run), memory writes do not (see Fix). The follow-ups below stay open.

Other follow-ups from the review of this branch, none fixed here:

- `validate_approve_path` still compares the approved directory with `$HOME` by equality
  (`best_effort_canonicalize(&h) == resolved`). It rejects `/` and `$HOME` themselves, but accepts a
  directory ABOVE the home such as `/home`, and `approve_write` of `/home` then admits `$HOME/.bashrc`
  (the deny list still holds; read from the code, not run). The cwd root refuses an ancestor of the canonical home;
  `validate_approve_path` does not, so that gap remains. For the TEMP root the gap is closed from the other
  side: the temp root is kept even above the home but admits nothing inside it (`temp_root_admits`), so
  `write_ack`'s suggestion of the temp dir's parent no longer undoes anything for the temp case. It does
  for the cwd/approve case.
- A path with a DANGLING symlink or an unreadable (EACCES) component is not followed by
  `canonicalize_through_existing_ancestor`: `fs::canonicalize` fails for it, the walk stops above it and
  keeps it as written. Measured for a dangling directory link with `create_dir_all`: it returned
  `AlreadyExists` and created nothing outside, so no escape came of it; the EACCES case was not run.
- A deny entry that is itself a symlink (`~/.kube/config -> config-prod`, a symlinked `~/.npmrc`) was
  not denied under its own name once its directory was approved, because the entry resolved through its
  last component while the target keeps its leaf as written. Pre-existing; fixed in `97401a1b` by holding
  each entry in two forms (fully resolved, and parent resolved with the leaf as written).
- Separate, pre-existing and independent of the allowlist: `atomic_write` follows a planted
  `<target>.tmp` symlink. See
  `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`.

## References

- `docs/issues/2026-10-16-write-allowlist-holds-the-temp-dir-uncanonicalized.md`
