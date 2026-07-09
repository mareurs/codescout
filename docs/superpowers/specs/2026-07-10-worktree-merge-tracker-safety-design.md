---
kind: spec
status: draft
title: Worktree-merge tracker safety — catalog reconciliation
owners: [marius]
tags: [librarian, worktree, catalog, trackers, merge, doctor, graft, skill]
topic: worktree-merge-tracker-safety
time_scope: 2026-07-10
---

# Worktree-merge tracker safety — catalog reconciliation

## Summary

When a git worktree branch that carries librarian trackers is merged back to
`master`, `git` reconciles **file content** but is structurally blind to the
librarian **catalog** — the machine-global SQLite DB that holds artifact
identity, event logs, link edges, and augmentation params. Worktree-born or
worktree-augmented tracker rows are keyed by a hash of their *absolute path*,
so after a merge they are orphaned: the file lands at its `master` path while
the catalog row still points into `.worktrees/<name>/…`, and its event /
augmentation history is invisible to any path-walk reindex.

This design adds:

1. Two **codescout MCP tools** (in the `codescout` repo) that make the
   catalog side of a worktree merge safe: a `doctor` check + `fix`, and an
   `artifact(action="graft")` verb.
2. One **codescout-companion skill** (in the `claude-plugins` repo) that
   orchestrates the end-to-end safe merge, delegating the git-visible half to
   the existing R-3 methodology and owning the git-blind (catalog) half.

The whole reconciliation is **intra-database** — there is no cross-DB import
(see Verified Facts). The merge is row re-pointing (`reseat`) and row folding
(`graft`) within one catalog.

## Motivation — what breaks today

A concrete instance lives in the repo right now: artifact
`e44f81b1eea2eac4` (`single-stage-cpsat-spike.md`) has
`abs_path = .worktrees/single-stage/docs/trackers/single-stage-cpsat-spike.md`.
It was created **while a project was activated at the worktree path** (the
indexer skips linked worktrees — see below — so `artifact(create)` is the only
way such a row appears). Dozens of `artifact(update)` / `append_entry` /
`artifact_event` calls have since written to that row by `id`. When the
`single-stage` branch merges, nothing reconciles that row: `git worktree
remove` deletes the checkout, and `doctor(fix=prune_missing)` would eventually
*delete* the orphaned row rather than migrate its history.

This is the catalog-layer form of the "fix-then-forget" bookkeeping leak the
project documents in three places (CLAUDE.md verify-open cadence,
`branch-cleanup-audit-session-log.md`, the `audit_doc_refs` gate).

## Verified facts (grounding — read against source this session)

| Fact | Evidence |
|---|---|
| Artifact id = `sha256(abs_path)[..16]`; the only production id fn is the abs-path one. `artifact_id(repo, rel_path)` exists but has **zero** production callers. | `src/librarian/ids.rs:4`, `:17`; `call_graph(artifact_id)` → tests only; `references(artifact_id_from_abs)` → `indexer.rs:94`, `create.rs:98` |
| The catalog DB is **machine-global**: `$LIBRARIAN_DB` or `dirs::data_local_dir()/librarian/catalog.db`, resolved once at server start, independent of cwd / repo / worktree. `current_project` is a scope *lens* only. | `src/librarian/mod.rs:41` (`build_tool_context`) |
| The indexer **skips linked worktrees** (`.git`-file → `gitdir: …/worktrees/<name>`). So worktree tracker rows only ever appear via `artifact(create)` at the worktree path, never via a walk. | `src/librarian/indexer.rs:56` (guard), `current_project.rs:56` (`is_linked_worktree`) |
| `append_entry` writes **only** `artifact_augmentation.params` (DB), never the `.md` file. So augmented-tracker structured entries are git-invisible. | `src/librarian/catalog/augmentation.rs:178` |
| `artifact(move)` renames the file on disk **and** updates the row, keeping the same `id`; it errors if the destination already exists. Not usable as-is post-merge (the file is already at the destination, put there by git). | `src/librarian/tools/mv.rs:15` |
| Events, links, augmentation, observations all `REFERENCES artifact(id) ON DELETE CASCADE`. Deleting a row cascade-deletes its history. | `src/librarian/catalog/migrate_v6.rs:489`, `:498` |
| `artifact_link` is directional: `(src_id, dst_id, rel, created_at)`, `INSERT OR IGNORE` (unique triple). | `src/librarian/catalog/links.rs:9`, `:23` |
| `doctor` is a detect-then-opt-in-fix tool: read-only `scan_*` checks emitting `Violation`s, plus `run_fix(ctx, fix, root)` (currently `fix=prune_missing`). | `src/librarian/tools/doctor.rs:88`, `:149` |

