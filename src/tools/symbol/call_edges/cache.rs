use std::path::Path;

use rusqlite::{params, Connection};

use crate::tools::symbol::call_edges::resolver::{Edge, EdgeSource};

/// Applies the `call_edges` DDL to an in-memory (or any) connection.
/// Used by tests that need a bare DB without the full `open_db` setup.
pub fn apply_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS call_edges (
            project_id   TEXT NOT NULL,
            caller_sym   TEXT NOT NULL,
            callee_sym   TEXT NOT NULL,
            file         TEXT NOT NULL,
            line         INTEGER NOT NULL,
            col          INTEGER NOT NULL,
            source       TEXT NOT NULL,
            computed_at  INTEGER NOT NULL,
            PRIMARY KEY (project_id, caller_sym, callee_sym, file, line, col)
        );
        CREATE INDEX IF NOT EXISTS call_edges_caller ON call_edges(project_id, caller_sym);
        CREATE INDEX IF NOT EXISTS call_edges_callee ON call_edges(project_id, callee_sym);
        CREATE INDEX IF NOT EXISTS call_edges_file   ON call_edges(project_id, file);",
    )
    .expect("apply_schema: DDL failed");
}

pub struct EdgeCache<'a> {
    conn: &'a Connection,
    project_id: &'a str,
}

impl<'a> EdgeCache<'a> {
    pub fn new(conn: &'a Connection, project_id: &'a str) -> Self {
        Self { conn, project_id }
    }

    pub fn lookup_callers(&self, callee_sym: &str) -> rusqlite::Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT caller_sym, callee_sym, file, line, col, source \
             FROM call_edges WHERE project_id = ?1 AND callee_sym = ?2",
        )?;
        let edges = stmt
            .query_map(params![self.project_id, callee_sym], row_to_edge)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(edges)
    }

    pub fn lookup_callees(&self, caller_sym: &str) -> rusqlite::Result<Vec<Edge>> {
        let mut stmt = self.conn.prepare(
            "SELECT caller_sym, callee_sym, file, line, col, source \
             FROM call_edges WHERE project_id = ?1 AND caller_sym = ?2",
        )?;
        let edges = stmt
            .query_map(params![self.project_id, caller_sym], row_to_edge)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(edges)
    }

    pub fn upsert(&self, edges: &[Edge]) -> rusqlite::Result<()> {
        for edge in edges {
            let source_str = match edge.source {
                EdgeSource::Lsp => "lsp",
                EdgeSource::Ts => "ts",
            };
            self.conn.execute(
                "INSERT OR REPLACE INTO call_edges \
                 (project_id, caller_sym, callee_sym, file, line, col, source, computed_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%s','now'))",
                params![
                    self.project_id,
                    edge.caller_sym,
                    edge.callee_sym,
                    edge.file.to_string_lossy(),
                    edge.line,
                    edge.col,
                    source_str,
                ],
            )?;
        }
        Ok(())
    }

    pub fn invalidate_file(&self, file: &Path) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM call_edges WHERE project_id = ?1 AND file = ?2",
            params![self.project_id, file.to_string_lossy()],
        )
    }
}

fn row_to_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<Edge> {
    let source_str: String = row.get(5)?;
    Ok(Edge {
        caller_sym: row.get(0)?,
        callee_sym: row.get(1)?,
        file: std::path::PathBuf::from(row.get::<_, String>(2)?),
        line: row.get(3)?,
        col: row.get(4)?,
        source: if source_str == "lsp" {
            EdgeSource::Lsp
        } else {
            EdgeSource::Ts
        },
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;

    use super::*;
    use crate::tools::symbol::call_edges::resolver::EdgeSource;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn);
        conn
    }

    #[test]
    fn upsert_then_lookup_round_trip() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "test-project");
        let edge = Edge {
            caller_sym: "b".into(),
            callee_sym: "a".into(),
            file: PathBuf::from("a.rs"),
            line: 3,
            col: 0,
            source: EdgeSource::Lsp,
        };
        cache.upsert(&[edge.clone()]).unwrap();
        let got = cache.lookup_callers("a").unwrap();
        assert_eq!(got, vec![edge]);
    }

    #[test]
    fn lookup_callees_returns_correct_edges() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "test-project");
        let edge = Edge {
            caller_sym: "a".into(),
            callee_sym: "b".into(),
            file: PathBuf::from("b.rs"),
            line: 5,
            col: 4,
            source: EdgeSource::Ts,
        };
        cache.upsert(&[edge.clone()]).unwrap();
        let got = cache.lookup_callees("a").unwrap();
        assert_eq!(got, vec![edge]);
    }

    #[test]
    fn invalidate_file_removes_only_that_files_edges() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "test-project");
        let e1 = Edge {
            caller_sym: "b".into(),
            callee_sym: "a".into(),
            file: PathBuf::from("a.rs"),
            line: 3,
            col: 0,
            source: EdgeSource::Lsp,
        };
        let e2 = Edge {
            caller_sym: "c".into(),
            callee_sym: "a".into(),
            file: PathBuf::from("b.rs"),
            line: 7,
            col: 2,
            source: EdgeSource::Ts,
        };
        cache.upsert(&[e1.clone(), e2.clone()]).unwrap();
        let removed = cache.invalidate_file(Path::new("a.rs")).unwrap();
        assert_eq!(removed, 1);
        let remaining = cache.lookup_callers("a").unwrap();
        assert!(!remaining.contains(&e1));
        assert!(remaining.contains(&e2));
    }

    #[test]
    fn upsert_is_idempotent() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "test-project");
        let edge = Edge {
            caller_sym: "b".into(),
            callee_sym: "a".into(),
            file: PathBuf::from("a.rs"),
            line: 3,
            col: 0,
            source: EdgeSource::Lsp,
        };
        cache.upsert(&[edge.clone()]).unwrap();
        cache.upsert(&[edge.clone()]).unwrap(); // second upsert
        let got = cache.lookup_callers("a").unwrap();
        assert_eq!(got.len(), 1); // not doubled
    }

    #[test]
    fn project_isolation() {
        let conn = setup_db();
        let cache_a = EdgeCache::new(&conn, "project-a");
        let cache_b = EdgeCache::new(&conn, "project-b");
        let edge = Edge {
            caller_sym: "b".into(),
            callee_sym: "a".into(),
            file: PathBuf::from("a.rs"),
            line: 1,
            col: 0,
            source: EdgeSource::Lsp,
        };
        cache_a.upsert(&[edge.clone()]).unwrap();
        // project-b should not see project-a's edges
        let got = cache_b.lookup_callers("a").unwrap();
        assert!(got.is_empty());
    }
}
