---
status: open
opened: 2026-08-17
closed:
severity: medium
owner: marius
related: []
tags: [librarian, ledger, entry-ids, worktree, merge, link-scan]
kind: bug
---

# BUG: prose ledgers have no worktree collision safety — the merge path can only renumber params entries

## Summary

`append_entry` without `entry_collection` (the prose-ledger branch) forks a
worktree shadow and allocates against it, exactly like the params branch. But
`merge_worktree` can only renumber colliding ids inside a params
`entry_collection`, and the fork event snapshots `base_params` with no
counterpart for the body. So two trees can each issue `HY-11`, and nothing
detects it: git merges two `## HY-11 — …` sections into one file, and the token
becomes uncitable. The params branch has had renumber protection since the
overlay shipped; extending id allocation to prose ledgers did not extend that
protection with it.

## Symptom (Effect)

Not yet observed — latent, found by reading the new prose branch against the
merge path. Predicted observable after merging a worktree branch that appended
to a prose ledger both trees touched:

```
docs/trackers/tracker-hygiene-log.md
  ## HY-11 — <the main checkout's entry>
  ## HY-11 — <the worktree's entry>

link_scan:  HY-11 -> Ambiguous { total: 2 }   # both definers active, same file
            every external citation of HY-11 gets no edge
merge_worktree: entries_renumbered: 0          # the path was never reachable
```

## Reproduction

Commit `66487591`, branch `experiments`. This repo currently has two linked
worktrees (`.claude/worktrees/peer-delegation`, `.worktrees/bench`), so the
precondition is live, not hypothetical.

1. Main checkout: `append_entry(id=<hygiene-log>, id_prefix="HY")` → `HY-11`.
   The `entry_reservation` row for the main id is now 11.
2. Worktree session, same ledger: `append_entry(id=<hygiene-log>, id_prefix="HY")`.
   `resolve_write_target` forks a shadow at the worktree path — a **different**
   `artifact_id` — so the reservation lookup misses, `body_max` comes from the
   worktree's own checkout copy (still 10), and the call returns `HY-11` again.
3. Both sessions write their `## HY-11 — …` section.
4. Merge the worktree branch. Git merges the two sections as text (no conflict
   if they landed in different places in the file).
5. `librarian(action="merge_worktree", root=…)` reports `entries_renumbered: 0`.

## Environment

Linux, `experiments` @ `66487591`. Affects the nine of ten prefixes in
`docs/TAXONOMY.md` that keep entries in prose — i.e. every ledger the prose
branch was built for.

## Root cause

**The prose branch forks, but the merge path cannot renumber what it forked.**

`src/librarian/tools/append_entry.rs:60-62` — the prose branch:

```rust
let mut cat = ctx.catalog.lock();
let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
let outcome = augmentation::allocate_entry_id(&mut cat, &target, &a.id_prefix)?;
```

`resolve_write_target` (`src/librarian/tools/worktree.rs:75-158`) mints
`shadow_id = ids::artifact_id_from_abs(shadow_path)` and seeds the shadow with
the artifact row, the augmentation, a `worktree_fork` event and a `worktree_of`
link. Two things it does **not** seed:

- an `entry_reservation` row — so the shadow's counter starts empty
  (*measured 2026-08-17:* `entry_reservation` appears in 2 files, neither of
  them `worktree.rs` or `merge_worktree.rs`);
- any snapshot of the body. The fork event payload carries `base_params` and
  `base_frontmatter` only (`worktree.rs`, the `FORK_EVENT_KIND` insert).

`src/librarian/tools/merge_worktree.rs:282-306` then folds entries **only**
inside a params collection:

```rust
// Step 3+4: split by entry_collection, fold appended entries, three-way edited-base entries.
if let Some(coll_name) = &coll {
    let base_arr = collection_of(&base_params, coll_name);
    …
    graft::fold_entries(&main_arr, &appended, &mut fold_report)
```

A prose ledger has no `entry_collection`, so `coll` is `None` and the whole
renumber block — the thing that would catch `HY-11` twice — is skipped. It
could not work anyway: `fold_entries` operates on JSON entry rows, and there is
no recorded base body to diff a prose section against.

**Why the damage is uncitability rather than a wrong edge.** Both `HY-11`
headings are in the same artifact and that artifact is `active`, so
`DefinitionIndex::build` pushes two `DefinerRef`s with `active: true`
(`src/librarian/tools/link_scan/resolve.rs:37-45`), and the multi-definer arm
finds `active.len() == 2` → `Ambiguous`, no edge
(`resolve.rs:200-206`, pinned by `multiple_active_definers_yield_ambiguous_no_edge`
at `resolve.rs:339`). Loud, unlike the sibling bug — but it lands after the
merge, when both entries are already written and one must be renumbered by hand.

*Status of this analysis:* inferred from the four call sites cited above —
**not measured at runtime.**

## Evidence

### The refusal this bug wants already exists, for the same reason

