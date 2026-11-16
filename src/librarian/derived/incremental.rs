//! The incremental update of the derived index, and the parse cache it rests on.
//!
//! [`update`] brings the index to a commit by the same write path as a full build
//! ([`super::build::build_full`]); the only difference is that it may take a document's parse
//! products from `parse_cache` instead of parsing the document again. The cache is a pure
//! optimisation, and the index it leaves is the one a full build gives:
//!
//! - A row is read only for the exact `(sha256, rel_path, rules_hash, parser_version)` of the
//!   document being indexed. The path is in the key because classification and chunk ids depend
//!   on it, so a rename reparses; `rules_hash` covers the rules the build classifies with (the
//!   committed `librarian.toml` and the built-in defaults); [`PARSER_VERSION`] names the parse
//!   logic.
//! - A row that does not read back is a miss, never an error.
//! - Resolution is not cached. The key index is built over the whole tree and every document's
//!   citations are resolved against it on every update.
//! - After each build the cache holds exactly that build's documents.
//!
//! [`PARSER_VERSION`]: super::build::PARSER_VERSION

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::build::{
    rebuild, BuildInput, BuildReport, ChangeCounts, DerivedEventRow, DocMetadata, EntryRow,
    ParsedDocument, Parser,
};
use crate::git::{diff_tree_to_tree, DiffStatus};
use crate::librarian::catalog::chunk::ChunkRow;
use crate::librarian::classify::CompiledRule;
use crate::librarian::identity::grammar::{Citation, Reference};
use crate::librarian::identity::key::Key;
use crate::librarian::identity::repo_name::RepoName;

// The cache stores a document's parse products as JSON through the mirror types below, and not
// through serde derives on the grammar's own types: those are Plan A's, and a change to one of
// them must not change what a cache row means without `PARSER_VERSION` saying so.

#[derive(Serialize, Deserialize)]
struct MetadataJson {
    kind: String,
    status: String,
    time_scope: Option<String>,
    confidence: f64,
    title: Option<String>,
    owners: Vec<String>,
    tags: Vec<String>,
    topic: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct EntryJson {
    token: String,
    level: usize,
    heading_line: u32,
    end_line: u32,
}

#[derive(Serialize, Deserialize)]
enum ReferenceJson {
    File {
        repo: Option<String>,
        key: String,
    },
    Entry {
        repo: Option<String>,
        key: String,
        token: String,
    },
    Bare {
        token: String,
    },
    Malformed {
        raw: String,
    },
}

#[derive(Serialize, Deserialize)]
struct CitationJson {
    reference: ReferenceJson,
    line: u32,
}

/// The facts of a parse that no column above holds. No field is an `Option` or carries a serde
/// default, so a row written before a fact existed does not decode and is a cache miss (E2-R3).
///
/// The rule for a fact added later: its key must be required on decode (a cache row without it is
/// a miss), because `PARSER_VERSION` is not bumped for it; otherwise bump `PARSER_VERSION`. A
/// nullable field of a nested type takes [`required`], which serde's plain `Option` does not give.
#[derive(Serialize, Deserialize)]
struct FactsJson {
    events: Vec<EventJson>,
}

/// An event section as the cache holds it: [`DerivedEventRow`] field for field. Every key is
/// required on decode, the nullable ones included (an explicit `null` is a value, a missing key is
/// a miss); a field added here follows the rule on [`FactsJson`].
#[derive(Serialize, Deserialize)]
struct EventJson {
    heading_line: u32,
    end_line: u32,
    event_id: String,
    kind: String,
    created_at_ms: i64,
    #[serde(deserialize_with = "required")]
    author: Option<String>,
    #[serde(deserialize_with = "required")]
    anchor_commit: Option<String>,
    #[serde(deserialize_with = "required")]
    head_commit: Option<String>,
    #[serde(deserialize_with = "required")]
    parent: Option<String>,
    #[serde(deserialize_with = "required")]
    resolves: Option<String>,
    #[serde(deserialize_with = "required")]
    source_json: Option<String>,
    mutates: Vec<String>,
    payload_json: String,
}

/// `Option<T>` that must be present: serde reads a missing `Option` key as `None`, but a field
/// with `deserialize_with` has no such default, so a missing key is an error and an explicit
/// `null` is `None`.
fn required<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// A chunk without its `artifact_id`, which is the document's own `rel_path`.
#[derive(Serialize, Deserialize)]
struct ChunkJson {
    chunk_id: String,
    chunk_ix: usize,
    start_line: usize,
    end_line: usize,
    entry_token: Option<String>,
    entry_part: Option<usize>,
    entry_parts: Option<usize>,
    content: String,
    content_hash: String,
}

/// What identifies one parse: the cache returns a row only for the exact same bytes, at the
/// same path, under the same rules, by the same parse logic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct CacheKey {
    pub(super) sha256: String,
    pub(super) rel_path: String,
    pub(super) rules_hash: String,
    pub(super) parser_version: String,
}

/// One document's parse products, and the cache row to write for them when this build made them
/// (`stored` is `Some`) instead of taking them from the cache. The row is encoded when the
/// products are made, before the write transaction opens.
pub(super) struct Parsed {
    pub(super) key: CacheKey,
    pub(super) products: ParsedDocument,
    stored: Option<Stored>,
}

impl Parsed {
    /// Products read from the cache: nothing to write.
    pub(super) fn cached(key: CacheKey, products: ParsedDocument) -> Parsed {
        Parsed {
            key,
            products,
            stored: None,
        }
    }

    /// Products made by this build: the cache row is encoded now.
    pub(super) fn fresh(key: CacheKey, products: ParsedDocument) -> serde_json::Result<Parsed> {
        let stored = Some(encode(&products)?);
        Ok(Parsed {
            key,
            products,
            stored,
        })
    }
}

/// The columns of a `parse_cache` row that hold the products.
pub(super) struct Stored {
    pub(super) parse_note: Option<String>,
    pub(super) metadata_json: String,
    pub(super) entries_json: String,
    pub(super) citations_json: String,
    pub(super) chunks_json: String,
    pub(super) facts_json: String,
}

/// The cache row for `products`. A reader gets exactly `products` back from [`decode`].
pub(super) fn encode(products: &ParsedDocument) -> serde_json::Result<Stored> {
    let m = &products.metadata;
    let metadata = MetadataJson {
        kind: m.kind.clone(),
        status: m.status.clone(),
        time_scope: m.time_scope.clone(),
        confidence: m.confidence,
        title: m.title.clone(),
        owners: m.owners.clone(),
        tags: m.tags.clone(),
        topic: m.topic.clone(),
    };
    let entries: Vec<EntryJson> = products
        .entries
        .iter()
        .map(|e| EntryJson {
            token: e.token.clone(),
            level: e.level,
            heading_line: e.heading_line,
            end_line: e.end_line,
        })
        .collect();
    let repo_text = |repo: &Option<RepoName>| repo.as_ref().map(|r| r.as_str().to_owned());
    let citations: Vec<CitationJson> = products
        .citations
        .iter()
        .map(|c| CitationJson {
            reference: match &c.reference {
                Reference::File { repo, key } => ReferenceJson::File {
                    repo: repo_text(repo),
                    key: key.as_str().to_owned(),
                },
                Reference::Entry { repo, key, token } => ReferenceJson::Entry {
                    repo: repo_text(repo),
                    key: key.as_str().to_owned(),
                    token: token.clone(),
                },
                Reference::Bare { token } => ReferenceJson::Bare {
                    token: token.clone(),
                },
                Reference::Malformed { raw } => ReferenceJson::Malformed { raw: raw.clone() },
            },
            line: c.line,
        })
        .collect();
    let chunks: Vec<ChunkJson> = products
        .chunks
        .iter()
        .map(|c| ChunkJson {
            chunk_id: c.chunk_id.clone(),
            chunk_ix: c.chunk_ix,
            start_line: c.start_line,
            end_line: c.end_line,
            entry_token: c.entry_token.clone(),
            entry_part: c.entry_part,
            entry_parts: c.entry_parts,
            content: c.content.clone(),
            content_hash: c.content_hash.clone(),
        })
        .collect();
    let events: Vec<EventJson> = products
        .events
        .iter()
        .map(|e| EventJson {
            heading_line: e.heading_line,
            end_line: e.end_line,
            event_id: e.event_id.clone(),
            kind: e.kind.clone(),
            created_at_ms: e.created_at_ms,
            author: e.author.clone(),
            anchor_commit: e.anchor_commit.clone(),
            head_commit: e.head_commit.clone(),
            parent: e.parent.clone(),
            resolves: e.resolves.clone(),
            source_json: e.source_json.clone(),
            mutates: e.mutates.clone(),
            payload_json: e.payload_json.clone(),
        })
        .collect();
    Ok(Stored {
        parse_note: products.parse_note.clone(),
        metadata_json: serde_json::to_string(&metadata)?,
        entries_json: serde_json::to_string(&entries)?,
        citations_json: serde_json::to_string(&citations)?,
        chunks_json: serde_json::to_string(&chunks)?,
        facts_json: serde_json::to_string(&FactsJson { events })?,
    })
}

