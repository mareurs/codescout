---
status: open
opened: 2026-08-16
closed:
severity: high
owner: marius
related:
  - docs/issues/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md
  - docs/issues/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
  - docs/trackers/open-issue-work-queue.md
tags:
  - librarian
  - catalog-drift
  - archive-flow
  - event-log
  - data-loss
kind: bug
---

# BUG: reindex re-keys a moved artifact and cascade-deletes its event history — the sanctioned archive path destroys the record it exists to protect

## Summary

`artifact(action="move")` is the prescribed way to archive a bug or tracker,
specifically so the catalog row is not orphaned. It does its job correctly: it
preserves the id and updates `abs_path`. The **next `reindex` then undoes it** —
it re-derives `id = sha256(abs_path)`, upserts under the new id, and `upsert`'s
own `DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2` removes the
move-preserved row. The FK cascade takes the artifact's events with it.

Net effect of the documented archive flow: the artifact gets a **new id**, its
**event history is destroyed**, the file's own frontmatter `id:` is left stating
a dead id, and every prose citation of that id silently breaks. `reindex`
reports `removed: 0`.

## Symptom (Effect)

Archive a bug the sanctioned way, then reindex, then try to use the id the move
returned:

```
artifact(action="move", id="875e5d03d980ceac",
         new_rel_path="docs/issues/archive/2026-08-15-jsonpath-subset-...md")
-> {"id": "875e5d03d980ceac", "moved": true}        <- reports the OLD id back

librarian(action="reindex")
-> {"added": 2, "updated": 3, "removed": 0, ...}    <- says nothing was removed

artifact(action="update", id="875e5d03d980ceac", patch={...})
-> {"error": "unknown id `875e5d03d980ceac`"}
```

The artifact is now `2bd71246fc807cba`. Its event log is empty.

## Reproduction

Commit `a9a397a9` (`experiments`), build `a9a397a9`.

1. `artifact(action="move", id=<any bug id>, new_rel_path="docs/issues/archive/<same-name>.md")`
2. `librarian(action="reindex")`
3. `artifact(action="get", id=<original id>)` → `unknown id`
4. `artifact(action="find", filter={"rel_path": {"contains": "<slug>"}}, include_archived=true)`
   → a **different** id at the archive path
5. `artifact_event(action="list", artifact_id=<new id>)` → empty (or only
   post-move events)
6. `read_markdown(<archived file>, start_line=1, end_line=3)` → frontmatter still
   carries the **old** id

## Environment

Linux, codescout `0.15.0`, branch `experiments`, MCP stdio. Catalog at
`~/.local/share/librarian/catalog.db` (35 MB, 1,845 events at time of writing).

## Root cause

*Read at the bytes 2026-08-16 from `src/librarian/tools/mv.rs` and
`src/librarian/catalog/artifact.rs`; the corpus-wide half measured directly
against the catalog DB, queries in § Evidence.*

**`move` is not the culprit — it is correct.** `src/librarian/tools/mv.rs:15-106`
renames the file and upserts with

```rust
let updated_row = ArtifactRow {
    abs_path: new_full.clone(),
    updated_at: now,
    file_mtime,
    file_sha256,
    ..row.clone()          // <- id preserved deliberately
};
```

Identity is treated as stable and path as a mutable field. Events survive.

**`reindex` holds the opposite model.** It derives an artifact's id from its
`abs_path`, so the same file at a new path is a new artifact. It upserts the
archive path under the freshly-derived id.

**`upsert` then executes the delete that does the damage** —
`src/librarian/catalog/artifact.rs:152-154`:

```sql
DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2
```

Any row sharing the abs_path under a different id is removed. That is exactly
the move-preserved row. `events.artifact_id` is
`REFERENCES artifact(id) ON DELETE CASCADE`, so the event log goes with it.

The two components disagree about what an artifact's identity *is*. `move` says
"id is stable, path is a field." `reindex` says "id is a function of path."
Both are internally coherent; run in sequence they delete history. The delete
lives inside `upsert`, below reindex's own accounting, which is why the call
reports `removed: 0`.

