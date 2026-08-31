# Catalog Audit Trail (T-1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every mutation to the librarian catalog leaves an in-transaction audit row that survives the deletion of what it describes, with writer identity when available and `'unknown'` as a first-class forensic answer.

**Architecture:** A `catalog_audit` table outside the FK graph, populated by main-schema triggers on 7 audited tables (rebuilt from live `PRAGMA table_info` on every `Catalog::open`, so table-copy migrations cannot orphan them); identity via a per-connection TEMP stamping trigger + `audit_ctx` temp table; query/prune via a new `librarian(action="audit_log")` plus a doctor health block.

**Tech Stack:** rusqlite (bundled SQLite, JSON1), serde_json, existing librarian tool dispatch.

**Spec:** `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` — **with one probe-validated amendment** (see Design Correction below).

## Design Correction (probed 2026-09-01, supersedes spec §"Capture")

The spec says triggers read `COALESCE((SELECT actor FROM audit_ctx), 'unknown')`. **Do not build that.** Probed on SQLite (scratch DB, three connections):

- A main-schema trigger referencing the temp table is **created silently**, then **fails every foreign writer's mutation** at fire time (`Parse error: no such table: main.audit_ctx`; the UPDATE is refused, row unchanged). That blocks out-of-band writers instead of recording them — the exact inversion of the requirement.
- The working mechanism (probe-confirmed): main triggers INSERT audit rows with `actor` **defaulting to `'unknown'`**; each codescout connection additionally creates a **TEMP trigger** `AFTER INSERT ON catalog_audit` that re-stamps `actor`/`verb` from the connection's temp `audit_ctx` table. Temp triggers fire only for their own connection, may reference both temp and main objects, and DO fire when the insert comes from inside another (main) trigger, with default pragmas.
- Failure direction is the safe one by construction: no temp objects → rows stay `'unknown'`; nothing is ever blocked or mis-attributed.

## Global Constraints

- Gate before completing any task-set: `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, `cargo test --workspace` — **lean test lane third, default lane last; the order is load-bearing** (CLAUDE.md § Development Commands).
- All new code lives under `src/librarian/` (feature-gated `librarian`); it must not be reachable from lean builds. Tests live in the same gated modules.
- **Shared checkout: every commit uses the pathspec composition** — `git add <paths>` then `git commit -m "..." -- <same paths>`. Never a bare `git commit` (a bare commit sweeps peers' staged work; it happened tonight — `3a422b31`).
- Errors: input-driven failures use `crate::librarian::tools::RecoverableError::new/with_hint` (`src/librarian/tools/mod.rs:69`); real failures use `anyhow`.
- Zero results name their scope (`docs/adrs/2026-08-27-negative-results-name-their-scope.md`).
- No FK, no `REFERENCES`, on `catalog_audit` — audit rows must survive their subjects.
- `seq` is `INTEGER PRIMARY KEY AUTOINCREMENT` (never-reused; a gap is a tamper signal). `catalog_meta` key `audit_exported_through_seq` is reserved for phase 2 (T-7) — do not use it here.
- Timestamps are epoch-**ms** UTC; label the unit in any output (memory `catalog-sql-hazards`).

## File Structure

- `src/librarian/catalog/audit.rs` — NEW: table DDL, trigger builder (from `PRAGMA table_info`), session stamping, query, prune, health. One responsibility: the audit subsystem.
- `src/librarian/catalog/mod.rs` — module decl; `install` + `install_session` calls in all three constructors; `Catalog::set_audit_verb`.
- `src/librarian/catalog/schema.sql` — documentation-only comment (table is created by `audit::install`, NOT here — see Task 1 rationale).
- `src/librarian/tools/audit_log.rs` — NEW: the `audit_log` action (query + prune).
- `src/librarian/tools/librarian.rs` — dispatch arm + input_schema + description.
- `src/librarian/tools/artifact.rs`, `src/librarian/tools/librarian.rs` — verb stamping at dispatch.
- `src/librarian/tools/doctor.rs` — `audit` block in `catalog_health`.
- `src/librarian/tools/mod.rs` — `pub mod audit_log;`.

---

### Task 1: `catalog_audit` table + capture triggers, installed on every open

**Files:**
- Create: `src/librarian/catalog/audit.rs`
- Modify: `src/librarian/catalog/mod.rs` (add `pub(crate) mod audit;` beside the other module decls at :7-20; add install calls in `open` :411-430, `open_in_memory` :432-442, `open_with_workspace` :444-470)
- Test: inline `#[cfg(test)] mod tests` in `audit.rs`

**Interfaces:**
- Consumes: `Catalog` (`src/librarian/catalog/mod.rs:32`, field `conn: Connection`), `run_migrations`.
- Produces: `audit::install(conn: &Connection) -> anyhow::Result<()>`; `audit::AUDITED_TABLES: &[AuditedTable]` with `pub(crate) struct AuditedTable { name: &'static str, row_id_new: &'static str, row_id_old: &'static str }`. Later tasks rely on table columns `seq, at_ms, tbl, op, row_id, actor, verb, payload`.

