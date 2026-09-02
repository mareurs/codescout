---
kind: bug
status: open
tags:
- cluster/repro-env-diverges-from-gate-env
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: medium
unverified: 'Mechanism NOT established. Seven candidate causes were ruled out at the bytes (below); the surviving ones are untested. Reproduced ONCE, on one ledger, on a checkout with a linked worktree — n=1, and no second ledger was probed because every probe mutates a high-water mark. Do not read this as ''the guard is dead'': `unpushed_is_per_file_not_per_branch` and `allocation_is_refused_while_the_ledger_has_unpushed_commits` both pass, so it demonstrably fires in a fixture.'
---

# `append_entry` allocated an id on a ledger with three unpushed commits, which is the state the guard refuses

## Symptom

`artifact(action="append_entry", id="2dd9d90bc83f9f49", id_prefix="F")` **succeeded**,
returning `F-107` and writing `entry_high_water_F: 107` — while
`docs/trackers/bug-fix-session-log.md` had three commits in `@{upstream}..HEAD`.

That is exactly the state `ledger_has_unpushed_commits` exists to refuse, and it is the
state `get_guide("tracker-conventions")` tells every author will be refused:

> **Not while the ledger's own commits are unpushed.** `append_entry` also refuses when the
> ledger *file* has commits in `@{upstream}..HEAD` … Push the ledger's commits and retry.

## Why it matters more than one allocation

The guard is **partial by construction** — its own comment says so — and what it converts is
an invisible divergence into a pushed one. A silent allowance therefore restores the exact
failure it was built for: two hosts issuing `F-107`, visible only after a merge, as one token
with two definitions. Nothing downstream fires, because `entry_defined_twice` is a
**detector, not a guard** and only sees the state once it is already merged.

Second-order, and the reason this is filed rather than shrugged at: I told my user twice this
session that ledgers were refusing appends at this branch depth, and cited it as a cost of
not pushing. That was **false**, and I had no way to know — the refusal is the only observable
this guard has, so "it did not refuse" and "there was nothing to refuse" are the same event
from outside.

## Ruled out, at the bytes

| candidate | checked | result |
|---|---|---|
| Guard not in the running binary | landed `0cb617cc` 14:57; release binary mtime 22:25:16 | in |
| `is_ledger` false | `bug-fix-session-log.md` frontmatter declares `entry_prefix: [F, W]` | true |
| No unpushed commits on the file | `git log origin/experiments..HEAD -- <file>` | **3** |
| Those commits are merges (`parent(0)` diff would differ) | `cffc3cf2`, `0280d2b7`, `35b9ef71` | all single-parent |
| File absent at `row.abs_path` (a documented allow-path) | `test -f` on the catalog's `abs_path` | exists |
| `strip_prefix(workdir)` mismatch from a symlinked root | `readlink -f` vs `git rev-parse --show-toplevel` | byte-identical, no symlink |
| Upstream unconfigured, or `head_oid == up_oid` | `@{upstream}` → `origin/experiments`; 40 commits ahead | configured, diverged |

## Not ruled out

- **`git2::Repository::discover()` is given a FILE path, not a directory.** Every call site
  passes `row.abs_path`, which names the `.md` file. The helper's own doc comment already
  records that `discover()` "errs for a nonexistent path"; whether it also errs — or resolves
  differently — for an existing *file* was not tested. Every failure path allows, so this
  would be silent by design.
- **A linked worktree in the repo.** This checkout has `.worktrees/tool-collapse`. `doctor`
  reports `worktree_scoped_row: 12` and `abs_path_outside_managed_roots: 10` on this catalog,
  so worktree-aware discovery is a live variable here and absent from every fixture.
- Something inside the revwalk / `diff_tree_to_tree` path.

## Why the passing tests do not settle it

`unpushed_is_per_file_not_per_branch`, `allocation_is_refused_while_the_ledger_has_unpushed_commits`
and `allocation_proceeds_when_the_ledger_has_no_unpushed_commits` all pass. They build their
own repo in a tempdir — no linked worktree, a local remote, and a path the fixture controls
end to end. So they establish the helper's *logic* and say nothing about the two surviving
candidates, both of which are properties of the environment rather than of the algorithm.
This is `cluster/repro-env-diverges-from-gate-env` in the shape where the gate env is the
*simpler* one.

## Reproduction

```
git log --oneline origin/experiments..HEAD -- docs/trackers/bug-fix-session-log.md   # 3
artifact(action="append_entry", id="2dd9d90bc83f9f49", id_prefix="F", ...)           # succeeds
```

**Costly to re-run**: every probe allocates an id and advances a committed high-water mark.
The cheap next step is a unit test that passes a *file* path to
`ledger_has_unpushed_commits` — the current three all pass a path the fixture built — and a
second that builds a fixture with a linked worktree. Both are assertions about the two
surviving candidates rather than a re-run of this observation.

## Environment

- codescout `experiments`, release binary built 22:25:16 from a tree at `741cda03`
- 40 commits ahead of `origin/experiments`; nine sessions sharing this checkout
- one linked worktree: `.worktrees/tool-collapse`

## References

- `src/librarian/tools/append_entry.rs:369` — `ledger_has_unpushed_commits`
- `src/librarian/tools/append_entry.rs:154` — the call site and its `is_ledger` condition
- `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md` — the
  bug this guard closed, whose failure mode a silent allowance reopens
- `src/prompts/guides/tracker-conventions.md` § *Entry ids* — the claim this falsifies
