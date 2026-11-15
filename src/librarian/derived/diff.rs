//! The shadow diff: how far the derived index is from the machine's old catalog, measured
//! without writing to the catalog or its `-wal` (SQLite itself updates read marks in an
//! existing `-shm`, see "The read-only open" below).
//!
//! [`open_old_read_only`] opens the old `catalog.db` with `SQLITE_OPEN_READ_ONLY` (never through
//! `Catalog::open`, which migrates; never `apply_schema`, `run_migrations` or `audit::install`)
//! and [`diff`] / [`diff_with_rules`] compare it with a derived index built from one commit. Both
//! are plain SQL through `rusqlite`: no file of a repository is read by path, and a failing
//! query is an error and never a panic (`schema::dump_all` panics, so it is not used here).
//!
//! # What is compared, and how the old rows are scoped
//!
//! The old catalog is machine-global: one file, many repositories and worktrees, absolute paths.
//! An old `artifact` row is kept only when its `abs_path` is under the git root, or under
//! another spelling of it that the caller gives ([`RootMap`]; the git root is tried first). The
//! test compares path components, so `/work/proj2/x.md` is not under `/work/proj`. The path is
//! made git-root-relative with `/`,
//! and it does not sit under `<git_root>/.worktrees/` (a shadow copy). Every other row is counted
//! and then ignored, each in exactly one bucket, tested in this order: `ignored_other_roots` (a
//! path outside the git root, or one with a `..` component after it), `ignored_worktrees`,
//! `ignored_missing` (`missing_since IS NOT NULL`: a catalog GC leftover). Two live rows that
//! name one file (`abs_path` is unique only as text: `/root/a.md` and `/root//a.md`) are kept
//! once: rows are read in `id` order, the lowest id is kept, and the others are counted in
//! `old_path_collisions`.
//!
//! When the catalog has rows but none lies under the git root (every row is in
//! `ignored_other_roots`), [`diff_with_rules`] returns [`NoOldRowsUnderRoot`] and no diff: the
//! old indexer stores `abs_path` as its caller held it, so a project indexed through a symlink
//! looks like this, and a diff would call every derived document new from no evidence. A catalog
//! with no row at all, or only worktree copies and GC leftovers, is not that case: it compares,
//! and every derived document is only in the derived index.
//!
//! Documents are joined on the git-root-relative path to `document.rel_path`. `kind`, `status`,
//! `title` and `file_sha256` (the derived `sha256`) are compared exactly; `owners` and `tags` as
//! sets (the derived index stores them sorted and keeps duplicates from the frontmatter; the old
//! catalog keeps author order); chunks as a count per document (`chunk` rows against
//! `artifact_chunk` rows). The old `file_sha256` is the working-tree file's hash and the derived
//! one is the committed blob's: a difference means the working tree differs from the commit, or
//! the old row is stale (as `FileSetWalkVsCommit` below admits for a file set), and is reported
//! as `changed` on the field `file_sha256` with cause [`DiffCause::Unexplained`] (this module
//! never reads the working tree to classify it).
//!
//! # Edges
//!
//! The old catalog records edges at two grains: `artifact_link` (file grain; only `rel = 'cites'`
//! is a citation, the other relations are written by hand) and `entry_cite` (entry grain; `rel` in
//! `cites` and `rests-on`). A derived edge names its destination by key, and the old rows name
//! theirs by artifact id or by `<slug>:<TOKEN>`. The mapping used here:
//!
//! - an old artifact id is resolved through `artifact.id` to the artifact's git-root-relative
//!   path; a slug through `artifact.slug` (unique across the whole catalog) to the same path.
//!   A slug is `slugify(title)` or the file stem and is **not** a key: it is never compared with
//!   `document."key"` as text;
//! - a derived `dst_key` is resolved through `document."key"` to that document's `rel_path`
//!   (`file` and `entry` edges only exist for a key one document holds). `document.owner_key`
//!   plays no part: for an entry cited through a folder index `dst_key` is already the owned
//!   document that defines the token, which is the document the old `<slug>:<TOKEN>` names.
//!
//! Edges are then compared at two grains, as sets: file grain `(src, dst)` (old `artifact_link`
//! rows and old `entry_cite` rows whose `dst_ref` is a 16-hex artifact id, against the derived
//! `file` and `entry` edges with `src != dst`, because the old scanner never links a document to
//! itself), and entry grain `(src, dst, token)` (old `entry_cite` rows whose `dst_ref` is
//! `<slug>:<TOKEN>`, against the derived `entry` edges and the `local` edges, which are a
//! document's citations of its own entries: the old catalog records those as entry cites with the
//! same slug on both sides). The grains overlap: the old catalog records a cross-document
//! citation in both `artifact_link` and `entry_cite`, so one citation present on one side only
//! gives a `FileEdge` row (the pair) and an `EntryEdge` row (the pair and the token). They are two
//! rows for one citation, not two citations; the text report says so in its header.
//!
//! The old `src_local` is not compared, but the derived index does record where an edge was
//! written (`edge.line`, and the `entry` table's spans `heading_line..=end_line`), and that
//! narrows the derived entry-grain edges to the ones the old scanner could have written
//! (`link_scan::attribute_entry_edge`):
//!
//! - a citation outside every entry section of its document (a preamble, an `## Index` row, any
//!   other prose) gets no `entry_cite` row. A line belongs to the innermost section whose span
//!   holds it, both ends inclusive, as in `link_scan::extract::entry_section_at`;
//! - a document citing its own token from inside the entry that defines it (`src == dst` and the
//!   token is the enclosing entry's) gets none: the scanner refuses a self-reference at entry
//!   grain. This holds for a `local` edge and for an `entry` edge that names the source document;
//! - an `entry_cite` row is keyed on the slugs of both documents, so an edge with an end that has
//!   no slug in the old catalog cannot be one.
//!
//! An edge that fails one of these is not listed as a row. It is counted under
//! `new_edges_not_compared_no_slug`, `new_edges_not_compared_outside_an_entry` and
//! `new_edges_not_compared_own_entry` (first reason in that order; a distinct `(src, dst, token)`
//! is counted once, and it stays compared when any one of its citations qualifies).
//!
//! Only edges whose source and destination are both documents present on both sides are
//! compared. The others are a consequence of a file-set difference already listed as a document
//! row, and are counted (`old_edges_not_compared`, `new_edges_not_compared`), not listed. Old
//! edge rows with an endpoint that is not an in-scope live document are counted in
//! `ignored_old_edges`.
//!
//! # Causes
//!
//! Every row carries a [`DiffCause`], and a cause is set only when its rule matches positively;
//! everything else is [`DiffCause::Unexplained`]. The rules, in full:
//!
//! - [`DiffCause::FileSetWalkVsCommit`]: a document only in the derived index (the commit tracks
//!   it and the old catalog has no live in-scope row for it), or only in the old catalog and in
//!   neither the derived `document` nor the derived `skipped` table (the commit holds no `.md`
//!   blob at that path). The data cannot tell "the old walk skips it" from "the old catalog is
//!   stale", so a row of the first kind may also be a file added since the old catalog last ran.
//! - [`DiffCause::KeyOutsideSpace`]: an only-old document whose path is in the derived `skipped`
//!   table with reason `outside_key_space`. A path skipped as `not_utf8` or `symlink` is not
//!   explained by a listed cause and stays `Unexplained` (the reason is shown in `new`).
//! - [`DiffCause::ParseRule`]: a `title`, `kind` or `status` difference on a document whose
//!   derived `parse_note` is `frontmatter_error`; or a `tags` difference whose every differing tag
//!   is a tag of the committed-plus-built-in rule that matches the path (only when the caller
//!   gives the rules, see [`diff_with_rules`]).
//! - [`DiffCause::ClosedGrammarDropsEdge`]: an only-old edge for which the old catalog recorded at
//!   least one target text and the closed citation grammar rejects every one of them. The text is
//!   `entry_cite.dst_ref`, and the scanner REBUILDS it (`link_scan::entry_dst_ref`): it is not
//!   what the author wrote. An artifact-id citation and a relative-path link both become the
//!   16-hex id of the destination, and a bare or stem-qualified entry token (`R-43`,
//!   `stem:R-43`) becomes `<dst_slug>:<TOKEN>` with the qualifier dropped. The closed grammar
//!   (`identity/grammar.rs`) reads wiki links only (`[[key]]`, `[[key:TOKEN]]`; a bare token only
//!   inside its own document), so a citation that was an id or a path link was never an edge
//!   there, and a rebuilt `<slug>:<TOKEN>` that the grammar refuses could not have been named by
//!   any wiki link either. The exact test is `identity::grammar::parse_target(text)` returning
//!   `Reference::Malformed`: a 16-hex id is refused by `Key::parse` (a key must not be
//!   hex-shaped), and `<slug>:<TOKEN>` is refused when the slug is not a key or the token is not
//!   one to three capitals, a hyphen and digits. An `artifact_link` row alone records no target
//!   text (its destination is an id by construction, so testing it would match every row): it
//!   gets the cause only through an `entry_cite` row for the same pair. A target the grammar does
//!   accept stays `Unexplained`, although the old scanner read bare cross-document tokens and
//!   path links, which the closed grammar does not: the rebuilt text does not say how the
//!   citation was written.
//!
//! The two opens of the old catalog (read-only, immutable), the side-file rule, [`RootMap`] and
//! the scoping of an old `abs_path` live in [`super::old_catalog`]; this module re-exports them.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{anyhow, bail, Context};
use rusqlite::Connection;
use serde::Serialize;

use super::build::PARSE_NOTE_FRONTMATTER_ERROR;
pub use super::old_catalog::{
    file_stamp, open_old_immutable, open_old_read_only, side_files_present, OldCatalog, RootMap,
};
pub(crate) use super::old_catalog::{scope_of, Scope};
use super::old_catalog::{OLD_ARTIFACTS_SQL, OLD_CHUNKS_SQL, OLD_CITES_SQL, OLD_LINKS_SQL};
use super::scan::Skip;
use crate::librarian::classify::{classify, CompiledRule};
use crate::librarian::identity::grammar::{parse_target, Reference};
use crate::librarian::identity::repo_name::is_hex16;

/// The `edge.kind` values the builder writes (`build::edge_kind`) that this diff reads. A test
/// checks them against a real build.
const EDGE_FILE: &str = "file";
const EDGE_ENTRY: &str = "entry";
const EDGE_LOCAL: &str = "local";

/// Why a row differs, or [`DiffCause::Unexplained`] when no rule in the module doc matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffCause {
    FileSetWalkVsCommit,
    ParseRule,
    ClosedGrammarDropsEdge,
    KeyOutsideSpace,
    Unexplained,
}

impl DiffCause {
    /// Every cause, in the order a report lists them.
    pub const ALL: [DiffCause; 5] = [
        DiffCause::FileSetWalkVsCommit,
        DiffCause::ParseRule,
        DiffCause::ClosedGrammarDropsEdge,
        DiffCause::KeyOutsideSpace,
        DiffCause::Unexplained,
    ];