- [ ] **Step 1: Write the failing tests** (in `audit.rs`'s tests module; `Catalog::open_in_memory()` per the house pattern, e.g. `src/librarian/catalog/events.rs:206`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, Catalog};

    fn seed(cat: &Catalog, id: &str) {
        let row = artifact::TestArtifactRowBuilder::new(id).build();
        artifact::upsert(&cat.conn, &row).unwrap();
    }

    fn audit_rows(cat: &Catalog) -> Vec<(String, String, String, String, Option<String>)> {
        let mut stmt = cat.conn.prepare(
            "SELECT tbl, op, row_id, actor, payload FROM catalog_audit ORDER BY seq").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    }

    #[test]
    fn insert_update_delete_on_artifact_each_leave_an_audit_row() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn.execute("UPDATE artifact SET status='archived' WHERE id='a1'", []).unwrap();
        cat.conn.execute("DELETE FROM artifact WHERE id='a1'", []).unwrap();
        let rows = audit_rows(&cat);
        assert_eq!(rows.len(), 3, "one audit row per mutation, got {rows:?}");
        assert_eq!((rows[0].0.as_str(), rows[0].1.as_str()), ("artifact", "insert"));
        assert_eq!(rows[0].2, "a1");
        assert!(rows[0].4.is_none(), "insert carries no payload");
        // update payload: changed columns only, as {"col": [old, new]}
        let diff: serde_json::Value =
            serde_json::from_str(rows[1].4.as_deref().unwrap()).unwrap();
        assert_eq!(diff["status"], serde_json::json!(["draft", "archived"]));
        assert!(diff.get("title").is_none(), "unchanged column must be absent: {diff}");
        // delete payload: full OLD image
        let img: serde_json::Value =
            serde_json::from_str(rows[2].4.as_deref().unwrap()).unwrap();
        assert_eq!(img["id"], "a1");
        assert!(img.get("abs_path").is_some(), "full image carries every column");
    }

    // Per guarded SITE, not per feature (CLAUDE.md Testing Discipline): every audited
    // table proves its own delete trigger fires with a payload.
    #[test]
    fn every_audited_table_captures_a_delete_with_old_image() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn.execute("UPDATE artifact SET slug='a-one' WHERE id='a1'", []).unwrap();
        // one row per table, minimal columns
        cat.conn.execute("INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES('a1','a1','cites',1)", []).unwrap();
        cat.conn.execute("INSERT INTO events(id,artifact_id,kind,payload,created_at) VALUES('e1','a1','note','{}',1)", []).unwrap();
        cat.conn.execute("INSERT INTO artifact_augmentation(artifact_id,prompt) VALUES('a1','p')", []).unwrap();
        cat.conn.execute("INSERT INTO entry_cite(src_slug,src_local,dst_ref,rel,created_at) VALUES('a-one','F-1','x','cites',1)", []).unwrap();
        cat.conn.execute("INSERT INTO commits(hash,git_root) VALUES('h1','/r')", []).unwrap();
        cat.conn.execute("INSERT INTO worktree_registration(worktree_root,main_root,created_at) VALUES('/w','/m',1)", []).unwrap();
        cat.conn.execute("PRAGMA foreign_keys=OFF", []).unwrap(); // isolate per-table deletes from cascades
        for t in super::AUDITED_TABLES {
            cat.conn.execute(&format!("DELETE FROM {}", t.name), []).unwrap();
            let n: i64 = cat.conn.query_row(
                "SELECT count(*) FROM catalog_audit WHERE tbl=?1 AND op='delete' AND payload IS NOT NULL",
                [t.name], |r| r.get(0)).unwrap();
            assert!(n >= 1, "no delete audit row with payload for table {}", t.name);
        }
    }

    // Deliberate break (CLAUDE.md Testing Discipline): the rows come from the
    // triggers and nowhere else — drop them and the silence proves it.
    #[test]
    fn dropping_the_triggers_stops_capture() {
        let cat = Catalog::open_in_memory().unwrap();
        let names: Vec<String> = {
            let mut s = cat.conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='trigger' AND name LIKE 'audit_%'").unwrap();
            let v = s.query_map([], |r| r.get(0)).unwrap()
                .collect::<rusqlite::Result<Vec<String>>>().unwrap(); v
        };
        assert!(!names.is_empty(), "install() must have created audit_ triggers");
        for n in &names {
            cat.conn.execute(&format!("DROP TRIGGER {n}"), []).unwrap();
        }
        seed(&cat, "a2");
        let n: i64 = cat.conn.query_row("SELECT count(*) FROM catalog_audit", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "with triggers dropped, no capture — proves triggers are the mechanism");
    }

    // Table-copy migrations drop a table's triggers with the table
    // (memory catalog-sql-hazards); every open reconverges the set.
    #[test]
    fn reopen_reinstalls_triggers_after_a_manual_drop() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("c.db");
        {
            let cat = Catalog::open(&db).unwrap();
            cat.conn.execute("DROP TRIGGER audit_artifact_delete", []).unwrap();
        }
        let cat = Catalog::open(&db).unwrap();
        seed(&cat, "a3");
        cat.conn.execute("DELETE FROM artifact WHERE id='a3'", []).unwrap();
        let n: i64 = cat.conn.query_row(
            "SELECT count(*) FROM catalog_audit WHERE tbl='artifact' AND op='delete'",
            [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "reopen must have reinstalled the dropped trigger");
    }

    #[test]
    fn audit_rows_survive_artifact_deletion_no_fk() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "gone");
        cat.conn.execute("DELETE FROM artifact WHERE id='gone'", []).unwrap();
        let n: i64 = cat.conn.query_row(
            "SELECT count(*) FROM catalog_audit WHERE row_id='gone'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2, "insert+delete rows outlive the artifact row");
    }
}
```

Note for the implementer: `TestArtifactRowBuilder::new` (`src/librarian/catalog/artifact.rs:44`) — check its default `status` in the builder body; the diff assertion above assumes `"draft"`. If the builder defaults differently, set it explicitly with `.with_status("draft")`. Do not weaken the assertion to "some diff exists" — the exact `[old, new]` shape is the contract.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout catalog::audit -- --nocapture`
Expected: compile error — `audit` module does not exist yet.

- [ ] **Step 3: Implement `src/librarian/catalog/audit.rs`**

