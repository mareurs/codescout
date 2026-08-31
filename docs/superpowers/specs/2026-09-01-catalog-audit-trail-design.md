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

- `audit_log export` appends rows since the watermark to
  `.codescout/audit/<host>-<YYYYMM>.jsonl` — one file per (host, month), append-only,
  committed.
- **Merge conflicts solved structurally:** different machines write different files; same
  host on two branches gets `.gitattributes` `merge=union` (correct for append-only line
  logs; global order is re-derived from `(host, seq, at_ms)`, never from line position).
- **Volume:** exports filter pure reindex churn by default (update rows whose changed-set ⊆
  `{file_mtime, file_sha256, updated_at, missing_since}`); everything stays queryable
  locally.
- **Honesty markers:** each export stamps `audit_exported_through_seq` (catalog_meta + shard);
  doctor reports the unexported delta; merged reads label each host's coverage window.
- **Not forgettable:** incremental export folds into surfaces that already run (`reindex`,
  `merge_worktree`) plus the manual verb; a pre-commit hook is possible later (H-N
  territory, out of scope here).
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
