---
id: e82deca98330f72c
kind: bug
status: open
title: 'BUG: frontmatter_id_mismatch still asserts a move for ids that were minted in another checkout'
tags:
- cluster/hint-composed-without-the-request
---

## Summary

`frontmatter_id_mismatch`'s `detail` states one cause as fact:

> `frontmatter declares id 'X' but the catalog row is 'Y' — a move re-keys the row and this file kept the id it was moved away from`

For a **worktree-minted** id that is false, and the check cannot tell. Measured 2026-09-05: **8 of 8**
live instances were minted in a worktree that has since been removed. None was ever moved.

This is the residue of `2026-08-18-frontmatter-id-mismatch-asserts-one-cause-and-repairs-placeholders.md`,
archived as SHIPPED. That fix split off values which are not catalog ids at all — template
placeholders, hand-written slugs — into `frontmatter_id_is_not_a_catalog_id`, and states that the
move message is *"unchanged for the rows it is actually true of."* The partition is by **shape**
(`is_librarian_id`), used as a proxy for the property the message asserts: **was this row moved?**
A worktree-minted id is well-formed 16-hex, so it passes the shape test and inherits a causal claim
nothing established. Well-formed does not imply moved.

## Symptom (Effect)

A reader who trusts the field goes looking for a move in `git log` and finds none, because there was
none. The message returns a plausible cause rather than an error, so nothing downstream fires.

## Reproduction

At `593614aa` on `experiments`, against the 8 rows repaired that day:

```
librarian(action="doctor", fix="repair_frontmatter_id")   # dry run
  → 8 files, every detail reading "a move re-keys the row …"

git log --follow --diff-filter=R -- <any of the 8>        # → empty; never renamed
sha256("<repo>/.worktrees/tool-collapse/<rel_path>")[:16] # → 6ccfcc15423f2ae5, the stale id
```

The third line is the positive identification: the stale id is reproduced exactly from a worktree
path, so the id was minted in a checkout that no longer exists.

## Environment

`experiments`. `.worktrees/tool-collapse` was removed before this was noticed — `git worktree list`
shows only the main checkout, which is why nothing on disk points at the real cause.

## Root cause

**The check observes a mismatch and the message asserts a history.** `id = sha256(abs_path)`, so a
declared id differing from the catalog's says only *"this id was minted against a different absolute
path."* Three ways that happens, and the check distinguishes none:

| how the id got there | shape | is the move message true? |
|---|---|---|
| the row was moved and the file kept the old id | 16-hex | yes |
| the file was authored in a worktree / second clone | 16-hex | **no** |
| the value was never a catalog id (placeholder, slug) | not 16-hex | no — split off 2026-08-18 |

Rows 1 and 2 are indistinguishable *by shape*, which is the only axis the 2026-08-18 partition uses.
Telling them apart needs evidence the check does not consult — git rename history, or a hash trial
against known roots.

There is a wired signal for the worktree case that this check does not reach for:
`worktree_scoped_row` (0 today, because the worktree was removed — `is_linked_worktree` reads the
worktree's `.git` pointer, so removal makes the row unrecognisable, exactly as memory
`worktree-merge-catalog-reconciliation` warns).

## Hypotheses tried

1. **Hypothesis:** a rediscovery of the archived 2026-08-18 bug.
   **Verdict:** rejected. That bug's population is values that are *not* catalog ids; its fix routes
   them to a second finding and deliberately leaves the move message for the rest. These 8 are in
   "the rest" and the message is still false for them.
2. **Hypothesis:** the files really were moved and git rename detection missed it.
   **Verdict:** rejected by positive identification — the worktree path reproduces the stale id
   under sha256. Not "no evidence of a move"; evidence of something else.

## Fix

Say what is known and stop. The mismatch is certain; the cause is not. Either:

- **State the disjunction** — *"this id was minted against a different absolute path: the row was
  moved, or the file was authored in another checkout (worktree / second clone). Check
  `git log --follow --diff-filter=R` to tell them apart."* Costs nothing and is true of all three rows.
- **Or determine it** — trial-hash the rel_path against known and historical worktree roots
  (`git worktree list`, plus roots recorded in the catalog) and name the match when one is found.
  Strictly better when it hits, and it hit on the first candidate here.

The repair itself is correct either way: rewriting the `id:` to the catalog row's id is right
whatever minted the old value. Only the explanation is wrong.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: build a fixture whose frontmatter id is the sha256 of a
path under a *different* root, with no rename in git, and assert the detail does not claim a move.
A test asserting the current string passes today and would keep passing while the claim stays false.

## Workarounds

Do not act on the `detail`. Establish the cause yourself: `git log --follow --diff-filter=R` for a
move, and a sha256 trial against candidate roots for a foreign checkout.

## Resume

Widen the message to the disjunction first — it is one string and true of every row. The trial-hash
refinement can follow.

## References

- Parent, archived as SHIPPED, whose partition this escapes:
  `docs/issues/archive/2026-08-18-frontmatter-id-mismatch-asserts-one-cause-and-repairs-placeholders.md`.
- Encountered while repairing the 8 rows: fix `593614aa`.
- Memory `worktree-merge-catalog-reconciliation` — why worktree-born rows orphan on merge, and why
  `worktree_scoped_row` cannot see them once the worktree is removed.
- `CLAUDE.md` § *Observer Blindness* — "Never close an authorship question by elimination — identify
  positively." The cause here was settled by reproducing the id, not by ruling a move out.