```rust
//! Append-only catalog audit trail (T-1, spec: docs/superpowers/specs/
//! 2026-09-01-catalog-audit-trail-design.md + this plan's Design Correction).
//!
//! Main-schema triggers capture every writer's mutations with actor DEFAULT
//! 'unknown'; a per-connection TEMP trigger (install_session, Task 2) enriches
//! this connection's rows. NEVER reference temp objects from the main triggers:
//! probed 2026-09-01 — it creates silently, then REFUSES foreign writers'
//! mutations at fire time ("no such table: main.audit_ctx").

use anyhow::Result;
use rusqlite::Connection;

/// Tables under audit. Columns are read from live PRAGMA table_info at
/// install time, so migration-added columns are always covered and a
/// column-list drift can never break an open. row_id exprs are the one
/// per-table static: a composite key flattened to TEXT.
pub(crate) struct AuditedTable {
    pub name: &'static str,
    pub row_id_new: &'static str,
    pub row_id_old: &'static str,
}

pub(crate) const AUDITED_TABLES: &[AuditedTable] = &[
    AuditedTable { name: "artifact", row_id_new: "NEW.id", row_id_old: "OLD.id" },
    AuditedTable { name: "artifact_augmentation", row_id_new: "NEW.artifact_id", row_id_old: "OLD.artifact_id" },
    AuditedTable { name: "events", row_id_new: "NEW.id", row_id_old: "OLD.id" },
    AuditedTable {
        name: "artifact_link",
        row_id_new: "NEW.src_id || '→' || NEW.dst_id || ':' || NEW.rel",
        row_id_old: "OLD.src_id || '→' || OLD.dst_id || ':' || OLD.rel",
    },
    AuditedTable {
        name: "entry_cite",
        row_id_new: "NEW.src_slug || ':' || NEW.src_local || '→' || NEW.dst_ref",
        row_id_old: "OLD.src_slug || ':' || OLD.src_local || '→' || OLD.dst_ref",
    },
    AuditedTable { name: "commits", row_id_new: "NEW.hash", row_id_old: "OLD.hash" },
    AuditedTable { name: "worktree_registration", row_id_new: "NEW.worktree_root", row_id_old: "OLD.worktree_root" },
];

/// Epoch-ms UTC, computed SQL-side so foreign-writer rows get real times too.
const NOW_MS: &str = "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)";

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    anyhow::ensure!(!cols.is_empty(), "audited table {table} not found — install() must run after migrations");
    Ok(cols)
}

/// json_object('c1', OLD.c1, 'c2', OLD.c2, ...) — full row image.
fn old_image_expr(cols: &[String]) -> String {
    let pairs = cols.iter()
        .map(|c| format!("'{c}', OLD.\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("json_object({pairs})")
}

/// Changed-columns diff: {"col": [old, new], ...} via a json_patch fold.
fn update_diff_expr(cols: &[String]) -> String {
    let mut expr = String::from("'{}'");
    for c in cols {
        expr = format!(
            "json_patch({expr}, CASE WHEN OLD.\"{c}\" IS NOT NEW.\"{c}\" \
             THEN json_object('{c}', json_array(OLD.\"{c}\", NEW.\"{c}\")) ELSE '{{}}' END)"
        );
    }
    expr
}

/// Create the audit table (idempotent) and drop+recreate all capture triggers
/// from the LIVE schema. Called by every Catalog constructor AFTER
/// run_migrations: a table-copy migration silently drops the table's triggers
/// (memory catalog-sql-hazards), and the same open that ran the migration
/// reinstalls them here — no cross-open self-heal window. Drop+create (not
/// IF NOT EXISTS) so a definition change or column add always converges.
pub(crate) fn install(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS catalog_audit (
           seq     INTEGER PRIMARY KEY AUTOINCREMENT,
           at_ms   INTEGER NOT NULL,
           tbl     TEXT NOT NULL,
           op      TEXT NOT NULL CHECK (op IN ('insert','update','delete')),
           row_id  TEXT NOT NULL,
           actor   TEXT NOT NULL DEFAULT 'unknown',
           verb    TEXT,
           payload TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_audit_tbl_row ON catalog_audit(tbl, row_id, seq);
         CREATE INDEX IF NOT EXISTS idx_audit_at ON catalog_audit(at_ms);",
    )?;
    for t in AUDITED_TABLES {
        let cols = table_columns(conn, t.name)?;
        let (name, image, diff) = (t.name, old_image_expr(&cols), update_diff_expr(&cols));
        conn.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS audit_{name}_insert;
             CREATE TRIGGER audit_{name}_insert AFTER INSERT ON \"{name}\" BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id)
               VALUES({NOW_MS}, '{name}', 'insert', {rid_new});
             END;
             DROP TRIGGER IF EXISTS audit_{name}_update;
             CREATE TRIGGER audit_{name}_update AFTER UPDATE ON \"{name}\" BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id, payload)
               VALUES({NOW_MS}, '{name}', 'update', {rid_new}, {diff});
             END;
             DROP TRIGGER IF EXISTS audit_{name}_delete;
             CREATE TRIGGER audit_{name}_delete AFTER DELETE ON \"{name}\" BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id, payload)
               VALUES({NOW_MS}, '{name}', 'delete', {rid_old}, {image});
             END;",
            rid_new = t.row_id_new,
            rid_old = t.row_id_old,
        ))?;
    }
    Ok(())
}
```

- [ ] **Step 4: Wire into the three constructors** (`src/librarian/catalog/mod.rs`)

Add `pub(crate) mod audit;` next to the existing module decls (:7-20). In each of `open` (:411), `open_in_memory` (:432), `open_with_workspace` (:444), insert **after** `run_migrations(...)` (and in `open_with_workspace`, after the `drop_legacy_and_stamp` block) and **before** the `DELETE FROM artifact_vec` cleanup:

