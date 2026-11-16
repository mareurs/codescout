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
//!   raw text is kept in `target_key`, so a reader must filter on `kind != 'malformed'` before it
//!   treats `target_key` as a key. `dst_key` is the key of the document a `file` or `entry`
//!   citation resolved to (NULL for every other kind).
//! - `problem`: `invalid_key` (`key` is the key the path gave, when it gave one, and `rel_paths`
//!   the one path) and `duplicate_key` (`rel_paths` every holder, sorted).
//! - `skipped`: one row per `(rel_path, reason)`; a repeated pair is stored once.
//! - `checkout`: one row; `rules_source` is always `committed+builtin`, because
//!   `BuildInput::from_commit` is the only way outside `derived` to build an input.

use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

use super::incremental::{self, CacheKey, Parsed};
use super::scan::{blob_text, scan_commit, ScannedFile};
use crate::librarian::catalog::chunk::{build_chunks, ChunkRow};
use crate::librarian::catalog::event_text::{parse_event_sections_spanned, EventSpan, EventText};
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

/// Names the version of the parse logic: [`parse_document`] and everything it calls
/// (frontmatter, entry sections, the citation grammar, chunking, metadata derivation) and the
/// shape the parse cache stores. It is part of the parse cache's key, so a cached row from another
/// version is never read.
///
/// Bump it whenever the parse of some text changes. The test
/// `the_parse_of_a_pinned_corpus_is_tied_to_parser_version` fails when it should have been.
pub const PARSER_VERSION: &str = "2";

/// The chunk grain, fixed: 2048 characters (512 tokens), the old indexer's `CHUNK_CHARS`. It is
/// not read from `.codescout/project.toml`, which is not committed.
const CHUNK_CHARS: usize = 2048;

/// The repo name used when the commit holds no `.codescout/repo.toml`.
const UNCONFIRMED_NAME: &str = "repo";
const NAME_COMMITTED: &str = "committed";
const NAME_UNCONFIRMED: &str = "unconfirmed";

/// Where the rules came from: the commit's own file, then the compiled-in defaults.
const RULES_SOURCE: &str = "committed+builtin";

/// `parse_note` of a document whose frontmatter did not parse. `pub(super)`: the shadow diff
/// names the documents whose title the first-H1 rule changes by this note.
pub(super) const PARSE_NOTE_FRONTMATTER_ERROR: &str = "frontmatter_error";

/// The tables a build rewrites. `parse_cache` is not here: it caches parse products and is no
/// part of the answer. The history tables (`history_meta`, `doc_history`) are not here either:
/// the history walk owns them.
const DERIVED_TABLES: [&str; 11] = [
    "checkout",
    "document",
    "entry",
    "edge",
    "chunk",
    "problem",
    "skipped",
    "event",
    "entry_doc",
    "augmentation",
    "worktree_layer",
];

/// What one build reads: a commit of a repository, the rules, and the key space.
///
/// [`BuildInput::from_commit`] is the only way to make one. The rules and the key space are read
/// from the commit itself, so no caller can pair a commit with rules or a key space of another
/// source. A struct literal from outside the `derived` module does not compile:
///
/// ```compile_fail,E0451
/// use codescout::librarian::derived::build::BuildInput;
///
/// fn make() {
///     let _input: BuildInput<'_> = BuildInput { ..todo!() };
/// }
/// ```
///
/// The block names no field, so a renamed, added or retyped field cannot change the reason it
/// fails: struct-update syntax is refused whenever any field is private.
///
/// The constructor compiles from outside the module:
///
/// ```no_run
/// use codescout::librarian::derived::build::BuildInput;
///
/// fn make(
///     repo: &git2::Repository,
///     commit: &git2::Commit,
///     root: &std::path::Path,
/// ) -> anyhow::Result<()> {
///     let input = BuildInput::from_commit(repo, commit, root)?;
///     let _rules = input.rules();
///     Ok(())
/// }
/// ```
pub struct BuildInput<'a> {
    pub(super) repo: &'a git2::Repository,
    pub(super) commit: &'a git2::Commit<'a>,
    /// Recorded in the `checkout` row and read from nowhere: the build never opens a path.
    pub(super) root: &'a Path,
    pub(super) rules: Vec<CompiledRule>,
    pub(super) space: KeySpace,
}

impl<'a> BuildInput<'a> {
    /// The input of `commit`: its rules ([`rules_from_commit`]) and its key space
    /// ([`key_space_from_commit`]). An error from either read passes through unchanged.
    pub fn from_commit(
        repo: &'a git2::Repository,
        commit: &'a git2::Commit<'a>,
        root: &'a Path,
    ) -> anyhow::Result<Self> {
        let rules = rules_from_commit(repo, commit)?;
        let space = key_space_from_commit(repo, commit)?;
        Ok(Self {
            repo,
            commit,
            root,
            rules,
            space,
        })
    }

    /// The classification rules the build uses: the commit's own, then the built-in defaults.
    pub fn rules(&self) -> &[CompiledRule] {
        &self.rules
    }
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
    /// What git says changed between the commit an update started from and the one it indexed;
    /// `None` for a full build, and for an update whose diff could not be taken. Nothing stored
    /// depends on it.
    pub changes: Option<ChangeCounts>,
}

/// The `.md` files a git diff between two commits reports, by kind of change. A rename counts
/// once, as a rename, and not as an addition and a deletion, as long as the commit deletes at most
/// `RENAME_LIMIT` files of any kind (a constant in `crate::git`): past that, git compares each
/// added file with only the first `RENAME_LIMIT + 1` deleted ones, and a rename whose old file is
/// not among them counts as an addition and a deletion. The counts depend on the two trees alone:
/// the diff sets its rename options itself, so a repo's or the machine's git config
/// (`diff.renameLimit`, `diff.renames`) changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChangeCounts {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
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
/// the whole tree; parse each document on its own (frontmatter, metadata, entries, citations,
/// chunks), all of them before the write transaction opens; then resolve each document's
/// citations against the index and write the rows. Rows go in path order, and nothing stored
/// depends on a clock, a random value or where a clone lies.
///
/// Every document is parsed afresh and the parse cache is never read, so this is the oracle the
/// incremental [`update`](super::incremental::update) is held to. It does write the cache, so an
/// update right after it reuses what it parsed.
pub fn build_full(conn: &mut Connection, input: &BuildInput) -> anyhow::Result<BuildReport> {
    rebuild(conn, input, &Parser::PRODUCTION, false, None)
}

