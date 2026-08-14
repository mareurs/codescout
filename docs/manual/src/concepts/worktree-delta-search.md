# Worktree Delta Search

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

`semantic_search` works inside a linked git worktree. It serves the main
checkout's vectors for every file that is byte-identical between the two
trees, and a small per-worktree delta index for the files that differ,
ranking both together as a single result page.

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
`{main_project_id}@{worktree_name}` (`delta_project_id`, `src/retrieval/sync.rs`).
The `@` separator keeps the id distinct from `chunk_id`'s own `:`-joined
format and from library ids (`lib:<name>`).

`{worktree_name}` is **git's own worktree name** — the `<name>` in the
`gitdir: <main>/.git/worktrees/<name>` pointer that `git worktree add` writes
into the worktree's `.git` file — not the checkout directory's basename. Git
keeps that name unique per repository; a basename is not unique, so `/a/wt`
and `/b/wt` of the same repo would otherwise share one delta index, and the
second worktree's sync would prune the first's chunks and then serve them
from the wrong branch with no warning. The basename is used only as a
fallback, when there is no linked-worktree pointer to read
(`worktree_key`, `src/retrieval/sync.rs`).

## The merged query

A `semantic_search` call from inside the worktree, with no explicit
`project_id`, covers **both** projects:

- **Main** contributes everything the worktree has not touched: the dirty
  paths are excluded (`exclude_paths` on `SearchOpts`), so main never serves
  a chunk the worktree has changed — unless a partial sync left
  `dirty_paths` incomplete, see `drift_note` below.
- **The delta** contributes everything it has, unfiltered. It holds exactly
  the dirty files, so excluding those paths from it too would empty it.

The union is expressed **at the vector store**
(`CodeVectorStore::query_overlay`), not composed in the tool layer, and this
is load-bearing rather than tidiness. Whether two result lists can be merged
by score depends on the backend:

- the sqlite-vec lite store scores `1 / (1 + distance)` and the in-memory
  test store scores cosine. Both are absolute functions of content, so
  ranking two lists together is meaningful — those backends satisfy the
  union by querying twice and merging, which is what the trait's default
  implementation does.
- Qdrant with hybrid retrieval on (the default) scores by **Reciprocal Rank
  Fusion**, which depends on rank *position* only — measured as `1/(1 +
  rank)`, the same ladder whether the project holds three chunks or half a
  million. Merging two such lists by score gives the smaller project a fixed
  share of every page no matter how irrelevant its contents are. Qdrant
  therefore answers with **one** query whose filter unions both project ids
  and nests the path exclusion so it binds to main alone.

So the tool layer asks for "these two projects, with these exclusions on the
first" and stays backend-agnostic; each store decides how to satisfy it.

## Response fields

The response can carry up to three extra, purely informational string
fields — none of them is an error, and the query still ran:

- **`drift_note`** — main's own index was rebuilt *after* this worktree's
  delta, so unchanged-file results may reflect main's newer content. Re-run
  `index(action="build")` in the worktree to refresh. Also covers a subtler
  case: a worktree sync that failed part-way can leave stale delta chunks
  for a file you have since reverted to match main. That path is no longer
  in `dirty_paths`, so main serves it — correctly — while the delta's
  leftover copy answers too, and the same path appears twice. The sync
  records its dirty set *before* it upserts anything, so the reverse
  failure (a path the delta holds but the sidecar never listed) cannot
  happen; a failed sync over-excludes, which returns too few results rather
  than results from another branch.
- **`worktree_state_warning`** — the delta has chunks but no dirty paths are
  recorded, an inconsistent state that should never happen in a healthy
  worktree (a delta only exists because something was dirty). Main was
  queried with no exclusions and may be serving stale chunks. Re-run
  `index(action="build")` in the worktree to repair the record.
- **`main_never_indexed_note`** — main itself has no indexed chunks at all,
  so every result comes only from the worktree's own delta. Main is not
  queried at all in this state: it could only return an empty page, and
  asking would ship the whole exclusion list to the store to find that out.

See the `semantic_search` tool's own extended description (its `long_docs`,
surfaced by MCP clients that expose per-tool detailed help) or
[Semantic Search Tools](../tools/semantic-search.md) for the exact field
list every result carries.

## What this does not cover yet

- **No automatic trigger.** Nothing runs `index(action="build")` for you on
  entering a worktree — see *Can I use this today?* above.
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