    /// The name a report prints; the same as the JSON value.
    pub fn label(self) -> &'static str {
        match self {
            DiffCause::FileSetWalkVsCommit => "file_set_walk_vs_commit",
            DiffCause::ParseRule => "parse_rule",
            DiffCause::ClosedGrammarDropsEdge => "closed_grammar_drops_edge",
            DiffCause::KeyOutsideSpace => "key_outside_space",
            DiffCause::Unexplained => "unexplained",
        }
    }
}

/// What a row is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    /// A document, or one field of it.
    Document,
    /// A file-grain edge: `path` cites `old`/`new` as a whole document.
    FileEdge,
    /// An entry-grain edge: `path` cites the entry `old`/`new` (`<dst path>:<TOKEN>`).
    EntryEdge,
}

/// One difference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffRow {
    pub subject: Subject,
    /// The document's git-root-relative path; for an edge, the source's.
    pub path: String,
    /// For a `changed` row, the field that differs: `kind`, `status`, `title`, `owners`, `tags`,
    /// `file_sha256` or `chunk_count`.
    pub field: Option<String>,
    /// The old value (a `changed` row), or the destination of an only-old edge.
    pub old: Option<String>,
    /// The derived value (a `changed` row), or the destination of an only-new edge; for an
    /// only-old document that the derived index skipped, `skipped: <reasons>`.
    pub new: Option<String>,
    pub cause: DiffCause,
}

/// What the diff counted. Every figure is a count of distinct things in one population; the
/// population is in the name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DiffCounts {
    /// Old documents in scope (under the git root, not a worktree copy, not missing).
    pub old_documents: usize,
    /// Derived documents.
    pub new_documents: usize,
    /// Documents on both sides.
    pub matched_documents: usize,
    pub only_old: usize,
    pub only_new: usize,
    pub changed: usize,
    /// Rows of the three lists above, by cause; every cause is present, with 0 when it has none.
    pub by_cause: BTreeMap<DiffCause, usize>,
    pub ignored_other_roots: usize,
    pub ignored_worktrees: usize,
    pub ignored_missing: usize,
    /// Distinct `(src, dst)` pairs at file grain, old and derived.
    pub old_file_edges: usize,
    pub new_file_edges: usize,
    /// Distinct `(src, dst, token)` triples at entry grain, old and derived.
    pub old_entry_edges: usize,
    pub new_entry_edges: usize,
    /// Edges of the figures above that were not compared because an endpoint is not a document
    /// on both sides (file and entry grain together).
    pub old_edges_not_compared: usize,
    pub new_edges_not_compared: usize,
    /// Derived entry-grain edges (distinct `(src, dst, token)`) whose two ends are documents on
    /// both sides but that the old scanner could not have written as an `entry_cite` row, so
    /// they are not compared and no row is listed. Each edge is counted under the first reason
    /// that applies, in this order. An end has no slug in the old catalog (`entry_cite` is keyed
    /// on the slug) ...
    pub new_edges_not_compared_no_slug: usize,
    /// ... none of its citations sits inside an entry section of the source (the scanner writes
    /// no entry row for a citation in a preamble, an `## Index` row or any prose outside an
    /// entry) ...
    pub new_edges_not_compared_outside_an_entry: usize,
    /// ... or every citation inside an entry is a document citing its own token from inside
    /// that very entry (the scanner refuses a self-reference at entry grain).
    pub new_edges_not_compared_own_entry: usize,
    /// Old catalog rows dropped because another row has the same git-root-relative path (two
    /// spellings of one file): the row with the lowest `id` is kept.
    pub old_path_collisions: usize,
    /// Old edge rows (`artifact_link` and `entry_cite`) with an endpoint that is not an in-scope
    /// live document, or an `entry_cite` whose `dst_ref` has neither shape.
    pub ignored_old_edges: usize,
}

/// The whole comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShadowDiff {
    pub only_old: Vec<DiffRow>,
    pub only_new: Vec<DiffRow>,
    pub changed: Vec<DiffRow>,
    pub counts: DiffCounts,
}

impl ShadowDiff {
    /// Every row of the three lists.
    pub fn rows(&self) -> impl Iterator<Item = &DiffRow> {
        self.only_old
            .iter()
            .chain(&self.only_new)
            .chain(&self.changed)
    }
}

/// The old catalog has rows, and none of them lies under the git root or an old root. Every
/// derived document
/// would then be "only in the derived index", a conclusion drawn from no evidence about this
/// repository: the usual cause is a root spelled differently on the two sides (the old indexer
/// stores `abs_path` as the caller held it, possibly through a symlink; the CLI canonicalizes
/// the root). [`diff_with_rules`] returns it as the error, so the CLI prints it as `NOT
/// COMPARED`. An old catalog with no rows at all is not this error: it compares, and every
/// derived document is only in the derived index. The text names no path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoOldRowsUnderRoot {
    /// How many `--old-root` paths were given.
    pub old_roots: usize,
    /// Old rows under some other root.
    pub other_roots: usize,
}

impl std::fmt::Display for NoOldRowsUnderRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rows = if self.other_roots == 1 {
            "row lies"
        } else {
            "rows lie"
        };
        if self.old_roots == 0 {
            write!(
                f,
                "no old row lies under the git root ({} {rows} under other roots; is the root \
                 spelled through a symlink? pass --old-root <the other spelling>)",
                self.other_roots
            )
        } else {
            write!(
                f,
                "no old row lies under the git root or under any of the {} --old-root paths \
                 given ({} {rows} under other roots)",
                self.old_roots, self.other_roots
            )
        }
    }
}

impl std::error::Error for NoOldRowsUnderRoot {}

/// [`diff_with_rules`] without rules: a `tags` difference is never attributed to a rule.
pub fn diff(old: &OldCatalog, derived: &Connection, roots: &RootMap) -> anyhow::Result<ShadowDiff> {
    diff_with_rules(old, derived, roots, &[])
}

/// Compare the old catalog with the derived index `derived`, for the repository whose root
/// `roots` spells. An old row is in scope when its `abs_path` lies under the git root or under
/// one of the old roots.
/// `rules` are the rules the derived index was built with (`build::rules_from_commit`), used only
/// to attribute a `tags` difference to a rule.
///
/// Errors: [`NoOldRowsUnderRoot`] (downcast it) when the old catalog has rows and none lies under
/// the git root or an old root; a SQLite error from either side.
pub fn diff_with_rules(
    old: &OldCatalog,
    derived: &Connection,
    roots: &RootMap,
    rules: &[CompiledRule],
) -> anyhow::Result<ShadowDiff> {
    // One read snapshot, so the four queries below see one state of a catalog that other
    // processes keep writing. It is dropped without a commit: no row is written (SQLite's read
    // marks in a WAL catalog's `-shm` are not ours to avoid).
    let snapshot = old
        .connection()
        .unchecked_transaction()
        .context("starting a read snapshot of the old catalog")?;
    let mut counts = DiffCounts::default();

    let old_docs = load_old_documents(&snapshot, roots, &mut counts)?;
    // Rows exist, none lies under the root: the root is probably spelled differently on the two
    // sides, and every derived document would get a cause from no evidence. An old catalog with
    // no row at all (or only worktree copies and GC leftovers) is not this case.
    if old_docs.is_empty() && counts.ignored_other_roots > 0 {
        return Err(NoOldRowsUnderRoot {
            old_roots: roots.old_roots.len(),
            other_roots: counts.ignored_other_roots,
        }
        .into());
    }
    let new_docs = load_new_documents(derived)?;
    let skipped = load_skipped(derived)?;
    counts.old_documents = old_docs.len();
    counts.new_documents = new_docs.len();

    let mut only_old = Vec::new();
    let mut only_new = Vec::new();
    let mut changed = Vec::new();

    // Documents.
    for path in old_docs.keys() {
        if new_docs.contains_key(path) {
            counts.matched_documents += 1;
        } else {
            only_old.push(only_old_document(path, skipped.get(path)));
        }
    }
    for path in new_docs.keys() {
        if !old_docs.contains_key(path) {
            only_new.push(DiffRow {
                subject: Subject::Document,
                path: path.clone(),
                field: None,
                old: None,
                new: None,
                cause: DiffCause::FileSetWalkVsCommit,
            });
        }
    }
    let old_chunks = load_old_chunk_counts(&snapshot, &old_docs)?;
    let new_chunks = load_new_chunk_counts(derived)?;
    for (path, o) in &old_docs {
        let Some(n) = new_docs.get(path) else {
            continue;
        };
        changed.extend(document_changes(
            o,
            n,
            old_chunks.get(path).copied().unwrap_or(0),
            new_chunks.get(path).copied().unwrap_or(0),
            rules,
        ));
    }

    // Edges, between documents both sides have.
    let id_to_rel: HashMap<&str, &str> = old_docs
        .values()
        .map(|d| (d.id.as_str(), d.rel_path.as_str()))
        .collect();
    let slug_to_rel: HashMap<&str, &str> = old_docs
        .values()
        .filter_map(|d| d.slug.as_deref().map(|s| (s, d.rel_path.as_str())))
        .collect();
    let old_edges = load_old_edges(&snapshot, &id_to_rel, &slug_to_rel, &mut counts)?;
    let key_to_rel = key_to_rel(&new_docs);
    let spans = load_entry_spans(derived)?;
    let new_edges = load_new_edges(derived, &key_to_rel, &spans)?;
    compare_edges(
        &old_edges,
        &new_edges,
        &old_docs,
        &new_docs,
        &mut counts,
        &mut only_old,
        &mut only_new,
    );

    counts.only_old = only_old.len();
    counts.only_new = only_new.len();
    counts.changed = changed.len();
    counts.by_cause = DiffCause::ALL.iter().map(|&c| (c, 0)).collect();
    for row in only_old.iter().chain(&only_new).chain(&changed) {
        *counts.by_cause.entry(row.cause).or_default() += 1;
    }
    Ok(ShadowDiff {
        only_old,
        only_new,
        changed,
        counts,
    })
}

/// A document of the old catalog that is in scope.
struct OldDoc {
    id: String,
    rel_path: String,
    kind: String,
    status: String,
    title: Option<String>,
    owners: BTreeSet<String>,
    tags: BTreeSet<String>,
    sha256: String,
    slug: Option<String>,
}

/// A document of the derived index.
struct NewDoc {
    rel_path: String,
    key: Option<String>,
    kind: String,
    status: String,
    title: Option<String>,
    owners: BTreeSet<String>,
    tags: BTreeSet<String>,
    sha256: String,
    parse_note: Option<String>,
}

/// A JSON array of strings as a set. Text that is not such an array becomes one element naming
/// it, so the row still differs and the report goes on.
fn json_set(text: &str) -> BTreeSet<String> {
    match serde_json::from_str::<Vec<String>>(text) {
        Ok(items) => items.into_iter().collect(),
        Err(_) => BTreeSet::from([format!("<not a JSON array: {text}>")]),
    }
}

/// A set as the sorted JSON array a row shows.
fn render_set(set: &BTreeSet<String>) -> String {
    serde_json::to_string(set).unwrap_or_default()
}