## The two collision surfaces

The merge problem splits cleanly along the git/DB line. The skill treats the
halves differently.

| Surface | Entries live in | Git sees it? | Handling |
|---|---|---|---|
| **Prose body** (F-N written into the markdown body via `edit_markdown`, e.g. session-logs) | `.md` file body | **Yes** — merge conflicts or interleaves | **Delegate** to the R-3 two-dot-diff disposition (`reconnaissance-patterns.md` R-3). Not reinvented here. |
| **Params table** (F-N/T-N in `params`, e.g. `tool-usage-patterns.md`) | catalog DB (`artifact_augmentation.params`) | **No** — git merges the body while params silently fork | **Owned** here: detect → graft → renumber + citation rewrite. |

The params surface is the novel value: it is the only place where *nothing but
codescout tooling can even detect the conflict*, and where "let git handle it"
fails silently.

## Scope & non-goals

**In scope:**
- Detecting worktree-scoped catalog rows and classifying no-collision vs collision.
- Auto-reseating no-collision rows to their `master` path (catalog re-point).
- Grafting collision rows (events + links + params) into the surviving row.
- A skill orchestrating the above, gated on the rebase invariant, with a
  growing conflict cookbook.

**Non-goals:**
- The git content merge itself (branch finishing, test-gating, the merge/rebase
  mechanics). Owned by `finishing-a-development-branch` + R-3.
- Prose-body F-N reconciliation (R-3 owns it).
- Rewriting historical commit messages (immutable; see Citation rewrite scope).
- Cross-database import. Not needed — the catalog is machine-global (Verified
  Facts). A per-worktree `$LIBRARIAN_DB` override is explicitly **unsupported**
  by this flow (see Preconditions).

## Architecture

**Two repos, by necessity:**
- **Tools → `codescout`** — identity/catalog mechanics can only live where the
  DB is.
- **Skill → `codescout-companion`** — orchestration is a prompt artifact,
  sibling to `reconnaissance` and `tracker-hygiene`.

The skill **composes with** `finishing-a-development-branch` — it runs *before*
that skill's Step 6 (`git worktree remove`), the point after which orphaned rows
are unrecoverable. It does not wrap or replace it.

**Version coupling:** the skill depends on a `codescout` build shipping `graft`
+ the new `doctor` check. It probes for the capability first (e.g. an
`artifact(action="graft")` dry-run or a schema check) and, if absent, instructs
the user to update codescout rather than failing opaquely.

## Tool surface

### Tool 1 — `doctor` check `worktree_scoped_row` + `fix=reseat_worktree`

**Read-only check** (added to the `scan_*` set in `doctor::call`,
`doctor.rs:88`): for each managed root, enumerate its linked worktrees
(`git worktree list`, or a filesystem scan confirming `is_linked_worktree`),
then flag every catalog row whose `abs_path` is under a worktree root. For each
flagged row, compute the **would-be main path**:

```
worktree root  = /repo/.worktrees/<name>            (has .git file → gitdir: /repo/.git/worktrees/<name>)
main root      = /repo                               (derived from the gitdir pointer)
main path      = main_root + (row.abs_path relative to worktree root)
```

Classify each flagged row:
- **no-collision** — no catalog row exists at the main path's id.
- **collision** — a row already exists at the main path. If **both** rows are
  augmented with an `entry_collection`, additionally emit the **ID-overlap
  diff**: each side's id set, the overlap, and per-overlapping-entry content +
  timestamp + origin.

`Violation` gains (or carries in `detail`) the computed main path and the
classification so the skill can act without re-deriving.

**`fix=reseat_worktree`** (added to `run_fix`, `doctor.rs:149`): for
**no-collision** rows only, re-point the catalog row's `abs_path` to the main
path **without a filesystem rename** (git already placed the file there; this is
the gap `artifact(move)` cannot fill — it renames and errors if the destination
exists). Collisions are left untouched, reported with a `use graft` pointer.
This is the fully-automatic common case.

### Tool 2 — `artifact(action="graft", from_id, into_id)`

