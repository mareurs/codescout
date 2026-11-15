//! The export plan: every change the cutover would make, composed in memory.
//!
//! [`plan_export`] is a function of (`HEAD`, the old rows): it reads committed blobs through
//! `git2` and the rows [`OldRows`] already holds, and writes nothing. `cutover plan` prints it;
//! `cutover export` applies it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::librarian::catalog::augmentation::AugmentationRow;
use crate::librarian::cutover::code_refs::CodeRefs;
use crate::librarian::cutover::history::HistoryIds;
use crate::librarian::cutover::old_rows::{OldRows, OutOfScope};
use crate::librarian::cutover::rewrite::{
    rewrite_document, Finding, Form, IdSource, IdTarget, Outcome, Resolver, REASON_UNKNOWN_ID,
};
use crate::librarian::derived::build::{key_space_from_commit, repo_name_of};
use crate::librarian::derived::scan::{scan_commit, Skip};
use crate::librarian::identity::index::{DocInput, KeyIndex};
use crate::librarian::identity::key::markdown_stem;
use crate::librarian::identity::repo_name::RepoName;
use crate::librarian::tools::link_scan::extract::{citation_spans, CitationKind};

/// What kind of old fact an item is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    ParamsEntry,
    ParamsOther,
    AugmentationShape,
    Event,
    Observation,
    ManualLink,
    WriteCite,
    Rename,
    Citation,
}

impl Class {
    /// The name the report and the JSON use.
    pub const fn label(self) -> &'static str {
        match self {
            Class::ParamsEntry => "params_entry",
            Class::ParamsOther => "params_other",
            Class::AugmentationShape => "augmentation_shape",
            Class::Event => "event",
            Class::Observation => "observation",
            Class::ManualLink => "manual_link",
            Class::WriteCite => "write_cite",
            Class::Rename => "rename",
            Class::Citation => "citation",
        }
    }
}

/// What the plan does with one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Moved,
    AlreadyExported,
    Skipped,
    Unrecoverable,
}

/// One line of the report: a fact, what happens to it, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub class: Class,
    pub status: Status,
    pub reason: String,
    /// The repository-relative file the item is about.
    pub path: Option<String>,
    pub line: Option<u32>,
    pub detail: Option<String>,
    pub suggestion: Option<String>,
}

/// The planned text of one file.
#[derive(Debug, Clone)]
pub struct FileChange {
    /// The final path.
    pub path: String,
    pub renamed_from: Option<String>,
    /// The text in `HEAD`; `None` for a new file.
    pub head_text: Option<String>,
    pub new_text: String,
}

/// How the items of one class ended.
#[derive(Debug, Default, Serialize)]
pub struct ClassTally {
    pub moved: usize,
    pub already_exported: usize,
    pub skipped: usize,
    pub unrecoverable: usize,
    /// Items per reason.
    pub reasons: BTreeMap<String, usize>,
}

/// Every change of every file, composed in memory.
pub struct ExportPlan {
    pub repo_name: String,
    /// The full id of the commit planned.
    pub commit: String,
    pub shallow: bool,
    /// Old rows in scope by class (`event:<kind>` per event kind); a class with no row is absent.
    pub population: BTreeMap<String, usize>,
    pub items: Vec<Item>,
    /// The planned files, by final path.
    pub files: BTreeMap<String, FileChange>,
    pub out_of_scope: OutOfScope,
}

impl ExportPlan {
    /// The items of each class that has one, counted by status and reason.
    pub fn tally(&self) -> BTreeMap<Class, ClassTally> {
        let mut tally: BTreeMap<Class, ClassTally> = BTreeMap::new();
        for item in &self.items {
            let cell = tally.entry(item.class).or_default();
            match item.status {
                Status::Moved => cell.moved += 1,
                Status::AlreadyExported => cell.already_exported += 1,
                Status::Skipped => cell.skipped += 1,
                Status::Unrecoverable => cell.unrecoverable += 1,
            }
            *cell.reasons.entry(item.reason.clone()).or_default() += 1;
        }
        tally
    }

    /// Files whose new text differs from `HEAD`, plus renames: what `export` would write.
    pub fn writes(&self) -> usize {
        self.files
            .values()
            .filter(|f| f.renamed_from.is_some() || f.head_text.as_deref() != Some(&f.new_text))
            .count()
    }
}

