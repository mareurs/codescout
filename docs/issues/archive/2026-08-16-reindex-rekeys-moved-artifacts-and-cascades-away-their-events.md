---
kind: bug
status: fixed
tags:
- librarian
- catalog-drift
- archive-flow
- event-log
- data-loss
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related:
- docs/issues/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md
- docs/issues/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
- docs/trackers/open-issue-work-queue.md
severity: high
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

> **Correction, 2026-08-16.** The first version of this section had the direction
> backwards — it called `move` correct and `reindex` the culprit. That was wrong,
> and it was wrong in the way the Iron Law warns about: it *sounded* right (the
> call that preserves data must be the good one) and I wrote it before reading
> `doctor.rs`. **The codebase states the opposite invariant outright, in prose, in
> two places** (`src/librarian/tools/doctor.rs`, module docs and `reseat_worktree`):
>
> > Catalog identity is `id == artifact_id_from_abs(abs_path)`, so a bare
> > `abs_path` UPDATE that kept `id_w` would leave the row mismatched, and the
> > next MAIN-repo reindex's `artifact::upsert` pre-clean would delete it —
> > cascading away exactly the history this exists to preserve.
>
> That is a precise description of what `move` was doing. `reindex` is
> implementing the stated model; `move` is the one breaking it. Corrected
> analysis below.

**`move` broke the invariant.** `src/librarian/tools/mv.rs:15-106`
renamed the file and upserted with

```rust
let updated_row = ArtifactRow {
    abs_path: new_full.clone(),
    updated_at: now,
    file_mtime,
    file_sha256,
    ..row.clone()          // <- id preserved deliberately
};
```

— treating identity as stable and path as a mutable field. That is precisely the
"bare `abs_path` UPDATE" `doctor.rs` says must never happen. It leaves a row whose
id no longer hashes from its path: a live tripwire, armed until the next walk.

**`reindex` implements the stated model.** It derives an artifact's id from its
`abs_path` (`src/librarian/indexer.rs:152`) and looks the row up *by that id*
(`indexer.rs:166`), so a mismatched row is invisible to it. `existing` comes back
`None`, a fresh id is minted, and the walk reports the file as `added`.

`doctor`'s `reseat_worktree` already solves exactly this situation the right way:
mint `id_m = artifact_id_from_abs(new_path)`, then `graft::graft_rows` folds the
old row's events, links, observations and augmentation onto it before deleting it.
`move` simply never adopted that pattern.

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
   **Verdict:** rejected on the fact (it preserved the id) but the *conclusion I
   drew from it* — "so `move` is correct" — was wrong. Preserving the id is the
   defect, not the proof of innocence. See hypothesis 5.

5. **Hypothesis:** identity is genuinely ambiguous in this codebase, so either
   half could reasonably be called the bug.
   **Test:** grep the invariant rather than reason about it —
   `src/librarian/tools/doctor.rs` states `id == artifact_id_from_abs(abs_path)`
   twice, `reseat_worktree` is built entirely around maintaining it, and
   `migrate_v6`'s implicit id migration depends on the pre-clean that enforces it.
   **Verdict:** rejected — three code surfaces implement path-derived identity and
   nothing implements stable identity. The ambiguity was in the docs
   (`tracker-conventions` told callers to "cite by stable ID"), not in the code.

6. **Hypothesis:** the cheapest fix is to teach the indexer to fall back to an
   `abs_path` lookup before minting a new id.
   **Test:** implemented it; the regression test went green. Then read
   `src/librarian/catalog/migrate_v6.rs:12-23`.
   **Verdict:** rejected and reverted. That module documents relying on the
   pre-clean to absorb an id-algorithm change, and its own reviewer note says a
   future hash change must add an explicit migration instead. A silent abs_path
   fallback would defeat both, and would make the invariant unenforceable by
   tolerating every violation instead of surfacing it. The fix belongs at the one
   call site that breaks the rule.

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

**Implemented 2026-08-16 on `experiments`.** Identity model chosen deliberately
(user decision, recorded here because the alternatives were live): **path-derived** —
`id = sha256(abs_path)`, as `doctor.rs` already states and `migrate_v6` already
relies on. `move` was made to maintain it instead of breaking it.