## Evidence

### Caught in the act: one reindex destroyed 11 events

The density figures below are a proxy. This is the direct measurement — a single
`librarian(action="reindex")` call, run 2026-08-16 minutes after the queries in the next
subsection, with the catalog sampled immediately before and after:

```
before:  SELECT COUNT(*) FROM events;  -> 1845

librarian(action="reindex")
  -> {"added": 25, "updated": 0, "removed": 0, "unchanged": 974}

after:   SELECT COUNT(*) FROM events;  -> 1834
```

**11 events gone, and the call reported `removed: 0`.**

The `added: 25` is the tell. Exactly **one** file was new to the repo (this bug file). The
other 24 were pre-existing artifacts re-keyed under freshly-derived ids — the arithmetic
closes: the prior run reported `unchanged: 993` against 998 rows, this one `unchanged: 974`,
so 24 previously-known artifacts fell out of `unchanged` and reappeared as `added`.

Sampling what got "added" names the population precisely:

```
2bd71246fc807cba  docs/issues/archive/2026-08-15-jsonpath-subset-...md
0dc35c5053dabcee  docs/issues/2026-08-16-reindex-rekeys-...md          <- genuinely new
3fb33d4da0791fe6  docs/issues/2026-08-16-edit-file-replace-all-...md   <- genuinely new
ae3c100e92576b5c  docs/trackers/archive/edit-markdown-batch-ordering-session-log.md
38e8442eb5b4fd7c  docs/trackers/archive/il1-friction-diagnosis-session-log.md
6a747672419ff2c7  docs/trackers/archive/perf-windows-session-log.md
8c1b0e304b16c7a9  docs/trackers/archive/pi-integration-session-log.md
79594a028ad93a90  docs/trackers/archive/prompt-guide-refactor-session-log.md
a8879c5862f74ae0  docs/trackers/archive/tracker-as-skill-session-log.md
d9eaac1817a3fcb1  docs/trackers/archive/tracker-redesign-session-log.md
e5fea0542b09aacc  docs/trackers/archive/worktree-overlay-session-log.md
6486c4e3ad18bab3  docs/trackers/archive/lancedb-upgrade-2026-05.md
```

Almost all of them sit under `docs/trackers/archive/` — and they were archived **minutes
earlier, by a concurrent session** running a tracker sweep (22 trackers moved out of
`docs/trackers/` in one pass). Their rows were sitting at the archive path under their
preserved pre-move ids, exactly as `move` leaves them. My reindex derived new ids from the
new paths, upserted, and the DELETE at `artifact.rs:152` took the originals — and their
events — with them.

That the old-path rows are **gone** rather than left behind is what rules out a bare `git mv`
and confirms `move` was used:

```sql
-- rows still at the pre-sweep tracker paths
SELECT COUNT(*) FROM artifact WHERE abs_path LIKE '%/docs/trackers/%'
  AND abs_path NOT LIKE '%/archive/%'
  AND (abs_path LIKE '%pi-integration-session-log.md'
    OR abs_path LIKE '%perf-windows-session-log.md'
    OR abs_path LIKE '%tracker-redesign-session-log.md');        -> 0
```

So the loss was not caused by anyone doing it wrong. Two sessions each followed the
documented procedure — one archived through `artifact(action="move")`, the other ran
`librarian(action="reindex")` — and the composition destroyed history neither call reported.

**`created_at` is destroyed too.** The replacement rows are genuinely new, and carry a fresh
creation timestamp:

```
e5fea0542b09aacc  docs/trackers/archive/worktree-overlay-session-log.md   created_at 1786871062851
d9eaac1817a3fcb1  docs/trackers/archive/tracker-redesign-session-log.md   created_at 1786871062839
a8879c5862f74ae0  docs/trackers/archive/tracker-as-skill-session-log.md   created_at 1786871062827
```