/// The in-scope documents of the old catalog by git-root-relative path, with the rows that are
/// not in scope counted in `counts`.
fn load_old_documents(
    conn: &Connection,
    roots: &RootMap,
    counts: &mut DiffCounts,
) -> anyhow::Result<BTreeMap<String, OldDoc>> {
    let mut stmt = conn
        .prepare(OLD_ARTIFACTS_SQL)
        .context("reading the old catalog's artifacts")?;
    let mut rows = stmt.query([])?;
    let mut docs = BTreeMap::new();
    while let Some(r) = rows.next()? {
        let abs_path: String = r.get(1)?;
        let rel_path = match roots.scope(&abs_path) {
            Scope::OtherRoot => {
                counts.ignored_other_roots += 1;
                continue;
            }
            Scope::Worktree => {
                counts.ignored_worktrees += 1;
                continue;
            }
            Scope::Inside(rel_path) => rel_path,
        };
        if r.get::<_, i64>(9)? != 0 {
            counts.ignored_missing += 1;
            continue;
        }
        let set = |index: usize| -> rusqlite::Result<BTreeSet<String>> {
            Ok(json_set(
                &r.get::<_, Option<String>>(index)?
                    .unwrap_or_else(|| "[]".to_owned()),
            ))
        };
        // Rows come in `id` order, so of two spellings of one file the lowest id is kept.
        let std::collections::btree_map::Entry::Vacant(slot) = docs.entry(rel_path.clone()) else {
            counts.old_path_collisions += 1;
            continue;
        };
        slot.insert(OldDoc {
            id: r.get(0)?,
            rel_path,
            kind: r.get(2)?,
            status: r.get(3)?,
            title: r.get(4)?,
            owners: set(5)?,
            tags: set(6)?,
            sha256: r.get(7)?,
            slug: r.get(8)?,
        });
    }
    Ok(docs)
}

fn load_new_documents(derived: &Connection) -> anyhow::Result<BTreeMap<String, NewDoc>> {
    let mut stmt = derived
        .prepare(
            "SELECT rel_path, \"key\", kind, status, title, owners, tags, sha256, parse_note \
             FROM document ORDER BY rel_path",
        )
        .context("reading the derived documents")?;
    let mut rows = stmt.query([])?;
    let mut docs = BTreeMap::new();
    while let Some(r) = rows.next()? {
        let rel_path: String = r.get(0)?;
        docs.insert(
            rel_path.clone(),
            NewDoc {
                rel_path,
                key: r.get(1)?,
                kind: r.get(2)?,
                status: r.get(3)?,
                title: r.get(4)?,
                owners: json_set(&r.get::<_, String>(5)?),
                tags: json_set(&r.get::<_, String>(6)?),
                sha256: r.get(7)?,
                parse_note: r.get(8)?,
            },
        );
    }
    Ok(docs)
}

/// The derived `skipped` rows: path to its reasons, sorted.
fn load_skipped(derived: &Connection) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let mut stmt = derived
        .prepare("SELECT rel_path, reason FROM skipped ORDER BY rel_path, reason")
        .context("reading the derived skipped files")?;
    let mut rows = stmt.query([])?;
    let mut skipped: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(r) = rows.next()? {
        skipped.entry(r.get(0)?).or_default().push(r.get(1)?);
    }
    Ok(skipped)
}

/// The row for a document only the old catalog has, and why.
fn only_old_document(path: &str, skipped: Option<&Vec<String>>) -> DiffRow {
    let (cause, new) = match skipped {
        // Neither a document nor a skipped file: the commit holds no `.md` blob at this path.
        None => (DiffCause::FileSetWalkVsCommit, None),
        Some(reasons) => {
            let cause = if reasons.iter().any(|r| r == Skip::OutsideKeySpace.reason()) {
                DiffCause::KeyOutsideSpace
            } else {
                DiffCause::Unexplained
            };
            (cause, Some(format!("skipped: {}", reasons.join(", "))))
        }
    };
    DiffRow {
        subject: Subject::Document,
        path: path.to_owned(),
        field: None,
        old: None,
        new,
        cause,
    }
}

/// Chunk rows per in-scope document in the old catalog (`artifact_chunk`) ...
fn load_old_chunk_counts(
    conn: &Connection,
    docs: &BTreeMap<String, OldDoc>,
) -> anyhow::Result<HashMap<String, usize>> {
    let by_id: HashMap<&str, &str> = docs
        .values()
        .map(|d| (d.id.as_str(), d.rel_path.as_str()))
        .collect();
    let mut stmt = conn
        .prepare(OLD_CHUNKS_SQL)
        .context("reading the old catalog's chunks")?;
    let mut rows = stmt.query([])?;
    let mut counts = HashMap::new();
    while let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        if let Some(rel) = by_id.get(id.as_str()) {
            counts.insert((*rel).to_owned(), r.get::<_, i64>(1)? as usize);
        }
    }
    Ok(counts)
}

/// ... and in the derived index (`chunk`).
fn load_new_chunk_counts(derived: &Connection) -> anyhow::Result<HashMap<String, usize>> {
    let mut stmt = derived
        .prepare("SELECT rel_path, count(*) FROM chunk GROUP BY rel_path")
        .context("reading the derived chunks")?;
    let mut rows = stmt.query([])?;
    let mut counts = HashMap::new();
    while let Some(r) = rows.next()? {
        counts.insert(r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize);
    }
    Ok(counts)
}

/// One row per compared field that differs on a document both sides have, in a fixed field
/// order: `kind`, `status`, `title`, `owners`, `tags`, `file_sha256`, `chunk_count`.
fn document_changes(
    old: &OldDoc,
    new: &NewDoc,
    old_chunks: usize,
    new_chunks: usize,
    rules: &[CompiledRule],
) -> Vec<DiffRow> {
    let row = |field: &str, o: Option<String>, n: Option<String>, cause: DiffCause| DiffRow {
        subject: Subject::Document,
        path: new.rel_path.clone(),
        field: Some(field.to_owned()),
        old: o,
        new: n,
        cause,
    };
    let frontmatter_error = new.parse_note.as_deref() == Some(PARSE_NOTE_FRONTMATTER_ERROR);
    let text_cause = if frontmatter_error {
        DiffCause::ParseRule
    } else {
        DiffCause::Unexplained
    };
    let mut rows = Vec::new();
    if old.kind != new.kind {
        rows.push(row(
            "kind",
            Some(old.kind.clone()),
            Some(new.kind.clone()),
            text_cause,
        ));
    }
    if old.status != new.status {
        rows.push(row(
            "status",
            Some(old.status.clone()),
            Some(new.status.clone()),
            text_cause,
        ));
    }
    if old.title != new.title {
        rows.push(row(
            "title",
            old.title.clone(),
            new.title.clone(),
            text_cause,
        ));
    }
    if old.owners != new.owners {
        rows.push(row(
            "owners",
            Some(render_set(&old.owners)),
            Some(render_set(&new.owners)),
            DiffCause::Unexplained,
        ));
    }
    if old.tags != new.tags {
        let cause = if tags_come_from_rules(rules, &new.rel_path, &old.tags, &new.tags) {
            DiffCause::ParseRule
        } else {
            DiffCause::Unexplained
        };
        rows.push(row(
            "tags",
            Some(render_set(&old.tags)),
            Some(render_set(&new.tags)),
            cause,
        ));
    }
    if old.sha256 != new.sha256 {
        rows.push(row(
            "file_sha256",
            Some(old.sha256.clone()),
            Some(new.sha256.clone()),
            DiffCause::Unexplained,
        ));
    }
    if old_chunks != new_chunks {
        rows.push(row(
            "chunk_count",
            Some(old_chunks.to_string()),
            Some(new_chunks.to_string()),
            DiffCause::Unexplained,
        ));
    }
    rows
}

/// True when the two tag sets differ and every tag that is in one and not the other is a tag of
/// the rule that matches `rel_path`: the rule gives the derived index that tag and the old
/// catalog was indexed without it.
fn tags_come_from_rules(
    rules: &[CompiledRule],
    rel_path: &str,
    old: &BTreeSet<String>,
    new: &BTreeSet<String>,
) -> bool {
    let Some(class) = classify(rules, rel_path) else {
        return false;
    };
    let mut differing = old.symmetric_difference(new).peekable();
    differing.peek().is_some() && differing.all(|tag| class.tags.contains(tag))
}

/// An edge at file grain: source and destination, git-root-relative.
type Pair = (String, String);
/// An edge at entry grain: source, destination, entry token.
type Triple = (String, String, String);

/// The old catalog's edges between in-scope documents.
#[derive(Default)]
struct OldEdges {
    file: BTreeSet<Pair>,
    entry: BTreeMap<Triple, BTreeSet<String>>,
    /// Every `entry_cite.dst_ref` recorded for a `(src, dst)` pair, at either grain. These are the
    /// scanner's rebuilt texts (see `link_scan::entry_dst_ref`): a 16-hex id or a
    /// `<dst_slug>:<TOKEN>`, not what the author wrote.
    texts: BTreeMap<Pair, BTreeSet<String>>,
}

/// The derived index's edges, resolved to paths. The entry-grain edges carry where their
/// citations were written, which decides whether the old scanner could have written them.
#[derive(Default)]
struct NewEdges {
    file: BTreeSet<Pair>,
    entry: BTreeMap<Triple, Written>,
}

/// Where the citations of one derived entry-grain edge sit, over every citation of it.
#[derive(Default, Clone, Copy)]
struct Written {
    /// Some citation lies inside an entry section of the source document.
    in_an_entry: bool,
    /// Some citation lies inside an entry section and is not the document citing its own token
    /// from inside that entry: the old scanner would have written an `entry_cite` row for it.
    scannable: bool,
}

/// An entry section of a derived document: the `entry` table's token and its line span.
struct Span {
    token: String,
    heading_line: i64,
    end_line: i64,
}

/// The entry section a citation on `line` belongs to: the innermost one whose span, both ends
/// inclusive, holds the line (the greatest `heading_line`), or `None` for a line outside every
/// entry. This is `link_scan::extract::entry_section_at` over the stored `entry` rows.
fn enclosing_entry(spans: &[Span], line: i64) -> Option<&Span> {
    spans
        .iter()
        .filter(|s| line >= s.heading_line && line <= s.end_line)
        .max_by_key(|s| s.heading_line)
}

/// The derived `entry` rows by document.
fn load_entry_spans(derived: &Connection) -> anyhow::Result<HashMap<String, Vec<Span>>> {
    let mut stmt = derived
        .prepare("SELECT rel_path, token, heading_line, end_line FROM entry")
        .context("reading the derived entries")?;
    let mut rows = stmt.query([])?;
    let mut spans: HashMap<String, Vec<Span>> = HashMap::new();
    while let Some(r) = rows.next()? {
        spans.entry(r.get(0)?).or_default().push(Span {
            token: r.get(1)?,
            heading_line: r.get(2)?,
            end_line: r.get(3)?,
        });
    }
    Ok(spans)
}

