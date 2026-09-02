# Catalog GC &amp; Repair

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

The artifact catalog tolerates files disappearing. A row whose file is gone is
stamped, not dropped — and if the file comes back, the row does too.

## Why a grace period

The catalog is keyed by absolute path, and paths move for reasons that have
nothing to do with an artifact being deleted: a branch checkout, a rebase, a
worktree pruned, a rename in progress. Dropping a row the moment its file is
missing loses that artifact's entire history — events, observations, links,
augmentation params — for what is usually a transient absence.

So the catalog records *when* a file went missing (`missing_since`, schema v10)
and defers the consequences.

## What the stamp changes

| Surface | Behaviour once past the grace window |
|---|---|
| `doc(action="find")` | Row is hidden (listing and semantic search) |
| `doc(action="get")` | Still returns it — an explicit id is an explicit request |
| `librarian(action="doctor")` | Still reports it, and counts it |

Hiding is deliberately confined to the surfaces an agent browses. A stale row
should stop polluting "what's live right now?" without becoming unreachable to
someone who holds its id.

An existence-based reconciliation pass (`reconcile_missing_since`) stamps rows
whose files vanished and *clears the stamp* on rows whose files returned. It
runs throttled and best-effort on ordinary librarian calls, so a catalog that
drifted heals without anyone remembering to run a repair.

## `doctor`'s health block

```text
librarian(action="doctor")
```

The report carries a `catalog_health` block:

- `hidden_rows` — how many rows are currently suppressed from `find`;
- `move_candidates` — dead rows that look like moves rather than deletions;
- `move_candidates_detail` — the evidence, per candidate;
- `hint` — which fix to reach for.

A move candidate is detected by **commit-hash overlap**: if a dead row and a
live row share git history, the file was moved, and pruning the dead row would
throw away history that has a valid new home.

## Repair modes

All three are dry-run by default and require `confirm=true` to write.

```text
librarian(action="doctor", fix="prune_missing", root="/abs/dead/root", confirm=true)
```

Batch-drops rows under a root that is genuinely gone. The request is validated
first: the root must be absolute, and it must actually be dead — pointing this
at a live root is refused rather than executed.

```text
librarian(action="doctor", fix="rehome", root="/abs/old", new_root="/abs/new", confirm=true)
```

The move case. Rewrites the ids of every row under `root` to the ids they would
have had under `new_root`, **preserving all child rows** — events,
observations, links, and augmentation follow the artifact rather than being
cascade-deleted. This is why it exists as its own mode instead of
prune-then-reindex: a prune drops the children, and reindex cannot invent them
back.

```text
librarian(action="doctor", fix="reseat_worktree", confirm=true)
```

Re-points worktree-scoped rows at their main-checkout paths. Legacy — see
[Worktree Overlay](worktree-overlay.md) for when this applies and when
`merge_worktree` is the right call instead.

## Where this lives

`src/librarian/catalog/gc.rs` (the `catalog_meta` accessors and grace/cutoff
helpers), the v10 migration, and `src/librarian/tools/doctor.rs` for the checks
and fixes.

## Related

- [Worktree Overlay](worktree-overlay.md)
- [Librarian](librarian-embedded.md)
