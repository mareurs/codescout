---
status: fixed
opened: 2026-08-17
closed: 2026-08-17
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

Not observed in the wild, but **reproduced under test 2026-08-17**. Main allocates
`HY-11`; a worktree session against the same ledger allocates `HY-11` again:

```
---- librarian::tools::append_entry::tests::prose_allocation_from_a_worktree_collides_with_the_main_checkout stdout ----
thread '...' panicked at src/librarian/tools/append_entry.rs:697:9:
assertion `left != right` failed: the worktree re-issued HY-11: the shadow is a
different artifact_id so the reservation misses, and merge_worktree can only renumber
params rows — so two `## HY-11 — …` sections merge into one file and the token becomes
uncitable
  left: String("HY-11")
 right: "HY-11"
```

The test's *preceding* assertion — `out["artifact_id"] != main_id` — passed, which is
what makes the failure meaningful: the shadow fork genuinely happened, so this is the
overlay path allocating a duplicate, not a fixture that failed to look like a worktree.

Predicted state after merging such a branch:

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

*Status of this analysis:* **measured 2026-08-17** — `cargo test --lib -- --ignored
prose_allocation_from_a_worktree` reproduces the duplicate id, with the fork itself
asserted rather than assumed (output in § Symptom). The four call sites above are each
read from the cited lines.

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

**Fix A, implemented 2026-08-17 in `0364c23a` (`experiments`).** Promotion to `master` is
a fast-forward (`git rev-list --left-right --count master...experiments` → `0` on the
left), so this SHA *is* the master SHA once promoted — no second SHA to record later.

The prose branch of
`src/librarian/tools/append_entry.rs` refuses id allocation from a worktree session,
mirroring the `cites` guard directly above it:

```
append_entry: id allocation is not supported from a worktree checkout
hint: An entry id is ledger-wide state and must key to the main tracker. Reserve the id
      from the main checkout, or record the entry in a worktree-local file and fold it
      into the ledger after the merge.
```

The guard tests `is_main_checkout_artifact(cp, &row.abs_path)` and fires **before**
`resolve_write_target`. That ordering is the non-obvious half and it is borrowed, not
reasoned from scratch: the `cites` guard originally refused *after* the fork, so a
refused call still left a shadow row, an augmentation, a fork event and a lineage link
behind — contradicting its own "writes nothing" contract (the 2026-07-17 regression).
Checking `is_main_checkout_artifact` rather than inspecting the resolved target is what
makes the early refusal possible.

**Fix B (snapshot body-claimed ids in the fork event and renumber prose at merge) was
not taken.** It is strictly more capable and strictly more machinery: it needs a body
rewrite at merge time, which the merge path does not do for anything today. Revisit only
if a real workflow needs to append to a ledger from a worktree.

Documented in `get_guide("tracker-conventions")` § *Entry ids* — a refusal nobody can
predict is a papercut, so the guide now states the rule and the reason next to "let the
server allocate".
## Tests added

`prose_allocation_is_refused_from_a_worktree` —
`src/librarian/tools/append_entry.rs`. Written red first under the previous name
`prose_allocation_from_a_worktree_collides_with_the_main_checkout` (that name appears in
the captured output in § Symptom, which is a historical record and deliberately not
rewritten). Now green, `#[ignore]` removed.

It sits immediately after `append_from_worktree_lands_on_shadow_not_main`, the protected
params-branch twin, so the pair reads as one story about what the prose branch did not
inherit.

Four assertions, and the shape matters more than the count:

- **the discriminating half** — the same ledger allocates `HY-11` fine from the main
  checkout. Without it the test would also pass against a fixture that refuses
  everything;
- the worktree call returns a `RecoverableError` naming "worktree";
- exactly **one** artifact row, **zero** `worktree_fork` events, **zero** `worktree_of`
  links afterwards — the refusal must beat the fork, copied from
  `append_with_cites_from_worktree_is_refused`.

Two fixture decisions worth keeping:

- **Its own fixture, not `wt_ctx` / `seed_main_tracker`.** Those seed
  `/repo/docs/trackers/t.md`, a path with no file behind it — fine for the params branch,
  which reads params from the DB, but `allocate_entry_id` reads the ledger body off disk
  and hard-errors on a missing file. That asymmetry is itself the defect restated: the
  two branches depend on different substrate, which is why one inherited the merge-time
  protection and the other could not.
- **The worktree root is nested inside the repo** (`repo/.worktrees/feat`), matching this
  project's own layout. `is_main_checkout_artifact` discriminates by
  `under(main) && !under(worktree)`, so the nesting resolves correctly.
## Workarounds

No longer a workaround — it is now the enforced rule: **append to prose ledgers from the
main checkout only.** If a worktree session must record something, write it to a
worktree-local file and fold it into the ledger from the main checkout after the merge.
## Resume

N/A — fixed and verified on `experiments`.
## References
- `src/librarian/tools/append_entry.rs:60-62` — the prose branch's fork + allocate
- `src/librarian/tools/append_entry.rs:86-97` — the `cites` refusal to mirror
- `src/librarian/tools/worktree.rs:75-158` — `resolve_write_target`, what the fork seeds
- `src/librarian/tools/merge_worktree.rs:282-306` — the params-only renumber
- `src/librarian/tools/link_scan/resolve.rs:37-45,200-206,339` — two active definers → Ambiguous
- `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md` — sibling
  defect, same counter, different trigger
- codescout memory `worktree-merge-catalog-reconciliation` — the overlay flow; its
  "renumbers colliding entry ids" line needs the params-only qualifier
