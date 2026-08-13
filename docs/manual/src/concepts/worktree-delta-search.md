# Worktree Delta Search

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

`semantic_search` works inside a linked git worktree. It serves the main
checkout's vectors for every file that is byte-identical between the two
trees, and a small per-worktree delta index for the files that differ,
merging the two result sets by score.

## Can I use this today?

Yes, on `experiments`, with one manual step: **you must run
`index(action="build")` inside the worktree yourself.** Nothing triggers it
for you yet — there is no hook or auto-index path that notices you switched
into a worktree and builds the delta on your behalf. Until you run it once,
`semantic_search` from the worktree returns a "not yet indexed" hint instead
of results.

```
workspace(action="activate", path="/repo/.worktrees/feat")
index(action="build")
semantic_search(query="...")
```

Re-run `index(action="build")` after editing files in the worktree, the same
way you would in a normal checkout — it only re-embeds what changed.

## Why content hash, not git diff

The delta is derived by comparing `(file_path, content_hash)` pairs between
main's already-indexed chunks and the worktree's files on disk
(`dirty_paths` in `src/retrieval/drift.rs`) — not by diffing against a base
commit. A path is dirty when either side has a `(file_path, content_hash)`
pair the other side lacks: added, modified, or deleted content all show up
the same way.

This sidesteps two problems a commit-diff approach would have: there is no
single base commit to pick for an arbitrary worktree, and there is no
staleness window during which the diff and the index disagree — the
comparison is always against main's actual indexed content, not a
point-in-time ref.

## The delta index

`index(action="build")` is the only thing that ever writes a worktree delta
(`sync_worktree` in `src/retrieval/sync.rs`) — `semantic_search` only reads.
It reuses main's vectors for byte-identical files and embeds only the files
`dirty_paths` marked dirty, storing them under a delta project id built as
`{main_project_id}@{worktree_dir}` (`delta_project_id`, `src/retrieval/sync.rs`).
The `@` separator keeps the id distinct from `chunk_id`'s own `:`-joined
format and from library ids (`lib:<name>`).

## The merged query

A `semantic_search` call from inside the worktree, with no explicit
`project_id`, queries **both** projects and merges the hits by score:

- **Main** is queried with the dirty paths excluded (`exclude_paths` on
  `SearchOpts` — a Qdrant `must_not` filter server-side; the sqlite-vec lite
  store post-filters and widens `k` to compensate), so it never serves a
  chunk the worktree has changed.
- **The delta** is queried for everything it has, unfiltered.

## Response fields

The response can carry up to three extra, purely informational string
fields — none of them is an error, and the query still ran:

- **`drift_note`** — main's own index was rebuilt *after* this worktree's
  delta, so unchanged-file results may reflect main's newer content. Re-run
  `index(action="build")` in the worktree to refresh.
- **`worktree_state_warning`** — the delta has chunks but no dirty paths are
  recorded, an inconsistent state that should never happen in a healthy
  worktree (a delta only exists because something was dirty). Main was
  queried with no exclusions and may be serving stale chunks. Re-run
  `index(action="build")` in the worktree to repair the record.
- **`main_never_indexed_note`** — main itself has no indexed chunks at all,
  so every result comes only from the worktree's own delta.

See the `semantic_search` tool's own extended description (its `long_docs`,
surfaced by MCP clients that expose per-tool detailed help) or
[Semantic Search Tools](../tools/semantic-search.md) for the exact field
list every result carries.

## What this does not cover yet

- **No automatic trigger.** Nothing runs `index(action="build")` for you on
  entering a worktree — see *Can I use this today?* above.
- **Single collection, basename-keyed.** `worktree_dir` must be a directory
  basename, not a path; two worktrees sharing a basename under different
  parents are not distinguished.
- **Read-only from `semantic_search`.** Only `index` produces a delta —
  calling `semantic_search` never writes one, even implicitly.

## Related

- [Git Worktrees](worktrees.md) — the write guard and `workspace(action:
  activate)` dance this feature builds on
- [Worktree Overlay](worktree-overlay.md) — the analogous mechanism for the
  artifact catalog
- [Semantic Search](semantic-search.md) — chunking, embedding, and scoring
  fundamentals
- [Semantic Search Tools](../tools/semantic-search.md) — full tool reference
