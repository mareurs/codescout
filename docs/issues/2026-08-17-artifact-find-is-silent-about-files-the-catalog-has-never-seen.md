---
id: '4b4618b310436dbc'
kind: bug
status: open
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

Not implemented. The shape is settled by precedent: make the answer self-describing
rather than change what is authoritative.

**Recommended — a staleness hint on `find`.** Compare a cheap on-disk count for the
queried scope against the catalog row count, and when disk is larger, add to `hints`:

```
"unindexed_files": 1,
"unindexed_hint": "1 file under this scope is not in the catalog and cannot match any
                   filter; run librarian(action=\"reindex\") to include it"
```

This mirrors `more_in_workspace_hint` (rows exist but this query cannot reach them) and
the grep completeness warning (a zero that says what it covered). It also has to be
**cheap** — `find` is called constantly, so a full walk per call is not acceptable; a
count restricted to the artifact globs, or an mtime-based dirty check on the scope's
directories, is the right budget.

**Alternative — reindex on session start.** Removes the lag instead of reporting it.
Rejected as the primary fix: it costs a walk on every session, it still leaves
mid-session arrivals silent (this file arrived mid-session, from a peer commit), and the
guide already advises it without effect.

Both could ship; the hint is the one that makes the failure visible rather than merely
less likely.

## Tests added

None yet. The regression test seeds an artifact file on disk without going through
`artifact(action="create")`, runs `find`, and asserts the response carries the
`unindexed_files` hint — then reindexes and asserts the hint is gone and the row is
returned. The load-bearing half is the first assertion: a test that only checks
post-reindex behaviour passes today.

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

Decide the staleness-check budget before writing anything — `find` is on the hot path, so
the mtime-based dirty check is likely the only affordable form, and it needs measuring
against a cold cache. Then add the hint to `build_hints`
(`src/librarian/tools/find.rs`), beside `more_in_workspace_hint`, which is the closest
existing precedent for "rows you cannot reach from here".

## References

- `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md` — the same failure shape, fixed by making the zero self-describing
- `docs/issues/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md` — the second defect this file exhibits
- `src/librarian/tools/find.rs` — `build_hints`, where the hint belongs
- `get_guide("librarian")` § Gotchas — the documented no-watcher behaviour

