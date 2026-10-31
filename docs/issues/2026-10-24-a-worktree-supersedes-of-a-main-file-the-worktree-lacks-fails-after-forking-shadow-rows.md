---
id: '3822f951e9d78ddf'
kind: bug
status: fixed
title: 'BUG: a supersedes link from a worktree session to a main file the worktree does not hold fails after forking shadow rows, and the error hint blames a deleted file'
owners:
- marius
tags:
- cluster/unclassified
- librarian
- worktree-overlay
opened: 2026-10-24
closed: 2026-10-30
related:
- docs/issues/archive/2026-10-17-a-supersedes-link-from-a-worktree-session-writes-the-main-checkouts-destination-file.md
severity: low
---

# BUG: a supersedes link from a worktree session to a main file the worktree does not hold fails after forking shadow rows, and the error hint blames a deleted file

## Summary

Since `link::call` resolves a `supersedes` destination through the overlay (fix of the 2026-10-17 issue), the destination is forked into a shadow row at `<worktree>/<rel>` BEFORE its frontmatter is written. When the main checkout's destination file has no copy at that path (the file is untracked in main, or was added to main after the worktree's branch point), the write fails with a hint that says the file "may have been deleted or moved", and the fork has already been committed.

## Class fit

The write gate forks catalog state before the operation it guards can succeed, so a refused call leaves the fork behind. It differs from the class's other members in that the refusal comes from the file write, not from a validation, and the hint it prints names a cause (deleted or moved) that is not what happened.

## Symptom (Effect)

```text
artifact file not found on disk: <repo>/.worktrees/feat/docs/specs/other.md (hint: the file may have been deleted or moved outside of librarian)
```

The main file is untouched and still exists. The catalog is left with two shadow rows (the link's source and destination, both at the worktree path) and two `worktree_of` lineage links, plus a worktree registration.

## Reproduction

Measured 2026-10-24 at the fix-g worktree (base 30f3e728d plus the uncommitted link fix), with a throwaway test since removed, in the shape of `link::tests::supersedes_from_a_worktree_session_writes_the_shadow_and_not_the_main_file`, minus the shadow file: a tempdir repo with `docs/specs/other.md` present ONLY under the main root, a `CurrentProject` whose `git_root` is `<repo>/.worktrees/feat` and `main_root` is `<repo>`, both rows registered at their main paths, then `link::call(ctx, {src_id, dst_id, rel: "supersedes"})`.

Observed: the error above, `SELECT count(*) FROM artifact WHERE abs_path LIKE '%/.worktrees/feat/%'` = 2, `SELECT count(*) FROM artifact_link` = 2.

## Environment

codescout 0.15.0, in-process test context, Linux, 2026-10-24.

## Root cause

Measured as above. `resolve_write_target` (`src/librarian/tools/worktree.rs`) forks in its own committed transaction: shadow row at `shadow_coordinates` (worktree root plus the row's path relative to the main root), `worktree_fork` event, `worktree_of` link, registration. It never checks that a file exists at the shadow path; the comment on the seeded row assumes "the FILE at the shadow path is git's checkout copy". `link::call` then calls `update::write_field_to_frontmatter`, whose `ReadError::Io(NotFound)` arm produces the "deleted or moved" hint. The same assumption holds for every other overlay write that edits a file (`update`), so this is not specific to `link`; it surfaces for `supersedes` because that write targets a second artifact the caller chose.

## Evidence

The error string and the two counts under Reproduction.

## Hypotheses tried

1. **Hypothesis:** the failure leaves the catalog unchanged. **Test:** counted rows after the failed call. **Verdict:** rejected: 2 shadow rows and 2 links remain.

## Fix

FIXED for the reported shape (a `supersedes` link) in `b8b57586d` (branch `worktree-agent-a88be249b09c25d52`, not merged). Owner choice (a): `link::call` now calls `worktree::require_shadow_file` BEFORE `resolve_write_target` of either id, so a `supersedes` destination with no file at the shadow path is refused with "<main path> exists in the main checkout only: this worktree has no file at <shadow path>" and a hint to commit or copy it onto the worktree's branch. Nothing is forked, so the catalog is untouched. Options (b) and (c) were not taken.

Not covered: the other overlay writers that edit a file (`update`, `append_entry`, `augment`, `update_entry`) were NOT checked for the same leftover rows; they call `resolve_write_target` directly and the new check is not wired into them.

## Tests added

`supersedes_of_a_main_file_the_worktree_lacks_refuses_before_forking` (`src/librarian/tools/link.rs`). It failed first with the old "deleted or moved" error, then passed. It asserts the message, that the hint does not blame a deletion, that the `artifact` and `artifact_link` row counts are unchanged, and that the main file is untouched. The existing `supersedes_from_a_worktree_session_writes_the_shadow_and_not_the_main_file` stays green; `cargo test --lib librarian::tools::link` passed 190.

## Workarounds

Add the destination to the worktree's branch (commit or copy it there) before linking, or supersede from the main checkout.

## Resume

Start at `src/librarian/tools/link.rs` `call` (the `resolve_write_target` of `dst_id`) and `src/librarian/tools/worktree.rs` `resolve_write_target`. Check whether `update`, `append_entry` and `augment` on an untracked main file leave the same rows behind.

## References

- `src/librarian/tools/link.rs`, `src/librarian/tools/worktree.rs`, `src/librarian/tools/update.rs` (`write_field_to_frontmatter`)
- `docs/issues/archive/2026-10-17-a-supersedes-link-from-a-worktree-session-writes-the-main-checkouts-destination-file.md`