```rust
audit::install(&conn).context("installing audit triggers")?;
```

Also append to `src/librarian/catalog/schema.sql` (documentation only — same convention as the ux_artifact_slug comment at the top of that file):

```sql
-- catalog_audit + its triggers are NOT created here: audit::install (audit.rs)
-- creates them on every open, AFTER migrations, rebuilding triggers from live
-- PRAGMA table_info so table-copy migrations can never orphan them. See
-- docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md.
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p codescout catalog::audit -- --nocapture`
Expected: 5 PASS. Also run `cargo test -p codescout catalog::` — the existing catalog suite must stay green (migration idempotency tests re-open catalogs and now exercise install()).

- [ ] **Step 6: Commit (pathspec form)**

```bash
git add src/librarian/catalog/audit.rs src/librarian/catalog/mod.rs src/librarian/catalog/schema.sql
git commit -m "feat(librarian): catalog_audit capture triggers, installed on every open" -- src/librarian/catalog/audit.rs src/librarian/catalog/mod.rs src/librarian/catalog/schema.sql
```

---

### Task 2: identity — session actor + TEMP stamping trigger + verb from dispatchers

**Files:**
- Modify: `src/librarian/catalog/audit.rs` (add `resolve_actor`, `install_session`), `src/librarian/catalog/mod.rs` (constructor calls + `Catalog::set_audit_verb`), `src/librarian/tools/artifact.rs:207-230` and `src/librarian/tools/librarian.rs:111-131` (verb stamp)
- Test: `audit.rs` tests + one dispatcher test in `src/librarian/tools/artifact.rs` tests

**Interfaces:**
- Consumes: `crate::tools::session_key::{resolve, SessionKey, HARNESS_SESSION_VARS}` (`src/tools/session_key.rs:36,40` — `resolve(explicit: Option<String>, harness: impl IntoIterator<Item=(&str, String)>)`); Task 1's `catalog_audit` columns.
- Produces: `audit::resolve_actor() -> String` (`"codescout:<id>"` or `"codescout:anonymous"`); `audit::install_session(conn: &Connection, actor: &str) -> anyhow::Result<()>`; `Catalog::set_audit_verb(&self, verb: &str) -> rusqlite::Result<()>`.

- [ ] **Step 1: Write the failing tests** (append to `audit.rs` tests)

```rust
#[test]
fn codescout_connection_rows_are_stamped_with_the_session_actor() {
    let cat = Catalog::open_in_memory().unwrap();
    // Deterministic: re-seed the session with a known actor (idempotent —
    // install_session replaces the audit_ctx row and the temp trigger).
    super::install_session(&cat.conn, "codescout:test-session").unwrap();
    seed(&cat, "s1");
    let actor: String = cat.conn.query_row(
        "SELECT actor FROM catalog_audit ORDER BY seq DESC LIMIT 1", [], |r| r.get(0)).unwrap();
    assert_eq!(actor, "codescout:test-session");
}

// THE vanished-rows reproduction (docs/issues/2026-08-25-sdd-ledger-and-
// catalog-rows-vanished.md): a raw second connection deletes a row; the trail
// answers with op, full OLD image, and actor 'unknown'.
#[test]
fn foreign_connection_mutations_record_as_unknown_with_full_image() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("c.db");
    let cat = Catalog::open(&db).unwrap();
    seed(&cat, "victim");
    let foreign = rusqlite::Connection::open(&db).unwrap(); // no install_session
    foreign.execute("PRAGMA busy_timeout=5000", []).unwrap();
    foreign.execute("DELETE FROM artifact WHERE id='victim'", []).unwrap();
    let (actor, payload): (String, String) = cat.conn.query_row(
        "SELECT actor, payload FROM catalog_audit WHERE op='delete' AND row_id='victim'",
        [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(actor, "unknown", "a writer that did not identify itself is 'unknown' — that IS the answer");
    let img: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(img["id"], "victim", "the deleted row is reconstructible from the trail alone");
}

// Deliberate break for the stamping path: without the temp trigger the system
// degrades to 'unknown' — never blocks, never mis-attributes (probe-pinned
// failure direction; see module doc).
#[test]
fn dropping_the_stamp_trigger_degrades_to_unknown_not_to_an_error() {
    let cat = Catalog::open_in_memory().unwrap();
    cat.conn.execute("DROP TRIGGER audit_stamp", []).unwrap();
    seed(&cat, "u1");
    let actor: String = cat.conn.query_row(
        "SELECT actor FROM catalog_audit ORDER BY seq DESC LIMIT 1", [], |r| r.get(0)).unwrap();
    assert_eq!(actor, "unknown");
}

#[test]
fn set_audit_verb_reaches_subsequent_rows() {
    let cat = Catalog::open_in_memory().unwrap();
    cat.set_audit_verb("artifact.update").unwrap();
    seed(&cat, "v1");
    let verb: Option<String> = cat.conn.query_row(
        "SELECT verb FROM catalog_audit ORDER BY seq DESC LIMIT 1", [], |r| r.get(0)).unwrap();
    assert_eq!(verb.as_deref(), Some("artifact.update"));
}
```

And in `src/librarian/tools/artifact.rs` tests (house pattern: `tests/mk_ctx` :239, route tests :244-304):

```rust
#[tokio::test]
async fn dispatch_stamps_the_audit_verb() {
    let ctx = mk_ctx();
    // find is read-only; the stamp happens at dispatch regardless of verb kind
    let _ = crate::librarian::tools::artifact::Artifact
        .call(&ctx, serde_json::json!({"action": "find"})).await;
    let verb: Option<String> = ctx.catalog.lock().conn.query_row(
        "SELECT verb FROM audit_ctx", [], |r| r.get(0)).unwrap();
    assert_eq!(verb.as_deref(), Some("artifact.find"));
}
```

