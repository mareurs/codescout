# Catalog GC Lifecycle + Rename/Move Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the catalog's detect→repair loop — dead rows auto-hide (reversible) and moved repos are recoverable — without ever re-introducing an unattended destructive path.

**Architecture:** A `missing_since` column + a pure existence-based reconcile pass (throttled on `workspace(activate)`) stamps/clears missing rows; `find`/`semantic_search` hide rows missing past a grace period; deletion stays the existing manual `doctor(prune_missing, confirm=true)`. A new `doctor(fix="rehome", old_root, new_root, confirm=true)` migrates a moved repo's rows (id-rewrite preserving children), auto-detected via commit-hash overlap but applied only on confirm.

**Tech Stack:** Rust, `rusqlite` (SQLite + vec0 extension), `serde_json`, existing librarian catalog + tool framework.

**Spec:** `docs/superpowers/specs/2026-07-19-catalog-gc-lifecycle-rehome-design.md`

## Global Constraints

- **Existence- or identity-based only, NEVER scope-based.** The over-aggression bug (`delete_orphan_repos`) deleted by workspace membership; nothing here may. (Verbatim guard to preserve: empty roots is a no-op, never `DELETE FROM artifact`.)
- **No automatic deletion, ever.** Auto steps are stamp / clear / hide — all reversible. Row deletion is only `doctor(prune_missing, confirm=true)` (already shipped).
- **Both mutating ops require `confirm=true`** — `prune_missing` (delete) and `rehome` (id-rewrite). Dry-run is the default for each.
- **Grace period `N` = 14 days**, read from `catalog_meta.gc_grace_days` (default 14 if unset).
- **Reconcile/detection on activate is best-effort** — errors logged and swallowed; never fails or measurably slows activate.
- **Hide cutoff is an inlined server-computed i64** (not a bound param) — safe (no user input) and splices uniformly into every read site.
- **Branch `experiments` only.** Do not touch `master`. Bugs stay `open` until the user cherry-picks.
- **Pre-commit gate on every commit:** `cargo fmt && cargo clippy -- -D warnings && cargo test` must pass. (Live-MCP verify `cargo rb` + `/mcp` is a final manual step, not per-task.)
- **Tests are co-located** in `#[cfg(test)] mod tests` within each source file (project convention — see `doctor.rs`, `find.rs`, `filter.rs`).
- **Timestamps are epoch-ms i64.** Use the same helper that stamps `artifact.updated_at` (grep `fn now_ms` / where `updated_at` is written); this plan writes `now_ms` for that value.

## File Structure

- **Create** `src/librarian/catalog/gc.rs` — the GC core: `catalog_meta` accessors, `reconcile_missing_since`, `visibility_cutoff_ms`, `hidden_count`, move detection (`detect_move_candidates`), and rehome (`plan_rehome` / `apply_rehome`). All SQL lives here (catalog layer).
- **Modify** `src/librarian/catalog/mod.rs` — register `pub mod gc;`; add migration **v10** (missing_since column + catalog_meta table) in `apply_migrations_in_txn`.
- **Modify** `src/librarian/catalog/find.rs` — splice the visibility predicate into `find`, `count_matching`, `catalog_summary`, `find_by_ids_filtered`, `semantic_find`.
- **Modify** `src/librarian/tools/doctor.rs` — thread `new_root`; add `rehome` arm to `run_fix`; `validate_rehome_request`; surface move candidates in the read-only scan.
- **Modify** `src/librarian/tools/librarian.rs` — doctor tool schema: add `new_root` param, extend `fix` description.
- **Modify** `src/tools/config/mod.rs` — `ActivateProject.call` throttled reconcile hook; `workspace(action="status")` `catalog_health` block.

---

## Task 1: Schema migration v10 (missing_since + catalog_meta)

**Files:**
- Modify: `src/librarian/catalog/mod.rs` (`apply_migrations_in_txn`, ~line 205 after the v9 block; add `pub mod gc;` near the other `pub mod` lines)

**Interfaces:**
- Produces: `artifact.missing_since` (INTEGER, nullable, epoch-ms); table `catalog_meta(key TEXT PRIMARY KEY, value TEXT)`; `schema_version` row `(10)`.

- [ ] **Step 1: Write the failing test** (in `mod.rs` `#[cfg(test)] mod tests`, alongside `migrations_are_idempotent`):

```rust
#[test]
fn v10_adds_missing_since_and_catalog_meta() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    // missing_since column exists and defaults NULL
    assert!(column_exists(&cat.conn, "artifact", "missing_since").unwrap());
    // catalog_meta table exists and is writable
    cat.conn
        .execute("INSERT INTO catalog_meta(key, value) VALUES ('k','v')", [])
        .unwrap();
    let v: String = cat
        .conn
        .query_row("SELECT value FROM catalog_meta WHERE key='k'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, "v");
    let ver: i64 = cat
        .conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert!(ver >= 10);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test v10_adds_missing_since_and_catalog_meta`
Expected: FAIL — `column_exists(... "missing_since")` is false / no `catalog_meta` table.

- [ ] **Step 3: Add the v10 migration block** in `apply_migrations_in_txn`, immediately after the v9 `INSERT OR IGNORE INTO schema_version (version) VALUES (9)` and before the `// v6:` block:

```rust
// v10: catalog GC lifecycle — missing_since on artifact + catalog_meta kv.
if !column_exists(conn, "artifact", "missing_since")? {
    conn.execute("ALTER TABLE artifact ADD COLUMN missing_since INTEGER", [])?;
}
conn.execute(
    "CREATE TABLE IF NOT EXISTS catalog_meta (
       key   TEXT PRIMARY KEY,
       value TEXT
     )",
    [],
)?;
conn.execute(
    "INSERT OR IGNORE INTO schema_version (version) VALUES (10)",
    [],
)?;
```

Also add `pub mod gc;` next to the other `pub mod` declarations at the top of `mod.rs` (Task 2 creates the file; declaring it now is fine only once the file exists — if the build breaks, create an empty `gc.rs` stub first).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test v10_adds_missing_since_and_catalog_meta && cargo test migrations_are_idempotent && cargo test every_schema_sql_artifact_column_survives_every_migration_path`
Expected: PASS (all three — the last guards the column against the v6 table rebuild).

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/mod.rs
git commit -m "feat(catalog): v10 migration — missing_since column + catalog_meta"
```

---

## Task 2: catalog_meta accessors + grace/cutoff helpers (gc.rs)

**Files:**
- Create: `src/librarian/catalog/gc.rs`
- Modify: `src/librarian/catalog/mod.rs` (ensure `pub mod gc;` present)

