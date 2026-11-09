use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::tools::symbol::call_edges::resolver::{Direction, Edge, EdgeSource};

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
        CREATE INDEX IF NOT EXISTS call_edges_file   ON call_edges(project_id, file);
        CREATE TABLE IF NOT EXISTS call_edge_resolutions (
            project_id      TEXT NOT NULL,
            symbol          TEXT NOT NULL,
            definition_file TEXT NOT NULL,
            definition_line INTEGER NOT NULL,
            definition_col  INTEGER NOT NULL,
            direction       TEXT NOT NULL,
            edges_json      TEXT NOT NULL,
            PRIMARY KEY (project_id, symbol, definition_file, definition_line, definition_col, direction)
        );",
    )
    .expect("apply_schema: DDL failed");
}

/// Open (and create if missing) the call_edges sqlite database at
/// `.codescout/call_edges.db` under the given project root. Idempotent:
/// applies the schema on every open so older dbs auto-migrate when new
/// indexes are added.
///
/// This used to share `.codescout/embeddings/project.db` with the legacy
/// `embed::index` storage, but the two concerns were structurally
/// unrelated — call_edges is an LSP cache, not a semantic index — so the
/// L-01 retrieval-stack migration split them into separate files.
pub fn open_db(project_root: &std::path::Path) -> rusqlite::Result<Connection> {
    let dir = project_root.join(".codescout");
    std::fs::create_dir_all(&dir).map_err(|e| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
            Some(format!("create_dir_all({}): {e}", dir.display())),
        )
    })?;
    let db_path = dir.join("call_edges.db");
    let conn = Connection::open(&db_path)?;
    // WAL improves concurrent readers (LSP background traversals + foreground
    // tool calls). Busy timeout prevents transient SQLITE_BUSY when two
    // processes touch the cache (e.g. dashboard + MCP server).
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    apply_schema(&conn);
    Ok(conn)
}

/// Identity of one completed analysis, independent of its returned call-site edges.
#[derive(Clone, Copy, Debug)]
pub struct ResolutionKey<'a> {
    pub symbol: &'a str,
    pub file: &'a Path,
    pub line: u32,
    pub col: u32,
    pub direction: &'a Direction,
}

pub struct EdgeCache<'a> {
    conn: &'a Connection,
    project_id: &'a str,
}