All three timestamps are the reindex itself. Multi-month session logs now assert they were
created today. Anything that reasons about artifact age — freshness ranking,
`workspace_state_at` time-travel, `librarian(action="context")` recency — reads the reindex
time, not the artifact's.

Two consequences the density table alone does not show:

- **The loss is recurring, not one-time.** Each `reindex` collects whatever has been moved
  since the last one. Every archive sweep plants a charge that the next reindex detonates.
- **The blast radius is trackers too, not just bugs.** Session logs are the artifacts whose
  event history is *most* worth keeping — they are the cross-session memory the tracker
  conventions are built around.

### The events were destroyed, not orphaned

```sql
-- events still keyed to the dead id
SELECT COUNT(*) FROM events WHERE artifact_id='875e5d03d980ceac';   -> 0

-- orphaned events anywhere in the catalog (artifact_id with no artifact row)
SELECT COUNT(*) FROM events e LEFT JOIN artifact a ON a.id=e.artifact_id
 WHERE a.id IS NULL;                                                -> 0

-- total events in the catalog
SELECT COUNT(*) FROM events;                                        -> 1845
```

Zero orphans across 1,845 events: the cascade fired. The history is gone, not
merely unreachable.

### This is corpus-wide, not one unlucky file

Event density, live bug files vs. archived ones:

```sql
-- live
SELECT COUNT(*) FROM artifact
 WHERE abs_path LIKE '%/docs/issues/%' AND abs_path NOT LIKE '%/archive/%';  -> 154
SELECT COUNT(*) FROM events e JOIN artifact a ON a.id=e.artifact_id
 WHERE a.abs_path LIKE '%/docs/issues/%' AND a.abs_path NOT LIKE '%/archive/%';  -> 100

-- archived
SELECT COUNT(*) FROM artifact WHERE abs_path LIKE '%/docs/issues/archive/%';     -> 348
SELECT COUNT(*) FROM events e JOIN artifact a ON a.id=e.artifact_id
 WHERE a.abs_path LIKE '%/docs/issues/archive/%';                               -> 7
```

| | rows | events | events/row |
|---|---:|---:|---:|
| live `docs/issues/` | 154 | 100 | **0.65** |
| archived `docs/issues/archive/` | 348 | 7 | **0.02** |

A **32× drop** in event density at exactly the moment a bug is archived. Archived
bugs are the *older* ones — they had longer to accrue events, so the density
should be higher, not 3% of live. 348 bug files have been through this flow.

### The frontmatter is left stating a dead id

```
docs/issues/archive/2026-08-15-jsonpath-subset-...md:2:  id: '875e5d03d980ceac'
docs/trackers/open-issue-work-queue.md:44:               | BL-1 | ... | `875e5d03d980ceac` |
```

Catalog says `2bd71246fc807cba`. Nothing rewrites the frontmatter, so the file
asserts an id that resolves to nothing — and the tracker citing it is now a dead
reference. Per `get_guide("tracker-conventions")`, citing artifacts by stable id
in prose is the *recommended* practice; this bug makes that practice decay on
every archive.

## Hypotheses tried

1. **Hypothesis:** `artifact(action="move")` re-keys the artifact.
   **Test:** read `src/librarian/tools/mv.rs:15-106`.
   **Verdict:** rejected — `..row.clone()` preserves the id explicitly, and the
   response echoes the original id. `move` is correct in isolation.

2. **Hypothesis:** the events were orphaned (left keyed to a dead id), so a
   repair could re-point them.
   **Test:** `SELECT COUNT(*) FROM events e LEFT JOIN artifact a ... WHERE a.id IS NULL`.
   **Verdict:** rejected — 0 orphans of 1,845. The cascade deleted them. No
   repair is possible for history already lost.

3. **Hypothesis:** the loss is specific to this one file / this one session.
   **Test:** event density live vs. archived across all 502 bug rows.
   **Verdict:** rejected — 0.65 vs 0.02 events/row. Corpus-wide.