**Interfaces:**
- Consumes: `rusqlite::Connection`, migration from Task 1.
- Produces:
  - `pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>>`
  - `pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()>`
  - `pub const DEFAULT_GRACE_DAYS: i64 = 14;`
  - `pub fn grace_days(conn: &Connection) -> Result<i64>` (reads `gc_grace_days`, else default)
  - `pub fn visibility_cutoff_ms(conn: &Connection, now_ms: i64) -> Result<i64>` (`now_ms - grace_days*86_400_000`)

- [ ] **Step 1: Write the failing test** (in `gc.rs` `#[cfg(test)] mod tests`):

```rust
use super::*;
use crate::librarian::catalog::Catalog;

#[test]
fn meta_roundtrip_and_grace_default_and_cutoff() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    assert_eq!(get_meta(&cat.conn, "x").unwrap(), None);
    set_meta(&cat.conn, "x", "1").unwrap();
    assert_eq!(get_meta(&cat.conn, "x").unwrap(), Some("1".to_string()));
    // grace default
    assert_eq!(grace_days(&cat.conn).unwrap(), DEFAULT_GRACE_DAYS);
    set_meta(&cat.conn, "gc_grace_days", "7").unwrap();
    assert_eq!(grace_days(&cat.conn).unwrap(), 7);
    // cutoff = now - grace*day
    let now = 1_000_000_000_000i64;
    assert_eq!(visibility_cutoff_ms(&cat.conn, now).unwrap(), now - 7 * 86_400_000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test meta_roundtrip_and_grace_default_and_cutoff`
Expected: FAIL — `gc` module / functions do not exist.

- [ ] **Step 3: Implement the accessors** in `gc.rs`:

```rust
//! Catalog GC lifecycle: missing_since reconcile, hide-from-find cutoff,
//! move detection, and rename/move rehome. All existence/identity-based;
//! no scope-based deletion, no automatic deletion.
use crate::librarian::catalog::Catalog;
use anyhow::Result;
use rusqlite::Connection;

pub const DEFAULT_GRACE_DAYS: i64 = 14;
const MS_PER_DAY: i64 = 86_400_000;

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v = conn
        .query_row("SELECT value FROM catalog_meta WHERE key = ?1", [key], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO catalog_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn grace_days(conn: &Connection) -> Result<i64> {
    Ok(get_meta(conn, "gc_grace_days")?
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|d| *d >= 0)
        .unwrap_or(DEFAULT_GRACE_DAYS))
}

pub fn visibility_cutoff_ms(conn: &Connection, now_ms: i64) -> Result<i64> {
    Ok(now_ms - grace_days(conn)? * MS_PER_DAY)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test meta_roundtrip_and_grace_default_and_cutoff`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/gc.rs src/librarian/catalog/mod.rs
git commit -m "feat(catalog): gc.rs — catalog_meta accessors + grace/cutoff helpers"
```

---

## Task 3: reconcile_missing_since (pure, existence-based)

**Files:**
- Modify: `src/librarian/catalog/gc.rs`

**Interfaces:**
- Consumes: Task 2 helpers.
- Produces:
  - `#[derive(Debug, Default, PartialEq)] pub struct ReconcileStats { pub newly_missing: usize, pub cleared: usize, pub still_missing: usize }`
  - `pub fn reconcile_missing_since(conn: &Connection, now_ms: i64) -> Result<ReconcileStats>`

- [ ] **Step 1: Write the failing test** (add to `gc.rs` tests; helper `seed(cat, id, abs_path)` mirrors doctor.rs `seed_artifact`):

```rust
fn seed(cat: &Catalog, id: &str, abs_path: &str) {
    cat.conn.execute(
        "INSERT INTO artifact(id, abs_path, kind, status, title, created_at, updated_at)
         VALUES (?1, ?2, 'tracker', 'active', 't', 0, 0)",
        rusqlite::params![id, abs_path],
    ).unwrap();
}

#[test]
fn reconcile_stamps_missing_clears_returned_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    let present = dir.path().join("here.md");
    std::fs::write(&present, "x").unwrap();
    seed(&cat, "gone", "/nonexistent/gone.md");
    seed(&cat, "here", present.to_str().unwrap());

    let s = reconcile_missing_since(&cat.conn, 1000).unwrap();
    assert_eq!(s.newly_missing, 1);
    assert_eq!(s.cleared, 0);
    let ms: Option<i64> = cat.conn.query_row(
        "SELECT missing_since FROM artifact WHERE id='gone'", [], |r| r.get(0)).unwrap();
    assert_eq!(ms, Some(1000));
    let ms2: Option<i64> = cat.conn.query_row(
        "SELECT missing_since FROM artifact WHERE id='here'", [], |r| r.get(0)).unwrap();
    assert_eq!(ms2, None);

    // idempotent: second run with no fs change stamps nothing new
    let s2 = reconcile_missing_since(&cat.conn, 2000).unwrap();
    assert_eq!(s2.newly_missing, 0);
    let ms3: Option<i64> = cat.conn.query_row(
        "SELECT missing_since FROM artifact WHERE id='gone'", [], |r| r.get(0)).unwrap();
    assert_eq!(ms3, Some(1000), "existing stamp is not overwritten");

    // file returns → cleared
    std::fs::write("/tmp", "").ok(); // no-op guard
    let returned = dir.path().join("returned.md");
    cat.conn.execute("UPDATE artifact SET abs_path=?1 WHERE id='gone'",
        rusqlite::params![returned.to_str().unwrap()]).unwrap();
    std::fs::write(&returned, "x").unwrap();
    let s3 = reconcile_missing_since(&cat.conn, 3000).unwrap();
    assert_eq!(s3.cleared, 1);
    let ms4: Option<i64> = cat.conn.query_row(
        "SELECT missing_since FROM artifact WHERE id='gone'", [], |r| r.get(0)).unwrap();
    assert_eq!(ms4, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test reconcile_stamps_missing_clears_returned_and_is_idempotent`
Expected: FAIL — `reconcile_missing_since` undefined.

- [ ] **Step 3: Implement** in `gc.rs`:

