use crate::librarian::catalog::Catalog;
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRow {
    pub id: String,
    pub uri: String,
    pub kind: String, // 'chat'|'jira'|'gmail'|'confluence'|'drive'|'calendar'|'manual'
    pub payload: Option<String>,
    pub ingested_at: i64,
}

pub fn upsert(cat: &Catalog, s: &SourceRow) -> Result<()> {
    upsert_with(&cat.conn, s)
}

/// Upsert into an existing connection or transaction. Use this when the
/// caller wants atomicity across multiple writes.
pub fn upsert_with(conn: &rusqlite::Connection, s: &SourceRow) -> Result<()> {
    conn.execute(
        "INSERT INTO sources (id, uri, kind, payload, ingested_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            uri=excluded.uri, kind=excluded.kind,
            payload=excluded.payload, ingested_at=excluded.ingested_at",
        params![s.id, s.uri, s.kind, s.payload, s.ingested_at],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_replaces_payload() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut s = SourceRow {
            id: "chat:1".into(),
            uri: "x".into(),
            kind: "chat".into(),
            payload: Some("v1".into()),
            ingested_at: 1,
        };
        upsert(&cat, &s).unwrap();
        s.payload = Some("v2".into());
        upsert(&cat, &s).unwrap();
        let p: String = cat
            .conn
            .query_row(
                "SELECT payload FROM sources WHERE id=?1",
                params!["chat:1"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p, "v2");
    }

    #[test]
    fn rejects_unknown_kind() {
        let cat = Catalog::open_in_memory().unwrap();
        let s = SourceRow {
            id: "x".into(),
            uri: "u".into(),
            kind: "nonsense".into(),
            payload: None,
            ingested_at: 1,
        };
        assert!(upsert(&cat, &s).is_err());
    }
}