Added to the action enum (`artifact.rs:33`) and dispatch (`artifact.rs:200`).
Folds `from_id` into `into_id` in **one transaction**, in this **mandatory
order** (delete-last, because of `ON DELETE CASCADE`):

1. **Re-point events:** `UPDATE artifact_event SET artifact_id = into_id WHERE artifact_id = from_id`.
2. **Re-point links, both directions**, with dedup against the unique
   `(src_id, dst_id, rel)` triple: re-point `src_id` and `dst_id` where the
   result does not collide with an existing edge; drop (or `UPDATE OR IGNORE`)
   where it would. Self-edges (`from_id → from_id`) collapse to
   `into_id → into_id`.
3. **Merge params:** if both rows are augmented with the same
   `entry_collection`, append `from_id`'s entries into `into_id`'s collection,
   **auto-renumbering** the incoming (worktree) side's colliding ids to continue
   after `into_id`'s max (reusing `next_index`,
   `augmentation.rs:255`). Migrate the augmentation only if `into_id` has none.
4. **Delete `from_id`** — now safe; its history has been re-pointed off it.

**Returns:** the ID remap (`{"F-8":"F-13", …}`), a `suspicious` list (incoming
entries whose content/timestamp closely match a surviving entry — candidate
"same finding discovered twice," which should be *merged*, not renumbered — see
cookbook archetype 3), and counts (events re-pointed, links re-pointed/dropped,
entries merged/renumbered).

**Does not** touch `into_id`'s body, title, status, tags, or non-colliding
params. Content disposition is settled upstream (R-3) before `graft` runs.

### Citation rewrite — lives in the skill, not a tool

Using the remap `graft` returns, the skill greps the merged tree and rewrites
`F-8`→`F-13` in tracker bodies / docs / other markdown, then drops a provenance
breadcrumb on each renumbered entry:
`F-13 (was F-8 on worktree <name>, commit <sha>)`.

**Scope limitation (stated, not hidden):** the rewrite covers the **live tree
only**. Commit messages already merged to `master` are immutable and are **not**
rewritten — the breadcrumb is the durable link back to the original id. Any
claim of a "total" citation rewrite would be false.

## The skill

**Name (proposed):** `worktree-tracker-merge` (codescout-companion).

**Trigger:** fires on "about to merge / finish / clean up a git worktree
branch," co-firing with `finishing-a-development-branch`, positioned before
worktree removal. Seed language: *"before merging or removing a git worktree,
before `git worktree remove`, reconciling trackers across a worktree merge."*
Trigger string gets its own eval (see Testing) — moment-recognition is the whole
game; a safety net that misfires the moment is worse than none.

**Procedure (8 steps):**
1. **Scan** — `doctor` read-only: enumerate worktree-scoped rows, classify,
   show ID-overlap diffs.
2. **Rebase-invariant gate** — check `merge-base(worktree, master) == master HEAD`.
   - **Linear descendant (rebased)** → "worktree entries are chronologically
     last" holds → deterministic auto path enabled.
   - **Divergent (not rebased)** → ordering is ambiguous → auto-renumber
     **disabled**; route every collision to cookbook judgment.
3. **Auto-reseat** — `doctor fix=reseat_worktree` for no-collision rows; report
   what relocated.
4. **Content disposition (delegate)** — for collisions, apply **R-3** two-dot
   diff to settle which *file* survives. Link out; do not reinvent.
5. **Graft** — `artifact(graft, from=<worktree_id>, into=<survivor_id>)` per
   collision; capture remap + `suspicious`.
6. **Judgment via cookbook** — for each `suspicious` entry, consult the cookbook
   archetype: merge-into-one vs keep-renumbered.
7. **Citation rewrite** — rewrite remapped ids in the live tree, drop
   breadcrumbs.
8. **Verify + log** — re-run `doctor`, assert zero worktree-scoped rows remain
   and event counts are preserved (evidence before the "done" claim); append the
   worked example to the project ledger.

## Collision handling

**Renumber policy (deterministic path only):** the surviving (base/`master`)
side keeps its ids; the incoming (worktree) side's colliding ids renumber to
continue after the base's max. This is **chronologically grounded** by the
rebase-before-merge workflow (rebase replays worktree commits onto `master`'s
tip, so worktree entries genuinely are later) — not an arbitrary convention. The
rebase-invariant gate (step 2) is what makes this true; without it the path is
disabled.

