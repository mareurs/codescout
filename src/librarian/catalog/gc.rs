//! Catalog GC lifecycle: missing_since reconcile, hide-from-find cutoff,
//! move detection, and rename/move rehome. All existence/identity-based;
//! no scope-based deletion, no automatic deletion.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

pub const DEFAULT_GRACE_DAYS: i64 = 14;
const MS_PER_DAY: i64 = 86_400_000;

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    match conn.query_row(
        "SELECT value FROM catalog_meta WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
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

/// 24h in ms — the default throttle interval for `reconcile_if_due`.
pub const RECONCILE_INTERVAL_MS: i64 = 24 * 3_600_000;

/// Run reconcile only if the last run was more than `min_interval_ms` ago.
/// Records `last_reconcile_at` on run. Returns `None` when throttled.
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

/// Best-effort, non-blocking, throttled reconcile for the librarian
/// tool-call path (`LibrarianAdapter::call`). Uses a non-blocking
/// `try_lock` — a busy catalog is skipped rather than waited on, so this
/// can never stall a call. Any error from the reconcile itself (e.g. a
/// broken catalog) is logged and swallowed: this must never break the
/// librarian call it's piggybacking on.
pub fn maybe_reconcile(
    catalog: &std::sync::Arc<parking_lot::Mutex<crate::librarian::catalog::Catalog>>,
    now_ms: i64,
) {
    let Some(cat) = catalog.try_lock() else {
        return;
    };
    if let Err(e) = reconcile_if_due(&cat.conn, now_ms, RECONCILE_INTERVAL_MS) {
        tracing::debug!("gc reconcile skipped: {e}");
    }
}

/// One artifact's rename/move: old id/path → new id/path, derived from
/// rebasing `abs_path` under `old_root` onto `new_root`.
pub struct RehomeRow {
    pub old_id: String,
    pub old_abs: String,
    pub new_id: String,
    pub new_abs: String,
}

/// Dry-run result of [`plan_rehome`]: rows safe to rewrite, paths that
/// collided with an existing catalog row (skipped, not rewritten), and the
/// count of `commits` rows anchored under `old_root` (informational only —
/// `commits.git_root` is not id-keyed, so it isn't rewritten here).
pub struct RehomePlan {
    pub rows: Vec<RehomeRow>,
    pub collisions: Vec<String>,
    pub commit_rows: usize,
}

/// Outcome of [`apply_rehome`].
#[derive(Debug, Default)]
pub struct RehomeStats {
    pub artifact_rows: usize,
    pub commit_rows: usize,
    pub skipped_collisions: usize,
}

/// Rebase an absolute path from under `old_root` onto `new_root`, preserving
/// the relative tail. Returns `None` if `old_abs` is not actually under
/// `old_root` (defensive — the caller's SQL scoping should already guarantee
/// this).
///
/// `old_abs == old_root` exactly (the row IS the root, not a descendant —
/// the common case for `commits.git_root`, and possible for `artifact.abs_path`
/// via `plan_rehome`'s `abs_path = ?1` clause) yields an EMPTY relative tail.
/// `PathBuf::join` on an empty path still appends a separator (`"/a".join("")
/// == "/a/"`), which would silently leave a trailing-slash-mangled path — so
/// that case returns `new_root` verbatim instead of joining.
fn rebase(old_abs: &str, old_root: &Path, new_root: &Path) -> Option<String> {
    let rel = Path::new(old_abs).strip_prefix(old_root).ok()?;
    let rebased = if rel.as_os_str().is_empty() {
        new_root.to_path_buf()
    } else {
        new_root.join(rel)
    };
    Some(crate::util::fs::RepoPath::from_path(&rebased).into_string())
}

/// Dry-run: derive the old-id → new-id/path mapping for every catalog row
/// anchored at or under `old_root`, without writing anything. A row whose
/// derived new id/path already exists in the catalog (e.g. a reindex under
/// the new root already minted it) is reported as a collision and excluded
/// from `rows` — the caller decides whether to skip or resolve it.
pub fn plan_rehome(conn: &Connection, old_root: &Path, new_root: &Path) -> Result<RehomePlan> {
    let old_root_str = crate::util::fs::RepoPath::from_path(old_root).into_string();
    let escaped_root = crate::librarian::util::escape_like_pattern(&old_root_str);
    let like = format!("{escaped_root}/%");
    let mut stmt = conn.prepare(
        "SELECT id, abs_path FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2 ESCAPE '\\'",
    )?;
    let raw: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![old_root_str, like], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<std::result::Result<_, _>>()?;
    drop(stmt);

    let mut rows = Vec::new();
    let mut collisions = Vec::new();
    for (old_id, old_abs) in raw {
        let Some(new_abs) = rebase(&old_abs, old_root, new_root) else {
            continue;
        };
        let new_id = crate::librarian::ids::artifact_id_from_abs(Path::new(&new_abs));
        // Collision: a row already exists at the new id/path (e.g. reindex
        // under new_root already minted it before the rehome ran).
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM artifact WHERE id = ?1 OR abs_path = ?2",
            rusqlite::params![new_id, new_abs],
            |r| r.get(0),
        )?;
        if exists > 0 {
            collisions.push(new_abs);
            continue;
        }
        rows.push(RehomeRow {
            old_id,
            old_abs,
            new_id,
            new_abs,
        });
    }

    let commit_rows: i64 = conn.query_row(
        "SELECT COUNT(*) FROM commits WHERE git_root = ?1 OR git_root LIKE ?2 ESCAPE '\\'",
        rusqlite::params![old_root_str, like],
        |r| r.get(0),
    )?;
    Ok(RehomePlan {
        rows,
        collisions,
        commit_rows: commit_rows.max(0) as usize,
    })
}

