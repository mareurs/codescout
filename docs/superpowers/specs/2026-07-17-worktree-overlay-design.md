# Worktree overlay for the librarian catalog

**Date:** 2026-07-17
**Status:** draft — pending user review
**Supersedes (partially):** `2026-07-10-worktree-merge-tracker-safety-design.md`
(that design's tools survive as merge plumbing; its post-hoc *flow* is replaced)
**Session log:** `docs/trackers/worktree-overlay-session-log.md` (F-1, F-2, W-1)

## Summary

Worktree sessions today are **blind, not isolated**: `scope=project` is a
path-prefix filter on the session root, so from inside a linked worktree
`artifact(find)` sees none of the main repo's trackers, and any explicit write
creates a catalog row at the worktree path with **no recorded relationship** to
its main-path twin. Merge is post-hoc repair: `doctor` guesses lineage from
path arithmetic, and the guess breaks the moment `git worktree remove` runs.

This design replaces guessing with recording, git-style:

- **Overlay reads** — a worktree session sees all main-repo artifacts live;
  shadowed artifacts show the worktree's version.
- **Fork-on-first-write** — the first mutating action against a main-root
  artifact forks it into a shadow row at the worktree path, records a
  `worktree_of` lineage link plus a `worktree_fork` event carrying the base
  snapshot, and registers the worktree durably in the catalog.
- **First-class merge** — `librarian(action="merge_worktree")` extracts each
  shadow's delta against its recorded base and folds *only the delta* onto the
  main row through the existing graft primitives, deterministically renumbering
  colliding entry ids and reporting (never silently resolving) conflicts.

Everything lives in the existing machine-global catalog
(`~/.local/share/librarian/catalog.db`). No satellite DBs, no clone step, no
change to artifact identity (`id = sha256(abs_path)`).

## Motivation — what breaks today

1. **No visibility:** a worktree session cannot query the bug ledger, open
   trackers, or specs (`apply_scope` path-prefix, `src/librarian/tools/scope.rs:54`).
   Agents working in worktrees lose the project's institutional memory.
2. **Accidental divergence:** `append_entry` from a worktree writes a
   worktree-path row; the main tracker's DB-only `entry_collection` (the
   git-invisible state) silently forks with no lineage recorded.
3. **Fragile merge:** the shipped doctor→reseat/graft flow infers lineage at
   merge time; ordering is load-bearing (`git worktree remove` before
   reconciliation orphans the rows beyond recognition — memory
   `worktree-merge-catalog-reconciliation`).
4. **F-N numbering races:** main and worktree can both mint `F-12`; nothing
   detects it until doctor runs.

## Verified facts (grounded this session, 2026-07-17)

- Catalog is machine-global: `~/.local/share/librarian/catalog.db`
  (`src/librarian/mod.rs:357-362`); artifact identity is
  `id = artifact_id_from_abs(abs_path)`.
- The indexer refuses to walk linked worktrees (`src/librarian/indexer.rs:66-77`),
  so shadow rows can only be created by explicit artifact ops — and the
  `artifact::upsert` pre-clean hazard (`DELETE ... WHERE abs_path=? AND id != ?`,
  documented at `src/librarian/tools/doctor.rs:55-58`) never fires on shadow
  paths via reindex.
- `CurrentProject::resolve` does **not** redirect worktrees
  (`src/librarian/current_project.rs:28-37`); `is_linked_worktree` (:56) and
  `worktree_main_root` (:85) exist and are filesystem-only.
- `scope=project|repo` are path-prefix clauses; `umbrella` ORs member prefixes
  (`src/librarian/tools/scope.rs:54-123`). Umbrella lookup keys on the session
  path, so it fails from outside-root worktrees today.
- `LinkRow` is `{src_id, dst_id, rel, created_at}` — **no payload column**
  (`src/librarian/catalog/links.rs:8-13`). (F-1)
- `merge_augmentation` does **collision-only** renumbering and only *flags*
  content near-dups as `suspicious` (`src/librarian/catalog/graft.rs:179-320`).
  Folding a base-seeded shadow through bare `graft_rows` would duplicate every
  base entry. (F-2)
