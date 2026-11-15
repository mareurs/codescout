//! The derived index's SQLite file: its schema, how it is opened, and a canonical dump.

#[cfg(test)]
use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::Connection;

/// The schema version stored in `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 1;
/// The file name of the new catalog, in the directory of the old one (D-R4).
pub const DERIVED_CATALOG_FILE: &str = "derived.db";

/// The new catalog for `old_catalog`: `derived.db` in the same directory, so
/// `VectorCache::beside` gives both the same `vector-cache.db` (D-R4). An old binary opens only
/// `LIBRARIAN_DB` or `<data dir>/librarian/catalog.db`, so it never opens this file.
pub fn derived_catalog_beside(old_catalog: &Path) -> std::path::PathBuf {
    old_catalog.with_file_name(DERIVED_CATALOG_FILE)
}

/// Milliseconds a connection waits on a lock held by another process.
const BUSY_TIMEOUT_MS: i64 = 5000;

/// Every table of schema version 1. All rows are deterministic: no default is a clock or a random
/// value. Every table except `checkout` (one row) and `problem` (a rowid table) has a natural key
/// naming its rows, not a generated id; `dump_all` sorts the rows of every table it dumps, so no
/// dump depends on a rowid or an insert order.
///
/// `skipped` is keyed on `(rel_path, reason)`: a lossy `not_utf8` path (U+FFFD is valid UTF-8) can
/// equal a real path's `rel_path`, so `rel_path` alone is not unique.
///
/// `edge.target_key` is the key as the citation wrote it, and for a malformed citation it holds
/// the target's raw text (which need not be a key at all): a reader that joins `target_key` to
/// `document.key` must filter on `kind != 'malformed'`. `edge.dst_key` is the key of the document
/// the citation resolved to (`kind` `file` or `entry`; NULL for every other kind), which for an
/// entry cited through a folder index is the owned document that defines the token, not the key
/// the citation named.
///
/// `parse_cache.metadata_json` is the document's `kind`, `status`, `title`, ... as parsed.
const SCHEMA_SQL: &str = r#"
CREATE TABLE checkout (
    root         TEXT NOT NULL,
    repo_name    TEXT NOT NULL,
    name_source  TEXT NOT NULL,
    rules_source TEXT NOT NULL,
    commit_sha   TEXT NOT NULL
);
CREATE TABLE document (
    rel_path   TEXT NOT NULL PRIMARY KEY,
    "key"      TEXT,
    key_error  TEXT,
    kind       TEXT NOT NULL,
    status     TEXT NOT NULL,
    title      TEXT,
    owners     TEXT NOT NULL,
    tags       TEXT NOT NULL,
    topic      TEXT,
    time_scope TEXT,
    confidence REAL NOT NULL,
    sha256     TEXT NOT NULL,
    owner_key  TEXT,
    parse_note TEXT
) WITHOUT ROWID;
CREATE TABLE entry (
    rel_path     TEXT NOT NULL,
    token        TEXT NOT NULL,
    level        INTEGER NOT NULL,
    heading_line INTEGER NOT NULL,
    end_line     INTEGER NOT NULL,
    PRIMARY KEY (rel_path, token, heading_line)
) WITHOUT ROWID;
CREATE TABLE edge (
    src_rel_path TEXT NOT NULL,
    ord          INTEGER NOT NULL,
    line         INTEGER NOT NULL,
    kind         TEXT NOT NULL,
    target_repo  TEXT,
    target_key   TEXT,
    target_token TEXT,
    dst_key      TEXT,
    PRIMARY KEY (src_rel_path, ord)
) WITHOUT ROWID;
CREATE TABLE chunk (
    chunk_id     TEXT NOT NULL PRIMARY KEY,
    rel_path     TEXT NOT NULL,
    chunk_ix     INTEGER NOT NULL,
    start_line   INTEGER NOT NULL,
    end_line     INTEGER NOT NULL,
    entry_token  TEXT,
    entry_part   INTEGER,
    entry_parts  INTEGER,
    content      TEXT NOT NULL,
    content_hash TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE problem (
    kind      TEXT NOT NULL,
    "key"     TEXT,
    rel_paths TEXT NOT NULL
);
CREATE TABLE skipped (
    rel_path TEXT NOT NULL,
    reason   TEXT NOT NULL,
    PRIMARY KEY (rel_path, reason)
) WITHOUT ROWID;
CREATE TABLE parse_cache (
    sha256         TEXT NOT NULL,
    rel_path       TEXT NOT NULL,
    rules_hash     TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    parse_note     TEXT,
    metadata_json  TEXT NOT NULL,
    entries_json   TEXT NOT NULL,
    citations_json TEXT NOT NULL,
    chunks_json    TEXT NOT NULL,
    PRIMARY KEY (sha256, rel_path, rules_hash, parser_version)
) WITHOUT ROWID;
"#;

/// Columns the dump leaves out. The root is a path: two clones of one commit lie at different
/// paths and must dump equal. The commit sha is not what two clones differ in (they share it); it
/// is what one tree committed twice differs in, and that tree must dump equal too.
#[cfg(test)]
const DUMP_EXCLUDED_COLUMNS: [(&str, &str); 2] = [("checkout", "root"), ("checkout", "commit_sha")];

/// The one table the dump leaves out: a cache of parse products, not an answer.
#[cfg(test)]
const DUMP_EXCLUDED_TABLE: &str = "parse_cache";

/// Open (creating the file and its parent directories) the derived index at `path`, with WAL, a
/// busy timeout and foreign keys on, its schema applied when the file is new, refused when the
/// file carries a newer `user_version`, and refused when it is another program's database
/// (`user_version` 0 with a table in it).
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| {
            failure(
                rusqlite::ffi::SQLITE_CANTOPEN,
                format!("create_dir_all({}): {e}", parent.display()),
            )
        })?;
    }
    configure_and_init(Connection::open(path)?, Some(path))
}