/// Rewrite `commits.git_root` for the root being rehomed and any nested
/// child roots under it (e.g. a worktree's `git_root` sitting below the
/// moved main-repo root). `plan_rehome`'s `commit_rows` only counts matching
/// rows for the dry-run summary; this is the corresponding apply step,
/// deferred from Task 7 into the `confirm=true` path of `fix=rehome`.
///
/// Reuses `rebase()` (the same old-root/new-root strip-and-rejoin `plan_rehome`
/// uses) rather than an SQL-side `REPLACE`, so a `git_root` that merely
/// *contains* `old_root` as a substring elsewhere in the string can't be
/// mis-rewritten — only an exact-prefix match under `old_root` qualifies.
/// Also reuses `crate::librarian::util::escape_like_pattern` rather than
/// re-inlining the LIKE-escape idiom (`like_escape_idiom_is_not_inlined_outside_helper`
/// in `librarian/util.rs` pins this).
///
/// Returns the number of `commits` rows rewritten.
pub fn rehome_commits(conn: &Connection, old_root: &Path, new_root: &Path) -> Result<usize> {
    let old_root_str = crate::util::fs::RepoPath::from_path(old_root).into_string();
    let escaped_root = crate::librarian::util::escape_like_pattern(&old_root_str);
    let like = format!("{escaped_root}/%");

    let matched_roots: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT git_root FROM commits WHERE git_root = ?1 OR git_root LIKE ?2 ESCAPE '\\'",
        )?
        .query_map(rusqlite::params![old_root_str, like], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;

    let mut total = 0usize;
    for old_git_root in matched_roots {
        let Some(new_git_root) = rebase(&old_git_root, old_root, new_root) else {
            continue;
        };
        total += conn.execute(
            "UPDATE commits SET git_root = ?1 WHERE git_root = ?2",
            rusqlite::params![new_git_root, old_git_root],
        )?;
    }
    Ok(total)
}

