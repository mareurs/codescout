use std::path::{Path, PathBuf};

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
            signatures_json TEXT,
            epoch           TEXT,
            PRIMARY KEY (project_id, symbol, definition_file, definition_line, definition_col, direction)
        );",
    )
    .expect("apply_schema: DDL failed");
    add_resolution_column_if_missing(conn, "signatures_json");
    add_resolution_column_if_missing(conn, "epoch");
}

/// A database created before a column of `call_edge_resolutions` existed (`signatures_json`,
/// `epoch`) keeps its table, so `CREATE TABLE IF NOT EXISTS` does not add the column. Add it once.
/// Two processes can open the same file at the same time (the MCP server and the dashboard), so
/// "the column is already there" after our own check is not an error.
///
/// `column` is one of the literals above, never input.
fn add_resolution_column_if_missing(conn: &Connection, column: &str) {
    let present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('call_edge_resolutions') WHERE name = ?1",
            [column],
            |row| row.get(0),
        )
        .expect("apply_schema: reading the table info failed");
    if present > 0 {
        return;
    }
    match conn.execute(
        &format!("ALTER TABLE call_edge_resolutions ADD COLUMN {column} TEXT"),
        [],
    ) {
        Ok(_) => {}
        Err(e) if e.to_string().contains("duplicate column") => {}
        Err(e) => panic!("apply_schema: adding {column} failed: {e}"),
    }
}

/// What the files behind one cached resolution looked like when it was stored: the `(len,
/// mtime)` pair that the LSP client uses to notice a file that changed behind its back
/// (`crate::lsp::client::disk_signature`), with the mtime as nanoseconds since the epoch so it
/// survives a JSON round trip. `None` means the file could not be read then.
type StoredSignatures = std::collections::BTreeMap<String, Option<(u64, Option<u64>)>>;

fn encode_signature(path: &Path) -> Option<(u64, Option<u64>)> {
    let (len, modified) = crate::lsp::client::disk_signature(path)?;
    let nanos = modified
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64);
    Some((len, nanos))
}

/// Every file that a resolution names: the definition it started from, the call sites and the
/// next-hop definitions. A change in any of them can change the result.
///
/// It does NOT name a file the result does not mention, so a caller added in such a file (a new
/// file, or one that had no call of the symbol) does not invalidate the entry. See bug
/// `489e013ddfd6c3fc`, `## Fix`.
fn files_behind(key: &ResolutionKey<'_>, edges: &[Edge]) -> std::collections::BTreeSet<PathBuf> {
    let mut files = std::collections::BTreeSet::new();
    files.insert(key.file.to_path_buf());
    for edge in edges {
        files.insert(edge.file.clone());
        if let Some(target) = &edge.target {
            files.insert(target.file.clone());
        }
    }
    files
}

fn current_signatures(files: &std::collections::BTreeSet<PathBuf>) -> StoredSignatures {
    files
        .iter()
        .map(|f| (f.to_string_lossy().into_owned(), encode_signature(f)))
        .collect()
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

/// How a lookup or a store treats the project-level epoch of a resolution
/// (`crate::tools::symbol::scan_budget::language_epoch`). A resolution is validated against the
/// files it names (`signatures_json`) and, with an epoch, against the whole tree of the seed's
/// language, which also catches a caller added in a file the resolution does not name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EpochCheck {
    /// The epoch is not consulted and none is stored. For callers that do not take part.
    #[default]
    Off,
    /// The epoch could not be computed (the walk was cut short): no resolution is a hit, and
    /// what is stored carries no epoch.
    Unknown,
    /// The epoch of the tree now: a resolution is a hit only if it was stored under the same one.
    Known(String),
}

pub struct EdgeCache<'a> {
    conn: &'a Connection,
    project_id: &'a str,
    epoch: EpochCheck,
}

impl<'a> EdgeCache<'a> {
    pub fn new(conn: &'a Connection, project_id: &'a str) -> Self {
        Self {
            conn,
            project_id,
            epoch: EpochCheck::Off,
        }
    }