fn failure(code: i32, message: String) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), Some(message))
}

/// Set the pragmas, apply or refuse the schema, and only then switch a file to WAL.
///
/// WAL is recorded in the file header, so it is set after every refusal: a database that is
/// refused is left byte-for-byte as it was found. `path` is the path the caller gave (`None` for
/// an in-memory database, which has no journal mode to set).
fn configure_and_init(conn: Connection, path: Option<&Path>) -> rusqlite::Result<Connection> {
    conn.pragma_update(None, "busy_timeout", BUSY_TIMEOUT_MS)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    init_schema(&conn, path)?;
    if path.is_some() {
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }
    Ok(conn)
}

fn user_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
}

fn refuse_if_newer(found: i64) -> rusqlite::Result<()> {
    if found > SCHEMA_VERSION {
        return Err(failure(
            rusqlite::ffi::SQLITE_SCHEMA,
            format!(
                "derived index has user_version {found}, newer than schema {SCHEMA_VERSION} \
                 this build understands; refusing to open it"
            ),
        ));
    }
    Ok(())
}

/// Apply the schema when `user_version` is 0; refuse a newer one; leave a current one alone.
///
/// The check is repeated inside an immediate transaction, so two processes opening a new file
/// cannot both create the tables.
fn init_schema(conn: &Connection, path: Option<&Path>) -> rusqlite::Result<()> {
    let found = user_version(conn)?;
    refuse_if_newer(found)?;
    if found == SCHEMA_VERSION {
        return Ok(());
    }
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let found = user_version(&tx)?;
    refuse_if_newer(found)?;
    if found == 0 {
        refuse_if_foreign(&tx, path)?;
        tx.execute_batch(SCHEMA_SQL)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    tx.commit()
}

/// `user_version` 0 is the state of every SQLite file, so a version-0 file that already holds a
/// table is somebody else's database (the old `catalog.db`, say): the schema is not applied over it.
fn refuse_if_foreign(conn: &Connection, path: Option<&Path>) -> rusqlite::Result<()> {
    let tables: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
        [],
        |r| r.get(0),
    )?;
    if tables > 0 {
        // The path the caller gave, not `conn.path()`: SQLite resolves symlinks in the latter.
        let name = path.map_or_else(
            || "this in-memory database".to_owned(),
            |p| p.display().to_string(),
        );
        return Err(failure(
            rusqlite::ffi::SQLITE_NOTADB,
            format!(
                "{name} is not a derived index: it has user_version 0 and already holds {tables} \
                 table(s); refusing to apply the schema over it"
            ),
        ));
    }
    Ok(())
}

/// An in-memory derived index with the same schema, for tests.
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    configure_and_init(Connection::open_in_memory()?, None)
}

