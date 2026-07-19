# Design — Catalog GC lifecycle + rename/move recovery

**Status:** design approved 2026-07-19 · branch `experiments` · follow-up to
`2026-07-17-catalog-hygiene-prevention-cleanup-design.md`
**Bugs:** [[2026-07-17-catalog-dead-rows-no-gc]] (root cause), pairs with the deferred
"option 1" of [[2026-06-13-catalog-orphans-survive-repo-rename]].

## Problem

The shared global catalog (`~/.local/share/librarian/catalog.db`) has a **detect→repair
loop that never closes**. `doctor` detects `missing_file` rows and `prune_missing` can
delete them, but nothing runs on its own, no read path consults the result, and dead rows
stay `status=active`. So `find` / `semantic_search` / staleness lists / link-graph return
ghosts. The prevention + batch-cleanup work (prior spec) stops *new* dead rows and cleans
the *current* backlog, but the ongoing lifecycle was deferred.

A second, distinct failure shares the same surface: a **renamed/moved repo**. Its files
still exist at a new path, but `id = sha256(abs_path)`, so every row is orphaned and — unless
migrated — its history (`events`, `augmentation`, `links`) is lost when the docs get
re-indexed as brand-new rows at the new path. Auto-migration on rename was the explicitly
deferred "option 1" of the rename/move bug.

### Hard constraint — do not repeat the over-aggression incident

The predecessor auto-GC (`delete_orphan_repos`) was removed because it deleted rows by
**workspace scope** on a **machine-global** catalog: anything not under the *current*
workspace was an "orphan", and an empty roots list ran `DELETE FROM artifact` (total wipe).
Every mechanism here must be **existence-based or identity-based, never scope-based**, and
**no deletion may ever be automatic**.

## Scope

**In scope**

1. `missing_since` timestamping (schema + reconcile pass).
2. Automatic, throttled reconcile on `workspace(activate)`.
3. Hide-from-find after a grace period `N` (reversible; row stays `active`).
4. Surfacing the hidden/candidate counts in `workspace(action="status")`.
5. Rename/move recovery: `doctor(fix="rehome", old_root, new_root)` — id-rewrite migration
   preserving children, with **automatic detection** (surfaced) and **manual, confirmed
   application**.

**Out of scope (dropped by decision, not deferred)**

- **Time-based auto-prune** — deletion stays human-gated forever (`doctor(prune_missing,
  confirm=true)`, already shipped).
- **Auto-apply of re-home** — the id-rewrite always requires `confirm=true`.

**Decisions locked (2026-07-19)**

- Prune autonomy: **auto-hide, never auto-delete.**
- Reconcile trigger: **`workspace(activate)`, throttled (~24h).**
- Re-home: **auto-detect on activate, apply on confirm.**
- Grace period **`N = 14 days`** (matches the CLAUDE.md verify-open cadence), tunable via
  `catalog_meta`.
- Hidden-count surface: **`workspace(status)` only**; SessionStart banner left off (flip via
  a one-line addition if desired).

## Component 1 — Schema (`missing_since` + `catalog_meta`), migration v10

- `ALTER TABLE artifact ADD COLUMN missing_since INTEGER` — nullable, epoch-ms; `NULL` = file
  present / never observed missing.
- New `catalog_meta(key TEXT PRIMARY KEY, value TEXT)` — holds `last_reconcile_at` (throttle)
  and `gc_grace_days` (tunable `N`, default 14).
- Idempotent migration in `apply_migrations_in_txn`, bumps `schema_version` to 10. Must
  survive the "every schema SQL column survives every migration path" regression test.

## Component 2 — Reconcile pass (pure, existence-based, non-destructive)

`reconcile_missing_since(cat) -> ReconcileStats` (new `catalog/gc.rs`):

- Iterate every `artifact` row; `stat(abs_path)`:
  - absent & `missing_since IS NULL` → set `missing_since = now`
  - present & `missing_since IS NOT NULL` → set `missing_since = NULL`
- Returns `{newly_missing, cleared, still_missing}`. **Never deletes. Never scope-based.**
  Idempotent — a second run with no filesystem change is a no-op.

## Component 3 — Automatic trigger (throttled, best-effort)

In `workspace(action="activate")`, after the existing bootstrap:

- Read `catalog_meta.last_reconcile_at`; if `< 24h` ago, skip.
- Otherwise run Component 2 + Component 6 detection, write `last_reconcile_at = now`.
- **Best-effort**: wrap in a catch that logs and swallows — reconcile must never fail or
  measurably slow activate. Time-bounded; unaffected by other-workspace unreachable paths
  (a `stat` of a missing path is cheap).

## Component 4 — Hide-from-find (reversible)

`find.rs` is the single read chokepoint (list, `COUNT(*)`, `GROUP BY kind`, and the
augmentation-staleness subquery). Add to each artifact query:

```sql
AND (missing_since IS NULL OR missing_since > :cutoff)   -- cutoff = now - N days
```