(If `mk_ctx` builds its `Catalog` via a path that skips the constructors, use `TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())` — the in-memory constructor installs everything.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout catalog::audit -- --nocapture`
Expected: compile error — `install_session` / `set_audit_verb` not defined.

- [ ] **Step 3: Implement** (append to `audit.rs`)

```rust
/// "codescout:<session-id>" for an identified connection, "codescout:anonymous"
/// for a codescout connection with no harness id. NEVER returns "unknown" —
/// 'unknown' is reserved for writers that did not identify themselves at all
/// (foreign processes), and the distinction is the forensic value.
pub(crate) fn resolve_actor() -> String {
    use crate::tools::session_key::{resolve, SessionKey, HARNESS_SESSION_VARS};
    let harness = HARNESS_SESSION_VARS
        .iter()
        .filter_map(|v| std::env::var(v).ok().map(|val| (*v, val)));
    match resolve(std::env::var("CODESCOUT_SESSION_ID").ok(), harness) {
        SessionKey::Keyed { .. } | SessionKey::Anonymous => { /* see note below */ }
    }
    // NOTE to implementer: match on the real enum shape — SessionKey (:17-22)
    // has Keyed and Anonymous; use its id() accessor (:26-31):
    let key = resolve(
        std::env::var("CODESCOUT_SESSION_ID").ok(),
        HARNESS_SESSION_VARS.iter().filter_map(|v| std::env::var(v).ok().map(|val| (*v, val))),
    );
    match key.id() {
        Some(id) => format!("codescout:{id}"),
        None => "codescout:anonymous".to_string(),
    }
}

/// Per-connection identity: audit_ctx temp table + TEMP stamping trigger.
/// Temp objects are invisible to other connections; the trigger fires only for
/// this connection's inserts into catalog_audit (probe-validated, including
/// nested firing from inside the main capture triggers). Idempotent: replaces
/// both the row and the trigger.
pub(crate) fn install_session(conn: &Connection, actor: &str) -> Result<()> {
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS audit_ctx(actor TEXT NOT NULL, verb TEXT);
         DELETE FROM audit_ctx;",
    )?;
    conn.execute("INSERT INTO audit_ctx(actor, verb) VALUES(?1, NULL)", [actor])?;
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS audit_stamp;
         CREATE TEMP TRIGGER audit_stamp AFTER INSERT ON catalog_audit BEGIN
           UPDATE catalog_audit
              SET actor = COALESCE((SELECT actor FROM audit_ctx), 'unknown'),
                  verb  = (SELECT verb FROM audit_ctx)
            WHERE seq = NEW.seq;
         END;",
    )?;
    Ok(())
}
```

Delete the dead first `match` in `resolve_actor` above — it exists in this plan only to warn about the enum shape; the final function is the `key.id()` form. In `mod.rs`, add to `impl Catalog` (:410-471):

```rust
/// Best-effort verb tag for subsequent audit rows on this connection.
/// The verb persists until the next stamp — it means "last dispatched verb",
/// not "verb of this exact statement"; audit_log documents this.
pub fn set_audit_verb(&self, verb: &str) -> rusqlite::Result<()> {
    self.conn.execute("UPDATE audit_ctx SET verb = ?1", [verb]).map(|_| ())
}
```

In each constructor, directly after the `audit::install(&conn)` call:

```rust
audit::install_session(&conn, &audit::resolve_actor()).context("installing audit session")?;
```

- [ ] **Step 4: Stamp verbs at the two dispatchers**

`src/librarian/tools/artifact.rs` `call` (:207) — after `action` is parsed, before the `match`:

```rust
// Best-effort: identity enrichment must never fail a tool call; a failed
// stamp degrades the row to verb=NULL, which audit_log surfaces honestly.
if let Err(e) = ctx.catalog.lock().set_audit_verb(&format!("artifact.{action}")) {
    tracing::warn!("audit verb stamp failed: {e}");
}
```

Same in `src/librarian/tools/librarian.rs` `call` (:111) with `format!("librarian.{action}")`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p codescout catalog::audit -- --nocapture && cargo test -p codescout tools::artifact::tests::dispatch_stamps`
Expected: PASS ×5 new. Then the full librarian suite: `cargo test -p codescout librarian` — green.

- [ ] **Step 6: Commit (pathspec form)**

```bash
git add src/librarian/catalog/audit.rs src/librarian/catalog/mod.rs src/librarian/tools/artifact.rs src/librarian/tools/librarian.rs
git commit -m "feat(librarian): audit identity — session actor, temp stamp trigger, dispatch verbs" -- src/librarian/catalog/audit.rs src/librarian/catalog/mod.rs src/librarian/tools/artifact.rs src/librarian/tools/librarian.rs
```

---

### Task 3: `librarian(action="audit_log")` — query + prune

**Files:**
- Create: `src/librarian/tools/audit_log.rs`
- Modify: `src/librarian/catalog/audit.rs` (query/prune fns), `src/librarian/tools/mod.rs` (module decl), `src/librarian/tools/librarian.rs` (arm + schema + description + both error strings)
- Test: inline in both new/modified files

**Interfaces:**
- Consumes: Task 1/2 surfaces; dynamic-SQL pattern of `timeline_for_artifact` (`src/librarian/catalog/events.rs:94-130`); `RecoverableError` (`src/librarian/tools/mod.rs:69`).
- Produces: `audit::AuditRow { seq: i64, at_ms: i64, tbl: String, op: String, row_id: String, actor: String, verb: Option<String>, payload: Option<String> }`; `audit::query(conn, &AuditFilter, limit) -> Result<Vec<AuditRow>>` with `pub(crate) struct AuditFilter { tbl: Option<String>, row_id: Option<String>, actor: Option<String>, since: Option<i64>, until: Option<i64> }`; `audit::prune_before(conn, before_ms) -> Result<usize>`; tool fn `audit_log::call(ctx, args) -> Result<Value>`.

