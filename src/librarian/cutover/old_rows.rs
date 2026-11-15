//! The rows of the old catalog that lie under one repository, read once.
//!
//! [`load`] runs its own SQL on the connection of an [`OldCatalog`], inside one read snapshot that
//! is dropped without a commit, so no row is written. It prepares every query before it reads a
//! row, so a missing table or column is named before any work starts. An artifact is in scope when
//! [`RootMap::scope`] says `Inside`; the rows of every other artifact are counted per root, never
//! loaded.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use rusqlite::{Connection, Statement};
use serde::Serialize;

use crate::librarian::catalog::augmentation::{
    row_from_sql, AugmentationRow, AUGMENTATION_COLUMNS,
};
use crate::librarian::derived::diff::{OldCatalog, RootMap, Scope};

/// An artifact row that lies under the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldDocument {
    pub id: String,
    /// Git-root-relative, `/`-separated.
    pub rel_path: String,
    /// The catalog marked the file missing (a catalog GC leftover).
    pub missing: bool,
}

/// The source an event was triggered by.
#[derive(Debug, Clone, PartialEq)]
pub struct OldSource {
    pub kind: String,
    pub uri: String,
    pub payload: Option<serde_json::Value>,
}

/// An event row with its edges folded in.
#[derive(Debug, Clone, PartialEq)]
pub struct OldEvent {
    pub id: String,
    pub artifact_id: String,
    pub kind: String,
    /// The payload as JSON. Text that is not JSON is carried as a JSON string.
    pub payload: serde_json::Value,
    pub anchor_commit: Option<String>,
    pub head_commit: Option<String>,
    pub author: Option<String>,
    pub created_at_ms: i64,
    /// The event this one follows (`parent` edge).
    pub parent: Option<String>,
    /// The intent this event resolves (`resolves` edge).
    pub resolves: Option<String>,
    /// The artifact ids this event mutates (`mutates` edges), in edge order.
    pub mutates: Vec<String>,
    /// The source that triggered the event (`triggered_by` edge).
    pub source: Option<OldSource>,
}

/// An observation row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldObservation {
    pub id: i64,
    pub artifact_id: String,
    pub text: String,
    pub source: Option<String>,
    pub created_at_ms: i64,
}

/// A manual link: an `artifact_link` row that `link_scan` did not write (`cites`) and that is not
/// overlay lineage (`worktree_of`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldLink {
    pub src_id: String,
    pub dst_id: String,
    pub rel: String,
}

/// How many artifact rows the load left out, by why.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct OutOfScope {
    /// Rows whose path lies outside the git root and every `--old-root`.
    pub other_roots: usize,
    /// Rows under `<root>/.worktrees/`.
    pub worktrees: usize,
}

/// Every row of the old catalog that lies under the repository.
#[derive(Debug, Default)]
pub struct OldRows {
    /// In-scope artifacts, by id. Two live rows may name one path (the same file under two
    /// spellings of the root, each hashed into its own id): both are kept, so a later step that
    /// works per path groups them (the cutover reports the params of both and exports the path
    /// once).
    pub documents: BTreeMap<String, OldDocument>,
    pub augmentations: Vec<AugmentationRow>,
    pub events: Vec<OldEvent>,
    pub observations: Vec<OldObservation>,
    pub links: Vec<OldLink>,
    /// `entry_cite` rows with `origin = 'write'` whose source document is in scope.
    pub write_cites: usize,
    pub out_of_scope: OutOfScope,
    /// The spellings of the repository root the rows were judged against: the git root, then
    /// each `--old-root`. An old artifact id hashes `<one of these>/<path>`.
    pub spellings: Vec<PathBuf>,
}

