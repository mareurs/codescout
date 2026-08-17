---
id: 0078deac99b4d2e8
kind: bug
status: fixed
title: 'BUG: artifact(find) answers from the catalog with no signal that N files on disk were never indexed — a bug file committed outside artifact(create) is invisible to every query, including the mandated queue query'
---

## Summary

`artifact(action="find")` reports what the catalog holds. A markdown artifact created
outside `artifact(action="create")` — with `create_file`, `Write`, or `git`— is absent
from the catalog until someone runs `librarian(action="reindex")`, and **nothing in the
response says so**. The count, the `hints` block, and `include_archived=true` all behave
exactly as they would if the file did not exist.

The project's own conventions make one `artifact(find)` call *the* way to answer "what's
open?" (`CLAUDE.md`, `get_guide("tracker-conventions")`). That query is therefore
structurally capable of omitting a just-filed bug and reading as complete.

## Symptom (Effect)

Measured 2026-08-17 at `a3f42f9a`. `docs/issues/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md`
was committed at 12:45 and existed on disk. Every catalog query denied it:

```
artifact(find, kind="bug", filter={"rel_path": {"contains": "never-reaches-any-profile"}},
         include_archived=true)
→ {"count": 0, "items": []}

artifact(find, kind="bug", filter={"rel_path": {"contains": "plugin-content-edit"}},
         include_archived=true)
→ {"count": 0, "items": []}
```

Two `count: 0` responses with empty `hints` — indistinguishable from "no such file".

The disk/catalog disagreement was visible only by counting both sides:

```
$ ls docs/issues/*.md | wc -l          → 27
$ ls docs/issues/archive/*.md | wc -l  → 320
artifact(find, kind="bug", include_archived=true, limit=1)
→ count 1 + hints.more_in_scope 344 = 345 rows
```

`librarian(action="reindex")` then reported `{"added": 1, "updated": 0, "removed": 0,
"unchanged": 1034}` and the same query returned the row as `d1aa7ecc7717b332`.

## Reproduction

1. Create a bug file with `create_file` (or `Write`, or `git checkout` a branch that adds
   one) — anything other than `artifact(action="create")`.
2. `artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})`
3. The new file is absent, and no field in the response indicates an un-indexed file
   exists.
4. `librarian(action="reindex")` → `added: 1`. Re-run step 2; it appears.

## Environment

Linux, codescout `experiments` @ `a3f42f9a`, stdio MCP, project
`/home/marius/work/claude/codescout`. Two sessions share this checkout, which is how a
file arrives without the querying session having created it.

## Root cause

Not a defect in the indexer — this is **documented** behaviour. `get_guide("librarian")`
§ *Gotchas*: *"**No file watcher.** Files added/moved outside `artifact`
`action=create`/`action=update` are invisible until `librarian` with `action=reindex`. On
busy workspaces, reindex once at the start of a session."*

The defect is the **silence**, not the staleness. Three things combine:

1. The catalog is authoritative for `find`, and can legitimately lag disk.
2. Nothing enforces or reminds about the session-start reindex — it is advice in a guide
   that must be pulled.
3. The response carries no completeness signal, so a lagging catalog and an empty
   filesystem produce byte-identical answers.

The severity comes from (3) alone. A caller who knew the count might be short would
reindex; a caller who is told nothing has no reason to.

*Measured, not inferred: the two queries and the reindex output are quoted above. The
absence of a completeness field was read from the responses themselves.*

## Evidence

### The precedent that names the right fix

`docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` (fixed) is
the same failure shape one tool over: *"grep's zero/absent result is silent about the
hidden-path skip, so `.github/` reads as 'not present anywhere'"*. The fix was not to
change what grep searches — it was to make the zero **self-describing**. That warning
now ships on every zero-match grep:

```
warning: this zero describes what was searched, not the pattern. Hidden paths were
not searched, including .buddy/, .cargo/, … Pass include_hidden=true to search them
```

`artifact(find)` has no equivalent. Its zero also describes what was searched.

### What was and was not demonstrated

The file that exposed this carried `status: mitigated`, so it would not have appeared in
the open-bug queue query regardless. The **demonstrated** harm is narrower and stranger:
it was invisible to *every* query, including targeted `rel_path` lookups with
`include_archived=true`.

The queue-omission consequence is **inferred from the same mechanism, not observed**: an
`open` bug file arriving the same way would be missing from the mandated
`status in [open, investigating]` query with no signal. Nothing about the status field
participates in indexing, so the inference is direct — but it has not been reproduced with
an `open` file, and this file should not be read as having proved it.

### A second, independent problem in the same file

After the reindex the row exists but the file still has no `id:` line in its frontmatter,
so it is registered and **unguarded** — a live instance of
`docs/issues/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`.
Reindex registers the artifact; it does not stamp the id into the file. Worth noting that
the two defects share a cause at one remove: a file that never went through
`artifact(action="create")` misses both the catalog row and the id stamp.

## Hypotheses tried

1. **Hypothesis:** the file is hidden by its terminal `mitigated` status, the way
   `archived`/`superseded` trackers are hidden by default.
   **Test:** `artifact(find, kind="bug", filter={"status": {"eq": "mitigated"}}, include_archived=true)`.
   **Verdict:** rejected — 13 `mitigated` bug rows are returned, so the status is
   queryable. Under the rival hypothesis (absent from the catalog) this query would return
   the other mitigated rows but not this file, which is what it did.
   **Evidence:** § Symptom.