```rust
#[derive(Debug, Default, PartialEq)]
pub struct ReconcileStats {
    pub newly_missing: usize,
    pub cleared: usize,
    pub still_missing: usize,
}

/// Stat every artifact row's abs_path; stamp newly-missing, clear returned.
/// NEVER deletes. NEVER scope-based. Idempotent on an unchanged filesystem.
pub fn reconcile_missing_since(conn: &Connection, now_ms: i64) -> Result<ReconcileStats> {
    let mut stmt = conn.prepare("SELECT id, abs_path, missing_since FROM artifact")?;
    let rows: Vec<(String, Option<String>, Option<i64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut stats = ReconcileStats::default();
    for (id, abs_path, missing_since) in rows {
        let exists = abs_path
            .as_deref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);
        match (exists, missing_since) {
            (false, None) => {
                conn.execute(
                    "UPDATE artifact SET missing_since = ?1 WHERE id = ?2",
                    rusqlite::params![now_ms, id],
                )?;
                stats.newly_missing += 1;
            }
            (false, Some(_)) => stats.still_missing += 1,
            (true, Some(_)) => {
                conn.execute(
                    "UPDATE artifact SET missing_since = NULL WHERE id = ?1",
                    [&id],
                )?;
                stats.cleared += 1;
            }
            (true, None) => {}
        }
    }
    Ok(stats)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test reconcile_stamps_missing_clears_returned_and_is_idempotent`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/gc.rs