    /// The same cache, taking part in the epoch check (see [`EpochCheck`]).
    pub fn with_epoch(mut self, epoch: EpochCheck) -> Self {
        self.epoch = epoch;
        self
    }

    /// Only completed results for this exact definition and direction are cache hits, and only
    /// while (1) every file the result names (`files_behind`) still has the `(len, mtime)` it had
    /// when the result was stored, and (2) the project-level epoch agrees (see [`EpochCheck`]),
    /// which also covers a file the result does not name. A file changed by anything but a
    /// codescout write tool, which invalidates by itself (`invalidate_file`), makes the entry a
    /// miss and the caller resolves again. An entry stored before the signatures existed has none
    /// and is a miss too.
    pub fn lookup_resolution(
        &self,
        key: &ResolutionKey<'_>,
    ) -> rusqlite::Result<Option<Vec<Edge>>> {
        let direction = match key.direction {
            Direction::Callers => "callers",
            Direction::Callees => "callees",
        };
        let row: Option<(String, Option<String>, Option<String>)> = self.conn.query_row(
            "SELECT edges_json, signatures_json, epoch FROM call_edge_resolutions WHERE project_id = ?1 AND symbol = ?2 \
             AND definition_file = ?3 AND definition_line = ?4 AND definition_col = ?5 AND direction = ?6",
            params![self.project_id, key.symbol, key.file.to_string_lossy(), key.line, key.col, direction],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()?;
        let Some((payload, signatures, stored_epoch)) = row else {
            return Ok(None);
        };
        let Some(signatures) = signatures else {
            return Ok(None);
        };
        // An unreadable signature record cannot vouch for the entry: resolve again.
        let Ok(stored) = serde_json::from_str::<StoredSignatures>(&signatures) else {
            return Ok(None);
        };
        if stored
            .iter()
            .any(|(file, signature)| encode_signature(Path::new(file)) != *signature)
        {
            return Ok(None);
        }
        match &self.epoch {
            EpochCheck::Off => {}
            // Nothing can vouch for the tree now: no stored result is a hit.
            EpochCheck::Unknown => return Ok(None),
            EpochCheck::Known(current) => {
                if stored_epoch.as_deref() != Some(current.as_str()) {
                    return Ok(None);
                }
            }
        }
        serde_json::from_str(&payload).map(Some).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    }

    /// Replace a successful adjacency; empty/recoverable analyses remain retryable.
    ///
    /// Stores, with the edges, the signature of every file the result names (`files_behind`) as
    /// they are NOW, and the epoch when the cache takes part in it (see [`EpochCheck`]; `Off` and
    /// `Unknown` store none). The signatures are read after the analysis, so a file that changed
    /// while the analysis ran is stored with its new signature next to edges computed from its old
    /// content; that window is the length of one analysis and is accepted, where before the entry
    /// was never revalidated at all. The epoch is computed once per tool call, BEFORE the analysis,
    /// so a change during it makes the stored epoch older than the tree and the next call a miss.
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
        let signatures = serde_json::to_string(&current_signatures(&files_behind(key, edges)))
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let epoch: Option<&str> = match &self.epoch {
            EpochCheck::Known(epoch) => Some(epoch.as_str()),
            EpochCheck::Off | EpochCheck::Unknown => None,
        };
        self.conn.execute(
            "INSERT OR REPLACE INTO call_edge_resolutions \
             (project_id, symbol, definition_file, definition_line, definition_col, direction, edges_json, signatures_json, epoch) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![self.project_id, key.symbol, key.file.to_string_lossy(), key.line, key.col, direction, payload, signatures, epoch],
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
        target: None,
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
            target: None,
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
            target: None,
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
            target: None,
        };
        let e2 = Edge {
            caller_sym: "c".into(),
            callee_sym: "a".into(),
            file: PathBuf::from("b.rs"),
            line: 7,
            col: 2,
            source: EdgeSource::Ts,
            target: None,
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
            target: None,
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
            target: None,
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
            target: None,
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
            target: None,
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

    // ── A cached resolution is revalidated against the files it names (bug 489e013ddfd6c3fc) ──

    fn key_for(file: &Path) -> ResolutionKey<'_> {
        ResolutionKey {
            symbol: "call",
            file,
            line: 0,
            col: 3,
            direction: &Direction::Callers,
        }
    }

    fn call_site_edge(file: &Path) -> Edge {
        Edge {
            caller_sym: "caller".into(),
            callee_sym: "call".into(),
            file: file.to_path_buf(),
            line: 1,
            col: 14,
            source: EdgeSource::Lsp,
            target: None,
        }
    }

    fn pin_mtime(file: &Path, secs: i64) {
        filetime::set_file_mtime(file, filetime::FileTime::from_unix_time(secs, 0)).unwrap();
    }

    /// A temp project with `a.rs` (the definition) and `b.rs` (a call site), and one stored
    /// resolution that names both. Both mtimes are pinned, so a later change is a change.
    fn stored_resolution_over_two_real_files() -> (tempfile::TempDir, Connection, PathBuf, PathBuf)
    {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        std::fs::write(&a, "fn call() {}\n").unwrap();
        std::fs::write(&b, "fn caller() { call(); }\n").unwrap();
        pin_mtime(&a, 1_600_000_000);
        pin_mtime(&b, 1_600_000_000);
        let conn = setup_db();
        EdgeCache::new(&conn, "project")
            .store_resolution(&key_for(&a), &[call_site_edge(&b)])
            .unwrap();
        (dir, conn, a, b)
    }

    #[test]
    fn a_stored_resolution_is_a_hit_while_every_file_it_names_is_unchanged() {
        let (_dir, conn, a, b) = stored_resolution_over_two_real_files();
        let got = EdgeCache::new(&conn, "project")
            .lookup_resolution(&key_for(&a))
            .unwrap();
        assert_eq!(got, Some(vec![call_site_edge(&b)]));
    }

    #[test]
    fn a_stored_resolution_is_a_miss_once_the_definition_file_changed() {
        let (_dir, conn, a, _b) = stored_resolution_over_two_real_files();
        std::fs::write(&a, "fn call() {}\nfn other() {}\n").unwrap();
        let got = EdgeCache::new(&conn, "project")
            .lookup_resolution(&key_for(&a))
            .unwrap();
        assert_eq!(got, None, "the file the resolution started from changed");
    }

    /// The case of the original report: an edit that moves nothing in the target file and keeps
    /// the size of the call-site file. Only the mtime tells.
    #[test]
    fn a_stored_resolution_is_a_miss_once_a_call_site_file_changed_with_the_same_length() {
        let (_dir, conn, a, b) = stored_resolution_over_two_real_files();
        std::fs::write(&b, "fn caller() { cell(); }\n").unwrap();
        pin_mtime(&b, 1_600_000_060);
        let got = EdgeCache::new(&conn, "project")
            .lookup_resolution(&key_for(&a))
            .unwrap();
        assert_eq!(got, None, "a call-site file changed behind the cache");
    }

    #[test]
    fn a_file_that_appears_after_the_resolution_was_stored_makes_it_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        std::fs::write(&a, "fn call() {}\n").unwrap();
        let conn = setup_db();
        let cache = EdgeCache::new(&conn, "project");
        cache
            .store_resolution(&key_for(&a), &[call_site_edge(&b)])
            .unwrap();
        assert!(
            cache.lookup_resolution(&key_for(&a)).unwrap().is_some(),
            "a file absent when stored and still absent has not changed"
        );
        std::fs::write(&b, "fn caller() { call(); }\n").unwrap();
        assert_eq!(cache.lookup_resolution(&key_for(&a)).unwrap(), None);
    }

