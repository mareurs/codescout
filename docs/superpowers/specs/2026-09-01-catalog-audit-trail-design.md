# Catalog Audit Trail — Design

**Date:** 2026-09-01 · **Status:** approved-pending-review · **Tracker:** `system-retrospective-improvements` T-1 (local WAL) + export phase (committed shards)

## Problem

The librarian catalog (`~/.local/share/librarian/catalog.db`) keeps no record of who mutated
what, when. The motivating failure is root-cause-undeterminable *by construction*:
`docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md` (zombie, high) — catalog rows
for on-disk files vanished between sessions, and no instrument can name the deleting process
once the window closes. The existing `events` table is not an audit trail: every row carries
`REFERENCES artifact(id) ON DELETE CASCADE`, so deleting an artifact deletes its history with
it — the evidence cascades away with the crime.

Requirements settled in brainstorming (2026-09-01):

1. **Capture any writer** — including raw `sqlite3` shells and foreign binary versions — via
   SQLite triggers, not a Rust-layer wrapper. A wrapper leaves out-of-band writers invisible
   and creates a standing IC-3 (declared-not-wired) risk per forgotten call site.
2. **Payload depth:** full OLD-row image on DELETE; changed-fields diff (old→new pairs) on
   UPDATE; id only on INSERT.
3. **Retention:** keep forever; manual GC only (dry-run-by-default prune verb).
4. **Phasing:** T-1 ships the local WAL + query surface; the committed-shard export is a
   follow-up phase (tracker task of its own), but the row format is designed for it NOW so
   the export lands without schema churn.

## Phase 1 — local WAL

### Schema

```sql
CREATE TABLE IF NOT EXISTS catalog_audit (
  seq        INTEGER PRIMARY KEY AUTOINCREMENT,  -- total order; a gap is a tamper signal
  at_ms      INTEGER NOT NULL,                   -- epoch-ms UTC (unit labeled per catalog-sql-hazards)
  tbl        TEXT    NOT NULL,
  op         TEXT    NOT NULL CHECK (op IN ('insert','update','delete')),
  row_id     TEXT    NOT NULL,                   -- plain TEXT, deliberately NO REFERENCES
  actor      TEXT    NOT NULL DEFAULT 'unknown', -- session id, or 'unknown' = out-of-band writer
  verb       TEXT,                               -- tool verb when known ('artifact.update', 'reindex', …)
  payload    TEXT                                -- delete: full OLD row JSON; update: changed old→new pairs; insert: NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_tbl_row ON catalog_audit(tbl, row_id, seq);
CREATE INDEX IF NOT EXISTS idx_audit_at ON catalog_audit(at_ms);
```

Design invariants:

- **No foreign keys.** Audit rows must survive the deletion of what they describe; the table
  sits entirely outside the FK cascade graph.
- **`AUTOINCREMENT`** (not bare rowid) so `seq` values are never reused; a gap in `seq` is
  itself evidence of audit-row deletion.
- **Audited tables:** `artifact`, `artifact_augmentation`, `events`, `artifact_link`,
  `entry_cite`, `commits`, `worktree_registration`.
- **Not audited:** `artifact_vec` (derived + rebuildable; vec0 virtual tables don't support
  triggers), `catalog_meta` (KV bookkeeping), `catalog_audit` itself (no recursion; protected
  by the seq-gap property instead).
- **Export-readiness (phase 2 contract):** global row identity is `(host, seq)`; `host` is
  not a column — it is supplied at export time — but `seq` semantics (monotone, never reused,
  per-catalog) are load-bearing for the shard format. The `catalog_meta` key
  `audit_exported_through_seq` is reserved.

### Capture — triggers + per-connection identity

Per audited table, three `AFTER INSERT/UPDATE/DELETE` triggers. DELETE builds the full OLD
image with `json_object(...)`; UPDATE emits only changed columns
(`CASE WHEN OLD.x IS NOT NEW.x THEN …`), so reindex's mtime/sha churn costs bytes per row,
not kilobytes.

