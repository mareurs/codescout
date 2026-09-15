---
status: fixed
opened: 2026-09-15
closed: 2026-09-15
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
| 2 | `backup_db`, `src/librarian/catalog/mod.rs` (called from `open_with_workspace` when `needs_v6`) | copies *before* this process opens its connection, so it is exposed only to **another** process's uncheckpointed WAL — real on a machine-shared catalog, smaller window | **fixed** — see *Fix provenance* |

Per § *Testing Discipline*, **mutate once per guarded SITE** — a kill at site 1 said
nothing about site 2, which is why this file stayed `open` for the eight commits between
them. The two are now one implementation: `snapshot_catalog` lives in `catalog`, the
module that sets `journal_mode = WAL`, and `indexer` calls it. A third site written against
the wrong model would have to reimplement it rather than merely forget the lesson.

**The count of two is derived, not inherited.** The only other `std::fs::copy` in `src/`
is `lsp/mux/test_support.rs`, copying LSP fixture files — not a database.

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

### Site 2's test had the same shape, plus a second blindness the first one did not have

`migration_v6_creates_backup_file` (`src/librarian/catalog/migrate_v6.rs`) asserts that a
directory entry `starts_with("catalog.db.pre-v6-bak.")` exists. **It never opens it
either** — the identical existence assertion, written independently, in a different
module, for the sibling site. Two authors reached for the same monotone shape, which is
what makes it a class rather than an oversight.

**And here it would not have helped to assert on the content, which is the part worth
keeping.** `seed_v3_db`, the fixture every v6 test builds on, opens a plain connection and
runs `CREATE TABLE` / `INSERT` with no journal pragma — so the database it leaves is in
**rollback-journal** mode, not WAL. Measured 2026-09-15 by rebuilding that exact shape:
`PRAGMA journal_mode` returns `delete`, on the seeding connection and on reopen, and no
`-wal` file is ever created. Every committed row is therefore already in the `.db` file,
where `fs::copy` finds it. A content assertion bolted onto the old fixture would have
passed against the defect and read as proof.

So the missing coverage was not one assertion. It was an assertion *and* a fixture in the
wrong journal mode, stacked — and the fixture half is the one no reviewer looking at the
assertion would see.

The replacement test annotates its load-bearing details on the fixture lines. **I first
wrote down three and measurement cut it to two**, which is worth recording because the
error ran the other way from the one this section is about: crediting an inert detail as
load-bearing is how a reader stops looking.

| detail | removed → | verdict |
|---|---|---|
| `journal_mode = WAL` | copy sees the row | load-bearing |
| holding the second connection open past the migration | copy sees the row | load-bearing |
| `wal_checkpoint(TRUNCATE)` | copy still sees **0** rows | **not** load-bearing — kept, annotated as a guarantee |

The checkpoint cannot matter here because converting a rollback-journal database to WAL
starts it with an *empty* `-wal`; there is nothing to flush. Site 1's fixture is different
— it opens through `Catalog` and writes embeddings first — so that comment's own
load-bearing claim is about a different database and is deliberately not re-derived.

## Fix provenance

Site 1 fixed by `dd05a56a`, patch-id `248d200cb7e89a4fd09fc58dd2b50470a14e71ad`
(merged to `experiments` in `720fbeb8`; cite the constituent commit, never the merge).
The pair is recorded here rather than promised: the SHA is positional and dies when
`experiments` is rebased, the patch-id is a content hash of the diff and survives rebase
and cherry-pick. Verified at fix time — CI 24/24 including the `server-stack` lane, and
the four-command gate green in an isolated worktree (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`,
1864 `librarian::` tests in the default lane).

Site 2 fixed by `4f21a6b1`, patch-id `1fc338d446cea96f992dad5009c00771febce0e1`, with
the fixture-annotation correction in `a3579710`, patch-id
`558ba7ac12550ee88f8d40af0fa5a0e1c446ab4c`.

**Observed RED at site 2, from `scripts/mutation-probe.sh` in an isolated worktree.**
Reverting `backup_db` to `std::fs::copy` reds `the_v6_backup_contains_rows_still_in_the_wal`
with `left: 0, right: 1` — and `migration_v6_creates_backup_file`, the existence-only
sibling, stays **green** in the same run (12 passed, 1 failed). That green is the
discrimination evidence: the prior coverage structurally could not see this defect, at
either site.

Gate green in a per-session target dir: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, 0 FAILED,
1875 `librarian::` tests in the default lane.

## Repro

```
journal_mode=WAL; connection held open
INSERT ...; PRAGMA wal_checkpoint(TRUNCATE);   -- everything so far -> main .db
INSERT x50                                      -- committed, lives in -wal
fs::copy(main.db, backup.db)
sqlite3 backup.db "SELECT COUNT(*)"             -- 1, not 51
```