- There is no single write choke point: each mutating action handler goes
  straight from args to catalog (e.g. `src/librarian/tools/append_entry.rs:20-31`).

## Scope & non-goals

**In scope:**
- Durable worktree registration + shadow-row lineage recorded at write time.
- Overlay read semantics for worktree sessions; shadow exclusion for main sessions.
- Fork-on-first-write for mutating artifact actions targeting main-root artifacts.
- First-class `merge_worktree` (merge / abandon / dry-run) built on graft
  primitives with delta extraction.
- Doctor reclassification: registered shadow rows are pending-merge info, not
  violations; unregistered legacy rows keep the existing reseat/graft flow.

**Non-goals:**
- Prose-body merging — `.md` bodies are git's job (unchanged from July design).
- Cross-repo isolation: writes from a worktree session to artifacts of *other*
  repos (umbrella peers) are NOT isolated in v1 — fork-on-first-write applies
  only to targets under the session's own `main_root`. Documented limitation.
- Eager registration hooks (EnterWorktree / superpowers skill integration).
  Lazy registration covers worktrees created by any tool; hooks can be layered
  later without schema changes.
- Memory-tool and semantic-index isolation (per-project files; git handles them).
- Rewriting historical commit messages (citation rewrite stays live-tree-only,
  owned by the existing skill step).

## Data model

All in the existing global DB. One new table; no changes to existing tables.

### 1. `worktree_registration` (new table, schema migration)

| column | type | notes |
|---|---|---|
| `worktree_root` | TEXT PK | canonical abs path of the linked worktree |
| `main_root` | TEXT | canonical abs path of the main checkout |
| `branch` | TEXT | branch checked out at registration time (best-effort) |
| `created_at` | INTEGER | epoch ms |
| `status` | TEXT | `active` \| `merged` \| `abandoned` |
| `closed_at` | INTEGER NULL | set on merge/abandon |

Created lazily by the first fork (never by reads). **Survives
`git worktree remove`** — after removal, `is_linked_worktree` is useless but the
registration still maps shadow paths to `main_root`, so merge remains exact.
This kills the July design's load-bearing ordering constraint.

### 2. Shadow rows

Plain artifact rows at the worktree abs path (`worktree_root` +
`rel(main_root, main_abs_path)`), id = `sha256(shadow_path)` — identity model
untouched. Seeded at fork with a copy of the main row's catalog fields
(kind/title/status/tags/frontmatter) **and** its augmentation params, so the
worktree's overlay view of a forked tracker is self-contained.

### 3. Lineage: `worktree_of` link + `worktree_fork` event

- **Link** `rel="worktree_of"` (shadow → main): bare traversal edge for
  `graph`/`get(include_links)`. Carries no data (F-1: `LinkRow` has no payload).
- **Event** `kind="worktree_fork"` on the shadow row, payload:

  ```json
  {
    "main_id": "<id of the main twin>",
    "branch": "<branch>",
    "base_event_seq": <main row's max event seq at fork>,
    "base_params": { ...full params snapshot at fork... },
    "base_frontmatter": { "status": "...", "title": "...", "tags": [...] }
  }
  ```

  Events carry JSON payloads and are already re-pointed atomically by
  `graft_rows`, so the base cursor survives every later operation. The full
  `base_params` snapshot (not a hash) is what makes merge-time delta extraction
  and three-way scalar comparison **exact** instead of heuristic (F-2 workaround).

New artifacts born in the worktree have no link/fork event; merge classifies
"row under a registered `worktree_root`, no `worktree_of` edge" as *new* and
reuses the existing reseat machinery.

## Read path — overlay

- `CurrentProject` gains `main_root: Option<PathBuf>`, populated via the
  existing `worktree_main_root()` when the session root is a linked worktree.
  Umbrella lookup resolves via `main_root` when present — `scope=umbrella`
  starts working from worktrees.
- `apply_scope` from a worktree session: `Project`/`Repo` scopes emit
  `prefix(worktree_root) OR prefix(main_root)`.
- **Overlay dedup in `find`:** post-query pass (worktree sessions only) —
  fetch `worktree_of` edges for result ids; where a shadow and its main twin
  both matched, drop the main twin and annotate the shadow `"overlay": true`.
