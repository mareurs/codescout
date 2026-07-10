# Worktree merge → catalog reconciliation

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

**Do NOT improvise `reindex` + manual re-augment + `prune_missing`.** A RED-baseline
agent that couldn't discover `graft`/`reseat` did exactly this — it silently DROPS the
event log (reindex makes a fresh row with no events; prune deletes the old) and relies
on hand-copying params. `reseat`/`graft` re-point events/observations/links/event_edges
+ migrate augmentation atomically; the improvised path does not.

Tools shipped on `experiments` (commits 49214372..bc1119c9 + 89fd089b); a companion
orchestration skill was investigated and REFUTED by baseline (tools + schema
discoverability suffice). Design: `docs/superpowers/specs/2026-07-10-worktree-merge-tracker-safety-design.md`.