`src/librarian/tools/append_entry.rs:86-97` refuses `cites` from a worktree:

```rust
return Err(RecoverableError::with_hint(
    "append_entry: `cites` is not supported from a worktree checkout".to_string(),
    "Entry-graph edges must key to the main tracker. Omit `cites`, or append from the main checkout.".to_string(),
));
```

The stated reason — entry-graph state must key to the main tracker — applies
verbatim to id allocation, which is entry-graph state of the most load-bearing
kind. The guard also carries a hard-won ordering lesson worth reusing: it must
fire **before** `resolve_write_target`, or a refused call still materializes an
empty shadow row, augmentation, fork event and lineage link (the 2026-07-17
regression documented in the same file, asserted by
`append_with_cites_from_worktree_is_refused`).

### The params branch is protected and the prose branch is not

`append_from_worktree_lands_on_shadow_not_main` (`append_entry.rs:565`) asserts
the params branch allocates `F-2` on the shadow against a base holding `F-1`,
and `merge_worktree` renumbers it on the way back. There is no prose-branch
equivalent, in either file.

## Hypotheses tried

1. **Hypothesis:** the shadow inherits the main row's reservation, since it is
   seeded from the main row.
   **Test:** read `resolve_write_target` end to end for what it copies.
   **Verdict:** rejected — it copies the artifact row and augmentation only.
   The reservation is keyed by `artifact_id`, and the shadow's id is different
   by construction (`artifact_id_from_abs(shadow_path)`).
2. **Hypothesis:** `merge_worktree`'s renumber covers body sections too, since
   the memory `worktree-merge-catalog-reconciliation` says merge "renumbers
   colliding entry ids".
   **Test:** read the fold block and the fork event payload.
   **Verdict:** rejected — the renumber is inside `if let Some(coll_name) = &coll`
   and operates on params arrays. The memory's phrasing is accurate for the
   params ledgers that existed when it was written and now over-reads.

## Fix

Not implemented. Two candidates:

**A. Refuse prose allocation from a worktree** (recommended, and cheap).
Mirror the `cites` guard: in the prose branch, if
`is_main_checkout_artifact(cp, &row.abs_path)` holds, refuse with a hint
pointing at the main checkout — **before** `resolve_write_target` can fork.
Rationale: an entry id is a ledger-wide fact, and a worktree is by definition
not the ledger. This is a correctness-by-refusal move, consistent with the
guard already there.

**B. Snapshot body-claimed ids in the fork event and renumber prose at merge.**
Add the fork event's missing counterpart — `base_body_ids: [1..10]` — and teach
`merge_worktree` to renumber colliding `## PREFIX-N` headings in the merged
file. Strictly more capable, and strictly more machinery: it needs a body
rewrite at merge time, which the merge path does not do today for anything.

Fix A first; B only if a real workflow needs to append to a ledger from a
worktree. If **A** is taken, say so in
`get_guide("tracker-conventions")` § *Entry ids* — a refusal nobody can predict
is a papercut.

## Tests added

None yet. Both fixes want the same red test first, against `66487591`:

- seed a prose ledger (`entry_prefix` in frontmatter, `## HY-10` in the body) in
  a `wt_ctx` worktree context via
  `crate::librarian::tools::worktree::test_support::seed_main_tracker`;
- allocate once from the main id and once through the worktree context;
- assert the two returned ids differ.

Under Fix A the second call becomes a `RecoverableError` instead, and the test
asserts the refusal plus zero shadow artifacts / zero `worktree_fork` events —
copying the discriminating assertions in
`append_with_cites_from_worktree_is_refused`.

## Workarounds

**Append to prose ledgers from the main checkout only.** This is already the
practical rule for `cites`; it just is not enforced or documented for ids. If a
worktree session must record something, write it to a worktree-local file and
fold it into the ledger from the main checkout after the merge.

## Resume

Write the red test described in **Tests added** in
`src/librarian/tools/append_entry.rs`'s `tests` module, next to
`append_from_worktree_lands_on_shadow_not_main` (which supplies the `wt_ctx` +
`seed_main_tracker` setup). Assert the two ids collide today; that is the
premise to re-check before implementing Fix A.

## References
- `src/librarian/tools/append_entry.rs:60-62` — the prose branch's fork + allocate
- `src/librarian/tools/append_entry.rs:86-97` — the `cites` refusal to mirror
- `src/librarian/tools/worktree.rs:75-158` — `resolve_write_target`, what the fork seeds
- `src/librarian/tools/merge_worktree.rs:282-306` — the params-only renumber
- `src/librarian/tools/link_scan/resolve.rs:37-45,200-206,339` — two active definers → Ambiguous
- `docs/issues/2026-08-17-ledger-id-reissue-silently-repoints-citations.md` — sibling
  defect, same counter, different trigger
- codescout memory `worktree-merge-catalog-reconciliation` — the overlay flow; its
  "renumbers colliding entry ids" line needs the params-only qualifier