/// Every table except `parse_cache`, as `table -> rows`; the one oracle for "these two indexes
/// are the same".
///
/// A row is its columns, in table order, each rendered with a type tag so that values of different
/// SQLite types never compare equal (a missing title and the text `NULL`, the integer 1 and the
/// text `1`): `N` for NULL, `I:<n>` for an integer, `R:<f>` for a real (Rust `{:?}`), `T:<text>`
/// for text, `B:<hex>` for a blob. `checkout.root` and `checkout.commit_sha` are left out (two
/// clones of one commit differ in the path, and one tree committed twice differs in the sha; the
/// dump of each pair must be equal). The rows of a table are sorted by their rendered text, so
/// the dump reads no rowid and no insert order. A table with no rows is present with an empty
/// list. A database that cannot be read panics: this is a test instrument, compiled only for
/// tests, not a production path.
#[cfg(test)]
pub(crate) fn dump_all(conn: &Connection) -> BTreeMap<String, Vec<Vec<String>>> {
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .expect("list the derived index's tables");
    let mut dump = BTreeMap::new();
    for table in tables.into_iter().filter(|t| t != DUMP_EXCLUDED_TABLE) {
        let columns: Vec<String> = conn
            .prepare(&format!(
                "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
            ))
            .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
            .expect("list a derived table's columns");
        let columns: Vec<String> = columns
            .into_iter()
            .filter(|c| !DUMP_EXCLUDED_COLUMNS.contains(&(table.as_str(), c.as_str())))
            .collect();
        let select = columns
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = conn
            .prepare(&format!("SELECT {select} FROM \"{table}\""))
            .expect("prepare a derived table's dump");
        let mut rows: Vec<Vec<String>> = stmt
            .query_map([], |row| {
                (0..columns.len())
                    .map(|i| row.get_ref(i).map(render))
                    .collect()
            })
            .and_then(|mapped| mapped.collect())
            .expect("read a derived table");
        rows.sort();
        dump.insert(table, rows);
    }
    dump
}