- [ ] **Step 1: Write the failing tests**

In `audit.rs` tests:

```rust
#[test]
fn query_filters_compose_and_order_newest_first() {
    let cat = Catalog::open_in_memory().unwrap();
    seed(&cat, "q1");
    cat.conn.execute("DELETE FROM artifact WHERE id='q1'", []).unwrap();
    let all = super::query(&cat.conn, &super::AuditFilter::default(), 50).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all[0].seq > all[1].seq, "newest first");
    let f = super::AuditFilter { op: Some("delete".into()), ..Default::default() };
    let dels = super::query(&cat.conn, &f, 50).unwrap();
    assert_eq!(dels.len(), 1);
    assert_eq!(dels[0].row_id, "q1");
}

#[test]
fn prune_deletes_old_rows_and_leaves_a_self_describing_marker() {
    let cat = Catalog::open_in_memory().unwrap();
    seed(&cat, "p1");
    let removed = super::prune_before(&cat.conn, i64::MAX).unwrap();
    assert_eq!(removed, 1);
    let rows = super::query(&cat.conn, &super::AuditFilter::default(), 50).unwrap();
    // the marker row explains the seq gap: tbl catalog_audit, op delete
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tbl, "catalog_audit");
    let p: serde_json::Value = serde_json::from_str(rows[0].payload.as_deref().unwrap()).unwrap();
    assert_eq!(p["pruned"], 1);
}
```

In `audit_log.rs` tests (route + ADR behavior; mirror `librarian.rs` test shapes :141-161):

```rust
#[tokio::test]
async fn zero_results_name_their_scope() {
    let ctx = mk_ctx();
    let out = call(&ctx, serde_json::json!({"action": "audit_log", "tbl": "commits"})).await.unwrap();
    assert_eq!(out["entries"].as_array().unwrap().len(), 0);
    let scope = &out["scope"];
    assert_eq!(scope["tbl"], "commits", "a zero says what was examined (negative-results ADR)");
    assert!(out["unit"].as_str().unwrap().contains("ms"), "timestamps label their unit");
}

#[tokio::test]
async fn prune_is_dry_run_without_confirm() {
    let ctx = mk_ctx();
    { // one row to prune
        let cat = ctx.catalog.lock();
        cat.conn.execute(
            "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(1,'artifact','insert','x')", []).unwrap();
    }
    let dry = call(&ctx, serde_json::json!({"action":"audit_log","prune_before_ms": 10})).await.unwrap();
    assert_eq!(dry["would_delete"], 1);
    assert!(dry.get("deleted").is_none());
    let wet = call(&ctx, serde_json::json!({"action":"audit_log","prune_before_ms": 10, "confirm": true})).await.unwrap();
    assert_eq!(wet["deleted"], 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout audit`
Expected: compile errors (query/AuditFilter/prune_before/audit_log missing).

- [ ] **Step 3: Implement query/prune in `audit.rs`**

```rust
#[derive(Default)]
pub(crate) struct AuditFilter {
    pub tbl: Option<String>,
    pub row_id: Option<String>,
    pub actor: Option<String>,
    pub op: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
}

pub(crate) struct AuditRow {
    pub seq: i64, pub at_ms: i64, pub tbl: String, pub op: String,
    pub row_id: String, pub actor: String, pub verb: Option<String>,
    pub payload: Option<String>,
}

pub(crate) fn query(conn: &Connection, f: &AuditFilter, limit: usize) -> Result<Vec<AuditRow>> {
    // Dynamic filter SQL: same pattern as events::timeline_for_artifact.
    let mut sql = String::from(
        "SELECT seq, at_ms, tbl, op, row_id, actor, verb, payload FROM catalog_audit WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut add = |sql: &mut String, clause: &str, v: Box<dyn rusqlite::ToSql>| {
        sql.push_str(clause);
        params.push(v);
    };
    if let Some(v) = &f.tbl    { add(&mut sql, " AND tbl = ?", Box::new(v.clone())); }
    if let Some(v) = &f.row_id { add(&mut sql, " AND row_id = ?", Box::new(v.clone())); }
    if let Some(v) = &f.actor  { add(&mut sql, " AND actor = ?", Box::new(v.clone())); }
    if let Some(v) = &f.op     { add(&mut sql, " AND op = ?", Box::new(v.clone())); }
    if let Some(v) = f.since   { add(&mut sql, " AND at_ms >= ?", Box::new(v)); }
    if let Some(v) = f.until   { add(&mut sql, " AND at_ms <= ?", Box::new(v)); }
    sql.push_str(" ORDER BY seq DESC LIMIT ?");
    params.push(Box::new(limit as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
        Ok(AuditRow {
            seq: r.get(0)?, at_ms: r.get(1)?, tbl: r.get(2)?, op: r.get(3)?,
            row_id: r.get(4)?, actor: r.get(5)?, verb: r.get(6)?, payload: r.get(7)?,
        })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Deletes audit rows older than the cutoff, then writes ONE marker row
/// describing the prune — so the resulting seq gap explains itself instead of
/// reading as tampering.
pub(crate) fn prune_before(conn: &Connection, before_ms: i64) -> Result<usize> {
    let n = conn.execute("DELETE FROM catalog_audit WHERE at_ms < ?1", [before_ms])?;
    if n > 0 {
        conn.execute(
            &format!(
                "INSERT INTO catalog_audit(at_ms, tbl, op, row_id, payload)
                 VALUES({NOW_MS}, 'catalog_audit', 'delete', 'prune',
                        json_object('pruned', ?1, 'before_ms', ?2))"),
            rusqlite::params![n as i64, before_ms],
        )?;
    }
    Ok(n)
}
```