fn load_old_edges(
    conn: &Connection,
    id_to_rel: &HashMap<&str, &str>,
    slug_to_rel: &HashMap<&str, &str>,
    counts: &mut DiffCounts,
) -> anyhow::Result<OldEdges> {
    let mut edges = OldEdges::default();

    let mut stmt = conn
        .prepare(OLD_LINKS_SQL)
        .context("reading the old catalog's links")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let (src, dst): (String, String) = (r.get(0)?, r.get(1)?);
        match (id_to_rel.get(src.as_str()), id_to_rel.get(dst.as_str())) {
            (Some(src), Some(dst)) => {
                edges.file.insert(((*src).to_owned(), (*dst).to_owned()));
            }
            _ => counts.ignored_old_edges += 1,
        }
    }

    let mut stmt = conn
        .prepare(OLD_CITES_SQL)
        .context("reading the old catalog's entry citations")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let (src_slug, dst_ref): (String, String) = (r.get(0)?, r.get(1)?);
        let Some(src) = slug_to_rel.get(src_slug.as_str()) else {
            counts.ignored_old_edges += 1;
            continue;
        };
        // `dst_ref` is a 16-hex artifact id (a citation of a whole file) or `<slug>:<TOKEN>`.
        if is_hex16(&dst_ref) {
            let Some(dst) = id_to_rel.get(dst_ref.as_str()) else {
                counts.ignored_old_edges += 1;
                continue;
            };
            let pair = ((*src).to_owned(), (*dst).to_owned());
            edges.file.insert(pair.clone());
            edges.texts.entry(pair).or_default().insert(dst_ref);
        } else if let Some((dst_slug, token)) = dst_ref.split_once(':') {
            let Some(dst) = slug_to_rel.get(dst_slug) else {
                counts.ignored_old_edges += 1;
                continue;
            };
            let pair = ((*src).to_owned(), (*dst).to_owned());
            edges
                .entry
                .entry((pair.0.clone(), pair.1.clone(), token.to_owned()))
                .or_default()
                .insert(dst_ref.clone());
            edges.texts.entry(pair).or_default().insert(dst_ref);
        } else {
            counts.ignored_old_edges += 1;
        }
    }
    Ok(edges)
}

/// Each key of the derived index to the one document that holds it (`None` for a key two
/// documents hold: no `file` or `entry` edge resolves to such a key).
fn key_to_rel(docs: &BTreeMap<String, NewDoc>) -> HashMap<String, Option<String>> {
    let mut map: HashMap<String, Option<String>> = HashMap::new();
    for doc in docs.values() {
        let Some(key) = &doc.key else {
            continue;
        };
        map.entry(key.clone())
            .and_modify(|holder| *holder = None)
            .or_insert_with(|| Some(doc.rel_path.clone()));
    }
    map
}

fn load_new_edges(
    derived: &Connection,
    key_to_rel: &HashMap<String, Option<String>>,
    spans: &HashMap<String, Vec<Span>>,
) -> anyhow::Result<NewEdges> {
    let mut stmt = derived
        .prepare(
            "SELECT src_rel_path, kind, dst_key, target_token, line FROM edge \
             WHERE kind IN (?1, ?2, ?3) ORDER BY src_rel_path, ord",
        )
        .context("reading the derived edges")?;
    let mut rows = stmt.query([EDGE_FILE, EDGE_ENTRY, EDGE_LOCAL])?;
    let mut edges = NewEdges::default();
    while let Some(r) = rows.next()? {
        let src: String = r.get(0)?;
        let kind: String = r.get(1)?;
        let dst_key: Option<String> = r.get(2)?;
        let token: Option<String> = r.get(3)?;
        let line: i64 = r.get(4)?;
        let dst = || -> anyhow::Result<String> {
            let key = dst_key
                .as_deref()
                .ok_or_else(|| anyhow!("derived {kind} edge from {src} has no dst_key"))?;
            match key_to_rel.get(key) {
                Some(Some(rel)) => Ok(rel.clone()),
                _ => bail!("derived {kind} edge from {src} names the key {key:?}, which no single document holds"),
            }
        };
        let token = || -> anyhow::Result<String> {
            token
                .clone()
                .ok_or_else(|| anyhow!("derived {kind} edge from {src} has no target_token"))
        };
        let mut entry_edge = |dst: String, token: String| {
            let enclosing = enclosing_entry(spans.get(&src).map_or(&[], Vec::as_slice), line);
            // The scanner keeps a self-reference between two entries of one document and
            // refuses a document citing a token from inside the entry that defines it.
            let own = |section: &Span| dst == src && section.token == token;
            let slot = edges.entry.entry((src.clone(), dst.clone(), token.clone()));
            let written = slot.or_default();
            written.in_an_entry |= enclosing.is_some();
            written.scannable |= enclosing.is_some_and(|section| !own(section));
        };
        match kind.as_str() {
            EDGE_FILE => {
                let dst = dst()?;
                if dst != src {
                    edges.file.insert((src.clone(), dst));
                }
            }
            EDGE_ENTRY => {
                let (dst, token) = (dst()?, token()?);
                if dst != src {
                    edges.file.insert((src.clone(), dst.clone()));
                }
                entry_edge(dst, token);
            }
            _ => entry_edge(src.clone(), token()?),
        }
    }
    Ok(edges)
}

/// [`DiffCause::ClosedGrammarDropsEdge`] when the old catalog recorded target texts for an edge
/// and the closed grammar rejects every one of them (`parse_target` is `Malformed`); otherwise
/// [`DiffCause::Unexplained`].
fn grammar_cause(texts: Option<&BTreeSet<String>>) -> DiffCause {
    match texts {
        Some(texts)
            if !texts.is_empty()
                && texts
                    .iter()
                    .all(|t| matches!(parse_target(t), Reference::Malformed { .. })) =>
        {
            DiffCause::ClosedGrammarDropsEdge
        }
        _ => DiffCause::Unexplained,
    }
}

fn edge_row(subject: Subject, src: &str, dst: String, in_old: bool, cause: DiffCause) -> DiffRow {
    let (old, new) = if in_old {
        (Some(dst), None)
    } else {
        (None, Some(dst))
    };
    DiffRow {
        subject,
        path: src.to_owned(),
        field: None,
        old,
        new,
        cause,
    }
}

