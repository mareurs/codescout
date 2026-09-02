# Worktree Overlay

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

A codescout session running from a linked git worktree shares the main
checkout's artifact catalog, and forks individual artifacts only when it writes
to them.

## Why overlay instead of a separate catalog

The catalog keys artifacts by absolute path (`id = sha256(abs_path)`), so the
same file at `/repo/docs/trackers/foo.md` and
`/repo/.worktrees/feat/docs/trackers/foo.md` are two different ids. There were
two ways out: give each worktree its own catalog, or overlay one onto the other.

A separate catalog means a branch session cannot see any tracker the main
checkout owns — every worktree session would start blind, which is exactly the
problem the catalog exists to solve. Overlay keeps reads unified and pays the
cost of divergence only where a session actually diverges.

## Reads: shadow wins

A worktree session's `find` and `get` return main-repo artifacts live. Once the
session has forked an artifact, rows exist on both sides for the same lineage;
the dedup keeps the shadow and annotates it `"overlay": true`, so the agent can
tell which copy it received. Every session excludes *foreign* shadows — one
worktree never sees another's in-progress rows.

## Fork-on-first-write

The first mutating call against a main-root artifact — `append_entry`,
`update`, `event_create`, `augment`, or `link` — forks it, creating:

- a shadow row at the worktree path;
- a `worktree_fork` event carrying the fork-time base params and frontmatter;
- a `worktree_of` lineage link back to the main row.

Every write after that lands on the shadow. `delete` and `move` against a
main-root target are refused from a worktree session — merge first, or act from
the main checkout.

The recorded fork-time base is what makes the merge a delta-fold rather than an
overwrite. Without it, folding a shadow back could only *replace* the main row,
silently discarding whatever the main checkout changed in the meantime.

## Merge

```text
librarian(action="merge_worktree", root="/repo/.worktrees/feat", dry_run=true)
```

Folds each shadow's delta — computed against its own fork-time base — onto the
main twin, reseats rows that were born in the worktree and so have no main twin
to fold onto, and closes the registration. Drop `dry_run` to write.

```text
librarian(action="merge_worktree", root="/repo/.worktrees/feat", abandon=true)
```

Drops the shadows and marks the registration abandoned.

## `doctor`'s worktree checks are now the legacy path

`doctor`'s `worktree_scoped_row` check and `fix="reseat_worktree"` predate the
overlay. They now apply only to worktree-scoped rows with **no active
registration** — pre-overlay drift, or a registration that was lost. A
registered row's finding carries `"registered": true` plus a hint pointing at
`merge_worktree`, and `reseat_worktree` skips those rows rather than reseating
them out from under a live session.

## Where this lives

The `worktree_registration` table and the overlay scope live in
`src/librarian/catalog/`; worktree-aware project resolution is
`CurrentProject.main_root`; the legacy checks are in
`src/librarian/tools/doctor.rs`. The agent-facing summary an LLM receives is
`get_guide("librarian")` § *Worktree overlay* — keep the two in step when either
changes.

## Related

- [Git Worktrees](worktrees.md) — the write guard that predates this
- [Catalog GC &amp; Repair](catalog-gc.md) — `doctor`'s other repair modes