- **Main-session shadow exclusion:** for the in-repo layout
  (`<main>/.worktrees/<name>/`) the main prefix matches shadow paths, so
  `find` from a non-worktree session appends `AND NOT prefix(worktree_root)`
  for each `active` registration whose `worktree_root` falls under the scope
  prefix. Registrations are few; the clause is cheap.
- `get(id)` stays id-literal (ids are global). `get` on a main id from a
  worktree session that has a shadow for it returns the main row plus an
  `overlay_hint: {shadow_id}` so the model knows a fork exists. Reads never
  redirect; only writes do.

## Write path — fork-on-first-write

New shared helper in `librarian/tools`:

```text
resolve_write_target(ctx, target_id) -> target_id'
```

Called at the top of every mutating action handler — `append_entry`, `update`,
`artifact_event`, `artifact_augment`, `link` (when `src_id` resolves under
`main_root`), `commit_refresh`. Behavior when the session is a worktree AND the
target row's abs_path is under `main_root` (and not under `worktree_root`):

1. Upsert the `worktree_registration` (status `active`).
2. Compute shadow path/id; if the shadow row does not exist: create it seeded
   from the main row, write the `worktree_fork` event (base payload above),
   write the `worktree_of` link. Single transaction.
3. Return the shadow id; the action proceeds against the shadow.