- [ ] **Step 4: Implement `src/librarian/tools/audit_log.rs`**

```rust
use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::audit;
use anyhow::Result;
use serde_json::{json, Value};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // Prune mode: dry-run by default, confirm=true applies (doctor's fix convention).
    if let Some(before) = args.get("prune_before_ms").and_then(Value::as_i64) {
        let confirm = args.get("confirm").and_then(Value::as_bool).unwrap_or(false);
        let cat = ctx.catalog.lock();
        if !confirm {
            let would: i64 = cat.conn.query_row(
                "SELECT count(*) FROM catalog_audit WHERE at_ms < ?1", [before], |r| r.get(0))?;
            return Ok(json!({"would_delete": would, "before_ms": before,
                             "hint": "pass confirm=true to apply"}));
        }
        let deleted = audit::prune_before(&cat.conn, before)?;
        return Ok(json!({"deleted": deleted, "before_ms": before}));
    }

    let limit = args.get("limit").and_then(Value::as_u64).map(|v| v as usize)
        .unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let f = audit::AuditFilter {
        tbl: args.get("tbl").and_then(Value::as_str).map(String::from),
        row_id: args.get("row_id").and_then(Value::as_str).map(String::from),
        actor: args.get("actor").and_then(Value::as_str).map(String::from),
        op: args.get("op").and_then(Value::as_str).map(String::from),
        since: args.get("since").and_then(Value::as_i64),
        until: args.get("until").and_then(Value::as_i64),
    };
    if let Some(op) = &f.op {
        if !matches!(op.as_str(), "insert" | "update" | "delete") {
            return Err(RecoverableError::new(format!(
                "op '{op}' — expected one of: insert, update, delete")));
        }
    }
    let cat = ctx.catalog.lock();
    let rows = audit::query(&cat.conn, &f, limit)?;
    let total: i64 = cat.conn.query_row("SELECT count(*) FROM catalog_audit", [], |r| r.get(0))?;
    let entries: Vec<Value> = rows.iter().map(|r| json!({
        "seq": r.seq, "at_ms": r.at_ms, "tbl": r.tbl, "op": r.op,
        "row_id": r.row_id, "actor": r.actor, "verb": r.verb,
        "payload": r.payload.as_deref()
            .and_then(|p| serde_json::from_str::<Value>(p).ok()),
    })).collect();
    // Negative-results ADR: the scope block says what was examined, always —
    // and a zero therefore names its window instead of implying "nothing happened".
    Ok(json!({
        "entries": entries,
        "count": entries.len(),
        "table_total": total,
        "unit": "at_ms/since/until are epoch-ms UTC",
        "scope": {
            "tbl": f.tbl, "row_id": f.row_id, "actor": f.actor, "op": f.op,
            "since": f.since, "until": f.until, "limit": limit,
        },
        "note": "verb means 'last dispatched verb on the writing connection', not per-statement; actor 'unknown' = a writer that did not identify itself (foreign process or raw sqlite3)."
    }))
}
```