2. **Hypothesis:** `include_archived=true` is the flag that would have surfaced it.
   **Test:** both quoted queries already pass it.
   **Verdict:** rejected. The flag widens a status filter; it cannot surface a row that
   does not exist.

## Fix

Implemented as the recommended shape: a staleness hint on `find`, not a change to
what is authoritative.

`count_disk_md` (`src/librarian/tools/find.rs`) does a single ignore-respecting walk
(`ignore::WalkBuilder::new(root).standard_filters(true)`) counting `.md` files under
the resolved scope's root, filtered by the workspace's own `ignore` globset — the
same two filters `index_repo_sync` applies (`.gitignore`/global-excludes via
`standard_filters`, then the project `ignore` patterns), minus its `force_include`
supplemental scan. That last omission can only undercount, which can only suppress
the hint, never spuriously fire it — the safe direction to be wrong in.

`build_hints` compares that disk count against `count_for_scope(cat, None, …)` — the
catalog's TOTAL row count for the scope, unfiltered by kind/status, so it is exact
relative to what a reindex would produce: `index_repo_sync` gives even an
unclassified file `kind: "unknown"` rather than skipping it, so nothing is excluded
on classification grounds — only on the same ignore grounds the walk replicates. When
disk > catalog:

```
"unindexed_files": 1,
"unindexed_hint": "1 file(s) under this scope are not in the catalog and cannot 
                   match any filter; run librarian(action=\"reindex\") to include them"
```

**Scope and budget, the two open questions from Resume:**

- **Which scopes.** `Project` and `Repo` only — the two whose `apply_scope` path
  prefix (`cp.abs_path`, `cp.git_root`) names one real directory this walk can
  anchor on. `Umbrella`/`All` span multiple roots and are not covered. `Scope::Repo`
  is `Scope::default()` — the scope a call with no `scope` param resolves to — so
  this is the arm that fires for the bug's own reproduction (a bare `find` call).
  Skipped from inside a linked worktree (`cp.main_root.is_some()`): `index_repo_sync`
  never indexes a worktree root directly, so a worktree-rooted disk count and the
  (overlay) catalog count are not the same quantity.
- **The budget.** Not mtime-based — there is no existing "last reindex" timestamp to
  diff against, and adding one is new state, not a hint. Measured instead: `git
  ls-files '*.md' | wc -l` over this repo's ~1100 markdown files took 4ms warm-cache;
  the in-process `ignore`-crate walk this uses is the same order of magnitude and
  avoids the subprocess spawn. The walk is bounded by the scope's OWN file count
  (one project's docs, or one repo's), never the whole workspace — that bound, not a
  glob restriction, is what keeps it affordable. A rule-glob-restricted walk (the
  other option Resume floated) was investigated and dropped: every default classify
  rule is `**/`-prefixed for multi-depth matching (`code-explorer/docs/issues/foo.md`
  is a real fixture case), so `literal_glob_prefix` — the exact helper
  `force_include_candidates` uses for this in `indexer.rs` — returns an empty anchor
  for every one of them. There is no cheaper anchor than the scope root itself.
## Tests added

`unindexed_disk_files_surface_a_staleness_hint_then_clear_after_reindex`
(`src/librarian/tools/find.rs`), using a REAL tempdir (not the fictitious
`/test/code-explorer` paths the rest of this file's tests use — `count_disk_md` does
real disk I/O, so a fake path would silently walk to nothing and pass for the wrong
reason). Two `.md` files on disk, one catalog row seeded for only one of them:

1. `find` with no `scope` param (the bug's own reproduction shape) asserts
   `hints.unindexed_files == 1` and `hints.unindexed_hint` names `reindex` — this is
   the load-bearing half; a test that only checked post-reindex behaviour would pass
   today, per this bug's own template.
2. A real `index_repo_sync` call against the same tempdir, then `find` again: asserts
   the hint is gone and `count == 2`.

All 37 `find` tests pass, including the pre-existing scope/hint suite — none of them
use a real `git_root`, so `count_disk_md` walks a nonexistent path and returns 0 for
all of them, which can only suppress the new hint, never spuriously fire it against
an unrelated assertion. Full gate: `cargo fmt`, `cargo clippy --all-targets -- -D
warnings`, `cargo test --lib` — 3912 passed, 0 failed, 7 ignored.
## Workarounds

Run `librarian(action="reindex")` before any "what's open?" report or backlog triage,
especially in a checkout shared with another session — a peer's commit can add a bug file
mid-session, which is exactly what happened here. Cross-check with the filesystem when the
count matters:

```
ls docs/issues/*.md docs/issues/archive/*.md | wc -l
```

against the row count from `artifact(find, kind="bug", include_archived=true, limit=1)`
(`count` + `hints.more_in_scope`).

## Resume

Fixed and closed. Not done: the "reindex on session start" alternative this file
considered and rejected as the *primary* fix remains unimplemented as a
*supplement* — it would still be worth doing, since it removes the lag for the
common case rather than only reporting it, but it was correctly out of scope for
this fix and stays that way.
## References

- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` — the same failure shape, fixed by making the zero self-describing
- `docs/issues/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md` — the second defect this file exhibits
- `src/librarian/tools/find.rs` — `build_hints`, where the hint belongs
- `get_guide("librarian")` § Gotchas — the documented no-watcher behaviour
