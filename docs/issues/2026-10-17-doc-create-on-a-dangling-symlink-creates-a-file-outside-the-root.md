---
status: fixed
opened: 2026-10-17
closed: 2026-10-17
severity: medium
owner: marius
related: [docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md, docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'The check and the write are two syscalls, so a link planted between them still wins. The parent-directory form was measured red through a mutation of the fix (create returned Ok with an in-root abs_path), not through a run that listed the file on the outside disk.'
---

# BUG: `doc(create)` checks `full.exists()` (false for a dangling link), and its `rel_path` check is spelling-only, so a link on the path makes it create a file outside the root

## Summary

Two escapes through one path build. `src/librarian/tools/create.rs` refused an existing path with `full.exists()`
(line 386), which follows links and is `false` for a DANGLING symlink; it then `create_dir_all`ed the parent and wrote
with `std::fs::write(&full, &content)`, which follows the link and creates its target. And `validate_rel_path`
(create.rs:9-36) checks only the SPELLING of `rel_path` (no `..`, not absolute); the path is joined onto the base
directory, `create_dir_all`ed and written with no canonicalize or `starts_with` anywhere in the file or above it, so
`rel_path="esc/new.md"` with `esc` an in-root symlink to a directory outside the root creates `/outside/new.md` with no
dangling link involved.

## Class fit

Same shape as the `doc(update)` leaf-link write (a librarian write that follows a link in a path built from
`base_dir.join(rel_path)`), and not the write-allowlist class: `doc(create)` consults no write classifier. Filed
unclassified for lack of a named class for a check-then-write that treats a dangling link as absent.

## Symptom (Effect)

`doc(create, rel_path="dangling.md")` returned an id and path, no error; a new file appeared at the dangling link's
destination, outside the project root. `doc(create, rel_path="esc/new.md")` through a directory link out of the root
returned an id and an in-root `abs_path`. For a link to an existing FILE `exists()` is `true` and the call refused with
`path exists` (still does).

## Reproduction

Dangling leaf. Measured 2026-10-17 on `worktree-agent-a26646b8b970ff11a` with a throwaway test: `root/dangling.md`
symlinked to `outside/never-created.md` (absent), then `create::call(ctx, {"repo":"r","rel_path":"dangling.md",...})`:

```
PROBE doc create dangling: ok=true None
PROBE outside file created=true content_has_marker=true
```

Re-measured on `worktree-agent-a66600865e063ffd2` by the permanent test against the unfixed code (red):
`create_refuses_a_dangling_leaf_link_instead_of_creating_its_target_outside_the_root` failed at `expect_err`.

Parent directory link. `create_refuses_a_path_through_a_symlinked_directory_that_leaves_the_root` and
`create_refuses_a_missing_subdirectory_below_a_symlinked_ancestor_before_building_it` ran with the containment check
mutated out and both failed at `expect_err`, the call having returned
`Ok {"id": "da49aaea0ce49c72", "abs_path": ".../esc/new.md"}` and `Ok {"id": "ddb80fadcaefeced", "abs_path": ".../esc/sub/deeper/new.md"}`.

## Environment

Linux, `cargo test --lib`.

## Root cause

`create.rs:386` `if full.exists() { return Err(path exists) }` then `std::fs::write(&full, &content)`. `Path::exists`
follows the link and reports `false` when the target is missing; `fs::write` is `File::create`, which follows the link and
creates the target. Separately nothing resolved the PARENT of `full` against the base directory, and `create_dir_all`
runs before the write, so a link to an outside directory had its subdirectories built there.

## Evidence

The `create_file` tool does NOT have the leaf defect: it routes through `atomic_write`, which renames over the link
(`create_file_overwrite_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`, `src/tools/edit_file/tests.rs` on
`worktree-agent-a26646b8b970ff11a`). `atomic_write`'s own doc says a symlinked PARENT redirects both its create and its
rename, so replacing `fs::write` with `atomic_write` would not have closed the parent form; that is why the fix is a
containment check and not a swap.

## Hypotheses tried

1. **Hypothesis:** a leaf `symlink_metadata` check plus `atomic_write` closes it. **Test:** read `atomic_write`'s doc and
   the parent form above. **Verdict:** rejected, the parent link still redirects.
2. **Hypothesis:** `validate_write_path` can be reused. **Test:** read `classify_write_path_in`. **Verdict:** rejected for
   THIS call site: it also admits the system temp dir and the server's working directory as write roots, so a link into
   either would pass, and the librarian holds no `PathSecurityConfig`. The question here is narrower, stay inside the repo
   root.

## Fix

Branch `worktree-agent-a66600865e063ffd2`: `0fb7fb68` patch-id `5eee321f8278452f17cc1ea744ec9daeb514de6b`; tests pinned by
`4efabf2d` patch-id `80aa4cef9351a74a6fc4666bfa0f93ada573ea8e`. Round 2 (normalise first, shared helper, neutral message):
`033bf24b` patch-id `b5b500849f73cad3f85234d916eadb5b1f144839`.

- Leaf: after the existing `full.exists()` refusal, `util::refuse_write_through_symlink(&full)` (a dangling link now
  refuses with the link and its target named). The existing `path exists` refusal for a link to an existing file is
  unchanged.
- Parent chain: `util::refuse_path_outside_root(&full, &base_dir, "create")` (moved from a private `create.rs` function in round 2
  so move, delete, write and the sidecars share it), run BEFORE `create_dir_all` so nothing is
  built outside before the refusal. It canonicalizes the deepest EXISTING ancestor of `full` (the same walk as
  `path_security`'s private `canonicalize_through_existing_ancestor`, which is not public and which I was told not to edit; the
  duplication is recorded in the doc(update) issue as a follow-up), appends the
  not-yet-existing components verbatim, resolves the root the same way, and requires `starts_with`. An unresolved `..` is
  refused rather than trusted (`starts_with` is component-wise and would match a root the `..` escapes).
- Round 2: `rel_path` is normalised BEFORE `validate_rel_path` runs. It was validated raw and normalised after, so on Unix
  `a\..\..\escaped.md` (a backslash is an ordinary name character there) passed the validator and became `a/../../escaped.md`.
  Measured by replaying the base steps in a throwaway test: `validator_on_raw_ok=true`, `normalized=a/../../escaped.md`,
  `full_exists=false`, `landed_outside_root=true` (the file appeared one level above the root). On the first fix the containment
  check already refused it, but with a message that blamed a symbolic link in every case; the message is now neutral
  ("a symbolic link, or a `..` that could not be resolved") and the validator catches this one first with its own.
- A link whose target is INSIDE the root is allowed for a parent directory (`alias/c.md` where `alias -> docs/real`),
  because the file lands in the repo; only the leaf rule is "any link".

## Tests added

`src/librarian/tools/create.rs` `tests`: `create_refuses_a_dangling_leaf_link_instead_of_creating_its_target_outside_the_root`,
`create_still_refuses_a_link_to_an_existing_file_and_still_creates_a_fresh_path` (control),
`create_refuses_a_path_through_a_symlinked_directory_that_leaves_the_root` (asserts nothing created outside and no catalog
row), `create_refuses_a_missing_subdirectory_below_a_symlinked_ancestor_before_building_it` (asserts the outside `sub/` was
never built), `create_still_works_under_real_directories_and_under_a_link_that_stays_inside` (control: ordinary nested path,
not-yet-existing nested path, in-root directory link), `create_still_works_when_the_repo_root_is_itself_a_symlink` (control,
killed the "skip the root's resolution" mutation, which survived the first set), `create_validates_the_rel_path_it_will_actually_use` (round 2; red first: the refusal came from the containment check with the
symlink message), `an_unresolved_dotdot_below_an_existing_ancestor_is_refused_not_trusted`
(killed the "drop the `..` term" mutation, which survived the first set).

Mutation probe, one run per guard, 72/74 tests each: leaf guard removed KILLED; parent check removed KILLED; containment
predicate never fires KILLED; unresolved-`..` term dropped KILLED (after the added test); root left unresolved KILLED (after the
added test). Round 2: normalising after validating KILLED (`CR1`); the create call to the shared containment helper dropped KILLED
(`CR2`); the shared helper's predicate, root resolution and `..` term KILLED (`UA3`-`UA5`).

## Workarounds

None needed for the project's own tree; a repository that ships dangling `.md` links or a directory link out of the root is the
exposure.

## Resume

N/A. The leaf and parent forms of `doc(create)` are closed. The `doc(update)` family and the site table are in
`docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`.

## References

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
- `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