/// Migrate an `artifact_vec` embedding row from `old_id` to `new_id`.
///
/// `artifact_vec` is a `vec0` VIRTUAL table (see schema.sql) with no FK to
/// `artifact` — only an `AFTER DELETE` trigger on `artifact` that cascades a
/// DELETE, never an UPDATE, into `artifact_vec`. Rewriting `artifact.id` via
/// UPDATE therefore does NOT move the vec row; it must be migrated here
/// explicitly, or it becomes an orphan under the old id.
///
/// Empirically verified (throwaway probe, since removed): a direct
/// `UPDATE artifact_vec SET id = ?1 WHERE id = ?2` is rejected outright by
/// sqlite-vec with `SqliteFailure` / "UPDATEs on vec0 primary key values are
/// not allowed." — not a silent no-op, a hard error. This matches the
/// existing DELETE-then-INSERT idiom `src/librarian/indexer.rs::write_embeddings`
/// already uses for the same table (there because vec0 also doesn't honor
/// `INSERT OR REPLACE` conflict resolution on this column). A no-op if there
/// is no embedding row for `old_id`.
fn migrate_vec_id(tx: &rusqlite::Transaction<'_>, old_id: &str, new_id: &str) -> Result<()> {
    let embedding: Option<Vec<u8>> = tx
        .query_row(
            "SELECT embedding FROM artifact_vec WHERE id = ?1",
            [old_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(embedding) = embedding {
        tx.execute("DELETE FROM artifact_vec WHERE id = ?1", [old_id])?;
        tx.execute(
            "INSERT INTO artifact_vec (id, embedding) VALUES (?1, ?2)",
            rusqlite::params![new_id, embedding],
        )?;
    }
    Ok(())
}

/// Apply the plan in ONE transaction with deferred FK checks, so the parent
/// `artifact.id` rewrite and every FK-child rewrite validate together at
/// COMMIT (the FKs are `ON DELETE CASCADE` only — they do not cover UPDATE,
/// so without deferral each child UPDATE would transiently reference a
/// not-yet-rewritten or already-rewritten parent id and could violate the FK
/// mid-loop). Never deletes content; a hard error rolls back the whole
/// batch atomically (no partial rehome).
pub fn apply_rehome(conn: &Connection, plan: &RehomePlan) -> Result<RehomeStats> {
    let mut stats = RehomeStats {
        skipped_collisions: plan.collisions.len(),
        commit_rows: plan.commit_rows,
        ..Default::default()
    };
    conn.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
    let tx = conn.unchecked_transaction()?;
    for row in &plan.rows {
        // FK children first (order among these is irrelevant — checks are
        // deferred to COMMIT, not per-statement):
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
        // entry_cite.dst_ref: free-TEXT, no FK to artifact — stores a raw
        // 16-hex artifact id for the hex-id/rel_path citation forms (the
        // `<slug>:<local>` form stores a slug there instead, which never
        // equals an id, so it's correctly left untouched by this UPDATE).
        // Without this, a citation pointing at the rehomed artifact would
        // dangle under the old id and idx_entry_cite_dst reverse lookups
        // would silently miss it.
        tx.execute(
            "UPDATE entry_cite SET dst_ref = ?1 WHERE dst_ref = ?2",
            rusqlite::params![row.new_id, row.old_id],
        )?;
        // artifact_vec: no FK, DELETE-trigger only — handled explicitly.
        migrate_vec_id(&tx, &row.old_id, &row.new_id)?;
        // Parent last.
        tx.execute(
            "UPDATE artifact SET id = ?1, abs_path = ?2, missing_since = NULL WHERE id = ?3",
            rusqlite::params![row.new_id, row.new_abs, row.old_id],
        )?;
        stats.artifact_rows += 1;
    }
    tx.commit()?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            visibility_cutoff_ms(&cat.conn, now).unwrap(),
            now - 7 * 86_400_000
        );
    }

    #[test]
    fn set_meta_overwrites_existing_key() {
        let cat = Catalog::open_in_memory().unwrap();
        set_meta(&cat.conn, "k", "first").unwrap();
        set_meta(&cat.conn, "k", "second").unwrap();
        assert_eq!(
            get_meta(&cat.conn, "k").unwrap(),
            Some("second".to_string())
        );
    }

    #[test]
    fn grace_days_falls_back_on_invalid_override() {
        let cat = Catalog::open_in_memory().unwrap();
        set_meta(&cat.conn, "gc_grace_days", "-1").unwrap();
        assert_eq!(grace_days(&cat.conn).unwrap(), DEFAULT_GRACE_DAYS);
        set_meta(&cat.conn, "gc_grace_days", "not-a-number").unwrap();
        assert_eq!(grace_days(&cat.conn).unwrap(), DEFAULT_GRACE_DAYS);
    }

    fn seed(cat: &Catalog, id: &str, abs_path: &str) {
        cat.conn
                .execute(
                    "INSERT INTO artifact \
                     (id, abs_path, kind, status, title, created_at, updated_at, file_mtime, file_sha256) \
                     VALUES (?1, ?2, 'tracker', 'active', 't', 0, 0, 0, '')",
                    rusqlite::params![id, abs_path],
                )
                .unwrap();
    }

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
        seed(&cat, "other", "/oldrepo/docs/o.md");
        // give "other" a slug so it can stand in as entry_cite's src_slug
        // (FK'd to artifact(slug)) below — the rehomed artifact (t.md) itself
        // has no slug in this test.
        cat.conn
            .execute(
                "UPDATE artifact SET slug = 'other-slug' WHERE id = 'other'",
                [],
            )
            .unwrap();
        cat.conn.execute("INSERT INTO artifact(id,abs_path,kind,status,title,created_at,updated_at,file_mtime,file_sha256) \
            VALUES (?1,?2,'tracker','active','t',0,0,0,'')", rusqlite::params![old_id, old_abs]).unwrap();
        // simulate: GC's reconcile already stamped this row missing (the
        // file vanished from the old path before the rename/move landed);
        // rehome must clear it since the file demonstrably exists again
        // at new_abs.
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 12345 WHERE id=?1",
                [&old_id],
            )
            .unwrap();
        // one child in EACH table keyed by old_id:
        cat.conn.execute("INSERT INTO events(id,artifact_id,kind,payload,created_at) VALUES ('e1',?1,'note','{}',0)", [&old_id]).unwrap();
        cat.conn.execute("INSERT INTO event_edges(src_event_id,dst_artifact_id,rel) VALUES ('e1',?1,'mutates')", [&old_id]).unwrap();
        cat.conn
            .execute(
                "INSERT INTO artifact_augmentation(artifact_id,prompt) VALUES (?1,'p')",
                [&old_id],
            )
            .unwrap();
        cat.conn
            .execute(
                "INSERT INTO artifact_observation(artifact_id,text,created_at) VALUES (?1,'obs',0)",
                [&old_id],
            )
            .unwrap();
        cat.conn.execute("INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES (?1,'other','implements',0)", [&old_id]).unwrap();
        cat.conn.execute("INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES ('other',?1,'implements',0)", [&old_id]).unwrap();
        // entry_cite: a raw hex-id citation pointing at the rehomed artifact
        // via dst_ref (no FK on that column — this is the gap Fix 1 covers).
        cat.conn
            .execute(
                "INSERT INTO entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at) \
                 VALUES ('other-slug', 'l1', ?1, 'cites', 'write', 0)",
                [&old_id],
            )
            .unwrap();
        // artifact_vec (vec0): a DISTINCTIVE (non-zero, non-uniform) embedding
        // under old_id, so migration of the VALUE — not just row presence —
        // can be asserted after rehome.
        let embedding_bytes: Vec<u8> = (0..768u32)
            .map(|i| i as f32)
            .flat_map(|f| f.to_le_bytes())
            .collect();
        cat.conn
            .execute(
                "INSERT INTO artifact_vec(id,embedding) VALUES (?1, ?2)",
                rusqlite::params![old_id, embedding_bytes],
            )
            .unwrap();

        let plan = plan_rehome(&cat.conn, std::path::Path::new("/oldrepo"), &new_root).unwrap();
        let new_id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(
            &new_root.join("docs/t.md").to_string_lossy().into_owned(),
        ));
        // pin plan.rows scope: "other" also lives under /oldrepo (its file is
        // absent, but plan_rehome doesn't check file existence) so it rebases
        // too — the plan should contain exactly these two rows, including the
        // t.md mapping this test exercises below. An over-broad or
        // over-narrow plan_rehome would otherwise pass unnoticed since the
        // rest of this test only asserts on the t.md row.
        assert_eq!(
            plan.rows.len(),
            2,
            "plan should include exactly the two /oldrepo artifacts (t.md + other)"
        );
        assert!(
            plan.rows
                .iter()
                .any(|r| r.old_id == old_id && r.new_id == new_id),
            "plan.rows must contain the t.md → new_id rehome mapping"
        );
        apply_rehome(&cat.conn, &plan).unwrap();

        // parent moved, no orphan under old_id
        let c: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE id=?1",
                [&old_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(c, 0, "no artifact orphan under old id");
        // missing_since (set above to simulate a prior GC reconcile stamp)
        // must clear on rehome — the file exists again at new_abs.
        let missing_since: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id=?1",
                [&new_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            missing_since, None,
            "missing_since must clear on rehome, not carry over"
        );
        // every child followed the id (history preserved) — assert NO child still references old_id:
        for (table, col) in [
            ("events", "artifact_id"),
            ("event_edges", "dst_artifact_id"),
            ("artifact_augmentation", "artifact_id"),
            ("artifact_observation", "artifact_id"),
        ] {
            let n: i64 = cat
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {col}=?1"),
                    [&old_id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 0, "{table}.{col} still references old id");
            let m: i64 = cat
                .conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {col}=?1"),
                    [&new_id],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(m >= 1, "{table}.{col} did not follow to new id");
        }
        // artifact_link both endpoints
        let ls: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_link WHERE src_id=?1 OR dst_id=?1",
                [&old_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ls, 0, "artifact_link still references old id");
        // entry_cite.dst_ref: the raw-id citation must follow the rehome too
        // (Fix 1) — dst_ref has no FK, so nothing but the explicit UPDATE in
        // apply_rehome keeps it from dangling.
        let cite_old: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM entry_cite WHERE dst_ref=?1",
                [&old_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cite_old, 0, "entry_cite.dst_ref still references old id");
        let cite_new: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM entry_cite WHERE dst_ref=?1",
                [&new_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cite_new, 1, "entry_cite.dst_ref did not follow to new id");
        // artifact_vec: no orphan under old_id (either migrated to new_id or removed for re-embed)
        let vold: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_vec WHERE id=?1",
                [&old_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            vold, 0,
            "artifact_vec orphan under old id (vec0 trigger only fires on DELETE, not UPDATE)"
        );
        // and the embedding actually migrated to new_id with its VALUE intact
        // (not just row presence, and not silently replaced/dropped).
        let migrated_embedding: Vec<u8> = cat
            .conn
            .query_row(
                "SELECT embedding FROM artifact_vec WHERE id=?1",
                [&new_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            migrated_embedding, embedding_bytes,
            "migrated embedding bytes must match the originally seeded value"
        );
    }

    #[test]
    fn migrate_vec_id_is_noop_when_no_embedding_row_exists() {
        // No artifact_vec row exists for "missing-old" — migrate_vec_id must
        // be a clean no-op: no error, and no row created under either id.
        let cat = Catalog::open_in_memory().unwrap();
        let tx = cat.conn.unchecked_transaction().unwrap();
        migrate_vec_id(&tx, "missing-old", "missing-new").unwrap();
        tx.commit().unwrap();
        let n: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            n, 0,
            "migrate_vec_id must not create a row when none existed for old_id"
        );
    }

    #[test]
    fn apply_rehome_rolls_back_atomically_on_mid_batch_failure() {
        // Two rows in one plan: the first row's parent UPDATE would
        // succeed on its own; the second row's parent UPDATE collides
        // with a pre-existing abs_path (a UNIQUE constraint, checked
        // immediately — not deferred like FKs) and must fail. The whole
        // transaction must then roll back, including the already-applied
        // first row, proving apply_rehome is atomic (no partial rehome).
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1", "/oldrepo/a.md");
        seed(&cat, "c1", "/oldrepo/c.md");
        seed(&cat, "existing", "/newrepo/b.md"); // occupies the path row c1 will collide into

        let plan = RehomePlan {
            rows: vec![
                RehomeRow {
                    old_id: "a1".to_string(),
                    old_abs: "/oldrepo/a.md".to_string(),
                    new_id: "a1-new".to_string(),
                    new_abs: "/newrepo/a.md".to_string(),
                },
                RehomeRow {
                    old_id: "c1".to_string(),
                    old_abs: "/oldrepo/c.md".to_string(),
                    new_id: "c1-new".to_string(),
                    new_abs: "/newrepo/b.md".to_string(), // collides with "existing"
                },
            ],
            collisions: vec![],
            commit_rows: 0,
        };

        let result = apply_rehome(&cat.conn, &plan);
        assert!(
            result.is_err(),
            "the abs_path collision on row 2 must surface as an error"
        );

        // Row 1 (a1) must NOT have been left rehomed — its own UPDATE
        // ran and would have "succeeded" in isolation, but the batch's
        // transaction must have rolled it back along with row 2.
        let a1_old: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id='a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            a1_old, 1,
            "a1 must still exist under its old id (rolled back)"
        );
        let a1_new: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id='a1-new'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(a1_new, 0, "a1-new must not exist — the batch rolled back");

        // Row 2 (c1) and the pre-existing collider are both untouched.
        let c1_old: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id='c1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            c1_old, 1,
            "c1 must still exist under its old id (rolled back)"
        );
        let existing: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE id='existing' AND abs_path='/newrepo/b.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(existing, 1, "the pre-existing collider is untouched");
    }

    #[test]
    fn plan_rehome_like_pattern_escapes_underscore_wildcard() {
        // catalog-sql-hazards: an unescaped `_` in old_root would act as a
        // LIKE single-char wildcard, sweeping up a sibling repo whose path
        // merely differs by one character at that position. old_root
        // itself contains a literal underscore to exercise this.
        //
        // `plan.rows`/`plan.collisions` turn out to be defended twice over:
        // `rebase()` re-validates with `Path::strip_prefix`, an exact
        // string/component match with no wildcard semantics, so a
        // SQL-level false-positive candidate is silently dropped there
        // regardless of whether the LIKE pattern was escaped. `commit_rows`
        // has no such second check — it trusts the SQL COUNT directly — so
        // that's where an unescaped `_`/`%` in old_root would actually leak
        // into a wrong (inflated) answer. This test pins both.
        let cat = Catalog::open_in_memory().unwrap();
        let old_root = "/tmp/proj_x";
        seed(&cat, "real", "/tmp/proj_x/docs/a.md"); // genuine match (literal underscore)
        seed(&cat, "sibling", "/tmp/projYx/docs/b.md"); // differs at the `_` position
        seed(&cat, "keep", "/tmp/unrelated.md");
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root) VALUES ('real-commit', '/tmp/proj_x')",
                [],
            )
            .unwrap();
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root) VALUES ('sib-commit', '/tmp/projYx/sub')",
                [],
            )
            .unwrap();

        let new_root = std::path::PathBuf::from("/tmp/newhome");
        let plan = plan_rehome(&cat.conn, std::path::Path::new(old_root), &new_root).unwrap();

        let old_ids: Vec<&str> = plan.rows.iter().map(|r| r.old_id.as_str()).collect();
        assert!(
            old_ids.contains(&"real"),
            "the genuine /tmp/proj_x/... row must be planned"
        );
        assert!(
            !old_ids.contains(&"sibling"),
            "rebase()'s exact strip_prefix must reject /tmp/projYx/... even as a raw SQL candidate"
        );
        assert!(
            !old_ids.contains(&"keep"),
            "unrelated row must not be planned"
        );
        assert_eq!(
                    plan.commit_rows, 1,
                    "commit_rows has no strip_prefix re-check — an unescaped `_` would wrongly count the sibling repo's commit too"
                );
    }

    #[test]
    fn rehome_commits_rewrites_git_root_for_root_and_children() {
        // Mirrors plan_rehome_like_pattern_escapes_underscore_wildcard's hazard:
        // old_root contains a literal underscore, and a sibling path differing
        // only at that position must NOT be swept up by an unescaped LIKE `_`
        // wildcard.
        let cat = Catalog::open_in_memory().unwrap();
        let old_root = "/tmp/proj_x";
        let new_root = std::path::PathBuf::from("/tmp/newhome");
        // The root's own commits row.
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root) VALUES ('root-commit', '/tmp/proj_x')",
                [],
            )
            .unwrap();
        // A child root nested under it (e.g. a worktree-scoped commits row).
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root) VALUES ('child-commit', '/tmp/proj_x/.worktrees/foo')",
                [],
            )
            .unwrap();
        // Sibling differing only at the underscore position — must be left alone.
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root) VALUES ('sib-commit', '/tmp/projYx/sub')",
                [],
            )
            .unwrap();
        // Unrelated root — must be left alone.
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root) VALUES ('unrelated-commit', '/tmp/unrelated')",
                [],
            )
            .unwrap();

        let n = rehome_commits(&cat.conn, std::path::Path::new(old_root), &new_root).unwrap();
        assert_eq!(
            n, 2,
            "root row + child row rewritten; sibling/unrelated untouched"
        );

        let root_git_root: String = cat
            .conn
            .query_row(
                "SELECT git_root FROM commits WHERE hash = 'root-commit'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(root_git_root, "/tmp/newhome");

        let child_git_root: String = cat
            .conn
            .query_row(
                "SELECT git_root FROM commits WHERE hash = 'child-commit'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(child_git_root, "/tmp/newhome/.worktrees/foo");

        let sib_git_root: String = cat
            .conn
            .query_row(
                "SELECT git_root FROM commits WHERE hash = 'sib-commit'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sib_git_root, "/tmp/projYx/sub", "sibling untouched");

        let unrelated_git_root: String = cat
            .conn
            .query_row(
                "SELECT git_root FROM commits WHERE hash = 'unrelated-commit'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unrelated_git_root, "/tmp/unrelated", "unrelated untouched");
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
        assert_eq!(s.still_missing, 0);
        let ms: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms, Some(1000));
        let ms2: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='here'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms2, None);

        // idempotent: second run with no fs change stamps nothing new
        let s2 = reconcile_missing_since(&cat.conn, 2000).unwrap();
        assert_eq!(s2.newly_missing, 0);
        assert_eq!(s2.still_missing, 1);
        let ms3: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms3, Some(1000), "existing stamp is not overwritten");

        // file returns → cleared
        let returned = dir.path().join("returned.md");
        cat.conn
            .execute(
                "UPDATE artifact SET abs_path=?1 WHERE id='gone'",
                rusqlite::params![returned.to_str().unwrap()],
            )
            .unwrap();
        std::fs::write(&returned, "x").unwrap();
        let s3 = reconcile_missing_since(&cat.conn, 3000).unwrap();
        assert_eq!(s3.cleared, 1);
        assert_eq!(s3.still_missing, 0);
        let ms4: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms4, None);
    }
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

    #[test]
    fn maybe_reconcile_stamps_and_throttles() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        seed(&cat, "gone", "/nonexistent/mr.md");
        let catalog = std::sync::Arc::new(parking_lot::Mutex::new(cat));

        // first call: due (no last_reconcile_at yet) -> stamps missing_since + records last_reconcile_at
        maybe_reconcile(&catalog, 100_000);
        {
            let cat = catalog.lock();
            let ms: Option<i64> = cat
                .conn
                .query_row(
                    "SELECT missing_since FROM artifact WHERE id='gone'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(ms, Some(100_000));
            assert_eq!(
                get_meta(&cat.conn, "last_reconcile_at").unwrap(),
                Some("100000".to_string())
            );
        }

        // seed a second missing row, then call again well within the 24h
        // interval -> throttled: neither the new row is touched nor
        // last_reconcile_at is bumped.
        {
            let cat = catalog.lock();
            seed(&cat, "gone2", "/nonexistent/mr2.md");
        }
        maybe_reconcile(&catalog, 100_500);
        {
            let cat = catalog.lock();
            let ms2: Option<i64> = cat
                .conn
                .query_row(
                    "SELECT missing_since FROM artifact WHERE id='gone2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(ms2, None, "throttled call must not run reconcile");
            assert_eq!(
                get_meta(&cat.conn, "last_reconcile_at").unwrap(),
                Some("100000".to_string()),
                "last_reconcile_at must not move while throttled"
            );
        }
    }
}