Non-worktree sessions, worktree-born targets, and cross-repo targets pass
through unchanged. `delete` and `move` targeting main-root artifacts from a
worktree session are **refused** with a `RecoverableError` ("merge or perform
from the main checkout") — v1 keeps destructive structure changes out of the
overlay.

`create` from a worktree needs no interception: `rel_path` resolves under the
worktree root naturally; registration still happens (step 1) so merge can find
the row.

## Merge — `librarian(action="merge_worktree")`

Resolves the July design's open question (doctor-fix vs first-class action):
the second caller has emerged, so merge is porcelain; `reseat`/`graft` are
plumbing.

**Signature:** `librarian(action="merge_worktree", root=<worktree_root>,
dry_run?, abandon?)`.

**Preconditions:**
- An `active` registration for `root` (else: RecoverableError pointing at
  `doctor` for unregistered legacy rows).
- If the branch still exists in git: the rebase invariant from the July skill
  (worktree branch fully rebased onto main HEAD before content merge). If the
  worktree directory is already gone, the git check is skipped — the DB state
  is self-sufficient.

**Per shadow row with a `worktree_of` edge (single IMMEDIATE transaction,
graft-style):**

1. **Delta extraction (the F-2 invariant).** From the `worktree_fork` payload:
   - *Appended entries* = entries in the shadow's `entry_collection` whose ids
     are not in `base_params`' id set.
   - *Edited base entries* = entries whose id is in the base set but whose
     content (minus id) differs from base.
   - *Scalar params / frontmatter deltas* = keys whose shadow value differs
     from base.
   **Bare `graft_rows` on a seeded shadow is never valid** — it would collide
   every base entry and re-append it as a duplicate
   (`src/librarian/catalog/graft.rs:179-320` contract; F-2).
2. **Fold appended entries** through graft's id allocator: collision-only
   renumber against the main row's live entries, deterministic, remap recorded.
3. **Three-way the rest:** edited base entries, scalar params, and frontmatter
   apply automatically iff main's value is unchanged since base; otherwise the
   field lands in the conflict report with both values — never silently resolved.
4. **Re-point history:** events with seq > `base_event_seq`, plus links added
   on the shadow, re-point to the main id via the existing graft primitives
   (unique-key collisions dropped, as today).
5. **Audit event:** write a `worktree_merge` event on the main row with payload
   = {branch, remap, conflicts, counts}. Conflicted worktree values are
   preserved in this payload, so nothing is ever lost even when not applied.
6. Delete the shadow row (cascade cleans its augmentation).

**Rows without an edge** (new in worktree): reseat to the main path via the
existing seed-and-graft machinery (`doctor.rs` reseat path, extracted for reuse).

**Completion:** registration → `merged`; return a MergeReport
`{per_artifact: remap/conflicts/suspicious, reseated: [...], skipped: [...]}`.
The existing citation-rewrite skill step consumes the remap table unchanged.

**`abandon=true`:** delete all shadow rows under `root` + mark registration
`abandoned`. **`dry_run=true`:** full report, no writes.

## Interplay with existing pieces

| Piece | Change |
|---|---|
| Indexer worktree skip | unchanged |
| `doctor` `worktree_scoped_row` | reclassifies: rows under an `active` registration → `pending_merge` info; unregistered rows → violation with today's reseat/graft guidance |
| `doctor` `prune_missing` | **guard added:** refuses a root covered by an `active` registration (hint: `merge_worktree` or `abandon`) — prevents deleting unmerged shadows after `git worktree remove` |
| `reseat_worktree` / `graft` | demoted to plumbing; remain available for legacy/unregistered rows |
| Citation rewrite | unchanged; consumes MergeReport remap |
| Memory `worktree-merge-catalog-reconciliation` | update after ship: overlay flow is primary, old flow is the legacy fallback |

## Error handling & edge cases

- **`git worktree remove` before merge:** registration + shadows persist; merge
  works entirely from DB state. The July hazard disappears.
- **Same repo, multiple worktrees:** independent registrations; same artifact
  forked in two worktrees merges sequentially — each merge three-ways against
  its own base; overlapping scalar edits surface as conflicts by design.
- **Overlay staleness asymmetry (documented behavior):** a worktree session
  sees live main rows for everything *except* artifacts it forked — those show
  the frozen base + its own delta until merge. Main-side appends to a forked
  tracker surface at merge as the renumber/conflict pass.
- **Concurrent processes:** row-disjoint by construction; SQLite `busy_timeout`
  already covers cross-process writers (`src/librarian/catalog/mod.rs`).
- **Fork of an unaugmented artifact:** `base_params` is `null`; delta extraction
  degenerates to "everything on the shadow is new" — same code path.
- **Giant params snapshots:** `base_params` duplicates a potentially large JSON
  blob once per forked artifact per worktree; bounded and internal
  (server-side, never round-tripped through the model). Revisit only if real
  usage shows bloat (open question 2).

## Testing strategy

Unit (all against temp-path `Catalog::open`, per existing test idiom):
- `resolve_write_target`: forks exactly once; seeds params + fork event + link
  atomically; passes through non-worktree sessions, worktree-born and
  cross-repo targets; refuses delete/move.
- Overlay: find dedup (shadow wins, `overlay: true`), main-session exclusion
  for in-root worktrees, `get` overlay_hint, umbrella-via-main_root.
- **F-2 regression (named invariant test):** fork-seed → append both sides →
  merge → assert no duplicate ids AND no duplicated content AND main-side
  appends preserved.
- Renumber determinism; three-way scalar matrix (worktree-only change / main-only
  / both-changed → conflict); edited-base-entry matrix; unaugmented fork.
- Doctor reclassification + `prune_missing` guard; abandon; dry-run writes nothing.

Integration (three-query sandwich style): create tracker on main → activate
worktree session → verify overlay read + isolation both directions →
`append_entry` × both sides → `merge_worktree` → verify entries + events under
the main id, remap correct, doctor clean — with a variant running
`git worktree remove` before the merge.

## Open questions / deferred

1. **Registration `branch` freshness** — branch recorded at first write may go
   stale if the worktree switches branches; merge should re-read from git when
   available. Decide at plan time whether to store or always re-derive.
2. **`base_params` size cap** — full snapshot chosen for exactness; if a real
   tracker's params exceed a sane bound (~1 MB), consider entry-id-set +
   per-key hashes. Deferred until observed.
3. **`link` action interception scope** — whether links whose `dst_id` (not
   just `src_id`) is a main-root artifact also need forking, or whether a
   worktree-born link to a live main row is acceptable (it survives merge
   re-pointing either way). Decide at plan time with a test either way.
4. **EnterWorktree / superpowers-skill eager hook** — layering a creation-time
   registration on top of lazy is additive; revisit after v1 ships.
