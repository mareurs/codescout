# Worktree merge → catalog reconciliation

> **UPDATE 2026-07-17 — a first-class overlay flow is built on `experiments`
> (not yet on `master`).** The post-hoc reconciliation below is what's true on
> `master` today and remains the LEGACY fallback for UNREGISTERED worktree rows.
> On `experiments`, a worktree-overlay feature makes reconciliation write-time
> instead of merge-time: worktree sessions read the main catalog via an overlay
> and fork-on-first-write into shadow rows (recorded `worktree_of` link +
> `worktree_fork` base-snapshot event + durable `worktree_registration` row that
> survives `git worktree remove`); merge is `librarian(action="merge_worktree",
> root=…, [dry_run]/[abandon])` — it folds ONLY the shadow's DELTA onto main
> (never bare-grafts a seeded shadow), three-ways scalars (main wins on
> conflict), renumbers colliding entry ids, and closes the registration. `doctor`
> now flags registered rows as `pending_merge` (skipped by `reseat_worktree`) and
> `prune_missing` refuses a root with an active registration. When this ships to
> `master`, the overlay flow becomes primary and the steps below apply only to
> pre-feature (unregistered) rows. Branch commits: 4450f20f..c2104e90. Design:
> `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md`; plan:
> `docs/superpowers/plans/2026-07-17-worktree-overlay.md`; session log:
> `docs/trackers/worktree-overlay-session-log.md` (F-1..F-4, W-1).

When merging a git worktree branch that created/updated **librarian trackers**, the
catalog needs explicit reconciliation — `git merge` moves file *content* but is blind
to the catalog (`artifact id = sha256(abs_path)`; `append_entry` writes only
`artifact_augmentation.params` in the DB, never the `.md`). Rows created at the
worktree path orphan on merge.

**Do this (once the MCP binary has the tools — `cargo rb` + `/mcp` if `doctor`'s
schema lacks `worktree_scoped_row` / the `fix` enum lacks `reseat_worktree`):**
1. Merge the branch (rebase-before-merge keeps the deterministic renumber path valid).
2. `librarian(action="doctor")` → find `worktree_scoped_row` violations; read each
   `classification` (`no_collision` | `collision`).
3. **no_collision** → `librarian(action="doctor", fix="reseat_worktree")` (seeds the
   main-path id + grafts children across — durable, survives reindex).
4. **collision** → settle file content via R-3 first, then
   `artifact(action="graft", from_id=<worktree-id>, into_id=<main-id>)`; check the
   returned `remap`/`suspicious`, rewrite live-tree citations + breadcrumb.
5. Verify (`doctor` clean, entries+events under the surviving id), THEN
   `git worktree remove`.

**Ordering is load-bearing: reconcile BEFORE `git worktree remove`.** Detection uses
`is_linked_worktree` (reads the worktree `.git` pointer); once removed, the row is
unrecognizable and becomes a missing-file orphan that `prune_missing` deletes.
(NOTE: the overlay flow above removes this hazard — its `worktree_registration` row
survives removal, so `merge_worktree` works from DB state alone.)

**Do NOT improvise `reindex` + manual re-augment + `prune_missing`.** A RED-baseline
agent that couldn't discover `graft`/`reseat` did exactly this — it silently DROPS the
event log (reindex makes a fresh row with no events; prune deletes the old) and relies
on hand-copying params. `reseat`/`graft` re-point events/observations/links/event_edges
+ migrate augmentation atomically; the improvised path does not.

Tools shipped on `experiments` (commits 49214372..bc1119c9 + 89fd089b); a companion
orchestration skill was investigated and REFUTED by baseline (tools + schema
discoverability suffice). Design: `docs/superpowers/specs/2026-07-10-worktree-merge-tracker-safety-design.md`.