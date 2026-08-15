//! Worktree overlay registration rows. One row per linked worktree that has
//! written to the catalog. `covering` is the hot lookup: "is this abs_path
//! inside an active worktree overlay?"
use anyhow::Result;
use rusqlite::{params, OptionalExtension};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationRow {
    pub worktree_root: String,
    pub main_root: String,
    pub branch: Option<String>,
    pub created_at: i64,
    pub status: String,
    pub closed_at: Option<i64>,
}

fn row_from_sql(r: &rusqlite::Row<'_>) -> rusqlite::Result<RegistrationRow> {
    Ok(RegistrationRow {
        worktree_root: r.get(0)?,
        main_root: r.get(1)?,
        branch: r.get(2)?,
        created_at: r.get(3)?,
        status: r.get(4)?,
        closed_at: r.get(5)?,
    })
}

const COLS: &str = "worktree_root, main_root, branch, created_at, status, closed_at";

/// Insert or re-activate a registration. Re-upserting a closed row (a re-used
/// worktree path) resets status to `active` and clears `closed_at`; the
/// original `created_at` is preserved on conflict.
pub fn upsert_active(
    cat: &Catalog,
    worktree_root: &str,
    main_root: &str,
    branch: Option<&str>,
    now: i64,
) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO worktree_registration (worktree_root, main_root, branch, created_at, status, closed_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', NULL) \
         ON CONFLICT(worktree_root) DO UPDATE SET \
           main_root=excluded.main_root, branch=excluded.branch, status='active', closed_at=NULL",
        params![worktree_root, main_root, branch, now],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, worktree_root: &str) -> Result<Option<RegistrationRow>> {
    Ok(cat
        .conn
        .query_row(
            &format!("SELECT {COLS} FROM worktree_registration WHERE worktree_root=?1"),
            [worktree_root],
            row_from_sql,
        )
        .optional()?)
}

/// The ACTIVE registration whose root is `abs_path` or an ancestor of it.
pub fn covering(cat: &Catalog, abs_path: &str) -> Result<Option<RegistrationRow>> {
    covering_conn(&cat.conn, abs_path)
}

/// Connection-level core of [`covering`], for call sites that only hold a
/// bare `&rusqlite::Connection` (e.g. `doctor.rs`, which doesn't otherwise
/// need a full `&Catalog`).
pub(crate) fn covering_conn(
    conn: &rusqlite::Connection,
    abs_path: &str,
) -> Result<Option<RegistrationRow>> {
    // `worktree_root` is a per-row COLUMN here, not a value held in Rust, so
    // the wildcard escaping has to happen inside the query —
    // `escape_like_pattern` cannot reach it. `descendant_path_like` is the
    // SQL-side twin that can; see its doc comment for why the escaping is
    // required at all.
    let under_root = crate::librarian::util::descendant_path_like("worktree_root");
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM worktree_registration \
                 WHERE status='active' AND (?1 = worktree_root OR ?1 {under_root})"
            ),
            [abs_path],
            row_from_sql,
        )
        .optional()?)
}

/// Roots of every ACTIVE registration (for scope exclusion clauses).
pub fn active_roots(cat: &Catalog) -> Result<Vec<String>> {
    let mut stmt = cat
        .conn
        .prepare("SELECT worktree_root FROM worktree_registration WHERE status='active' ORDER BY worktree_root")?;
    let roots = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(roots)
}

/// Returns false when no row exists for `worktree_root`.
pub fn set_status(cat: &Catalog, worktree_root: &str, status: &str, now: i64) -> Result<bool> {
    let n = cat.conn.execute(
        "UPDATE worktree_registration SET status=?2, closed_at=?3 WHERE worktree_root=?1",
        params![worktree_root, status, now],
    )?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;

    #[test]
    fn upsert_get_roundtrip_and_reactivation() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/feat", "/repo", Some("feat-x"), 1000).unwrap();
        let r = get(&cat, "/repo/.worktrees/feat").unwrap().unwrap();
        assert_eq!(r.main_root, "/repo");
        assert_eq!(r.status, "active");
        assert_eq!(r.branch.as_deref(), Some("feat-x"));
        // Re-upsert after close flips it back to active (a re-used worktree path).
        assert!(set_status(&cat, "/repo/.worktrees/feat", "merged", 2000).unwrap());
        upsert_active(&cat, "/repo/.worktrees/feat", "/repo", Some("feat-x"), 3000).unwrap();
        assert_eq!(
            get(&cat, "/repo/.worktrees/feat").unwrap().unwrap().status,
            "active"
        );
    }

    #[test]
    fn covering_matches_paths_under_active_root_only() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/feat", "/repo", None, 1000).unwrap();
        assert!(covering(&cat, "/repo/.worktrees/feat/docs/t.md")
            .unwrap()
            .is_some());
        assert!(covering(&cat, "/repo/.worktrees/feat").unwrap().is_some());
        assert!(covering(&cat, "/repo/.worktrees/feature-other/x.md")
            .unwrap()
            .is_none());
        assert!(covering(&cat, "/repo/docs/t.md").unwrap().is_none());
        set_status(&cat, "/repo/.worktrees/feat", "abandoned", 2000).unwrap();
        assert!(covering(&cat, "/repo/.worktrees/feat/docs/t.md")
            .unwrap()
            .is_none());
    }

    #[test]
    fn covering_escapes_like_wildcards_in_root() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/fix_1", "/repo", None, 1000).unwrap();
        // The stored '_' must be a literal, not a single-char wildcard.
        assert!(covering(&cat, "/repo/.worktrees/fixe1/readme.md")
            .unwrap()
            .is_none());
        assert!(covering(&cat, "/repo/.worktrees/fix.1/readme.md")
            .unwrap()
            .is_none());
        // The real path still matches.
        assert!(covering(&cat, "/repo/.worktrees/fix_1/readme.md")
            .unwrap()
            .is_some());
        assert!(covering(&cat, "/repo/.worktrees/fix_1").unwrap().is_some());
    }

    #[test]
    fn active_roots_lists_only_active() {
        let cat = Catalog::open_in_memory().unwrap();
        upsert_active(&cat, "/repo/.worktrees/a", "/repo", None, 1).unwrap();
        upsert_active(&cat, "/repo/.worktrees/b", "/repo", None, 2).unwrap();
        set_status(&cat, "/repo/.worktrees/b", "merged", 3).unwrap();
        assert_eq!(
            active_roots(&cat).unwrap(),
            vec!["/repo/.worktrees/a".to_string()]
        );
    }
}