/// List the edges only one side has, between documents both sides have, and count the rest.
///
/// At entry grain the derived edges are first narrowed to the ones the old scanner could have
/// written (`attribute_entry_edge`): it writes an `entry_cite` row only for a citation inside an
/// entry section, keys the row on the slugs of both documents, and refuses a document citing its
/// own token from inside that entry. A derived edge that fails one of these is counted under
/// `new_edges_not_compared_*` (first reason that applies: no slug, outside an entry, own entry)
/// and gives no row.
fn compare_edges(
    old: &OldEdges,
    new: &NewEdges,
    old_docs: &BTreeMap<String, OldDoc>,
    new_docs: &BTreeMap<String, NewDoc>,
    counts: &mut DiffCounts,
    only_old: &mut Vec<DiffRow>,
    only_new: &mut Vec<DiffRow>,
) {
    let matched = |path: &str| old_docs.contains_key(path) && new_docs.contains_key(path);
    let slugged = |path: &str| old_docs.get(path).is_some_and(|d| d.slug.is_some());
    let pair_ok = |(src, dst): &Pair| matched(src) && matched(dst);
    let triple_ok = |(src, dst, _): &Triple| matched(src) && matched(dst);

    counts.old_file_edges = old.file.len();
    counts.new_file_edges = new.file.len();
    counts.old_entry_edges = old.entry.len();
    counts.new_entry_edges = new.entry.len();
    counts.old_edges_not_compared = old.file.iter().filter(|e| !pair_ok(e)).count()
        + old.entry.keys().filter(|e| !triple_ok(e)).count();
    counts.new_edges_not_compared = new.file.iter().filter(|e| !pair_ok(e)).count()
        + new.entry.keys().filter(|e| !triple_ok(e)).count();

    // The derived entry-grain edges the old scanner could have written.
    let mut new_entry: BTreeSet<&Triple> = BTreeSet::new();
    for (triple, written) in new.entry.iter().filter(|(e, _)| triple_ok(e)) {
        if !slugged(&triple.0) || !slugged(&triple.1) {
            counts.new_edges_not_compared_no_slug += 1;
        } else if !written.in_an_entry {
            counts.new_edges_not_compared_outside_an_entry += 1;
        } else if !written.scannable {
            counts.new_edges_not_compared_own_entry += 1;
        } else {
            new_entry.insert(triple);
        }
    }

    for pair in old.file.iter().filter(|e| pair_ok(e)) {
        if !new.file.contains(pair) {
            only_old.push(edge_row(
                Subject::FileEdge,
                &pair.0,
                pair.1.clone(),
                true,
                grammar_cause(old.texts.get(pair)),
            ));
        }
    }
    for pair in new.file.iter().filter(|e| pair_ok(e)) {
        if !old.file.contains(pair) {
            only_new.push(edge_row(
                Subject::FileEdge,
                &pair.0,
                pair.1.clone(),
                false,
                DiffCause::Unexplained,
            ));
        }
    }
    for (triple, texts) in old.entry.iter().filter(|(e, _)| triple_ok(e)) {
        if !new_entry.contains(triple) {
            only_old.push(edge_row(
                Subject::EntryEdge,
                &triple.0,
                format!("{}:{}", triple.1, triple.2),
                true,
                grammar_cause(Some(texts)),
            ));
        }
    }
    for triple in &new_entry {
        if !old.entry.contains_key(*triple) {
            only_new.push(edge_row(
                Subject::EntryEdge,
                &triple.0,
                format!("{}:{}", triple.1, triple.2),
                false,
                DiffCause::Unexplained,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::derived::build::{build_full, BuildInput};
    use crate::librarian::derived::scan::open_commit;
    use crate::librarian::derived::schema::open_in_memory;
    use crate::librarian::derived::testkit::{
        fill_old_catalog, old_row, repo_with, OldRow, FILE, OLD_CATALOG_SCHEMA,
    };
    use rusqlite::OpenFlags;
    use std::path::{Path, PathBuf};

    /// The git root the hand-built old rows spell their absolute paths under.
    const ROOT: &str = "/work/proj";

    struct Built {
        _dir: tempfile::TempDir,
        conn: Connection,
        rules: Vec<CompiledRule>,
    }

    /// A derived index (in memory) of a one-commit repo holding `files` (path, text); the
    /// commit's own rules and key space are used, as the CLI does.
    fn built(files: &[(&str, &str)]) -> Built {
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|&(path, text)| (path, FILE, text.as_bytes()))
            .collect();
        built_entries(&entries)
    }

    /// [`built`] with the tree entries (path, mode, bytes) given as they are.
    fn built_entries(entries: &[(&str, u32, &[u8])]) -> Built {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(dir.path(), entries);
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, Path::new(ROOT)).unwrap();
        let mut conn = open_in_memory().unwrap();
        build_full(&mut conn, &input).unwrap();
        Built {
            _dir: dir,
            conn,
            rules: input.rules,
        }
    }

    /// Old rows (and chunk counts) that agree with the derived index on every compared field,
    /// for `(id, rel_path)` pairs: a test then changes the one field it is about.
    fn mirrors(
        derived: &Connection,
        pairs: &[(&str, &str)],
    ) -> (Vec<OldRow>, Vec<(String, usize)>) {
        let mut rows = Vec::new();
        let mut chunks = Vec::new();
        for &(id, rel) in pairs {
            let (kind, status, title, owners, tags, sha): (
                String,
                String,
                Option<String>,
                String,
                String,
                String,
            ) = derived
                .query_row(
                    "SELECT kind, status, title, owners, tags, sha256 FROM document \
                     WHERE rel_path = ?1",
                    [rel],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                        ))
                    },
                )
                .unwrap();
            let count: i64 = derived
                .query_row(
                    "SELECT count(*) FROM chunk WHERE rel_path = ?1",
                    [rel],
                    |r| r.get(0),
                )
                .unwrap();
            rows.push(OldRow {
                kind,
                status,
                title,
                owners,
                tags,
                sha,
                ..old_row(id, &format!("{ROOT}/{rel}"))
            });
            chunks.push((id.to_owned(), count as usize));
        }
        (rows, chunks)
    }

    fn old_with(
        rows: &[OldRow],
        links: &[(&str, &str)],
        cites: &[(&str, &str)],
        chunks: &[(String, usize)],
    ) -> OldCatalog {
        let conn = Connection::open_in_memory().unwrap();
        fill_old_catalog(&conn, rows, links, cites, chunks);
        OldCatalog::from_connection(conn).unwrap()
    }

    /// The root map of a test that gives no old root.
    fn roots() -> RootMap {
        RootMap::new(PathBuf::from(ROOT))
    }

    fn run(old: &OldCatalog, b: &Built) -> ShadowDiff {
        diff(old, &b.conn, &roots()).unwrap()
    }

    fn document(path: &str, cause: DiffCause) -> DiffRow {
        DiffRow {
            subject: Subject::Document,
            path: path.to_owned(),
            field: None,
            old: None,
            new: None,
            cause,
        }
    }

    fn edge(subject: Subject, src: &str, dst: &str, cause: DiffCause) -> DiffRow {
        let (old, new) = (Some(dst.to_owned()), None);
        DiffRow {
            subject,
            path: src.to_owned(),
            field: None,
            old,
            new,
            cause,
        }
    }

    fn field(
        path: &str,
        name: &str,
        old: Option<&str>,
        new: Option<&str>,
        cause: DiffCause,
    ) -> DiffRow {
        DiffRow {
            subject: Subject::Document,
            path: path.to_owned(),
            field: Some(name.to_owned()),
            old: old.map(str::to_owned),
            new: new.map(str::to_owned),
            cause,
        }
    }

    #[test]
    fn a_file_the_walk_skipped_but_the_commit_tracks_is_only_new_with_the_walk_cause() {
        let b = built(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        let (rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.only_new,
            vec![document("b.md", DiffCause::FileSetWalkVsCommit)]
        );
        assert!(d.only_old.is_empty() && d.changed.is_empty(), "{d:#?}");
        assert_eq!(d.counts.old_documents, 1);
        assert_eq!(d.counts.new_documents, 2);
        assert_eq!(d.counts.matched_documents, 1);
        assert_eq!(d.counts.only_new, 1);
        assert_eq!(d.counts.by_cause[&DiffCause::FileSetWalkVsCommit], 1);
        assert_eq!(d.counts.by_cause[&DiffCause::Unexplained], 0);
    }

    #[test]
    fn a_file_only_the_old_catalog_has_is_only_old() {
        let b = built(&[("a.md", "# A\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows.push(old_row("id-ghost", &format!("{ROOT}/notes/untracked.md")));
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.only_old,
            vec![document(
                "notes/untracked.md",
                DiffCause::FileSetWalkVsCommit
            )]
        );
        assert!(d.only_new.is_empty() && d.changed.is_empty(), "{d:#?}");
        assert_eq!(d.counts.only_old, 1);
    }

    #[test]
    fn a_path_outside_the_key_space_is_only_old_with_that_cause() {
        let b = built(&[
            ("a.md", "# A\n"),
            (".codescout/key-space.toml", "exclude = [\"vendor/**\"]\n"),
            ("vendor/lib/readme.md", "# Vendored\n"),
        ]);
        let skipped: Vec<(String, String)> = b
            .conn
            .prepare("SELECT rel_path, reason FROM skipped")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            skipped,
            vec![(
                "vendor/lib/readme.md".to_owned(),
                "outside_key_space".to_owned()
            )],
            "the fixture's key space must exclude the vendored file"
        );
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows.push(old_row("id-v", &format!("{ROOT}/vendor/lib/readme.md")));
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.only_old,
            vec![DiffRow {
                new: Some("skipped: outside_key_space".to_owned()),
                ..document("vendor/lib/readme.md", DiffCause::KeyOutsideSpace)
            }]
        );
    }

    #[test]
    fn an_old_document_the_derived_index_skipped_for_another_reason_stays_unexplained() {
        use crate::librarian::derived::testkit::LINK;
        let b = built_entries(&[
            ("a.md", FILE, b"# A\n"),
            ("link.md", LINK, b"a.md"),
            ("bad.md", FILE, &[0x66, 0xff, 0xfe]),
        ]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows.push(old_row("id-l", &format!("{ROOT}/link.md")));
        rows.push(old_row("id-b", &format!("{ROOT}/bad.md")));
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        let only_old: Vec<_> = d
            .only_old
            .iter()
            .map(|r| (r.path.as_str(), r.new.as_deref(), r.cause))
            .collect();
        // Not in the closed list of causes: the reason is shown, the cause is not guessed.
        assert_eq!(
            only_old,
            vec![
                ("bad.md", Some("skipped: not_utf8"), DiffCause::Unexplained),
                ("link.md", Some("skipped: symlink"), DiffCause::Unexplained),
            ]
        );
    }

    #[test]
    fn a_title_difference_is_changed_and_names_the_field() {
        let b = built(&[("a.md", "# A title\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows[0].title = Some("Another title".to_owned());
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.changed,
            vec![field(
                "a.md",
                "title",
                Some("Another title"),
                Some("A title"),
                DiffCause::Unexplained
            )]
        );
        assert!(d.only_old.is_empty() && d.only_new.is_empty(), "{d:#?}");
        assert_eq!(d.counts.changed, 1);
        assert_eq!(d.counts.by_cause[&DiffCause::Unexplained], 1);
    }

    #[test]
    fn a_title_difference_on_a_frontmatter_error_document_is_the_parse_rule() {
        let broken = "---\nkind: [unterminated\n---\n# Real Title\n\nbody text\n";
        let b = built(&[("a.md", broken)]);
        // The old indexer parsed an empty body for such a file, so its title was NULL.
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows[0].title = None;
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.changed,
            vec![field(
                "a.md",
                "title",
                None,
                Some("Real Title"),
                DiffCause::ParseRule
            )]
        );
    }

    #[test]
    fn a_hex_id_edge_the_grammar_drops_is_explained() {
        let b = built(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        let (mut rows, chunks) = mirrors(
            &b.conn,
            &[("aaaaaaaaaaaaaaaa", "a.md"), ("0123456789abcdef", "b.md")],
        );
        rows[0].slug = Some("a-slug".to_owned());
        // The old scanner cites by artifact id: one artifact_link row and one entry_cite row
        // with the 16-hex id as `dst_ref`. The derived index has no such edge.
        let old = old_with(
            &rows,
            &[("aaaaaaaaaaaaaaaa", "0123456789abcdef")],
            &[("a-slug", "0123456789abcdef")],
            &chunks,
        );
        let d = run(&old, &b);
        assert_eq!(
            d.only_old,
            vec![edge(
                Subject::FileEdge,
                "a.md",
                "b.md",
                DiffCause::ClosedGrammarDropsEdge
            )]
        );
        assert!(d.only_new.is_empty() && d.changed.is_empty(), "{d:#?}");
        assert_eq!(d.counts.old_file_edges, 1);
        assert_eq!(d.counts.new_file_edges, 0);
        assert_eq!(d.counts.by_cause[&DiffCause::ClosedGrammarDropsEdge], 1);
    }

    #[test]
    fn an_entry_token_outside_the_grammar_is_explained_and_one_inside_it_is_not() {
        let b = built(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md"), ("id-b", "b.md")]);
        rows[0].slug = Some("a-slug".to_owned());
        rows[1].slug = Some("b-slug".to_owned());
        // SAGE-1 is four capitals: not an entry token. R-1 is one.
        let old = old_with(
            &rows,
            &[("id-a", "id-b")],
            &[("a-slug", "b-slug:SAGE-1"), ("a-slug", "b-slug:R-1")],
            &chunks,
        );
        let d = run(&old, &b);
        assert_eq!(
            d.only_old,
            vec![
                // The file pair has two target texts and the grammar accepts one of them.
                edge(Subject::FileEdge, "a.md", "b.md", DiffCause::Unexplained),
                edge(
                    Subject::EntryEdge,
                    "a.md",
                    "b.md:R-1",
                    DiffCause::Unexplained
                ),
                edge(
                    Subject::EntryEdge,
                    "a.md",
                    "b.md:SAGE-1",
                    DiffCause::ClosedGrammarDropsEdge
                ),
            ]
        );
    }

    #[test]
    fn edges_both_sides_agree_on_make_no_row_and_the_kind_strings_are_the_builders() {
        let b = built(&[
            (
                "a.md",
                "# A\n\n## R-0 — other\n\nSee [[b]], [[b:R-2]] and R-1.\n\n## R-1 — mine\n\nbody\n",
            ),
            ("b.md", "# B\n\n## R-2 — theirs\n"),
        ]);
        let kinds: Vec<String> = b
            .conn
            .prepare("SELECT kind FROM edge ORDER BY ord")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            kinds,
            vec![EDGE_FILE, EDGE_ENTRY, EDGE_LOCAL],
            "the builder's kinds"
        );
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md"), ("id-b", "b.md")]);
        rows[0].slug = Some("a-slug".to_owned());
        rows[1].slug = Some("b-slug".to_owned());
        let old = old_with(
            &rows,
            &[("id-a", "id-b")],
            &[("a-slug", "b-slug:R-2"), ("a-slug", "a-slug:R-1")],
            &chunks,
        );
        let d = run(&old, &b);
        assert!(d.rows().next().is_none(), "{d:#?}");
        assert_eq!(
            (
                d.counts.old_file_edges,
                d.counts.new_file_edges,
                d.counts.old_entry_edges,
                d.counts.new_entry_edges
            ),
            (1, 1, 2, 2)
        );
    }

    #[test]
    fn entry_cites_count_for_cites_and_rests_on_and_for_no_other_relation() {
        let b = built(&[
            ("a.md", "# A\n\n## R-0 — x\n\nSee [[b:R-2]].\n"),
            ("b.md", "# B\n\n## R-2 — theirs\n"),
        ]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md"), ("id-b", "b.md")]);
        rows[0].slug = Some("a-slug".to_owned());
        rows[1].slug = Some("b-slug".to_owned());
        // The only old record of a -> b:R-2 is a `rests-on` row, and of a -> b it is the
        // artifact_link row below.
        let old = old_with(&rows, &[("id-a", "id-b")], &[], &chunks);
        let insert = |src_local: &str, dst_ref: &str, rel: &str| {
            old.conn
                .execute(
                    "INSERT INTO entry_cite (src_slug, src_local, dst_ref, rel) \
                     VALUES ('a-slug', ?1, ?2, ?3)",
                    [src_local, dst_ref, rel],
                )
                .unwrap();
        };
        insert("X-1", "b-slug:R-2", "rests-on");
        // A relation that is not a citation is not compared: no only-old row for it.
        insert("X-1", "b-slug:R-9", "supersedes");
        let d = run(&old, &b);
        assert!(d.rows().next().is_none(), "{d:#?}");
        assert_eq!(d.counts.old_entry_edges, 1);
    }

    /// The old catalog for `a.md` -> `b.md`: the file-grain link, no entry cite, and the given
    /// slugs (the scanner keys an `entry_cite` row on both).
    fn linked_pair(b: &Built, a_slug: Option<&str>, b_slug: Option<&str>) -> OldCatalog {
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md"), ("id-b", "b.md")]);
        rows[0].slug = a_slug.map(str::to_owned);
        rows[1].slug = b_slug.map(str::to_owned);
        old_with(&rows, &[("id-a", "id-b")], &[], &chunks)
    }

    const B_WITH_R2: (&str, &str) = ("b.md", "# B\n\n## R-2 — theirs\n");

    #[test]
    fn a_citation_in_prose_outside_any_entry_gives_no_only_new_entry_row() {
        let b = built(&[
            (
                "a.md",
                "# A\n\nThe preamble cites [[b:R-2]].\n\n## R-0 — x\n\nbody\n",
            ),
            B_WITH_R2,
        ]);
        let d = run(&linked_pair(&b, Some("a-slug"), Some("b-slug")), &b);
        // The file-grain pair agrees. The entry-grain edge sits before the first entry of
        // a.md, where the old scanner writes no entry_cite row: not a difference.
        assert!(d.rows().next().is_none(), "{d:#?}");
        let c = &d.counts;
        assert_eq!(c.new_entry_edges, 1);
        assert_eq!(c.new_edges_not_compared_outside_an_entry, 1);
        assert_eq!(c.new_edges_not_compared_no_slug, 0);
        assert_eq!(c.new_edges_not_compared_own_entry, 0);
    }

    #[test]
    fn a_citation_inside_an_entry_that_the_old_side_lacks_is_an_only_new_entry_edge() {
        // The citation is on the last line of the entry R-0 (and of the file): a section's
        // `end_line` is inclusive, as in `entry_section_at`.
        let b = built(&[
            ("a.md", "# A\n\n## R-0 — x\n\nbody\nSee [[b:R-2]].\n"),
            B_WITH_R2,
        ]);
        let d = run(&linked_pair(&b, Some("a-slug"), Some("b-slug")), &b);
        assert_eq!(
            d.only_new,
            vec![edge_new(Subject::EntryEdge, "a.md", "b.md:R-2")]
        );
        assert!(d.only_old.is_empty() && d.changed.is_empty(), "{d:#?}");
        let c = &d.counts;
        assert_eq!(
            (
                c.new_edges_not_compared_outside_an_entry,
                c.new_edges_not_compared_no_slug,
                c.new_edges_not_compared_own_entry
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn a_local_edge_inside_its_own_tokens_entry_is_dropped() {
        // R-1 cited inside R-1: the scanner refuses a self-reference. R-3 cited inside R-2 is
        // an edge between two entries of one ledger, which it keeps.
        let text = "# A\n\n## R-1 — one\n\nsee R-1 here.\n\n## R-2 — two\n\nsee R-3 there.\n\n\
                    ## R-3 — three\n\nbody\n";
        let b = built(&[("a.md", text)]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows[0].slug = Some("a-slug".to_owned());
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.only_new,
            vec![edge_new(Subject::EntryEdge, "a.md", "a.md:R-3")]
        );
        assert!(d.only_old.is_empty() && d.changed.is_empty(), "{d:#?}");
        assert_eq!(d.counts.new_entry_edges, 2);
        assert_eq!(d.counts.new_edges_not_compared_own_entry, 1);
    }

    #[test]
    fn an_edge_from_a_document_with_no_slug_is_counted_not_compared_and_gives_no_row() {
        let b = built(&[("a.md", "# A\n\n## R-0 — x\n\nSee [[b:R-2]].\n"), B_WITH_R2]);
        // Either end without a slug: `entry_cite` cannot hold the row.
        for (a_slug, b_slug) in [(None, Some("b-slug")), (Some("a-slug"), None)] {
            let d = run(&linked_pair(&b, a_slug, b_slug), &b);
            assert!(d.rows().next().is_none(), "{a_slug:?} {b_slug:?}: {d:#?}");
            assert_eq!(d.counts.new_edges_not_compared_no_slug, 1);
            assert_eq!(d.counts.new_edges_not_compared_outside_an_entry, 0);
        }
    }

    #[test]
    fn an_old_catalog_whose_rows_all_lie_under_another_root_is_refused_by_name() {
        let b = built(&[("a.md", "# A\n")]);
        let rows = [
            old_row("id-x", "/elsewhere/proj/a.md"),
            old_row("id-y", "/elsewhere/proj/b.md"),
        ];
        let old = old_with(&rows, &[], &[], &[]);
        let err = diff(&old, &b.conn, &roots()).expect_err("nothing lies under the root");
        assert_eq!(
            err.downcast_ref::<NoOldRowsUnderRoot>(),
            Some(&NoOldRowsUnderRoot {
                old_roots: 0,
                other_roots: 2
            })
        );
        assert_eq!(
            err.to_string(),
            "no old row lies under the git root (2 rows lie under other roots; is the root \
             spelled through a symlink? pass --old-root <the other spelling>)"
        );
    }

    #[test]
    fn rows_under_an_old_root_are_compared_as_if_under_the_git_root() {
        let b = built(&[("a.md", "# A\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows[0].abs_path = "/old/spelling/a.md".to_owned();
        let old = old_with(&rows, &[], &[], &chunks);
        let map =
            RootMap::with_old_roots(PathBuf::from(ROOT), vec![PathBuf::from("/old/spelling")])
                .unwrap();
        let d = diff(&old, &b.conn, &map).unwrap();
        assert_eq!(d.counts.matched_documents, 1, "{d:#?}");
        assert_eq!(d.counts.ignored_other_roots, 0, "{d:#?}");
        assert!(d.rows().next().is_none(), "{d:#?}");
    }

    #[test]
    fn the_git_root_is_tried_before_an_old_root() {
        // An old root nested in the git root: `/work/proj/sub/a.md` is `sub/a.md` under the
        // git root and `a.md` under the old root. The git root answers first, so the two
        // rows below name two files and do not collide.
        let b = built(&[("a.md", "# A\n"), ("sub/a.md", "# A\n")]);
        let (rows, chunks) = mirrors(&b.conn, &[("id-1", "sub/a.md"), ("id-2", "a.md")]);
        let old = old_with(&rows, &[], &[], &chunks);
        let nested = RootMap::with_old_roots(
            PathBuf::from(ROOT),
            vec![PathBuf::from(format!("{ROOT}/sub"))],
        )
        .unwrap();
        let d = diff(&old, &b.conn, &nested).unwrap();
        assert_eq!(d.counts.old_path_collisions, 0, "{d:#?}");
        assert_eq!(d.counts.matched_documents, 2, "{d:#?}");
        assert!(d.rows().next().is_none(), "{d:#?}");

        // Two spellings of one file collide, and the lowest id is kept.
        let b = built(&[("a.md", "# A\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-b", "a.md")]);
        let mut twin = old_row("id-a", "/old/spelling/a.md");
        twin.title = Some("Other".to_owned());
        rows.insert(0, twin);
        let old = old_with(&rows, &[], &[], &chunks);
        let map =
            RootMap::with_old_roots(PathBuf::from(ROOT), vec![PathBuf::from("/old/spelling")])
                .unwrap();
        let d = diff(&old, &b.conn, &map).unwrap();
        assert_eq!(d.counts.old_path_collisions, 1, "{d:#?}");
        assert_eq!(d.counts.old_documents, 1, "{d:#?}");
        // `id-a` (the lowest id) is kept: the title it holds is the one the diff reports.
        assert!(
            d.changed
                .iter()
                .any(|r| r.field.as_deref() == Some("title") && r.old.as_deref() == Some("Other")),
            "{d:#?}"
        );
    }

    #[test]
    fn a_root_map_lists_the_git_root_first_and_judges_each_spelling() {
        let map = RootMap::with_old_roots(
            PathBuf::from("/work/proj"),
            vec![PathBuf::from("/old/a"), PathBuf::from("/old/b")],
        )
        .unwrap();
        assert_eq!(
            map.spellings(),
            vec![
                Path::new("/work/proj"),
                Path::new("/old/a"),
                Path::new("/old/b")
            ]
        );
        assert!(matches!(map.scope("/old/b/x/y.md"), Scope::Inside(p) if p == "x/y.md"));
        assert!(matches!(
            map.scope("/old/b/.worktrees/w/y.md"),
            Scope::Worktree
        ));
        assert!(matches!(map.scope("/old/c/y.md"), Scope::OtherRoot));
        assert!(matches!(
            RootMap::new(PathBuf::from("/work/proj")).scope("/old/b/y.md"),
            Scope::OtherRoot
        ));
    }

    #[test]
    fn no_row_under_the_root_or_an_old_root_is_refused_without_a_path() {
        let b = built(&[("a.md", "# A\n")]);
        let old = old_with(&[old_row("id-x", "/elsewhere/proj/a.md")], &[], &[], &[]);
        let map =
            RootMap::with_old_roots(PathBuf::from(ROOT), vec![PathBuf::from("/old/spelling")])
                .unwrap();
        let err = diff(&old, &b.conn, &map).expect_err("nothing lies under either root");
        assert_eq!(
            err.downcast_ref::<NoOldRowsUnderRoot>(),
            Some(&NoOldRowsUnderRoot {
                old_roots: 1,
                other_roots: 1
            })
        );
        let text = err.to_string();
        assert!(text.contains("--old-root"), "{text}");
        assert!(!text.contains(ROOT), "the git root is in the text: {text}");
        assert!(
            !text.contains("/old/spelling"),
            "the old root is in the text: {text}"
        );
        assert!(!text.contains('/'), "a path is in the text: {text}");
    }

    #[test]
    fn the_symlink_hint_stays_when_no_old_root_is_given() {
        let text = NoOldRowsUnderRoot {
            old_roots: 0,
            other_roots: 3,
        }
        .to_string();
        assert!(text.contains("symlink"), "{text}");
        assert!(text.contains("--old-root"), "{text}");
        assert!(!text.contains(ROOT), "{text}");
    }

    #[test]
    fn an_old_catalog_with_no_row_under_any_other_root_still_compares() {
        let b = built(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        // No rows at all, and rows that are only worktree copies or GC leftovers: nothing says
        // the root is misspelled, so every derived document is only in the derived index.
        let mut gone = old_row("id-gone", &format!("{ROOT}/gone.md"));
        gone.missing = true;
        let copy = old_row("id-copy", &format!("{ROOT}/.worktrees/w/a.md"));
        for rows in [vec![], vec![gone, copy]] {
            let d = run(&old_with(&rows, &[], &[], &[]), &b);
            assert_eq!(
                d.only_new,
                vec![
                    document("a.md", DiffCause::FileSetWalkVsCommit),
                    document("b.md", DiffCause::FileSetWalkVsCommit)
                ]
            );
            assert!(d.only_old.is_empty() && d.changed.is_empty(), "{d:#?}");
            assert_eq!(d.counts.old_documents, 0);
            assert_eq!(d.counts.ignored_other_roots, 0);
        }
    }

    #[test]
    fn two_old_paths_that_name_one_file_are_kept_once_by_lowest_id_and_counted() {
        let b = built(&[("a.md", "# A\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        // The same file spelled with a doubled slash, a higher id, and stored FIRST.
        let mut twin = old_row("id-b", &format!("{ROOT}//a.md"));
        twin.title = Some("Other".to_owned());
        rows.insert(0, twin);
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        // `id-a` is kept whatever the table's row order: no `title` difference.
        assert!(d.rows().next().is_none(), "{d:#?}");
        assert_eq!(d.counts.old_path_collisions, 1);
        assert_eq!(d.counts.old_documents, 1);
    }

    #[test]
    fn an_edge_only_the_derived_index_has_is_only_new_and_a_hand_written_link_rel_is_not_compared()
    {
        let b = built(&[
            ("a.md", "# A\n\n## R-0 — x\n\nSee [[b:R-2]].\n"),
            ("b.md", "# B\n\n## R-2 — theirs\n"),
        ]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md"), ("id-b", "b.md")]);
        rows[0].slug = Some("a-slug".to_owned());
        rows[1].slug = Some("b-slug".to_owned());
        let old = old_with(&rows, &[], &[], &chunks);
        // A `supersedes` link is written by hand, not derived from a citation.
        old.conn
            .execute(
                "INSERT INTO artifact_link (src_id, dst_id, rel) \
                 VALUES ('id-b', 'id-a', 'supersedes')",
                [],
            )
            .unwrap();
        let d = run(&old, &b);
        assert_eq!(
            d.only_new,
            vec![
                edge_new(Subject::FileEdge, "a.md", "b.md"),
                edge_new(Subject::EntryEdge, "a.md", "b.md:R-2"),
            ]
        );
        assert!(d.only_old.is_empty() && d.changed.is_empty(), "{d:#?}");
    }

    fn edge_new(subject: Subject, src: &str, dst: &str) -> DiffRow {
        DiffRow {
            subject,
            path: src.to_owned(),
            field: None,
            old: None,
            new: Some(dst.to_owned()),
            cause: DiffCause::Unexplained,
        }
    }

    #[test]
    fn a_missing_old_catalog_is_not_compared_and_is_never_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let err = match open_old_read_only(&path) {
            Ok(_) => panic!("a missing catalog must not open"),
            Err(e) => e,
        };
        assert!(!path.exists(), "the open created {}", path.display());
        let beside: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert!(beside.is_empty(), "files appeared beside it: {beside:?}");
        assert!(
            format!("{err:#}").contains(&path.display().to_string()),
            "the error names the path: {err:#}"
        );
    }

    #[test]
    fn the_diff_opens_the_old_file_read_only() {
        let b = built(&[("a.md", "# A\n")]);
        let (rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        for read_only_on_disk in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("catalog.db");
            {
                let conn = Connection::open(&path).unwrap();
                fill_old_catalog(&conn, &rows, &[], &[], &chunks);
            }
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_readonly(read_only_on_disk);
            std::fs::set_permissions(&path, perms).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
            let beside = |d: &Path| {
                let mut names: Vec<_> = std::fs::read_dir(d)
                    .unwrap()
                    .map(|e| e.unwrap().file_name())
                    .collect();
                names.sort();
                names
            };
            let files_before = beside(dir.path());

            let old = open_old_read_only(&path)
                .unwrap_or_else(|e| panic!("read_only_on_disk={read_only_on_disk}: {e:#}"));
            let d = diff(&old, &b.conn, &roots()).unwrap();
            assert!(d.rows().next().is_none(), "{d:#?}");
            // The connection itself refuses a write, whatever the file's permissions say.
            let err = old
                .conn
                .execute("DELETE FROM artifact", [])
                .expect_err("a write through the old catalog's connection must fail");
            assert!(
                err.to_string()
                    .contains("attempt to write a readonly database"),
                "read_only_on_disk={read_only_on_disk}: {err}"
            );
            drop(old);

            assert_eq!(
                std::fs::read(&path).unwrap(),
                bytes,
                "read_only_on_disk={read_only_on_disk}: the file's bytes changed"
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().modified().unwrap(),
                mtime,
                "read_only_on_disk={read_only_on_disk}: the file's mtime changed"
            );
            assert_eq!(beside(dir.path()), files_before, "files appeared beside it");
        }
    }

    /// A WAL catalog file, cleanly closed: no `-wal` and no `-shm` beside it.
    fn clean_wal_catalog(dir: &Path, rows: &[OldRow]) -> std::path::PathBuf {
        let path = dir.join("catalog.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            fill_old_catalog(&conn, rows, &[], &[], &[]);
        }
        let names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("catalog.db")]);
        path
    }

    /// The finding that `cli::derive_shadow` guards against: with the bundled SQLite a read-only
    /// open of a cleanly closed WAL database in a writable directory succeeds, and SQLite
    /// itself creates `-wal` and `-shm` beside it. `open_old_read_only` therefore cannot promise
    /// "nothing appears beside the catalog" by itself, and the CLI refuses that case first.
    #[test]
    fn a_read_only_open_of_a_clean_wal_catalog_makes_sqlite_create_its_side_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = clean_wal_catalog(dir.path(), &[]);
        let old = open_old_read_only(&path).expect("the open succeeds");
        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["catalog.db", "catalog.db-shm", "catalog.db-wal"],
            "SQLite created the side files (if this stops being true, the CLI guard is not needed)"
        );
        drop(old);
    }

    /// ... and in a directory that cannot be written the same open is refused by SQLite, with
    /// its own message, and nothing is created.
    #[cfg(unix)]
    #[test]
    fn a_read_only_open_of_a_clean_wal_catalog_in_a_read_only_directory_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = clean_wal_catalog(dir.path(), &[]);
        let writable = std::fs::metadata(dir.path()).unwrap().permissions();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let restore = || std::fs::set_permissions(dir.path(), writable.clone()).unwrap();
        // A process that may write anyway (root) cannot show a refusal.
        if std::fs::File::create(dir.path().join("probe")).is_ok() {
            restore();
            return;
        }
        let result = open_old_read_only(&path);
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        restore();
        let err = match result {
            Ok(_) => panic!("a read-only open that needs the side files must be refused"),
            Err(e) => e,
        };
        assert!(
            format!("{err:#}").contains("attempt to write a readonly database"),
            "{err:#}"
        );
        assert_eq!(names, vec![std::ffi::OsString::from("catalog.db")]);
    }
    /// The names in `dir`, sorted.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn an_immutable_open_of_a_clean_wal_catalog_creates_nothing_and_changes_no_byte() {
        let dir = tempfile::tempdir().unwrap();
        let path = clean_wal_catalog(dir.path(), &[old_row("id-a", &format!("{ROOT}/a.md"))]);
        let bytes = std::fs::read(&path).unwrap();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert!(
            !side_files_present(&path),
            "a clean catalog has no side file"
        );

        let old = open_old_immutable(&path).expect("a clean WAL catalog opens");
        let rows: i64 = old
            .connection()
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "the open reads the rows");
        let err = old
            .connection()
            .execute("DELETE FROM artifact", [])
            .expect_err("a write through the immutable connection must fail");
        assert!(
            err.to_string().contains("readonly"),
            "the connection is read-only: {err}"
        );
        drop(old);

        assert_eq!(names_in(dir.path()), vec!["catalog.db".to_owned()]);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "the file's bytes changed"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            mtime,
            "the file's mtime changed"
        );
    }

    #[test]
    fn an_immutable_open_is_refused_for_each_side_file() {
        for suffix in ["-wal", "-shm", "-journal"] {
            let dir = tempfile::tempdir().unwrap();
            let path = clean_wal_catalog(dir.path(), &[]);
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            std::fs::write(&side, b"x").unwrap();
            assert!(side_files_present(&path), "{suffix}");

            let err = match open_old_immutable(&path) {
                Ok(_) => panic!("{suffix}: an immutable open must be refused"),
                Err(e) => e,
            };
            let text = format!("{err:#}");
            assert!(
                text.contains("Stop every codescout process"),
                "{suffix}: {text}"
            );
            assert!(
                text.contains("PRAGMA wal_checkpoint(TRUNCATE)"),
                "{suffix}: {text}"
            );
            assert!(
                !text.contains(&dir.path().display().to_string()),
                "{suffix}: the text holds an absolute path: {text}"
            );
            let mut want = vec!["catalog.db".to_owned(), format!("catalog.db{suffix}")];
            want.sort();
            assert_eq!(
                names_in(dir.path()),
                want,
                "{suffix}: no other file appears"
            );
        }
    }

    #[test]
    fn an_immutable_open_would_read_stale_rows_so_a_wal_with_rows_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        // This connection stands for a server that is still running: its rows lie only in the
        // `-wal` (the schema is checkpointed into the main file first, so a stale read finds
        // the tables and misses the row).
        let holder = Connection::open(&path).unwrap();
        holder.pragma_update(None, "journal_mode", "WAL").unwrap();
        fill_old_catalog(&holder, &[], &[], &[], &[]);
        holder
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .unwrap();
        holder.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        holder
            .execute(
                "INSERT INTO artifact (id, abs_path, kind, status, owners, tags, file_sha256) \
                 VALUES ('id-w', '/work/proj/w.md', 'doc', 'active', '[]', '[]', 'x')",
                [],
            )
            .unwrap();

        // The premise, measured: an immutable open ignores the `-wal` and reads no row.
        let raw = Connection::open_with_flags(
            format!("file:{}?mode=ro&immutable=1", path.display()),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .unwrap();
        let stale: i64 = raw
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stale, 0, "an immutable open reads the main file only");
        drop(raw);

        // The refusal is what keeps the diff from running on that stale answer.
        let err = match open_old_immutable(&path) {
            Ok(_) => panic!("a catalog whose rows lie in the -wal must be refused"),
            Err(e) => e,
        };
        assert!(
            format!("{err:#}").contains("Stop every codescout process"),
            "{err:#}"
        );
        let live: i64 = holder
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(live, 1, "the holder still sees its row");
    }

    #[test]
    fn an_immutable_open_reads_a_path_with_a_space_and_a_percent() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("my catalogs%20here");
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("old catalog#1.db");
        {
            let conn = Connection::open(&path).unwrap();
            fill_old_catalog(
                &conn,
                &[old_row("id-a", &format!("{ROOT}/a.md"))],
                &[],
                &[],
                &[],
            );
        }
        let old = open_old_immutable(&path).expect("the open decodes the path as given");
        let rows: i64 = old
            .connection()
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn a_relative_path_is_refused_by_the_immutable_open() {
        let err = match open_old_immutable(Path::new("catalog.db")) {
            Ok(_) => panic!("a relative path must be refused"),
            Err(e) => e,
        };
        assert!(
            format!("{err:#}").contains("absolute"),
            "the refusal says why: {err:#}"
        );
    }
    /// SQLite resolves a symlink and keeps `-wal` and `-shm` beside the TARGET. A check built from
    /// the path as given looks beside the link, finds nothing, and an immutable open then reads
    /// the main file alone.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_catalog_a_writer_holds_is_refused_through_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().join("real");
        let link_dir = dir.path().join("link");
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::create_dir(&link_dir).unwrap();
        let real = clean_wal_catalog(&real_dir, &[]);
        let link = link_dir.join("catalog.db");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        // A running server opened the catalog through the link; its row lies only in the `-wal`.
        let holder = Connection::open(&link).unwrap();
        holder.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        holder
            .execute(
                "INSERT INTO artifact (id, abs_path, kind, status, owners, tags, file_sha256) \
                 VALUES ('id-w', '/work/proj/w.md', 'doc', 'active', '[]', '[]', 'x')",
                [],
            )
            .unwrap();

        // The premise, measured: the side files lie beside the target, not beside the link, and
        // an immutable read through the link sees no row.
        assert_eq!(names_in(&link_dir), vec!["catalog.db".to_owned()]);
        assert!(names_in(&real_dir).contains(&"catalog.db-wal".to_owned()));
        let raw = Connection::open_with_flags(
            format!("file:{}?mode=ro&immutable=1", link.display()),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .unwrap();
        let stale: i64 = raw
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stale, 0, "the immutable read through the link is stale");
        drop(raw);

        let err = match open_old_immutable(&link) {
            Ok(_) => panic!("a symlinked catalog with rows in its -wal must be refused"),
            Err(e) => e,
        };
        let text = format!("{err:#}");
        assert!(text.contains("Stop every codescout process"), "{text}");
        assert!(!text.contains('/'), "the text holds a path: {text}");
        let live: i64 = holder
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(live, 1, "the holder still sees its row");
    }

    #[test]
    fn a_zero_length_journal_is_not_a_side_file_and_a_journal_with_bytes_is() {
        let dir = tempfile::tempdir().unwrap();
        let path = clean_wal_catalog(dir.path(), &[old_row("id-a", &format!("{ROOT}/a.md"))]);
        let journal = dir.path().join("catalog.db-journal");

        std::fs::write(&journal, b"").unwrap();
        assert!(
            !side_files_present(&path),
            "an empty journal is no hot journal"
        );
        let old = open_old_immutable(&path).expect("an empty journal does not stop the open");
        drop(old);
        assert_eq!(
            names_in(dir.path()),
            vec!["catalog.db".to_owned(), "catalog.db-journal".to_owned()],
            "the open neither created nor removed a file"
        );

        std::fs::write(&journal, b"x").unwrap();
        assert!(side_files_present(&path), "a journal with bytes is hot");
        assert!(open_old_immutable(&path).is_err());
    }

    #[test]
    fn an_old_root_above_the_git_root_is_refused_without_a_path() {
        for above in ["/work", "/work/", "/"] {
            let err = RootMap::with_old_roots(PathBuf::from(ROOT), vec![PathBuf::from(above)])
                .expect_err(above);
            let text = err.to_string();
            assert!(text.contains("--old-root"), "{above}: {text}");
            assert!(
                !text.contains('/'),
                "{above}: the text holds a path: {text}"
            );
        }
        // Accepted: the git root itself (however spelled), a root inside it, and a sibling.
        for fine in [
            ROOT,
            "/work/proj/",
            "/work/proj/sub",
            "/work/proj2",
            "/elsewhere",
        ] {
            let map = RootMap::with_old_roots(PathBuf::from(ROOT), vec![PathBuf::from(fine)])
                .unwrap_or_else(|e| panic!("{fine}: {e}"));
            assert_eq!(map.old_roots, vec![PathBuf::from(fine)]);
        }
    }

    /// An old root that is a link to a directory above the git root takes in the same rows.
    #[cfg(unix)]
    #[test]
    fn an_old_root_that_links_to_a_directory_above_the_git_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let top = std::fs::canonicalize(dir.path()).unwrap();
        let proj = top.join("proj");
        std::fs::create_dir(&proj).unwrap();
        let link = top.join("alias");
        std::os::unix::fs::symlink(&top, &link).unwrap();
        let err = RootMap::with_old_roots(proj.clone(), vec![link]).expect_err("above");
        assert!(err.to_string().contains("--old-root"), "{err}");
        // A link to the git root itself is the symlink case the flag exists for.
        let same = top.join("same");
        std::os::unix::fs::symlink(&proj, &same).unwrap();
        RootMap::with_old_roots(proj, vec![same]).expect("a link to the git root is fine");
    }

    #[test]
    fn a_failed_immutable_open_names_no_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.db");
        let err = match open_old_immutable(&path) {
            Ok(_) => panic!("a missing catalog must not open"),
            Err(e) => e,
        };
        let text = format!("{err:#}");
        assert!(text.contains("does not exist"), "{text}");
        assert!(!text.contains('/'), "the text holds a path: {text}");
        assert!(!path.exists(), "the open created the file");
        assert!(names_in(dir.path()).is_empty());
    }

    #[test]
    fn old_rows_outside_the_git_root_are_ignored_and_counted_and_worktree_copies_are_excluded() {
        let b = built(&[("a.md", "# A\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows.push(old_row("id-x", "/elsewhere/other-repo/docs/x.md"));
        // A string-prefix match would put this under `/work/proj`.
        rows.push(old_row("id-sib", "/work/proj2/docs/y.md"));
        rows.push(old_row("id-wt", &format!("{ROOT}/.worktrees/feature/a.md")));
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert!(d.rows().next().is_none(), "{d:#?}");
        assert_eq!(d.counts.ignored_other_roots, 2);
        assert_eq!(d.counts.ignored_worktrees, 1);
        assert_eq!(d.counts.ignored_missing, 0);
        assert_eq!(d.counts.old_documents, 1);
    }

    #[test]
    fn old_rows_with_missing_since_are_excluded_and_counted() {
        let b = built(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md"), ("id-b", "b.md")]);
        rows[1].missing = true;
        let mut ghost = old_row("id-ghost", &format!("{ROOT}/gone.md"));
        ghost.missing = true;
        rows.push(ghost);
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        // `gone.md` is not an old-only row, and `b.md` is only new: the old row for it is a GC
        // leftover, not a document the old catalog has.
        assert_eq!(
            d.only_new,
            vec![document("b.md", DiffCause::FileSetWalkVsCommit)]
        );
        assert!(d.only_old.is_empty() && d.changed.is_empty(), "{d:#?}");
        assert_eq!(d.counts.ignored_missing, 2);
        assert_eq!(d.counts.old_documents, 1);
    }

    #[test]
    fn owners_and_tags_are_compared_as_sets() {
        let text = "---\nowners: [zed, amy, amy]\ntags: [b, a]\n---\n# T\n";
        let b = built(&[("a.md", text)]);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        // Author order, no duplicate: the same sets.
        rows[0].owners = r#"["zed","amy"]"#.to_owned();
        rows[0].tags = r#"["b","a"]"#.to_owned();
        let same = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert!(same.rows().next().is_none(), "{same:#?}");
        // A different set is a difference, shown as a sorted set.
        rows[0].owners = r#"["amy","quentin"]"#.to_owned();
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        assert_eq!(
            d.changed,
            vec![field(
                "a.md",
                "owners",
                Some(r#"["amy","quentin"]"#),
                Some(r#"["amy","zed"]"#),
                DiffCause::Unexplained
            )]
        );
    }

    #[test]
    fn a_tag_the_committed_rules_give_is_the_parse_rule_only_when_it_is_the_whole_difference() {
        let b = built(&[
            (
                ".codescout/librarian.toml",
                "[[rule]]\nglob = \"src/**/*.md\"\nkind = \"doc\"\ntags = [\"ruled\"]\n",
            ),
            ("src/a.md", "# A\n"),
        ]);
        let tags_of: String = b
            .conn
            .query_row(
                "SELECT tags FROM document WHERE rel_path = 'src/a.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tags_of, r#"["ruled"]"#);
        let (mut rows, chunks) = mirrors(&b.conn, &[("id-a", "src/a.md")]);
        let root = roots();

        // The old catalog was indexed under other rules: it lacks the tag.
        rows[0].tags = "[]".to_owned();
        let old = old_with(&rows, &[], &[], &chunks);
        let with = diff_with_rules(&old, &b.conn, &root, &b.rules).unwrap();
        assert_eq!(
            with.changed,
            vec![field(
                "src/a.md",
                "tags",
                Some("[]"),
                Some(r#"["ruled"]"#),
                DiffCause::ParseRule
            )]
        );
        // Without the rules nothing says where the tag came from.
        let without = diff(&old, &b.conn, &root).unwrap();
        assert_eq!(without.changed[0].cause, DiffCause::Unexplained);

        // The old row has a tag of its own as well: the difference is not all the rule's.
        rows[0].tags = r#"["extra"]"#.to_owned();
        let old = old_with(&rows, &[], &[], &chunks);
        let mixed = diff_with_rules(&old, &b.conn, &root, &b.rules).unwrap();
        assert_eq!(mixed.changed[0].cause, DiffCause::Unexplained);
    }

    #[test]
    fn a_sha_a_status_and_a_chunk_count_difference_are_changed_and_unexplained() {
        let b = built(&[("a.md", "# A\n\nbody\n")]);
        let (mut rows, mut chunks) = mirrors(&b.conn, &[("id-a", "a.md")]);
        rows[0].sha = "ff".to_owned();
        rows[0].status = "archived".to_owned();
        let derived_chunks = chunks[0].1;
        chunks[0].1 = derived_chunks + 2;
        let d = run(&old_with(&rows, &[], &[], &chunks), &b);
        let derived_status: String = b
            .conn
            .query_row(
                "SELECT status FROM document WHERE rel_path = 'a.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let derived_sha: String = b
            .conn
            .query_row(
                "SELECT sha256 FROM document WHERE rel_path = 'a.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            d.changed,
            vec![
                field(
                    "a.md",
                    "status",
                    Some("archived"),
                    Some(&derived_status),
                    DiffCause::Unexplained
                ),
                field(
                    "a.md",
                    "file_sha256",
                    Some("ff"),
                    Some(&derived_sha),
                    DiffCause::Unexplained
                ),
                field(
                    "a.md",
                    "chunk_count",
                    Some(&(derived_chunks + 2).to_string()),
                    Some(&derived_chunks.to_string()),
                    DiffCause::Unexplained
                ),
            ]
        );
    }

    #[test]
    fn a_catalog_without_a_table_the_diff_reads_is_refused_by_name() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(OLD_CATALOG_SCHEMA).unwrap();
        conn.execute_batch("DROP TABLE entry_cite").unwrap();
        let err = match OldCatalog::from_connection(conn) {
            Ok(_) => panic!("a catalog with no entry_cite must be refused"),
            Err(e) => e,
        };
        assert!(
            format!("{err:#}").contains("entry_cite"),
            "the error names the table: {err:#}"
        );

        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE unrelated (x)").unwrap();
        let err = match OldCatalog::from_connection(conn) {
            Ok(_) => panic!("a database with no artifact table must be refused"),
            Err(e) => e,
        };
        assert!(format!("{err:#}").contains("artifact"), "{err:#}");
    }

    #[test]
    fn a_file_that_is_not_a_database_is_refused_with_sqlites_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        std::fs::write(&path, vec![b'x'; 4096]).unwrap();
        let err = match open_old_read_only(&path) {
            Ok(_) => panic!("a text file must not open as a catalog"),
            Err(e) => e,
        };
        assert!(
            format!("{err:#}").contains("not a database"),
            "SQLite's reason is kept: {err:#}"
        );
    }
}