/// Load the in-scope rows of `old`.
///
/// Every query is prepared first, in the order below, so a missing table or column fails before
/// any row is read; the error names SQLite's reason (`no such table: events`). The reads happen in
/// one snapshot that is dropped without a commit.
pub fn load(old: &OldCatalog, roots: &RootMap) -> anyhow::Result<OldRows> {
    let snapshot = old
        .connection()
        .unchecked_transaction()
        .context("starting a read snapshot of the old catalog")?;
    let mut queries = Queries::prepare(&snapshot)?;

    let mut rows = OldRows {
        spellings: roots
            .spellings()
            .into_iter()
            .map(Path::to_path_buf)
            .collect(),
        ..OldRows::default()
    };
    load_documents(&mut queries.documents, roots, &mut rows)?;
    let events = load_events(&mut queries.events, &rows.documents)?;
    let sources = load_sources(&mut queries.sources)?;
    rows.events = attach_edges(&mut queries.edges, events, &sources)?;
    rows.observations = load_observations(&mut queries.observations, &rows.documents)?;
    rows.links = load_links(&mut queries.links, &rows.documents)?;
    rows.write_cites = count_write_cites(&mut queries.write_cites, &rows.documents)?;
    rows.augmentations = load_augmentations(&mut queries.augmentations, &rows.documents)?;
    Ok(rows)
}

const DOCUMENTS_SQL: &str =
    "SELECT id, abs_path, missing_since IS NOT NULL FROM artifact ORDER BY id";
const EVENTS_SQL: &str = "SELECT id, artifact_id, kind, payload, anchor_commit, head_commit, \
     author, created_at FROM events ORDER BY created_at, id";
const EDGES_SQL: &str = "SELECT src_event_id, rel, dst_event_id, dst_artifact_id, dst_source_id \
     FROM event_edges WHERE rel IN ('parent', 'resolves', 'mutates', 'triggered_by') ORDER BY id";
const SOURCES_SQL: &str = "SELECT id, uri, kind, payload FROM sources";
const OBSERVATIONS_SQL: &str =
    "SELECT id, artifact_id, text, source, created_at FROM artifact_observation ORDER BY id";
const LINKS_SQL: &str = "SELECT src_id, dst_id, rel FROM artifact_link \
     WHERE rel NOT IN ('cites', 'worktree_of') ORDER BY src_id, dst_id, rel";
const WRITE_CITES_SQL: &str = "SELECT a.id, count(*) FROM entry_cite c \
     JOIN artifact a ON a.slug = c.src_slug WHERE c.origin = 'write' GROUP BY a.id";

/// Every statement the load runs, prepared before the first row is read.
struct Queries<'c> {
    documents: Statement<'c>,
    events: Statement<'c>,
    edges: Statement<'c>,
    sources: Statement<'c>,
    observations: Statement<'c>,
    links: Statement<'c>,
    write_cites: Statement<'c>,
    augmentations: Statement<'c>,
}

impl<'c> Queries<'c> {
    /// `events` comes right after `artifact`, so a catalog that has only the artifact tables is
    /// refused with `events` named.
    fn prepare(conn: &'c Connection) -> anyhow::Result<Self> {
        let prepare = |sql: &str| {
            conn.prepare(sql)
                .map_err(|e| anyhow!("the old catalog cannot be read: {e}"))
        };
        Ok(Queries {
            documents: prepare(DOCUMENTS_SQL)?,
            events: prepare(EVENTS_SQL)?,
            edges: prepare(EDGES_SQL)?,
            sources: prepare(SOURCES_SQL)?,
            observations: prepare(OBSERVATIONS_SQL)?,
            links: prepare(LINKS_SQL)?,
            write_cites: prepare(WRITE_CITES_SQL)?,
            augmentations: prepare(&format!(
                "SELECT {AUGMENTATION_COLUMNS} FROM artifact_augmentation ORDER BY artifact_id"
            ))?,
        })
    }
}