`src/librarian/tools/mv.rs` now does what `doctor`'s `reseat_worktree` does:

```rust
let new_id = ids::artifact_id_from_abs(&new_full);
artifact::upsert(&cat, &ArtifactRow { id: new_id.clone(), abs_path: new_full.clone(), ..row.clone() })?;
let grafted = if new_id != a.id {
    Some(graft::graft_rows(&mut cat, &a.id, &new_id)?)
} else { None };
```

`graft_rows` re-points events, observations, links and `event_edges`, merges the
augmentation, and deletes the old row — in one `IMMEDIATE` transaction. After it,
`id == hash(path)` holds again, so the next reindex hits `ON CONFLICT(id)` instead
of the pre-clean `DELETE` and nothing is lost.

**The response now reports the re-key rather than hiding it:**

```json
{"id": "<new>", "previous_id": "<old>", "id_changed": true,
 "history_grafted": {"events": 1, "observations": 0, "links": 1, "event_edges": 0},
 "old_abs_path": "...", "new_abs_path": "...", "moved": true}
```

The id changing is **by design** under this model, and was already the documented
cost (`migrate_v6`: *"External citations to the old IDs go stale — that's the
documented user-visible cost"*). What was not by design is that it happened
*silently and later*, at the next reindex, taking the history with it. Now it
happens at move time, atomically, with the history intact and both ids reported.

### Docs corrected in the same commit

Three surfaces asserted the old contract:

- `src/librarian/tools/artifact.rs` — `new_rel_path`'s schema description now
  states that a move mints a new id and names the response fields. This is the
  surface an agent actually reads before calling.
- `src/prompts/guides/librarian.md` § *Archiving / Moving Trackers* — documents
  the re-key, the graft, and the two consequences (re-point citations; never
  cache an id across a move).
- `src/prompts/guides/tracker-conventions.md` — **"Cite by stable ID in prose"**
  was simply false for 16-hex artifact ids and is now split: entry IDs (`F-3`,
  `BUG-40`) are stable, artifact ids are not, prefer an entry ID or rel_path for
  anything likely to be archived. Its `link_scan` note also claimed the pre-clean
  drops a moved artifact's links — no longer true, since the move grafts them.

## Worktree interaction

Raised during implementation: a re-key could plausibly strand the worktree
overlay's bookkeeping. Checked each surface rather than assumed.

| Surface | Keyed by | Effect of a re-key |
|---|---|---|
| `worktree_registration` | `worktree_root` **path** (PK) | none — no artifact id in the table |
| `worktree_of` lineage link | `artifact_link(src_id, dst_id)` | **follows** — `repoint_history` updates *both* endpoints |
| `shadow_main_pairs` | the lineage link + shadow `abs_path` | resolves either way; both inputs follow |
| shadow vs. main of the same file | distinct `abs_path` → distinct ids | independent; a move in one cannot touch the other |
| tracker *created* in a worktree | `artifact_id_from_abs` already | unchanged — creation was always path-derived |
| `mv` on a main artifact from a worktree | — | already refused before this change |

The load-bearing one is the lineage link, because `merge_worktree` finds shadows
through it: if it did not follow a main-side re-key, merging would silently stop
seeing a shadow it is supposed to fold. `repoint_history` updates `src_id` **and**
`dst_id` (`src/librarian/catalog/graft.rs`), so it does — and
`shadow_main_pairs_follows_a_main_twin_re_keyed_by_a_move` now pins that, with a
baseline assertion before the move so the test cannot pass vacuously.
## Tests added

- `move_carries_history_onto_the_new_id_and_survives_a_reindex`
  (`src/librarian/tools/mv.rs`) — the regression test. Seeds an event, moves the
  artifact, asserts the response reports the new id, the old id no longer
  resolves, the history landed on the new id — **then reindexes and asserts it is
  still there.** Written first and watched fail (`left: aabbccdd11223344, right:
  b6d380ce1bc5e21b`).

  The reindex step is the whole test. Asserting only that history follows the move
  passes the moment `graft_rows` is wired up and would still pass if the row were
  left mismatched — the deletion happens on a later walk.

- `shadow_main_pairs_follows_a_main_twin_re_keyed_by_a_move`
  (`src/librarian/tools/worktree.rs`) — the overlay's lineage pairing survives a
  main-side re-key. Asserts the baseline pair *before* the move too, so it fails
  if the fixture stops producing a pair at all.

- `move_renames_file_and_updates_catalog` and
  `move_succeeds_for_active_project_absent_from_legacy_roots` **updated** — both
  asserted the old id still resolved after a move. Neither reindexed, so both
  passed on a contract that expired on the next walk. They now assert
  `previous_id` / `id_changed` and that the old row does not linger.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 3860 passed / 0 failed / 45 ignored.
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

**Verified live 2026-08-16, and this file is its own test case.** Archiving it was
the verification: the move that put it in `docs/issues/archive/` is the call under
test.

```
artifact(action="move", id="0dc35c5053dabcee", new_rel_path="docs/issues/archive/…")
-> {"id": "18a637f59289192c", "previous_id": "0dc35c5053dabcee", "id_changed": true,
    "history_grafted": {"events": 4, "observations": 0, "links": 2, "event_edges": 0}}

librarian(action="reindex")
-> {"added": 0, "updated": 0, "removed": 0, "unchanged": 1002}
```

`added: 0` is the proof. The archived file registered as **unchanged**, not
`added` — its id already matched its path, so the walk hit `ON CONFLICT(id)`
instead of the abs_path pre-clean. Compare the pre-fix reindex in § Evidence:
`added: 25`, of which 24 were re-keys.

The catalog, before the move and after the reindex:

| | before | after |
|---|---:|---:|
| events on this artifact | 4 | **4** (on the new id) |
| total catalog events | 1843 | **1843** |
| rows / events under the old id | 1 / 4 | 0 / 0 |
| orphaned events, catalog-wide | 0 | 0 |

The same sequence under the old code destroyed 4 events. Zero loss.

**A note on how this was verified, because the first attempt was bad evidence.**
`codescout_sha` on this session's rows said `536b9581` — a commit *before* the
fix — which read as "the fix is not live." I then ran `strings` on
`target/release/codescout`, found the new response fields, and concluded it was.
That check was invalid: the on-disk binary is not necessarily the image a
long-lived server process is running, which is the very reason the json_path bug's
archive recommends ranking on `codescout_sha`. What actually settled it was the
**behavioural** check — calling `move` and reading `previous_id` /
`history_grafted` out of the response. The build had been made from a dirty tree
containing the fix, seconds before the commit that named it. Filed as BL-24
(`docs/issues/2026-08-16-usage-db-records-a-sha-that-need-not-describe-the-built-code.md`):
`build.rs` already computes a dirty flag and `usage.db` never records it.

**Not repaired, and not repairable:** the history already destroyed — 11 events in
the measured call, plus whatever the 348 previously-archived bug files and the
`docs/trackers/archive/` cohort lost before this. Cascade-deleted, not orphaned (0
of 1845), so there is nothing to re-point. The bodies are in git; the event logs
are gone.

**Follow-up, filed separately:** BL-23 — a moved file's frontmatter still asserts
its pre-move id
(`docs/issues/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`).

Fix SHA on **`experiments`**: `2d8c7f39`. `git rev-list --left-right --count
master...experiments` has 0 on the left, so promotion is a fast-forward and this
SHA is the master SHA — no second SHA to record.
## References

- `src/librarian/catalog/artifact.rs:137-187` — `upsert`, and the abs_path DELETE at 152
- `src/librarian/tools/mv.rs:15-106` — `move`, which preserves the id
- `get_guide("tracker-conventions")` § Bug files — the archive flow this breaks
- `get_guide("librarian")` § Archiving / Moving Trackers — states `move` is "the safe path"
- `docs/issues/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md` — BL-20
- `docs/issues/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md` — BL-21
- `docs/trackers/open-issue-work-queue.md` — BL-22
