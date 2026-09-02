# `doc(action="graft")` — Fold One Artifact Into Another

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

```text
doc(action="graft", from_id="<source>", into_id="<destination>")
```

Merges the source artifact into the destination and deletes the source.

## When you need it

Two artifacts turn out to be the same thing. A tracker was created twice under
different names; a session log and its successor both accumulated entries; a
worktree row and a main row diverged past what a merge can reconcile.

`delete` is wrong here, because the source has history — events, observations,
links, augmentation params — that would be cascade-dropped with it. Editing the
destination by hand and then deleting is the same loss with extra steps.

## What it does

- **Re-points children.** The source's events, observations and links are moved
  onto the destination, so nothing is orphaned.
- **Merges augmentation params**, renumbering entry ids that collide so both
  sets survive rather than one overwriting the other.
- **Flags near-duplicates** for review instead of silently deduping them —
  "these two entries look like the same observation" is a judgement call the
  tool declines to make on your behalf.
- **Deletes the source last**, so a failure part-way through does not leave the
  history stranded with no owner.

## Order of operations matters

The source is deleted only after every child row has been re-pointed. That
ordering is the reason to use `graft` rather than a hand-rolled sequence: doing
it the other way round hits the `ON DELETE CASCADE` on the source's children and
destroys exactly what you were trying to preserve.

## Where this lives

`src/librarian/tools/graft.rs` (the tool surface) and
`src/librarian/catalog/graft.rs` (`graft_rows`, the re-pointing and
delete-last ordering).

## Related

- [Librarian](librarian-embedded.md)
- [artifact (action="move")](artifact-move.md) — relocating one artifact, not merging two