/// Fill `rows.documents` and the out-of-scope counts. A document is in scope when its path is
/// inside the git root (or an old root) and outside `.worktrees/`; every other row is counted in
/// one bucket.
fn load_documents(
    stmt: &mut Statement<'_>,
    roots: &RootMap,
    rows: &mut OldRows,
) -> anyhow::Result<()> {
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let id: String = row.get(0)?;
        let abs_path: String = row.get(1)?;
        let missing: bool = row.get(2)?;
        match roots.scope(&abs_path) {
            Scope::Inside(rel_path) => {
                rows.documents.insert(
                    id.clone(),
                    OldDocument {
                        id,
                        rel_path,
                        missing,
                    },
                );
            }
            Scope::OtherRoot => rows.out_of_scope.other_roots += 1,
            Scope::Worktree => rows.out_of_scope.worktrees += 1,
        }
    }
    Ok(())
}

/// The payload as JSON; text that does not parse is carried as a JSON string.
fn json_or_text(text: String) -> serde_json::Value {
    serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text))
}

/// The events of in-scope artifacts, without their edges.
fn load_events(
    stmt: &mut Statement<'_>,
    documents: &BTreeMap<String, OldDocument>,
) -> anyhow::Result<Vec<OldEvent>> {
    let mut events = Vec::new();
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let artifact_id: String = row.get(1)?;
        if !documents.contains_key(&artifact_id) {
            continue;
        }
        events.push(OldEvent {
            id: row.get(0)?,
            artifact_id,
            kind: row.get(2)?,
            payload: json_or_text(row.get(3)?),
            anchor_commit: row.get(4)?,
            head_commit: row.get(5)?,
            author: row.get(6)?,
            created_at_ms: row.get(7)?,
            parent: None,
            resolves: None,
            mutates: Vec::new(),
            source: None,
        });
    }
    Ok(events)
}

fn load_sources(stmt: &mut Statement<'_>) -> anyhow::Result<HashMap<String, OldSource>> {
    let mut sources = HashMap::new();
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let id: String = row.get(0)?;
        let payload: Option<String> = row.get(3)?;
        sources.insert(
            id,
            OldSource {
                uri: row.get(1)?,
                kind: row.get(2)?,
                payload: payload.map(json_or_text),
            },
        );
    }
    Ok(sources)
}

/// Fold each event's `parent`, `resolves`, `mutates` and `triggered_by` edges into the event. An
/// event with two edges of a single-valued relation keeps the first (lowest edge id).
fn attach_edges(
    stmt: &mut Statement<'_>,
    mut events: Vec<OldEvent>,
    sources: &HashMap<String, OldSource>,
) -> anyhow::Result<Vec<OldEvent>> {
    let at: HashMap<String, usize> = events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.clone(), i))
        .collect();
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let src: String = row.get(0)?;
        let Some(&i) = at.get(&src) else { continue };
        let rel: String = row.get(1)?;
        let event = &mut events[i];
        match rel.as_str() {
            "parent" if event.parent.is_none() => event.parent = row.get(2)?,
            "resolves" if event.resolves.is_none() => event.resolves = row.get(2)?,
            "mutates" => {
                let artifact: Option<String> = row.get(3)?;
                event.mutates.extend(artifact);
            }
            "triggered_by" if event.source.is_none() => {
                let source: Option<String> = row.get(4)?;
                event.source = source.and_then(|id| sources.get(&id).cloned());
            }
            _ => {}
        }
    }
    Ok(events)
}

fn load_observations(
    stmt: &mut Statement<'_>,
    documents: &BTreeMap<String, OldDocument>,
) -> anyhow::Result<Vec<OldObservation>> {
    let mut observations = Vec::new();
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let artifact_id: String = row.get(1)?;
        if !documents.contains_key(&artifact_id) {
            continue;
        }
        observations.push(OldObservation {
            id: row.get(0)?,
            artifact_id,
            text: row.get(2)?,
            source: row.get(3)?,
            created_at_ms: row.get(4)?,
        });
    }
    Ok(observations)
}

