---
id: 0622f053fa599a5d
kind: bug
status: fixed
title: repair_frontmatter_id has no worktree-registration guard, so it rewrites files inside an active worktree — and the check that feeds it misdiagnoses every shadow as a stale move
owners:
- marius
tags:
- doctor
- librarian
- worktree
- fix-safety
closed: 2026-08-19
opened: 2026-08-19
owner: marius
severity: medium
---

# BUG: `repair_frontmatter_id` has no worktree-registration guard — and `frontmatter_id_mismatch` calls every worktree shadow a stale move

## Summary

Two defects, one root cause: neither `check_frontmatter_id_matches_catalog` nor
`fix=repair_frontmatter_id` knows what a worktree shadow is.

1. **Report:** a shadow row created by the overlay's fork-on-first-write is reported as
   `frontmatter_id_mismatch` with the detail *"a move re-keys the row and this file kept
   the id it was moved away from"*. No move happened. The shadow's frontmatter carries its
   **main twin's** id because the fork copies the main file's frontmatter — which is
   correct, not drift.
2. **Fix:** `fix=repair_frontmatter_id` feeds off exactly that population and filters only
   on `containing_root`. A worktree lives under its main checkout's root, so a registered,
   actively-worked worktree is inside the sweep. `confirm=true` writes to a file in another
   session's live working tree.

Doctor's other two fixes both carry a registration guard and document why. This one does
not.

## Symptom (Effect)

Observed live 2026-08-19 via `librarian(action="doctor")` on the rebuilt binary. One
artifact fires **both** checks:

```
check: frontmatter_id_mismatch
artifact_id: aeece182252e710d
path: /home/marius/work/mirela/backend-kotlin/.claude/worktrees/
      feat+academic-year-scoping-phase2/docs/superpowers/plans/
      2026-08-19-academic-year-scoping-phase2-resolution-write-barrier.md
detail: "frontmatter declares id '8dcbd4fcb9fd5ffc' but the catalog row is
         'aeece182252e710d' — a move re-keys the row and this file kept the id it
         was moved away from"

check: worktree_scoped_row
artifact_id: aeece182252e710d          <- same row
detail: {"main_path": ".../docs/superpowers/plans/2026-08-19-...md",
         "classification": "collision",
         "registered": true,
         "collision_with": "8dcbd4fcb9fd5ffc",     <- the id the frontmatter declares
         "hint": "pending merge — use librarian(action=\"merge_worktree\")"}
```

`collision_with` and the declared frontmatter id are the **same value**. That is the
mechanism working, reported as the mechanism failing.

## Reproduction

1. From a linked worktree, make the first mutating librarian call against a main-repo
   artifact — this forks a shadow row (`get_guide("librarian")` § Worktree overlay).
2. `librarian(action="doctor")` → the shadow appears under `frontmatter_id_mismatch`.
3. `librarian(action="doctor", fix="repair_frontmatter_id", root="<main repo root>")` →
   the dry-run lists the worktree file. `confirm=true` rewrites it.

Step 3's dry-run was not executed against the live backend-kotlin worktree, deliberately:
it belongs to a concurrent session. The containment is read from the code, below.

## Root cause

**`check_frontmatter_id_matches_catalog`** (`src/librarian/tools/doctor.rs`) documents
exactly three abstentions — no `id:` at all, a missing file, unparseable frontmatter. A
worktree shadow is not among them, so it falls through to the `frontmatter_id_mismatch`
arm whose detail text asserts a move.

**`scan_frontmatter_id_mismatches`** filters on `v.check == "frontmatter_id_mismatch"` and
nothing else. Its doc comment calls that filter "the write guard" — true for the
`frontmatter_id_is_not_a_catalog_id` split it was added for, and it is the only guard
between the scan and the writer.

**`run_fix`'s `repair_frontmatter_id` arm** scopes with
`containing_root(&roots, ...)` where `roots = [scope_root]`, and `scope_root` is `root=`
or `current_project.git_root`. `<repo>/.claude/worktrees/<name>/` is under `<repo>`, so
containment passes.

The scope comment above that arm is instructive — it was added after a measured incident
(207 files across five unrelated repos on a live dry-run) and reasons carefully about
**cross-repo** blast radius. The cross-**worktree** axis was never considered, so the fix
is guarded on one axis and open on the other.

## Evidence — the asymmetry is already the house style

Both sibling fixes guard on registration, per the module header:

- `fix=reseat_worktree`: *"`registered` rows (an ACTIVE `worktree_registration` covers
  them) are SKIPPED entirely and reported under `skipped` — they belong to
  `librarian(action="merge_worktree")`."*
- `fix=prune_missing`: *"refuses to prune a dead root an ACTIVE registration still
  covers, so a `git worktree remove` before merge can't silently delete the catalog's
  only remaining record of that worktree's unmerged history."*
- `fix=repair_frontmatter_id`: no registration check anywhere in its arm.

`worktree::covering_conn` is the predicate the other two use, so the guard exists and is
one call away.

## Why it matters

The report defect is the smaller half — a wrong sentence in a diagnostic.

