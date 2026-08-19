---
id: '43b65f77ef0e8187'
kind: bug
status: open
title: repair_frontmatter_id has no worktree-registration guard, so it rewrites files inside an active worktree — and the check that feeds it misdiagnoses every shadow as a stale move
owners:
- marius
tags:
- doctor
- librarian
- worktree
- fix-safety
---

---
status: open
opened: 2026-08-19
closed:
severity: medium
owner: marius
tags: [doctor, librarian, worktree, fix-safety]
kind: bug
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

Add the registration guard the siblings already have. Either:

- **In the scan** — abstain in `check_frontmatter_id_matches_catalog` when the row is
  worktree-scoped with an active registration, and say so, so the report stops asserting a
  move. This also keeps the count honest for anyone measuring the mismatch population.
- **Or in the fix** — skip covered rows in `run_fix` and report them under `skipped`,
  mirroring `reseat_worktree` exactly.

Prefer **both**: the scan fix stops the misdiagnosis, the fix-side guard stops the write.
They close different halves and neither implies the other. Mirror `prune_missing`'s
dry-run treatment too — it surfaces `would_skip: "active worktree registration"` so the
preview never promises more than `confirm=true` delivers.

## Tests

Must observe a *planted* violation and then apply mutations, per `CLAUDE.md` §
mutation-apply discipline:

- a registered worktree shadow whose frontmatter names its main twin produces **no**
  `frontmatter_id_mismatch`;
- a genuinely stale post-move id still does;
- a dry-run over a root containing a registered worktree lists the stale row and not the
  shadow.

## Resume

Not fixed in the session that found it — surfaced during reconnaissance after an unrelated
`doctor` change, and the write path belongs to a concurrent session's worktree. Decide the
scan-side vs fix-side split (recommendation: both) before implementing.

## References

- `src/librarian/tools/doctor.rs` — `check_frontmatter_id_matches_catalog`,
  `scan_frontmatter_id_mismatches`, `run_fix` (`repair_frontmatter_id` arm), module header
  items 7 and 8
- `docs/trackers/capability-proposals.md` — CAP-4 (cross-session collision hint), CAP-8
  (whose substrate check counted this population)
- `get_guide("librarian")` § Worktree overlay — fork-on-first-write and `merge_worktree`

