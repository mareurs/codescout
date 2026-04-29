use crate::catalog::Catalog;
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRow {
    pub hash: String,
    pub repo: String,
    pub authored_at: Option<i64>,
    pub subject: Option<String>,
    pub topo_order: Option<i64>,
}

pub fn upsert_many(cat: &Catalog, rows: &[CommitRow]) -> Result<usize> {
    let tx = cat.conn.unchecked_transaction()?;
    let mut n = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO commits (hash, repo, authored_at, subject, topo_order)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(hash) DO UPDATE SET
               authored_at=excluded.authored_at,
               subject=excluded.subject,
               topo_order=COALESCE(excluded.topo_order, commits.topo_order)",
        )?;
        for r in rows {
            stmt.execute(params![
                r.hash,
                r.repo,
                r.authored_at,
                r.subject,
                r.topo_order
            ])?;
            n += 1;
        }
    }
    tx.commit()?;
    Ok(n)
}

/// Topo distance between two commits in the same repo. Returns None if either
/// commit is missing or `topo_order` is not yet computed.
pub fn topo_distance(cat: &Catalog, repo: &str, a: &str, b: &str) -> Result<Option<i64>> {
    let pair: (Option<i64>, Option<i64>) = cat.conn.query_row(
        "SELECT
            (SELECT topo_order FROM commits WHERE repo=?1 AND hash=?2),
            (SELECT topo_order FROM commits WHERE repo=?1 AND hash=?3)",
        params![repo, a, b],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(match pair {
        (Some(x), Some(y)) => Some((x - y).abs()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_then_topo_distance() {
        let cat = Catalog::open_in_memory().unwrap();
        let rows = vec![
            CommitRow {
                hash: "a".into(),
                repo: "r".into(),
                authored_at: Some(1),
                subject: Some("a".into()),
                topo_order: Some(0),
            },
            CommitRow {
                hash: "b".into(),
                repo: "r".into(),
                authored_at: Some(2),
                subject: Some("b".into()),
                topo_order: Some(1),
            },
            CommitRow {
                hash: "c".into(),
                repo: "r".into(),
                authored_at: Some(3),
                subject: Some("c".into()),
                topo_order: Some(2),
            },
        ];
        let n = upsert_many(&cat, &rows).unwrap();
        assert_eq!(n, 3);
        assert_eq!(topo_distance(&cat, "r", "a", "c").unwrap(), Some(2));
        assert_eq!(topo_distance(&cat, "r", "a", "missing").unwrap(), None);
    }

    #[test]
    fn upsert_is_idempotent() {
        let cat = Catalog::open_in_memory().unwrap();
        let row = CommitRow {
            hash: "a".into(),
            repo: "r".into(),
            authored_at: Some(1),
            subject: Some("a".into()),
            topo_order: Some(0),
        };
        upsert_many(&cat, &[row.clone()]).unwrap();
        upsert_many(&cat, &[row]).unwrap();
        let count: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM commits", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