git commit -m "feat(catalog): reconcile_missing_since — existence-based stamp/clear pass"
```

---

## Task 4: Hide-from-find (visibility predicate at all read sites)

**Files:**
- Modify: `src/librarian/catalog/find.rs` (`find` 13-33, `count_matching` 37-49, `catalog_summary` 62-109, `find_by_ids_filtered` 143-191, `semantic_find` 201-233)
- Modify: `src/librarian/catalog/gc.rs` (add `visibility_sql` + `hidden_count`)

**Interfaces:**
- Consumes: `visibility_cutoff_ms` (Task 2).
- Produces:
  - `pub fn visibility_sql(cutoff_ms: i64) -> String` → `"(missing_since IS NULL OR missing_since > <cutoff>)"` (cutoff inlined — a server i64, no injection)
  - `pub fn hidden_count(conn: &Connection, cutoff_ms: i64) -> Result<usize>`

- [ ] **Step 1: Write the failing test** (in `find.rs` tests; the `art`/seed helpers already exist there — extend to set `missing_since`):

```rust
#[test]
fn find_hides_rows_missing_past_grace_and_shows_within_grace() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    // present row
    cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at) \
        VALUES ('live','/x/a.md','tracker','active','a',0,10)", []).unwrap();
    // missing long ago (before cutoff) → hidden
    cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,missing_since) \
        VALUES ('old','/x/b.md','tracker','active','b',0,9, 100)", []).unwrap();
    // missing recently (within grace, after cutoff) → visible
    cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,missing_since) \
        VALUES ('new','/x/c.md','tracker','active','c',0,8, 5000)", []).unwrap();

    let cutoff = 1000i64; // rows with missing_since <= 1000 are hidden
    let opts = FindOpts { filter: None, limit: 100, offset: 0 };
    let rows = find_visible(&cat, &opts, cutoff).unwrap();
    let ids: Vec<_> = rows.iter().map(|r| r.id.clone()).collect();
    assert!(ids.contains(&"live".to_string()));
    assert!(ids.contains(&"new".to_string()));
    assert!(!ids.contains(&"old".to_string()), "old missing row is hidden");
    assert_eq!(super::gc::hidden_count(&cat.conn, cutoff).unwrap(), 1);
}
```

> Note: the plan threads the cutoff explicitly (`find_visible`) so it stays testable without a clock. Callers compute `cutoff = gc::visibility_cutoff_ms(conn, now_ms)?`. If you prefer to keep the existing `find` name, add the cutoff parameter to it and update all call sites — grep `catalog::find(` / `find(&` for callers.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test find_hides_rows_missing_past_grace_and_shows_within_grace`
Expected: FAIL — `find_visible` / `gc::hidden_count` undefined.

- [ ] **Step 3a: Add the predicate helpers** to `gc.rs`:

```rust
/// The hide-from-find predicate. `cutoff_ms` is inlined (a server-computed
/// i64 — no user input, no injection). A row is visible if it was never
/// missing, or went missing within the grace window (missing_since > cutoff).
pub fn visibility_sql(cutoff_ms: i64) -> String {
    format!("(missing_since IS NULL OR missing_since > {cutoff_ms})")
}

/// Count of rows currently hidden (missing_since <= cutoff).
pub fn hidden_count(conn: &Connection, cutoff_ms: i64) -> Result<usize> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE missing_since IS NOT NULL AND missing_since <= ?1",
        [cutoff_ms],
        |r| r.get(0),
    )?;
    Ok(n.max(0) as usize)
}
```

- [ ] **Step 3b: Splice the predicate into every read site** in `find.rs`. Pattern: after `FROM artifact`, always start the WHERE with the visibility predicate, then AND the compiled filter. For `find` (do the identical transform to `count_matching`, `catalog_summary`, `find_by_ids_filtered`, and `semantic_find`):

```rust
pub fn find_visible(cat: &Catalog, opts: &FindOpts, cutoff_ms: i64) -> Result<Vec<ArtifactRow>> {
    let mut sql = String::from(
        "SELECT id, abs_path, kind, status, title, owners, tags,\
         topic, time_scope, source, created_at, updated_at, file_mtime,\
         file_sha256, confidence FROM artifact WHERE ",
    );
    sql.push_str(&crate::librarian::catalog::gc::visibility_sql(cutoff_ms));
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = &opts.filter {
        let frag = compile(f)?;
        sql.push_str(" AND (");
        sql.push_str(&frag.sql);
        sql.push(')');
        params.extend(frag.params);
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");
    params.push(rusqlite::types::Value::Integer(opts.limit as i64));
    params.push(rusqlite::types::Value::Integer(opts.offset as i64));
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), row_from_sql)?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}
```

Apply the same "`FROM artifact WHERE <visibility> [AND (<filter>)]`" edit to:
- `count_matching` → `count_matching_visible(cat, filter, cutoff_ms)`
- `catalog_summary` (both the `GROUP BY kind` query at ~line 78 and the augmentation subquery at ~line 96 — each `FROM artifact` gets ` WHERE <visibility>` prepended, ANDing any existing scope filter)
- `find_by_ids_filtered` (numbered-param path — the visibility predicate has NO params, so just prepend ` AND <visibility>` to its existing WHERE; no index shift needed since it adds zero placeholders)
- `semantic_find` (same: append ` AND <visibility>` to its catalog-join WHERE)

Then update callers to pass `cutoff_ms = gc::visibility_cutoff_ms(&cat.conn, now_ms)?`. Grep callers: `find(&`, `count_matching(`, `catalog_summary(`, `find_by_ids_filtered(`, `semantic_find(`. **Preserve `find_by_ids_filtered`'s existing behavior for `artifact(get, id=…)` forensics** — see Step 3c.

- [ ] **Step 3c: Add a forensic bypass.** `artifact(action="get", id=…)` and `doctor` must still reach hidden rows. Keep the original (unfiltered) query paths available: retain `find_by_ids` without the visibility predicate for direct-id `get`, and only apply visibility to the *listing/search* paths (`find`, `count_matching`, `catalog_summary`, `semantic_find`). Add a test asserting `get`-by-id returns a hidden row.

```rust
#[test]
fn get_by_id_still_returns_hidden_row() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,missing_since) \
        VALUES ('h','/x/h.md','tracker','active','h',0,1, 100)", []).unwrap();
    // the direct-id path (used by artifact get) ignores visibility
    let rows = find_by_ids(&cat, &["h".to_string()]).unwrap();
    assert_eq!(rows.len(), 1);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test find_hides_rows_missing_past_grace_and_shows_within_grace && cargo test get_by_id_still_returns_hidden_row && cargo test -- catalog::find`
Expected: PASS (and no existing find/summary test regresses).

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/find.rs src/librarian/catalog/gc.rs
git commit -m "feat(catalog): hide-from-find past grace (listing/search only; get/doctor bypass)"
```

---

## Task 5: Throttled reconcile on workspace(activate)

**Files:**
- Modify: `src/tools/config/mod.rs` (`ActivateProject.call`)
- Modify: `src/librarian/catalog/gc.rs` (add `reconcile_if_due`)

**Interfaces:**
- Consumes: `reconcile_missing_since` (Task 3), `get_meta`/`set_meta` (Task 2).
- Produces: `pub fn reconcile_if_due(conn: &Connection, now_ms: i64, min_interval_ms: i64) -> Result<Option<ReconcileStats>>` (returns `None` when throttled). Const `pub const RECONCILE_INTERVAL_MS: i64 = 24 * 3_600_000;`

- [ ] **Step 1: Write the failing test** (in `gc.rs` tests):

```rust
#[test]
fn reconcile_if_due_respects_throttle() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    seed(&cat, "gone", "/nonexistent/z.md");
    // first call runs
    let a = reconcile_if_due(&cat.conn, 100_000, 1000).unwrap();
    assert!(a.is_some());
    // second call within interval is throttled
    let b = reconcile_if_due(&cat.conn, 100_500, 1000).unwrap();
    assert!(b.is_none());
    // after interval, runs again
    let c = reconcile_if_due(&cat.conn, 101_200, 1000).unwrap();
    assert!(c.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test reconcile_if_due_respects_throttle`
Expected: FAIL — `reconcile_if_due` undefined.

- [ ] **Step 3: Implement `reconcile_if_due`** in `gc.rs`:

```rust
pub const RECONCILE_INTERVAL_MS: i64 = 24 * 3_600_000;

/// Run reconcile only if the last run was more than `min_interval_ms` ago.
/// Records `last_reconcile_at` on run. Returns None when throttled.
pub fn reconcile_if_due(
    conn: &Connection,
    now_ms: i64,
    min_interval_ms: i64,
) -> Result<Option<ReconcileStats>> {
    let last = get_meta(conn, "last_reconcile_at")?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(i64::MIN);
    if now_ms.saturating_sub(last) < min_interval_ms {
        return Ok(None);
    }
    let stats = reconcile_missing_since(conn, now_ms)?;
    set_meta(conn, "last_reconcile_at", &now_ms.to_string())?;
    Ok(Some(stats))
}
```

- [ ] **Step 4a: Run test to verify it passes**

Run: `cargo test reconcile_if_due_respects_throttle`
Expected: PASS

- [ ] **Step 4b: Wire the best-effort hook** into `ActivateProject.call` (`src/tools/config/mod.rs`), after the existing bootstrap succeeds and the catalog is available. It MUST NOT affect the activate result on error:

```rust
// Best-effort catalog GC reconcile (throttled). Never fails activate.
if let Ok(cat) = ctx.catalog.try_lock_for(std::time::Duration::from_millis(50)) {
    let now_ms = crate::util::time::now_ms(); // same helper used for updated_at
    if let Err(e) = crate::librarian::catalog::gc::reconcile_if_due(
        &cat.conn,
        now_ms,
        crate::librarian::catalog::gc::RECONCILE_INTERVAL_MS,
    ) {
        tracing::debug!("gc reconcile on activate skipped: {e}");
    }
}
```

> Verify the exact catalog accessor on `ToolContext` (grep `ctx.catalog` in this file / `src/tools/core/types.rs`) and the now-ms helper path (grep `now_ms`). Match whatever `run_fix` uses (`ctx.catalog.lock()`), but prefer a non-blocking try-lock here so activate never stalls on a busy catalog.

- [ ] **Step 4c: Add an integration assertion** near `workspace_action_activate_dispatches_to_activate_project` (`src/tools/config/tests.rs`): activate a project whose catalog has a missing row, then assert `last_reconcile_at` is set and the row's `missing_since` is stamped.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/gc.rs src/tools/config/mod.rs src/tools/config/tests.rs
git commit -m "feat(activate): throttled best-effort catalog GC reconcile on workspace(activate)"
```

---

## Task 6: Surface catalog health in librarian(action="doctor")

> **Re-scoped 2026-07-19:** originally `workspace(action="status")`, but `ProjectStatus`/`ActivateProject` run on the catalog-less `src/tools/core/types.rs::ToolContext` (see Task 5). `doctor` is a librarian tool with native catalog access, is the catalog-health tool, computes fresh on demand, and co-locates with the move-candidate detection (Task 9). Surface `catalog_health` there.

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (the read-only `call`, ~124-157 — the branch with no `fix`, which returns `{violations, summary}`).

**Interfaces:**
- Consumes: `gc::hidden_count`, `gc::visibility_cutoff_ms`, `gc::grace_days`.
- Produces: a `catalog_health` field in the read-only doctor response: `{ hidden_rows: usize, move_candidates: usize, hint: String }`. `move_candidates` is `0` here; Task 9 fills it.

- [ ] **Step 1: Write the failing test** (in `doctor.rs` `#[cfg(test)] mod tests`, mirroring `doctor_call_surfaces_seeded_drift`):

```rust
#[tokio::test]
async fn doctor_reports_catalog_health_hidden_rows() {
    // Build a ToolContext with a catalog (mirror doctor_call_surfaces_seeded_drift's setup).
    // Seed one row missing past grace (missing_since small) and one live row.
    // e.g. via the test's catalog handle:
    //   INSERT ... missing_since = 1   (hidden, since cutoff = now - 14d >> 1)
    let out = super::call(&ctx, json!({})).await.unwrap();
    assert!(out["catalog_health"]["hidden_rows"].as_u64().unwrap() >= 1);
    assert_eq!(out["catalog_health"]["move_candidates"].as_u64().unwrap(), 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test doctor_reports_catalog_health_hidden_rows`
Expected: FAIL — no `catalog_health` key in the doctor response.

- [ ] **Step 3: Implement** in the read-only branch of `doctor::call` (the path that builds the `{violations, summary}` JSON, after the scans, while the catalog lock is still held or re-acquire it). Compute the counts and add the block:

```rust
// after computing `by_check` / before the final Ok(json!({...})):
let now_ms = chrono::Utc::now().timestamp_millis();
let cutoff = crate::librarian::catalog::gc::visibility_cutoff_ms(&cat.conn, now_ms)?;
let hidden_rows = crate::librarian::catalog::gc::hidden_count(&cat.conn, cutoff)?;
let grace = crate::librarian::catalog::gc::grace_days(&cat.conn)?;
let health_hint = if hidden_rows > 0 {
    format!("{hidden_rows} row(s) hidden as missing (>{grace}d). Run librarian(action=\"doctor\", fix=\"prune_missing\") to remove, or doctor(fix=\"rehome\", old_root, new_root) to migrate a moved repo.")
} else {
    String::new()
};

Ok(json!({
    "violations": all_violations,
    "summary": { "total": all_violations.len(), "by_check": by_check },
    "catalog_health": {
        "hidden_rows": hidden_rows,
        "move_candidates": 0, // filled by Task 9
        "hint": health_hint,
    },
}))
```

> Note: the current `call` drops the catalog lock (`drop(cat)`) before computing the summary. Re-acquire (`let cat = ctx.catalog.lock();`) for the `hidden_count`/`grace_days` reads, or move these reads before the `drop`. Keep the lock scope minimal.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test doctor_reports_catalog_health_hidden_rows`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs
git commit -m "feat(doctor): catalog_health block — hidden-row count + prune/rehome hint"
```

---
---

## Task 7: Rehome core — transactional id-rewrite (gc.rs)

> **Schema verified 2026-07-19 against `src/librarian/catalog/schema.sql`.** The FK-children of `artifact(id)` are: `events.artifact_id`, `event_edges.dst_artifact_id`, `artifact_augmentation.artifact_id`, `artifact_observation.artifact_id`, and `artifact_link.src_id` + `artifact_link.dst_id` (columns are `src_id`/`dst_id`, NOT `src`/`dst`). `artifact_vec` is a **vec0 virtual table** with `id TEXT PRIMARY KEY` and NO FK — only an `AFTER DELETE` trigger; its id must be handled manually. `entry_cite.src_slug` references `artifact(slug)` → unaffected (slug is not rewritten). BUT `entry_cite.dst_ref` is free-text storing a raw artifact **id** for hex-id/rel_path citation forms → it **IS migrated** (`UPDATE entry_cite SET dst_ref=new WHERE dst_ref=old`) so intra-repo citations follow the move. All real FKs are `ON DELETE CASCADE` only (no `ON UPDATE`), so an id UPDATE needs `PRAGMA defer_foreign_keys = ON` within the transaction.

**Files:**
- Modify: `src/librarian/catalog/gc.rs`

**Interfaces:**
- Consumes: catalog schema; `crate::librarian::ids::artifact_id_from_abs`.
- Produces:
  - `pub struct RehomeRow { pub old_id: String, pub old_abs: String, pub new_id: String, pub new_abs: String }`
  - `pub struct RehomePlan { pub rows: Vec<RehomeRow>, pub collisions: Vec<String>, pub commit_rows: usize }`
  - `pub fn plan_rehome(conn: &Connection, old_root: &Path, new_root: &Path) -> Result<RehomePlan>`
  - `#[derive(Debug, Default)] pub struct RehomeStats { pub artifact_rows: usize, pub commit_rows: usize, pub skipped_collisions: usize }`
  - `pub fn apply_rehome(conn: &Connection, plan: &RehomePlan) -> Result<RehomeStats>`

- [ ] **Step 1: Write the failing test** (in `gc.rs` tests — assert EVERY child table follows the id, and no orphan remains under the old id):

```rust
#[test]
fn rehome_rewrites_id_and_preserves_all_children() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    let new_root = dir.path().join("newrepo");
    std::fs::create_dir_all(new_root.join("docs")).unwrap();
    std::fs::write(new_root.join("docs/t.md"), "x").unwrap();
    let old_abs = "/oldrepo/docs/t.md";
    let old_id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(old_abs));
    // a second, stable artifact to be the other endpoint of a link
    seed(&cat, "other", "/oldrepo/docs/o.md"); // seed helper from earlier tasks (sets NOT NULL cols)
    cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256) \
        VALUES (?1,?2,'tracker','active','t',0,0,0,'')", rusqlite::params![old_id, old_abs]).unwrap();
    // one child in EACH table keyed by old_id:
    cat.conn.execute("INSERT INTO events(id,artifact_id,kind,payload,created_at) VALUES ('e1',?1,'note','{}',0)", [&old_id]).unwrap();
    cat.conn.execute("INSERT INTO event_edges(src_event_id,dst_artifact_id,rel) VALUES ('e1',?1,'mutates')", [&old_id]).unwrap();
    cat.conn.execute("INSERT INTO artifact_augmentation(artifact_id,prompt) VALUES (?1,'p')", [&old_id]).unwrap();
    cat.conn.execute("INSERT INTO artifact_observation(artifact_id,text,created_at) VALUES (?1,'obs',0)", [&old_id]).unwrap();
    cat.conn.execute("INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES (?1,'other','implements',0)", [&old_id]).unwrap();
    cat.conn.execute("INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES ('other',?1,'implements',0)", [&old_id]).unwrap();
    // artifact_vec (vec0): embedding under old_id
    cat.conn.execute("INSERT INTO artifact_vec(id,embedding) VALUES (?1, ?2)",
        rusqlite::params![old_id, vec![0.0f32; 768].iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>()]).unwrap();

    let plan = plan_rehome(&cat.conn, std::path::Path::new("/oldrepo"), &new_root).unwrap();
    // "other" also lives under /oldrepo but its file is absent → it rebases too;
    // for THIS test we only assert on the t.md row. (Either assert plan.rows has both, or narrow old_root.)
    let new_id = crate::librarian::ids::artifact_id_from_abs(
        std::path::Path::new(&new_root.join("docs/t.md").to_string_lossy().into_owned()));
    apply_rehome(&cat.conn, &plan).unwrap();

    // parent moved, no orphan under old_id
    let c: i64 = cat.conn.query_row("SELECT COUNT(*) FROM artifact WHERE id=?1", [&old_id], |r| r.get(0)).unwrap();
    assert_eq!(c, 0, "no artifact orphan under old id");
    // every child followed the id (history preserved) — assert NO child still references old_id:
    for (table, col) in [("events","artifact_id"),("event_edges","dst_artifact_id"),
                         ("artifact_augmentation","artifact_id"),("artifact_observation","artifact_id")] {
        let n: i64 = cat.conn.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE {col}=?1"), [&old_id], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "{table}.{col} still references old id");
        let m: i64 = cat.conn.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE {col}=?1"), [&new_id], |r| r.get(0)).unwrap();
        assert!(m >= 1, "{table}.{col} did not follow to new id");
    }
    // artifact_link both endpoints
    let ls: i64 = cat.conn.query_row("SELECT COUNT(*) FROM artifact_link WHERE src_id=?1 OR dst_id=?1", [&old_id], |r| r.get(0)).unwrap();
    assert_eq!(ls, 0, "artifact_link still references old id");
    // artifact_vec: no orphan under old_id (either migrated to new_id or removed for re-embed)
    let vold: i64 = cat.conn.query_row("SELECT COUNT(*) FROM artifact_vec WHERE id=?1", [&old_id], |r| r.get(0)).unwrap();
    assert_eq!(vold, 0, "artifact_vec orphan under old id (vec0 trigger only fires on DELETE, not UPDATE)");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test rehome_rewrites_id_and_preserves_all_children`
Expected: FAIL — `plan_rehome`/`apply_rehome` undefined.

- [ ] **Step 3: Implement** in `gc.rs`:

```rust
use std::path::Path;

pub struct RehomeRow { pub old_id: String, pub old_abs: String, pub new_id: String, pub new_abs: String }
pub struct RehomePlan { pub rows: Vec<RehomeRow>, pub collisions: Vec<String>, pub commit_rows: usize }
#[derive(Debug, Default)]
pub struct RehomeStats { pub artifact_rows: usize, pub commit_rows: usize, pub skipped_collisions: usize }

fn rebase(old_abs: &str, old_root: &Path, new_root: &Path) -> Option<String> {
    let rel = Path::new(old_abs).strip_prefix(old_root).ok()?;
    Some(new_root.join(rel).to_string_lossy().into_owned())
}

pub fn plan_rehome(conn: &Connection, old_root: &Path, new_root: &Path) -> Result<RehomePlan> {
    let old_root_str = crate::util::fs::RepoPath::from_path(old_root).to_string();
    let like = format!("{old_root_str}/%");
    let mut stmt = conn.prepare(
        "SELECT id, abs_path FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2 ESCAPE '\\'")?;
    let raw: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![old_root_str, like], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut rows = Vec::new();
    let mut collisions = Vec::new();
    for (old_id, old_abs) in raw {
        let Some(new_abs) = rebase(&old_abs, old_root, new_root) else { continue };
        let new_id = crate::librarian::ids::artifact_id_from_abs(Path::new(&new_abs));
        // collision: a row already exists at the new id/path (e.g. reindex minted it)
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artifact WHERE id = ?1 OR abs_path = ?2",
            rusqlite::params![new_id, new_abs], |r| r.get(0))?;
        if exists > 0 { collisions.push(new_abs); continue; }
        rows.push(RehomeRow { old_id, old_abs, new_id, new_abs });
    }
    let commit_rows: i64 = conn.query_row(
        "SELECT COUNT(*) FROM commits WHERE git_root = ?1 OR git_root LIKE ?2 ESCAPE '\\'",
        rusqlite::params![old_root_str, like], |r| r.get(0))?;
    Ok(RehomePlan { rows, collisions, commit_rows: commit_rows.max(0) as usize })
}

/// Apply the plan in ONE transaction with deferred FK checks so parent+child
/// id rewrites validate together at COMMIT (the FKs are ON DELETE CASCADE
/// only — they do NOT cover UPDATE). Never deletes content.
pub fn apply_rehome(conn: &Connection, plan: &RehomePlan) -> Result<RehomeStats> {
    let mut stats = RehomeStats { skipped_collisions: plan.collisions.len(), ..Default::default() };
    conn.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
    let tx = conn.unchecked_transaction()?;
    for row in &plan.rows {
        // FK children (order irrelevant — checks deferred to COMMIT):
        for (table, col) in [
            ("events", "artifact_id"),
            ("event_edges", "dst_artifact_id"),
            ("artifact_augmentation", "artifact_id"),
            ("artifact_observation", "artifact_id"),
            ("artifact_link", "src_id"),
            ("artifact_link", "dst_id"),
        ] {
            tx.execute(
                &format!("UPDATE {table} SET {col} = ?1 WHERE {col} = ?2"),
                rusqlite::params![row.new_id, row.old_id],
            )?;
        }
        // artifact_vec (vec0 virtual table, no FK, trigger only on DELETE):
        // migrate the embedding to the new id if present, else ensure no orphan.
        // vec0 may not support UPDATE of the PK — if the straight UPDATE errors
        // or leaves an orphan, delete-old + insert-new the embedding instead.
        // VERIFY empirically which vec0 supports; the test asserts no old-id orphan.
        migrate_vec_id(&tx, &row.old_id, &row.new_id)?;
        // parent last:
        tx.execute("UPDATE artifact SET id = ?1, abs_path = ?2, missing_since = NULL WHERE id = ?3",
            rusqlite::params![row.new_id, row.new_abs, row.old_id])?;
        stats.artifact_rows += 1;
    }
    tx.commit()?;
    Ok(stats)
}
```

Implement `migrate_vec_id(tx, old_id, new_id)` to handle the vec0 table correctly (determine whether `UPDATE artifact_vec SET id=?1 WHERE id=?2` works on vec0; if not, `SELECT embedding ... WHERE id=old`, `DELETE ... WHERE id=old`, `INSERT (id=new, embedding)`). If there is no vec row for `old_id`, it's a no-op. The test asserts no `artifact_vec` row remains under `old_id`.

> **Guards note (used by Task 8):** `plan_rehome` only reads; the guards (`new_root` must exist, `old_root` must not, both absolute) live in Task 8's `validate_rehome_request`. If an active `worktree_registration` covers `old_root`, Task 8 should refuse (mirror `prune`'s worktree guard) — note this for Task 8.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test rehome_rewrites_id_and_preserves_all_children`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/gc.rs
git commit -m "feat(catalog): rehome core — deferred-FK id-rewrite preserving all children"
```

---
---

## Task 8: doctor(fix="rehome") tool plumbing + guards

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (`call` 124-133 to thread `new_root`; `run_fix` add `rehome` arm; add `validate_rehome_request`)
- Modify: `src/librarian/tools/librarian.rs` (doctor input schema: add `new_root`; extend `fix` enum/description)

**Interfaces:**
- Consumes: `plan_rehome`/`apply_rehome` (Task 7).
- Produces: `fn validate_rehome_request(old_root, new_root, conn) -> Result<(PathBuf, PathBuf)>`; `run_fix(ctx, fix, root, new_root, confirm)` (new param).

- [ ] **Step 1: Write the failing test** (in `doctor.rs` tests, mirroring `validate_prune_request_gates`):

```rust
#[test]
fn validate_rehome_gates() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    let live = dir.path().join("live"); std::fs::create_dir_all(&live).unwrap();
    // new_root must exist
    assert!(validate_rehome_request(Some("/gone/old"), Some("/also/gone"), &cat.conn).is_err());
    // old_root must NOT exist
    assert!(validate_rehome_request(Some(live.to_str().unwrap()), Some(live.to_str().unwrap()), &cat.conn).is_err());
    // both required + absolute
    assert!(validate_rehome_request(None, Some(live.to_str().unwrap()), &cat.conn).is_err());
    assert!(validate_rehome_request(Some("relative/old"), Some(live.to_str().unwrap()), &cat.conn).is_err());
    // happy path: old gone, new exists
    assert!(validate_rehome_request(Some("/gone/old"), Some(live.to_str().unwrap()), &cat.conn).is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test validate_rehome_gates`
Expected: FAIL — `validate_rehome_request` undefined.

- [ ] **Step 3a: Add `validate_rehome_request`** in `doctor.rs` (mirror `validate_prune_request`):

```rust
fn validate_rehome_request<'a>(
    old_root: Option<&'a str>,
    new_root: Option<&'a str>,
    _conn: &rusqlite::Connection,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let old = old_root.ok_or_else(|| RecoverableError::new(
        "fix=rehome requires old_root=<absolute path the repo used to live at>"))?;
    let new = new_root.ok_or_else(|| RecoverableError::new(
        "fix=rehome requires new_root=<absolute path the repo now lives at>"))?;
    let (op, np) = (std::path::Path::new(old), std::path::Path::new(new));
    if !op.is_absolute() || !np.is_absolute() {
        return Err(RecoverableError::new("old_root and new_root must both be absolute paths"));
    }
    if op.exists() {
        return Err(RecoverableError::new(format!(
            "old_root '{old}' still exists — rehome only migrates rows from a path that is gone")));
    }
    if !np.exists() {
        return Err(RecoverableError::new(format!(
            "new_root '{new}' does not exist — cannot rehome onto a missing directory")));
    }
    Ok((op.to_path_buf(), np.to_path_buf()))
}
```

- [ ] **Step 3b: Thread `new_root` and add the `rehome` arm.** Change `call` (line ~132) to `run_fix(ctx, fix, args.get("root").and_then(Value::as_str), args.get("new_root").and_then(Value::as_str), confirm).await` and `run_fix`'s signature to accept `new_root: Option<&str>`. Add before the `other =>` arm:

```rust
"rehome" => {
    let cat = ctx.catalog.lock();
    let (old, new) = validate_rehome_request(root, new_root, &cat.conn)?;
    let plan = crate::librarian::catalog::gc::plan_rehome(&cat.conn, &old, &new)?;
    if plan.rows.is_empty() && plan.collisions.is_empty() {
        return Err(RecoverableError::new(format!(
            "no catalog rows found under old_root '{}'", old.display())));
    }
    if !confirm {
        return Ok(json!({
            "fix": "rehome", "mode": "dry_run",
            "old_root": old.to_string_lossy(), "new_root": new.to_string_lossy(),
            "artifact_rows": plan.rows.len(),
            "commit_rows": plan.commit_rows,
            "collisions": plan.collisions,
            "hint": "re-run with confirm=true to migrate these rows (ids + history preserved)",
        }));
    }
    // rewrite commits.git_root too
    let stats = crate::librarian::catalog::gc::apply_rehome(&cat.conn, &plan)?;
    crate::librarian::catalog::gc::rehome_commits(&cat.conn, &old, &new)?; // add this small helper in gc.rs
    Ok(json!({
        "fix": "rehome", "mode": "applied",
        "old_root": old.to_string_lossy(), "new_root": new.to_string_lossy(),
        "migrated": { "artifact_rows": stats.artifact_rows, "commit_rows": plan.commit_rows,
                      "skipped_collisions": stats.skipped_collisions },
    }))
}
```

Add `rehome_commits(conn, old_root, new_root)` to `gc.rs` (an `UPDATE commits SET git_root = replace(git_root, old, new) WHERE git_root = old OR git_root LIKE old||'/%'`), and a test for it.

- [ ] **Step 3c: Update the doctor schema** in `librarian.rs`: add `"new_root": {"type": "string", "description": "For fix=rehome: absolute path the repo now lives at."}` to the input schema, and extend the `fix` description to mention `rehome` (old_root + new_root, dry-run default, migrates a moved repo preserving ids/history). Keep the existing `prompt_surfaces_reference_only_real_tools` gate green.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test validate_rehome_gates && cargo test -- doctor`
Expected: PASS (existing doctor tests still green — `run_fix`'s new param must not change `prune_missing` behavior; add `None` for `new_root` at existing call sites/tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs src/librarian/tools/librarian.rs src/librarian/catalog/gc.rs
git commit -m "feat(doctor): fix=rehome — dry-run/confirm move recovery with guards + schema"
```

---

## Task 9: Move-candidate detection (commit-hash overlap) + surfacing

> **Re-scoped 2026-07-19:** surfacing moves to `librarian(action="doctor")` (Task 6's `catalog_health` block), NOT `workspace(status)`/activate caching — same reason as Task 6 (config-tool ToolContext has no catalog). Task 9 fills the `move_candidates` placeholder that Task 6 set to `0`.

**Files:**
- Modify: `src/librarian/catalog/gc.rs` (`detect_move_candidates`)
- Modify: `src/librarian/tools/doctor.rs` (fill `catalog_health.move_candidates` in the read-only `call`)

**Interfaces:**
- Consumes: `commits` table (git_root, hash), `artifact.file_sha256`.
- Produces:
  - `pub struct MoveCandidate { pub old_root: String, pub new_root: String, pub shared_commits: usize, pub artifact_rows: usize }`
  - `pub fn detect_move_candidates(conn: &Connection, active_git_root: &str) -> Result<Vec<MoveCandidate>>`

- [ ] **Step 1: Write the failing test** (in `gc.rs` tests):

```rust
#[test]
fn detect_move_by_shared_commit_hash() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
    // old (gone) repo with commit h1; active repo (new path) also has h1
    cat.conn.execute("INSERT INTO commits(hash,git_root,authored_at,subject,topo_order) \
        VALUES ('h1','/gone/oldrepo',0,'s',0)", []).unwrap();
    cat.conn.execute("INSERT INTO commits(hash,git_root,authored_at,subject,topo_order) \
        VALUES ('h1','/live/newrepo',0,'s',0)", []).unwrap();
    // an unrelated gone repo shares no commit → not a candidate
    cat.conn.execute("INSERT INTO commits(hash,git_root,authored_at,subject,topo_order) \
        VALUES ('z9','/gone/unrelated',0,'s',0)", []).unwrap();

    let cands = detect_move_candidates(&cat.conn, "/live/newrepo").unwrap();
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].old_root, "/gone/oldrepo");
    assert_eq!(cands[0].new_root, "/live/newrepo");
    assert!(cands[0].shared_commits >= 1);
}
```

Also add an **ambiguity test**: active root shares commits with TWO distinct gone roots → `detect_move_candidates` returns empty (no actionable candidate).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test detect_move_by_shared_commit_hash`
Expected: FAIL — `detect_move_candidates` undefined.

- [ ] **Step 3: Implement `detect_move_candidates`** in `gc.rs`. A `git_root` is a move candidate for `active_git_root` iff it (a) shares ≥1 commit hash with the active root, (b) is NOT itself on disk, and (c) is the unique such gone root (ambiguous = overlaps 2+ gone roots → return empty, per spec):

```rust
pub struct MoveCandidate {
    pub old_root: String,
    pub new_root: String,
    pub shared_commits: usize,
    pub artifact_rows: usize,
}

pub fn detect_move_candidates(conn: &Connection, active_git_root: &str) -> Result<Vec<MoveCandidate>> {
    // gone roots sharing commit hashes with the active root
    let mut stmt = conn.prepare(
        "SELECT c2.git_root, COUNT(*) FROM commits c1
           JOIN commits c2 ON c1.hash = c2.hash
          WHERE c1.git_root = ?1 AND c2.git_root <> ?1
          GROUP BY c2.git_root")?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([active_git_root], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut out = Vec::new();
    for (old_root, shared) in rows {
        if std::path::Path::new(&old_root).exists() { continue; } // only GONE roots
        // escape LIKE metachars in old_root (memory catalog-sql-hazards) — use the
        // same escape_like_pattern helper plan_rehome uses.
        let like = format!("{}/%", crate::librarian::util::escape_like_pattern(&old_root));
        let artifact_rows: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artifact WHERE abs_path LIKE ?1 ESCAPE '\\'",
            [like], |r| r.get(0))?;
        out.push(MoveCandidate {
            old_root, new_root: active_git_root.to_string(),
            shared_commits: shared.max(0) as usize,
            artifact_rows: artifact_rows.max(0) as usize,
        });
    }
    // ambiguity guard: if the active root's commits map to 2+ distinct gone
    // roots, do not surface any (fall back to explicit old_root/new_root).
    if out.len() > 1 { return Ok(Vec::new()); }
    Ok(out)
}
```

> `file_sha256` confirmation (spec) can be added as a second filter later; commit-hash overlap alone is near-certain identity. Note in code that the sha256 cross-check is deferred.

- [ ] **Step 3b: Surface in doctor's `catalog_health`.** In `doctor.rs`'s read-only `call` (where Task 6 added the `catalog_health` block with `move_candidates: 0`), replace the placeholder with a real count: resolve the ACTIVE repo's git root from the librarian ToolContext (grep how doctor/other librarian tools get the active project root — e.g. `ctx.active` / the resident project root; if the active root is a git repo it is the `git_root` used in `commits`), call `gc::detect_move_candidates(&cat.conn, active_git_root)`, set `move_candidates` to `candidates.len()`, and include a `move_candidates_detail` array (old_root → new_root, shared_commits, artifact_rows) when non-empty. Extend the health hint: when `move_candidates > 0`, mention `doctor(fix="rehome", old_root=…, new_root=…)`.
> If the active git root is not readily resolvable in doctor's `call`, report NEEDS_CONTEXT — do NOT fabricate it.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test detect_move_by_shared_commit_hash && cargo test -- doctor`
Expected: PASS. Extend the Task-6 doctor test (or add a new one) to seed a move-candidate (active + gone root sharing a commit hash, gone root absent on disk) and assert `catalog_health.move_candidates >= 1`.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/gc.rs src/librarian/tools/doctor.rs
git commit -m "feat(gc): move-candidate detection via commit-hash overlap, surfaced in doctor catalog_health"
```

---
---

## Final verification (after all tasks)

- [ ] `cargo fmt && cargo clippy -- -D warnings && cargo test` — full suite green.
- [ ] `cargo rb` then `/mcp` to reconnect; live-verify: `workspace(action="status")` shows `catalog_health`; on a synthetic moved repo, activate surfaces a move candidate and `doctor(fix="rehome", old_root, new_root)` dry-run→confirm migrates rows with events/augmentation intact.
- [ ] Update the two bug files' status logs (do NOT archive — bugs stay `open` until the user cherry-picks to `master`): note the root-cause lifecycle shipped on `experiments`, cross-link the rename/move bug's "option 1" as delivered.

## Self-Review

- **Spec coverage:** `missing_since` (T1) ✓ · reconcile (T3) ✓ · throttled activate trigger (T5) ✓ · hide-from-find + grace N (T4) ✓ · surfacing (T6/T9) ✓ · rehome detect (T9) + apply-on-confirm (T7/T8) ✓ · "no auto-delete" honored (nothing deletes) ✓ · "never scope-based" honored (existence/identity only) ✓.
- **Placeholder scan:** none — every code step carries real code; the two "verify against schema" notes are explicit verification instructions, not deferred implementation.
- **Type consistency:** `ReconcileStats` (T3) reused by `reconcile_if_due` (T5); `RehomePlan`/`apply_rehome` (T7) consumed by the `rehome` arm (T8); `visibility_sql`/`visibility_cutoff_ms`/`hidden_count` (T2/T4) consumed by find (T4) and status (T6); `MoveCandidate` (T9) consumed by status (T6 placeholder → T9 fill).
- **Known follow-through for the implementer:** confirm exact FK-child table/column names against `mod.rs` base schema before trusting Task 7's list; confirm the `now_ms` helper path and `ctx.catalog` lock accessor.