4. **Hypothesis:** `reindex`'s `removed: 0` means no row was deleted.
   **Test:** read `src/librarian/catalog/artifact.rs:137-187`.
   **Verdict:** rejected — the DELETE is inside `upsert`, beneath reindex's
   counters. The report is accurate about its own accounting and blind to this.

## Fix

Not yet implemented. The design question comes first: **is an artifact's id
path-derived or stable?** The codebase currently answers both ways. Options, in
increasing order of ambition:

1. **Teach reindex to recognise a move.** Before minting a new id for a path,
   look for an existing row whose `file_sha256` matches and whose `abs_path` no
   longer exists — that is a rename, so update the row's path in place rather
   than re-keying. Cheapest, and it fixes the archive flow without touching the
   identity model.

2. **Make `upsert`'s abs_path DELETE loud, or refuse it.** Line 152 silently
   destroys a row with history. At minimum it should return what it deleted so
   the caller can report it; better, refuse when the doomed row has events and
   require an explicit reseat. This is the same shape as BL-20's params wipe and
   BL-21's guard gap — **a destructive write with no report and no opt-in**, the
   third instance found this session.

3. **Decouple identity from path.** A stable minted id with `abs_path` as a
   plain mutable column removes the whole class. Largest change; also the one
   that makes "cite artifacts by id in prose" durable, which the tracker
   conventions already assume.

Whatever is chosen, **the frontmatter `id:` must be rewritten when the id
changes**, or files will keep asserting dead ids.

## Tests added

None yet — bug is `open`. The regression test is a three-step integration:
create an artifact with an event → `move` it → `reindex` → assert the id is
unchanged **and** `artifact_event(action="list")` still returns the event. Note
that a test asserting only "the id survives `move`" passes today and proves
nothing; the reindex step is the whole test.

## Workarounds

- **After archiving, re-resolve the id** before citing it:
  `artifact(action="find", filter={"rel_path": {"contains": "<slug>"}}, include_archived=true)`.
  Do not reuse the id the `move` call returned.
- **Do not rely on event history surviving an archive.** If a bug's event log
  matters, copy the relevant facts into the file body before moving it — the body
  is in git, the events are not.
- Re-point prose citations of the old id in the same commit as the move, the way
  `get_guide("tracker-conventions")` already requires for *path* citations. The
  guide covers paths but not ids; ids break the same way.

## Resume

Start at `src/librarian/catalog/artifact.rs:152-154` (the DELETE) and
`src/librarian/tools/mv.rs:15-106` (the id-preserving upsert that reindex
undoes). Decide the identity question before writing code — option 1 is a local
fix, option 3 is the real one, and picking 1 without recording the decision will
leave the same contradiction in place for the next surface that moves a file.

Write the integration test first (create → event → move → reindex → assert id
and event survive) and watch it fail at the reindex step.

Fallout from this bug still outstanding in the repo, to fix once the id model is
settled:

- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`
  frontmatter asserts `id: '875e5d03d980ceac'`; the live id is `2bd71246fc807cba`.
- `docs/trackers/open-issue-work-queue.md` BL-1 row cites the dead id, in both
  the body snapshot and the `tasks` params entry. **The params copy was left
  deliberately**: there is no entry-grain update (BL-20), so correcting one
  field of one row requires the wholesale array replace that already wiped this
  tracker once. A one-field correction is currently unsafe to make — which is
  the clearest argument yet for BL-20's fix 3.

## References

- `src/librarian/catalog/artifact.rs:137-187` — `upsert`, and the abs_path DELETE at 152
- `src/librarian/tools/mv.rs:15-106` — `move`, which preserves the id
- `get_guide("tracker-conventions")` § Bug files — the archive flow this breaks
- `get_guide("librarian")` § Archiving / Moving Trackers — states `move` is "the safe path"
- `docs/issues/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md` — BL-20
- `docs/issues/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md` — BL-21
- `docs/trackers/open-issue-work-queue.md` — BL-22