- [ ] **Step 5: Wire dispatch** — `src/librarian/tools/mod.rs`: add `pub mod audit_log;`. `src/librarian/tools/librarian.rs`: add `"audit_log" => super::audit_log::call(ctx, args).await,` to the match (:118-127), add `audit_log` to **both** action-list error strings (:113-115 and :127-129), to the `description` (:15-39), and to `input_schema` (:41-109): extend the `action` enum array and add properties `tbl`, `row_id`, `actor`, `op`, `since`, `until`, `limit`, `prune_before_ms`, `confirm` with one-line descriptions each (state epoch-ms on the time fields).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p codescout audit`
Expected: all new tests PASS; `librarian::tools::librarian` route tests still green (they enumerate the action list — update `unknown_action_returns_recoverable_error`'s expected string if it asserts the full list).

- [ ] **Step 7: Commit (pathspec form)**

```bash
git add src/librarian/catalog/audit.rs src/librarian/tools/audit_log.rs src/librarian/tools/mod.rs src/librarian/tools/librarian.rs
git commit -m "feat(librarian): audit_log action — query the audit trail, dry-run prune" -- src/librarian/catalog/audit.rs src/librarian/tools/audit_log.rs src/librarian/tools/mod.rs src/librarian/tools/librarian.rs
```

---

### Task 4: doctor `audit` health block

**Files:**
- Modify: `src/librarian/catalog/audit.rs` (health fn), `src/librarian/tools/doctor.rs` (insert block into `catalog_health`, assembled at :690-742)
- Test: `audit.rs` + doctor test

**Interfaces:**
- Consumes: `catalog_health: serde_json::Map` assembly in `doctor::call` (the `catalog_health.insert(...)` cluster ending at the `Ok(json!({...}))` at :733-741).
- Produces: `audit::health(conn) -> Result<serde_json::Value>` — `{rows, span_ms: [min,max]|null, unknown_actor_rows, hint}`.

- [ ] **Step 1: Write the failing test** (in `audit.rs` tests)

```rust
#[test]
fn health_counts_rows_and_unknown_actors() {
    let cat = Catalog::open_in_memory().unwrap();
    seed(&cat, "h1"); // stamped by this connection's session actor
    cat.conn.execute(
        "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(5,'artifact','insert','h2')",
        []).unwrap(); // direct insert: stamped too (same conn) — so force one unknown:
    cat.conn.execute("UPDATE catalog_audit SET actor='unknown' WHERE row_id='h2'", []).unwrap();
    let h = super::health(&cat.conn).unwrap();
    assert!(h["rows"].as_i64().unwrap() >= 2);
    assert_eq!(h["unknown_actor_rows"], 1);
    assert!(h["span_ms"].is_array());
}
```

(Note the fixture's load-bearing detail, annotated in the test: the direct
`INSERT INTO catalog_audit` is itself stamped by this connection's temp
trigger — that is why the test then forces `'unknown'` explicitly. Removing
that UPDATE makes the assertion vacuous, not the code wrong.)

- [ ] **Step 2: Run to verify it fails** — `cargo test -p codescout health_counts` → compile error (`health` missing).

- [ ] **Step 3: Implement `health` in `audit.rs`**

```rust
pub(crate) fn health(conn: &Connection) -> Result<serde_json::Value> {
    let (rows, min_ms, max_ms, unknown): (i64, Option<i64>, Option<i64>, i64) = conn.query_row(
        "SELECT count(*), min(at_ms), max(at_ms),
                count(*) FILTER (WHERE actor = 'unknown')
         FROM catalog_audit",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok(serde_json::json!({
        "rows": rows,
        "span_ms": min_ms.zip(max_ms).map(|(a, b)| serde_json::json!([a, b])),
        "unknown_actor_rows": unknown,
        "hint": "unknown_actor_rows counts writers that did not identify themselves (foreign processes). Query with librarian(action=\"audit_log\", actor=\"unknown\")."
    }))
}
```

- [ ] **Step 4: Insert into doctor** — in `doctor::call`, beside `catalog_health.insert("declared_roots", ...)` (:729):

```rust
catalog_health.insert("audit".to_string(), crate::librarian::catalog::audit::health(&cat.conn)?);
```

Note the borrow scope: this must run while `cat` (the lock taken at :291) is
still alive — place it inside that region, with the other inserts.

- [ ] **Step 5: Run** — `cargo test -p codescout health_counts && cargo test -p codescout doctor` → PASS, doctor suite green.

- [ ] **Step 6: Commit (pathspec form)**

```bash
git add src/librarian/catalog/audit.rs src/librarian/tools/doctor.rs
git commit -m "feat(librarian): doctor reports audit-trail health incl. unknown-actor count" -- src/librarian/catalog/audit.rs src/librarian/tools/doctor.rs
```

---

### Task 5: guide/doc surfaces, tracker close-out, full gate

**Files:**
- Modify: the `get_guide("librarian")` action-table source — locate with `grep(pattern="merge_worktree", glob="**/*.md")` restricted to non-tracker sources plus `grep(pattern=\"merge_worktree\", glob=\"src/**\")`; the reference table that renders in `librarian(action=...)` — Reference gains one row: `audit_log | Query the catalog audit trail (who mutated what, when; actor 'unknown' = unidentified writer). prune_before_ms + confirm prunes.`
- Modify: `docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md` — Resume section gains: "Prospective instrument now exists: `librarian(action=\"audit_log\")` records every catalog mutation incl. foreign writers (T-1, this plan). The historical loss stays undeterminable; any recurrence is now answerable. Re-open trigger unchanged."
- Modify: tracker `docs/trackers/system-retrospective-improvements.md` (artifact `6f5ec09c63aef864`) — through the librarian, never by hand:
  ```
  artifact(action="update_entry", id="6f5ec09c63aef864", entry_collection="tasks",
           entry_id="T-1", fields={status: "done"})
  artifact(action="update", id="6f5ec09c63aef864", patch={body_edits: [{
    heading: "## History", action: "insert_after", at: "end-of-section",
    content: "### <date> — T-1 landed\n<fix SHA + patch-id, gate numbers>"}]})
  ```
- [ ] **Step 1: Update the guide action table** (find it first; it is the table this session saw rendered under "librarian(action=...) — Reference"). Add the `audit_log` row. Run the deprecated-tool-name/prompt-surface gates: `cargo test -p codescout prompt_surfaces && cargo test -p codescout claude_md_contains`.
- [ ] **Step 2: Update the vanished-rows bug file** Resume (text above) — `edit_markdown` is fine for `docs/issues/*` (not librarian-managed).
- [ ] **Step 3: Tracker updates** (commands above; record the fix SHA **and** patch-id: `git show <sha> > /tmp/x.patch && git patch-id --stable < /tmp/x.patch`).
- [ ] **Step 4: Full gate, in order:** `cargo fmt` → `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` → `cargo test --workspace --no-default-features` → `cargo test --workspace`. The lean lane must not see any of this code (it is librarian-gated); if it does, that is a Task-1-placement bug, not a gate flake.
- [ ] **Step 5: Commit (pathspec form)**

```bash
git add <guide file> docs/issues/2026-08-25-sdd-ledger-and-catalog-rows-vanished.md docs/trackers/system-retrospective-improvements.md
git commit -m "docs(librarian): audit_log guide row; vanished-rows bug gains its prospective instrument" -- <same paths>
```

---

## Explicitly Out of Scope (phase 2 = tracker T-7)

Committed JSONL shards, `merge=union` gitattribute, `audit_exported_through_seq` watermark, export-on-reindex. Nothing in this plan may write files under `.codescout/audit/`.

## Self-review notes (already applied)

- Spec §Capture superseded by the probe (Design Correction section) — the spec file itself is NOT edited by this plan; the correction note in `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` should be added at execution start (one-line pointer to this plan's Design Correction) so the two documents cannot disagree silently.
- `SessionKey` enum variants: implementer must check the real shape at `src/tools/session_key.rs:17-22` and keep only the `key.id()` form in `resolve_actor` (the plan flags the dead match for deletion).
- All timestamps labeled ms; zeros name scope; prune leaves a self-describing marker; `'unknown'` never fabricated for codescout connections (`codescout:anonymous` instead).
