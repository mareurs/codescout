//! A full rebuild of the derived index from one commit.
//!
//! [`build_full`] reads a commit's tree through `git2` (never the working tree, and no path from
//! the file system), then rewrites every derived table in one transaction. Every input is a
//! committed blob: the markdown files, `.codescout/repo.toml`, `.codescout/librarian.toml` and
//! `.codescout/key-space.toml`. Two clones at one commit therefore give one answer.
//!
//! What the tables hold, where the plan left a choice:
//!
//! - `edge`: one row per citation that points at something. `ord` is the citation's position in
//!   its document's citation list, so a bare token dropped as not local (it names nothing) leaves
//!   a gap. `kind` is the resolution (`file`, `entry`, `local`, `unknown_key`, `key_collision`,
//!   `undefined_token`, `ambiguous_token`, `cross_repo`, `malformed`). `target_repo`,
//!   `target_key` and `target_token` are what the citation says as written; a malformed target's
//!   raw text is kept in `target_key`.
//! - `problem`: `invalid_key` (`key` is the key the path gave, when it gave one, and `rel_paths`
//!   the one path) and `duplicate_key` (`rel_paths` every holder, sorted).
//! - `skipped`: one row per `(rel_path, reason)`; a repeated pair is stored once.
//! - `checkout`: one row; `rules_source` is always `committed+builtin`.

use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use super::scan::{blob_text, scan_commit};
use crate::librarian::catalog::chunk::{build_chunks, ChunkRow};
use crate::librarian::classify::{
    classify, default_rules, load_rules, CompiledRule, PROJECT_RULES_REL,
};
use crate::librarian::frontmatter::{self, Frontmatter};
use crate::librarian::identity::grammar::{extract, Citation, Reference};
use crate::librarian::identity::index::{DocInput, IndexProblem, KeyIndex, Resolution};
use crate::librarian::identity::key::{key_of, KeyError};
use crate::librarian::identity::key_space::{KeySpace, KEY_SPACE_FILE_REL};
use crate::librarian::identity::repo_name::{self, RepoName, REPO_FILE_REL};
use crate::librarian::indexer::first_h1;
use crate::librarian::tools::link_scan::extract::entry_sections;

/// The chunk grain, fixed: 2048 characters (512 tokens), the old indexer's `CHUNK_CHARS`. It is
/// not read from `.codescout/project.toml`, which is not committed.
const CHUNK_CHARS: usize = 2048;

/// The repo name used when the commit holds no `.codescout/repo.toml`.
const UNCONFIRMED_NAME: &str = "repo";
const NAME_COMMITTED: &str = "committed";
const NAME_UNCONFIRMED: &str = "unconfirmed";

/// Where the rules came from: the commit's own file, then the compiled-in defaults.
const RULES_SOURCE: &str = "committed+builtin";

/// `parse_note` of a document whose frontmatter did not parse.
const PARSE_NOTE_FRONTMATTER_ERROR: &str = "frontmatter_error";

/// The tables a build rewrites. `parse_cache` is not here: it caches parse products and is no
/// part of the answer.
const DERIVED_TABLES: [&str; 7] = [
    "checkout", "document", "entry", "edge", "chunk", "problem", "skipped",
];

/// What one build reads: a commit of a repository, the rules, and the key space.
pub struct BuildInput<'a> {
    pub repo: &'a git2::Repository,
    pub commit: &'a git2::Commit<'a>,
    /// Recorded in the `checkout` row and read from nowhere: the build never opens a path.
    pub root: &'a Path,
    pub rules: &'a [CompiledRule],
    pub space: &'a KeySpace,
}

/// What one build stored: the rows of each table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildReport {
    pub documents: usize,
    pub entries: usize,
    pub edges: usize,
    pub chunks: usize,
    pub skipped: usize,
    pub problems: usize,
}

