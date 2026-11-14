//! The export plan: every change the cutover would make, composed in memory.
//!
//! [`plan_export`] is a function of (`HEAD`, the old rows): it reads committed blobs through
//! `git2` and the rows [`OldRows`] already holds, and writes nothing. `cutover plan` prints it;
//! `cutover export` applies it.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::librarian::catalog::augmentation::AugmentationRow;
use crate::librarian::cutover::old_rows::{OldRows, OutOfScope};
use crate::librarian::derived::build::{key_space_from_commit, repo_name_of};
use crate::librarian::derived::scan::{scan_commit, Skip};
use crate::librarian::identity::repo_name::RepoName;

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
/// The task that plans the rewrite, the renames and the exports reads `files` and `outside`.
#[allow(dead_code)]
struct Head {
    files: BTreeMap<String, String>,
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
    Ok(plan)
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