/// The committed files of `HEAD` that the later steps of the plan read: the text of every
/// markdown file in the key space, the paths the key space leaves out, and the repository name.
struct Head {
    files: BTreeMap<String, String>,
    /// Read by the rename step, which asks which paths are outside the key space.
    #[allow(dead_code)]
    outside: BTreeSet<String>,
    repo_name: RepoName,
}

impl Head {
    /// Read `commit` once: its key space, every markdown blob under it, and its repo name.
    fn load(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<Head> {
        let space = key_space_from_commit(repo, commit)?;
        let mut files = BTreeMap::new();
        let mut outside = BTreeSet::new();
        for scanned in scan_commit(repo, commit, &space)? {
            match scanned.text {
                Ok(text) => {
                    files.insert(scanned.rel_path, text);
                }
                Err(Skip::OutsideKeySpace) => {
                    outside.insert(scanned.rel_path);
                }
                // A blob with no text or a symlink has no text to plan against.
                Err(Skip::NotUtf8 | Skip::Symlink) => {}
            }
        }
        let (repo_name, _) = repo_name_of(repo, commit)?;
        Ok(Head {
            files,
            outside,
            repo_name,
        })
    }
}

/// Plan the export of `commit` for the rows in `old`.
///
/// Fills the population of the old rows by class, and the items for the events whose kind has
/// no home in any text and for the entry citations that are reported and not exported.
pub fn plan_export(
    repo: &git2::Repository,
    commit: &git2::Commit,
    old: &OldRows,
) -> anyhow::Result<ExportPlan> {
    let head = Head::load(repo, commit)?;
    let mut plan = ExportPlan {
        repo_name: head.repo_name.to_string(),
        commit: commit.id().to_string(),
        shallow: repo.is_shallow(),
        population: population_of(old),
        items: Vec::new(),
        files: BTreeMap::new(),
        out_of_scope: old.out_of_scope.clone(),
    };
    plan_skipped_events(old, &mut plan);
    plan_write_cites(old, &mut plan);
    plan_citations(repo, commit, old, &head, &mut plan)?;
    Ok(plan)
}

/// Rewrite the old citation forms of every in-space document (D-R5). A document that code,
/// a dot directory or an mdBook reads by path is frozen (D-R26): its text stays, and each
/// rewrite it would have had is an `unrecoverable` item.
fn plan_citations(
    repo: &git2::Repository,
    commit: &git2::Commit,
    old: &OldRows,
    head: &Head,
    plan: &mut ExportPlan,
) -> anyhow::Result<()> {
    let index = KeyIndex::build(
        head.repo_name.clone(),
        head.files.iter().map(|(path, text)| DocInput {
            rel_path: path.clone(),
            text: text.clone(),
        }),
    );
    let stems = stems_of(head);
    let (ids, id_hints) = recover_ids(repo, commit, old, head, plan.shallow)?;
    let renamed_keys = BTreeMap::new();
    let resolver = Resolver {
        index: &index,
        this_repo: &head.repo_name,
        renamed_keys: &renamed_keys,
        stems: &stems,
        ids: &ids,
        id_hints: &id_hints,
    };
    let code = CodeRefs::load(repo, commit)?;
    for (path, text) in &head.files {
        let frozen = code.frozen(path);
        let (new_text, findings) = rewrite_document(path, text, &resolver);
        for finding in &findings {
            plan.items.push(citation_item(
                path,
                finding,
                frozen.as_deref(),
                plan.shallow,
            ));
        }
        if frozen.is_none() && new_text != *text {
            plan.files.insert(
                path.clone(),
                FileChange {
                    path: path.clone(),
                    renamed_from: None,
                    head_text: Some(text.clone()),
                    new_text,
                },
            );
        }
    }
    Ok(())
}

/// The plan item of one citation the rewrite looked at.
fn citation_item(path: &str, finding: &Finding, frozen: Option<&str>, shallow: bool) -> Item {
    let item = |status, reason: &str, detail: String, suggestion| Item {
        class: Class::Citation,
        status,
        reason: reason.to_owned(),
        path: Some(path.to_owned()),
        line: Some(finding.line),
        detail: Some(detail),
        suggestion,
    };
    let raw = finding.raw.clone();
    match &finding.outcome {
        Outcome::Rewritten { to } => {
            let detail = format!("{raw} -> {to}");
            match frozen {
                Some(why) => item(
                    Status::Unrecoverable,
                    &format!("frozen: {why}"),
                    detail,
                    None,
                ),
                None => item(
                    Status::Moved,
                    &format!("rewritten: {}", finding.form.label()),
                    detail,
                    None,
                ),
            }
        }
        Outcome::Unresolved { reason, suggestion } => {
            // Without history, an id nothing else names is open for that reason.
            let reason = if shallow && finding.form == Form::HexId && reason == REASON_UNKNOWN_ID {
                "shallow clone: history is incomplete"
            } else {
                reason
            };
            item(Status::Unrecoverable, reason, raw, suggestion.clone())
        }
        Outcome::Ambiguous { candidates } => item(
            Status::Unrecoverable,
            "ambiguous: several documents qualify",
            format!("{raw}: {}", candidates.join(", ")),
            None,
        ),
        Outcome::InCode => item(Status::Skipped, "inside inline code", raw, None),
    }
}

/// Exact file stem of each in-space document to the paths that carry it.
fn stems_of(head: &Head) -> BTreeMap<String, Vec<String>> {
    let mut stems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in head.files.keys() {
        let name = path.rsplit('/').next().unwrap_or(path);
        if let Some(stem) = markdown_stem(name) {
            stems.entry(stem.to_owned()).or_default().push(path.clone());
        }
    }
    stems
}

/// What each old artifact id names. The old catalog first (`Catalog`): an id of a document that
/// `HEAD` holds is its target. Then, for every cited id with no target yet, git history
/// (`History`): the file may have moved since the catalog saw it. Last, an id of a document `HEAD`
/// lacks that history did not settle is a hint with the path the catalog knew. A shallow clone
/// has incomplete history and skips it: an id nothing else names is reported so.
fn recover_ids(
    repo: &git2::Repository,
    commit: &git2::Commit,
    old: &OldRows,
    head: &Head,
    shallow: bool,
) -> anyhow::Result<(BTreeMap<String, IdTarget>, BTreeMap<String, String>)> {
    let mut ids = BTreeMap::new();
    let mut hints = BTreeMap::new();
    // The path the catalog knew for each id whose file `HEAD` lacks.
    let mut gone: BTreeMap<&str, &str> = BTreeMap::new();
    for doc in old.documents.values() {
        if head.files.contains_key(&doc.rel_path) {
            ids.entry(doc.id.clone()).or_insert(IdTarget {
                rel_path: doc.rel_path.clone(),
                source: IdSource::Catalog,
            });
        } else {
            gone.entry(doc.id.as_str()).or_insert(doc.rel_path.as_str());
        }
    }
    // An id with no target in `HEAD` is still open, whether or not the catalog knew its file:
    // git may show where the file went. The id binds to the path it hashes and to nothing else.
    let open: BTreeSet<String> = cited_ids(head)
        .into_iter()
        .filter(|id| !ids.contains_key(id))
        .collect();
    if !shallow && !open.is_empty() {
        let spellings: Vec<&Path> = old.spellings.iter().map(PathBuf::as_path).collect();
        let history = HistoryIds::load(repo, commit, &spellings)?;
        for id in &open {
            match history.head_path(id) {
                Some(Ok(rel_path)) => {
                    ids.insert(
                        id.clone(),
                        IdTarget {
                            rel_path,
                            source: IdSource::History,
                        },
                    );
                }
                Some(Err(historical)) => {
                    hints.insert(id.clone(), historical);
                }
                None => {}
            }
        }
    }
    // What history did not settle falls back to the path the catalog knew.
    for (id, path) in gone {
        if !ids.contains_key(id) && !hints.contains_key(id) {
            hints.insert(id.to_owned(), path.to_owned());
        }
    }
    Ok((ids, hints))
}

/// The 16-hex ids that in-space documents cite outside code.
fn cited_ids(head: &Head) -> BTreeSet<String> {
    head.files
        .values()
        .flat_map(|text| citation_spans(text))
        .filter(|s| s.kind == CitationKind::ArtifactId && !s.in_code && s.range.is_some())
        .map(|s| s.raw)
        .collect()
}

/// Why an event of `kind` has no home in a text, or `None` when it has one.
fn no_text_home(kind: &str) -> Option<&'static str> {
    match kind {
        "field_patch" => Some("git history of the file holds it"),
        "status_change" | "superseded_by" => Some("the frontmatter holds it"),
        "worktree_fork" | "worktree_merge" => Some("the worktree overlay is removed"),
        _ => None,
    }
}

