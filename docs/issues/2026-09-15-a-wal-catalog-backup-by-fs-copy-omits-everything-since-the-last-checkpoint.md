---
status: open
opened: 2026-09-15
closed:
severity: high
owner: marius
related: []
tags: [cluster/record-asserts-an-unchecked-completion]
kind: bug
---

# A WAL catalog backed up by `fs::copy` omits everything since the last checkpoint

## Summary

`catalog.db` is opened `journal_mode = WAL` (`src/librarian/catalog/mod.rs`, all three
`open*` paths). In WAL mode the main `.db` file holds only **checkpointed** pages;
everything committed since the last checkpoint lives in the `-wal` sidecar. Two sites back
the catalog up with `std::fs::copy` of the main file alone, so both produce a backup
missing every recent commit — silently, through a call that returns `Ok`.

**Measured 2026-09-15** on a WAL database in this exact shape (`journal_mode=WAL`,
connection held open, checkpoint, then 50 more committed rows): **51 rows visible to the
live connection, 1 row in the copy.**

Not theoretical on this machine: the live catalog was 420 MB with a **4.2 MB `-wal`
written seconds before the reading**.

## The two sites

| # | site | exposure | status |
|---|---|---|---|
| 1 | `rebuild_artifact_vec_v2_at_dim`, `src/librarian/indexer.rs` | copies while **its own** connection is open and WAL-active — guaranteed to miss its own uncheckpointed commits | **fixed** — see *Fix provenance* |
| 2 | `backup_db`, `src/librarian/catalog/mod.rs:500` (called from `open_with_workspace` when `needs_v6`) | copies *before* this process opens its connection, so it is exposed only to **another** process's uncheckpointed WAL — real on a machine-shared catalog, smaller window | **open** |

Site 2 is why this file stays `open` after site 1 ships. Per § *Testing Discipline*,
**mutate once per guarded SITE** — a kill at site 1 says nothing about site 2.

## Why it mattered more the moment it was noticed

Site 1's backup was, until PR #20, taken before rebuilding `artifact_vec` (v1) — a table
search had stopped reading. Restoring it bought nothing anyone needed, so the defect was
inert. PR #20 retargets that same migration at `artifact_vec_v2`, the table chunk-grain
search actually queries, and
`docs/adrs/2026-07-20-artifact-vec-shared-catalog-boundary.md`'s amendment enshrines *"an
opt-in, **backed-up** rebuild"* as the accepted decision. The backup became the sole
safety net in the same change that made it load-bearing.

What a stale restore loses is **not** the vectors — `backfill-chunks` regenerates those.
It is every artifact, event and **augmentation** committed since the last checkpoint, and
augmentations are not in git (`docs/conventions/cross-machine-catalog-resume.md`).

## Why the obvious fix is not one

`PRAGMA wal_checkpoint(TRUNCATE)` before the copy **does not work here**, and it is named
in the code so the next reader does not reach for it. It reports `busy = 1` in its result
row rather than failing when any other connection holds a read lock. Measured: **one**
concurrent reader was enough to return `(busy=1, log=100, checkpointed=100)`. This catalog
is shared by every codescout process on the machine — 6 sessions in this checkout at the
time of writing — so busy is the **ordinary** case, not the edge one. Checkpoint-then-copy
returns the same stale backup through a call that looks like it succeeded, which is
strictly worse than the bare copy: it adds a step that reads as diligence.

`VACUUM INTO` takes a read transaction and writes a snapshot including WAL content,
unblocked by concurrent readers — verified 51/51 with a reader holding a transaction open.
It refuses inside an open transaction; that error is propagated rather than falling back
to a copy, because a rebuild that destroys vectors must not proceed on a backup that
cannot restore.

## Why no test caught it

`write_embeddings_v2_migration_backs_up_file_backed_catalog`
(`src/librarian/indexer.rs`) asserts only that a file whose name contains
`pre-vec-v2-dim-bak` appeared. **It never opens it.** That is an existence assertion,
monotone under the backup being empty, stale, or truncated — green for a 0-byte file. The
defect shipped with coverage that read as real.

The replacement asserts the other direction and both are kept: the new test checkpoints,
commits an artifact that then exists only in `-wal`, triggers the migration, and opens the
backup to read that row back.

**Observed RED, not an assertion's existence.** Mutating the production path
(`snapshot_catalog` → `std::fs::copy`) reds the new test (`left: 0, right: 1`) while
the pre-existing test stays **green** — which is the discrimination evidence that prior
coverage structurally could not see this.

## Fix provenance

Site 1 fixed by `dd05a56a`, patch-id `248d200cb7e89a4fd09fc58dd2b50470a14e71ad`
(merged to `experiments` in `720fbeb8`; cite the constituent commit, never the merge).
The pair is recorded here rather than promised: the SHA is positional and dies when
`experiments` is rebased, the patch-id is a content hash of the diff and survives rebase
and cherry-pick. Verified at fix time — CI 24/24 including the `server-stack` lane, and
the four-command gate green in an isolated worktree (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`,
1864 `librarian::` tests in the default lane).

**Site 2 (`catalog::backup_db`) remains open, which is why this file is not archived.**
A kill at site 1 says nothing about site 2 — `write_embeddings_v2`'s path and
`open_with_workspace`'s path are separate guarded sites, and only the first has a test
that opens its backup.

## Repro

```
journal_mode=WAL; connection held open
INSERT ...; PRAGMA wal_checkpoint(TRUNCATE);   -- everything so far -> main .db
INSERT x50                                      -- committed, lives in -wal
fs::copy(main.db, backup.db)
sqlite3 backup.db "SELECT COUNT(*)"             -- 1, not 51
```