/// How a build parses a document, as a value so that a test can count parse calls or pretend the
/// parse logic changed. Production uses [`Parser::PRODUCTION`] and nothing else.
pub(super) struct Parser<'a> {
    /// Stored in every cache row and part of the cache key.
    pub(super) version: &'a str,
    pub(super) parse: &'a dyn Fn(&str, &str, &[CompiledRule]) -> ParsedDocument,
}

impl Parser<'static> {
    pub(super) const PRODUCTION: Parser<'static> = Parser {
        version: PARSER_VERSION,
        parse: &parse_document,
    };
}

/// The one write path of `build_full` and of the incremental update: they differ only in whether
/// a document's parse products may come from the cache (`reuse_cache`) and in `changes`, the
/// counts the update reports.
///
/// The key index is built over the whole tree and every document's citations are resolved
/// against it on every call, so a document that did not change still gets the edges the new
/// tree gives it.
pub(super) fn rebuild(
    conn: &mut Connection,
    input: &BuildInput,
    parser: &Parser,
    reuse_cache: bool,
    changes: Option<ChangeCounts>,
) -> anyhow::Result<BuildReport> {
    rebuild_from(conn, input, parser, reuse_cache, changes, &|| {
        scan_commit(input.repo, input.commit, &input.space)
    })
}