impl<'a> EdgeCache<'a> {
    pub fn new(conn: &'a Connection, project_id: &'a str) -> Self {
        Self { conn, project_id }
    }
    /// Only completed results for this exact definition and direction are cache hits.
    pub fn lookup_resolution(
        &self,
        key: &ResolutionKey<'_>,
    ) -> rusqlite::Result<Option<Vec<Edge>>> {
        let direction = match key.direction {
            Direction::Callers => "callers",
            Direction::Callees => "callees",
        };
        let payload: Option<String> = self.conn.query_row(
            "SELECT edges_json FROM call_edge_resolutions WHERE project_id = ?1 AND symbol = ?2 \
             AND definition_file = ?3 AND definition_line = ?4 AND definition_col = ?5 AND direction = ?6",
            params![self.project_id, key.symbol, key.file.to_string_lossy(), key.line, key.col, direction],
            |row| row.get(0),
        ).optional()?;
        payload
            .map(|json| {
                serde_json::from_str(&json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .transpose()
    }

    /// Replace a successful adjacency; empty/recoverable analyses remain retryable.
    pub fn store_resolution(
        &self,
        key: &ResolutionKey<'_>,
        edges: &[Edge],
    ) -> rusqlite::Result<()> {
        if edges.is_empty() {
            return Ok(());
        }
        let direction = match key.direction {
            Direction::Callers => "callers",
            Direction::Callees => "callees",
        };
        let payload = serde_json::to_string(edges)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        self.conn.execute(
            "INSERT OR REPLACE INTO call_edge_resolutions \
             (project_id, symbol, definition_file, definition_line, definition_col, direction, edges_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![self.project_id, key.symbol, key.file.to_string_lossy(), key.line, key.col, direction, payload],
        )?;
        Ok(())
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
        if edges.is_empty() {
            return Ok(());
        }
        // unchecked_transaction: EdgeCache holds &Connection (not &mut), so the
        // safe transaction() API is unavailable. unchecked_transaction is sound
        // here because upsert is the only writer on this connection at call time.
        let tx = self.conn.unchecked_transaction()?;
        for edge in edges {
            let source_str = match edge.source {
                EdgeSource::Lsp => "lsp",
                EdgeSource::Ts => "ts",
            };
            tx.execute(
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
        tx.commit()
    }

    pub fn invalidate_file(&self, file: &Path) -> rusqlite::Result<usize> {
        let removed = self.conn.execute(
            "DELETE FROM call_edges WHERE project_id = ?1 AND file = ?2",
            params![self.project_id, file.to_string_lossy()],
        )?;
        // A changed caller can affect an incoming result owned by a different file.
        self.conn.execute(
            "DELETE FROM call_edge_resolutions WHERE project_id = ?1",
            params![self.project_id],
        )?;
        // Preserve the raw-edge count returned to existing invalidation consumers.
        Ok(removed)
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
        cache.upsert(std::slice::from_ref(&edge)).unwrap();
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
        cache.upsert(std::slice::from_ref(&edge)).unwrap();
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
        cache.upsert(std::slice::from_ref(&edge)).unwrap();
        cache.upsert(std::slice::from_ref(&edge)).unwrap(); // second upsert
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
        cache_a.upsert(std::slice::from_ref(&edge)).unwrap();
        // project-b should not see project-a's edges
        let got = cache_b.lookup_callers("a").unwrap();
        assert!(got.is_empty());
    }

    /// Three-query sandwich: proves that `invalidate_file` clears stale cache
    /// entries and that the cache is indeed stale before invalidation fires.
    ///
    /// Structure per CLAUDE.md § Testing Patterns:
    /// 1. Seed the cache with an edge from a.rs.
    /// 2. Query → assert edge is PRESENT (baseline).
    /// 3. (Simulate "file changed but cache not yet invalidated") — query again
    ///    → assert edge is STILL present (stale). This is the regression step.
    /// 4. Call `invalidate_file("a.rs")`.
    /// 5. Query → assert edge is GONE (fresh).
    #[test]
    fn invalidate_file_clears_stale_entries_three_query_sandwich() {
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

        // Step 1: seed.
        cache.upsert(std::slice::from_ref(&edge)).unwrap();

        // Step 2: baseline — edge is present.
        let before = cache.lookup_callers("a").unwrap();
        assert!(
            before.contains(&edge),
            "baseline: edge must be present after upsert"
        );

        // Step 3: stale-assertion — WITHOUT calling invalidate_file the edge
        // must still be present. This proves the invalidation isn't happening
        // automatically and that a future regression (e.g. eager re-read) will
        // be caught here.
        let still_stale = cache.lookup_callers("a").unwrap();
        assert!(
            still_stale.contains(&edge),
            "stale: edge must still be present before invalidation fires"
        );

        // Step 4: invalidate.
        let removed = cache.invalidate_file(Path::new("a.rs")).unwrap();
        assert_eq!(removed, 1, "invalidate_file must report 1 row deleted");

        // Step 5: fresh — edge must be gone.
        let after = cache.lookup_callers("a").unwrap();
        assert!(
            !after.contains(&edge),
            "fresh: edge must be absent after invalidation"
        );
    }
    fn resolution_key() -> ResolutionKey<'static> {
        ResolutionKey {
            symbol: "call",
            file: Path::new("/project/a.rs"),
            line: 7,
            col: 3,
            direction: &Direction::Callees,
        }
    }

    fn resolution_edge(callee: &str) -> Edge {
        Edge {
            caller_sym: "call".into(),
            callee_sym: callee.into(),
            file: PathBuf::from("/project/target.rs"),
            line: 11,
            col: 9,
            source: EdgeSource::Lsp,
        }
    }

    #[test]
    fn completed_resolution_keys_discriminate_every_identity_dimension() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "project-a");
        let key = resolution_key();
        let edges = vec![resolution_edge("leaf")];
        assert_eq!(cache.lookup_resolution(&key).unwrap(), None);
        cache.store_resolution(&key, &edges).unwrap();
        assert_eq!(cache.lookup_resolution(&key).unwrap(), Some(edges));
        for other in [
            ResolutionKey {
                symbol: "other",
                ..key
            },
            ResolutionKey {
                file: Path::new("/project/b.rs"),
                ..key
            },
            ResolutionKey { line: 8, ..key },
            ResolutionKey { col: 4, ..key },
            ResolutionKey {
                direction: &Direction::Callers,
                ..key
            },
        ] {
            assert_eq!(cache.lookup_resolution(&other).unwrap(), None, "{other:?}");
        }
        assert_eq!(
            EdgeCache::new(&conn, "project-b")
                .lookup_resolution(&key)
                .unwrap(),
            None
        );
    }

    #[test]
    fn completed_resolution_replaces_and_invalidates_only_its_project() {
        let conn = setup_db();
        let a = EdgeCache::new(&conn, "project-a");
        let b = EdgeCache::new(&conn, "project-b");
        let key = resolution_key();
        let first = vec![resolution_edge("first"), resolution_edge("second")];
        let replacement = vec![resolution_edge("replacement")];
        a.store_resolution(&key, &first).unwrap();
        a.store_resolution(&key, &replacement).unwrap();
        b.store_resolution(&key, &first).unwrap();
        assert_eq!(a.lookup_resolution(&key).unwrap(), Some(replacement));
        // A changed caller can affect a cached incoming graph owned by another file.
        assert_eq!(
            a.invalidate_file(Path::new("/project/unrelated-caller.rs"))
                .unwrap(),
            0
        );
        assert_eq!(a.lookup_resolution(&key).unwrap(), None);
        assert_eq!(b.lookup_resolution(&key).unwrap(), Some(first));
    }

    #[test]
    fn completed_resolution_does_not_persist_an_empty_analysis() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "project");
        let key = resolution_key();
        cache.store_resolution(&key, &[]).unwrap();
        assert_eq!(cache.lookup_resolution(&key).unwrap(), None);
        let edges = vec![resolution_edge("later")];
        cache.store_resolution(&key, &edges).unwrap();
        assert_eq!(cache.lookup_resolution(&key).unwrap(), Some(edges));
    }

    #[test]
    fn completed_resolution_reports_corrupt_payload_as_an_error() {
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "project");
        let key = resolution_key();
        cache
            .store_resolution(&key, &[resolution_edge("leaf")])
            .unwrap();
        conn.execute(
            "UPDATE call_edge_resolutions SET edges_json = 'invalid-json'",
            [],
        )
        .unwrap();
        assert!(
            cache.lookup_resolution(&key).is_err(),
            "corruption must not become an empty or missing graph"
        );
    }
}