Identity plumbing: `Catalog::open` creates
`TEMP TABLE audit_ctx(actor TEXT, verb TEXT)` and seeds `actor` from
`SessionKey::resolve` (`src/tools/session_key.rs` — existing infra, probes harness vars).
The tool layer updates `audit_ctx.verb` at dispatch. Triggers read
`COALESCE((SELECT actor FROM audit_ctx), 'unknown')`. Temp tables are per-connection, so a
codescout connection stamps a real session id + verb, while a raw `sqlite3` shell or foreign
process has no temp table and records as `'unknown'` — which is the forensic answer ("an
unidentified writer deleted 12 rows at 03:12"), not a failure. This is the negative-results
ADR applied to identity: an honest "couldn't see who" beats a plausible guess.

### Installation and the table-copy migration trap

Triggers are installed by `Catalog::open` **after** `run_migrations`, unconditionally, via
`CREATE TRIGGER IF NOT EXISTS` — every open converges the trigger set.

- Handles the documented hazard (memory `catalog-sql-hazards`): a table-copy migration's
  `DROP TABLE` silently drops that table's triggers, but the same open that ran the
  migration reinstalls them before any tool call — no cross-open self-heal window (the v9
  slug bug's failure shape).
- Migrations run before installation, so bulk table-copies don't flood the audit with
  phantom inserts.
- A schema-invariant test asserts the full trigger set exists after every legacy-seed
  migration path (pattern: `every_schema_sql_artifact_column_survives_every_migration_path`).

### Query surface

- `librarian(action="doctor")` gains an informational `audit` block: row count, time span,
  count of `actor='unknown'` rows, and (phase 2) unexported delta.
- New `librarian(action="audit_log", tbl?, row_id?, actor?, since?, until?, limit)` —
  newest-first, progressive-disclosure capped; a zero result names the window and filters it
  examined (negative-results ADR). Answers "what happened to artifact X" even after X is gone.
- Pruning: `audit_log` takes `prune_before_ms` + `confirm`; dry-run by default (doctor's fix
  convention). No automatic pruning anywhere.

### Testing (per CLAUDE.md § Testing Discipline)

- **Deliberate break / vanished-rows reproduction:** delete an artifact row via a raw second
  connection (no `audit_ctx`); assert the trail shows `delete/artifact/<id>/'unknown'` with
  the full OLD image.
- **Monotonicity:** the guard test asserts audit rows exist *with specific content*; a
  companion test drops one trigger and shows the guard fires (the trail must not be
  satisfiable by accident).
- **Per guarded site:** one mutation test per audited table — a kill on `artifact` proves
  nothing about `entry_cite`.
- **Migration paths:** trigger set survives `migrate_v6` and `widen_events_kind_check`
  table copies.
- **Identity:** codescout-connection rows carry the session id; verb stamping observed for at
  least one real tool path.

### Files touched (phase 1)

`src/librarian/catalog/schema.sql`, `src/librarian/catalog/mod.rs` (install fn + open),
new `src/librarian/catalog/audit.rs`, `src/librarian/tools/audit_log.rs` (new),
`src/librarian/tools/doctor.rs` (audit block), librarian dispatch + tool schema
(`src/librarian/tools/librarian.rs` or equivalent), `src/librarian/server.rs` wiring,
prompt-surface note for the new action.

## Phase 2 — committed audit shards (separate tracker task)

The WAL cannot itself live in git — the in-transaction guarantee exists only at mutation
time on a gitignored database. A git-native trail is structurally a **replica**, and must be
surfaced as one (else it is an IC-13: a committed log that reads as complete but is only as
fresh as its last export). Precedent: the augmentation sidecar (`f565504a`).

### Revised 2026-09-01 — what the measurement changed

This section was written from estimates. Histogramming the live trail before planning
falsified its central volume claim, and three decisions were settled against the numbers.

**The volume analysis below was aimed at the wrong term.** It proposed filtering "pure
reindex churn"; measured over a 1.74-hour window (27,914 rows), reindex churn is **0.4%** of
rows. **98.5%** were `commits` updates whose payload was literally `{}` — an UPDATE trigger
with no `WHEN` clause firing on statements that changed nothing — and **88% of payload bytes**
sat in 23 `artifact_augmentation` update rows averaging 34KB. Unfiltered, that is ~380k
rows/day into a committed file; phase 2 was not shippable on that population.

Both are fixed (T-13, `40ab56f6`), and **the fix belongs upstream rather than in the export
filter**: filtering at export would have left the local query surface unusable and the
database growing, while the trail's own `seq`-gap tamper signal stayed diluted by noise.
Post-fix, a reindex writes 20 rows instead of ~2,750, and a tracker append writes 441 chars
instead of 7,364.

**Steady state, grounded on the catalog's own 98-day history** (3,545 events ≈ 36/day, 4,446
artifacts, 3,408 links): roughly **1–2 MB/month/host**. The window this was scoped in was a
burst — an SDD run with nine commits, several reindexes and a merge — and is not a rate.

### Revised 2026-09-02 — the catalog is MACHINE-WIDE, and this section assumed otherwise

Everything above was written treating the catalog as per-repo. It is not. There is one SQLite
file at `dirs::data_local_dir()/librarian/catalog.db` spanning every workspace root on the
machine, so `catalog_audit` holds rows for artifacts in **every** repo. Measured 2026-09-02 on
the author's machine: **54,304 audit rows across 8 repositories**, of which 4,685 commit rows
are codescout's — the rest belong to unrelated projects, three of them client work.

Composed with the design above, that produced a Critical defect, found by Task 4's review:

- `export`'s row selection is `WHERE seq > ?1` with **no repo predicate**, and
  `audit_exported_through_seq` is a **single global key**.
- The destination is the **active project's** root.

So the first `reindex` in any repo drains the machine's entire audit backlog into that repo and
advances the global watermark — leaving every other repo's committed shard permanently and
silently short. That is this document's own IC-13, on the write side, inside the feature built
to prevent it.

Two consequences worth stating separately, because each has a different remedy:

1. **Sub-projects.** The destination is `current_project.abs_path`, not `git_root`. Activating
   a registered sub-project (`crates/codescout-embed` is one, in this very repo) writes shards
   to a path the repo-root `.gitattributes`/`.gitignore` entries do not cover and
   `read_shards(repo_root)` never reads back.
2. **Data exposure.** `delete` payloads are full OLD-row images, and `artifact`'s columns
   include `abs_path`. A committed shard could therefore publish absolute filesystem paths and
   artifact metadata for unrelated — including private — repositories into this repo's git
   history. Outward-facing and effectively irreversible once pushed.

**Adjudicated 2026-09-02: repo scoping is the whole remedy.** A separate redaction pass over
committed payloads was considered and rejected — `abs_path` is frequently *how* a reader
identifies what was deleted, so redacting it costs the forensic value the trail exists for,
and it would be a second mechanism guarding a hole the first one closes. Correctness and
privacy have the same fix here.

### Scoping design (supersedes the row-selection and watermark rules above)

- **The watermark is per-repo**, keyed by the repo root:
  `audit_exported_through_seq:<git_root>`. The global key is superseded; a catalog holding the
  old unkeyed value must not have it read as any repo's watermark.
- **The destination is `git_root`, never `abs_path`.** `CurrentProject` carries both precisely
  because they differ for sub-projects.
- **Rows are attributed to a repo through the artifact they hang off**, since every audited
  table except `commits` (already excluded) and `worktree_registration` ultimately references
  one: resolve the audit row to an artifact id, then to `artifact.abs_path`, then prefix-match
  the repo root on a component boundary (`/repo-backup` must not match `/repo`).
- **A `delete` row's artifact is gone by definition**, so its attribution comes from the
  `abs_path` inside its own payload image.
- **Attribution can genuinely fail** — an `events` row whose artifact was deleted earlier, for
  instance. Those rows are **reported as `unattributed`, never guessed and never silently
  dropped.** An unattributed row stays unexported and stays past its repo's watermark, so it is
  recoverable once attribution improves; a guessed one is a wrong row in a committed file.
- **The cross-machine end-to-end test must use a multi-repo fixture.** A single-repo fixture
  makes this entire class unrepresentable, which is how the original design passed its own
  review.
### Settled design

- **`audit_log export`** appends rows since the watermark to
  `.codescout/audit/<host>-<YYYYMM>.jsonl` — one file per (host, month), append-only,
  committed. `.codescout/` has no blanket ignore rule (it already tracks
  `/.codescout/projects/`), so the directory is tracked by default; an explicit
  `.gitignore` comment records that this is deliberate rather than an oversight.
- **Host identity is persisted, never re-derived.** `catalog_meta['audit_host_id']`, resolved
  **once** from `CODESCOUT_AUDIT_HOST` → `COMPUTERNAME` → `/etc/hostname` → `HOSTNAME`,
  sanitized to `[a-z0-9-]{1,24}`, and suffixed with 6 hex chars. The catalog is machine-local,
  so a value stored in it *is* a host identity by construction; the suffix is what keeps two
  machines that both call themselves `arch` from writing the same file. No new dependency —
  `gethostname` was considered and rejected as not worth a crate for a value we must persist
  anyway. A readable prefix is a courtesy; the suffix is the correctness.
- **Read path: merge-on-query, stateless.** `audit_log` reads the local `catalog_audit` and
  streams the shard files, merging on `(at_ms, host, seq)` — never on line position. There is
  no import and no second table. An imported replica is exactly the two-representations-one-
  truth shape T-6 exists to remove, and it would add a sync step that can silently not have
  run. Shard filenames encode host and `YYYYMM`, so `since`/`until` prunes whole files before
  opening them.
- **Shard scope: every audited table except `commits`.** `commits` is a cache of git, so
  exporting an audit of it *into* git is circular — and at ~192 rows/day it is the largest
  remaining term. It stays audited locally.
- **Merge conflicts solved structurally:** different machines write different files; same
  host on two branches gets `.gitattributes` `merge=union`, correct for append-only line logs.
- **Honesty markers.** Each export stamps `audit_exported_through_seq` (catalog_meta + shard
  header); doctor reports the unexported delta; a merged read labels each host's coverage
  window. **`filtered_total` and `truncated` must count shard rows too, or say they do not** —
  a merged query whose total silently reflects only the local table is this document's own
  IC-13, committed inside the feature that exists to avoid it.
- **Not forgettable:** incremental export folds into surfaces that already run (`reindex`,
  `merge_worktree`) plus the manual verb. Concurrent appends take the `fs4` file lock already
  used by `src/retrieval/index_lock.rs`; two sessions reindexing at once must not interleave
  partial lines into a committed file.
- Payoff: after a pull, `audit_log` answers "which session on the other machine deleted
  these rows" — the vanished-rows question across the machine boundary.
## Rejected alternatives

- **Rust-layer audit wrapper** — rich identity, but out-of-band writers invisible; standing
  IC-3 risk per forgotten call site.
- **Full before/after images on every op** — reindex churn × ~1KB artifact rows (and 25KB
  augmentation params) for no forensic gain over diffs.
- **Auto-pruning by age** — an old mystery can outlive its evidence; the motivating bug was
  noticed days late.
- **Git as the primary trail** — cannot carry the in-transaction guarantee; kept as replica
  only (phase 2).