/// One `skipped` item per event of a kind that no text holds.
fn plan_skipped_events(old: &OldRows, plan: &mut ExportPlan) {
    for event in &old.events {
        let Some(reason) = no_text_home(&event.kind) else {
            continue;
        };
        plan.items.push(Item {
            class: Class::Event,
            status: Status::Skipped,
            reason: reason.to_owned(),
            path: old
                .documents
                .get(&event.artifact_id)
                .map(|d| d.rel_path.clone()),
            line: None,
            detail: Some(event.kind.clone()),
            suggestion: None,
        });
    }
}

/// One `skipped` item for all write-origin entry citations: they are report-only.
fn plan_write_cites(old: &OldRows, plan: &mut ExportPlan) {
    if old.write_cites == 0 {
        return;
    }
    plan.items.push(Item {
        class: Class::WriteCite,
        status: Status::Skipped,
        reason: "an entry citation written by a tool call stays a catalog row".to_owned(),
        path: None,
        line: None,
        detail: Some(format!("{} rows", old.write_cites)),
        suggestion: None,
    });
}

/// The old rows in scope by class. A class with no row is absent.
fn population_of(old: &OldRows) -> BTreeMap<String, usize> {
    let mut population: BTreeMap<String, usize> = BTreeMap::new();
    let mut add = |class: String, n: usize| {
        if n > 0 {
            *population.entry(class).or_default() += n;
        }
    };
    for aug in &old.augmentations {
        let (entries, other) = params_counts(aug);
        add("params_entry".to_owned(), entries);
        add("params_other".to_owned(), other);
    }
    add("augmentation_shape".to_owned(), old.augmentations.len());
    for event in &old.events {
        add(format!("event:{}", event.kind), 1);
    }
    add("observation".to_owned(), old.observations.len());
    add("manual_link".to_owned(), old.links.len());
    add("write_cite".to_owned(), old.write_cites);
    population
}