/// The products a cache row holds, for the document at `rel_path`. `None` when the row does not
/// read back: JSON that does not match the mirror types, or a key or repo name that is not one.
/// The caller treats that as a cache miss and parses the document.
pub(super) fn decode(rel_path: &str, stored: &Stored) -> Option<ParsedDocument> {
    let m: MetadataJson = serde_json::from_str(&stored.metadata_json).ok()?;
    let entries: Vec<EntryJson> = serde_json::from_str(&stored.entries_json).ok()?;
    let citations: Vec<CitationJson> = serde_json::from_str(&stored.citations_json).ok()?;
    let chunks: Vec<ChunkJson> = serde_json::from_str(&stored.chunks_json).ok()?;
    let FactsJson { events } = serde_json::from_str(&stored.facts_json).ok()?;
    let repo_of = |repo: Option<String>| match repo {
        None => Some(None),
        Some(name) => RepoName::parse(&name).ok().map(Some),
    };
    let citations = citations
        .into_iter()
        .map(|c| {
            let reference = match c.reference {
                ReferenceJson::File { repo, key } => Reference::File {
                    repo: repo_of(repo)?,
                    key: Key::parse(&key).ok()?,
                },
                ReferenceJson::Entry { repo, key, token } => Reference::Entry {
                    repo: repo_of(repo)?,
                    key: Key::parse(&key).ok()?,
                    token,
                },
                ReferenceJson::Bare { token } => Reference::Bare { token },
                ReferenceJson::Malformed { raw } => Reference::Malformed { raw },
            };
            Some(Citation {
                reference,
                line: c.line,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ParsedDocument {
        metadata: DocMetadata {
            kind: m.kind,
            status: m.status,
            time_scope: m.time_scope,
            confidence: m.confidence,
            title: m.title,
            owners: m.owners,
            tags: m.tags,
            topic: m.topic,
        },
        parse_note: stored.parse_note.clone(),
        entries: entries
            .into_iter()
            .map(|e| EntryRow {
                token: e.token,
                level: e.level,
                heading_line: e.heading_line,
                end_line: e.end_line,
            })
            .collect(),
        citations,
        chunks: chunks
            .into_iter()
            .map(|c| ChunkRow {
                chunk_id: c.chunk_id,
                artifact_id: rel_path.to_owned(),
                chunk_ix: c.chunk_ix,
                start_line: c.start_line,
                end_line: c.end_line,
                entry_token: c.entry_token,
                entry_part: c.entry_part,
                entry_parts: c.entry_parts,
                content: c.content,
                content_hash: c.content_hash,
            })
            .collect(),
        events: events
            .into_iter()
            .map(|e| DerivedEventRow {
                heading_line: e.heading_line,
                end_line: e.end_line,
                event_id: e.event_id,
                kind: e.kind,
                created_at_ms: e.created_at_ms,
                author: e.author,
                anchor_commit: e.anchor_commit,
                head_commit: e.head_commit,
                parent: e.parent,
                resolves: e.resolves,
                source_json: e.source_json,
                mutates: e.mutates,
                payload_json: e.payload_json,
            })
            .collect(),
    })
}

/// A digest of the rules a build classifies with: each rule's glob, kind, status, time scope and
/// tags, in order (the first match wins, so the order is part of the answer).
///
/// It is taken over the compiled rules the build was given, which is the commit's
/// `.codescout/librarian.toml` followed by the compiled-in `DEFAULT_RULES_TOML`. So a change to
/// the committed blob and a change to the built-in defaults both change it, and so would rules
/// that did not come from the commit at all.
pub(super) fn rules_hash(rules: &[CompiledRule]) -> String {
    let shape: Vec<serde_json::Value> = rules
        .iter()
        .map(|r| {
            serde_json::json!([
                r.matcher.glob().glob(),
                r.kind,
                r.status,
                r.time_scope,
                r.tags
            ])
        })
        .collect();
    format!(
        "{:x}",
        Sha256::digest(serde_json::Value::Array(shape).to_string().as_bytes())
    )
}

/// The cached products for `key`, or `None` on a miss. A row that is there but does not read
/// back (see [`decode`]) is a miss, not an error: the cache is an optimisation and a bad row is
/// only a reparse. A failure of the database itself is an error.
pub(super) fn load(conn: &Connection, key: &CacheKey) -> rusqlite::Result<Option<ParsedDocument>> {
    let stored = conn
        .prepare_cached(
            "SELECT parse_note, metadata_json, entries_json, citations_json, chunks_json, \
             facts_json FROM parse_cache \
             WHERE sha256 = ?1 AND rel_path = ?2 AND rules_hash = ?3 AND parser_version = ?4",
        )?
        .query_row(
            params![key.sha256, key.rel_path, key.rules_hash, key.parser_version],
            |r| {
                Ok(Stored {
                    parse_note: r.get(0)?,
                    metadata_json: r.get(1)?,
                    entries_json: r.get(2)?,
                    citations_json: r.get(3)?,
                    chunks_json: r.get(4)?,
                    facts_json: r.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(stored.and_then(|stored| decode(&key.rel_path, &stored)))
}

/// Make the cache hold exactly this build's documents: write the rows this build parsed, and
/// delete every other row (a document that was edited, renamed or removed, or a row of another
/// rules hash or parser version). Run inside the build's own transaction, so a failed build
/// changes neither the derived tables nor the cache.
pub(super) fn sync_cache(tx: &Connection, parsed: &[Parsed]) -> anyhow::Result<()> {
    let keep: HashSet<&CacheKey> = parsed.iter().map(|p| &p.key).collect();
    let held: Vec<CacheKey> = tx
        .prepare("SELECT sha256, rel_path, rules_hash, parser_version FROM parse_cache")?
        .query_map([], |r| {
            Ok(CacheKey {
                sha256: r.get(0)?,
                rel_path: r.get(1)?,
                rules_hash: r.get(2)?,
                parser_version: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    for key in held.iter().filter(|key| !keep.contains(key)) {
        tx.prepare_cached(
            "DELETE FROM parse_cache \
             WHERE sha256 = ?1 AND rel_path = ?2 AND rules_hash = ?3 AND parser_version = ?4",
        )?
        .execute(params![
            key.sha256,
            key.rel_path,
            key.rules_hash,
            key.parser_version
        ])?;
    }
    for (key, stored) in parsed
        .iter()
        .filter_map(|p| p.stored.as_ref().map(|s| (&p.key, s)))
    {
        // `OR REPLACE`: a row that was there but did not read back is replaced by the new one.
        tx.prepare_cached(
            "INSERT OR REPLACE INTO parse_cache (sha256, rel_path, rules_hash, parser_version, \
             parse_note, metadata_json, entries_json, citations_json, chunks_json, facts_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?
        .execute(params![
            key.sha256,
            key.rel_path,
            key.rules_hash,
            key.parser_version,
            stored.parse_note,
            stored.metadata_json,
            stored.entries_json,
            stored.citations_json,
            stored.chunks_json,
            stored.facts_json
        ])?;
    }
    Ok(())
}

/// Bring the derived index to `input.commit`, whatever it held before and whichever commit
/// `from_commit` is: the result dumps equal to [`build_full`](super::build::build_full) at the
/// same commit.
///
/// The commit is scanned in full. A document whose `(sha256, rel_path, rules_hash,
/// PARSER_VERSION)` is in the parse cache is not parsed again; every other document is, and the
/// cache is made to hold exactly this build's documents. The key index is rebuilt over the whole
/// tree and every document's citations are resolved against it afresh, since a document that did
/// not change can still change what its citations point at (a new document defines a key, two
/// documents come to share one). The rows are written in one transaction.
///
/// `from_commit` is used for [`ChangeCounts`] alone, taken from a git diff of the two commits.
/// Nothing the update stores depends on it, so it need not be an ancestor, nor even the commit
/// the index was last built from; a diff that cannot be taken only leaves the counts out.
pub fn update(
    conn: &mut Connection,
    input: &BuildInput,
    from_commit: &git2::Commit,
) -> anyhow::Result<BuildReport> {
    update_with(conn, input, from_commit, &Parser::PRODUCTION)
}

/// [`update`] with the parser given, so that a test can count parse calls or change the version.
pub(super) fn update_with(
    conn: &mut Connection,
    input: &BuildInput,
    from_commit: &git2::Commit,
    parser: &Parser,
) -> anyhow::Result<BuildReport> {
    rebuild(conn, input, parser, true, change_counts(input, from_commit))
}

/// What git says changed in markdown files between `from_commit` and `input.commit`; `None` when
/// the diff cannot be taken (the commits are in different repositories, an object is missing).
fn change_counts(input: &BuildInput, from_commit: &git2::Commit) -> Option<ChangeCounts> {
    let entries = diff_tree_to_tree(
        input.repo,
        &from_commit.id().to_string(),
        &input.commit.id().to_string(),
    )
    .ok()?;
    let is_markdown = |path: &str| path.ends_with(".md");
    let mut counts = ChangeCounts::default();
    for entry in entries {
        match entry.status {
            DiffStatus::Added if is_markdown(&entry.path) => counts.added += 1,
            DiffStatus::Modified if is_markdown(&entry.path) => counts.modified += 1,
            DiffStatus::Deleted if is_markdown(&entry.path) => counts.deleted += 1,
            DiffStatus::Renamed { old_path }
                if is_markdown(&entry.path) || is_markdown(&old_path) =>
            {
                counts.renamed += 1
            }
            _ => {}
        }
    }
    Some(counts)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::time::Duration;

    use super::*;
    use crate::librarian::catalog::event_text::{render_event_section, EventSource, EventText};
    use crate::librarian::classify::load_rules;
    use crate::librarian::derived::build::{
        build_full, parse_document, rules_from_commit, PARSER_VERSION,
    };
    use crate::librarian::derived::schema::{dump_all, open, open_in_memory};
    use crate::librarian::derived::testkit::{commit_files, repo_with, FILE};

    const ROOT: &str = "/fixture/root";

    type Files<'a> = &'a [(&'a str, &'a str)];
    type Dump = BTreeMap<String, Vec<Vec<String>>>;

    const REPO_TOML: (&str, &str) = (".codescout/repo.toml", "name = \"home\"\n");

    /// One repo with several commits. Snapshot 0 is a root commit on `HEAD`; each later snapshot
    /// is a child of the one before it. No ref moves after the first, so a commit is named by its
    /// index here and nothing else.
    struct History {
        _dir: tempfile::TempDir,
        repo: git2::Repository,
        ids: Vec<git2::Oid>,
    }

    fn bytes<'a>(files: Files<'a>) -> Vec<(&'a str, &'a [u8])> {
        files.iter().map(|&(p, t)| (p, t.as_bytes())).collect()
    }

    fn history(snapshots: &[Files]) -> History {
        let dir = tempfile::tempdir().unwrap();
        let first: Vec<(&str, u32, &[u8])> = bytes(snapshots[0])
            .into_iter()
            .map(|(p, b)| (p, FILE, b))
            .collect();
        let repo = repo_with(dir.path(), &first);
        let mut ids = vec![repo.head().unwrap().peel_to_commit().unwrap().id()];
        for (i, files) in snapshots.iter().enumerate().skip(1) {
            let parent = repo.find_commit(ids[i - 1]).unwrap();
            ids.push(commit_files(
                &repo,
                &bytes(files),
                &[&parent],
                &format!("snapshot {i}"),
            ));
        }
        History {
            _dir: dir,
            repo,
            ids,
        }
    }

    impl History {
        fn commit(&self, i: usize) -> git2::Commit<'_> {
            self.repo.find_commit(self.ids[i]).unwrap()
        }

        /// A commit with no parent and so no history in common with the others; its index.
        fn add_root(&mut self, files: Files) -> usize {
            let id = commit_files(&self.repo, &bytes(files), &[], "unrelated root");
            self.ids.push(id);
            self.ids.len() - 1
        }
    }

    /// What a caller does: the commit's own rules and key space, then `f` over the input.
    fn with_input<R>(
        repo: &git2::Repository,
        commit: &git2::Commit,
        f: impl FnOnce(&BuildInput) -> R,
    ) -> R {
        let input = BuildInput::from_commit(repo, commit, Path::new(ROOT)).unwrap();
        f(&input)
    }

    fn build_at(conn: &mut Connection, h: &History, i: usize) -> BuildReport {
        with_input(&h.repo, &h.commit(i), |input| build_full(conn, input)).unwrap()
    }

    fn update_at(conn: &mut Connection, h: &History, from: usize, to: usize) -> BuildReport {
        with_input(&h.repo, &h.commit(to), |input| {
            update(conn, input, &h.commit(from))
        })
        .unwrap()
    }

    /// The dump of a fresh index built from commit `i` by `build_full`: the oracle.
    fn fresh_dump(h: &History, i: usize) -> Dump {
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, h, i);
        dump_all(&conn)
    }

    /// The paths a parser was asked to parse, sorted.
    fn recording<'a>(
        parsed: &'a RefCell<Vec<String>>,
    ) -> impl Fn(&str, &str, &[CompiledRule]) -> ParsedDocument + 'a {
        move |rel_path, text, rules| {
            parsed.borrow_mut().push(rel_path.to_owned());
            parse_document(rel_path, text, rules)
        }
    }

    fn sorted(parsed: &RefCell<Vec<String>>) -> Vec<String> {
        let mut paths = parsed.borrow().clone();
        paths.sort();
        paths
    }

    /// `update` from commit `from` to `to` with a parser of the given version that records what
    /// it parses.
    fn update_recording(
        conn: &mut Connection,
        h: &History,
        from: usize,
        to: usize,
        version: &str,
    ) -> (BuildReport, Vec<String>) {
        let parsed = RefCell::new(Vec::new());
        let parse = recording(&parsed);
        let parser = Parser {
            version,
            parse: &parse,
        };
        let report = with_input(&h.repo, &h.commit(to), |input| {
            update_with(conn, input, &h.commit(from), &parser)
        })
        .unwrap();
        (report, sorted(&parsed))
    }

    fn count(conn: &Connection, table: &str) -> usize {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap() as usize
    }

    fn doc_paths(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT rel_path FROM document ORDER BY rel_path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn doc_cell(conn: &Connection, rel_path: &str, column: &str) -> Option<String> {
        conn.query_row(
            &format!("SELECT \"{column}\" FROM document WHERE rel_path = ?1"),
            [rel_path],
            |r| r.get(0),
        )
        .unwrap()
    }

    type EdgeSummary = (String, Option<String>, Option<String>);

    /// `(kind, target_key, dst_key)` of a document's edges, by `ord`.
    fn edges(conn: &Connection, src: &str) -> Vec<EdgeSummary> {
        conn.prepare(
            "SELECT kind, target_key, dst_key FROM edge WHERE src_rel_path = ?1 ORDER BY ord",
        )
        .unwrap()
        .query_map([src], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    fn edge(kind: &str, key: &str, dst: Option<&str>) -> EdgeSummary {
        (
            kind.to_owned(),
            Some(key.to_owned()),
            dst.map(str::to_owned),
        )
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// Review Focus 2: a modification, a deletion and a rename between two commits.
    #[test]
    fn update_dumps_equal_to_a_full_rebuild_after_modify_delete_and_rename() {
        let a = "# A\n\n## A-1 — entry\n\nSee [[b]], [[c]], [[d]] and [[b:B-1]].\n";
        let b = "# B\n\n## B-1 — an entry of b\n";
        let c = "# C\n\nThis page is deleted in the second commit and shares nothing with the new one.\n";
        let d =
            "# D\n\nA page that moves to a new name in the second commit. It keeps every byte.\n";
        let x = "# X\n\nCites [[e]] and [[a:A-1]].\n";
        let m0 = "# M\n\nOld text citing [[b]].\n";
        let m1 = "# M, retitled\n\nNew text citing [[a]] and [[b:B-1]] and [[d]].\n";
        let new =
            "# Brand new\n\nAn addition in entirely different words: quartz, lantern, orchard.\n";
        let h = history(&[
            &[
                REPO_TOML,
                ("docs/a.md", a),
                ("docs/b.md", b),
                ("docs/c.md", c),
                ("docs/d.md", d),
                ("docs/x.md", x),
                ("docs/m.md", m0),
            ],
            &[
                REPO_TOML,
                ("docs/a.md", a),
                ("docs/b.md", b),
                ("docs/e.md", d),
                ("docs/x.md", x),
                ("docs/m.md", m1),
                ("docs/new.md", new),
            ],
        ]);

        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let before = dump_all(&conn);
        let report = update_at(&mut conn, &h, 0, 1);
        let rebuilt = fresh_dump(&h, 1);

        assert_eq!(
            dump_all(&conn),
            rebuilt,
            "an update must dump byte-equal to a rebuild at the same commit"
        );
        assert_ne!(before, rebuilt, "the two commits really differ");
        assert_eq!(
            doc_paths(&conn),
            strings(&[
                "docs/a.md",
                "docs/b.md",
                "docs/e.md",
                "docs/m.md",
                "docs/new.md",
                "docs/x.md"
            ])
        );
        assert_eq!(
            report.changes,
            Some(ChangeCounts {
                added: 1,
                modified: 1,
                deleted: 1,
                renamed: 1
            })
        );
        // The unchanged `a.md` and `x.md` got the edges the new tree gives them.
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [
                edge("file", "b", Some("b")),
                edge("unknown_key", "c", None),
                edge("unknown_key", "d", None),
                edge("entry", "b", Some("b")),
            ]
        );
        assert_eq!(
            edges(&conn, "docs/x.md"),
            [edge("file", "e", Some("e")), edge("entry", "a", Some("a"))]
        );
        assert_eq!(count(&conn, "parse_cache"), report.documents);
    }

    /// An update reads the repo name and the key space from the commit it indexes. Every other
    /// fixture here shares `REPO_TOML` and has no key-space file, so none of them could tell an
    /// update that reused the previous build's `checkout.repo_name` or `skipped` rows from one that
    /// reads the new commit. Here the second commit renames the repo and adds a key-space file
    /// that excludes one existing document, and the documents that cite across the change do not
    /// change at all.
    #[test]
    fn update_across_a_changed_repo_name_and_key_space_dumps_equal_to_a_full_rebuild() {
        let a = "# A\n\nCites [[beta/b]], [[b]] and [[c]].\n";
        let b = "# B\n";
        let c = "# C\n";
        let h = history(&[
            &[
                (".codescout/repo.toml", "name = \"alpha\"\n"),
                ("docs/a.md", a),
                ("docs/b.md", b),
                ("contrib/c.md", c),
            ],
            &[
                (".codescout/repo.toml", "name = \"beta\"\n"),
                (".codescout/key-space.toml", "exclude = [\"contrib\"]\n"),
                ("docs/a.md", a),
                ("docs/b.md", b),
                ("contrib/c.md", c),
            ],
        ]);

        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let before = dump_all(&conn);
        // The premises: the first commit is named `alpha`, skips nothing, and `c` is a document.
        assert_eq!(before["checkout"][0][0], "T:alpha");
        assert!(before["skipped"].is_empty(), "{:?}", before["skipped"]);
        assert_eq!(
            doc_paths(&conn),
            strings(&["contrib/c.md", "docs/a.md", "docs/b.md"])
        );
        let before_edges = edges(&conn, "docs/a.md");

        let (_, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);

        // Every document that remains is byte-identical to the first commit's: nothing is parsed,
        // and what changed in `a.md` is only how its citations resolve.
        assert!(parsed.is_empty(), "{parsed:?}");
        let rebuilt = fresh_dump(&h, 1);
        assert_eq!(
            dump_all(&conn),
            rebuilt,
            "an update must dump byte-equal to a rebuild at the same commit"
        );
        assert_ne!(before, rebuilt, "the two commits really differ");
        assert_eq!(rebuilt["checkout"][0][0], "T:beta", "the new repo name");
        assert_eq!(
            rebuilt["skipped"],
            [["T:contrib/c.md", "T:outside_key_space"]],
            "the new key space's skipped row"
        );
        assert_eq!(doc_paths(&conn), strings(&["docs/a.md", "docs/b.md"]));
        assert_eq!(
            before_edges,
            [
                edge("cross_repo", "b", None),
                edge("file", "b", Some("b")),
                edge("file", "c", Some("c"))
            ]
        );
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [
                edge("file", "b", Some("b")),
                edge("file", "b", Some("b")),
                edge("unknown_key", "c", None)
            ],
            "the unchanged `a.md` resolves its citations against the new name and key space"
        );
    }

    /// Review Focus 3, first half.
    #[test]
    fn a_new_document_resolves_an_old_unknown_key_edge() {
        let a = "# A\n\nSee [[later]] and [[later:L-1]].\n";
        let h = history(&[
            &[REPO_TOML, ("docs/a.md", a)],
            &[
                REPO_TOML,
                ("docs/a.md", a),
                ("docs/later.md", "# Later\n\n## L-1 — defined later\n"),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [
                edge("unknown_key", "later", None),
                edge("unknown_key", "later", None)
            ]
        );

        let (_, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);

        assert_eq!(
            parsed,
            strings(&["docs/later.md"]),
            "`a.md` did not change: its edges moved without a reparse"
        );
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [
                edge("file", "later", Some("later")),
                edge("entry", "later", Some("later"))
            ]
        );
        assert_eq!(dump_all(&conn), fresh_dump(&h, 1));
    }

    /// Review Focus 3, second half, and the way back.
    #[test]
    fn a_new_colliding_document_turns_old_file_edges_into_collisions() {
        let a = "# A\n\nSee [[same]].\n";
        let h = history(&[
            &[REPO_TOML, ("docs/a.md", a), ("docs/one/same.md", "# One\n")],
            &[
                REPO_TOML,
                ("docs/a.md", a),
                ("docs/one/same.md", "# One\n"),
                ("docs/two/same.md", "# Two\n"),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [edge("file", "same", Some("same"))]
        );
        assert_eq!(count(&conn, "problem"), 0);

        let (_, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);

        assert_eq!(parsed, strings(&["docs/two/same.md"]));
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [edge("key_collision", "same", None)]
        );
        let problems: Vec<(String, Option<String>, String)> = conn
            .prepare("SELECT kind, \"key\", rel_paths FROM problem")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            problems,
            [(
                "duplicate_key".to_owned(),
                Some("same".to_owned()),
                "[\"docs/one/same.md\",\"docs/two/same.md\"]".to_owned()
            )]
        );
        assert_eq!(dump_all(&conn), fresh_dump(&h, 1));

        // And back: the collision is gone and the edge is a file edge again.
        update_at(&mut conn, &h, 1, 0);
        assert_eq!(
            edges(&conn, "docs/a.md"),
            [edge("file", "same", Some("same"))]
        );
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));
    }

    #[test]
    fn unchanged_files_are_not_reparsed() {
        let b0 = "# B\n\nfirst text\n";
        let b1 = "# B\n\nsecond text, citing [[a]]\n";
        let h = history(&[
            &[
                REPO_TOML,
                ("docs/a.md", "# A\n\n[[b]]\n"),
                ("docs/b.md", b0),
                ("docs/c.md", "# C\n"),
            ],
            &[
                REPO_TOML,
                ("docs/a.md", "# A\n\n[[b]]\n"),
                ("docs/b.md", b1),
                ("docs/c.md", "# C\n"),
                ("docs/n.md", "# N\n"),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);

        let (report, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);
        assert_eq!(parsed, strings(&["docs/b.md", "docs/n.md"]));
        assert_eq!(
            count(&conn, "parse_cache"),
            report.documents,
            "the cache holds one row per document of the last build"
        );

        let (_, parsed) = update_recording(&mut conn, &h, 1, 1, PARSER_VERSION);
        assert!(
            parsed.is_empty(),
            "nothing changed, nothing parsed: {parsed:?}"
        );

        // Going back: `b.md`'s old text is no longer cached, everything else is.
        let (_, parsed) = update_recording(&mut conn, &h, 1, 0, PARSER_VERSION);
        assert_eq!(parsed, strings(&["docs/b.md"]));
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));
    }

    /// `build_full` is the oracle the update is held to, so it must not trust the cache.
    #[test]
    fn build_full_never_reads_the_parse_cache() {
        let h = history(&[&[REPO_TOML, ("docs/a.md", "# A\n")]]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let honest = dump_all(&conn);
        conn.execute(
            "UPDATE parse_cache SET metadata_json = json_set(metadata_json, '$.kind', 'poisoned')",
            [],
        )
        .unwrap();

        // Control: the update reads the poisoned row, so the poison is where a read would look.
        update_at(&mut conn, &h, 0, 0);
        assert_eq!(
            doc_cell(&conn, "docs/a.md", "kind").as_deref(),
            Some("poisoned")
        );
        // A full build parses afresh and replaces the row.
        build_at(&mut conn, &h, 0);
        assert_eq!(dump_all(&conn), honest);
    }

    #[test]
    fn a_changed_rules_blob_invalidates_the_cache() {
        let active = "[[rule]]\nglob = \"docs/**/*.md\"\nkind = \"spec\"\nstatus = \"active\"\n";
        let stale = "[[rule]]\nglob = \"docs/**/*.md\"\nkind = \"spec\"\nstatus = \"stale\"\n";
        let h = history(&[
            &[
                (".codescout/librarian.toml", active),
                ("docs/a.md", "# A\n"),
                ("docs/b.md", "# B\n"),
            ],
            &[
                (".codescout/librarian.toml", stale),
                ("docs/a.md", "# A\n"),
                ("docs/b.md", "# B\n"),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        assert_eq!(
            doc_cell(&conn, "docs/a.md", "status").as_deref(),
            Some("active")
        );

        let (_, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);

        assert_eq!(
            parsed,
            strings(&["docs/a.md", "docs/b.md"]),
            "no document changed, but the rules did"
        );
        assert_eq!(
            doc_cell(&conn, "docs/a.md", "status").as_deref(),
            Some("stale")
        );
        assert_eq!(dump_all(&conn), fresh_dump(&h, 1));

        let (_, parsed) = update_recording(&mut conn, &h, 1, 1, PARSER_VERSION);
        assert!(parsed.is_empty(), "the same rules again hit the cache");
    }

    #[test]
    fn the_rules_hash_covers_every_field_of_every_rule_in_order() {
        let rule = |glob: &str, kind: &str, status: &str, scope: &str, tags: &str| {
            format!(
                "[[rule]]\nglob = \"{glob}\"\nkind = \"{kind}\"\nstatus = \"{status}\"\n\
                 time_scope = \"{scope}\"\ntags = [{tags}]\n"
            )
        };
        let hash = |toml: &str| rules_hash(&load_rules(toml).unwrap());
        let base = rule("docs/**/*.md", "spec", "active", "rolling", "\"a\"");
        let variants = [
            rule("docs/*.md", "spec", "active", "rolling", "\"a\""),
            rule("docs/**/*.md", "plan", "active", "rolling", "\"a\""),
            rule("docs/**/*.md", "spec", "stale", "rolling", "\"a\""),
            rule("docs/**/*.md", "spec", "active", "point", "\"a\""),
            rule("docs/**/*.md", "spec", "active", "rolling", "\"b\""),
            rule("docs/**/*.md", "spec", "active", "rolling", "\"a\", \"b\""),
            String::new(),
        ];
        assert_eq!(hash(&base), hash(&base), "deterministic");
        for variant in &variants {
            assert_ne!(hash(&base), hash(variant), "variant: {variant}");
        }
        // First match wins in `classify`, so the order of the rules is part of the answer.
        let other = rule("notes/**/*.md", "note", "active", "rolling", "");
        assert_ne!(
            hash(&format!("{base}{other}")),
            hash(&format!("{other}{base}"))
        );
    }

    /// The rules a build uses include the compiled-in defaults, so a change to them changes the
    /// hash: it covers both the committed blob and `DEFAULT_RULES_TOML`.
    #[test]
    fn the_rules_hash_covers_the_committed_blob_and_the_built_in_defaults() {
        let h = history(&[
            &[("docs/a.md", "# A\n")],
            &[
                (
                    ".codescout/librarian.toml",
                    "[[rule]]\nglob = \"x/**\"\nkind = \"k\"\n",
                ),
                ("docs/a.md", "# A\n"),
            ],
        ]);
        let of = |i: usize| rules_hash(&rules_from_commit(&h.repo, &h.commit(i)).unwrap());
        assert_ne!(of(0), rules_hash(&[]), "the defaults are in the hash");
        assert_ne!(of(0), of(1), "the committed blob is in the hash");
        assert_eq!(of(0), of(0));
    }

    #[test]
    fn a_rename_reparses_the_file_and_matches_a_rebuild() {
        let rules = "[[rule]]\nglob = \"docs/archive/**/*.md\"\nkind = \"archive\"\n\
                     status = \"archived\"\n";
        let text = format!(
            "# Notes\n\n{}\n\n## Part two\n\n{}\n",
            "alpha ".repeat(60),
            "beta ".repeat(60)
        );
        let h = history(&[
            &[
                (".codescout/librarian.toml", rules),
                ("docs/notes/a.md", &text),
            ],
            &[
                (".codescout/librarian.toml", rules),
                ("docs/archive/a.md", &text),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let chunk_ids = |conn: &Connection| -> Vec<String> {
            conn.prepare("SELECT chunk_id FROM chunk ORDER BY chunk_id")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let old_ids = chunk_ids(&conn);
        assert!(!old_ids.is_empty());

        let (report, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);

        assert_eq!(
            parsed,
            strings(&["docs/archive/a.md"]),
            "same bytes at a new path: classification and chunk ids depend on the path"
        );
        assert_eq!(
            doc_cell(&conn, "docs/archive/a.md", "kind").as_deref(),
            Some("archive")
        );
        let new_ids = chunk_ids(&conn);
        assert_eq!(new_ids.len(), old_ids.len());
        assert!(
            new_ids.iter().all(|id| !old_ids.contains(id)),
            "a chunk id includes the path"
        );
        assert_eq!(dump_all(&conn), fresh_dump(&h, 1));
        assert_eq!(report.changes.map(|c| c.renamed), Some(1));
    }

    #[test]
    fn the_parser_version_is_part_of_the_cache_key() {
        let h = history(&[&[REPO_TOML, ("docs/a.md", "# A\n"), ("docs/b.md", "# B\n")]]);
        let all = strings(&["docs/a.md", "docs/b.md"]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);

        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, PARSER_VERSION);
        assert!(parsed.is_empty(), "control: the same version hits");

        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, "a later version");
        assert_eq!(parsed, all, "another parser version reads no cached row");
        let versions: Vec<String> = conn
            .prepare("SELECT DISTINCT parser_version FROM parse_cache")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(versions, ["a later version"], "and the old rows are gone");

        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, "a later version");
        assert!(parsed.is_empty());

        // The production parser has `PARSER_VERSION`, so it misses the other version's rows.
        let parsed_by_production = RefCell::new(Vec::new());
        let parse = recording(&parsed_by_production);
        let parser = Parser {
            version: PARSER_VERSION,
            parse: &parse,
        };
        with_input(&h.repo, &h.commit(0), |input| {
            update_with(&mut conn, input, &h.commit(0), &parser)
        })
        .unwrap();
        assert_eq!(sorted(&parsed_by_production), all);
    }

    #[test]
    fn update_from_an_unrelated_commit_still_equals_a_rebuild() {
        let mut h = history(&[&[
            REPO_TOML,
            ("docs/a.md", "# A\n\n[[b]] [[only-here]]\n"),
            ("docs/b.md", "# B\n\n## B-1 — one\n"),
            ("docs/gone.md", "# Gone\n"),
        ]]);
        let unrelated = h.add_root(&[
            REPO_TOML,
            ("docs/a.md", "# A, otherwise\n\n[[b:B-1]] [[only-here]]\n"),
            ("docs/b.md", "# B\n\n## B-1 — one\n"),
            ("docs/only-here.md", "# Only here\n"),
        ]);
        let descends = |a: usize, b: usize| h.repo.graph_descendant_of(h.ids[a], h.ids[b]).unwrap();
        assert!(
            !descends(unrelated, 0) && !descends(0, unrelated),
            "neither commit is an ancestor of the other"
        );

        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        update_at(&mut conn, &h, 0, unrelated);
        assert_eq!(dump_all(&conn), fresh_dump(&h, unrelated));
        update_at(&mut conn, &h, unrelated, 0);
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));

        // From the same commit, and into an index that was never built.
        update_at(&mut conn, &h, 0, 0);
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));
        let mut empty = open_in_memory().unwrap();
        update_at(&mut empty, &h, unrelated, 0);
        assert_eq!(dump_all(&empty), fresh_dump(&h, 0));
    }

    #[test]
    fn correctness_does_not_depend_on_the_diff() {
        // `from_commit` lives in another repository, so the diff cannot be taken. The update
        // still succeeds and equals a rebuild; only the counts are missing.
        let h = history(&[&[
            REPO_TOML,
            ("docs/a.md", "# A\n\n[[b]]\n"),
            ("docs/b.md", "# B\n"),
        ]]);
        let other = history(&[&[("docs/z.md", "# Z\n")]]);
        let mut conn = open_in_memory().unwrap();
        let report = with_input(&h.repo, &h.commit(0), |input| {
            update(&mut conn, input, &other.commit(0))
        })
        .unwrap();
        assert_eq!(report.changes, None);
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));
    }

    /// The counts depend on the two trees alone. libgit2 reads a repo's own `diff.renameLimit`
    /// when the caller sets no limit (it then compares only that many deleted files with each
    /// added one), and `diff.renames` when the caller sets no rename flag; neither may turn
    /// renames into additions and deletions.
    #[test]
    fn the_change_counts_do_not_depend_on_the_repos_git_config() {
        let text = |i: usize| {
            format!(
                "# Doc {i}\n\n{}\n",
                format!("words that are only in doc {i} ").repeat(20)
            )
        };
        let mut got = Vec::new();
        let mut expected = Vec::new();
        for n in [1_usize, 2, 3, 6] {
            let old: Vec<(String, String)> = (0..n)
                .map(|i| (format!("docs/old-{i}.md"), text(i)))
                .collect();
            let new: Vec<(String, String)> = (0..n)
                .map(|i| (format!("docs/new-{i}.md"), text(i)))
                .collect();
            let old_files: Vec<(&str, &str)> =
                old.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
            let new_files: Vec<(&str, &str)> =
                new.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
            let h = history(&[&old_files, &new_files]);
            let mut config = h.repo.config().unwrap();
            config.set_i32("diff.renameLimit", 1).unwrap();
            config.set_str("diff.renames", "false").unwrap();
            assert_eq!(config.get_i32("diff.renameLimit").unwrap(), 1);

            let mut conn = open_in_memory().unwrap();
            got.push((n, update_at(&mut conn, &h, 0, 1).changes));
            expected.push((
                n,
                Some(ChangeCounts {
                    added: 0,
                    modified: 0,
                    deleted: 0,
                    renamed: n,
                }),
            ));
        }
        assert_eq!(got, expected, "n renames, each with unchanged bytes");
    }

    #[test]
    fn the_parse_cache_is_excluded_from_the_dump() {
        let h = history(&[&[REPO_TOML, ("docs/a.md", "# A\n"), ("docs/b.md", "# B\n")]]);
        let mut with_cache = open_in_memory().unwrap();
        build_at(&mut with_cache, &h, 0);
        assert_eq!(
            count(&with_cache, "parse_cache"),
            2,
            "the cache is populated"
        );
        let dump = dump_all(&with_cache);
        assert!(!dump.contains_key("parse_cache"));

        // Two indexes that differ in the cache alone dump equal.
        let mut without_cache = open_in_memory().unwrap();
        build_at(&mut without_cache, &h, 0);
        without_cache
            .execute("DELETE FROM parse_cache", [])
            .unwrap();
        assert_eq!(dump, dump_all(&without_cache));
    }

    #[test]
    fn a_cache_row_that_does_not_read_back_is_a_miss_not_an_error() {
        let h = history(&[&[
            REPO_TOML,
            ("docs/a.md", "# A\n"),
            ("docs/b.md", "# B\n"),
            ("docs/c.md", "# C\n"),
        ]]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        // `a.md`: not JSON at all. `b.md`: JSON that reads, but names a key that is no key.
        conn.execute(
            "UPDATE parse_cache SET entries_json = 'not json' WHERE rel_path = 'docs/a.md'",
            [],
        )
        .unwrap();
        let bad_key = serde_json::to_string(&vec![CitationJson {
            reference: ReferenceJson::File {
                repo: None,
                key: "Not A Key".to_owned(),
            },
            line: 1,
        }])
        .unwrap();
        conn.execute(
            "UPDATE parse_cache SET citations_json = ?1 WHERE rel_path = 'docs/b.md'",
            [bad_key],
        )
        .unwrap();

        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, PARSER_VERSION);

        assert_eq!(parsed, strings(&["docs/a.md", "docs/b.md"]));
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));
        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, PARSER_VERSION);
        assert!(parsed.is_empty(), "the bad rows were replaced by good ones");
    }

    /// A row written before a fact existed has no field for it: `{}` is what `FactsJson` was before
    /// `events`. It must not read back as "no events" (E2-R3: the tasks that add a fact do not
    /// bump `PARSER_VERSION`, so a miss is all that keeps an old test database honest).
    // mutation M4: `decode` ignores `facts_json` (reads no events from it, and no error).
    #[test]
    fn a_cache_row_without_the_events_fact_is_a_miss() {
        let notes = notes_document(&[sample_event(
            "n-1",
            "note",
            serde_json::json!({"text": "kept"}),
        )]);
        let h = history(&[&[REPO_TOML, ("docs/a.md", "# A\n"), ("docs/notes.md", &notes)]]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        assert_eq!(
            conn.query_row("SELECT count(*) FROM event", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1,
            "the fixture's build stores its event"
        );
        let stored: Vec<String> = conn
            .prepare("SELECT facts_json FROM parse_cache ORDER BY rel_path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            stored.iter().any(|f| f.contains("n-1")),
            "the notes document's facts hold its event: {stored:?}"
        );
        conn.execute(
            "UPDATE parse_cache SET facts_json = '{}' WHERE rel_path = 'docs/notes.md'",
            [],
        )
        .unwrap();

        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, PARSER_VERSION);

        assert_eq!(parsed, strings(&["docs/notes.md"]));
        assert_eq!(dump_all(&conn), fresh_dump(&h, 0));
    }

    /// Every key of a cached event is required, the nullable ones included: a row with one key
    /// removed does not read back as a row with that field `None`, it is a miss (the rule on
    /// `FactsJson`: a field added later is required, or `PARSER_VERSION` is bumped). An explicit
    /// `null` is a value and stays a hit.
    // mutation: plain `Option` fields in `EventJson` (no `deserialize_with = "required"`).
    #[test]
    fn a_cache_row_with_an_event_key_removed_is_a_miss() {
        let notes = notes_document(&[full_event("n-1")]);
        let h = history(&[&[REPO_TOML, ("docs/a.md", "# A\n"), ("docs/notes.md", &notes)]]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let facts = |conn: &Connection| -> serde_json::Value {
            let text: String = conn
                .query_row(
                    "SELECT facts_json FROM parse_cache WHERE rel_path = 'docs/notes.md'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            serde_json::from_str(&text).unwrap()
        };
        let whole = facts(&conn);
        let keys: Vec<String> = whole["events"][0]
            .as_object()
            .expect("the cached event is an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys.len(), 13, "every field of an event is a key: {keys:?}");
        for key in &keys {
            let mut cut = whole.clone();
            cut["events"][0].as_object_mut().unwrap().remove(key);
            conn.execute(
                "UPDATE parse_cache SET facts_json = ?1 WHERE rel_path = 'docs/notes.md'",
                [cut.to_string()],
            )
            .unwrap();

            let (_, parsed) = update_recording(&mut conn, &h, 0, 0, PARSER_VERSION);

            assert_eq!(parsed, strings(&["docs/notes.md"]), "without `{key}`");
            assert_eq!(dump_all(&conn), fresh_dump(&h, 0), "without `{key}`");
            assert_eq!(facts(&conn), whole, "the miss rewrote the row");
        }
        // An explicit null is a value, not a missing key.
        let mut nulled = whole.clone();
        nulled["events"][0]["parent"] = serde_json::Value::Null;
        conn.execute(
            "UPDATE parse_cache SET facts_json = ?1 WHERE rel_path = 'docs/notes.md'",
            [nulled.to_string()],
        )
        .unwrap();
        let (_, parsed) = update_recording(&mut conn, &h, 0, 0, PARSER_VERSION);
        assert!(parsed.is_empty(), "a null `parent` reads back: {parsed:?}");
    }

    /// A document that holds `events` (id, kind, payload) as committed `###` sections under its
    /// `## Events` heading.
    fn notes_document(events: &[EventText]) -> String {
        let mut doc = "# Notes\n\n## Events\n\n".to_owned();
        for event in events {
            doc.push_str(&render_event_section(event));
        }
        doc
    }

    /// A note-shaped event with the required fields only.
    fn sample_event(id: &str, kind: &str, payload: serde_json::Value) -> EventText {
        EventText {
            id: id.to_owned(),
            kind: kind.to_owned(),
            created_at_ms: 1_794_637_800_000,
            author: Some("alice".to_owned()),
            anchor_commit: None,
            head_commit: None,
            parent: None,
            resolves: None,
            source: None,
            mutates: Vec::new(),
            payload,
        }
    }

    /// An event with every optional field set: a source with a payload, two `mutates` forms, and
    /// the parent, resolves, anchor and head commits. A cache round trip that drops one of them is
    /// seen only through this event.
    fn full_event(id: &str) -> EventText {
        EventText {
            anchor_commit: Some("a1b2".to_owned()),
            head_commit: Some("c3d4".to_owned()),
            parent: Some("p-parent".to_owned()),
            resolves: Some("r-resolved".to_owned()),
            source: Some(EventSource {
                kind: "url".to_owned(),
                uri: "https://example.invalid/x".to_owned(),
                payload: Some(serde_json::json!({"k": [1, 2], "z": null})),
            }),
            mutates: vec!["[[b]]".to_owned(), "[[b:F-1]]".to_owned()],
            ..sample_event(id, "note", serde_json::json!({"text": "full"}))
        }
    }

    /// A parse corpus that exercises every part of a `ParsedDocument`.
    fn corpus() -> Vec<(&'static str, String)> {
        let long = format!(
            "# Long\n\n{}\n\n## S2\n\n{}\n\n## F-9 — an entry\n\n{}\n",
            "alpha ".repeat(300),
            "beta ".repeat(300),
            "gamma ".repeat(40)
        );
        vec![
            (
                "docs/specs/one.md",
                "---\nkind: spec\ntitle: One\nowners: [zed, amy]\ntags: [b, a]\ntopic: t\n\
                 time_scope: rolling\n---\n# Heading\n\n## F-1 — an entry\n\nSee [[two]], \
                 [[repo-x/three:F-2]], [[repo-x/four]], [[Bad Key!]], [[five:F-3]] and F-1.\n\n\
                 ```\n[[in-a-fence]]\n```\n"
                    .to_owned(),
            ),
            (
                "docs/broken.md",
                "---\nkind: [unterminated\n---\n# Broken title\n\ntext [[one]]\n".to_owned(),
            ),
            ("notes.md", "no heading\n".to_owned()),
            ("docs/specs/long.md", long),
            ("empty.md", String::new()),
            // One entry whose section is longer than the chunk grain (2048 characters), so the
            // parse cuts it into several chunks numbered `entry_part` of `entry_parts`.
            (
                "docs/specs/entry.md",
                format!(
                    "# Entries\n\nintro\n\n## F-7 — a long entry\n\n{}\n",
                    "delta ".repeat(500)
                ),
            ),
            // Inline code is an escape: the wiki link and the bare token inside the span are not
            // citations, the two outside it are.
            (
                "docs/specs/inline.md",
                "# Inline\n\nWrite `[[in-code]]` and `F-4` as code, then cite [[outside]] and F-5.\n"
                    .to_owned(),
            ),
            // CRLF line ends: the lines still count from one, and the entry's section spans them.
            (
                "docs/specs/crlf.md",
                "# Crlf\r\n\r\n## C-1 — a crlf entry\r\n\r\nSee [[two]] and C-1.\r\n\r\nlast line\r\n"
                    .to_owned(),
            ),
            // A `###` entry inside a `##` entry: the child's section lies inside its parent's, and
            // a sibling `##` entry closes both.
            (
                "docs/specs/nested.md",
                "# Nested\n\n## N-1 — parent\n\nparent text citing [[two]]\n\n\
                 ### N-2 — child\n\nchild text citing [[two:F-2]]\n\n## N-3 — sibling\n\nafter\n"
                    .to_owned(),
            ),
            // A bare token that another document defines, with no wiki link: the parse records it
            // as it records any bare token, and resolving it is the build's work, not the parse's.
            (
                "docs/specs/bare.md",
                "# Bare\n\nThe decision in R-43 settles this, and so does R-44.\n".to_owned(),
            ),
            // Event sections under `## Events`, one per payload form: a note's blockquote and a
            // verdict's fenced payload. The parse stores them as events, with whole-document lines.
            (
                "docs/notes.md",
                notes_document(&[
                    sample_event("n-1", "note", serde_json::json!({"text": "first\nsecond"})),
                    sample_event(
                        "v-1",
                        "verdict",
                        serde_json::json!({"outcome": "go", "score": 1.5}),
                    ),
                    full_event("f-1"),
                ]),
            ),
        ]
    }

    fn corpus_rules() -> Vec<CompiledRule> {
        load_rules(
            "[[rule]]\nglob = \"docs/specs/**/*.md\"\nkind = \"spec\"\nstatus = \"active\"\n\
             tags = [\"rule-tag\", \"b\"]\n",
        )
        .unwrap()
    }

    #[test]
    fn a_cache_row_reads_back_as_the_parse_that_wrote_it() {
        let rules = corpus_rules();
        for (rel_path, text) in corpus() {
            let parsed = parse_document(rel_path, &text, &rules);
            let stored = encode(&parsed).unwrap();
            assert_eq!(
                decode(rel_path, &stored).as_ref(),
                Some(&parsed),
                "{rel_path}"
            );
        }
    }

    /// The corpus must keep an entry longer than the chunk grain: a change to how `build_chunks`
    /// numbers the parts of an entry changes the parse only if some pinned text has such an entry.
    /// If this fails the corpus was edited so that no entry spans chunks; restore one, and the
    /// digest below keeps covering `entry_part` and `entry_parts`.
    #[test]
    fn the_pinned_corpus_has_an_entry_that_spans_several_chunks() {
        let rules = corpus_rules();
        let (rel_path, text) = corpus()
            .into_iter()
            .find(|(rel_path, _)| *rel_path == "docs/specs/entry.md")
            .unwrap();
        let parsed = parse_document(rel_path, &text, &rules);
        let parts: Vec<(Option<usize>, Option<usize>)> = parsed
            .chunks
            .iter()
            .filter(|c| c.entry_token.as_deref() == Some("F-7"))
            .map(|c| (c.entry_part, c.entry_parts))
            .collect();
        let n = parts.len();
        assert!(
            n >= 2,
            "the F-7 section must be cut into several chunks: {parts:?}"
        );
        assert_eq!(
            parts,
            (1..=n).map(|i| (Some(i), Some(n))).collect::<Vec<_>>(),
            "parts are numbered 1..=n of n"
        );
    }

    /// The corpus must keep event sections of both payload forms, and one event with every optional
    /// field set: the digest below and the round-trip test cover `events` only if some pinned text
    /// has them.
    #[test]
    fn the_pinned_corpus_has_events_of_both_payload_forms() {
        let rules = corpus_rules();
        let (rel_path, text) = corpus()
            .into_iter()
            .find(|(rel_path, _)| *rel_path == "docs/notes.md")
            .unwrap();
        let parsed = parse_document(rel_path, &text, &rules);
        let payloads: Vec<&str> = parsed
            .events
            .iter()
            .map(|e| e.payload_json.as_str())
            .collect();
        assert_eq!(
            payloads,
            [
                r#"{"text":"first\nsecond"}"#,
                r#"{"outcome":"go","score":1.5}"#,
                r#"{"text":"full"}"#
            ]
        );
        let full = &parsed.events[2];
        assert_eq!(full.mutates, ["[[b]]", "[[b:F-1]]"]);
        for (name, field) in [
            ("author", &full.author),
            ("anchor_commit", &full.anchor_commit),
            ("head_commit", &full.head_commit),
            ("parent", &full.parent),
            ("resolves", &full.resolves),
            ("source_json", &full.source_json),
        ] {
            assert!(field.is_some(), "the corpus event leaves `{name}` empty");
        }
    }

    /// The pinned digest of the corpus's parse. When this fails, the parse of some text changed
    /// (or the shape the cache stores did): bump `PARSER_VERSION`, so that no cached row from
    /// the old logic is read, and then set this to the new digest.
    const CORPUS_DIGEST: &str = "f09b66f6dc76c48924e830c88d54ea48fa070ae2bd045ea04d882fc0d12c713b";

    #[test]
    fn the_parse_of_a_pinned_corpus_is_tied_to_parser_version() {
        let rules = corpus_rules();
        let mut hasher = Sha256::new();
        hasher.update(PARSER_VERSION.as_bytes());
        for (rel_path, text) in corpus() {
            let stored = encode(&parse_document(rel_path, &text, &rules)).unwrap();
            for part in [
                rel_path,
                stored.parse_note.as_deref().unwrap_or("<none>"),
                &stored.metadata_json,
                &stored.entries_json,
                &stored.citations_json,
                &stored.chunks_json,
                &stored.facts_json,
            ] {
                hasher.update(b"\n");
                hasher.update(part.as_bytes());
            }
        }
        let digest = format!("{:x}", hasher.finalize());
        assert_eq!(
            digest, CORPUS_DIGEST,
            "the parse changed: bump PARSER_VERSION ({PARSER_VERSION:?}) and pin this digest"
        );
    }

    /// The row set of `parse_cache`, key columns only.
    fn cache_keys(conn: &Connection) -> Vec<Vec<String>> {
        conn.prepare(
            "SELECT sha256, rel_path, rules_hash, parser_version FROM parse_cache \
             ORDER BY sha256, rel_path",
        )
        .unwrap()
        .query_map([], |r| (0..4).map(|i| r.get::<_, String>(i)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    #[test]
    fn a_failed_update_leaves_the_derived_tables_and_the_cache_as_they_were() {
        let h = history(&[
            &[
                REPO_TOML,
                ("docs/a.md", "# A\n\n[[b]]\n"),
                ("docs/b.md", "# B\n"),
            ],
            &[
                REPO_TOML,
                ("docs/a.md", "# A\n\n[[b]]\n"),
                ("docs/b.md", "# B, changed\n"),
                ("docs/n.md", "# N\n"),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let (dump, keys) = (dump_all(&conn), cache_keys(&conn));

        // The last write of the transaction fails: everything before it must roll back.
        conn.execute_batch(
            "CREATE TEMP TRIGGER boom BEFORE INSERT ON parse_cache \
             BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();
        let err = with_input(&h.repo, &h.commit(1), |input| {
            update(&mut conn, input, &h.commit(0))
        })
        .unwrap_err();

        assert!(format!("{err:#}").contains("boom"), "{err:#}");
        assert_eq!(dump_all(&conn), dump, "the derived tables are as they were");
        assert_eq!(cache_keys(&conn), keys, "so is the cache");
    }

    /// R15: the write lock is held for the deletes and inserts alone. A second connection asks
    /// for the write lock at every parse; it is never refused.
    #[test]
    fn no_parse_runs_while_the_write_transaction_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("derived.db");
        let mut conn = open(&path).unwrap();
        let probe = Connection::open(&path).unwrap();
        probe.busy_timeout(Duration::ZERO).unwrap();

        let h = history(&[
            &[REPO_TOML, ("docs/a.md", "# A\n"), ("docs/b.md", "# B\n")],
            &[
                REPO_TOML,
                ("docs/a.md", "# A, changed\n"),
                ("docs/b.md", "# B\n"),
            ],
        ]);
        let refused = RefCell::new(Vec::<String>::new());
        let parsed = RefCell::new(Vec::<String>::new());
        let parse = |rel_path: &str, text: &str, rules: &[CompiledRule]| {
            parsed.borrow_mut().push(rel_path.to_owned());
            if let Err(e) = probe.execute_batch("BEGIN IMMEDIATE; COMMIT;") {
                refused.borrow_mut().push(format!("{rel_path}: {e}"));
            }
            parse_document(rel_path, text, rules)
        };
        let parser = Parser {
            version: PARSER_VERSION,
            parse: &parse,
        };
        with_input(&h.repo, &h.commit(0), |input| {
            rebuild(&mut conn, input, &parser, false, None)
        })
        .unwrap();
        with_input(&h.repo, &h.commit(1), |input| {
            update_with(&mut conn, input, &h.commit(0), &parser)
        })
        .unwrap();

        assert_eq!(
            parsed.borrow().len(),
            3,
            "two in the full build, one in the update"
        );
        assert!(
            refused.borrow().is_empty(),
            "a parse ran while the write lock was held: {:?}",
            refused.borrow()
        );
    }

    /// R14: an entry cited through a folder index resolves to the owned document that defines
    /// it, and that document's key is stored; an update and a rebuild agree on it.
    #[test]
    fn an_entry_edge_through_a_folder_index_stores_the_destination_key_in_update_and_rebuild() {
        let a = "# A\n\nSee [[readme:F-1]], [[readme]] and [[readme:Z-9]].\n";
        let h = history(&[
            &[
                REPO_TOML,
                ("docs/a.md", a),
                ("docs/readme/README.md", "# Readme\n"),
                ("docs/readme/f1-log.md", "# Log\n\n## F-1 — defined here\n"),
            ],
            &[
                REPO_TOML,
                ("docs/a.md", a),
                ("docs/readme/README.md", "# Readme\n"),
                ("docs/readme/f1-log.md", "# Log, F-1 moved out\n"),
                ("docs/readme/g-log.md", "# G\n\n## F-1 — defined here now\n"),
            ],
        ]);
        let mut conn = open_in_memory().unwrap();
        build_at(&mut conn, &h, 0);
        let expected = |dst: &str| {
            [
                edge("entry", "readme", Some(dst)),
                edge("file", "readme", Some("readme")),
                edge("undefined_token", "readme", None),
            ]
        };
        assert_eq!(edges(&conn, "docs/a.md"), expected("f1-log"));

        let (_, parsed) = update_recording(&mut conn, &h, 0, 1, PARSER_VERSION);

        assert_eq!(
            parsed,
            strings(&["docs/readme/f1-log.md", "docs/readme/g-log.md"])
        );
        assert_eq!(edges(&conn, "docs/a.md"), expected("g-log"));
        assert_eq!(dump_all(&conn), fresh_dump(&h, 1));
    }
}