The write defect has a concrete path to a durable wrong value:

1. The sweep rewrites the shadow's `id:` to the **worktree-path-derived** id.
2. That is an uncommitted change in a tracked file, in a working tree belonging to another
   session, appearing mid-task in its `git status`.
3. If that session commits its work — which is the entire point of a worktree — the
   worktree-path-derived id ships.
4. On merge back to main, the file's declared id now names a path that no longer exists,
   which is a **genuine** `frontmatter_id_mismatch`. The repair manufactures the defect it
   exists to remove.

Same family as `docs/trackers/capability-proposals.md` CAP-4 (cross-session annexation):
two sessions in one repo, one silently writing where the other is working.

## Fix idea

**Shipped 2026-08-19** — scan-side abstention only, which corrects this file's own
recommendation.

What this section originally said: *"Prefer **both**: the scan fix stops the misdiagnosis,
the fix-side guard stops the write. They close different halves and neither implies the
other."* **The last clause is wrong.** `scan_frontmatter_id_mismatches` derives its rows
from `check_frontmatter_id_matches_catalog`, so a row that never becomes a violation can
never reach the writer — the scan fix *does* imply the fix-side one. Reading the call chain
refutes it; the recommendation was written from the shape of the two defects rather than
from the code that connects them.

And building the redundant guard would have been actively worse than useless. It would be
unreachable code, so no planted violation could exercise it — a guard that names an
invariant it never runs, which is `prompt-surface-compaction-session-log:F-5`, recorded in
this same session's log hours earlier. The two sibling fixes each carry their own
registration guard because each has a *reachable* path that needs one.

The implementation: `check_frontmatter_id_matches_catalog` gains a fourth abstention,
alongside no-`id:`, missing-file and unparseable-frontmatter. A new `worktree_twin_id`
helper (filesystem-only, mirroring `scan_worktree_scoped`'s own ancestor walk, so no
connection is needed) computes the id the shadow's MAIN twin would carry; when the declared
id equals it, the check abstains.

Only the twin id is excused — a worktree file declaring some *other* id is ordinary
post-move drift and still fires. Nothing is silently dropped either way, because
`scan_worktree_scoped` already reports the row with `collision_with` naming this very id.
## Tests

Three, all planted-violation, then mutation-verified by application:

- `frontmatter_id_mismatch_abstains_for_a_worktree_shadow_declaring_its_main_twin` — with a
  fixture guard asserting the row id and twin id actually differ, so the plain
  `declared == id` early return cannot be what makes it pass.
- `frontmatter_id_mismatch_still_fires_for_a_worktree_file_declaring_an_unrelated_id` — the
  discriminator. Without it, "abstain for anything inside a worktree" passes.
- `repair_frontmatter_id_leaves_a_worktree_shadow_alone_and_still_sweeps_the_stale_row` —
  end to end through `run_fix`. `make_worktree_fixture` puts the worktree UNDER the main
  root, which is the real layout and the reason the old code reached it, so the scope filter
  cannot be what excludes the shadow. A stale row in the main checkout is the positive
  control.

**Mutations applied and run: 3. Killed: 3. Surviving: 0.**

| Mutation | Observed |
|---|---|
| abstention disabled | KILLED — and the failure output reproduced the defect verbatim: `repaired: [{path: ".../main/.worktrees/feat/docs/plan.md", id: "ff6e215eb2aea6e5"}]` |
| abstain for ANY file in a worktree | KILLED by the discriminator |
| `worktree_twin_id` returns the row's own id | KILLED by both positive tests |

The first is the one worth keeping: the regression test is a genuine reproduction of the
write into another session's working tree, not merely a guard against it.
## Resume

Nothing outstanding. Fixed and archived in the same session it was filed.

One thing deliberately NOT done: the report still says nothing about shadows under
`frontmatter_id_mismatch`, because `scan_worktree_scoped` already reports every one of them
with full detail. Emitting a second finding per shadow would double-count the same row in
two checks — which is the shape `check_frontmatter_id_matches_catalog`'s own
missing-file abstention already rejects (*"Reporting it here too would inflate the count on
precisely the rows a repair cannot help"*).
## References

- `src/librarian/tools/doctor.rs` — `check_frontmatter_id_matches_catalog`,
  `scan_frontmatter_id_mismatches`, `run_fix` (`repair_frontmatter_id` arm), module header
  items 7 and 8
- `docs/trackers/capability-proposals.md` — CAP-4 (cross-session collision hint), CAP-8
  (whose substrate check counted this population)
- `get_guide("librarian")` § Worktree overlay — fork-on-first-write and `merge_worktree`


## Fix provenance

- **SHA:** `f772b8fe` (experiments-only) — positional; does not survive a rebase of
  `experiments`.
- **patch-id:** `c5128d873990b049ce956c695a3899750d7b3f08` — content hash of the diff;
  survives rebase and cherry-pick.

Gate at fix time: `cargo fmt` clean, `cargo clippy --all-targets --features dashboard -D
warnings` clean, **4232 passed / 45 ignored**.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep c5128d873990 /tmp/patch-ids.txt
```
