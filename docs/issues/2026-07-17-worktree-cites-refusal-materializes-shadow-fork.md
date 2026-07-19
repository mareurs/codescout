---
status: open
opened: 2026-07-17
closed:
severity: low
owner: marius
related: []
tags: [worktree, append_entry, entry-graph]
kind: bug
---

# BUG: append_entry cites-from-worktree refusal still materializes an empty shadow fork

## Summary
When `append_entry` is called with `cites` from a linked-worktree checkout, the
tool correctly REFUSES the call (entry-graph edges must key to the main tracker).
But the refusal fires *after* `resolve_write_target` has already forked and
committed a shadow tracker, so a spurious empty shadow row + `worktree_fork`
event + `worktree_of` lineage link are materialized before the error returns —
contradicting the tool's advertised "aborts the whole call / writes nothing"
contract.

## Symptom (Effect)
A `cites`-from-worktree `append_entry` returns a `RecoverableError`
("append_entry: `cites` is not supported from a worktree checkout"), and no
`entry_cite` row and no entry are written (the entry write itself is atomic).
However, the catalog gains an empty shadow-fork artifact + its augmentation +
a `worktree_fork` event + a `worktree_of` link for the target tracker.

## Reproduction
Not independently reproduced beyond the code trace; the branch's own test
`append_with_cites_from_worktree_is_refused` (`src/librarian/tools/append_entry.rs`)
asserts `COUNT(entry_cite)=0` on refusal but does NOT assert the *absence* of a
shadow fork, so it passes despite this.

Commit: entry-graph Stage 2 head `27176006` on `experiments`.

## Environment
codescout `experiments`, librarian catalog, worktree-overlay path.

## Root cause
In `src/librarian/tools/append_entry.rs` `call`, the ordering is:

```
let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;  // forks + commits shadow
if !a.cites.is_empty() && target != a.id { return Err(... refuse ...); }     // refuse AFTER the fork
```

`resolve_write_target` (`src/librarian/tools/worktree.rs:110-157`) upserts a
shadow row + shadow augmentation, inserts a `worktree_fork` event and a
`worktree_of` lineage link, and commits its own transaction. The guard runs only
after that returns, so the shadow is already durable when the call is refused.

## Evidence
Final whole-branch review of range `37560641..27176006` (2026-07-17, Opus),
finding #2. Code trace confirmed against `worktree.rs` fork path.

## Hypotheses tried
N/A — mechanism confirmed by code trace at the time of filing.

## Fix
Detect the worktree-main condition (via `CurrentProject.main_root` +
`is_main_checkout_artifact`, mirroring the guard's `target != a.id` intent)
and refuse BEFORE calling `resolve_write_target`, so no shadow is materialized.
Deferred from the Stage-2 fix wave to avoid scope-creeping the worktree-overlay
subsystem; low impact because the shadow fork is idempotent and would be created
by the next legitimate worktree append anyway.

## Tests added
N/A — deferred. When fixed, extend `append_with_cites_from_worktree_is_refused`
to assert no shadow-fork artifact / `worktree_fork` event exists after refusal.

## Workarounds
Append `cites` from the main checkout (the MVP-supported path). The stray shadow
is harmless (idempotent; reused by the next real worktree append).

## Resume
Fixed at `4c0f8874` (branch `experiments`). `append_entry::call` now checks
`is_main_checkout_artifact` directly (predicting `resolve_write_target`'s
`target != a.id` outcome) and refuses BEFORE ever calling
`resolve_write_target` when `cites` is non-empty — no shadow fork is
materialized on the refused path. `append_with_cites_from_worktree_is_refused`
extended to assert artifact count, `worktree_fork` event count, and
`worktree_of` link count all stay at zero/baseline after refusal. Kept
`open`->`fixed` in `docs/issues/`; archives once the fix ships to `master`.
## References
- Final whole-branch review, entry-graph Stage 2 (range `37560641..27176006`).
- `docs/superpowers/plans/2026-07-17-tracker-entry-graph-stage2.md` (Task 4).
- Related MVP boundary: cites refused from worktree (spec § MVP boundaries).