Apply the same predicate to the `semantic_search` catalog-join path. Rows remain
`status=active`; a returning file clears `missing_since` (Component 2) and un-hides
automatically. An **explicit opt-in** (`include_missing=true`, or existing scope escalation)
still surfaces hidden rows for forensics/doctor.

## Component 5 — Surfacing

`workspace(action="status")` gains a `catalog_health` block:

> `{k} rows hidden as missing (>N days); {m} move candidate(s) detected. Run
> doctor(prune_missing) to remove, or doctor(rehome …) to migrate.`

Counts come from a cheap aggregate (no filesystem walk — reads `missing_since`). SessionStart
banner intentionally unchanged.

## Component 6 — Rename/move recovery (`doctor(fix="rehome")`)

**Detection (automatic, on activate — surfaced only).** The active repo's ingested commit
hashes are compared against the `commits` rows of any *missing* `git_root`. **Threshold:** commit
SHAs are globally unique, so **≥1 shared commit hash** already establishes same-repo
identity; the candidate is **ambiguous** (and therefore *not* surfaced as actionable) only
if the active repo's hashes overlap **two or more distinct missing `git_root`s**, or if a
non-git move leaves zero commit rows to match on — those fall back to explicit
`old_root/new_root`. Confirmed by `file_sha256` overlap between the missing artifact rows and
the files now under `new_root`. Candidates are recorded for Component 5. **Never
auto-applied.**

**Application (manual, confirmed).** `doctor(fix="rehome", old_root=<abs>, new_root=<abs>)`:

- **Dry-run (default):** list the mapping, per-table row counts, and any collisions. No
  mutation.
- **Apply (`confirm=true`)**, one atomic vec0-aware transaction:
  1. For each `artifact` row under `old_root`: compute `new_abs_path` (rebase
     `old_root`→`new_root`), `new_id = sha256(new_abs_path)`.
  2. Cascade the id: update `events.artifact_id`, `artifact_augmentation.artifact_id`,
     `artifact_link.src/dst`, `artifact_observation`, `artifact_vec.id`, `entry_cite` slug
     refs, then the `artifact` row (`abs_path`, `id`).
  3. Rewrite `commits.git_root` for the moved root.
- **Guards:** `new_root` must **exist**; `old_root` must **not** exist; both absolute;
  abs_path/id **collisions are skipped, never clobbered** (a reindex-minted row at the new
  path wins; the orphan is reported, left for `prune_missing`). Any error → full rollback.

## Data flow

```
workspace(activate)
  └─(throttled)─▶ reconcile_missing_since        (stamp/clear missing_since)
              └─▶ detect_move_candidates         (commit-hash + file_sha256 overlap)
                     └─▶ catalog_meta / status cache

find / semantic_search ──▶ WHERE missing_since NULL OR > cutoff   (hide ghosts)

workspace(status) ──▶ catalog_health {hidden_k, move_candidates_m}

human ──▶ doctor(prune_missing, confirm)   (delete, existing)
     └──▶ doctor(rehome, old, new, confirm) (migrate + preserve history, new)
```

## Error handling

- Reconcile/detection on activate: best-effort, logged, swallowed — never surfaces as an
  activate failure (`RecoverableError` only inside explicit `doctor` calls).
- `rehome`: `RecoverableError` for guard violations (non-absolute, `new_root` missing,
  `old_root` present, no rows under `old_root`, ambiguous or zero-overlap candidate). Atomic —
  all-or-nothing.

## Testing

- **Reconcile:** stamps newly-missing; clears on return; leaves present rows untouched;
  idempotent.
- **Hide-from-find:** hidden past `N`; visible within grace; un-hides on clear; `COUNT` /
  `GROUP BY kind` / augmentation-staleness all honor the predicate; `include_missing`
  opt-in bypasses it.
- **Throttle:** second activate within window skips reconcile.
- **Re-home:** `abs_path` + `id` rewritten; **child rows follow** (events / augmentation /
  links / observations / vec / entry_cite asserted present under `new_id`); `commits.git_root`
  rewritten; guards (`new_root` missing, `old_root` present, non-absolute) reject; collision
  skipped not clobbered; error → rollback leaves catalog unchanged; idempotent second apply.
- **Detection:** commit-hash overlap ⇒ candidate; unrelated repo ⇒ none; confirmed by
  `file_sha256`.
- **Regression:** empty-roots is still a no-op, never `DELETE FROM artifact`
  (the over-aggression guard).

## Execution / rollout

Code-only on `experiments`; the migration runs on first open of any v9 catalog. No
destructive step ships enabled. Cherry-pick to `master` is the user's call (bugs stay `open`
until then per project convention). No catalog data is deleted by this feature.

## Bug bookkeeping

- Closes the root-cause half of [[2026-07-17-catalog-dead-rows-no-gc]] (the "open loop").
- Delivers the deferred "option 1" of [[2026-06-13-catalog-orphans-survive-repo-rename]]
  (auto-migration on rename) — update/cross-link that bug on ship.