/// `(entry rows, other keys)` of one augmentation's params: the elements of the entry collection,
/// and every key outside it (every key when the augmentation has no entry collection). Params
/// that are not a JSON object count as nothing.
fn params_counts(aug: &AugmentationRow) -> (usize, usize) {
    let Ok(serde_json::Value::Object(params)) = serde_json::from_str(&aug.params) else {
        return (0, 0);
    };
    let collection = aug.entry_collection.as_deref();
    let entries = collection
        .and_then(|name| params.get(name))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let other = params
        .keys()
        .filter(|key| Some(key.as_str()) != collection)
        .count();
    (entries, other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
    use crate::librarian::catalog::links::{self, LinkRow};
    use crate::librarian::catalog::observations::{self, ObservationRow};
    use crate::librarian::cutover::old_rows::load;
    use crate::librarian::cutover::testkit::*;
    use crate::librarian::derived::diff::RootMap;
    use crate::librarian::derived::scan::open_commit;
    use crate::librarian::derived::testkit::{repo_with, FILE};
    use std::path::Path;

    /// A repository in a temporary directory holding `a.md` and `sub/b.md`, and the plan for the
    /// catalog `fill` builds, with every document at `<repo>/<rel>`.
    fn plan_for(fill: impl FnOnce(&crate::librarian::catalog::Catalog, &Path)) -> ExportPlan {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let repo = repo_with(
            &root,
            &[
                ("a.md", FILE, b"# A\n" as &[u8]),
                ("sub/b.md", FILE, b"# B\n"),
            ],
        );
        let old = old_catalog_with(|cat| fill(cat, &root));
        let rows = load(&old, &RootMap::new(root.clone())).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        plan_export(&repo, &commit, &rows).unwrap()
    }

    fn with_docs(cat: &crate::librarian::catalog::Catalog, root: &Path) {
        add_doc(cat, root, "a", "a.md");
        add_doc(cat, root, "b", "sub/b.md");
    }

    #[test]
    fn a_catalog_with_no_rows_plans_nothing() {
        let plan = plan_for(|_, _| {});
        assert!(plan.population.is_empty());
        assert!(plan.items.is_empty());
        assert!(plan.files.is_empty());
        assert_eq!(plan.writes(), 0);
        assert!(plan.tally().is_empty());
        assert_eq!(plan.repo_name, "repo", "no repo.toml: the fallback name");
        assert_eq!(plan.commit.len(), 40);
        assert!(!plan.shallow);
        assert_eq!(plan.out_of_scope, OutOfScope::default());
    }

    #[test]
    fn the_population_counts_every_class() {
        let plan = plan_for(|cat, root| {
            let a_slug = add_doc(cat, root, "a", "a.md");
            add_doc(cat, root, "b", "sub/b.md");
            add_doc_at(cat, "c", "/elsewhere/c.md");
            // Two entry rows in `items`, plus two other keys; no entry collection on b: its two
            // keys are all "other".
            add_augmentation(
                cat,
                "a",
                r#"{"items":[{"id":1},{"id":2},{"id":3}],"owner":"x","stamp":2}"#,
                Some("items"),
            );
            add_augmentation(cat, "b", r#"{"k1":1,"k2":2}"#, None);
            add_augmentation(cat, "c", r#"{"ignored":[1,2,3]}"#, None);
            add_event(cat, "e1", "a", "note", "{}");
            add_event(cat, "e2", "a", "note", "{}");
            add_event(cat, "e3", "b", "verdict", "{}");
            add_event(cat, "e4", "a", "field_patch", "{}");
            add_event(cat, "e5", "c", "note", "{}");
            observations::insert(
                cat,
                &ObservationRow {
                    id: None,
                    artifact_id: "a".into(),
                    text: "seen".into(),
                    source: None,
                    created_at: 1,
                },
            )
            .unwrap();
            links::insert(
                cat,
                &LinkRow {
                    src_id: "a".into(),
                    dst_id: "b".into(),
                    rel: "depends_on".into(),
                    created_at: 1,
                },
            )
            .unwrap();
            entry_cite::insert_with(
                &cat.conn,
                &EntryCiteRow {
                    src_slug: a_slug,
                    src_local: "R-1".into(),
                    dst_ref: "x:R-2".into(),
                    rel: "cites".into(),
                    origin: "write".into(),
                    created_at: 1,
                },
            )
            .unwrap();
        });
        let want: BTreeMap<String, usize> = [
            ("params_entry", 3),
            ("params_other", 4),
            ("augmentation_shape", 2),
            ("event:note", 2),
            ("event:verdict", 1),
            ("event:field_patch", 1),
            ("observation", 1),
            ("manual_link", 1),
            ("write_cite", 1),
        ]
        .into_iter()
        .map(|(k, n)| (k.to_owned(), n))
        .collect();
        assert_eq!(plan.population, want);
    }

    #[test]
    fn event_kinds_with_no_text_home_are_skipped_with_the_specs_reason() {
        let plan = plan_for(|cat, root| {
            with_docs(cat, root);
            for (n, kind) in [
                "field_patch",
                "status_change",
                "superseded_by",
                "worktree_fork",
                "worktree_merge",
                "note",
            ]
            .iter()
            .enumerate()
            {
                add_event(cat, &format!("e{n}"), "a", kind, "{}");
            }
        });
        let reason_of = |kind: &str| -> Vec<String> {
            plan.items
                .iter()
                .filter(|i| i.class == Class::Event && i.detail.as_deref() == Some(kind))
                .map(|i| i.reason.clone())
                .collect()
        };
        assert_eq!(
            reason_of("field_patch"),
            ["git history of the file holds it"]
        );
        assert_eq!(reason_of("status_change"), ["the frontmatter holds it"]);
        assert_eq!(reason_of("superseded_by"), ["the frontmatter holds it"]);
        assert_eq!(
            reason_of("worktree_fork"),
            ["the worktree overlay is removed"]
        );
        assert_eq!(
            reason_of("worktree_merge"),
            ["the worktree overlay is removed"]
        );
        assert!(
            reason_of("note").is_empty(),
            "a note has a text home: it is not skipped here"
        );
        assert_eq!(plan.items.len(), 5);
        for item in &plan.items {
            assert_eq!(item.status, Status::Skipped);
            assert_eq!(item.path.as_deref(), Some("a.md"));
        }
        let tally = plan.tally();
        let events = &tally[&Class::Event];
        assert_eq!(events.skipped, 5);
        assert_eq!(
            events.moved + events.already_exported + events.unrecoverable,
            0
        );
        assert_eq!(events.reasons["the frontmatter holds it"], 2);
        assert_eq!(events.reasons["the worktree overlay is removed"], 2);
        assert_eq!(plan.writes(), 0, "a skipped fact writes no file");
    }

    #[test]
    fn write_cites_are_reported_not_exported() {
        let plan = plan_for(|cat, root| {
            let a_slug = with_docs_slug(cat, root);
            for local in ["R-1", "R-2", "R-3"] {
                entry_cite::insert_with(
                    &cat.conn,
                    &EntryCiteRow {
                        src_slug: a_slug.clone(),
                        src_local: local.into(),
                        dst_ref: "x:R-9".into(),
                        rel: "cites".into(),
                        origin: "write".into(),
                        created_at: 1,
                    },
                )
                .unwrap();
            }
        });
        let cites: Vec<&Item> = plan
            .items
            .iter()
            .filter(|i| i.class == Class::WriteCite)
            .collect();
        assert_eq!(cites.len(), 1, "one line stands for the rows");
        assert_eq!(cites[0].status, Status::Skipped);
        assert_eq!(cites[0].detail.as_deref(), Some("3 rows"));
        assert!(plan.files.is_empty());
        assert_eq!(plan.writes(), 0);
        assert_eq!(plan.population["write_cite"], 3);
    }

    fn with_docs_slug(cat: &crate::librarian::catalog::Catalog, root: &Path) -> String {
        let slug = add_doc(cat, root, "a", "a.md");
        add_doc(cat, root, "b", "sub/b.md");
        slug
    }

    /// The plan of the single commit holding `files`, for the old catalog `fill` builds.
    fn plan_of_files(
        files: &[(&str, &[u8])],
        fill: impl FnOnce(&crate::librarian::catalog::Catalog, &Path),
    ) -> ExportPlan {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files.iter().map(|&(p, b)| (p, FILE, b)).collect();
        let repo = repo_with(&root, &entries);
        let old = old_catalog_with(|cat| fill(cat, &root));
        let rows = load(&old, &RootMap::new(root.clone())).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        plan_export(&repo, &commit, &rows).unwrap()
    }

    fn citation_items(plan: &ExportPlan) -> Vec<&Item> {
        plan.items
            .iter()
            .filter(|i| i.class == Class::Citation)
            .collect()
    }

    #[test]
    fn every_in_space_document_is_rewritten_and_frozen_ones_are_reported() {
        let plan = plan_of_files(
            &[
                ("docs/ledger.md", b"# L\n\n## F-9 \xe2\x80\x94 nine\n"),
                ("docs/b.md", b"# B\n\nsee F-9.\n"),
                ("docs/frozen.md", b"# Z\n\nsee F-9.\n"),
                ("src/x.rs", b"const P: &str = \"../docs/frozen.md\";\n"),
            ],
            |_, _| {},
        );
        let changed: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(changed, ["docs/b.md"], "the frozen file keeps its text");
        let b = &plan.files["docs/b.md"];
        assert_eq!(b.new_text, "# B\n\nsee [[ledger:F-9]].\n");
        assert_eq!(b.head_text.as_deref(), Some("# B\n\nsee F-9.\n"));
        assert_eq!(plan.writes(), 1);

        let items = citation_items(&plan);
        assert_eq!(items.len(), 2, "{items:?}");
        let moved = items.iter().find(|i| i.status == Status::Moved).unwrap();
        assert_eq!(moved.path.as_deref(), Some("docs/b.md"));
        assert_eq!(moved.line, Some(3));
        let frozen = items
            .iter()
            .find(|i| i.status == Status::Unrecoverable)
            .unwrap();
        assert_eq!(frozen.path.as_deref(), Some("docs/frozen.md"));
        assert_eq!(frozen.line, Some(3));
        assert!(frozen.reason.starts_with("frozen: "), "{}", frozen.reason);
        assert!(frozen.reason.contains("src/x.rs:1"), "{}", frozen.reason);
    }

    #[test]
    fn a_catalog_id_citation_is_rewritten_and_one_the_catalog_lacks_is_reported() {
        let plan = plan_of_files(
            &[
                (
                    "docs/a.md",
                    b"# A\n\nsee 0123456789abcdef and 00112233aabbccdd.\n",
                ),
                ("docs/b.md", b"# B\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "0123456789abcdef", "docs/b.md");
            },
        );
        assert_eq!(
            plan.files["docs/a.md"].new_text,
            "# A\n\nsee [[b]] and 00112233aabbccdd.\n"
        );
        let items = citation_items(&plan);
        assert_eq!(items.len(), 2, "{items:?}");
        let open = items
            .iter()
            .find(|i| i.status == Status::Unrecoverable)
            .unwrap();
        assert_eq!(
            open.reason,
            crate::librarian::cutover::rewrite::REASON_UNKNOWN_ID
        );
    }

    /// A two-commit repository: `docs/old.md` of the first commit is `docs/new.md` in the second,
    /// and `docs/a.md` of the second cites the id of the old path.
    fn history_plan(shallow: bool, catalog_row: bool) -> ExportPlan {
        use crate::librarian::derived::testkit::commit_files;
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        const BODY: &[u8] =
            b"# Notes\n\nSome text that makes the file big enough to match.\n\nMore.\n";
        let repo = repo_with(
            &root,
            &[("docs/old.md", FILE, BODY), ("docs/a.md", FILE, b"# A\n")],
        );
        let first = repo.head().unwrap().peel_to_commit().unwrap();
        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/old.md"));
        let a_text = format!("# A\n\nsee {id}.\n");
        let tip = commit_files(
            &repo,
            &[("docs/new.md", BODY), ("docs/a.md", a_text.as_bytes())],
            &[&first],
            "move",
        );
        if shallow {
            std::fs::write(root.join(".git/shallow"), format!("{tip}\n")).unwrap();
        }
        // The old catalog may still hold the file at the path the first commit gave it.
        let old = old_catalog_with(|cat| {
            if catalog_row {
                add_doc(cat, &root, &id, "docs/old.md");
            }
        });
        let rows = load(&old, &RootMap::new(root.clone())).unwrap();
        let commit = repo.find_commit(tip).unwrap();
        plan_export(&repo, &commit, &rows).unwrap()
    }

    #[test]
    fn an_id_the_catalog_lacks_is_recovered_through_a_git_rename() {
        let plan = history_plan(false, false);
        assert!(!plan.shallow);
        assert_eq!(plan.files["docs/a.md"].new_text, "# A\n\nsee [[new]].\n");
    }

    #[test]
    fn a_shallow_clone_reports_the_ids_history_would_have_needed() {
        let plan = history_plan(true, false);
        assert!(plan.shallow);
        assert!(
            plan.files.is_empty(),
            "nothing is rewritten without history"
        );
        let items = citation_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].status, Status::Unrecoverable);
        assert_eq!(items[0].reason, "shallow clone: history is incomplete");
    }

    #[test]
    fn a_catalog_id_whose_file_git_moved_is_recovered_through_history() {
        let plan = history_plan(false, true);
        assert_eq!(
            plan.files["docs/a.md"].new_text, "# A\n\nsee [[new]].\n",
            "the catalog knew docs/old.md; a later commit moved it"
        );
        let items = citation_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].status, Status::Moved);
    }

    #[test]
    fn a_catalog_id_with_no_history_falls_back_to_the_catalogs_hint() {
        let plan = plan_of_files(
            &[
                ("docs/a.md", b"# A\n\nsee 0123456789abcdef.\n"),
                ("docs/b.md", b"# B\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "0123456789abcdef", "docs/gone.md");
            },
        );
        assert!(plan.files.is_empty());
        let items = citation_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].status, Status::Unrecoverable);
        assert_eq!(
            items[0].reason,
            "the id names a file that HEAD no longer holds"
        );
    }

    #[test]
    fn a_shallow_clone_keeps_the_catalogs_hint_for_an_id_it_knows() {
        let plan = history_plan(true, true);
        assert!(plan.files.is_empty(), "no history, so no move is followed");
        let items = citation_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(
            items[0].reason,
            "the id names a file that HEAD no longer holds"
        );
    }

    #[test]
    fn writes_counts_changed_files_and_renames() {
        let change = |path: &str, from: Option<&str>, head: Option<&str>, new: &str| FileChange {
            path: path.to_owned(),
            renamed_from: from.map(str::to_owned),
            head_text: head.map(str::to_owned),
            new_text: new.to_owned(),
        };
        let mut plan = plan_for(|_, _| {});
        for f in [
            change("same.md", None, Some("x"), "x"),
            change("edited.md", None, Some("x"), "y"),
            change("new.md", None, None, "z"),
            change("moved.md", Some("old.md"), Some("x"), "x"),
        ] {
            plan.files.insert(f.path.clone(), f);
        }
        assert_eq!(
            plan.writes(),
            3,
            "edited, new and renamed; not the unchanged one"
        );
    }
}