/// [`rebuild`] with the scan of the commit passed in, so that a test can hand the build the same
/// files in another order and see that nothing stored depends on it. `scan` runs once, after the
/// repo name is read, as the scan always did; production passes [`scan_commit`] and nothing else.
pub(super) fn rebuild_from(
    conn: &mut Connection,
    input: &BuildInput,
    parser: &Parser,
    reuse_cache: bool,
    changes: Option<ChangeCounts>,
    scan: &dyn Fn() -> anyhow::Result<Vec<ScannedFile>>,
) -> anyhow::Result<BuildReport> {
    let (name, name_source) = repo_name_of(input.repo, input.commit)?;

    let mut documents: Vec<Document> = Vec::new();
    let mut skipped: Vec<(String, &'static str)> = Vec::new();
    for file in scan()? {
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

    // Every parse, and every cache read, happens here and not inside the write transaction: the
    // write lock is held for the deletes and inserts alone.
    let rules_hash = incremental::rules_hash(&input.rules);
    let mut parsed: Vec<Parsed> = Vec::with_capacity(documents.len());
    for doc in &documents {
        let key = CacheKey {
            sha256: doc.sha256.clone(),
            rel_path: doc.rel_path.clone(),
            rules_hash: rules_hash.clone(),
            parser_version: parser.version.to_owned(),
        };
        let cached = if reuse_cache {
            incremental::load(conn, &key)
                .with_context(|| format!("reading the parse cache for {}", doc.rel_path))?
        } else {
            None
        };
        parsed.push(match cached {
            Some(products) => Parsed::cached(key, products),
            None => {
                let products = (parser.parse)(&doc.rel_path, &doc.text, &input.rules);
                Parsed::fresh(key, products)
                    .with_context(|| format!("encoding the parse cache row of {}", doc.rel_path))?
            }
        });
    }

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

    let mut report = BuildReport {
        changes,
        ..BuildReport::default()
    };
    for (doc, parsed) in documents.iter().zip(&parsed) {
        store_document(&tx, doc, &parsed.products, &index, &mut report)
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
    incremental::sync_cache(&tx, &parsed).context("writing the parse cache")?;
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
pub(crate) fn repo_name_of(
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
#[derive(Debug, Clone, PartialEq)]
pub struct DocMetadata {
    pub kind: String,
    pub status: String,
    pub time_scope: Option<String>,
    pub confidence: f64,
    pub title: Option<String>,
    /// Sorted.
    pub owners: Vec<String>,
    /// Sorted.
    pub tags: Vec<String>,
    pub topic: Option<String>,
}

/// One defined entry: its heading line and the last line of its section, in file lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRow {
    pub token: String,
    pub level: usize,
    pub heading_line: u32,
    pub end_line: u32,
}

/// One event section of a document: where it sits (file lines) and the event the text holds.
/// `source_json` and `payload_json` are `serde_json::to_string` of the parsed values (key order as
/// written); `mutates` is a JSON array of the forms on the section's `**Mutates:**` line. Events
/// are keyed by `(rel_path, heading_line)`, so two sections with one id are two rows (E2-R7).
#[derive(Debug, Clone, PartialEq)]
pub struct EventRow {
    pub heading_line: u32,
    pub end_line: u32,
    pub event_id: String,
    pub kind: String,
    pub created_at_ms: i64,
    pub author: Option<String>,
    pub anchor_commit: Option<String>,
    pub head_commit: Option<String>,
    pub parent: Option<String>,
    pub resolves: Option<String>,
    pub source_json: Option<String>,
    pub mutates: Vec<String>,
    pub payload_json: String,
}

/// Everything one document's text and path give, whatever else is in the tree. Resolution is not
/// here: an edge's resolution depends on every other document, so it is computed over the whole
/// tree after all documents are parsed. This is what the parse cache holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDocument {
    pub metadata: DocMetadata,
    pub parse_note: Option<String>,
    pub entries: Vec<EntryRow>,
    pub citations: Vec<Citation>,
    /// `chunk_id` is filled by [`chunk_id`]; `artifact_id` is the document's `rel_path`.
    pub chunks: Vec<ChunkRow>,
    /// The event sections, in document order.
    pub events: Vec<EventRow>,
}

/// Parse one document: a pure function of its path, its text and the rules.
///
/// A frontmatter block that does not parse is not an error: the document is indexed with no
/// frontmatter, its whole text as the body, and `parse_note = "frontmatter_error"`. Entries and
/// citations are read from the whole text (both skip a valid frontmatter block themselves);
/// chunks are cut from the body and carry file lines. Event sections are read from the whole text
/// and carry file lines too.
pub fn parse_document(rel_path: &str, text: &str, rules: &[CompiledRule]) -> ParsedDocument {
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
    let events = parse_event_sections_spanned(text)
        .into_iter()
        .map(|(event, span)| event_row(event, span))
        .collect();
    ParsedDocument {
        metadata,
        parse_note,
        entries,
        citations,
        chunks,
        events,
    }
}

/// The row of one event section. The source and the payload are stored as `serde_json::to_string`
/// of the parsed values (key order as written, `preserve_order`), the `mutates` forms as a list
/// that [`store_document`] writes as a JSON array (E2-R7).
fn event_row(event: EventText, span: EventSpan) -> EventRow {
    EventRow {
        heading_line: span.heading_line,
        end_line: span.end_line,
        event_id: event.id,
        kind: event.kind,
        created_at_ms: event.created_at_ms,
        author: event.author,
        anchor_commit: event.anchor_commit,
        head_commit: event.head_commit,
        parent: event.parent,
        resolves: event.resolves,
        source_json: event.source.map(|source| {
            serde_json::to_string(&source).expect("two strings and a JSON value always serialize")
        }),
        mutates: event.mutates,
        payload_json: serde_json::to_string(&event.payload)
            .expect("a JSON value always serializes"),
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
/// Two differences from the old indexer:
///
/// - owners and tags are sorted, so a stored row does not depend on the order an author happened
///   to write them in;
/// - a file whose frontmatter fails to parse gets its title from the first H1 of its whole text
///   (it is indexed with no frontmatter and its whole text as the body). The old indexer parsed
///   an empty body for such a file, so its title was NULL.
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

/// Write one document's rows (document, entries, events, edges, chunks) and count them; the
/// report has no count for events.
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

    for event in &parsed.events {
        conn.prepare_cached(
            "INSERT INTO event (rel_path, heading_line, end_line, event_id, kind, created_at_ms, \
             author, anchor_commit, head_commit, parent, resolves, source_json, mutates, \
             payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?
        .execute(params![
            doc.rel_path,
            event.heading_line,
            event.end_line,
            event.event_id,
            event.kind,
            event.created_at_ms,
            event.author,
            event.anchor_commit,
            event.head_commit,
            event.parent,
            event.resolves,
            event.source_json,
            json_array(&event.mutates),
            event.payload_json,
        ])?;
    }

    for (ord, citation) in parsed.citations.iter().enumerate() {
        let resolution = index.resolve(&doc.rel_path, &citation.reference);
        let Some(kind) = edge_kind(&resolution) else {
            continue;
        };
        let (target_repo, target_key, target_token) = edge_target(&citation.reference);
        conn.prepare_cached(
            "INSERT INTO edge (src_rel_path, ord, line, kind, target_repo, target_key, \
             target_token, dst_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?
        .execute(params![
            doc.rel_path,
            ord as i64,
            citation.line,
            kind,
            target_repo,
            target_key,
            target_token,
            edge_dst_key(&resolution)
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
/// The key of the document a citation resolved to: `file` and `entry` have one. For an entry
/// cited through a folder index it is the owned document that defines the token. Exhaustive, so
/// a new resolution has to say whether it names a document.
fn edge_dst_key(resolution: &Resolution) -> Option<&str> {
    match resolution {
        Resolution::File { dst } | Resolution::Entry { dst, .. } => Some(dst.as_str()),
        Resolution::Local { .. }
        | Resolution::NotLocal
        | Resolution::UnknownKey { .. }
        | Resolution::KeyCollision { .. }
        | Resolution::UndefinedToken { .. }
        | Resolution::AmbiguousToken { .. }
        | Resolution::CrossRepo { .. }
        | Resolution::Malformed { .. } => None,
    }
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
    use crate::librarian::derived::schema::{dump_all, open as open_index, open_in_memory};
    use crate::librarian::derived::testkit::{
        repo_with, repo_with_commit, repo_with_raw_tree, FILE, LINK,
    };
    use rusqlite::types::Value;
    use sha2::{Digest, Sha256};

    const NULL: Value = Value::Null;

    fn t(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    fn sha_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    // mutation: widen one field of `BuildInput` to `pub(crate)` or `pub`. The compile_fail doctest
    // is outside the crate, so it cannot see a widening that stays inside the crate; this text
    // check can. Every field must be private or `pub(super)`: the one way to make the input is
    // `BuildInput::from_commit`, which takes the rules and the key space from the commit.
    #[test]
    fn every_field_of_build_input_stays_inside_derived() {
        let source = include_str!("build.rs");
        let start = source
            .find("pub struct BuildInput<'a> {")
            .expect("the struct BuildInput is in build.rs");
        let body = &source[start..];
        let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];
        let fields: Vec<&str> = body
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("#["))
            .collect();
        let names: Vec<&str> = fields
            .iter()
            .filter_map(|l| l.split(':').next()?.split_whitespace().last())
            .collect();
        // Non-vacuity: the scan found the fields, not an empty or a wrong span.
        assert!(
            names.contains(&"repo") && names.contains(&"space"),
            "the scan of BuildInput found these fields: {names:?}"
        );
        for field in fields {
            assert!(
                field.starts_with("pub(super) ") || !field.starts_with("pub"),
                "a field of BuildInput is visible outside `derived`: {field}"
            );
        }
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
        build_full(
            conn,
            &BuildInput::from_commit(repo, &commit, Path::new(ROOT))?,
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
    /// An event as a committed section, rendered by the writer.
    fn event_text(
        id: &str,
        kind: &str,
        mutates: &[&str],
        payload: serde_json::Value,
    ) -> crate::librarian::catalog::event_text::EventText {
        crate::librarian::catalog::event_text::EventText {
            id: id.to_owned(),
            kind: kind.to_owned(),
            created_at_ms: 1_794_637_800_000,
            author: Some("alice".to_owned()),
            anchor_commit: None,
            head_commit: None,
            parent: None,
            resolves: None,
            source: None,
            mutates: mutates.iter().map(|m| (*m).to_owned()).collect(),
            payload,
        }
    }

    fn section(e: &crate::librarian::catalog::event_text::EventText) -> String {
        crate::librarian::catalog::event_text::render_event_section(e)
    }

    /// `(event_id, kind, created_at_ms, mutates, payload_json, heading_line, end_line)` of every
    /// `event` row, by heading line.
    type EventCells = (String, String, i64, String, String, i64, i64);

    fn event_cells(conn: &Connection) -> Vec<EventCells> {
        conn.prepare(
            "SELECT event_id, kind, created_at_ms, mutates, payload_json, heading_line, end_line \
             FROM event ORDER BY heading_line",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    // mutation: store `mutates.join(" ")` instead of the JSON array.
    #[test]
    fn event_sections_become_event_rows() {
        let note = event_text(
            "n-1",
            "note",
            &["[[b]]"],
            serde_json::json!({"text": "first\nsecond"}),
        );
        let verdict = event_text(
            "v-1",
            "verdict",
            &[],
            serde_json::json!({"outcome": "go", "score": 1.5}),
        );
        let doc = format!(
            "---\ntitle: Notes\n---\n# Notes\n\n## Events\n\n{}{}\n## Later\n\ntext\n",
            section(&note),
            section(&verdict)
        );
        let headings: Vec<i64> = doc
            .lines()
            .enumerate()
            .filter(|(_, l)| l.starts_with("### "))
            .map(|(i, _)| i as i64 + 1)
            .collect();
        let later = doc.lines().position(|l| l == "## Later").unwrap() as i64 + 1;
        let (_dir, repo) = fixture(&[("docs/notes.md", &doc)]);

        let (conn, _) = built(&repo);

        assert_eq!(
            event_cells(&conn),
            vec![
                (
                    "n-1".to_owned(),
                    "note".to_owned(),
                    1_794_637_800_000,
                    r#"["[[b]]"]"#.to_owned(),
                    r#"{"text":"first\nsecond"}"#.to_owned(),
                    headings[0],
                    headings[1] - 1
                ),
                (
                    "v-1".to_owned(),
                    "verdict".to_owned(),
                    1_794_637_800_000,
                    "[]".to_owned(),
                    r#"{"outcome":"go","score":1.5}"#.to_owned(),
                    headings[1],
                    later - 1
                ),
            ]
        );
    }

    /// The columns an event row may leave empty, and the source, round-trip through the build.
    #[test]
    fn an_events_optional_fields_and_source_are_stored() {
        let mut full = event_text("n-2", "note", &[], serde_json::json!({"text": "x"}));
        full.anchor_commit = Some("a1".to_owned());
        full.head_commit = Some("h2".to_owned());
        full.parent = Some("p3".to_owned());
        full.resolves = Some("r4".to_owned());
        full.source = Some(crate::librarian::catalog::event_text::EventSource {
            kind: "url".to_owned(),
            uri: "https://example.invalid/x".to_owned(),
            payload: Some(serde_json::json!({"k": 1})),
        });
        let mut bare = event_text("n-3", "note", &[], serde_json::json!({"text": "y"}));
        bare.author = None;
        let doc = format!("# D\n\n## Events\n\n{}{}", section(&full), section(&bare));
        let (_dir, repo) = fixture(&[("docs/d.md", &doc)]);

        let (conn, _) = built(&repo);

        type Cells = Vec<Option<String>>;
        let cells: Vec<(String, Cells)> = conn
            .prepare(
                "SELECT event_id, author, anchor_commit, head_commit, parent, resolves, \
                 source_json FROM event ORDER BY heading_line",
            )
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    (1..7)
                        .map(|i| r.get::<_, Option<String>>(i))
                        .collect::<Result<Cells, _>>()?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let s = |v: &str| Some(v.to_owned());
        assert_eq!(
            cells,
            vec![
                (
                    "n-2".to_owned(),
                    vec![
                        s("alice"),
                        s("a1"),
                        s("h2"),
                        s("p3"),
                        s("r4"),
                        s(r#"{"kind":"url","uri":"https://example.invalid/x","payload":{"k":1}}"#)
                    ]
                ),
                ("n-3".to_owned(), vec![None, None, None, None, None, None]),
            ]
        );
    }

    // mutation: key `event` on `(rel_path, event_id)`.
    #[test]
    fn one_id_twice_is_two_event_rows() {
        let one = event_text("same", "note", &[], serde_json::json!({"text": "one"}));
        let two = event_text("same", "note", &[], serde_json::json!({"text": "two"}));
        let doc = format!("# D\n\n## Events\n\n{}{}", section(&one), section(&two));
        let (_dir, repo) = fixture(&[("docs/d.md", &doc)]);

        let (conn, _) = built(&repo);

        let cells = event_cells(&conn);
        assert_eq!(cells.len(), 2, "{cells:?}");
        assert_eq!(cells[0].0, cells[1].0, "one id");
        assert_ne!(cells[0].5, cells[1].5, "two heading lines");
        assert_ne!(cells[0].4, cells[1].4, "two payloads");
    }

    // mutation: drop `event` from `DERIVED_TABLES`.
    #[test]
    fn a_rebuild_replaces_the_event_rows() {
        let e = event_text("n-1", "note", &[], serde_json::json!({"text": "gone soon"}));
        let with = format!("# D\n\n## Events\n\n{}", section(&e));
        let (_dir, repo) = fixture(&[("docs/d.md", &with)]);
        let (mut conn, _) = built(&repo);
        assert_eq!(count(&conn, "event"), 1);
        let parent = open_commit(&repo, "HEAD").unwrap();
        let second = crate::librarian::derived::testkit::commit_files(
            &repo,
            &[("docs/d.md", b"# D\n".as_slice())],
            &[&parent],
            "drop the event",
        );
        let commit = open_commit(&repo, &second.to_string()).unwrap();

        build_full(
            &mut conn,
            &BuildInput::from_commit(&repo, &commit, Path::new(ROOT)).unwrap(),
        )
        .unwrap();

        assert_eq!(count(&conn, "event"), 0, "a stale event row survived");
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

    /// R15: the build is one transaction. A failure after it began leaves the previous contents.
    #[test]
    fn a_failed_build_leaves_the_previous_contents_unchanged() {
        let (_d1, first) = fixture(&[("docs/a.md", "# A\n\n[[b]]\n"), ("docs/b.md", "# B\n")]);
        let (_d2, second) = fixture(&[("docs/c.md", "# C\n\ntext\n"), ("docs/d.md", "# D\n")]);
        let (mut conn, _) = built(&first);
        let before = dump_all(&conn);

        // A chunk insert fails part-way through the second build's write transaction.
        conn.execute_batch(
            "CREATE TEMP TRIGGER boom BEFORE INSERT ON chunk \
             BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();
        let err = build_commit(&mut conn, &second).unwrap_err();
        assert!(format!("{err:#}").contains("boom"), "{err:#}");
        assert_eq!(
            dump_all(&conn),
            before,
            "nothing of the failed build is left"
        );

        // And the same connection builds again once the fault is gone.
        conn.execute_batch("DROP TRIGGER boom").unwrap();
        build_commit(&mut conn, &second).unwrap();
        assert_ne!(dump_all(&conn), before);
    }

    /// R14: `dst_key` is the key of the document a `file` or `entry` citation resolved to.
    #[test]
    fn an_edge_stores_the_key_of_the_document_it_resolved_to() {
        let a = "# A\n\n## L-1 — local\n\nSee [[t]], [[t:T-1]], [[readme:F-1]], [[nokey]], \
                 [[same]], [[other-repo/t]], [[Bad Key!]], [[t:Z-9]], [[readme:F-2]] and L-1.\n";
        let (_dir, repo) = fixture(&[
            (".codescout/repo.toml", "name = \"home\"\n"),
            ("docs/a.md", a),
            ("docs/t.md", "# T\n\n## T-1 — entry\n"),
            ("docs/readme/README.md", "# Readme\n"),
            ("docs/readme/f1-log.md", "## F-1 — defined here\n"),
            ("docs/readme/x-log.md", "## F-2 — one\n"),
            ("docs/readme/y-log.md", "## F-2 — two\n"),
            ("docs/dup/same.md", "# one\n"),
            ("docs/other/same.md", "# two\n"),
        ]);
        let (conn, _) = built(&repo);
        let rows: Vec<(String, Option<String>)> = conn
            .prepare("SELECT kind, dst_key FROM edge WHERE src_rel_path = 'docs/a.md' ORDER BY ord")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let row = |kind: &str, dst: Option<&str>| (kind.to_owned(), dst.map(str::to_owned));
        assert_eq!(
            rows,
            [
                row("file", Some("t")),
                row("entry", Some("t")),
                // Cited through the folder index `readme`; defined in the document it owns.
                row("entry", Some("f1-log")),
                row("unknown_key", None),
                row("key_collision", None),
                row("cross_repo", None),
                row("malformed", None),
                row("undefined_token", None),
                row("ambiguous_token", None),
                row("local", None),
            ]
        );
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

    // ---- Determinism: nothing a clone's path, name, identity or time can leak into a row ----

    /// Run `git` in `dir` with the machine's own configuration out of the way, so that what a test
    /// sees does not depend on the developer's git setup (nor on a hook that set `GIT_DIR`).
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// `git clone` of `src` into `dst` over a `file://` URL: a second checkout of one repository,
    /// with its own path, its own working tree and its own local config.
    fn clone_repo(src: &Path, dst: &Path) {
        let url = format!("file://{}", src.display());
        git(
            dst.parent().unwrap(),
            &["clone", "--quiet", &url, dst.to_str().unwrap()],
        );
    }

    /// `(path, mode, bytes)` of a tree entry, owned.
    type Entry = (String, u32, Vec<u8>);

    fn borrowed(entries: &[Entry]) -> Vec<(&str, u32, &[u8])> {
        entries
            .iter()
            .map(|(path, mode, bytes)| (path.as_str(), *mode, bytes.as_slice()))
            .collect()
    }

    /// A corpus that gives every table rows and every ordering something to decide: a committed
    /// repo name, key space and rule; a folder index that owns a document; citations of most
    /// kinds; a broken frontmatter; two invalid keys; five keys that two documents each hold;
    /// a document of several chunks; and three skipped files (outside the key space, not UTF-8,
    /// a symlink).
    fn corpus() -> Vec<Entry> {
        let text =
            |path: &str, body: &str| -> Entry { (path.to_owned(), FILE, body.as_bytes().to_vec()) };
        let bug_log = "---\nkind: tracker\nstatus: archived\ntitle: Bug log\n\
                       owners: [zed, amy]\ntags: [b, a]\ntopic: bugs\ntime_scope: rolling\n---\n\
                       # Not the title\n\n## B-1 — a bug\n\nbody\n";
        let mut entries =
            vec![
            text(".codescout/repo.toml", "name = \"home\"\n"),
            text(".codescout/key-space.toml", "exclude = [\"contrib\"]\n"),
            text(
                ".codescout/librarian.toml",
                "[[rule]]\nglob = \"docs/trackers/**/*.md\"\nkind = \"tracker\"\n\
                 status = \"active\"\ntags = [\"rule-tag\", \"b\"]\n",
            ),
            text(
                "docs/trackers/README.md",
                "# Trackers\n\n## T-1 — a tracker token\n",
            ),
            text("docs/trackers/bug-log.md", bug_log),
            text(
                "docs/a.md",
                "# A\n\n## F-1 — def\n\nsee F-1, [[b]], [[home/b]], [[elsewhere/x]], [[nokey]], \
                 [[trackers:T-1]], [[bug-log:B-1]], [[dup1]], [[dup1:D-1]], [[Bad_Name]]\n",
            ),
            text("docs/b.md", "# B\n"),
            text(
                "docs/notes.md",
                &format!(
                    "# Notes\n\n## Events\n\n{}{}",
                    section(&event_text(
                        "n-1",
                        "note",
                        &[],
                        serde_json::json!({"text": "first\nsecond"})
                    )),
                    section(&event_text(
                        "v-1",
                        "verdict",
                        &[],
                        serde_json::json!({"outcome": "go", "score": 1.5})
                    ))
                ),
            ),
            text("docs/Bad_Name.md", "# Bad\n\n## F-9 — still indexed\n"),
            text("docs/0123456789abcdef.md", "# Hex\n"),
            text("docs/broken.md", "---\nkind: [unterminated\n---\n# Broken\n"),
            text("contrib/c.md", "# C\n"),
        ];
        for i in 1..=5 {
            for dir in ["x", "y"] {
                entries.push(text(
                    &format!("{dir}/dup{i}.md"),
                    &format!("# Dup {i} in {dir}\n\n## D-{i} — defined in {dir}\n"),
                ));
            }
        }
        let long: String = (0..60)
            .map(|i| {
                format!(
                    "Paragraph {i} of a long document.\n\n{}\n\n",
                    "filler ".repeat(12)
                )
            })
            .collect();
        entries.push(text("docs/long.md", &format!("# Long\n\n{long}")));
        entries.push(("docs/bad-bytes.md".to_owned(), FILE, vec![0x66, 0xff, 0xfe]));
        entries.push(("docs/link.md".to_owned(), LINK, b"a.md".to_vec()));
        entries
    }

    /// The corpus really exercises what the determinism tests claim to compare: every table has
    /// rows, the edges cover several kinds, and a document has more than one chunk.
    fn assert_corpus_is_not_vacuous(dump: &std::collections::BTreeMap<String, Vec<Vec<String>>>) {
        for table in [
            "checkout", "document", "entry", "edge", "chunk", "problem", "skipped", "event",
        ] {
            assert!(!dump[table].is_empty(), "the corpus gives {table} no rows");
        }
        let kinds: std::collections::BTreeSet<&str> =
            dump["edge"].iter().map(|row| row[3].as_str()).collect();
        assert!(kinds.len() >= 4, "edge kinds: {kinds:?}");
        assert!(
            dump["chunk"].iter().any(|row| row[2] != "I:0"),
            "no document has a second chunk"
        );
    }

    /// Build `sha` of the repository at `clone` (opened by its own path) into a new index file.
    fn build_clone(clone: &Path, sha: &str, index_file: &Path) -> (Connection, BuildReport) {
        let repo = git2::Repository::open(clone).unwrap();
        let commit = open_commit(&repo, sha).unwrap();
        let mut conn = open_index(index_file).unwrap();
        let report = build_full(
            &mut conn,
            &BuildInput::from_commit(&repo, &commit, clone).unwrap(),
        )
        .unwrap();
        (conn, report)
    }

    /// The commit a repository's `HEAD` names.
    fn head_sha(repo_path: &Path) -> String {
        let repo = git2::Repository::open(repo_path).unwrap();
        let id = repo.head().unwrap().peel_to_commit().unwrap().id();
        id.to_string()
    }

    /// `checkout.root`, the one column that carries a path.
    fn recorded_root(conn: &Connection) -> String {
        conn.query_row("SELECT root FROM checkout", [], |r| r.get(0))
            .unwrap()
    }

    /// `problem` rows in the order they were stored, which `dump_all` (it sorts every table)
    /// cannot see: `problem` is the one derived table with a rowid.
    fn problems_in_insert_order(conn: &Connection) -> Vec<(String, Option<String>, String)> {
        conn.prepare("SELECT kind, \"key\", rel_paths FROM problem ORDER BY rowid")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn two_clones_at_one_commit_dump_identically() {
        let parent = tempfile::tempdir().unwrap();
        let origin = parent.path().join("origin-5d2e");
        let alpha = parent.path().join("clone-alpha-7f3a");
        let beta = parent.path().join("clone-beta-91cc");
        let entries = corpus();
        repo_with(&origin, &borrowed(&entries));
        clone_repo(&origin, &alpha);
        clone_repo(&origin, &beta);
        // Two different people at two different machines: each clone has its own identity.
        for (clone, name, email) in [
            (&alpha, "Alice Alpha", "alice@alpha.invalid"),
            (&beta, "Bob Beta", "bob@beta.invalid"),
        ] {
            git(clone, &["config", "user.name", name]);
            git(clone, &["config", "user.email", email]);
            assert_eq!(git(clone, &["config", "user.email"]).trim(), email);
        }

        let sha = head_sha(&origin);
        assert_eq!(head_sha(&alpha), sha, "the clone is at the commit");
        assert_eq!(head_sha(&beta), sha, "the clone is at the commit");
        let (conn_a, report_a) = build_clone(&alpha, &sha, &parent.path().join("index-a.sqlite"));
        let (conn_b, report_b) = build_clone(&beta, &sha, &parent.path().join("index-b.sqlite"));

        let dump_a = dump_all(&conn_a);
        assert_corpus_is_not_vacuous(&dump_a);
        assert_eq!(report_a, report_b);
        assert_eq!(dump_a, dump_all(&conn_b));
        // The two builds did differ in what the dump leaves out: where each clone lies.
        assert_ne!(recorded_root(&conn_a), recorded_root(&conn_b));
    }

    #[test]
    fn the_same_tree_committed_on_different_days_by_different_identities_dumps_identically() {
        let entries = corpus();
        let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        // Author and committer differ within a commit, and the two commits differ in the day,
        // the time zone, the people and the message.
        let sig = |name: &str, email: &str, seconds: i64, offset_minutes: i32| {
            git2::Signature::new(name, email, &git2::Time::new(seconds, offset_minutes)).unwrap()
        };
        let repo_a = repo_with_commit(
            dir_a.path(),
            &borrowed(&entries),
            &sig("Alice Early", "alice@early.invalid", 1_262_304_000, 120),
            &sig("Carol Committer", "carol@early.invalid", 1_262_390_400, 120),
            "the first message",
        );
        let repo_b = repo_with_commit(
            dir_b.path(),
            &borrowed(&entries),
            &sig("Bob Late", "bob@late.invalid", 1_893_456_000, -300),
            &sig("Dave Deployer", "dave@late.invalid", 1_893_542_400, -300),
            "a different message entirely",
        );
        let tree_of_head =
            |repo: &git2::Repository| repo.head().unwrap().peel_to_commit().unwrap().tree_id();
        assert_eq!(tree_of_head(&repo_a), tree_of_head(&repo_b), "one tree");
        assert_ne!(
            head_sha(dir_a.path()),
            head_sha(dir_b.path()),
            "two commits: the test cannot pass by an accident of identical history"
        );

        let (conn_a, report_a) = built(&repo_a);
        let (conn_b, report_b) = built(&repo_b);
        let dump_a = dump_all(&conn_a);
        assert_corpus_is_not_vacuous(&dump_a);
        assert_eq!(report_a, report_b);
        assert_eq!(dump_a, dump_all(&conn_b));
        // The commit id is excluded from the dump by design, and it is what differs.
        let sha_of = |conn: &Connection| -> String {
            conn.query_row("SELECT commit_sha FROM checkout", [], |r| r.get(0))
                .unwrap()
        };
        assert_ne!(sha_of(&conn_a), sha_of(&conn_b));
    }

    #[test]
    fn the_checkout_row_is_the_only_place_a_path_appears() {
        let parent = tempfile::tempdir().unwrap();
        let origin = parent.path().join("origin-5d2e");
        let clone = parent.path().join("clone-alpha-7f3a");
        let entries = corpus();
        repo_with(&origin, &borrowed(&entries));
        clone_repo(&origin, &clone);
        let sha = head_sha(&clone);
        let (conn, _) = build_clone(&clone, &sha, &parent.path().join("index.sqlite"));

        // What a clone's whereabouts could be spelled as: its own name and path, the directory
        // that holds it (the one thing two sibling clones share), and the repo it was cloned from.
        let parent_path = parent.path().to_string_lossy().into_owned();
        let parent_name = parent
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let clone_path = clone.to_string_lossy().into_owned();
        let needles = [
            "clone-alpha-7f3a",
            clone_path.as_str(),
            parent_path.as_str(),
            parent_name.as_str(),
            "origin-5d2e",
        ];

        // Every string `dump_all` returns: the table names, then every rendered cell.
        let dump = dump_all(&conn);
        assert_corpus_is_not_vacuous(&dump);
        let mut strings: Vec<&str> = Vec::new();
        for (table, rows) in &dump {
            strings.push(table);
            strings.extend(rows.iter().flatten().map(String::as_str));
        }
        let in_dump = strings.len();
        assert!(in_dump > 500, "the search covers the whole dump: {in_dump}");
        for needle in needles {
            for s in &strings {
                assert!(!s.contains(needle), "{needle:?} is in the dump value {s:?}");
            }
        }
        // The dump leaves `checkout.root` and `checkout.commit_sha` out: the one row it shows has
        // the other three columns.
        assert_eq!(dump["checkout"].len(), 1);
        assert_eq!(dump["checkout"][0].len(), 3);

        // What the dump leaves out is searched too: `parse_cache` and `checkout.commit_sha`.
        let leftovers: Vec<String> = {
            let mut out = Vec::new();
            let mut cache = conn.prepare("SELECT * FROM parse_cache").unwrap();
            let columns = cache.column_count();
            let rows = cache
                .query_map([], |row| {
                    (0..columns)
                        .map(|i| row.get::<_, Value>(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap();
            for row in rows {
                out.extend(row.unwrap().into_iter().map(|v| format!("{v:?}")));
            }
            let sha_column: String = conn
                .query_row("SELECT commit_sha FROM checkout", [], |r| r.get(0))
                .unwrap();
            out.push(sha_column);
            out
        };
        assert!(leftovers.len() > 100, "{}", leftovers.len());
        for needle in needles {
            for s in &leftovers {
                assert!(!s.contains(needle), "{needle:?} is in a stored value");
            }
        }

        // The column that does carry the path still does, and carries all of it: were the
        // exclusion in `dump_all` to stop hiding it, the search above would find it.
        let root = recorded_root(&conn);
        assert_eq!(root, clone_path);
        assert!(root.contains("clone-alpha-7f3a"), "{root}");
    }

    #[test]
    fn build_order_does_not_change_the_dump() {
        let dir = tempfile::tempdir().unwrap();
        let entries = corpus();
        let repo = repo_with(dir.path(), &borrowed(&entries));
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, Path::new(ROOT)).unwrap();
        let natural_order: Vec<String> = scan_commit(&repo, &commit, &input.space)
            .unwrap()
            .into_iter()
            .map(|f| f.rel_path)
            .collect();

        // Fixed permutations, not a random shuffle: a failure repeats.
        type Permute = fn(&mut Vec<ScannedFile>);
        let permutations: [(&str, Permute); 4] = [
            ("as scanned", |_| {}),
            ("reversed", |files| files.reverse()),
            ("rotated by five", |files| files.rotate_left(5)),
            ("by blob hash", |files| {
                files.sort_by(|a, b| a.sha256.cmp(&b.sha256))
            }),
        ];
        let mut builds = Vec::new();
        for (name, permute) in permutations {
            let order = std::cell::RefCell::new(Vec::new());
            let mut conn = open_in_memory().unwrap();
            let report = rebuild_from(&mut conn, &input, &Parser::PRODUCTION, false, None, &|| {
                let mut files = scan_commit(&repo, &commit, &input.space)?;
                permute(&mut files);
                *order.borrow_mut() = files.iter().map(|f| f.rel_path.clone()).collect();
                Ok(files)
            })
            .unwrap();
            let order = order.into_inner();
            if name == "as scanned" {
                assert_eq!(order, natural_order);
            } else {
                assert_ne!(order, natural_order, "{name} is a different order");
            }
            builds.push((name, conn, report));
        }

        let (_, first, first_report) = &builds[0];
        let expected_dump = dump_all(first);
        assert_corpus_is_not_vacuous(&expected_dump);
        // Stored in a fixed order whatever the scan order was: the invalid keys by path, then the
        // keys held twice by key. Five of the latter, so that an order taken from a hash map would
        // not be this one by luck.
        let mut expected_problems = vec![
            (
                "invalid_key".to_owned(),
                Some("0123456789abcdef".to_owned()),
                r#"["docs/0123456789abcdef.md"]"#.to_owned(),
            ),
            (
                "invalid_key".to_owned(),
                Some("bad_name".to_owned()),
                r#"["docs/Bad_Name.md"]"#.to_owned(),
            ),
        ];
        for i in 1..=5 {
            expected_problems.push((
                "duplicate_key".to_owned(),
                Some(format!("dup{i}")),
                format!(r#"["x/dup{i}.md","y/dup{i}.md"]"#),
            ));
        }
        for (name, conn, report) in &builds {
            assert_eq!(report, first_report, "{name}: report");
            assert_eq!(dump_all(conn), expected_dump, "{name}: dump");
            assert_eq!(
                problems_in_insert_order(conn),
                expected_problems,
                "{name}: problem rows in the order stored"
            );
        }
    }

    #[test]
    fn a_working_tree_edit_that_is_not_committed_changes_nothing() {
        let parent = tempfile::tempdir().unwrap();
        let origin = parent.path().join("origin-5d2e");
        let clone = parent.path().join("clone-alpha-7f3a");
        let entries = corpus();
        repo_with(&origin, &borrowed(&entries));
        clone_repo(&origin, &clone);
        let sha = head_sha(&clone);
        let (before_conn, before_report) =
            build_clone(&clone, &sha, &parent.path().join("before.sqlite"));
        let before = dump_all(&before_conn);
        assert_corpus_is_not_vacuous(&before);

        // Edit what the commit holds, add what it does not, and change the three config files.
        // Two of the changes are also staged: the index is not the commit either.
        let edits = [
            (
                "docs/a.md",
                "# A, edited\n\nnothing of the commit's text is left [[b]]\n",
            ),
            (
                "docs/untracked-note.md",
                "# Not in any commit\n\n## U-1 — untracked\n",
            ),
            (".codescout/repo.toml", "name = \"working-tree-name\"\n"),
            (".codescout/key-space.toml", "exclude = [\"docs\"]\n"),
            (
                ".codescout/librarian.toml",
                "[[rule]]\nglob = \"**/*.md\"\nkind = \"worktree\"\n",
            ),
            (
                "docs/trackers/bug-log.md",
                "# Staged, not committed\n\n## S-1 — staged\n",
            ),
            ("docs/staged-new.md", "# New and staged\n"),
        ];
        for (rel, body) in edits {
            std::fs::write(clone.join(rel), body).unwrap();
        }
        std::fs::remove_file(clone.join("docs/b.md")).unwrap();
        git(
            &clone,
            &["add", "docs/trackers/bug-log.md", "docs/staged-new.md"],
        );
        // The edits are on disk, and git sees them as uncommitted changes of the one checkout.
        for (rel, body) in edits {
            assert_eq!(
                std::fs::read_to_string(clone.join(rel)).unwrap(),
                body,
                "{rel}"
            );
        }
        assert!(!clone.join("docs/b.md").exists());
        let status = git(&clone, &["status", "--porcelain"]);
        for line in [
            " M docs/a.md",
            "?? docs/untracked-note.md",
            " M .codescout/repo.toml",
            " M .codescout/key-space.toml",
            " M .codescout/librarian.toml",
            " D docs/b.md",
            "M  docs/trackers/bug-log.md",
            "A  docs/staged-new.md",
        ] {
            assert!(
                status.lines().any(|l| l == line),
                "{line:?} not in:\n{status}"
            );
        }
        assert_eq!(head_sha(&clone), sha, "nothing was committed");

        let (after_conn, after_report) =
            build_clone(&clone, &sha, &parent.path().join("after.sqlite"));
        assert_eq!(before_report, after_report);
        assert_eq!(
            before,
            dump_all(&after_conn),
            "the working tree changed nothing"
        );
        let name: String = after_conn
            .query_row("SELECT repo_name FROM checkout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "home", "the committed name, not the edited file's");
    }

    /// A measurement, not a regression test: the derived index of THIS checkout's HEAD, built
    /// through the entry points `derive-shadow` uses and printed with every count. Run it with
    /// `cargo test --lib derived_index_of_this_repo -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement of this repository's HEAD: run with --ignored --nocapture"]
    fn derived_index_of_this_repo() {
        const KNOWN_EDGE_KINDS: [&str; 9] = [
            "file",
            "entry",
            "local",
            "unknown_key",
            "key_collision",
            "undefined_token",
            "ambiguous_token",
            "cross_repo",
            "malformed",
        ];
        const KNOWN_SKIP_REASONS: [&str; 3] = ["not_utf8", "symlink", "outside_key_space"];

        let repo = git2::Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let root = std::fs::canonicalize(repo.workdir().unwrap()).unwrap();
        // The index goes to a temp file outside the repository.
        let scratch = tempfile::tempdir().unwrap();
        assert!(
            !std::fs::canonicalize(scratch.path())
                .unwrap()
                .starts_with(&root),
            "the scratch index must lie outside {}",
            root.display()
        );
        let mut conn = open_index(&scratch.path().join("derived.db")).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let report = build_full(
            &mut conn,
            &BuildInput::from_commit(&repo, &commit, &root).unwrap(),
        )
        .unwrap();
        assert_report_matches_tables(&conn, &report);

        let grouped = |sql: &str| -> Vec<(String, usize)> {
            conn.prepare(sql)
                .unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as usize)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let edges = grouped("SELECT kind, count(*) FROM edge GROUP BY kind ORDER BY kind");
        let problems = grouped("SELECT kind, count(*) FROM problem GROUP BY kind ORDER BY kind");
        let skipped =
            grouped("SELECT reason, count(*) FROM skipped GROUP BY reason ORDER BY reason");
        let (name, name_source, rules_source): (String, String, String) = conn
            .query_row(
                "SELECT repo_name, name_source, rules_source FROM checkout",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        println!("commit {} in {}", commit.id(), root.display());
        println!("repo_name {name} ({name_source}), rules_source {rules_source}");
        println!("{report:?}");
        println!(
            "documents {}, entries {}, edges {}, chunks {}, problems {}, skipped {}",
            report.documents,
            report.entries,
            report.edges,
            report.chunks,
            report.problems,
            report.skipped
        );
        println!("edges by kind: {edges:?}");
        println!("problems by kind: {problems:?}");
        println!("skipped by reason: {skipped:?}");

        assert!(report.documents > 0, "no document in HEAD's tree");
        assert_eq!(
            edges.iter().map(|(_, n)| n).sum::<usize>(),
            report.edges,
            "the per-kind edge counts sum to the total"
        );
        for (kind, _) in &edges {
            assert!(
                KNOWN_EDGE_KINDS.contains(&kind.as_str()),
                "edge kind {kind}"
            );
        }
        for (reason, _) in &skipped {
            assert!(
                KNOWN_SKIP_REASONS.contains(&reason.as_str()),
                "skip reason {reason}"
            );
        }
    }
}