**Conflict cookbook** (`references/conflict-archetypes.md`, beside the skill —
the "guide + growing list of examples"). Seeded with real cases:

| # | Archetype | Resolution |
|---|---|---|
| 1 | Worktree-born, no collision | `reseat` (auto) |
| 2 | Independent same-id, **distinct** findings | `graft` auto-renumber + rewrite + breadcrumb (deterministic path) |
| 3 | Same finding discovered **twice** (near-dup across sides) | **merge into one entry**, do not renumber |
| 4 | Prose-body collision (F-N in body) | delegate to R-3 |
| 5 | Citation in historical commit message | breadcrumb only (immutable) |

Archetype 3 is not hypothetical: this session found
`placement-refactor-session-log.md` carrying an `F-10` written by a
*concurrent, `master`-rooted* session describing the **same** loader mis-bucket
bug the spike tracker's `F-10` covered. A blind auto-renumber would wrongly split
one finding into two — which is exactly why the `suspicious` list and cookbook
judgment step exist.

**Accrual + promotion** (mirrors `reconnaissance-patterns`): per-incident worked
examples accrue in a project-level ledger (`docs/trackers/worktree-merge-log.md`,
in whichever repo the merge happens); recurring archetypes get promoted
project-ledger → skill-cookbook at a threshold, via the same manual, human-gated
sync flow.

## Preconditions & assumptions

- **Single shared catalog.** Worktree and main share one machine-global catalog
  (`$LIBRARIAN_DB` unset or identical across both). A per-worktree
  `$LIBRARIAN_DB` override would turn this into a cross-DB import problem and is
  explicitly unsupported.
- **Rebase-before-merge** is the workflow the deterministic renumber path
  assumes. The rebase-invariant gate enforces the precondition; divergent merges
  fall back to cookbook judgment rather than silently mis-ordering.
- **Content settled before catalog.** `graft`/`reseat` run *after* the file
  content is merged (git + R-3). They never substitute for content disposition.

## Error handling & edge cases

- **`graft` transactionality:** the four steps run in one `IMMEDIATE`
  transaction; any failure rolls back — never a half-grafted row.
- **Link dedup:** re-pointing that would violate the unique `(src_id, dst_id,
  rel)` triple drops the redundant edge (or `UPDATE OR IGNORE`); reported in the
  counts.
- **Only one side augmented:** if `from_id` has an augmentation and `into_id`
  does not, migrate it wholesale (no renumber needed); if `into_id` has one and
  `from_id` does not, params step is a no-op.
- **`from_id` file still present:** `graft` is a catalog op; it does not require
  the file to be gone, but the skill runs it only after content disposition. A
  `doctor`-detected worktree row whose file still exists in an *un-removed*
  worktree is reported, not auto-reseated, until the merge is actually done.
- **Capability absent:** skill probes for `graft`; if the codescout build
  predates it, instruct the user to update rather than partial-running.

## Testing strategy

- **`graft` unit tests:** events re-pointed; links re-pointed both directions
  with dedup; self-edge collapse; params merged with correct renumber; cascade
  ordering (assert history survives — the delete-last invariant); rollback on
  mid-transaction failure; `suspicious` detection on near-dup entries.
- **`doctor` check tests:** worktree-scoped detection via `is_linked_worktree`;
  main-path derivation; no-collision vs collision classification; ID-overlap
  diff; `fix=reseat_worktree` re-points without rename and leaves collisions
  untouched.
- **Skill trigger eval:** a `docs/evals/worktree-merge-trigger.md` scored set
  (mirror `reconnaissance-trigger.md`), baseline pinned before any description
  change.
- **Skill behavioral eval (later):** does a triggered run produce a correct
  reconciliation on a seeded two-row collision fixture? Mutation check: feed a
  near-dup collision and confirm the run routes it to merge, not renumber.

## Open questions / deferred

- Exact `Violation` shape for carrying the main path + classification (new field
  vs structured `detail`) — implementation detail, decided at plan time.
- Whether the rebase-invariant gate should also handle the "worktree merged via
  non-fast-forward but still linear" edge — likely folds into the same
  `merge-base == HEAD` check; confirm at plan time.
- Whether `reseat` should be a `doctor` fix (chosen here, matches the
  detect+fix pattern) vs a first-class `artifact` action — revisit if a second
  non-doctor caller emerges (rule-of-three).
