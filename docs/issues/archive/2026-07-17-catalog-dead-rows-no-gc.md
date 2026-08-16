---
id: ce8b21ed4762de34
kind: bug
status: fixed
title: 'BUG: shared catalog silently serves 179 dead rows (deleted repos never pruned) — doctor detects, but nothing runs it and find/search return ghosts'
tags:
- catalog
- librarian
- gc
- doctor
closed: 2026-07-20
---

## Summary

`librarian(action="doctor")` on 2026-07-17 reports **181 violations**: 179 `missing_file` (catalog rows whose `abs_path` no longer exists on disk) + 2 `worktree_scoped_row`. Cluster breakdown of the 179:

| Root | Rows | Likely cause |
|---|---|---|
| `stefanini/AI-enablement` | 77 | repo deleted/moved |
| `stefanini/IATA` | 52 | repo deleted/moved |
| `/tmp/…` | 28 | probe/test pollution — separate bug `2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md` |
| `stefanini/southpole` | 11 | partial deletions/moves |
| `mirela/deployment` | 5 | file deletions |
| other `/home` | 6 | — |

## Why it's a bug

Detection exists (`doctor`) and repair exists (`fix="prune_missing"`, per-root), but the loop is open: nothing schedules doctor, no read path consults its results, and dead rows stay `active` — so `artifact(find, scope="all")`, semantic search, staleness lists, and link-graph traversals can all return artifacts that don't exist. During the 2026-07-17 tracker survey this manifested as ghost augmented trackers in an `artifact_augmentation` audit.

This is the shared-catalog twin of the fix-then-forget pattern already documented for bug files / doc refs / session-log statuses (CLAUDE.md "Verify-open cadence"): a fourth bookkeeping surface leaking the same way.

Related prior art: `2026-06-13-delete-orphan-repos-cross-workspace-wipe.md` (fixed) removed the *automatic* orphan-repo deletion because it was dangerously over-broad — the pendulum landed at fully-manual, and the result two months later is 179 dead rows.

## Fix directions

- A safe middle ground between the wiped auto-GC and manual doctor: e.g. `missing_since` timestamping on rows doctor flags, hide-from-find after N days missing, prune only after M days + explicit confirm.
- Batch prune UX: `fix="prune_missing"` takes one `root` per call and refuses existing paths — pruning 6 clusters requires 6+ invocations and can't handle `/tmp` (exists) without per-subdir roots.
- Surface doctor summary in `workspace(action="status")` or the SessionStart banner so the drift is at least visible.

## Status log
- 2026-07-19 — **ROOT-CAUSE FIX (ongoing-GC lifecycle) SHIPPED on `experiments`, NOT master.** The open detect→repair loop is now closed: `missing_since` column (v10 migration) + existence-based reconcile pass (`gc::reconcile_missing_since`) throttled best-effort on the librarian-adapter call path → hide-from-find past a 14-day grace (all catalog read sites, reversible; `workspace_state_at` bypasses for historical fidelity) → surfaced in `librarian(action="doctor")` `catalog_health` {hidden_rows, move_candidates}. **Deletion stays human-gated forever** (auto-hide, never auto-delete — decided to avoid repeating the [[2026-06-13-delete-orphan-repos-cross-workspace-wipe]] over-aggression). ALSO delivered rename/move recovery (the deferred "option 1" of [[2026-06-13-catalog-orphans-survive-repo-rename]]): `doctor(fix="rehome", old_root, new_root)` — id-rewrite preserving ALL children (events/event_edges/augmentation/observation/links/vec/entry_cite.dst_ref) under deferred FKs, dry-run→confirm, auto-detected via git2 commit-hash overlap. 9-task SDD, each Opus/Sonnet-reviewed + whole-branch Opus review; full suite 3366/0. Spec docs/superpowers/specs/2026-07-19-catalog-gc-lifecycle-rehome-design.md, plan docs/superpowers/plans/2026-07-19-catalog-gc-lifecycle-rehome.md. Feature commits c62fe9a5..10e5ed0e (21 incl. plan docs). **Kept `open`** — archive only after cherry-pick to master (+ live `cargo rb`/`/mcp` verify).

- 2026-07-17 — opened. Read-only survey; no prune performed (deletion decisions belong to the user — AI-enablement/IATA may be intentionally offline).
- 2026-07-18 — **CLEANUP capability shipped** on `experiments` (not yet master): `librarian(action="doctor", fix="prune_missing")` with NO `root=` now runs BATCH mode — derives dead roots via the "parent-also-gone" rule (highest nonexistent ancestor whose parent exists; single missing files under a live repo are left to reindex), dry-run by default (lists roots + row counts, marks worktree-covered roots `would_skip`), `confirm=true` prunes each via the existing guarded `prune_dead_root` (skips active worktree registrations; `is_absolute` guard prevents an empty-root catalog-wipe). Reviewed (Opus) + full suite green (3323/0). NOT yet executed — the 179 dead rows need a gated dry-run→confirm→apply against the real catalog (needs `cargo rb` + /mcp reconnect). **Ongoing-GC lifecycle** (`missing_since` / hide-from-find / time-based auto-prune / status surfacing) explicitly **DEFERRED** to a follow-up spec. Spec/plan: docs/superpowers/specs/2026-07-17-catalog-hygiene-prevention-cleanup-design.md, docs/superpowers/plans/2026-07-18-catalog-hygiene-prevention-cleanup.md. Kept `open`.
- 2026-07-18 (cont.) — **`/tmp` portion of the dead rows CLEANED** (28+ ephemeral test/probe roots, see [[2026-07-17-tmp-probe-artifacts-pollute-global-catalog]]). The **142 real deleted-repo rows REMAIN, intact and preserved**: AI-enablement (77), IATA (52), southpole/MRV-poc/docs/meetings (10) + southpole/docs/trackers/archive (1), agents/system/emag (2) — all 2026-04-20/06 project work. Pending USER decision on whether those repos are permanently gone (prune) vs. possibly returning (keep the catalog history). `derive_dead_roots` correctly leaves southpole's hundreds of LIVE rows untouched. Ongoing-GC lifecycle still deferred to a follow-up spec. Kept `open`.