/// Render one SQLite value with its type tag (see [`dump_all`]).
#[cfg(test)]
fn render(value: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => "N".to_owned(),
        ValueRef::Integer(i) => format!("I:{i}"),
        ValueRef::Real(f) => format!("R:{f:?}"),
        ValueRef::Text(t) => format!("T:{}", String::from_utf8_lossy(t)),
        ValueRef::Blob(b) => format!(
            "B:{}",
            b.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn user_version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    }

    /// Every `sqlite_master` row, as `(type, name, sql)`, in a fixed order.
    fn master_snapshot(conn: &Connection) -> Vec<(String, String, Option<String>)> {
        let mut stmt = conn
            .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn insert_document(conn: &Connection, rel_path: &str, key: &str) {
        conn.execute(
            "INSERT INTO document (rel_path, \"key\", key_error, kind, status, title, owners, \
             tags, topic, time_scope, confidence, sha256, owner_key, parse_note) \
             VALUES (?1, ?2, NULL, 'spec', 'draft', ?3, '[\"a\",\"b\"]', '[\"t\"]', NULL, NULL, \
             0.5, 'abc', NULL, NULL)",
            params![rel_path, key, format!("title of {rel_path}")],
        )
        .unwrap();
    }

    fn insert_edge(conn: &Connection, src: &str, ord: i64, line: i64) {
        conn.execute(
            "INSERT INTO edge (src_rel_path, ord, line, kind, target_repo, target_key, \
             target_token) VALUES (?1, ?2, ?3, 'file', NULL, 'k', NULL)",
            params![src, ord, line],
        )
        .unwrap();
    }

    fn insert_problem(conn: &Connection, kind: &str, key: &str) {
        conn.execute(
            "INSERT INTO problem (kind, key, rel_paths) VALUES (?1, ?2, '[\"x.md\",\"y.md\"]')",
            params![kind, key],
        )
        .unwrap();
    }

    fn insert_skipped(conn: &Connection, rel_path: &str, reason: &str) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO skipped (rel_path, reason) VALUES (?1, ?2)",
            params![rel_path, reason],
        )
    }

    #[test]
    fn open_creates_the_schema_once_and_a_second_open_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        // The parent directories do not exist yet: `open` must create them.
        let path = dir.path().join("a").join("b").join("derived.db");

        let conn = open(&path).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        assert_eq!(
            table_names(&conn),
            [
                "checkout",
                "chunk",
                "document",
                "edge",
                "entry",
                "parse_cache",
                "problem",
                "skipped"
            ]
        );
        insert_document(&conn, "docs/a.md", "k-a");
        let before = master_snapshot(&conn);
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        assert_eq!(
            master_snapshot(&conn),
            before,
            "a second open must not touch the schema"
        );
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM document", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "a second open must keep the data");
    }

    #[test]
    fn open_sets_the_pragmas() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("derived.db")).unwrap();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let busy: i64 = conn
            .pragma_query_value(None, "busy_timeout", |r| r.get(0))
            .unwrap();
        assert_eq!(busy, 5000);
        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
        let mem = open_in_memory().unwrap();
        assert_eq!(user_version(&mem), SCHEMA_VERSION);
    }

    #[test]
    fn a_newer_user_version_is_refused_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("derived.db");
        // Built by a plain connection (rollback journal, the SQLite default), so that a refused
        // `open` that switched the file to WAL would change its bytes.
        let newer = SCHEMA_VERSION + 6;
        {
            let raw = Connection::open(&path).unwrap();
            raw.pragma_update(None, "user_version", newer).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            before[18..20],
            [1, 1],
            "a rollback-journal file to start from"
        );

        let err = open(&path).expect_err("a newer schema must be refused");
        let text = err.to_string();
        assert!(
            text.contains(&format!("user_version {newer}"))
                && text.contains(&format!("schema {SCHEMA_VERSION}")),
            "the refusal names both versions: {text}"
        );
        let raw = Connection::open(&path).unwrap();
        assert_eq!(
            user_version(&raw),
            newer,
            "the refused file is left as it was"
        );
        drop(raw);
        assert!(
            std::fs::read(&path).unwrap() == before,
            "a refused open leaves the file byte-for-byte unchanged"
        );
    }

    #[test]
    fn dump_all_is_sorted_and_stable_across_insert_order() {
        let fill = |conn: &Connection, reverse: bool| {
            let mut docs = vec![
                ("docs/b.md", "k-b"),
                ("docs/a.md", "k-a"),
                ("docs/c.md", "k-c"),
            ];
            let mut edges = vec![
                ("docs/a.md", 1_i64, 7_i64),
                ("docs/a.md", 0, 3),
                ("docs/b.md", 0, 9),
            ];
            let mut problems = vec![("key_collision", "k-a"), ("key_collision", "k-b")];
            if reverse {
                docs.reverse();
                edges.reverse();
                problems.reverse();
            }
            for (p, k) in docs {
                insert_document(conn, p, k);
            }
            for (s, o, l) in edges {
                insert_edge(conn, s, o, l);
            }
            for (k, key) in problems {
                insert_problem(conn, k, key);
            }
            insert_skipped(conn, "z.md", "symlink").unwrap();
            insert_skipped(conn, "y.md", "not_utf8").unwrap();
        };
        let forward = open_in_memory().unwrap();
        fill(&forward, false);
        let backward = open_in_memory().unwrap();
        fill(&backward, true);

        let (a, b) = (dump_all(&forward), dump_all(&backward));
        assert_eq!(a, b, "the dump must not depend on insert order or rowids");

        // Sorted: document rows by their rendered text, edge rows likewise.
        let docs: Vec<&str> = a["document"].iter().map(|r| r[0].as_str()).collect();
        assert_eq!(docs, ["T:docs/a.md", "T:docs/b.md", "T:docs/c.md"]);
        let mut sorted = a["edge"].clone();
        sorted.sort();
        assert_eq!(a["edge"], sorted);
        assert_eq!(a["edge"][0][..3], ["T:docs/a.md", "I:0", "I:3"]);
        // Every table but parse_cache is present, empty ones included.
        let tables: Vec<&str> = a.keys().map(String::as_str).collect();
        assert_eq!(
            tables,
            ["checkout", "chunk", "document", "edge", "entry", "problem", "skipped"]
        );
        assert!(a["checkout"].is_empty() && a["chunk"].is_empty());
    }

    #[test]
    fn the_dump_leaves_out_the_clone_identity_columns() {
        let make = |root: &str, sha: &str| {
            let conn = open_in_memory().unwrap();
            conn.execute(
                "INSERT INTO checkout (root, repo_name, name_source, rules_source, commit_sha) \
                 VALUES (?1, 'repo-x', 'committed', 'committed+builtin', ?2)",
                params![root, sha],
            )
            .unwrap();
            conn
        };
        let one = dump_all(&make("/clone/one", "1111"));
        let two = dump_all(&make("/clone/two", "2222"));
        assert_eq!(
            one, two,
            "root and commit_sha are left out of the dump: two clones differ in the root, and one \
             tree committed twice differs in the commit_sha"
        );
        assert_eq!(
            one["checkout"],
            [["T:repo-x", "T:committed", "T:committed+builtin"]]
        );
    }
    #[test]
    fn an_edge_has_a_nullable_dst_key_after_the_target_columns_and_the_dump_shows_it() {
        let conn = open_in_memory().unwrap();
        let columns: Vec<(String, i64)> = conn
            .prepare("SELECT name, \"notnull\" FROM pragma_table_info('edge') ORDER BY cid")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let names: Vec<&str> = columns.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "src_rel_path",
                "ord",
                "line",
                "kind",
                "target_repo",
                "target_key",
                "target_token",
                "dst_key"
            ]
        );
        assert_eq!(columns.last().unwrap().1, 0, "dst_key may be NULL");

        for (ord, dst) in [(0_i64, Some("dst-doc")), (1, None)] {
            conn.execute(
                "INSERT INTO edge (src_rel_path, ord, line, kind, target_repo, target_key, \
                 target_token, dst_key) VALUES ('a.md', ?1, 1, 'file', NULL, 'k', NULL, ?2)",
                params![ord, dst],
            )
            .unwrap();
        }
        let dump = dump_all(&conn);
        let last: Vec<&str> = dump["edge"].iter().map(|r| r[7].as_str()).collect();
        assert_eq!(last, ["T:dst-doc", "N"]);
    }

    #[test]
    fn no_table_has_a_timestamp_or_random_default() {
        let conn = open_in_memory().unwrap();
        let mut found = Vec::new();

        // Strong form: the schema has no default and no generated column at all, so nothing in a
        // row can come from a clock or a random source. `table_xinfo` (not `table_info`) lists
        // generated columns too.
        for table in table_names(&conn) {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT name, dflt_value, hidden FROM pragma_table_xinfo('{table}')"
                ))
                .unwrap();
            let cols: Vec<(String, Option<String>, i64)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            for (col, dflt, hidden) in cols {
                if let Some(dflt) = dflt {
                    found.push(format!("{table}.{col} has DEFAULT {dflt}"));
                }
                if hidden != 0 {
                    found.push(format!("{table}.{col} is hidden or generated ({hidden})"));
                }
            }
        }
        // No trigger at all.
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'")
            .unwrap();
        let triggers: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        found.extend(triggers.into_iter().map(|t| format!("trigger {t}")));

        // Second check: no clock or random function named anywhere in the stored schema text.
        let banned = [
            "datetime",
            "random",
            "uuid",
            "current_timestamp",
            "current_date",
            "current_time",
            "strftime",
            "julianday",
            "unixepoch",
            "'now'",
        ];
        let mut stmt = conn
            .prepare("SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL")
            .unwrap();
        let objects: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for (name, sql) in objects {
            let sql = sql.to_lowercase();
            if let Some(word) = banned.iter().find(|b| sql.contains(**b)) {
                found.push(format!("{name} mentions {word}"));
            }
        }
        assert!(
            found.is_empty(),
            "a derived row must be deterministic: {found:?}"
        );
    }

    #[test]
    fn skipped_is_keyed_on_path_and_reason() {
        let conn = open_in_memory().unwrap();
        // A lossy `not_utf8` path (U+FFFD) can equal a real path: both rows must fit.
        insert_skipped(&conn, "docs/a\u{FFFD}.md", "not_utf8").unwrap();
        insert_skipped(&conn, "docs/a\u{FFFD}.md", "outside_key_space").unwrap();
        let err = insert_skipped(&conn, "docs/a\u{FFFD}.md", "not_utf8")
            .expect_err("the same (rel_path, reason) twice is a duplicate");
        assert!(err.to_string().contains("UNIQUE"), "{err}");
        assert_eq!(dump_all(&conn)["skipped"].len(), 2);
    }
    /// A document row whose only variable column is `title` (nullable TEXT).
    fn document_with_title(title: Option<&str>) -> Connection {
        let conn = open_in_memory().unwrap();
        insert_document(&conn, "docs/a.md", "k-a");
        conn.execute("UPDATE document SET title = ?1", params![title])
            .unwrap();
        conn
    }

    #[test]
    fn the_dump_tells_a_missing_value_from_the_text_null() {
        let missing = dump_all(&document_with_title(None));
        let text_null = dump_all(&document_with_title(Some("NULL")));
        let empty = dump_all(&document_with_title(Some("")));
        assert_ne!(
            missing, text_null,
            "NULL and the text NULL must dump differently"
        );
        assert_ne!(missing, empty);
        assert_ne!(text_null, empty);
        assert_eq!(missing["document"][0][5], "N");
        assert_eq!(text_null["document"][0][5], "T:NULL");
    }

    #[test]
    fn the_dump_tells_every_sqlite_type_apart() {
        use rusqlite::types::ValueRef;
        // No column of the schema keeps integer 1 and text "1" apart (type affinity converts
        // one into the other on insert), so the renderer is checked directly.
        let rendered = [
            render(ValueRef::Null),
            render(ValueRef::Integer(1)),
            render(ValueRef::Real(1.0)),
            render(ValueRef::Text(b"1")),
            render(ValueRef::Text(b"NULL")),
            render(ValueRef::Blob(&[1])),
        ];
        let distinct: std::collections::BTreeSet<&String> = rendered.iter().collect();
        assert_eq!(distinct.len(), rendered.len(), "{rendered:?}");
        assert_eq!(rendered, ["N", "I:1", "R:1.0", "T:1", "T:NULL", "B:01"]);
    }

    #[test]
    fn open_refuses_a_foreign_database_by_name_and_leaves_it_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        {
            let raw = Connection::open(&path).unwrap();
            raw.execute_batch("CREATE TABLE chunks (id TEXT); INSERT INTO chunks VALUES ('x');")
                .unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            before[18..20],
            [1, 1],
            "a rollback-journal file to start from"
        );

        let err = open(&path).expect_err("a foreign database must be refused");
        let text = err.to_string();
        assert!(
            text.contains(&path.display().to_string()) && text.contains("not a derived index"),
            "the refusal names the file: {text}"
        );
        assert!(
            std::fs::read(&path).unwrap() == before,
            "a refused open leaves the file byte-for-byte unchanged"
        );
        let raw = Connection::open(&path).unwrap();
        assert_eq!(table_names(&raw), ["chunks"], "no derived table was added");
        assert_eq!(user_version(&raw), 0);
        let rows: i64 = raw
            .query_row("SELECT count(*) FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
    #[cfg(unix)]
    #[test]
    fn the_refusal_names_the_path_as_the_caller_spelled_it() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        {
            let raw = Connection::open(real.join("catalog.db")).unwrap();
            raw.execute_batch("CREATE TABLE chunks (id TEXT);").unwrap();
        }

        // SQLite reports the resolved path (`real/catalog.db`); the message must not.
        let spelled = link.join("catalog.db");
        let err = open(&spelled).expect_err("a foreign database must be refused");
        let text = err.to_string();
        assert!(
            text.contains(&spelled.display().to_string()),
            "the refusal names the path the caller gave ({}): {text}",
            spelled.display()
        );
    }

    #[test]
    fn a_fresh_file_and_an_empty_existing_file_still_open() {
        let dir = tempfile::tempdir().unwrap();

        let fresh = dir.path().join("fresh.db");
        assert_eq!(user_version(&open(&fresh).unwrap()), SCHEMA_VERSION);

        // A zero-byte file.
        let zero = dir.path().join("zero.db");
        std::fs::write(&zero, b"").unwrap();
        assert_eq!(user_version(&open(&zero).unwrap()), SCHEMA_VERSION);

        // A real SQLite file with no table (one was created and dropped).
        let emptied = dir.path().join("emptied.db");
        {
            let raw = Connection::open(&emptied).unwrap();
            raw.execute_batch("CREATE TABLE t (x); DROP TABLE t;")
                .unwrap();
        }
        let conn = open(&emptied).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        assert_eq!(table_names(&conn).len(), 8);
    }
    // mutation: make `derived_catalog_beside` return the old path, or a file of another directory.
    #[test]
    fn the_new_catalog_sits_beside_the_old_one_and_shares_its_vector_cache() {
        use crate::librarian::vector_cache::{VectorCache, VECTOR_CACHE_FILE};
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("catalog.db");
        let new = derived_catalog_beside(&old);
        assert_eq!(new, dir.path().join("derived.db"));
        assert_eq!(new.file_name().unwrap(), DERIVED_CATALOG_FILE);

        let from_new = VectorCache::beside(Some(&new));
        let from_old = VectorCache::beside(Some(&old));
        assert_eq!(from_new.path(), from_old.path());
        assert_eq!(
            from_new.path().unwrap(),
            dir.path().join(VECTOR_CACHE_FILE),
            "both catalogs share one vector cache"
        );
    }
}
