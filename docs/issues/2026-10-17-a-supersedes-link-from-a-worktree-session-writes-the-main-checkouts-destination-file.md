---
kind: bug
status: open
title: 'a supersedes link written from a worktree session rewrites the MAIN checkout''s destination file: link::call resolves src_id through the overlay and passes dst_id unresolved'
tags:
- cluster/guard-narrower-than-its-name
- librarian
- worktree-overlay
opened: 2026-10-17
closed:
owner: marius
severity: medium
related:
- docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md
- docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md
---

# BUG: `doc(link, rel="supersedes")` from a linked-worktree session writes the main checkout's file

## Summary

The worktree overlay exists so a session in a linked worktree never writes the shared main
checkout: a write to a main-checkout artifact forks it into a shadow row at the worktree path and
proceeds there (`worktree::resolve_write_target`). `link::call` applies that to `src_id` only
(`link.rs:39`). A `supersedes` link also rewrites the DESTINATION's frontmatter (`status:
superseded`, `link.rs:57-64`), and it hands `a.dst_id` to `write_field_to_frontmatter` unresolved, so
the file written is the row's own `abs_path`: the MAIN checkout's file, from a session that was
supposed to be isolated.

## Class fit

The guard (the overlay write gate) is narrower than the operation it is named for: it covers the
first artifact a write names and not the second one the same call writes to. Differs from the
class's other members in that the unguarded half is a file on disk, shared with every other session
in the repo, rather than a record.

## Symptom (Effect)

A worktree session supersedes a bug or plan, and the shared main checkout's copy of the destination
gets `status: superseded` in its working tree, uncommitted, under a peer's feet. The result names
the main path (by design since 2026-10-17: `name_the_target` names the row whose file `link`
wrote), and the catalog row of the destination is the main row, not a shadow.

## Reproduction

Measured 2026-10-17 by a dispatcher-level test,
`librarian::tools::artifact::tests::a_link_names_the_row_whose_file_it_wrote_for_each_rel_in_and_out_of_a_worktree`
(the `worktree = true`, `rel = "supersedes"` case): a tempdir repo with a main checkout and a
`.worktrees/feat` session, a main-root destination file `docs/specs/other.md`, and
`doc(link, src_id=<tracker>, dst_id=<spec>, rel="supersedes")` through `Artifact::call`. The main
checkout's `other.md` reads `status: superseded` afterwards, and nothing exists at the shadow path
(`<repo>/.worktrees/feat/docs/specs/other.md`). The test asserts the former as its premise.

## Environment

codescout 0.15.0, branch `worktree-agent-ad70a7016cb7773af-remainder`, in-process test context, 2026-10-17.

## Root cause

`src/librarian/tools/link.rs`: `resolve_write_target` is called on `a.src_id` only; the destination is
fetched with `artifact::get(&cat, &a.dst_id)` and written through
`update::write_field_to_frontmatter(ctx, &a.dst_id, "status", ...)`, which writes
`row.abs_path` of whatever row it is given (`update.rs:942-1002`).

## Evidence

The reproduction above. `update::call` resolves its own id through the overlay
(`update.rs:577-580`), so the same status change made with `doc(update)` lands on the shadow; only
`link` skips it.

## Hypotheses tried

1. **Hypothesis:** the destination is deliberately a main-checkout write because a link is
   cross-artifact. **Test:** read `link.rs` and `worktree.rs`: no comment or test says so, and the
   overlay's own doc says every mutating action targeting a main-root artifact forks it.
   **Verdict:** not supported; looks like an omission.

## Fix

Not started. Resolve `dst_id` through `resolve_write_target` before `write_field_to_frontmatter`
and before the `artifact::upsert` of the destination row, and let `name_the_target` name it through
the overlay (today it deliberately names it AS PASSED, with a comment pointing here; that branch
and its test case must change with the fix).

## Tests added

None for the bug itself. The current behaviour is pinned as the premise of the test named under
Reproduction, which reds when the fix lands and says what to change.

## Workarounds

From a worktree session, change the destination's status with `doc(update, patch={status:
"superseded"})` and link with a rel other than `supersedes`.

## Resume

Start at `link.rs:37-64`. After the fix, flip the `supersedes` expectation in the test above to the
shadow path and drop the `through_overlay: false` branch in `write_target_id`.

## References

- `src/librarian/tools/link.rs`, `src/librarian/tools/worktree.rs`, `src/librarian/tools/update.rs`
- `docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md` (Amendment 2026-10-17)