/// The manual links whose source is in scope. The destination may lie anywhere: a later step
/// decides what a link to a document outside the repository becomes.
fn load_links(
    stmt: &mut Statement<'_>,
    documents: &BTreeMap<String, OldDocument>,
) -> anyhow::Result<Vec<OldLink>> {
    let mut links = Vec::new();
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let src_id: String = row.get(0)?;
        if !documents.contains_key(&src_id) {
            continue;
        }
        links.push(OldLink {
            src_id,
            dst_id: row.get(1)?,
            rel: row.get(2)?,
        });
    }
    Ok(links)
}

fn count_write_cites(
    stmt: &mut Statement<'_>,
    documents: &BTreeMap<String, OldDocument>,
) -> anyhow::Result<usize> {
    let mut total = 0usize;
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let artifact_id: String = row.get(0)?;
        if documents.contains_key(&artifact_id) {
            let n: i64 = row.get(1)?;
            total += usize::try_from(n).unwrap_or(0);
        }
    }
    Ok(total)
}

fn load_augmentations(
    stmt: &mut Statement<'_>,
    documents: &BTreeMap<String, OldDocument>,
) -> anyhow::Result<Vec<AugmentationRow>> {
    let mut augmentations = Vec::new();
    let mut found = stmt.query([])?;
    while let Some(row) = found.next()? {
        let augmentation = row_from_sql(row)?;
        if documents.contains_key(&augmentation.artifact_id) {
            augmentations.push(augmentation);
        }
    }
    Ok(augmentations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
    use crate::librarian::catalog::links::{self, LinkRow};
    use crate::librarian::catalog::observations::{self, ObservationRow};
    use crate::librarian::catalog::sources::{self, SourceRow};
    use crate::librarian::cutover::testkit::*;
    use crate::librarian::derived::testkit::{fill_old_catalog, OLD_CATALOG_SCHEMA};
    use std::path::{Path, PathBuf};

    const ROOT: &str = "/work/proj";

    fn roots() -> RootMap {
        RootMap::new(PathBuf::from(ROOT))
    }

    fn doc(cat: &crate::librarian::catalog::Catalog, id: &str, rel: &str) -> String {
        add_doc(cat, Path::new(ROOT), id, rel)
    }

    /// Two live rows may name one path: an old artifact id hashes `<spelling of the root>/<path>`,
    /// so the same file under two spellings is two rows. `documents` is keyed by id and keeps both.
    /// Later steps group them by path (the cutover reports the params of both and exports once).
    #[test]
    fn two_live_rows_naming_one_path_are_both_kept() {
        let old = old_catalog_with(|cat| {
            add_doc_at(cat, "d1", "/work/proj/docs/l.md");
            add_doc_at(cat, "d2", "/old/spelling/docs/l.md");
            add_augmentation(cat, "d1", r#"{"a":1}"#, None);
            add_augmentation(cat, "d2", r#"{"b":2}"#, None);
        });
        let map =
            RootMap::with_old_roots(PathBuf::from(ROOT), vec![PathBuf::from("/old/spelling")])
                .unwrap();
        let rows = load(&old, &map).unwrap();
        assert_eq!(rows.documents.len(), 2);
        assert_eq!(rows.documents["d1"].rel_path, "docs/l.md");
        assert_eq!(rows.documents["d2"].rel_path, "docs/l.md");
        let ids: Vec<&str> = rows
            .augmentations
            .iter()
            .map(|a| a.artifact_id.as_str())
            .collect();
        assert_eq!(ids, ["d1", "d2"], "both augmentations are loaded, by id");
    }

    #[test]
    fn load_keeps_rows_under_the_root_and_counts_the_rest() {
        let old = old_catalog_with(|cat| {
            doc(cat, "a", "a.md");
            doc(cat, "b", "sub/b.md");
            doc(cat, "m", "gone.md");
            cat.conn
                .execute("UPDATE artifact SET missing_since = 5 WHERE id = 'm'", [])
                .unwrap();
            add_doc_at(cat, "c", "/other/c.md");
            add_doc_at(cat, "w", "/work/proj/.worktrees/x/w.md");
            add_doc_at(cat, "d", "/work/proj2/d.md");
            for (n, art) in ["a", "b", "m", "c", "w", "d"].iter().enumerate() {
                add_event(cat, &format!("ev{n}"), art, "note", r#"{"text":"t"}"#);
                add_augmentation(cat, art, "{}", None);
                observations::insert(
                    cat,
                    &ObservationRow {
                        id: None,
                        artifact_id: (*art).to_owned(),
                        text: format!("obs {art}"),
                        source: None,
                        created_at: 7,
                    },
                )
                .unwrap();
            }
        });
        let rows = load(&old, &roots()).unwrap();
        let ids: Vec<&str> = rows.documents.keys().map(String::as_str).collect();
        assert_eq!(ids, ["a", "b", "m"]);
        assert_eq!(rows.documents["a"].rel_path, "a.md");
        assert_eq!(rows.documents["b"].rel_path, "sub/b.md");
        assert!(rows.documents["m"].missing);
        assert!(!rows.documents["a"].missing);
        assert_eq!(rows.out_of_scope.other_roots, 2, "/other and /work/proj2");
        assert_eq!(rows.out_of_scope.worktrees, 1);
        let ev: Vec<&str> = rows.events.iter().map(|e| e.artifact_id.as_str()).collect();
        assert_eq!(ev, ["a", "b", "m"]);
        let aug: Vec<&str> = rows
            .augmentations
            .iter()
            .map(|a| a.artifact_id.as_str())
            .collect();
        assert_eq!(aug, ["a", "b", "m"]);
        let obs: Vec<&str> = rows.observations.iter().map(|o| o.text.as_str()).collect();
        assert_eq!(obs, ["obs a", "obs b", "obs m"]);
        assert_eq!(rows.observations[0].created_at_ms, 7);
    }

    #[test]
    fn an_event_carries_its_parent_resolves_mutates_and_source() {
        let old = old_catalog_with(|cat| {
            doc(cat, "a", "a.md");
            doc(cat, "b", "b.md");
            add_event(cat, "e1", "a", "intent", r#"{"goal":"g"}"#);
            add_event(cat, "e2", "a", "verdict", r#"{"outcome":"ok"}"#);
            sources::upsert(
                cat,
                &SourceRow {
                    id: "s1".into(),
                    uri: "https://example.test/1".into(),
                    kind: "jira".into(),
                    payload: Some(r#"{"key":"X-1"}"#.into()),
                    ingested_at: 3,
                },
            )
            .unwrap();
            add_edge(cat, "e2", "parent", Some("e1"), None, None);
            add_edge(cat, "e2", "resolves", Some("e1"), None, None);
            add_edge(cat, "e2", "mutates", None, Some("a"), None);
            add_edge(cat, "e2", "mutates", None, Some("b"), None);
            add_edge(cat, "e2", "triggered_by", None, None, Some("s1"));
        });
        let rows = load(&old, &roots()).unwrap();
        assert_eq!(rows.events.len(), 2);
        let e1 = rows.events.iter().find(|e| e.id == "e1").unwrap();
        assert_eq!(e1.parent, None);
        assert_eq!(e1.resolves, None);
        assert!(e1.mutates.is_empty());
        assert_eq!(e1.source, None);
        assert_eq!(e1.payload, serde_json::json!({"goal": "g"}));
        let e2 = rows.events.iter().find(|e| e.id == "e2").unwrap();
        assert_eq!(e2.kind, "verdict");
        assert_eq!(e2.parent.as_deref(), Some("e1"));
        assert_eq!(e2.resolves.as_deref(), Some("e1"));
        assert_eq!(e2.mutates, ["a", "b"]);
        assert_eq!(e2.created_at_ms, 1_000);
        let source = e2.source.as_ref().expect("the triggering source");
        assert_eq!(source.kind, "jira");
        assert_eq!(source.uri, "https://example.test/1");
        assert_eq!(source.payload, Some(serde_json::json!({"key": "X-1"})));
    }

    #[test]
    fn manual_links_exclude_cites_and_worktree_of() {
        let old = old_catalog_with(|cat| {
            doc(cat, "a", "a.md");
            doc(cat, "b", "b.md");
            add_doc_at(cat, "c", "/other/c.md");
            for (src, dst, rel) in [
                ("a", "b", "cites"),
                ("a", "b", "worktree_of"),
                ("a", "b", "depends_on"),
                ("b", "a", "supersedes"),
                ("c", "a", "depends_on"),
            ] {
                links::insert(
                    cat,
                    &LinkRow {
                        src_id: src.into(),
                        dst_id: dst.into(),
                        rel: rel.into(),
                        created_at: 1,
                    },
                )
                .unwrap();
            }
        });
        let rows = load(&old, &roots()).unwrap();
        let got: Vec<(&str, &str, &str)> = rows
            .links
            .iter()
            .map(|l| (l.src_id.as_str(), l.dst_id.as_str(), l.rel.as_str()))
            .collect();
        assert_eq!(
            got,
            [("a", "b", "depends_on"), ("b", "a", "supersedes")],
            "cites, worktree_of and a link from an out-of-scope source are not manual links"
        );
    }

    #[test]
    fn only_write_cites_are_counted() {
        let old = old_catalog_with(|cat| {
            let a = doc(cat, "a", "a.md");
            add_doc_at(cat, "c", "/other/c.md");
            let c = crate::librarian::catalog::artifact::ensure_slug(&cat.conn, "c").unwrap();
            for (slug, local, origin) in [
                (&a, "R-1", "write"),
                (&a, "R-2", "write"),
                (&a, "R-3", "scan"),
                (&c, "R-4", "write"),
            ] {
                let n = entry_cite::insert_with(
                    &cat.conn,
                    &EntryCiteRow {
                        src_slug: slug.clone(),
                        src_local: local.into(),
                        dst_ref: "x:R-9".into(),
                        rel: "cites".into(),
                        origin: origin.into(),
                        created_at: 1,
                    },
                )
                .unwrap();
                assert_eq!(n, 1);
            }
        });
        let rows = load(&old, &roots()).unwrap();
        assert_eq!(
            rows.write_cites, 2,
            "two write cites of an in-scope document; the scan cite and the out-of-scope one do not count"
        );
    }

    #[test]
    fn rows_of_an_out_of_scope_artifact_are_not_loaded() {
        let old = old_catalog_with(|cat| {
            add_doc_at(cat, "c", "/other/c.md");
            add_doc_at(cat, "w", "/work/proj/.worktrees/x/w.md");
            for art in ["c", "w"] {
                add_event(cat, &format!("ev-{art}"), art, "note", "{}");
                add_augmentation(cat, art, r#"{"k":1}"#, None);
            }
        });
        let rows = load(&old, &roots()).unwrap();
        assert!(rows.documents.is_empty());
        assert!(rows.events.is_empty());
        assert!(rows.augmentations.is_empty());
        assert_eq!(rows.out_of_scope.other_roots, 1);
        assert_eq!(rows.out_of_scope.worktrees, 1);
    }

    #[test]
    fn a_missing_table_is_named() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        fill_old_catalog(&conn, &[], &[], &[], &[]);
        assert!(OLD_CATALOG_SCHEMA.contains("artifact_link"));
        let old = OldCatalog::from_connection(conn).unwrap();
        let result = load(&old, &roots());
        let err = match result {
            Ok(_) => panic!("a catalog with no events table must be refused"),
            Err(e) => e,
        };
        assert!(
            format!("{err:#}").contains("events"),
            "the missing table is named: {err:#}"
        );
    }
}