    #[test]
    fn a_resolution_stored_without_signatures_is_a_miss() {
        let (_dir, conn, a, _b) = stored_resolution_over_two_real_files();
        conn.execute(
            "UPDATE call_edge_resolutions SET signatures_json = NULL",
            [],
        )
        .unwrap();
        let got = EdgeCache::new(&conn, "project")
            .lookup_resolution(&key_for(&a))
            .unwrap();
        assert_eq!(
            got, None,
            "an entry that cannot be revalidated is not served"
        );
    }

    #[test]
    fn an_old_database_gets_the_signatures_column_and_its_rows_are_misses() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE call_edge_resolutions (
                project_id TEXT NOT NULL, symbol TEXT NOT NULL, definition_file TEXT NOT NULL,
                definition_line INTEGER NOT NULL, definition_col INTEGER NOT NULL,
                direction TEXT NOT NULL, edges_json TEXT NOT NULL,
                PRIMARY KEY (project_id, symbol, definition_file, definition_line, definition_col, direction)
             );
             INSERT INTO call_edge_resolutions
                VALUES ('project', 'call', '/project/a.rs', 7, 3, 'callees', '[]');",
        )
        .unwrap();
        apply_schema(&conn);
        apply_schema(&conn); // idempotent: the column is added once
        for column in ["signatures_json", "epoch"] {
            let present: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('call_edge_resolutions') \
                     WHERE name = ?1",
                    [column],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(present, 1, "column {column} must exist after apply_schema");
        }
        assert_eq!(
            EdgeCache::new(&conn, "project")
                .lookup_resolution(&resolution_key())
                .unwrap(),
            None,
            "a row stored before the signatures existed must be resolved again"
        );
    }

    // ── The project-level epoch: a result is also valid only for the tree it was stored under ──

    fn store_under(conn: &Connection, epoch: EpochCheck, file: &Path) {
        EdgeCache::new(conn, "project")
            .with_epoch(epoch)
            .store_resolution(&key_for(file), &[call_site_edge(file)])
            .unwrap();
    }

    fn lookup_under(conn: &Connection, epoch: EpochCheck, file: &Path) -> Option<Vec<Edge>> {
        EdgeCache::new(conn, "project")
            .with_epoch(epoch)
            .lookup_resolution(&key_for(file))
            .unwrap()
    }

    fn known(epoch: &str) -> EpochCheck {
        EpochCheck::Known(epoch.to_string())
    }

    fn a_real_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn call() {}\n").unwrap();
        (dir, file)
    }

    #[test]
    fn a_resolution_is_a_hit_only_under_the_epoch_it_was_stored_under() {
        let (_dir, a) = a_real_file();
        let conn = setup_db();
        store_under(&conn, known("tree-1"), &a);
        assert_eq!(
            lookup_under(&conn, known("tree-1"), &a),
            Some(vec![call_site_edge(&a)]),
            "the same tree"
        );
        assert_eq!(
            lookup_under(&conn, known("tree-2"), &a),
            None,
            "a file of the tree changed, which the signatures of the named files cannot see"
        );
    }

    #[test]
    fn an_unknown_epoch_is_never_a_hit_and_stores_none() {
        let (_dir, a) = a_real_file();
        let conn = setup_db();
        store_under(&conn, EpochCheck::Unknown, &a);
        assert_eq!(lookup_under(&conn, EpochCheck::Unknown, &a), None);
        assert_eq!(
            lookup_under(&conn, known("tree-1"), &a),
            None,
            "what an Unknown cache stored carries no epoch, so a Known one cannot vouch for it"
        );
        assert!(
            lookup_under(&conn, EpochCheck::Off, &a).is_some(),
            "an Off cache does not consult the epoch, and the entry itself is valid"
        );
    }

    #[test]
    fn a_resolution_stored_without_an_epoch_is_a_miss_under_a_known_epoch() {
        let (_dir, a) = a_real_file();
        let conn = setup_db();
        store_under(&conn, EpochCheck::Off, &a);
        assert_eq!(lookup_under(&conn, known("tree-1"), &a), None);
    }
}