/// The classification rules of `commit`: its `.codescout/librarian.toml` blob (when present),
/// then the built-in defaults, so a committed rule wins as `classify` takes the first match.
///
/// No configuration path is taken and none is read: a machine-global rule has no way in. A rules
/// file that does not parse, or is a symlink in the commit, is an error naming the file.
pub fn rules_from_commit(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> anyhow::Result<Vec<CompiledRule>> {
    let mut rules = match blob_text(repo, commit, PROJECT_RULES_REL)? {
        Some(text) => load_rules(&text)
            .with_context(|| format!("reading {PROJECT_RULES_REL} in {}", commit.id()))?,
        None => Vec::new(),
    };
    rules.extend(default_rules()?);
    Ok(rules)
}

/// The key space of `commit`: its `.codescout/key-space.toml` blob, or the empty rule (nothing is
/// outside the key space) when the commit holds none. A file that does not parse, or is a
/// symlink in the commit, is an error naming the file.
pub fn key_space_from_commit(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> anyhow::Result<KeySpace> {
    match blob_text(repo, commit, KEY_SPACE_FILE_REL)? {
        Some(text) => KeySpace::from_text(&text)
            .with_context(|| format!("reading {KEY_SPACE_FILE_REL} in {}", commit.id())),
        None => Ok(KeySpace::empty()),
    }
}

/// Rebuild every derived table from `input.commit` in one transaction: an error leaves the
/// previous contents as they were.
///
/// Scan the commit; name the repo from the commit's own `repo.toml`; build the key index over
/// the whole tree; then parse each document on its own (frontmatter, metadata, entries,
/// citations, chunks) and resolve its citations against the index. Rows go in path order, and
/// nothing stored depends on a clock, a random value or where a clone lies.
pub fn build_full(conn: &mut Connection, input: &BuildInput) -> anyhow::Result<BuildReport> {
    let (name, name_source) = repo_name_of(input.repo, input.commit)?;

    let mut documents: Vec<Document> = Vec::new();
    let mut skipped: Vec<(String, &'static str)> = Vec::new();
    for file in scan_commit(input.repo, input.commit, input.space)? {
        match file.text {
            Ok(text) => documents.push(Document {
                rel_path: file.rel_path,
                sha256: file.sha256,
                text,
            }),
            Err(skip) => skipped.push((file.rel_path, skip.reason())),
        }
    }
    // Sorted by the pair: two reasons for one lossy path are two rows, in a fixed order.
    skipped.sort();

    // `build` keeps no text, so one document's copy lives only while it is read.
    let index = KeyIndex::build(
        name.clone(),
        documents.iter().map(|d| DocInput {
            rel_path: d.rel_path.clone(),
            text: d.text.clone(),
        }),
    );

    let tx = conn.transaction()?;
    for table in DERIVED_TABLES {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }
    tx.execute(
        "INSERT INTO checkout (root, repo_name, name_source, rules_source, commit_sha) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            input.root.to_string_lossy(),
            name.as_str(),
            name_source,
            RULES_SOURCE,
            input.commit.id().to_string()
        ],
    )?;

    let mut report = BuildReport::default();
    for doc in &documents {
        let parsed = parse_document(&doc.rel_path, &doc.text, input.rules);
        store_document(&tx, doc, &parsed, &index, &mut report)
            .with_context(|| format!("storing {}", doc.rel_path))?;
    }
    for problem in index.problems() {
        let (kind, key, rel_paths) = problem_row(problem);
        tx.prepare_cached("INSERT INTO problem (kind, \"key\", rel_paths) VALUES (?1, ?2, ?3)")?
            .execute(params![kind, key, json_array(&rel_paths)])?;
        report.problems += 1;
    }
    for (rel_path, reason) in &skipped {
        // `OR IGNORE`: the same pair twice is one row, and only a stored row is counted.
        report.skipped += tx
            .prepare_cached("INSERT OR IGNORE INTO skipped (rel_path, reason) VALUES (?1, ?2)")?
            .execute(params![rel_path, reason])?;
    }
    tx.commit()?;
    Ok(report)
}

/// One indexed markdown file of the commit.
struct Document {
    rel_path: String,
    sha256: String,
    text: String,
}

/// The repo name of `commit`: its `.codescout/repo.toml` blob (`committed`), or the literal
/// `repo` (`unconfirmed`) when it has none. Never a suggestion from the file system.
fn repo_name_of(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> anyhow::Result<(RepoName, &'static str)> {
    match blob_text(repo, commit, REPO_FILE_REL)? {
        Some(text) => {
            let name = repo_name::parse_text(&text)
                .with_context(|| format!("reading {REPO_FILE_REL} in {}", commit.id()))?;
            Ok((name, NAME_COMMITTED))
        }
        None => Ok((
            RepoName::parse(UNCONFIRMED_NAME).context("the fallback repo name")?,
            NAME_UNCONFIRMED,
        )),
    }
}

/// The columns of a `document` row that the text and the rules decide.
struct DocMetadata {
    kind: String,
    status: String,
    time_scope: Option<String>,
    confidence: f64,
    title: Option<String>,
    /// Sorted.
    owners: Vec<String>,
    /// Sorted.
    tags: Vec<String>,
    topic: Option<String>,
}

/// One defined entry: its heading line and the last line of its section, in file lines.
struct EntryRow {
    token: String,
    level: usize,
    heading_line: u32,
    end_line: u32,
}

/// Everything one document's text and path give, whatever else is in the tree. Resolution is not
/// here: an edge's resolution depends on every other document, so it is computed over the whole
/// tree after all documents are parsed.
struct ParsedDocument {
    metadata: DocMetadata,
    parse_note: Option<String>,
    entries: Vec<EntryRow>,
    citations: Vec<Citation>,
    /// `chunk_id` is filled by [`chunk_id`].
    chunks: Vec<ChunkRow>,
}

/// Parse one document: a pure function of its path, its text and the rules.
///
/// A frontmatter block that does not parse is not an error: the document is indexed with no
/// frontmatter, its whole text as the body, and `parse_note = "frontmatter_error"`. Entries and
/// citations are read from the whole text (both skip a valid frontmatter block themselves);
/// chunks are cut from the body and carry file lines.
fn parse_document(rel_path: &str, text: &str, rules: &[CompiledRule]) -> ParsedDocument {
    let (fm, body, parse_note) = match frontmatter::parse(text) {
        Ok((fm, body)) => (fm, body, None),
        Err(_) => (None, text, Some(PARSE_NOTE_FRONTMATTER_ERROR.to_owned())),
    };
    let metadata = derive_metadata(rel_path, fm.as_ref(), body, rules);
    let entries = entry_sections(text)
        .into_iter()
        .map(|s| EntryRow {
            token: s.id,
            level: s.level,
            heading_line: s.heading_line,
            end_line: s.end_line,
        })
        .collect();
    let citations = extract(text);
    // `body` is a suffix of `text` in both arms, which `body_line_offset` requires.
    let line_offset = frontmatter::body_line_offset(text, body);
    let chunks = build_chunks(rel_path, body, CHUNK_CHARS, line_offset)
        .into_iter()
        .map(|mut chunk| {
            chunk.chunk_id = chunk_id(rel_path, chunk.chunk_ix, &chunk.content_hash);
            chunk
        })
        .collect();
    ParsedDocument {
        metadata,
        parse_note,
        entries,
        citations,
        chunks,
    }
}

/// A chunk's id: `sha256(rel_path "\n" chunk_ix "\n" content_hash)` in lower-case hex. A pure
/// function of what the chunk is and where it sits, unlike the old `replace_chunks` uuid.
fn chunk_id(rel_path: &str, chunk_ix: usize, content_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rel_path.as_bytes());
    hasher.update(b"\n");
    hasher.update(chunk_ix.to_string().as_bytes());
    hasher.update(b"\n");
    hasher.update(content_hash.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// The metadata a document gets from its frontmatter and the matching rule.
///
/// Copied from the old indexer (`indexer.rs`, the block after `frontmatter::parse`: search
/// `"unknown"`, `"draft"`, `confidence`), with the same precedence:
///
/// - kind: frontmatter, else the first matching rule, else `unknown`;
/// - status: frontmatter, else the rule's, else `unknown` when the kind is unknown and `draft`
///   when it is not;
/// - time_scope: frontmatter, else the rule's;
/// - confidence: 1.0 when the frontmatter sets a kind, else 0.5;
/// - title: frontmatter, else the first H1 of the body;
/// - tags: the frontmatter's, then each tag of the rule not already there;
/// - owners and topic: the frontmatter's.
///
/// One difference: owners and tags are sorted, so a stored row does not depend on the order an
/// author happened to write them in.
fn derive_metadata(
    rel_path: &str,
    fm: Option<&Frontmatter>,
    body: &str,
    rules: &[CompiledRule],
) -> DocMetadata {
    let rule_match = classify(rules, rel_path);
    let kind = fm
        .and_then(|f| f.kind.clone())
        .or_else(|| rule_match.as_ref().map(|r| r.kind.clone()))
        .unwrap_or_else(|| "unknown".into());
    let status = fm
        .and_then(|f| f.status.clone())
        .or_else(|| rule_match.as_ref().and_then(|r| r.status.clone()))
        .unwrap_or_else(|| {
            if kind == "unknown" {
                "unknown".into()
            } else {
                "draft".into()
            }
        });
    let time_scope = fm
        .and_then(|f| f.time_scope.clone())
        .or_else(|| rule_match.as_ref().and_then(|r| r.time_scope.clone()));
    let confidence = if fm.and_then(|f| f.kind.as_ref()).is_some() {
        1.0
    } else {
        0.5
    };
    let title = fm.and_then(|f| f.title.clone()).or_else(|| first_h1(body));
    let mut owners = fm.map(|f| f.owners.clone()).unwrap_or_default();
    owners.sort();
    let mut tags = fm.map(|f| f.tags.clone()).unwrap_or_default();
    if let Some(rule) = rule_match.as_ref() {
        for tag in &rule.tags {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
    }
    tags.sort();
    let topic = fm.and_then(|f| f.topic.clone());
    DocMetadata {
        kind,
        status,
        time_scope,
        confidence,
        title,
        owners,
        tags,
        topic,
    }
}

/// Write one document's rows (document, entries, edges, chunks) and count them.
fn store_document(
    conn: &Connection,
    doc: &Document,
    parsed: &ParsedDocument,
    index: &KeyIndex,
    report: &mut BuildReport,
) -> rusqlite::Result<()> {
    let key = key_of(&doc.rel_path);
    let (key_text, key_error) = match &key {
        Ok(key) => (Some(key.as_str()), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let owner_key = key
        .as_ref()
        .ok()
        .and_then(|key| index.owner(key))
        .map(|owner| owner.as_str());
    let meta = &parsed.metadata;
    conn.prepare_cached(
        "INSERT INTO document (rel_path, \"key\", key_error, kind, status, title, owners, tags, \
         topic, time_scope, confidence, sha256, owner_key, parse_note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    )?
    .execute(params![
        doc.rel_path,
        key_text,
        key_error,
        meta.kind,
        meta.status,
        meta.title,
        json_array(&meta.owners),
        json_array(&meta.tags),
        meta.topic,
        meta.time_scope,
        meta.confidence,
        doc.sha256,
        owner_key,
        parsed.parse_note,
    ])?;
    report.documents += 1;

    for entry in &parsed.entries {
        conn.prepare_cached(
            "INSERT INTO entry (rel_path, token, level, heading_line, end_line) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?
        .execute(params![
            doc.rel_path,
            entry.token,
            entry.level as i64,
            entry.heading_line,
            entry.end_line
        ])?;
        report.entries += 1;
    }

    for (ord, citation) in parsed.citations.iter().enumerate() {
        let resolution = index.resolve(&doc.rel_path, &citation.reference);
        let Some(kind) = edge_kind(&resolution) else {
            continue;
        };
        let (target_repo, target_key, target_token) = edge_target(&citation.reference);
        conn.prepare_cached(
            "INSERT INTO edge (src_rel_path, ord, line, kind, target_repo, target_key, \
             target_token) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![
            doc.rel_path,
            ord as i64,
            citation.line,
            kind,
            target_repo,
            target_key,
            target_token
        ])?;
        report.edges += 1;
    }

    for chunk in &parsed.chunks {
        conn.prepare_cached(
            "INSERT INTO chunk (chunk_id, rel_path, chunk_ix, start_line, end_line, entry_token, \
             entry_part, entry_parts, content, content_hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?
        .execute(params![
            chunk.chunk_id,
            doc.rel_path,
            chunk.chunk_ix as i64,
            chunk.start_line as i64,
            chunk.end_line as i64,
            chunk.entry_token,
            chunk.entry_part.map(|n| n as i64),
            chunk.entry_parts.map(|n| n as i64),
            chunk.content,
            chunk.content_hash
        ])?;
        report.chunks += 1;
    }
    Ok(())
}

/// The `edge.kind` of a resolution; `None` for a bare token that names nothing in its own
/// document (not a citation, so no edge). Exhaustive, so a new resolution cannot go unstored.
fn edge_kind(resolution: &Resolution) -> Option<&'static str> {
    Some(match resolution {
        Resolution::File { .. } => "file",
        Resolution::Entry { .. } => "entry",
        Resolution::Local { .. } => "local",
        Resolution::NotLocal => return None,
        Resolution::UnknownKey { .. } => "unknown_key",
        Resolution::KeyCollision { .. } => "key_collision",
        Resolution::UndefinedToken { .. } => "undefined_token",
        Resolution::AmbiguousToken { .. } => "ambiguous_token",
        Resolution::CrossRepo { .. } => "cross_repo",
        Resolution::Malformed { .. } => "malformed",
    })
}

/// `(target_repo, target_key, target_token)` of a citation, as written. A malformed target's raw
/// text goes in `target_key`.
fn edge_target(reference: &Reference) -> (Option<&str>, Option<&str>, Option<&str>) {
    match reference {
        Reference::File { repo, key } => (
            repo.as_ref().map(RepoName::as_str),
            Some(key.as_str()),
            None,
        ),
        Reference::Entry { repo, key, token } => (
            repo.as_ref().map(RepoName::as_str),
            Some(key.as_str()),
            Some(token.as_str()),
        ),
        Reference::Bare { token } => (None, None, Some(token.as_str())),
        Reference::Malformed { raw } => (None, Some(raw.as_str()), None),
    }
}

/// `(kind, key, rel_paths)` of a `problem` row.
fn problem_row(problem: &IndexProblem) -> (&'static str, Option<String>, Vec<String>) {
    match problem {
        IndexProblem::InvalidKey(KeyError::NotMarkdown(rel_path)) => {
            ("invalid_key", None, vec![rel_path.clone()])
        }
        IndexProblem::InvalidKey(
            KeyError::NonCanonical { rel_path, key } | KeyError::HexShaped { rel_path, key },
        ) => ("invalid_key", Some(key.clone()), vec![rel_path.clone()]),
        IndexProblem::DuplicateKey { key, rel_paths } => (
            "duplicate_key",
            Some(key.as_str().to_owned()),
            rel_paths.clone(),
        ),
    }
}

/// A compact JSON array of strings: `["a","b"]`.
fn json_array(items: &[String]) -> String {
    serde_json::Value::from(items.to_vec()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::derived::scan::open_commit;
    use crate::librarian::derived::schema::{dump_all, open_in_memory};
    use crate::librarian::derived::testkit::{repo_with, repo_with_raw_tree, FILE, LINK};
    use rusqlite::types::Value;
    use sha2::{Digest, Sha256};

    const NULL: Value = Value::Null;

    fn t(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    fn sha_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// A one-commit repo holding `files` (path, text).
    fn fixture(files: &[(&str, &str)]) -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|&(path, text)| (path, FILE, text.as_bytes()))
            .collect();
        let repo = repo_with(dir.path(), &entries);
        (dir, repo)
    }

    /// The path every test records as the checkout root: it is only written to `checkout.root`.
    const ROOT: &str = "/fixture/root";

    /// What a caller does: the commit's own rules and key space, then the build. Every config
    /// file is read from the commit, so a failure of any of the three comes out of here.
    fn build_commit(conn: &mut Connection, repo: &git2::Repository) -> anyhow::Result<BuildReport> {
        let commit = open_commit(repo, "HEAD")?;
        let rules = rules_from_commit(repo, &commit)?;
        let space = key_space_from_commit(repo, &commit)?;
        build_full(
            conn,
            &BuildInput {
                repo,
                commit: &commit,
                root: Path::new(ROOT),
                rules: &rules,
                space: &space,
            },
        )
    }

    fn count(conn: &Connection, table: &str) -> usize {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap() as usize
    }

    /// A report is what the tables hold: no row is stored that the report does not count.
    fn assert_report_matches_tables(conn: &Connection, report: &BuildReport) {
        assert_eq!(report.documents, count(conn, "document"), "documents");
        assert_eq!(report.entries, count(conn, "entry"), "entries");
        assert_eq!(report.edges, count(conn, "edge"), "edges");
        assert_eq!(report.chunks, count(conn, "chunk"), "chunks");
        assert_eq!(report.skipped, count(conn, "skipped"), "skipped");
        assert_eq!(report.problems, count(conn, "problem"), "problems");
    }

    /// A fresh in-memory index of the repo's HEAD.
    fn built(repo: &git2::Repository) -> (Connection, BuildReport) {
        let mut conn = open_in_memory().unwrap();
        let report = build_commit(&mut conn, repo).unwrap();
        assert_report_matches_tables(&conn, &report);
        (conn, report)
    }

    /// A document's whole row, in column order.
    fn doc_row(conn: &Connection, rel_path: &str) -> Vec<Value> {
        conn.query_row(
            "SELECT * FROM document WHERE rel_path = ?1",
            [rel_path],
            |r| {
                (0..r.as_ref().column_count())
                    .map(|i| r.get::<_, Value>(i))
                    .collect()
            },
        )
        .unwrap()
    }

    fn doc_cell(conn: &Connection, rel_path: &str, column: &str) -> Value {
        conn.query_row(
            &format!("SELECT \"{column}\" FROM document WHERE rel_path = ?1"),
            [rel_path],
            |r| r.get::<_, Value>(0),
        )
        .unwrap()
    }

    type EdgeRow = (
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    );

    /// `(ord, line, kind, target_repo, target_key, target_token)` of a document's edges, by `ord`.
    fn edges_of(conn: &Connection, src: &str) -> Vec<EdgeRow> {
        conn.prepare(
            "SELECT ord, line, kind, target_repo, target_key, target_token \
             FROM edge WHERE src_rel_path = ?1 ORDER BY ord",
        )
        .unwrap()
        .query_map([src], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    fn edge(
        ord: i64,
        line: i64,
        kind: &str,
        repo: Option<&str>,
        key: Option<&str>,
        token: Option<&str>,
    ) -> EdgeRow {
        (
            ord,
            line,
            kind.to_owned(),
            repo.map(str::to_owned),
            key.map(str::to_owned),
            token.map(str::to_owned),
        )
    }

    fn skipped_rows(conn: &Connection) -> Vec<(String, String)> {
        conn.prepare("SELECT rel_path, reason FROM skipped ORDER BY rel_path, reason")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn problem_rows(conn: &Connection) -> Vec<(String, Option<String>, String)> {
        conn.prepare(
            "SELECT kind, \"key\", rel_paths FROM problem ORDER BY kind, \"key\", rel_paths",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    /// `(chunk_id, rel_path, chunk_ix, content, content_hash)` of every chunk, by path and index.
    fn chunk_rows(conn: &Connection) -> Vec<(String, String, i64, String, String)> {
        conn.prepare(
            "SELECT chunk_id, rel_path, chunk_ix, content, content_hash \
             FROM chunk ORDER BY rel_path, chunk_ix",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    #[test]
    fn documents_get_keys_owner_and_metadata() {
        let bug_log = "---\nkind: tracker\nstatus: archived\ntitle: Bug log\n\
                       owners: [zed, amy]\ntags: [b, a]\ntopic: bugs\ntime_scope: rolling\n---\n\
                       # Not the title\n\nbody\n";
        let (_dir, repo) = fixture(&[
            (".codescout/repo.toml", "name = \"home\"\n"),
            (
                ".codescout/librarian.toml",
                "[[rule]]\nglob = \"docs/trackers/**/*.md\"\nkind = \"tracker\"\n\
                 status = \"active\"\ntags = [\"rule-tag\", \"b\"]\n",
            ),
            ("docs/trackers/README.md", "# Trackers\n"),
            ("docs/trackers/bug-log.md", bug_log),
            (
                "docs/research/2026-11-01-note.md",
                "# Research note\n\ntext\n",
            ),
            ("notes.md", "no heading here\n"),
        ]);
        let (conn, report) = built(&repo);
        assert_eq!(report.documents, 4);
        assert_eq!((report.skipped, report.problems), (0, 0));

        // Frontmatter wins over the committed rule (status), tags are the sorted union, the
        // owner is the nearest folder index.
        assert_eq!(
            doc_row(&conn, "docs/trackers/bug-log.md"),
            vec![
                t("docs/trackers/bug-log.md"),
                t("bug-log"),
                NULL,
                t("tracker"),
                t("archived"),
                t("Bug log"),
                t(r#"["amy","zed"]"#),
                t(r#"["a","b","rule-tag"]"#),
                t("bugs"),
                t("rolling"),
                Value::Real(1.0),
                t(&sha_hex(bug_log.as_bytes())),
                t("trackers"),
                NULL,
            ]
        );
        // No frontmatter: the committed rule gives kind, status and its tags; the title is the
        // first H1; confidence is 0.5; a folder index has no owner above it here.
        assert_eq!(
            doc_row(&conn, "docs/trackers/README.md"),
            vec![
                t("docs/trackers/README.md"),
                t("trackers"),
                NULL,
                t("tracker"),
                t("active"),
                t("Trackers"),
                t("[]"),
                t(r#"["b","rule-tag"]"#),
                NULL,
                NULL,
                Value::Real(0.5),
                t(&sha_hex(b"# Trackers\n")),
                NULL,
                NULL,
            ]
        );
        // A built-in default rule: kind and time scope from the rule, status `draft` because
        // the kind is known and the rule has none.
        assert_eq!(
            doc_row(&conn, "docs/research/2026-11-01-note.md"),
            vec![
                t("docs/research/2026-11-01-note.md"),
                t("2026-11-01-note"),
                NULL,
                t("memory"),
                t("draft"),
                t("Research note"),
                t("[]"),
                t("[]"),
                NULL,
                t("dated_snapshot"),
                Value::Real(0.5),
                t(&sha_hex(b"# Research note\n\ntext\n")),
                NULL,
                NULL,
            ]
        );
        // No rule, no frontmatter, no heading: `unknown` kind and status, no title.
        assert_eq!(
            doc_row(&conn, "notes.md"),
            vec![
                t("notes.md"),
                t("notes"),
                NULL,
                t("unknown"),
                t("unknown"),
                NULL,
                t("[]"),
                t("[]"),
                NULL,
                NULL,
                Value::Real(0.5),
                t(&sha_hex(b"no heading here\n")),
                NULL,
                NULL,
            ]
        );

        // The one checkout row: what was built, from which name, rules and commit.
        let commit_id = repo.head().unwrap().peel_to_commit().unwrap().id();
        let checkout: Vec<Value> = conn
            .query_row("SELECT * FROM checkout", [], |r| {
                (0..5).map(|i| r.get::<_, Value>(i)).collect()
            })
            .unwrap();
        assert_eq!(
            checkout,
            vec![
                t(ROOT),
                t("home"),
                t("committed"),
                t("committed+builtin"),
                t(&commit_id.to_string()),
            ]
        );
    }

    #[test]
    fn a_broken_frontmatter_file_is_indexed_with_a_parse_note() {
        // Review Focus 1: all three in ONE commit, and the build does not abort.
        let dir = tempfile::tempdir().unwrap();
        let broken = "---\nkind: [unterminated\n---\n# Real Title\n\nbody text\n";
        let repo = repo_with(
            dir.path(),
            &[
                ("docs/broken.md", FILE, broken.as_bytes()),
                ("docs/bad-bytes.md", FILE, &[0x66, 0xff, 0xfe]),
                ("docs/link.md", LINK, b"fine.md"),
                ("docs/fine.md", FILE, b"# Fine\n"),
            ],
        );
        let (conn, report) = built(&repo);

        assert_eq!(report.documents, 2, "the broken file and the fine one");
        assert_eq!(report.skipped, 2);
        assert_eq!(
            skipped_rows(&conn),
            [
                ("docs/bad-bytes.md".to_owned(), "not_utf8".to_owned()),
                ("docs/link.md".to_owned(), "symlink".to_owned()),
            ]
        );
        for skipped in ["docs/bad-bytes.md", "docs/link.md"] {
            let rows: i64 = conn
                .query_row(
                    "SELECT count(*) FROM document WHERE rel_path = ?1",
                    [skipped],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(rows, 0, "{skipped} has no document row");
        }

        // No frontmatter was read: the kind in the broken block did not count, the title is the
        // first H1 of the whole text, and the note says why.
        assert_eq!(
            doc_cell(&conn, "docs/broken.md", "parse_note"),
            t("frontmatter_error")
        );
        assert_eq!(doc_cell(&conn, "docs/broken.md", "kind"), t("unknown"));
        assert_eq!(doc_cell(&conn, "docs/broken.md", "title"), t("Real Title"));
        assert_eq!(
            doc_cell(&conn, "docs/broken.md", "confidence"),
            Value::Real(0.5)
        );
        // The whole text is the body: the chunks start at line 1 and hold the broken block.
        let chunks: Vec<_> = chunk_rows(&conn)
            .into_iter()
            .filter(|c| c.1 == "docs/broken.md")
            .collect();
        assert!(!chunks.is_empty());
        assert!(
            chunks[0].3.contains("kind: [unterminated"),
            "{:?}",
            chunks[0].3
        );
        let first_line: i64 = conn
            .query_row(
                "SELECT start_line FROM chunk WHERE rel_path = 'docs/broken.md' AND chunk_ix = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(first_line, 1);
        // A healthy file carries no note.
        assert_eq!(doc_cell(&conn, "docs/fine.md", "parse_note"), NULL);
    }

    #[test]
    fn entries_are_the_headings_that_define_tokens() {
        let text = "---\nkind: tracker\n---\n# Log\n\n## F-1 — first\ntext one\n\n\
                    ### F-2 — nested\ntext two\n\n## Plain heading\ntext\n\n\
                    ```\n## X-9 — in a fence\n```\n\n## R-3 - third\nlast\n";
        let (_dir, repo) = fixture(&[("docs/log.md", text)]);
        let (conn, report) = built(&repo);
        let entries: Vec<(String, i64, i64, i64)> = conn
            .prepare(
                "SELECT token, level, heading_line, end_line FROM entry \
                 WHERE rel_path = 'docs/log.md' ORDER BY heading_line",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        // Lines are the file's own, frontmatter included; a section ends before the next
        // heading of its level or above; a heading in a fence defines nothing.
        assert_eq!(
            entries,
            [
                ("F-1".to_owned(), 2, 6, 11),
                ("F-2".to_owned(), 3, 9, 11),
                ("R-3".to_owned(), 2, 19, 20),
            ]
        );
        assert_eq!(report.entries, 3);
        // The chunk ranges are file lines too: the body starts below the 3-line frontmatter.
        let first_start: i64 = conn
            .query_row(
                "SELECT start_line FROM chunk WHERE chunk_ix = 0 AND rel_path = 'docs/log.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(first_start, 4);
    }

    #[test]
    fn citations_become_edges_with_their_resolution_kind() {
        let a = "# A\n\n## L-1 — local definition\n\nSee [[target]] and [[target:F-1]].\n\
                 Back to L-1 here.\nNobody defines Q-5.\nUnknown [[nokey]].\n\
                 Colliding [[same]].\nCross [[other-repo/target]].\n\
                 Malformed [[Bad Key!]].\nUndefined [[target:Z-9]].\n\
                 Ambiguous [[logs:F-1]].\n";
        let (_dir, repo) = fixture(&[
            (".codescout/repo.toml", "name = \"home\"\n"),
            ("docs/a.md", a),
            ("docs/target.md", "# Target\n\n## F-1 — the entry\n"),
            ("docs/dup/same.md", "# one\n"),
            ("docs/other/same.md", "# two\n"),
            ("logs/readme.md", "# Logs\n"),
            ("logs/x-log.md", "## F-1 — x\n"),
            ("logs/y-log.md", "## F-1 — y\n"),
        ]);
        let (conn, report) = built(&repo);
        assert_eq!(
            edges_of(&conn, "docs/a.md"),
            [
                edge(0, 5, "file", None, Some("target"), None),
                edge(1, 5, "entry", None, Some("target"), Some("F-1")),
                edge(2, 6, "local", None, None, Some("L-1")),
                // `ord` 3 is `Q-5`, a bare token no document defines: not stored, and the
                // later citations keep their own positions.
                edge(4, 8, "unknown_key", None, Some("nokey"), None),
                edge(5, 9, "key_collision", None, Some("same"), None),
                edge(
                    6,
                    10,
                    "cross_repo",
                    Some("other-repo"),
                    Some("target"),
                    None
                ),
                edge(7, 11, "malformed", None, Some("Bad Key!"), None),
                edge(8, 12, "undefined_token", None, Some("target"), Some("Z-9")),
                edge(9, 13, "ambiguous_token", None, Some("logs"), Some("F-1")),
            ]
        );
        assert_eq!(report.edges, 9);
        let not_local: i64 = conn
            .query_row(
                "SELECT count(*) FROM edge WHERE kind = 'not_local' OR target_token = 'Q-5'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(not_local, 0);
    }

    #[test]
    fn a_token_cited_twice_on_one_line_stores_two_edges() {
        let (_dir, repo) = fixture(&[
            (
                "a.md",
                "## T-1 — def\n\nsee T-1 and T-1 again [[b]] [[b]]\n",
            ),
            ("b.md", "# B\n"),
        ]);
        let (conn, _) = built(&repo);
        assert_eq!(
            edges_of(&conn, "a.md"),
            [
                edge(0, 3, "local", None, None, Some("T-1")),
                edge(1, 3, "local", None, None, Some("T-1")),
                edge(2, 3, "file", None, Some("b"), None),
                edge(3, 3, "file", None, Some("b"), None),
            ]
        );
    }

    #[test]
    fn chunk_ids_are_deterministic() {
        let long = format!(
            "# T\n\n{}\n\n## S2\n\n{}\n",
            "alpha ".repeat(300),
            "beta ".repeat(300)
        );
        let files = [
            ("docs/one.md", long.as_str()),
            ("docs/two.md", long.as_str()),
        ];
        let (_d1, repo1) = fixture(&files);
        let (_d2, repo2) = fixture(&files);
        let (conn1, _) = built(&repo1);
        let (conn2, _) = built(&repo2);
        let first = chunk_rows(&conn1);
        assert_eq!(
            first,
            chunk_rows(&conn2),
            "two builds, equal ids and content"
        );
        assert!(first.len() >= 4, "two documents, several chunks each");
        // The rule: sha256(rel_path "\n" chunk_ix "\n" content_hash), lower-case hex.
        for (id, path, ix, content, content_hash) in &first {
            assert_eq!(content_hash, &sha_hex(content.as_bytes()));
            let expected = sha_hex(format!("{path}\n{ix}\n{content_hash}").as_bytes());
            assert_eq!(id, &expected, "{path} chunk {ix}");
        }
        // Two documents with one text still have distinct ids: the path is part of the rule.
        let mut ids: Vec<&String> = first.iter().map(|c| &c.0).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), first.len());
    }

    #[test]
    fn an_invalid_key_is_stored_as_a_problem_and_the_document_row_keeps_key_error() {
        let (_dir, repo) = fixture(&[
            ("docs/Bad_Name.md", "# Bad\n\n## F-1 — still indexed\n"),
            ("docs/0123456789abcdef.md", "# Hex\n"),
            ("docs/fine.md", "# Fine\n"),
        ]);
        let (conn, report) = built(&repo);
        assert_eq!(
            report.documents, 3,
            "an invalid key does not drop the document"
        );
        assert_eq!(report.problems, 2);
        assert_eq!(doc_cell(&conn, "docs/Bad_Name.md", "key"), NULL);
        let Value::Text(message) = doc_cell(&conn, "docs/Bad_Name.md", "key_error") else {
            panic!("key_error is text");
        };
        assert!(
            message.contains("docs/Bad_Name.md") && message.contains("bad_name"),
            "{message}"
        );
        assert_eq!(doc_cell(&conn, "docs/0123456789abcdef.md", "key"), NULL);
        let Value::Text(message) = doc_cell(&conn, "docs/0123456789abcdef.md", "key_error") else {
            panic!("key_error is text");
        };
        assert!(message.contains("16 hex"), "{message}");
        assert_eq!(doc_cell(&conn, "docs/fine.md", "key_error"), NULL);
        // The rest of the document is still read: its entries and chunks exist.
        let entries: i64 = conn
            .query_row(
                "SELECT count(*) FROM entry WHERE rel_path = 'docs/Bad_Name.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(entries, 1);
        assert_eq!(
            problem_rows(&conn),
            [
                (
                    "invalid_key".to_owned(),
                    Some("0123456789abcdef".to_owned()),
                    r#"["docs/0123456789abcdef.md"]"#.to_owned()
                ),
                (
                    "invalid_key".to_owned(),
                    Some("bad_name".to_owned()),
                    r#"["docs/Bad_Name.md"]"#.to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_collision_excludes_both_from_the_key_maps_but_keeps_both_rows() {
        let (_dir, repo) = fixture(&[
            ("idx/readme.md", "# Idx\n"),
            (
                "idx/same.md",
                "## F-1 — in idx same\n\nF-1 is defined here.\n",
            ),
            ("other/same.md", "# Other\n"),
            ("cite.md", "[[same]] [[same:F-1]] [[idx:F-1]]\n"),
        ]);
        let (conn, report) = built(&repo);
        assert_eq!(
            report.documents, 4,
            "both colliding documents keep their rows"
        );
        for path in ["idx/same.md", "other/same.md"] {
            assert_eq!(doc_cell(&conn, path, "key"), t("same"), "{path}");
            assert_eq!(doc_cell(&conn, path, "key_error"), NULL, "{path}");
            // Left out of the owner map: a colliding document has no owner.
            assert_eq!(doc_cell(&conn, path, "owner_key"), NULL, "{path}");
        }
        assert_eq!(
            problem_rows(&conn),
            [(
                "duplicate_key".to_owned(),
                Some("same".to_owned()),
                r#"["idx/same.md","other/same.md"]"#.to_owned()
            )]
        );
        assert_eq!(
            edges_of(&conn, "cite.md"),
            [
                edge(0, 1, "key_collision", None, Some("same"), None),
                edge(1, 1, "key_collision", None, Some("same"), Some("F-1")),
                // `idx/same.md` defines F-1 under the owner `idx`, but it is excluded from the
                // owner's candidates, so nobody under `idx` defines it.
                edge(2, 1, "undefined_token", None, Some("idx"), Some("F-1")),
            ]
        );
        // Out of the key maps, yet in the path map: its own bare token still resolves locally.
        assert_eq!(
            edges_of(&conn, "idx/same.md"),
            [edge(0, 3, "local", None, None, Some("F-1"))]
        );
    }

    #[test]
    fn rebuilding_twice_dumps_identically() {
        let (_dir, repo) = fixture(&[
            (".codescout/repo.toml", "name = \"home\"\n"),
            (
                "docs/a.md",
                "# A\n\n## F-1 — def\n\nsee F-1 and [[b]] and [[nokey]]\n",
            ),
            ("docs/b.md", "# B\n"),
            ("docs/Bad_Name.md", "# Bad\n"),
        ]);
        let mut conn = open_in_memory().unwrap();
        build_commit(&mut conn, &repo).unwrap();
        let first = dump_all(&conn);
        // The dump is not vacuous: every derived table but `skipped` has rows.
        for table in ["checkout", "document", "entry", "edge", "chunk", "problem"] {
            assert!(!first[table].is_empty(), "{table} has rows");
        }
        // A second build into the same connection.
        build_commit(&mut conn, &repo).unwrap();
        assert_eq!(first, dump_all(&conn));
        // And a build into a fresh connection from a second clone of the content.
        let (_dir2, repo2) = fixture(&[
            (".codescout/repo.toml", "name = \"home\"\n"),
            (
                "docs/a.md",
                "# A\n\n## F-1 — def\n\nsee F-1 and [[b]] and [[nokey]]\n",
            ),
            ("docs/b.md", "# B\n"),
            ("docs/Bad_Name.md", "# Bad\n"),
        ]);
        let (fresh, _) = built(&repo2);
        assert_eq!(first, dump_all(&fresh));
    }

    #[test]
    fn a_second_build_leaves_exactly_one_checkout_row() {
        let (_dir, repo) = fixture(&[("a.md", "# A\n")]);
        let mut conn = open_in_memory().unwrap();
        build_commit(&mut conn, &repo).unwrap();
        build_commit(&mut conn, &repo).unwrap();
        assert_eq!(count(&conn, "checkout"), 1);
    }

    #[test]
    fn a_rebuild_deletes_every_row_of_the_previous_build() {
        // The first commit fills every derived table (one file is not UTF-8, so `skipped` too);
        // the second is one small document.
        let dir = tempfile::tempdir().unwrap();
        let rich = repo_with(
            dir.path(),
            &[
                (
                    "docs/a.md",
                    FILE,
                    "# A\n\n## F-1 — def\n\nF-1 [[b]] [[nokey]]\n".as_bytes(),
                ),
                ("docs/b.md", FILE, b"# B\n"),
                ("docs/Bad_Name.md", FILE, b"# Bad\n"),
                ("docs/bad-bytes.md", FILE, &[0xff]),
            ],
        );
        let (_d2, small) = fixture(&[("only.md", "# Only\n")]);

        let mut conn = open_in_memory().unwrap();
        build_commit(&mut conn, &rich).unwrap();
        for table in ["document", "entry", "edge", "chunk", "problem", "skipped"] {
            assert!(
                count(&conn, table) > 0,
                "{table} is filled by the first build"
            );
        }
        build_commit(&mut conn, &small).unwrap();
        let (fresh, _) = built(&small);
        assert_eq!(dump_all(&conn), dump_all(&fresh));
        assert_eq!(count(&conn, "document"), 1);
    }

    #[test]
    fn rules_come_from_the_commit_blob_then_the_builtin_defaults() {
        let files = [
            ("docs/research/a.md", "# A\n"),
            ("docs/scratch/b.md", "# B\n"),
            ("docs/adrs/c.md", "# C\n"),
        ];
        // A committed rule changes a kind, and shadows the default for the same path.
        let mut with_rules = vec![(
            ".codescout/librarian.toml",
            "[[rule]]\nglob = \"**/docs/research/*.md\"\nkind = \"custom\"\nstatus = \"open\"\n\n\
             [[rule]]\nglob = \"docs/scratch/*.md\"\nkind = \"scratch\"\n",
        )];
        with_rules.extend(files);
        let (_d1, committed) = fixture(&with_rules);
        let (conn, _) = built(&committed);
        assert_eq!(doc_cell(&conn, "docs/research/a.md", "kind"), t("custom"));
        assert_eq!(doc_cell(&conn, "docs/research/a.md", "status"), t("open"));
        assert_eq!(doc_cell(&conn, "docs/scratch/b.md", "kind"), t("scratch"));
        // The built-in defaults follow the committed rules.
        assert_eq!(doc_cell(&conn, "docs/adrs/c.md", "kind"), t("adr"));
        let commit = open_commit(&committed, "HEAD").unwrap();
        let defaults = crate::librarian::classify::default_rules().unwrap().len();
        assert_eq!(
            rules_from_commit(&committed, &commit).unwrap().len(),
            2 + defaults
        );

        // With no `.codescout/librarian.toml` in the commit, the defaults alone apply.
        let (_d2, plain) = fixture(&files);
        let (conn, _) = built(&plain);
        assert_eq!(doc_cell(&conn, "docs/research/a.md", "kind"), t("memory"));
        assert_eq!(doc_cell(&conn, "docs/scratch/b.md", "kind"), t("unknown"));
        assert_eq!(doc_cell(&conn, "docs/adrs/c.md", "kind"), t("adr"));
        let commit = open_commit(&plain, "HEAD").unwrap();
        assert_eq!(rules_from_commit(&plain, &commit).unwrap().len(), defaults);

        // A committed rules file that does not parse is an error naming the file.
        let (_d3, broken) = fixture(&[(".codescout/librarian.toml", "this is = not [ toml\n")]);
        let commit = open_commit(&broken, "HEAD").unwrap();
        let err = format!("{:#}", rules_from_commit(&broken, &commit).unwrap_err());
        assert!(err.contains(".codescout/librarian.toml"), "{err}");
    }

    #[test]
    fn the_repo_name_and_key_space_come_from_the_commit_not_the_working_tree() {
        let (dir, repo) = fixture(&[
            (".codescout/repo.toml", "name = \"committed-name\"\n"),
            (".codescout/key-space.toml", "exclude = [\"contrib\"]\n"),
            ("docs/a.md", "see [[committed-name/b]]\n"),
            ("docs/b.md", "# B\n"),
            ("contrib/c.md", "# C\n"),
        ]);
        let (conn, report) = built(&repo);
        let before = dump_all(&conn);
        assert_eq!(report.documents, 2);
        assert_eq!(
            skipped_rows(&conn),
            [("contrib/c.md".to_owned(), "outside_key_space".to_owned())]
        );
        // The committed name is this repo's own, so a citation spelled with it is local.
        assert_eq!(
            edges_of(&conn, "docs/a.md"),
            [edge(0, 1, "file", Some("committed-name"), Some("b"), None)]
        );

        // Change both files in the working tree, and add the files the commit does not hold.
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/repo.toml"),
            "name = \"working-tree-name\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".codescout/key-space.toml"),
            "exclude = [\"docs\"]\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".codescout/librarian.toml"),
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"worktree\"\n",
        )
        .unwrap();
        let (again, _) = built(&repo);
        assert_eq!(before, dump_all(&again), "the working tree changed nothing");
        let name: String = again
            .query_row("SELECT repo_name FROM checkout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "committed-name");
    }

    #[test]
    fn a_commit_without_repo_toml_uses_the_literal_repo_and_says_unconfirmed() {
        let (_dir, repo) = fixture(&[
            ("a.md", "own [[repo/b]] other [[elsewhere/b]]\n"),
            ("b.md", "# B\n"),
        ]);
        let (conn, _) = built(&repo);
        let (name, source): (String, String) = conn
            .query_row("SELECT repo_name, name_source FROM checkout", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((name.as_str(), source.as_str()), ("repo", "unconfirmed"));
        assert_eq!(
            edges_of(&conn, "a.md"),
            [
                edge(0, 1, "file", Some("repo"), Some("b"), None),
                edge(1, 1, "cross_repo", Some("elsewhere"), Some("b"), None),
            ]
        );
    }

    #[test]
    fn a_malformed_repo_toml_is_an_error_naming_the_file() {
        let (_d0, good) = fixture(&[("ok.md", "# Ok\n")]);
        let mut conn = open_in_memory().unwrap();
        build_commit(&mut conn, &good).unwrap();
        let before = dump_all(&conn);
        let (_dir, repo) = fixture(&[
            (".codescout/repo.toml", "name = \"Bad Name\"\n"),
            ("a.md", "# A\n"),
        ]);
        let err = format!("{:#}", build_commit(&mut conn, &repo).unwrap_err());
        assert!(err.contains(".codescout/repo.toml"), "{err}");
        // The refused build touched nothing: the previous build's rows are all still there.
        assert_eq!(before, dump_all(&conn));
    }

    #[test]
    fn a_symlinked_config_file_makes_the_build_fail_loudly_naming_the_file() {
        // One config file at a time is a link in the commit; each is read through its own
        // route (repo name in `build_full`, rules, key space), and each route must refuse it.
        let mut failures: Vec<String> = Vec::new();
        for config in [
            ".codescout/repo.toml",
            ".codescout/librarian.toml",
            ".codescout/key-space.toml",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let repo = repo_with(
                dir.path(),
                &[
                    (config, LINK, b"../elsewhere.toml"),
                    ("a.md", FILE, b"# A\n"),
                ],
            );
            let mut conn = open_in_memory().unwrap();
            match build_commit(&mut conn, &repo) {
                Ok(_) => failures.push(format!("{config}: the build succeeded")),
                Err(err) => {
                    let message = format!("{err:#}");
                    if !(message.contains(config) && message.contains("symlink")) {
                        failures.push(format!("{config}: {message}"));
                    }
                }
            }
            if count(&conn, "document") != 0 {
                failures.push(format!("{config}: rows were written"));
            }
        }
        // Every config is tried before the verdict, so one run names every route that failed.
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn skipped_rows_tolerate_a_repeated_path_and_keep_two_reasons() {
        // U+FFFD is valid UTF-8, so the lossy name of a non-UTF-8 entry can equal a real path.
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with_raw_tree(dir.path(), |repo, root| {
            let text = repo.blob(b"# valid\n").unwrap();
            let bad_bytes = repo.blob(&[0xff]).unwrap();
            // Two entries whose lossy name is the same `a\u{fffd}.md`, both `not_utf8`.
            root.insert(&b"a\xff.md"[..], text, FILE as i32).unwrap();
            root.insert(&b"a\xfe.md"[..], text, FILE as i32).unwrap();
            // A symlink and a non-UTF-8 file that both read `b\u{fffd}.md`: two reasons.
            root.insert(&b"b\xff.md"[..], text, LINK as i32).unwrap();
            root.insert("b\u{fffd}.md", bad_bytes, FILE as i32).unwrap();
            root.insert("ok.md", text, FILE as i32).unwrap();
        });
        let (conn, report) = built(&repo);
        assert_eq!(report.documents, 1);
        assert_eq!(
            skipped_rows(&conn),
            [
                ("a\u{fffd}.md".to_owned(), "not_utf8".to_owned()),
                ("b\u{fffd}.md".to_owned(), "not_utf8".to_owned()),
                ("b\u{fffd}.md".to_owned(), "symlink".to_owned()),
            ]
        );
        assert_eq!(report.skipped, 3, "the repeated pair is one row");
    }
}
