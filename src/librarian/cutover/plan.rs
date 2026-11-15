//! The export plan: every change the cutover would make, composed in memory.
//!
//! [`plan_export`] is a function of (`HEAD`, the old rows): it reads committed blobs through
//! `git2` and the rows [`OldRows`] already holds, and writes nothing. `cutover plan` prints it;
//! `cutover export` applies it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::librarian::catalog::augmentation::AugmentationRow;
use crate::librarian::cutover::code_refs::{paths_named_in, CodeRefs, LINK_MODE};
use crate::librarian::cutover::entries::{
    other_params, params_of, recover_params_entries, refuse_entries, Params,
};
use crate::librarian::cutover::events::{export_events, refuse_events};
use crate::librarian::cutover::history::HistoryIds;
use crate::librarian::cutover::links::{export_links, refuse_links};
use crate::librarian::cutover::old_rows::{OldEvent, OldObservation, OldRows, OutOfScope};
use crate::librarian::cutover::rename::{
    applied_item, in_head_item, plan_renames_against, renames_already_in_head, RenameSet,
};
use crate::librarian::cutover::rewrite::{
    join_path, rewrite_added, rewrite_document, Finding, Form, IdSource, IdTarget, Outcome,
    Resolver, REASON_UNKNOWN_ID,
};
use crate::librarian::cutover::sidecars::{
    compare_declared, declared_sidecar, plan_shape, sidecar_rel, ShapeExport,
};
use crate::librarian::derived::build::{key_space_from_commit, repo_name_of};
use crate::librarian::derived::scan::{scan_commit, Skip};
use crate::librarian::identity::grammar::Reference;
use crate::librarian::identity::index::{DocInput, KeyIndex, Resolution};
use crate::librarian::identity::key::{folder_of, key_of, markdown_stem, Key};
use crate::librarian::identity::key_space::KeySpace;
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
    /// Every class, in the order of the enum.
    const ALL: [Class; 9] = [
        Class::ParamsEntry,
        Class::ParamsOther,
        Class::AugmentationShape,
        Class::Event,
        Class::Observation,
        Class::ManualLink,
        Class::WriteCite,
        Class::Rename,
        Class::Citation,
    ];

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

    /// The class a key of `ExportPlan::population` counts rows of: the label, or `event:<kind>`.
    fn of_population_key(key: &str) -> Option<Class> {
        let name = key.split(':').next()?;
        Class::ALL.into_iter().find(|class| class.label() == name)
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
    /// Export moves the old path to this one; the old path is not kept.
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
    /// The items of each class counted by status and reason, and a zero row for every class that has
    /// old rows in scope and no item: a class with rows always has a line of its own.
    pub fn tally(&self) -> BTreeMap<Class, ClassTally> {
        let mut tally: BTreeMap<Class, ClassTally> = BTreeMap::new();
        for key in self.population.keys() {
            if let Some(class) = Class::of_population_key(key) {
                tally.entry(class).or_default();
            }
        }
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
    /// The markdown paths of `HEAD` that are not in `files`: outside the key space, or a file
    /// with no text (not UTF-8, a symlink). A rename never lands on one of them.
    outside: BTreeSet<String>,
    repo_name: RepoName,
    /// The key space of `HEAD`: code in a subtree it excludes is no evidence ([`CodeRefs::load`]).
    space: KeySpace,
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
                // A blob with no text or a symlink has no text to plan against, but its path
                // is taken.
                Err(Skip::NotUtf8 | Skip::Symlink) => {
                    outside.insert(scanned.rel_path);
                }
            }
        }
        let (repo_name, _) = repo_name_of(repo, commit)?;
        Ok(Head {
            files,
            outside,
            repo_name,
            space,
        })
    }
}

/// Plan the export of `commit` for the rows in `old`.
///
/// Fills the population of the old rows by class, the renames of the files whose names give no
/// valid key (with the ones it cannot make as items), the items for the events whose kind has no
/// home in any text, and the rewrite of every citation. Old rows follow a renamed document to the
/// path it has after the renames, and every citation is rendered against that final path set.
///
/// The params, the sidecars, the events, the observations and the manual links of a document go into
/// its text in that order ([`plan_augmentations`], then [`plan_events_and_links`]), each step from
/// the text the one before left. One set of [`CitationTables`] serves every step: the citation
/// rewrite of the committed text, and of each section a later step adds (D-R9). Its index reads
/// the texts with the sections of the recovered params entries ([`texts_with_entries`]), as the
/// plan of the commit that holds the export reads them, so that plan writes nothing.
pub fn plan_export(
    repo: &git2::Repository,
    commit: &git2::Commit,
    old: &OldRows,
) -> anyhow::Result<ExportPlan> {
    let head = Head::load(repo, commit)?;
    let code = CodeRefs::load(repo, commit, &head.space)?;
    let mut plan = ExportPlan {
        repo_name: head.repo_name.to_string(),
        commit: commit.id().to_string(),
        shallow: repo.is_shallow(),
        population: population_of(old),
        items: Vec::new(),
        files: BTreeMap::new(),
        out_of_scope: old.out_of_scope.clone(),
    };
    let renames = plan_file_renames(&head, old, &code, &mut plan);
    plan_skipped_events(old, &renames.paths(), &mut plan);
    plan_write_cites(old, &mut plan);
    let tables = CitationTables::build(repo, commit, old, &head, &code, &renames, plan.shallow)?;
    plan_citations(&head, &code, &tables, &mut plan);
    plan_augmentations(repo, commit, old, &head, &code, &tables, &mut plan)?;
    plan_events_and_links(old, &head, &code, &tables, &mut plan);
    plan_outside_links(repo, commit, &head, &renames, &mut plan)?;
    Ok(plan)
}

/// The renames of the run (D-R14): the ones this run applies, with an item for each and for each
/// it cannot make, and the ones `HEAD` already holds (D-R25), each an `already_exported` item.
///
/// A path `HEAD` lacks that the old catalog still holds a live row for is taken: a rename onto it
/// would put two documents' rows on one path, and the run after the commit could not work the
/// rename out.
fn plan_file_renames(
    head: &Head,
    old: &OldRows,
    code: &CodeRefs,
    plan: &mut ExportPlan,
) -> RenameSet {
    let before = KeyIndex::build(
        head.repo_name.clone(),
        head.files.iter().map(|(path, text)| DocInput {
            rel_path: path.clone(),
            text: text.clone(),
        }),
    );
    let paths: Vec<String> = head.files.keys().chain(&head.outside).cloned().collect();
    let head_paths: BTreeSet<String> = paths.iter().cloned().collect();
    let held: BTreeSet<String> = old
        .documents
        .values()
        .filter(|d| !d.missing && !head_paths.contains(&d.rel_path))
        .map(|d| d.rel_path.clone())
        .collect();
    let keys: BTreeSet<Key> = head.files.keys().filter_map(|p| key_of(p).ok()).collect();
    let (planned, blocked) = plan_renames_against(&paths, &held, &keys, before.problems(), code);
    let in_head = renames_already_in_head(old, &head_paths);
    plan.items.extend(blocked);
    plan.items.extend(planned.iter().map(applied_item));
    plan.items.extend(in_head.iter().map(in_head_item));
    RenameSet { planned, in_head }
}

/// Rewrite the old citation forms of every in-space document (D-R5). A document that code,
/// a dot directory or an mdBook reads by path is frozen (D-R26): its text stays, and each
/// rewrite it would have had is an `unrecoverable` item.
///
/// The index, the stem table and the id targets all use the final paths (after the renames), so a
/// form is rendered with the key the file has once the plan is applied. A renamed file is one
/// entry of the plan, under its new path.
fn plan_citations(head: &Head, code: &CodeRefs, tables: &CitationTables, plan: &mut ExportPlan) {
    let resolver = tables.resolver(&head.repo_name);
    for (path, text) in &head.files {
        let now = final_path(&tables.moved, path);
        let frozen = code.frozen(path);
        let (new_text, findings) = rewrite_document(now, text, &resolver);
        for finding in &findings {
            plan.items
                .push(citation_item(now, finding, frozen.as_deref(), plan.shallow));
        }
        let renamed_from = (now != path).then(|| path.clone());
        if frozen.is_none() && (new_text != *text || renamed_from.is_some()) {
            plan.files.insert(
                now.to_owned(),
                FileChange {
                    path: now.to_owned(),
                    renamed_from,
                    head_text: Some(text.clone()),
                    new_text,
                },
            );
        }
    }
}

/// What the citation rewrite resolves against (D-R5, D-R28), built once per plan: the index of the
/// final paths, the renamed keys and paths, the stems, and the ids of the catalog and of history.
struct CitationTables {
    index: KeyIndex,
    /// Old path to final path ([`RenameSet::paths`]).
    moved: BTreeMap<String, String>,
    renamed_keys: BTreeMap<String, Key>,
    ambiguous_keys: BTreeMap<String, Vec<String>>,
    stems: BTreeMap<String, Vec<String>>,
    ids: BTreeMap<String, IdTarget>,
    id_hints: BTreeMap<String, String>,
}

impl CitationTables {
    /// The index is built from the texts of `HEAD` with the sections of the recovered params
    /// entries ([`texts_with_entries`]), so a token such a section defines resolves in the first plan.
    fn build(
        repo: &git2::Repository,
        commit: &git2::Commit,
        old: &OldRows,
        head: &Head,
        code: &CodeRefs,
        renames: &RenameSet,
        shallow: bool,
    ) -> anyhow::Result<CitationTables> {
        let moved = renames.paths();
        let index = KeyIndex::build(
            head.repo_name.clone(),
            texts_with_entries(old, head, code, &moved)
                .into_iter()
                .map(|(path, text)| DocInput {
                    rel_path: final_path(&moved, &path).to_owned(),
                    text,
                }),
        );
        let (ids, id_hints) = recover_ids(repo, commit, old, head, &moved, shallow)?;
        Ok(CitationTables {
            index,
            renamed_keys: renames.keys(),
            ambiguous_keys: renames.ambiguous_keys(head.files.keys()),
            stems: stems_of(head, renames),
            ids,
            id_hints,
            moved,
        })
    }

    fn resolver<'a>(&'a self, this_repo: &'a RepoName) -> Resolver<'a> {
        Resolver {
            index: &self.index,
            this_repo,
            renamed_keys: &self.renamed_keys,
            renamed_paths: &self.moved,
            ambiguous_keys: &self.ambiguous_keys,
            stems: &self.stems,
            ids: &self.ids,
            id_hints: &self.id_hints,
        }
    }
}

/// How an export step has the lines it adds to a document rewritten: `(before, after)` to the
/// rewritten `after` and one citation item per form found in the added lines.
pub type RewriteAdded<'a> = dyn Fn(&str, &str) -> (String, Vec<Item>) + 'a;

/// The [`RewriteAdded`] of the document at `now` ([`rewrite_added`]). The document is not frozen:
/// a step edits only a document [`open_document`] opened.
fn added_text_rewrite<'a>(
    resolver: &'a Resolver<'a>,
    now: &'a str,
    shallow: bool,
) -> impl Fn(&str, &str) -> (String, Vec<Item>) + 'a {
    move |before, after| {
        let (text, findings) = rewrite_added(now, before, after, resolver);
        let items = findings
            .iter()
            .map(|finding| citation_item(now, finding, None, shallow))
            .collect();
        (text, items)
    }
}

/// What `HEAD` holds at a path that may carry a sidecar.
enum Committed {
    Absent,
    Text(String),
    /// A link, or bytes that are not UTF-8: it exists and cannot be compared.
    Unreadable,
}

/// The blob of `commit` at `path`, as text.
fn committed_at(
    repo: &git2::Repository,
    commit: &git2::Commit,
    path: &str,
) -> anyhow::Result<Committed> {
    let tree = commit.tree()?;
    let Ok(entry) = tree.get_path(Path::new(path)) else {
        return Ok(Committed::Absent);
    };
    if entry.kind() != Some(git2::ObjectType::Blob) || entry.filemode() == LINK_MODE {
        return Ok(Committed::Unreadable);
    }
    let blob = repo.find_blob(entry.id())?;
    Ok(match std::str::from_utf8(blob.content()) {
        Ok(text) => Committed::Text(text.to_owned()),
        Err(_) => Committed::Unreadable,
    })
}

const REASON_NOT_IN_HEAD: &str = "document not in HEAD";
const REASON_NOT_IN_SPACE: &str = "document is outside the key space or has no text";

/// The augmentation rows of each document, by its path after the renames, in id order.
fn augmentations_by_path<'a>(
    old: &'a OldRows,
    moved: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<&'a AugmentationRow>> {
    let mut by_path: BTreeMap<String, Vec<&AugmentationRow>> = BTreeMap::new();
    for row in &old.augmentations {
        let Some(document) = old.documents.get(&row.artifact_id) else {
            continue;
        };
        let now = final_path(moved, &document.rel_path).to_owned();
        by_path.entry(now).or_default().push(row);
    }
    for rows in by_path.values_mut() {
        rows.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    }
    by_path
}

/// The text of every in-space document of `HEAD`, keyed by its path in `HEAD`, with the sections
/// [`plan_augmentations`] adds for its params entries (D-R13); a frozen document keeps its text
/// (D-R26). The commit that holds the export defines each recovered token, so the citation index
/// is built from these texts and the first plan resolves a token as the plan after the commit does
/// (D-R20, D-R9).
///
/// The sections are found with a rewrite that changes nothing. Recovery reads which tokens a text
/// defines with the reader the index uses (`entry_sections`), and the citation rewrite changes no
/// line that reader reads as a definer: no heading, nothing inside a code fence and nothing in the
/// frontmatter. A line it does change, such as a seven-hash line, defines nothing before or after.
/// So the rewrite does not change which sections a document defines or which read back.
fn texts_with_entries(
    old: &OldRows,
    head: &Head,
    code: &CodeRefs,
    moved: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let by_path = augmentations_by_path(old, moved);
    let unchanged = |_: &str, after: &str| (after.to_owned(), Vec::<Item>::new());
    head.files
        .iter()
        .map(|(path, text)| {
            let now = final_path(moved, path);
            let rows = by_path
                .get(now)
                .filter(|_| code.frozen(path).is_none())
                .into_iter()
                .flatten();
            let text = rows.fold(text.clone(), |text, row| {
                recover_params_entries(now, &text, row, &unchanged).0
            });
            (path.clone(), text)
        })
        .collect()
}

/// Params entries, other params and the shape of every augmentation (D-R13, D-R23, D-R24), per
/// document path after the renames. A document's text goes through the citation rewrite first
/// ([`plan_citations`]), then the entries (a recovered section is rewritten like committed text,
/// D-R9), then the declaration stamp; a sidecar is a new file.
///
/// Two live rows may name one path (the same file under two spellings of the root). The rows are
/// taken in id order against one running text: an entry the first row recovered is `already_exported`
/// or `skipped` for the second, the second row's shape is compared with the sidecar the first one
/// planned (so it is `already_exported` or drift, never a second export), and every row reports its
/// own other params. No params are lost and nothing is exported twice.
///
/// A path that `HEAD` does not hold, one outside the key space, and a frozen document (D-R26) get
/// `unrecoverable` items for their entries and their shape; their other params are reported as
/// everywhere else.
///
/// The sidecar name is not injective (`docs/a-b.md` and `docs/a/b.md` give one name). A sidecar
/// path that another document names, by its own name or by its declaration in `HEAD`, is a claim:
/// every document that names it gets an `unrecoverable` shape item and none is planned, so the
/// result does not depend on the order of the rows. A document that declares a committed sidecar
/// under another name (a rename gave it a new one) has the catalog shape compared with that file:
/// a difference is drift and nothing is re-pointed; an equal one is replaced by the new sidecar
/// and the old file is reported as no longer declared. The tool never plans a deletion.
fn plan_augmentations(
    repo: &git2::Repository,
    commit: &git2::Commit,
    old: &OldRows,
    head: &Head,
    code: &CodeRefs,
    tables: &CitationTables,
    plan: &mut ExportPlan,
) -> anyhow::Result<()> {
    let moved = &tables.moved;
    let resolver = tables.resolver(&head.repo_name);
    let by_path = augmentations_by_path(old, moved);
    let refusal_of = |plan: &ExportPlan, now: &str| document_refusal(head, code, plan, now);

    // Who names each sidecar path: a document with a shape to plan, by its name, and a document of
    // `HEAD`, by its declaration.
    let mut claims: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for now in by_path.keys() {
        if refusal_of(plan, now).is_none() {
            claims
                .entry(sidecar_rel(now))
                .or_default()
                .insert(now.clone());
        }
    }
    for (path, text) in &head.files {
        if let Some(declared) = declared_sidecar(text) {
            let now = final_path(moved, path).to_owned();
            claims.entry(declared).or_default().insert(now);
        }
    }

    for (now, rows) in by_path {
        let document = match open_document(head, code, plan, &now) {
            Ok(document) => document,
            Err(reason) => {
                for row in rows {
                    plan.items.extend(refuse_entries(&now, row, &reason));
                    plan.items.extend(other_params(&now, row));
                    plan.items
                        .push(shape_item(&now, Status::Unrecoverable, &reason, None));
                }
                continue;
            }
        };
        let base = document.base.clone();
        let mut text = base.clone();

        let sidecar_path = sidecar_rel(&now);
        let claimed_by = claims
            .get(&sidecar_path)
            .and_then(|names| names.iter().find(|name| **name != now))
            .cloned();
        let committed = committed_at(repo, commit, &sidecar_path)?;
        // A committed sidecar the document declares under another name.
        let elsewhere = declared_sidecar(&base).filter(|declared| *declared != sidecar_path);
        let elsewhere_committed = match &elsewhere {
            Some(declared) => Some(committed_at(repo, commit, declared)?),
            None => None,
        };
        // The text of the sidecar this path will have: the committed one, or the one the first row
        // planned.
        let mut sidecar: Option<String> = match &committed {
            Committed::Text(text) => Some(text.clone()),
            _ => None,
        };
        let rewrite = added_text_rewrite(&resolver, &now, plan.shallow);
        for row in rows {
            let (next, items) = recover_params_entries(&now, &text, row, &rewrite);
            text = next;
            plan.items.extend(items);
            plan.items.extend(other_params(&now, row));

            if let Some(other) = &claimed_by {
                let reason = format!("sidecar path {sidecar_path} is claimed by {other}");
                plan.items.push(shape_item(
                    &now,
                    Status::Unrecoverable,
                    &reason,
                    Some(sidecar_path.clone()),
                ));
                continue;
            }
            if matches!(committed, Committed::Unreadable) {
                let reason = "the committed sidecar cannot be read as text";
                plan.items.push(shape_item(
                    &now,
                    Status::Unrecoverable,
                    reason,
                    Some(sidecar_path.clone()),
                ));
                continue;
            }
            if let (Some(declared), Some(old_committed)) = (&elsewhere, &elsewhere_committed) {
                let blocked = match old_committed {
                    Committed::Text(old_text) => match compare_declared(row, old_text, declared) {
                        Some(ShapeExport::Drift { fields }) => {
                            Some(format!("shape drift: {}", fields.join(", ")))
                        }
                        Some(ShapeExport::Refused(why)) => Some(why),
                        _ => None,
                    },
                    Committed::Unreadable => {
                        Some("the committed sidecar cannot be read as text".to_owned())
                    }
                    Committed::Absent => None,
                };
                if let Some(reason) = blocked {
                    plan.items.push(shape_item(
                        &now,
                        Status::Unrecoverable,
                        &reason,
                        Some(declared.clone()),
                    ));
                    continue;
                }
            }
            let item = match plan_shape(&now, &text, row, sidecar.as_deref()) {
                ShapeExport::Export {
                    sidecar_rel,
                    sidecar_text,
                    stamped_doc,
                } => {
                    plan.files.insert(
                        sidecar_rel.clone(),
                        FileChange {
                            path: sidecar_rel.clone(),
                            renamed_from: None,
                            head_text: None,
                            new_text: sidecar_text.clone(),
                        },
                    );
                    sidecar = Some(sidecar_text);
                    text = stamped_doc;
                    shape_item(
                        &now,
                        Status::Moved,
                        "sidecar written and declaration stamped",
                        Some(sidecar_rel),
                    )
                }
                ShapeExport::AlreadyExported { stamped_doc: None } => shape_item(
                    &now,
                    Status::AlreadyExported,
                    "the sidecar equals the catalog's shape",
                    Some(sidecar_path.clone()),
                ),
                ShapeExport::AlreadyExported {
                    stamped_doc: Some(stamped_doc),
                } => {
                    text = stamped_doc;
                    shape_item(
                        &now,
                        Status::Moved,
                        "declaration stamped; the sidecar equals the catalog's shape",
                        Some(sidecar_path.clone()),
                    )
                }
                ShapeExport::Drift { fields } => shape_item(
                    &now,
                    Status::Unrecoverable,
                    &format!("shape drift: {}", fields.join(", ")),
                    Some(sidecar_path.clone()),
                ),
                ShapeExport::Refused(why) => shape_item(&now, Status::Unrecoverable, &why, None),
            };
            plan.items.push(item);
        }

        // The old sidecar stays in the repository when the declaration now names the new one.
        if let (Some(declared), Some(Committed::Text(_))) = (&elsewhere, &elsewhere_committed) {
            if declared_sidecar(&text).as_deref() != Some(declared.as_str()) {
                let reason = format!("sidecar {declared} is no longer declared; remove it by hand");
                plan.items.push(shape_item(
                    &now,
                    Status::Skipped,
                    &reason,
                    Some(declared.clone()),
                ));
            }
        }

        document.close(head, plan, text);
    }
    Ok(())
}

/// Why the plan may not edit the text of the document at `now` (a path after the renames), or
/// `None` when it may. The document's path in `HEAD` is the one it was renamed from.
///
/// `HEAD` may not hold the path ([`REASON_NOT_IN_HEAD`]), the path may lie outside the key space
/// ([`REASON_NOT_IN_SPACE`]), and code, a dot directory or an mdBook may read the file by path
/// (`frozen: <why>`, D-R26).
fn document_refusal(head: &Head, code: &CodeRefs, plan: &ExportPlan, now: &str) -> Option<String> {
    let head_path = head_path_of(plan, now);
    match (head.files.get(&head_path), code.frozen(&head_path)) {
        (None, _) if head.outside.contains(&head_path) => Some(REASON_NOT_IN_SPACE.to_owned()),
        (None, _) => Some(REASON_NOT_IN_HEAD.to_owned()),
        (Some(_), Some(why)) => Some(format!("frozen: {why}")),
        (Some(_), None) => None,
    }
}

/// The path the document at `now` has in `HEAD`: the one a planned rename moved it from, else `now`.
fn head_path_of(plan: &ExportPlan, now: &str) -> String {
    plan.files
        .get(now)
        .and_then(|file| file.renamed_from.clone())
        .unwrap_or_else(|| now.to_owned())
}

/// A document a step edits: the text the steps before it left (the plan's text for the path, else
/// `HEAD`'s), and where to put the result back. Every step that edits a document's text goes
/// through [`open_document`] and [`OpenDocument::close`], so none of them can drop what an earlier
/// step wrote or what the plan knows of the file (its rename and its text in `HEAD`).
struct OpenDocument {
    /// The path after the renames.
    now: String,
    /// The path in `HEAD`.
    head_path: String,
    /// The text before this step.
    base: String,
}

/// The document at `now` for editing, or the reason the plan may not edit it
/// ([`document_refusal`]).
fn open_document(
    head: &Head,
    code: &CodeRefs,
    plan: &ExportPlan,
    now: &str,
) -> Result<OpenDocument, String> {
    if let Some(reason) = document_refusal(head, code, plan, now) {
        return Err(reason);
    }
    let head_path = head_path_of(plan, now);
    let base = plan
        .files
        .get(now)
        .map_or_else(|| head.files[&head_path].clone(), |f| f.new_text.clone());
    Ok(OpenDocument {
        now: now.to_owned(),
        head_path,
        base,
    })
}

impl OpenDocument {
    /// Put `text` back into the plan. The lines the step added take the line ending of the
    /// document in `HEAD` ([`with_line_ends_of`]). Nothing changes when the text then equals the
    /// text the step started from. A path the plan already holds keeps its `renamed_from` and
    /// `head_text` and takes the new text; any other path gets a file whose `head_text` is the text
    /// in `HEAD`.
    fn close(self, head: &Head, plan: &mut ExportPlan, text: String) {
        let head_text = &head.files[&self.head_path];
        let text = with_line_ends_of(head_text, text);
        if text == self.base {
            return;
        }
        match plan.files.get_mut(&self.now) {
            Some(file) => file.new_text = text,
            None => {
                let renamed_from = (self.head_path != self.now).then(|| self.head_path.clone());
                plan.files.insert(
                    self.now.clone(),
                    FileChange {
                        path: self.now,
                        renamed_from,
                        head_text: Some(head_text.clone()),
                        new_text: text,
                    },
                );
            }
        }
    }
}

/// `text` with every bare `\n` written `\r\n` when `head_text` ends every line with `\r\n`, so
/// what a step adds (a section, a frontmatter line) uses the document's own line ending. A
/// document with a bare `\n`, or with no line end at all, gets `text` as it is.
fn with_line_ends_of(head_text: &str, text: String) -> String {
    let lf = head_text.matches('\n').count();
    if lf == 0 || head_text.matches("\r\n").count() != lf {
        return text;
    }
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut after_cr = false;
    for c in text.chars() {
        if c == '\n' && !after_cr {
            out.push('\r');
        }
        out.push(c);
        after_cr = c == '\r';
    }
    out
}

/// The rows of one document that [`plan_events_and_links`] writes.
#[derive(Default)]
struct DocumentRows<'a> {
    /// Events of a kind a text can hold.
    events: Vec<&'a OldEvent>,
    /// Its `note` events: they carry the observations that repeat their text.
    notes: Vec<&'a OldEvent>,
    observations: Vec<&'a OldObservation>,
    /// `(rel, grammar form of the destination)`.
    links: Vec<(String, Option<String>)>,
}

/// Events, observations and manual links (D-R6, D-R12, D-R19), per document path after the renames,
/// into the text the steps before this one left (the citation rewrite, the params entries, the
/// declaration stamp). A document's events and observations go first, then its relation lines. A
/// new event or observation section goes through the citation rewrite like committed text (D-R9),
/// and its citation items follow its own. A relation line holds a label and a `[[key]]` form only,
/// so it has no old form to rewrite.
///
/// A path `HEAD` does not hold, one outside the key space and a frozen document (D-R26) are never
/// edited: each of their rows is an `unrecoverable` item with that reason. The events of a kind
/// that no text holds are reported by [`plan_skipped_events`]; the others are counted here, so
/// every row of the three classes has one item.
///
/// A destination's form is the one its document has after the renames in the index of the final
/// paths ([`KeyIndex`] over `HEAD`, D-R11), and only when that index resolves it to a file.
fn plan_events_and_links(
    old: &OldRows,
    head: &Head,
    code: &CodeRefs,
    tables: &CitationTables,
    plan: &mut ExportPlan,
) {
    let moved = &tables.moved;
    let index = &tables.index;
    let resolver = tables.resolver(&head.repo_name);
    let form_of = |artifact_id: &str| -> Option<String> {
        let document = old.documents.get(artifact_id)?;
        let key = index.key_at(final_path(moved, &document.rel_path))?;
        let reference = Reference::File {
            repo: None,
            key: key.clone(),
        };
        matches!(
            index.resolve(&document.rel_path, &reference),
            Resolution::File { .. }
        )
        .then(|| format!("[[{key}]]"))
    };

    let mut by_path: BTreeMap<String, DocumentRows> = BTreeMap::new();
    let path_of = |artifact_id: &str| {
        old.documents
            .get(artifact_id)
            .map(|document| final_path(moved, &document.rel_path).to_owned())
    };
    for event in &old.events {
        let Some(now) = path_of(&event.artifact_id) else {
            continue;
        };
        let rows = by_path.entry(now).or_default();
        if event.kind == "note" {
            rows.notes.push(event);
        }
        if no_text_home(&event.kind).is_none() {
            rows.events.push(event);
        }
    }
    for row in &old.observations {
        if let Some(now) = path_of(&row.artifact_id) {
            by_path.entry(now).or_default().observations.push(row);
        }
    }
    for link in &old.links {
        if let Some(now) = path_of(&link.src_id) {
            by_path
                .entry(now)
                .or_default()
                .links
                .push((link.rel.clone(), form_of(&link.dst_id)));
        }
    }

    for (now, rows) in by_path {
        let document = match open_document(head, code, plan, &now) {
            Ok(document) => document,
            Err(reason) => {
                plan.items.extend(refuse_events(
                    &now,
                    &rows.events,
                    &rows.observations,
                    &reason,
                ));
                plan.items.extend(refuse_links(&now, &rows.links, &reason));
                continue;
            }
        };
        let rewrite = added_text_rewrite(&resolver, &now, plan.shallow);
        let (text, items) = export_events(
            &now,
            &document.base,
            &rows.events,
            &rows.observations,
            &rows.notes,
            &form_of,
            &rewrite,
        );
        plan.items.extend(items);
        let (text, items) = export_links(&now, &text, &rows.links);
        plan.items.extend(items);
        document.close(head, plan, text);
    }
}

/// The item of one augmentation's shape; `detail` is the sidecar's path.
fn shape_item(path: &str, status: Status, reason: &str, detail: Option<String>) -> Item {
    Item {
        class: Class::AugmentationShape,
        status,
        reason: reason.to_owned(),
        path: Some(path.to_owned()),
        line: None,
        detail,
        suggestion: None,
    }
}

/// Markdown outside the key space (and so outside the rewrite, D-R15) that links to a file this run
/// renames: the link breaks and no rewrite can fix it, so each one is an `unrecoverable` item
/// (D-R7). The file is not rewritten and the rename is not blocked (D-R21 b blocks on code only).
/// A path with no text (not UTF-8, a symlink) has nothing to scan. Items come sorted by path, then
/// line, one per line and renamed file.
///
/// A file is found two ways: the paths its text names (`paths_named_in`), and its link
/// destinations, percent-decoded and resolved against the folder of the file that holds them (a
/// name with a space is written `My%20Notes.md` or `<My Notes.md>`, which no path run reads).
fn plan_outside_links(
    repo: &git2::Repository,
    commit: &git2::Commit,
    head: &Head,
    renames: &RenameSet,
    plan: &mut ExportPlan,
) -> anyhow::Result<()> {
    if renames.planned.is_empty() {
        return Ok(());
    }
    let tree = commit.tree()?;
    for path in &head.outside {
        let Ok(entry) = tree.get_path(Path::new(path)) else {
            continue;
        };
        if entry.kind() != Some(git2::ObjectType::Blob) || entry.filemode() == LINK_MODE {
            continue;
        }
        let Ok(blob) = repo.find_blob(entry.id()) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(blob.content()) else {
            continue;
        };
        let named = paths_named_in(path, text);
        let mut hits: BTreeSet<(u32, usize)> = BTreeSet::new();
        for (n, rename) in renames.planned.iter().enumerate() {
            for (_, line) in named.get(&rename.from).into_iter().flatten() {
                hits.insert((*line, n));
            }
        }
        let folder = folder_of(path);
        for span in citation_spans(text)
            .iter()
            .filter(|s| s.kind == CitationKind::RelPathLink)
        {
            let Some(target) = join_path(&folder, &percent_decoded(&span.raw)) else {
                continue;
            };
            for (n, rename) in renames.planned.iter().enumerate() {
                if rename.from == target {
                    hits.insert((span.line, n));
                }
            }
        }
        for (line, n) in hits {
            let rename = &renames.planned[n];
            plan.items.push(Item {
                class: Class::Citation,
                status: Status::Unrecoverable,
                reason: "outside the key space: links to a file this run renames".to_owned(),
                path: Some(path.clone()),
                line: Some(line),
                detail: Some(format!("{} -> {}", rename.from, rename.to)),
                suggestion: Some(rename.to.clone()),
            });
        }
    }
    Ok(())
}

/// `s` with each `%XX` byte sequence decoded; `s` itself when the result is not UTF-8.
fn percent_decoded(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

/// The path of the document written at `path` in `HEAD`, after the renames.
fn final_path<'a>(moved: &'a BTreeMap<String, String>, path: &'a str) -> &'a str {
    moved.get(path).map_or(path, String::as_str)
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

/// Each file stem of `HEAD` (as written in the file name) to the final paths of the files that
/// carried it. The old stem of a file `HEAD` already holds under its new name names it too.
fn stems_of(head: &Head, renames: &RenameSet) -> BTreeMap<String, Vec<String>> {
    let moved = renames.paths();
    let mut stems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in head.files.keys() {
        let name = path.rsplit('/').next().unwrap_or(path);
        if let Some(stem) = markdown_stem(name) {
            stems
                .entry(stem.to_owned())
                .or_default()
                .push(final_path(&moved, path).to_owned());
        }
    }
    for rename in &renames.in_head {
        let name = rename.from.rsplit('/').next().unwrap_or(&rename.from);
        if let Some(stem) = markdown_stem(name) {
            let paths = stems.entry(stem.to_owned()).or_default();
            if !paths.contains(&rename.to) {
                paths.push(rename.to.clone());
            }
        }
    }
    stems
}

/// What each old artifact id names. The old catalog first (`Catalog`): an id of a document that
/// `HEAD` holds is its target, at the path the document has after the renames. Then, for every
/// cited id with no target yet, git history (`History`): the file may have moved since the catalog
/// saw it. Last, an id of a document `HEAD` lacks that history did not settle is a hint with the
/// path the catalog knew. A shallow clone has incomplete history and skips it: an id nothing else
/// names is reported so.
fn recover_ids(
    repo: &git2::Repository,
    commit: &git2::Commit,
    old: &OldRows,
    head: &Head,
    moved: &BTreeMap<String, String>,
    shallow: bool,
) -> anyhow::Result<(BTreeMap<String, IdTarget>, BTreeMap<String, String>)> {
    let mut ids = BTreeMap::new();
    let mut hints = BTreeMap::new();
    // The path the catalog knew for each id whose file `HEAD` lacks.
    let mut gone: BTreeMap<&str, &str> = BTreeMap::new();
    for doc in old.documents.values() {
        // `HEAD` holds the file under its old name (this run renames it) or already under its new
        // one (a run after the commit).
        let now = final_path(moved, &doc.rel_path);
        if head.files.contains_key(&doc.rel_path) || head.files.contains_key(now) {
            ids.entry(doc.id.clone()).or_insert(IdTarget {
                rel_path: now.to_owned(),
                source: IdSource::Catalog,
            });
        } else {
            gone.entry(doc.id.as_str()).or_insert(doc.rel_path.as_str());
        }
    }
    // An id with no target in `HEAD` is still open, whether or not the catalog knew its file:
    // git may show where the file went. The id binds to the path it hashes and to nothing else.
    let open: BTreeSet<String> = cited_ids(head, old)
        .into_iter()
        .filter(|id| !ids.contains_key(id))
        .collect();
    if !shallow && !open.is_empty() {
        let spellings: Vec<&Path> = old.spellings.iter().map(PathBuf::as_path).collect();
        let history = HistoryIds::load(repo, commit, &spellings)?;
        for id in &open {
            match history.head_path(id) {
                Some(Ok(rel_path)) => {
                    let now = final_path(moved, &rel_path).to_owned();
                    ids.insert(
                        id.clone(),
                        IdTarget {
                            rel_path: now,
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

/// The 16-hex ids cited outside code: by the in-space documents of `HEAD`, and by the text an
/// export step may add to one ([`added_texts`]). The plan of the commit that holds the export reads
/// that text in `HEAD`, so the first plan binds its ids through history as well (D-R28), and the
/// two plans agree.
fn cited_ids(head: &Head, old: &OldRows) -> BTreeSet<String> {
    let added = added_texts(old);
    head.files
        .values()
        .chain(&added)
        .flat_map(|text| citation_spans(text))
        .filter(|s| s.kind == CitationKind::ArtifactId && !s.in_code && s.range.is_some())
        .map(|s| s.raw)
        .collect()
}

/// The texts of the old rows that an export step may write as prose: the `text` of an event's
/// payload, an observation's text, and the `title` and `summary` of a params entry (its heading).
fn added_texts(old: &OldRows) -> Vec<String> {
    let mut texts: Vec<String> = old
        .events
        .iter()
        .filter_map(|event| event.payload.get("text")?.as_str().map(str::to_owned))
        .collect();
    texts.extend(old.observations.iter().map(|row| row.text.clone()));
    for row in &old.augmentations {
        let (Params::Object(params), Some(collection)) = (params_of(row), &row.entry_collection)
        else {
            continue;
        };
        let entries = params.get(collection).and_then(|v| v.as_array());
        for entry in entries.into_iter().flatten() {
            for key in ["title", "summary"] {
                if let Some(text) = entry.get(key).and_then(|v| v.as_str()) {
                    texts.push(text.to_owned());
                }
            }
        }
    }
    texts
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

/// One `skipped` item per event of a kind that no text holds. The path is the document's path
/// after the renames: its rows follow the file.
fn plan_skipped_events(old: &OldRows, moved: &BTreeMap<String, String>, plan: &mut ExportPlan) {
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
                .map(|d| final_path(moved, &d.rel_path).to_owned()),
            line: None,
            detail: Some(event.kind.clone()),
            suggestion: None,
        });
    }
}

/// One `skipped` item per write-origin entry citation: they are report-only, and the tally counts
/// rows, so a class with `n` rows has `n` items.
fn plan_write_cites(old: &OldRows, plan: &mut ExportPlan) {
    for _ in 0..old.write_cites {
        plan.items.push(Item {
            class: Class::WriteCite,
            status: Status::Skipped,
            reason: "an entry citation written by a tool call stays a catalog row".to_owned(),
            path: None,
            line: None,
            detail: None,
            suggestion: None,
        });
    }
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
/// that are not JSON, or not an object, count as one other row, the one `other_params` reports;
/// empty params count as nothing.
fn params_counts(aug: &AugmentationRow) -> (usize, usize) {
    let params = match params_of(aug) {
        Params::Empty => return (0, 0),
        Params::NotJson | Params::NotObject => return (0, 1),
        Params::Object(params) => params,
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
        assert_eq!(
            reason_of("note"),
            ["no committed home yet: not a single-writer document"],
            "a note has a text home in a single-writer document only: a.md is not one"
        );
        assert_eq!(plan.items.len(), 6);
        for item in &plan.items {
            assert_eq!(item.status, Status::Skipped);
            assert_eq!(item.path.as_deref(), Some("a.md"));
        }
        let tally = plan.tally();
        let events = &tally[&Class::Event];
        assert_eq!(events.skipped, 6);
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
        assert_eq!(cites.len(), 3, "one item per row, so the tally counts rows");
        assert!(cites.iter().all(|c| c.status == Status::Skipped));
        assert!(cites.iter().all(|c| c.detail.is_none()));
        assert_eq!(plan.tally()[&Class::WriteCite].skipped, 3);
        assert_eq!(
            plan.tally()[&Class::WriteCite]
                .reasons
                .values()
                .sum::<usize>(),
            3
        );
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

    fn rename_items(plan: &ExportPlan) -> Vec<&Item> {
        plan.items
            .iter()
            .filter(|i| i.class == Class::Rename)
            .collect()
    }

    #[test]
    fn a_renamed_file_appears_once_under_its_new_path() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                ("docs/b.md", b"# B\n"),
            ],
            |_, _| {},
        );
        let paths: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(paths, ["docs/my-notes.md"], "one entry, under the new path");
        let file = &plan.files["docs/my-notes.md"];
        assert_eq!(file.path, "docs/my-notes.md");
        assert_eq!(file.renamed_from.as_deref(), Some("docs/My_Notes.md"));
        assert_eq!(
            file.head_text.as_deref(),
            Some("# My\n\n## F-2 \u{2014} two\n")
        );
        assert_eq!(file.new_text, "# My\n\n## F-2 \u{2014} two\n");
        assert_eq!(
            plan.writes(),
            1,
            "a rename is a write even with the text unchanged"
        );
        let items = rename_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].status, Status::Moved);
        assert_eq!(items[0].path.as_deref(), Some("docs/My_Notes.md"));
        assert_eq!(
            items[0].detail.as_deref(),
            Some("docs/My_Notes.md -> docs/my-notes.md")
        );
    }

    #[test]
    fn a_rewrite_inside_a_renamed_file_is_reported_under_its_new_path() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n\nsee F-9.\n"),
                ("docs/ledger.md", b"# L\n\n## F-9 \xe2\x80\x94 nine\n"),
            ],
            |_, _| {},
        );
        let file = &plan.files["docs/my-notes.md"];
        assert_eq!(file.renamed_from.as_deref(), Some("docs/My_Notes.md"));
        assert_eq!(file.new_text, "# My\n\nsee [[ledger:F-9]].\n");
        assert_eq!(
            file.head_text.as_deref(),
            Some("# My\n\nsee F-9.\n"),
            "the text of HEAD is kept to compare the working tree against"
        );
        let items = citation_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].path.as_deref(), Some("docs/my-notes.md"));
        assert_eq!(plan.files.len(), 1);
    }

    #[test]
    fn a_bare_token_is_rendered_against_the_final_paths() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                ("docs/b.md", b"# B\n\nsee F-2 and [[My_Notes:F-2]].\n"),
            ],
            |_, _| {},
        );
        assert_eq!(
            plan.files["docs/b.md"].new_text, "# B\n\nsee [[my-notes:F-2]] and [[my-notes:F-2]].\n",
            "never [[my_notes:F-2]]: the key of the file after the rename"
        );
    }

    #[test]
    fn every_old_form_of_a_renamed_file_follows_it() {
        let plan = plan_of_files(
            &[
                ("docs/my_notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                (
                    "docs/b.md",
                    b"# B\n\n[[my_notes]] my_notes:F-2 [n](my_notes.md) 0123456789abcdef\n",
                ),
            ],
            |cat, root| {
                add_doc(cat, root, "0123456789abcdef", "docs/my_notes.md");
            },
        );
        assert_eq!(
            plan.files["docs/b.md"].new_text,
            "# B\n\n[[my-notes]] [[my-notes:F-2]] [[my-notes|n]] [[my-notes]]\n"
        );
        let open: Vec<&Item> = citation_items(&plan)
            .into_iter()
            .filter(|i| i.status != Status::Moved)
            .collect();
        assert!(open.is_empty(), "{open:?}");
    }

    #[test]
    fn a_refused_rename_leaves_the_citations_of_the_file_alone() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                (
                    "docs/b.md",
                    b"# B\n\n[[My_Notes]] and [n](My_Notes.md) and F-2.\n",
                ),
                ("src/x.rs", b"const P: &str = \"../docs/My_Notes.md\";\n"),
            ],
            |_, _| {},
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let items = rename_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].status, Status::Unrecoverable);
        assert!(
            items[0].reason.starts_with("frozen: "),
            "{}",
            items[0].reason
        );
        assert!(
            items[0].reason.contains("src/x.rs:1"),
            "{}",
            items[0].reason
        );
        // Nothing is rewritten: the path link has no target with a key, and the one document
        // that defines the token has none either.
        let reasons: Vec<&str> = citation_items(&plan)
            .iter()
            .map(|i| i.reason.as_str())
            .collect();
        assert_eq!(
            reasons,
            [
                "the target is not a document with a unique key",
                "the one document that defines the token has no unique key"
            ],
            "{reasons:?}"
        );
    }

    #[test]
    fn a_target_outside_the_key_space_is_taken() {
        let plan = plan_of_files(
            &[
                (
                    ".codescout/key-space.toml",
                    b"exclude = [\"**/my-notes.md\"]\n",
                ),
                ("docs/My_Notes.md", b"# My\n"),
                ("docs/my-notes.md", b"# Vendored\n"),
            ],
            |_, _| {},
        );
        assert!(plan.files.is_empty());
        let items = rename_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].reason, "target exists");
    }

    #[test]
    fn rows_of_a_renamed_document_follow_it_after_the_commit() {
        // `HEAD` already holds the rename: the old catalog still knows `docs/My_Notes.md`.
        let plan = plan_of_files(
            &[
                ("docs/my-notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                (
                    "docs/b.md",
                    b"# B\n\nsee [[My_Notes]] and 0123456789abcdef and [n](My_Notes.md) and My_Notes:F-2.\n",
                ),
            ],
            |cat, root| {
                add_doc(cat, root, "0123456789abcdef", "docs/My_Notes.md");
                add_event(cat, "e1", "0123456789abcdef", "field_patch", "{}");
            },
        );
        assert_eq!(
            plan.files["docs/b.md"].new_text,
            "# B\n\nsee [[my-notes]] and [[my-notes]] and [[my-notes|n]] and My_Notes:F-2.\n"
        );
        assert_eq!(plan.files.len(), 1, "no file is renamed again");
        let renames = rename_items(&plan);
        assert_eq!(renames.len(), 1, "{renames:?}");
        assert_eq!(renames[0].status, Status::AlreadyExported);
        assert_eq!(renames[0].path.as_deref(), Some("docs/my-notes.md"));
        assert_eq!(
            renames[0].detail.as_deref(),
            Some("docs/My_Notes.md -> docs/my-notes.md")
        );
        // The rows land on the new path, and no citation says the document is gone.
        let skipped: Vec<&Item> = plan
            .items
            .iter()
            .filter(|i| i.class == Class::Event)
            .collect();
        assert_eq!(skipped.len(), 1, "{skipped:?}");
        assert_eq!(skipped[0].path.as_deref(), Some("docs/my-notes.md"));
        let gone: Vec<&Item> = citation_items(&plan)
            .into_iter()
            .filter(|i| i.reason.contains("no longer holds"))
            .collect();
        assert!(gone.is_empty(), "{gone:?}");
    }

    #[test]
    fn a_blocker_of_one_rename_is_a_report_line_and_the_others_still_happen() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n"),
                ("docs/Other_Notes.md", b"# O\n"),
                ("scripts/run.sh", b"cat docs/Other_Notes.md\n"),
            ],
            |_, _| {},
        );
        let paths: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(paths, ["docs/my-notes.md"]);
        let blocked: Vec<&Item> = rename_items(&plan)
            .into_iter()
            .filter(|i| i.status == Status::Unrecoverable)
            .collect();
        assert_eq!(blocked.len(), 1, "{blocked:?}");
        assert_eq!(blocked[0].path.as_deref(), Some("docs/Other_Notes.md"));
    }

    #[test]
    fn a_history_target_this_run_renames_is_followed_to_its_new_path() {
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
        // Git moved the file to a name that is no key; this run renames it again.
        let tip = commit_files(
            &repo,
            &[
                ("docs/New_Notes.md", BODY),
                ("docs/a.md", a_text.as_bytes()),
            ],
            &[&first],
            "move",
        );
        let old = old_catalog_with(|_| {});
        let rows = load(&old, &RootMap::new(root.clone())).unwrap();
        let commit = repo.find_commit(tip).unwrap();
        let plan = plan_export(&repo, &commit, &rows).unwrap();
        assert_eq!(
            plan.files["docs/a.md"].new_text,
            "# A\n\nsee [[new-notes]].\n"
        );
        assert_eq!(
            plan.files["docs/new-notes.md"].renamed_from.as_deref(),
            Some("docs/New_Notes.md")
        );
    }

    #[test]
    fn a_stem_token_of_a_file_head_already_renamed_follows_it() {
        let plan = plan_of_files(
            &[
                ("docs/my-notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                ("docs/b.md", b"# B\n\nsee my_notes:F-2 and [[my_notes]].\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "0123456789abcdef", "docs/my_notes.md");
            },
        );
        assert_eq!(
            plan.files["docs/b.md"].new_text,
            "# B\n\nsee [[my-notes:F-2]] and [[my-notes]].\n"
        );
    }

    #[test]
    fn a_target_with_no_text_is_taken() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n"),
                ("docs/my-notes.md", b"\xff\xfe not text"),
            ],
            |_, _| {},
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let items = rename_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].reason, "target exists");
    }

    fn ambiguous_items(plan: &ExportPlan) -> Vec<&Item> {
        citation_items(plan)
            .into_iter()
            .filter(|i| i.reason == "ambiguous: several documents qualify")
            .collect()
    }

    #[test]
    fn an_old_key_a_refused_file_also_has_is_not_rewritten_to_the_renamed_one() {
        // `.github/My_Notes.md` is frozen and keeps its invalid key: `[[My_Notes]]` could mean either.
        let plan = plan_of_files(
            &[
                (".github/My_Notes.md", b"# G\n"),
                ("docs/My_Notes.md", b"# My\n\n## F-2 \xe2\x80\x94 two\n"),
                ("docs/b.md", b"# B\n\nsee [[My_Notes]] and My_Notes:F-2.\n"),
            ],
            |_, _| {},
        );
        let paths: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(paths, ["docs/my-notes.md"], "docs/b.md keeps its text");
        let ambiguous = ambiguous_items(&plan);
        assert_eq!(ambiguous.len(), 1, "{ambiguous:?}");
        assert_eq!(ambiguous[0].path.as_deref(), Some("docs/b.md"));
        assert_eq!(
            ambiguous[0].detail.as_deref(),
            Some("[[My_Notes]]: .github/My_Notes.md, docs/My_Notes.md")
        );
    }

    #[test]
    fn a_rename_already_in_head_next_to_a_refused_file_leaves_the_old_key_ambiguous() {
        // `notes/My_Notes.md` is refused (its key is held by `docs/my-notes.md`), so the old key
        // `my_notes` names it as well as the file the catalog knew at `docs/My_Notes.md`.
        let plan = plan_of_files(
            &[
                ("docs/my-notes.md", b"# My\n"),
                ("notes/My_Notes.md", b"# Other\n"),
                ("docs/b.md", b"# B\n\nsee [[My_Notes]].\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "0123456789abcdef", "docs/My_Notes.md");
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let ambiguous = ambiguous_items(&plan);
        assert_eq!(ambiguous.len(), 1, "{ambiguous:?}");
        assert_eq!(
            ambiguous[0].detail.as_deref(),
            Some("[[My_Notes]]: docs/my-notes.md, notes/My_Notes.md")
        );
        let in_head = rename_items(&plan)
            .into_iter()
            .filter(|i| i.status == Status::AlreadyExported)
            .count();
        assert_eq!(in_head, 1);
    }

    #[test]
    fn a_link_in_a_file_outside_the_key_space_to_a_renamed_file_is_reported() {
        let plan = plan_of_files(
            &[
                (".codescout/key-space.toml", b"exclude = [\"contrib/**\"]\n"),
                ("docs/My_Notes.md", b"# My\n"),
                ("docs/Kept_Name.md", b"# K\n"),
                (
                    "contrib/x.md",
                    b"# X\n\nsee [n](../docs/My_Notes.md) and\n[k](../docs/Fine.md) and [again](../docs/My_Notes.md#s).\n",
                ),
                ("contrib/sub/y.md", b"# Y\n\n[n](../../docs/My_Notes.md)\n"),
                ("contrib/untouched.md", b"# U\n\n[o](other/My_Notes.md)\n"),
                ("contrib/binary.md", b"\xff\xfe ../docs/My_Notes.md"),
            ],
            |_, _| {},
        );
        let paths: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(
            paths,
            ["docs/kept-name.md", "docs/my-notes.md"],
            "the files outside are not rewritten, and the renames still happen"
        );
        let hits: Vec<(&str, Option<u32>)> = citation_items(&plan)
            .into_iter()
            .filter(|i| i.status == Status::Unrecoverable)
            .map(|i| (i.path.as_deref().unwrap(), i.line))
            .collect();
        assert_eq!(
            hits,
            [
                ("contrib/sub/y.md", Some(3)),
                ("contrib/x.md", Some(3)),
                ("contrib/x.md", Some(4)),
            ]
        );
        let item = citation_items(&plan)
            .into_iter()
            .find(|i| i.path.as_deref() == Some("contrib/x.md"))
            .unwrap();
        assert_eq!(
            item.reason,
            "outside the key space: links to a file this run renames"
        );
        assert_eq!(
            item.detail.as_deref(),
            Some("docs/My_Notes.md -> docs/my-notes.md")
        );
    }

    #[test]
    fn a_symlink_outside_the_key_space_is_not_scanned_for_links() {
        use crate::librarian::derived::testkit::LINK;
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let repo = repo_with(
            &root,
            &[
                (
                    ".codescout/key-space.toml",
                    FILE,
                    b"exclude = [\"contrib/**\"]\n" as &[u8],
                ),
                ("docs/My_Notes.md", FILE, b"# My\n"),
                // The blob of a link holds its target, a path: not text that links anywhere.
                ("contrib/link.md", LINK, b"../docs/My_Notes.md"),
            ],
        );
        let old = old_catalog_with(|_| {});
        let rows = load(&old, &RootMap::new(root.clone())).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let plan = plan_export(&repo, &commit, &rows).unwrap();
        assert!(citation_items(&plan).is_empty(), "{:?}", plan.items);
        assert_eq!(plan.files.keys().collect::<Vec<_>>(), ["docs/my-notes.md"]);
    }

    #[test]
    fn a_folder_index_whose_folder_gives_the_old_key_is_a_competitor() {
        // `guides/My_Notes/README.md` has the key `my_notes` (its folder's) and its rename is refused
        // (a folder index): `[[My_Notes]]` could mean it as well as `docs/My_Notes.md`.
        let plan = plan_of_files(
            &[
                ("guides/My_Notes/README.md", b"# G\n"),
                ("docs/My_Notes.md", b"# My\n"),
                ("docs/b.md", b"# B\n\nsee [[My_Notes]].\n"),
            ],
            |_, _| {},
        );
        let paths: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(paths, ["docs/my-notes.md"], "docs/b.md keeps its text");
        let ambiguous = ambiguous_items(&plan);
        assert_eq!(ambiguous.len(), 1, "{ambiguous:?}");
        assert_eq!(
            ambiguous[0].detail.as_deref(),
            Some("[[My_Notes]]: docs/My_Notes.md, guides/My_Notes/README.md")
        );
    }

    #[test]
    fn a_canonical_folder_index_is_no_competitor() {
        let plan = plan_of_files(
            &[
                ("guides/notes/README.md", b"# G\n"),
                ("docs/My_Notes.md", b"# My\n"),
                ("docs/b.md", b"# B\n\nsee [[My_Notes]].\n"),
            ],
            |_, _| {},
        );
        assert_eq!(
            plan.files["docs/b.md"].new_text,
            "# B\n\nsee [[my-notes]].\n"
        );
        assert!(ambiguous_items(&plan).is_empty());
    }

    #[test]
    fn a_link_to_a_renamed_file_with_a_space_in_its_name_is_found_decoded() {
        let plan = plan_of_files(
            &[
                (".codescout/key-space.toml", b"exclude = [\"contrib/**\"]\n"),
                ("docs/My Notes.md", b"# My\n"),
                ("contrib/a.md", b"# A\n\n[x](../docs/My%20Notes.md)\n"),
                ("contrib/b.md", b"# B\n\n[x](<../docs/My Notes.md>) and [y](../docs/My%20Notes.md#s)\n"),
                // Only the tail matches: from this folder the path is another file's.
                ("contrib/c.md", b"# C\n\n[x](other/docs/My%20Notes.md) and [y](My%20Notes.md)\n"),
                ("contrib/d.md", b"# D\n\n[x](../docs/My%20Notes.md) [y](../docs/My%20Notes.md)\n[z](<../docs/My Notes.md>)\n"),
            ],
            |_, _| {},
        );
        assert_eq!(
            plan.files.keys().collect::<Vec<_>>(),
            ["docs/my-notes.md"],
            "the name with a space is renamed"
        );
        let hits: Vec<(&str, Option<u32>)> = citation_items(&plan)
            .into_iter()
            .filter(|i| i.status == Status::Unrecoverable)
            .map(|i| (i.path.as_deref().unwrap(), i.line))
            .collect();
        assert_eq!(
            hits,
            [
                ("contrib/a.md", Some(3)),
                ("contrib/b.md", Some(3)),
                ("contrib/d.md", Some(3)),
                ("contrib/d.md", Some(4)),
            ],
            "one item per line and renamed file, in order of path then line"
        );
    }

    #[test]
    fn a_live_old_catalog_row_at_the_target_blocks_the_rename() {
        let fill = |stale: bool| {
            move |cat: &crate::librarian::catalog::Catalog, root: &Path| {
                add_doc(cat, root, "0000000000000001", "docs/My_Notes.md");
                add_doc(cat, root, "0000000000000002", "docs/my-notes.md");
                if stale {
                    cat.conn
                        .execute(
                            "UPDATE artifact SET missing_since = 5 WHERE id = '0000000000000002'",
                            [],
                        )
                        .unwrap();
                }
            }
        };
        let files: &[(&str, &[u8])] = &[("docs/My_Notes.md", b"# My\n")];
        let plan = plan_of_files(files, fill(false));
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let items = rename_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].reason, "target exists");
        assert!(
            items[0].detail.as_deref().unwrap().contains("old catalog"),
            "{:?}",
            items[0].detail
        );
        // A row the catalog marked missing is a leftover, not a document.
        let plan = plan_of_files(files, fill(true));
        assert_eq!(plan.files.keys().collect::<Vec<_>>(), ["docs/my-notes.md"]);
    }

    #[test]
    fn a_rename_blocked_by_code_names_the_first_place_and_the_count_in_the_detail() {
        let plan = plan_of_files(
            &[
                ("docs/My_Notes.md", b"# My\n"),
                ("scripts/a.sh", b"cat My_Notes.md\n"),
                ("scripts/b.sh", b"cat My_Notes.md\n"),
                ("scripts/c.sh", b"cat My_Notes.md\n"),
            ],
            |_, _| {},
        );
        let items = rename_items(&plan);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].reason, "named by code");
        assert_eq!(
            items[0].detail.as_deref(),
            Some("scripts/a.sh:1 and 2 more; would be docs/my-notes.md")
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

    const LEDGER_MD: &str =
        "---\nkind: tracker\nentry_prefix: F\n---\n\n# L\n\n## F-1 — one\n\nx\n";

    fn items_of(plan: &ExportPlan, class: Class) -> Vec<&Item> {
        plan.items.iter().filter(|i| i.class == class).collect()
    }

    fn statuses_of(plan: &ExportPlan, class: Class) -> Vec<Status> {
        items_of(plan, class).iter().map(|i| i.status).collect()
    }

    /// The declaration of a document text, as `expects_augmentation` reads in its frontmatter.
    fn declared_in(text: &str) -> Option<String> {
        let (fm, _) = crate::librarian::frontmatter::parse(text).ok()?;
        fm?.extra
            .get("expects_augmentation")
            .and_then(|v| v.as_str().map(str::to_owned))
    }

    #[test]
    fn a_ledger_with_an_entry_and_a_shape_changes_once_and_gains_one_sidecar() {
        let plan = plan_of_files(&[("docs/ledger.md", LEDGER_MD.as_bytes())], |cat, root| {
            add_doc(cat, root, "led", "docs/ledger.md");
            add_augmentation(
                cat,
                "led",
                r#"{"entries":[{"id":"F-1"},{"id":"F-2","title":"two"}],"gather":"x"}"#,
                Some("entries"),
            );
        });
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(
            files,
            ["docs/augmentations/docs-ledger.yaml", "docs/ledger.md"]
        );
        assert_eq!(plan.writes(), 2);

        let ledger = &plan.files["docs/ledger.md"];
        assert_eq!(ledger.head_text.as_deref(), Some(LEDGER_MD));
        assert_eq!(ledger.renamed_from, None);
        assert!(
            ledger.new_text.contains("\n## F-2 — two\n"),
            "{}",
            ledger.new_text
        );
        assert!(
            ledger.new_text.contains("entry_high_water_F: 2"),
            "{}",
            ledger.new_text
        );
        assert_eq!(
            declared_in(&ledger.new_text).as_deref(),
            Some("docs/augmentations/docs-ledger.yaml")
        );

        let sidecar = &plan.files["docs/augmentations/docs-ledger.yaml"];
        assert_eq!(sidecar.head_text, None);
        assert_eq!(sidecar.renamed_from, None);
        let shape: crate::librarian::augmentation_sidecar::AugmentationSidecar =
            serde_yml::from_str(&sidecar.new_text).unwrap();
        assert_eq!(shape.prompt, "keep it current");
        assert_eq!(shape.entry_collection.as_deref(), Some("entries"));

        assert_eq!(
            statuses_of(&plan, Class::ParamsEntry),
            [Status::AlreadyExported, Status::Moved]
        );
        assert_eq!(
            statuses_of(&plan, Class::AugmentationShape),
            [Status::Moved]
        );
        let other = items_of(&plan, Class::ParamsOther);
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].status, Status::Unrecoverable);
        assert_eq!(other[0].detail.as_deref(), Some("gather"));
        assert!(items_of(&plan, Class::ParamsEntry)
            .iter()
            .all(|i| i.path.as_deref() == Some("docs/ledger.md")));
        // Every entry of the population has an outcome.
        let tally = plan.tally();
        let outcomes = |class: Class| {
            let t = &tally[&class];
            t.moved + t.already_exported + t.skipped + t.unrecoverable
        };
        assert_eq!(
            outcomes(Class::ParamsEntry),
            plan.population["params_entry"]
        );
        assert_eq!(
            outcomes(Class::AugmentationShape),
            plan.population["augmentation_shape"]
        );
    }

    #[test]
    fn a_renamed_ledger_gets_its_entries_and_sidecar_under_the_new_path() {
        // The text goes through the citation rewrite first, then the entries, then the stamp; the
        // rows land on the path the document has after the rename, and so does the sidecar's name.
        let text = "---\nkind: tracker\nentry_prefix: F\n---\n\n# L\n\nsee [[my_notes]]\n\n## F-1 — one\n\nx\n";
        let plan = plan_of_files(&[("docs/my_notes.md", text.as_bytes())], |cat, root| {
            add_doc(cat, root, "led", "docs/my_notes.md");
            add_augmentation(
                cat,
                "led",
                r#"{"entries":[{"id":"F-2","title":"two"}]}"#,
                Some("entries"),
            );
        });
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(
            files,
            ["docs/augmentations/docs-my-notes.yaml", "docs/my-notes.md"]
        );
        let ledger = &plan.files["docs/my-notes.md"];
        assert_eq!(ledger.renamed_from.as_deref(), Some("docs/my_notes.md"));
        assert_eq!(ledger.head_text.as_deref(), Some(text));
        assert!(
            ledger.new_text.contains("see [[my-notes]]"),
            "{}",
            ledger.new_text
        );
        assert!(
            ledger.new_text.contains("\n## F-2 — two\n"),
            "{}",
            ledger.new_text
        );
        assert_eq!(
            declared_in(&ledger.new_text).as_deref(),
            Some("docs/augmentations/docs-my-notes.yaml")
        );
        for item in items_of(&plan, Class::ParamsEntry)
            .into_iter()
            .chain(items_of(&plan, Class::AugmentationShape))
        {
            assert_eq!(item.path.as_deref(), Some("docs/my-notes.md"), "{item:?}");
        }
    }

    #[test]
    fn an_augmentation_whose_document_is_not_in_head_is_unrecoverable() {
        let plan = plan_of_files(&[("docs/ledger.md", LEDGER_MD.as_bytes())], |cat, root| {
            add_doc(cat, root, "gone", "docs/gone.md");
            add_augmentation(
                cat,
                "gone",
                r#"{"entries":[{"id":"F-2"},{"id":"F-3"}],"extra":1}"#,
                Some("entries"),
            );
        });
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let entries = items_of(&plan, Class::ParamsEntry);
        assert_eq!(entries.len(), 2);
        for item in entries
            .into_iter()
            .chain(items_of(&plan, Class::AugmentationShape))
        {
            assert_eq!(item.status, Status::Unrecoverable, "{item:?}");
            assert_eq!(item.reason, "document not in HEAD");
            assert_eq!(item.path.as_deref(), Some("docs/gone.md"));
        }
        assert_eq!(items_of(&plan, Class::AugmentationShape).len(), 1);
        assert_eq!(
            statuses_of(&plan, Class::ParamsOther),
            [Status::Unrecoverable]
        );
    }

    #[test]
    fn a_frozen_document_gets_frozen_items_and_keeps_its_text() {
        let plan = plan_of_files(
            &[
                ("docs/ledger.md", LEDGER_MD.as_bytes()),
                ("src/x.rs", b"const P: &str = \"../docs/ledger.md\";\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "led", "docs/ledger.md");
                add_augmentation(
                    cat,
                    "led",
                    r#"{"entries":[{"id":"F-2"}],"extra":1}"#,
                    Some("entries"),
                );
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        for class in [Class::ParamsEntry, Class::AugmentationShape] {
            let items = items_of(&plan, class);
            assert_eq!(items.len(), 1, "{class:?}");
            assert_eq!(items[0].status, Status::Unrecoverable);
            assert!(
                items[0].reason.starts_with("frozen: "),
                "{}",
                items[0].reason
            );
            assert_eq!(items[0].path.as_deref(), Some("docs/ledger.md"));
        }
        assert_eq!(
            statuses_of(&plan, Class::ParamsOther),
            [Status::Unrecoverable]
        );
    }
    const SINGLE_WRITER_MD: &str = "---\nsingle_writer: true\n---\n\n# Notes\n";

    fn add_observation(cat: &crate::librarian::catalog::Catalog, artifact_id: &str, text: &str) {
        observations::insert(
            cat,
            &ObservationRow {
                id: None,
                artifact_id: artifact_id.into(),
                text: text.into(),
                source: None,
                created_at: 1,
            },
        )
        .unwrap();
    }

    fn add_link(cat: &crate::librarian::catalog::Catalog, src: &str, dst: &str, rel: &str) {
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

    #[test]
    fn events_and_links_of_a_renamed_document_land_under_its_new_path() {
        let plan = plan_of_files(
            &[
                ("docs/my_notes.md", SINGLE_WRITER_MD.as_bytes()),
                ("docs/b.md", b"# B\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "mine", "docs/my_notes.md");
                add_doc(cat, root, "bee", "docs/b.md");
                add_event(cat, "e1", "mine", "note", r#"{"text":"hello"}"#);
                add_edge(cat, "e1", "mutates", None, Some("bee"), None);
                add_observation(cat, "mine", "seen");
                add_link(cat, "mine", "bee", "depends_on");
                // A link toward the renamed file is written with the key it now has.
                add_link(cat, "bee", "mine", "supersedes");
            },
        );
        let mine = &plan.files["docs/my-notes.md"];
        assert_eq!(mine.renamed_from.as_deref(), Some("docs/my_notes.md"));
        assert_eq!(mine.head_text.as_deref(), Some(SINGLE_WRITER_MD));
        let text = &mine.new_text;
        assert!(text.starts_with(SINGLE_WRITER_MD), "{text}");
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle}: {text}"))
        };
        assert!(at("<!-- event {\"id\":\"e1\"") < at("<!-- event {\"id\":\"observation-1\""));
        assert!(at("<!-- event {\"id\":\"observation-1\"") < at("## Relations"));
        assert!(text.contains("**Mutates:** [[b]]\n"), "{text}");
        assert!(text.contains("**Depends on:** [[b]]\n"), "{text}");
        let b = &plan.files["docs/b.md"];
        assert_eq!(b.renamed_from, None);
        assert_eq!(b.head_text.as_deref(), Some("# B\n"));
        assert_eq!(
            b.new_text,
            "# B\n\n## Relations\n\n**Supersedes:** [[my-notes]]\n"
        );
        for class in [Class::Event, Class::Observation, Class::ManualLink] {
            for item in items_of(&plan, class) {
                assert_eq!(item.status, Status::Moved, "{item:?}");
                assert!(
                    matches!(item.path.as_deref(), Some("docs/my-notes.md" | "docs/b.md")),
                    "{item:?}"
                );
            }
        }
        let tally = plan.tally();
        assert_eq!(tally[&Class::Event].moved, 1);
        assert_eq!(tally[&Class::Observation].moved, 1);
        assert_eq!(tally[&Class::ManualLink].moved, 2);
    }

    #[test]
    fn events_go_into_the_text_the_augmentation_step_left() {
        // Task 6 stamps the declaration into the same document; the events then start from that
        // text, not from `HEAD`'s.
        let plan = plan_of_files(
            &[("docs/n.md", SINGLE_WRITER_MD.as_bytes())],
            |cat, root| {
                add_doc(cat, root, "n", "docs/n.md");
                add_augmentation(cat, "n", r#"{"k":1}"#, None);
                add_event(cat, "e1", "n", "note", r#"{"text":"hello"}"#);
            },
        );
        let text = &plan.files["docs/n.md"].new_text;
        assert_eq!(
            declared_in(text).as_deref(),
            Some("docs/augmentations/docs-n.yaml")
        );
        assert!(text.contains("<!-- event {\"id\":\"e1\""), "{text}");
    }
    #[test]
    fn an_observation_a_live_note_recorded_is_carried_by_that_note() {
        // `event_create` records the observation of a note as well: the note's text twin is the
        // fact, and the observation row repeats it.
        let plan = plan_of_files(
            &[("docs/n.md", SINGLE_WRITER_MD.as_bytes())],
            |cat, root| {
                add_doc(cat, root, "n", "docs/n.md");
                add_event(cat, "e1", "n", "note", r#"{"text":"hello"}"#);
                add_observation(cat, "n", "hello");
                add_observation(cat, "n", "only an observation");
            },
        );
        let observations = items_of(&plan, Class::Observation);
        let reasons: Vec<(&Status, &str)> = observations
            .iter()
            .map(|i| (&i.status, i.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                (&Status::Skipped, "carried by its note event"),
                (&Status::Moved, "section written")
            ]
        );
        let text = &plan.files["docs/n.md"].new_text;
        assert!(text.contains("\"id\":\"observation-2\""), "{text}");
        assert!(!text.contains("\"id\":\"observation-1\""), "{text}");
    }
    fn note_section(id: &str, text: &str) -> crate::librarian::catalog::event_text::EventText {
        crate::librarian::catalog::event_text::EventText {
            id: id.to_owned(),
            kind: "note".to_owned(),
            created_at_ms: 1,
            author: None,
            anchor_commit: None,
            head_commit: None,
            parent: None,
            resolves: None,
            source: None,
            mutates: Vec::new(),
            payload: serde_json::json!({ "text": text }),
        }
    }

    /// `base` with the sections of `sections` spliced under its `## Events` heading.
    fn with_sections(
        base: &str,
        sections: &[crate::librarian::catalog::event_text::EventText],
    ) -> String {
        sections.iter().fold(base.to_owned(), |text, section| {
            crate::librarian::catalog::event_text::splice_event(&text, section).unwrap()
        })
    }

    #[test]
    fn a_held_observation_id_with_other_text_is_reported_not_skipped() {
        // Observation ids are rowids of one machine's catalog. `observation-2` in the committed text
        // is another machine's fact: this machine's observation 2 must not be reported as exported.
        let head = with_sections(
            SINGLE_WRITER_MD,
            &[
                note_section("observation-1", "same"),
                note_section("observation-2", "theirs"),
            ],
        );
        let plan = plan_of_files(&[("docs/my_notes.md", head.as_bytes())], |cat, root| {
            add_doc(cat, root, "mine", "docs/my_notes.md");
            add_observation(cat, "mine", "same");
            add_observation(cat, "mine", "mine, not theirs");
        });
        let items = items_of(&plan, Class::Observation);
        let outcome: Vec<(&Status, &str)> = items
            .iter()
            .map(|i| (&i.status, i.reason.as_str()))
            .collect();
        assert_eq!(
            outcome,
            [
                (&Status::AlreadyExported, "the text already holds it"),
                (
                    &Status::Unrecoverable,
                    "another section holds id observation-2"
                )
            ]
        );
        for item in &items {
            assert_eq!(item.path.as_deref(), Some("docs/my-notes.md"));
        }
        let file = &plan.files["docs/my-notes.md"];
        assert_eq!(file.renamed_from.as_deref(), Some("docs/my_notes.md"));
        assert!(file.new_text.contains("theirs"));
        assert!(
            !file.new_text.contains("mine, not theirs"),
            "the other machine's section is not overwritten and this row is not written"
        );
        assert_eq!(plan.tally()[&Class::Observation].unrecoverable, 1);
    }

    /// D-R7: every row of an event, observation or link class is moved, already exported, or named in
    /// a report line, so the population of each class equals its items. The fixture mixes the cases
    /// that each have a path of their own through the planner.
    #[test]
    fn the_population_of_events_observations_and_links_equals_their_items() {
        let home_head = with_sections(
            SINGLE_WRITER_MD,
            &[
                note_section("e-held", "an event the text holds"),
                note_section("observation-3", "held, same text"),
                note_section("observation-4", "held, other machine's text"),
            ],
        );
        let home_head = format!("{home_head}\n## Relations\n\n**Supersedes:** [[plain]]\n");
        let plan = plan_of_files(
            &[
                ("docs/home.md", home_head.as_bytes()),
                ("docs/plain.md", b"# Plain\n"),
                ("docs/frozen.md", SINGLE_WRITER_MD.as_bytes()),
                ("src/x.rs", b"const P: &str = \"../docs/frozen.md\";\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "home", "docs/home.md");
                add_doc(cat, root, "plain", "docs/plain.md");
                add_doc(cat, root, "fro", "docs/frozen.md");
                add_doc(cat, root, "gone", "docs/gone.md");
                // Twin kinds, a held id and the kinds no text holds (the catalog admits no other kind).
                add_event(cat, "e1", "home", "note", r#"{"text":"hello"}"#);
                add_event(cat, "e2", "home", "verdict", r#"{"v":1}"#);
                add_event(cat, "e3", "home", "intent", r#"{"i":1}"#);
                add_event(
                    cat,
                    "e-held",
                    "home",
                    "note",
                    r#"{"text":"an event the text holds"}"#,
                );
                for (n, kind) in [
                    "field_patch",
                    "status_change",
                    "superseded_by",
                    "worktree_fork",
                    "worktree_merge",
                ]
                .iter()
                .enumerate()
                {
                    add_event(cat, &format!("k{n}"), "home", kind, "{}");
                }
                // Observations: carried (1), written (2), held with the same text (3) and with
                // other text (4).
                add_observation(cat, "home", "hello");
                add_observation(cat, "home", "a lone observation");
                add_observation(cat, "home", "held, same text");
                add_observation(cat, "home", "this machine's text");
                // Links: written, no label, no target, a line the text holds.
                add_link(cat, "home", "plain", "depends_on");
                add_link(cat, "home", "plain", "v2_rel");
                add_link(cat, "home", "gone", "depends_on");
                add_link(cat, "home", "plain", "supersedes");
                // A non-home, a frozen document and one `HEAD` lacks: every row refused.
                for doc in ["plain", "fro", "gone"] {
                    add_event(cat, &format!("e-{doc}"), doc, "note", r#"{"text":"t"}"#);
                    add_event(cat, &format!("f-{doc}"), doc, "field_patch", "{}");
                    add_observation(cat, doc, &format!("seen on {doc}"));
                    add_link(cat, doc, "home", "depends_on");
                }
            },
        );
        let events: usize = plan
            .population
            .iter()
            .filter(|(key, _)| key.starts_with("event:"))
            .map(|(_, n)| n)
            .sum();
        assert_eq!(events, 4 + 5 + 3 * 2, "the fixture's own count");
        assert_eq!(plan.population["observation"], 4 + 3);
        assert_eq!(plan.population["manual_link"], 4 + 3);
        let count = |class: Class| items_of(&plan, class).len();
        assert_eq!(count(Class::Event), events, "every event has one item");
        assert_eq!(
            count(Class::Observation),
            plan.population["observation"],
            "every observation has one item"
        );
        assert_eq!(
            count(Class::ManualLink),
            plan.population["manual_link"],
            "every link has one item"
        );
        // Each status the fixture was built to reach is reached.
        let tally = plan.tally();
        for class in [Class::Event, Class::Observation, Class::ManualLink] {
            let t = &tally[&class];
            assert!(t.moved > 0, "{class:?}");
            assert!(t.already_exported > 0, "{class:?}");
            assert!(t.unrecoverable > 0, "{class:?}");
        }
        assert!(tally[&Class::Event].skipped > 0);
        assert!(tally[&Class::Observation].skipped > 0);
    }

    #[test]
    fn a_rerun_over_the_written_text_reports_everything_already_exported() {
        let first = plan_of_files(
            &[("docs/n.md", SINGLE_WRITER_MD.as_bytes())],
            |cat, root| {
                add_doc(cat, root, "n", "docs/n.md");
                add_event(cat, "e1", "n", "note", r#"{"text":"hello"}"#);
            },
        );
        assert_eq!(first.tally()[&Class::Event].moved, 1);
        let written = first.files["docs/n.md"].new_text.clone();
        let second = plan_of_files(&[("docs/n.md", written.as_bytes())], |cat, root| {
            add_doc(cat, root, "n", "docs/n.md");
            add_event(cat, "e1", "n", "note", r#"{"text":"hello"}"#);
        });
        assert!(second.files.is_empty(), "{:?}", second.files.keys());
        let events = &second.tally()[&Class::Event];
        assert_eq!((events.moved, events.already_exported), (0, 1));
    }

    /// The plan's texts committed as the next `HEAD` (a renamed file moves), planned again with the
    /// same old catalog: `(first plan, second plan)`. The manual's sequence (export, commit, a plan
    /// that writes nothing, build) needs the second plan to be a fixed point.
    fn plan_commit_and_replan(
        files: &[(&str, &[u8])],
        fill: impl Fn(&crate::librarian::catalog::Catalog, &Path),
    ) -> (ExportPlan, ExportPlan) {
        let first = plan_of_files(files, &fill);
        let mut next: BTreeMap<String, Vec<u8>> = files
            .iter()
            .map(|&(path, bytes)| (path.to_owned(), bytes.to_vec()))
            .collect();
        for change in first.files.values() {
            if let Some(from) = &change.renamed_from {
                next.remove(from);
            }
            next.insert(change.path.clone(), change.new_text.clone().into_bytes());
        }
        let next: Vec<(&str, &[u8])> = next
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
            .collect();
        let second = plan_of_files(&next, &fill);
        (first, second)
    }

    /// The second plan writes no file and moves nothing, in any class.
    fn assert_fixed_point(second: &ExportPlan) {
        let changed: Vec<&String> = second.files.keys().collect();
        assert!(
            changed.is_empty(),
            "the second plan writes {changed:?}: {:#?}",
            second.files
        );
        assert_eq!(second.writes(), 0);
        for (class, tally) in second.tally() {
            assert_eq!(tally.moved, 0, "{class:?} moves again: {:#?}", second.items);
        }
    }

    const OTHER_MD: &str = "# Other\n\n## R-1 \u{2014} one\n\nx\n";
    const OTHER_ID: &str = "0123456789abcdef";

    /// `docs/n.md` (single-writer) with an optional note event and an optional observation, and
    /// `docs/other.md`, which defines `R-1` and has the catalog id [`OTHER_ID`].
    fn note_and_observation_plans(
        note: Option<&str>,
        observation: Option<&str>,
    ) -> (ExportPlan, ExportPlan) {
        plan_commit_and_replan(
            &[
                ("docs/n.md", SINGLE_WRITER_MD.as_bytes()),
                ("docs/other.md", OTHER_MD.as_bytes()),
            ],
            |cat, root| {
                add_doc(cat, root, "n", "docs/n.md");
                add_doc(cat, root, OTHER_ID, "docs/other.md");
                if let Some(text) = note {
                    let payload = serde_json::json!({ "text": text }).to_string();
                    add_event(cat, "e1", "n", "note", &payload);
                }
                if let Some(text) = observation {
                    add_observation(cat, "n", text);
                }
            },
        )
    }

    fn moved_citations_in<'a>(plan: &'a ExportPlan, path: &str) -> Vec<&'a Item> {
        citation_items(plan)
            .into_iter()
            .filter(|i| i.path.as_deref() == Some(path) && i.status == Status::Moved)
            .collect()
    }

    /// C1: text an export step adds is rewritten like committed text (D-R9), and the first plan
    /// counts the rewrites, so the plan after the commit writes nothing.
    #[test]
    fn a_note_that_cites_old_forms_is_written_rewritten_and_the_commit_is_a_fixed_point() {
        let cited = "see other:R-1 and 0123456789abcdef";
        // The observation a live note recorded repeats the note's text: it stays carried.
        let (first, second) = note_and_observation_plans(Some(cited), Some(cited));
        let text = &first.files["docs/n.md"].new_text;
        assert!(
            text.contains("> see [[other:R-1]] and [[other]]\n"),
            "{text}"
        );
        assert_eq!(
            moved_citations_in(&first, "docs/n.md").len(),
            2,
            "{:#?}",
            first.items
        );
        assert_fixed_point(&second);
        assert_eq!(
            statuses_of(&second, Class::Event),
            [Status::AlreadyExported]
        );
        for plan in [&first, &second] {
            let observation = items_of(plan, Class::Observation);
            assert_eq!(observation.len(), 1);
            assert_eq!(observation[0].reason, "carried by its note event");
        }
    }

    #[test]
    fn an_observation_that_cites_old_forms_stays_this_machines_fact_after_the_commit() {
        let (first, second) =
            note_and_observation_plans(None, Some("see other:R-1 and 0123456789abcdef"));
        let text = &first.files["docs/n.md"].new_text;
        assert!(
            text.contains("> see [[other:R-1]] and [[other]]\n"),
            "{text}"
        );
        assert_eq!(
            moved_citations_in(&first, "docs/n.md").len(),
            2,
            "{:#?}",
            first.items
        );
        assert_fixed_point(&second);
        let observation = items_of(&second, Class::Observation);
        assert_eq!(observation.len(), 1);
        assert_eq!(
            (&observation[0].status, observation[0].reason.as_str()),
            (&Status::AlreadyExported, "the text already holds it"),
            "the committed section is this row, rewritten"
        );
    }

    #[test]
    fn an_observation_with_an_old_path_link_is_rewritten_and_the_commit_is_a_fixed_point() {
        let (first, second) = note_and_observation_plans(None, Some("see [O](other.md)"));
        let text = &first.files["docs/n.md"].new_text;
        assert!(text.contains("> see [[other|O]]\n"), "{text}");
        assert_eq!(
            moved_citations_in(&first, "docs/n.md").len(),
            1,
            "{:#?}",
            first.items
        );
        assert_fixed_point(&second);
        assert_eq!(
            statuses_of(&second, Class::Observation),
            [Status::AlreadyExported]
        );
    }

    /// D-R28 binds an id that history knows. `add_row` adds to the old catalog the row of
    /// `docs/n.md` (single-writer) whose text is the given one, which cites an id only history
    /// knows: the first plan writes it with the id rewritten, and the plan after the commit writes
    /// nothing.
    fn assert_a_history_id_is_rewritten_once(
        add_row: impl Fn(&crate::librarian::catalog::Catalog, &str),
    ) {
        use crate::librarian::derived::testkit::commit_files;
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        const BODY: &[u8] =
            b"# Notes\n\nSome text that makes the file big enough to match.\n\nMore.\n";
        let repo = repo_with(
            &root,
            &[
                ("docs/old.md", FILE, BODY),
                ("docs/n.md", FILE, SINGLE_WRITER_MD.as_bytes()),
            ],
        );
        let first_commit = repo.head().unwrap().peel_to_commit().unwrap();
        let id = crate::librarian::ids::artifact_id_from_abs(&root.join("docs/old.md"));
        let tip = commit_files(
            &repo,
            &[
                ("docs/new.md", BODY),
                ("docs/n.md", SINGLE_WRITER_MD.as_bytes()),
            ],
            &[&first_commit],
            "move",
        );
        let plan_at = |commit: git2::Oid| {
            let old = old_catalog_with(|cat| {
                add_doc(cat, &root, "n", "docs/n.md");
                add_row(cat, &format!("see {id}"));
            });
            let rows = load(&old, &RootMap::new(root.clone())).unwrap();
            plan_export(&repo, &repo.find_commit(commit).unwrap(), &rows).unwrap()
        };
        let first = plan_at(tip);
        let text = first.files["docs/n.md"].new_text.clone();
        assert!(text.contains("> see [[new]]\n"), "{text}");
        let tip_commit = repo.find_commit(tip).unwrap();
        let exported = commit_files(
            &repo,
            &[("docs/new.md", BODY), ("docs/n.md", text.as_bytes())],
            &[&tip_commit],
            "export",
        );
        assert_fixed_point(&plan_at(exported));
    }

    /// The text a note adds is searched for ids like the committed text, so the first plan binds
    /// it as the plan after the commit would.
    #[test]
    fn a_note_that_cites_an_id_only_history_knows_is_rewritten_once() {
        assert_a_history_id_is_rewritten_once(|cat, text| {
            let payload = serde_json::json!({ "text": text }).to_string();
            add_event(cat, "e1", "n", "note", &payload);
        });
    }

    /// C1-d: an observation's text is searched too (an observation with no note event).
    #[test]
    fn an_observation_that_cites_an_id_only_history_knows_is_rewritten_once() {
        assert_a_history_id_is_rewritten_once(|cat, text| add_observation(cat, "n", text));
    }

    /// m7: an entry title goes into a heading, where no form is rewritten. The first plan reports
    /// the form as the plan after the commit does.
    #[test]
    fn an_entry_title_with_an_old_form_is_reported_by_the_first_plan() {
        let (first, second) = plan_commit_and_replan(
            &[
                ("docs/ledger.md", LEDGER_MD.as_bytes()),
                ("docs/other.md", OTHER_MD.as_bytes()),
            ],
            |cat, root| {
                add_doc(cat, root, "led", "docs/ledger.md");
                add_doc(cat, root, OTHER_ID, "docs/other.md");
                add_augmentation(
                    cat,
                    "led",
                    r#"{"entries":[{"id":"F-2","title":"follows other:R-1"}]}"#,
                    Some("entries"),
                );
            },
        );
        let in_heading = |plan: &ExportPlan| -> Vec<(Status, String)> {
            citation_items(plan)
                .into_iter()
                .filter(|i| i.path.as_deref() == Some("docs/ledger.md"))
                .map(|i| (i.status, i.reason.clone()))
                .collect()
        };
        let want = [(Status::Unrecoverable, "inside a heading".to_owned())];
        assert_eq!(in_heading(&first), want);
        assert_fixed_point(&second);
        assert_eq!(in_heading(&second), want);
    }
    /// C1-a: `docs/ledger.md` ([`LEDGER_MD`], which defines `F-1`) with the params entry `F-2`,
    /// which only the export writes, beside `files`, and a note event on `docs/n.md` when `note`
    /// is given. `(first plan, plan after the commit)`.
    fn recovered_entry_plans(
        files: &[(&str, &str)],
        note: Option<&str>,
    ) -> (ExportPlan, ExportPlan) {
        let mut all: Vec<(&str, &[u8])> = vec![("docs/ledger.md", LEDGER_MD.as_bytes())];
        all.extend(files.iter().map(|&(path, text)| (path, text.as_bytes())));
        plan_commit_and_replan(&all, |cat, root| {
            add_doc(cat, root, "led", "docs/ledger.md");
            add_augmentation(
                cat,
                "led",
                r#"{"entries":[{"id":"F-2","title":"two"}]}"#,
                Some("entries"),
            );
            if let Some(text) = note {
                add_doc(cat, root, "n", "docs/n.md");
                let payload = serde_json::json!({ "text": text }).to_string();
                add_event(cat, "e1", "n", "note", &payload);
            }
        })
    }

    fn citation_reasons_in<'a>(plan: &'a ExportPlan, path: &str) -> Vec<(Status, &'a str)> {
        citation_items(plan)
            .into_iter()
            .filter(|i| i.path.as_deref() == Some(path))
            .map(|i| (i.status, i.reason.as_str()))
            .collect()
    }

    /// C1-a: the export writes the section of the params entry `F-2`, so the first plan reads a
    /// bare `F-2` against it, as the plan of the commit that holds the export does (D-R20).
    #[test]
    fn a_bare_token_only_a_recovered_entry_defines_is_rewritten_by_the_first_plan() {
        let (first, second) =
            recovered_entry_plans(&[("docs/src.md", "# Src\n\nsee F-2 here.\n")], None);
        let text = first.files.get("docs/src.md").map(|f| f.new_text.as_str());
        assert_eq!(text, Some("# Src\n\nsee [[ledger:F-2]] here.\n"));
        assert_eq!(
            citation_reasons_in(&first, "docs/src.md"),
            [(Status::Moved, "rewritten: bare token")]
        );
        assert_fixed_point(&second);
    }

    #[test]
    fn a_stem_token_only_a_recovered_entry_defines_is_rewritten_by_the_first_plan() {
        let (first, second) =
            recovered_entry_plans(&[("docs/src.md", "# Src\n\nsee ledger:F-2 here.\n")], None);
        let text = first.files.get("docs/src.md").map(|f| f.new_text.as_str());
        assert_eq!(text, Some("# Src\n\nsee [[ledger:F-2]] here.\n"));
        assert_eq!(
            citation_reasons_in(&first, "docs/src.md"),
            [(Status::Moved, "rewritten: stem token")]
        );
        assert_fixed_point(&second);
    }

    #[test]
    fn a_note_that_cites_a_recovered_entry_is_written_rewritten() {
        let (first, second) =
            recovered_entry_plans(&[("docs/n.md", SINGLE_WRITER_MD)], Some("see ledger:F-2"));
        let text = &first.files["docs/n.md"].new_text;
        assert!(text.contains("> see [[ledger:F-2]]\n"), "{text}");
        assert_eq!(
            citation_reasons_in(&first, "docs/n.md"),
            [(Status::Moved, "rewritten: stem token")]
        );
        assert_fixed_point(&second);
    }

    /// D-R20 with a recovered definer: the ledger that defines `F-2` once the export writes it
    /// keeps its own bare `F-2`.
    #[test]
    fn the_ledger_that_recovers_an_entry_keeps_its_own_bare_token() {
        let head = format!("{LEDGER_MD}\nsee F-2 here.\n");
        let (first, second) =
            plan_commit_and_replan(&[("docs/ledger.md", head.as_bytes())], |cat, root| {
                add_doc(cat, root, "led", "docs/ledger.md");
                add_augmentation(
                    cat,
                    "led",
                    r#"{"entries":[{"id":"F-2","title":"two"}]}"#,
                    Some("entries"),
                );
            });
        let text = &first.files["docs/ledger.md"].new_text;
        assert!(text.contains("\nsee F-2 here.\n"), "{text}");
        assert!(text.contains("\n## F-2 — two\n"), "{text}");
        assert_eq!(citation_reasons_in(&first, "docs/ledger.md"), []);
        assert_fixed_point(&second);
        assert_eq!(citation_reasons_in(&second, "docs/ledger.md"), []);
    }

    /// D-R20 counts every definer: a committed `F-2` elsewhere and the recovered one make two.
    #[test]
    fn a_token_a_recovered_entry_and_another_document_define_is_ambiguous() {
        const OTHER_LEDGER: &str =
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# B\n\n## F-2 — b\n\ny\n";
        let (first, second) = recovered_entry_plans(
            &[
                ("docs/b.md", OTHER_LEDGER),
                ("docs/src.md", "# Src\n\nsee F-2 here.\n"),
            ],
            None,
        );
        assert!(
            !first.files.contains_key("docs/src.md"),
            "{}",
            first.files["docs/src.md"].new_text
        );
        let ambiguous = [(
            Status::Unrecoverable,
            "ambiguous: several documents qualify",
        )];
        assert_eq!(citation_reasons_in(&first, "docs/src.md"), ambiguous);
        assert_fixed_point(&second);
        assert_eq!(citation_reasons_in(&second, "docs/src.md"), ambiguous);
    }

    /// A frozen ledger gets no section (D-R26), so its params entry defines nothing.
    #[test]
    fn the_entry_of_a_frozen_ledger_defines_no_token() {
        let (first, second) = recovered_entry_plans(
            &[
                ("docs/src.md", "# Src\n\nsee F-2 here.\n"),
                ("src/x.rs", "const P: &str = \"../docs/ledger.md\";\n"),
            ],
            None,
        );
        assert!(first.files.is_empty(), "{:?}", first.files.keys());
        assert_eq!(citation_reasons_in(&first, "docs/src.md"), []);
        assert_fixed_point(&second);
    }

    /// The ledger `head` with the params entry `F-2` beside `files`: `(first plan, plan after the
    /// commit)`.
    fn ledger_entry_plans(head: &str, files: &[(&str, &str)]) -> (ExportPlan, ExportPlan) {
        let mut all: Vec<(&str, &[u8])> = vec![("docs/ledger.md", head.as_bytes())];
        all.extend(files.iter().map(|&(path, text)| (path, text.as_bytes())));
        plan_commit_and_replan(&all, |cat, root| {
            add_doc(cat, root, "led", "docs/ledger.md");
            add_augmentation(
                cat,
                "led",
                r#"{"entries":[{"id":"F-2","title":"two"}]}"#,
                Some("entries"),
            );
        })
    }

    /// F1: a seven-hash line is no heading for the index, so recovery does not read it as the
    /// section of `F-2` either. The first plan recovers the section, and the line, a bare `F-2` its
    /// own ledger defines, stays as it is (D-R20).
    #[test]
    fn a_seven_hash_line_defines_no_entry_for_recovery_either() {
        let head = format!("{LEDGER_MD}\n####### F-2 — fake\n");
        let (first, second) = ledger_entry_plans(
            &head,
            &[
                ("docs/b.md", "# B\n\n## F-2 — b\n"),
                ("docs/src.md", "# Src\n\nsee ledger:F-2 here.\n"),
            ],
        );
        assert_eq!(
            statuses_of(&first, Class::ParamsEntry),
            [Status::Moved],
            "{:#?}",
            first.items
        );
        let ledger = &first.files["docs/ledger.md"].new_text;
        assert!(ledger.contains("\n####### F-2 — fake\n"), "{ledger}");
        assert!(ledger.contains("\n## F-2 — two\n"), "{ledger}");
        let text = first.files.get("docs/src.md").map(|f| f.new_text.as_str());
        assert_eq!(text, Some("# Src\n\nsee [[ledger:F-2]] here.\n"));
        assert_eq!(
            citation_reasons_in(&first, "docs/src.md"),
            [(Status::Moved, "rewritten: stem token")]
        );
        assert_fixed_point(&second);
    }

    /// F2: a frontmatter comment shaped like an entry heading is not the section of `F-2`: the
    /// index reads only the body, and so does recovery. The first plan recovers the section and
    /// rewrites both citations.
    #[test]
    fn a_frontmatter_comment_defines_no_entry_for_recovery_either() {
        let head =
            "---\nkind: tracker\nentry_prefix: F\n## F-2 — two\n---\n\n# L\n\n## F-1 — one\n\nx\n";
        let (first, second) =
            ledger_entry_plans(head, &[("docs/src.md", "see F-2 here and ledger:F-2.\n")]);
        assert_eq!(
            statuses_of(&first, Class::ParamsEntry),
            [Status::Moved],
            "{:#?}",
            first.items
        );
        let ledger = &first.files["docs/ledger.md"].new_text;
        let (_, body) = crate::librarian::frontmatter::parse(ledger).unwrap();
        assert!(body.contains("\n## F-2 — two\n"), "{ledger}");
        let text = first.files.get("docs/src.md").map(|f| f.new_text.as_str());
        assert_eq!(text, Some("see [[ledger:F-2]] here and [[ledger:F-2]].\n"));
        assert_fixed_point(&second);
    }

    /// Whether `text` ends a line with a bare `\n`.
    fn has_a_bare_lf(text: &str) -> bool {
        text.replace("\r\n", "").contains('\n')
    }

    /// A CRLF ledger gets its recovered section, its high-water mark and its declaration in CRLF.
    #[test]
    fn a_crlf_ledger_keeps_crlf_in_what_the_export_adds() {
        let head = LEDGER_MD.replace('\n', "\r\n");
        let (first, second) = ledger_entry_plans(&head, &[]);
        let text = &first.files["docs/ledger.md"].new_text;
        assert!(text.contains("\r\n## F-2 — two\r\n"), "{text:?}");
        assert!(text.contains("entry_high_water_F: 2\r\n"), "{text:?}");
        assert!(!has_a_bare_lf(text), "{text:?}");
        assert_fixed_point(&second);
    }

    /// A CRLF document gets its events and relations sections in CRLF.
    #[test]
    fn a_crlf_document_keeps_crlf_in_its_events_and_relations() {
        let head = SINGLE_WRITER_MD.replace('\n', "\r\n");
        let (first, second) = plan_commit_and_replan(
            &[
                ("docs/n.md", head.as_bytes()),
                ("docs/other.md", OTHER_MD.as_bytes()),
            ],
            |cat, root| {
                add_doc(cat, root, "n", "docs/n.md");
                add_doc(cat, root, OTHER_ID, "docs/other.md");
                add_event(cat, "e1", "n", "note", r#"{"text":"one\ntwo"}"#);
                add_link(cat, "n", OTHER_ID, "depends_on");
            },
        );
        let text = &first.files["docs/n.md"].new_text;
        assert!(text.contains("\r\n## Events\r\n"), "{text:?}");
        assert!(text.contains("\r\n## Relations\r\n"), "{text:?}");
        assert!(!has_a_bare_lf(text), "{text:?}");
        assert_fixed_point(&second);
    }

    /// A document whose line ends are mixed has no line ending of its own: what the export adds
    /// keeps `\n`, and a body line it does not touch keeps its `\r\n`.
    #[test]
    fn a_document_with_mixed_line_ends_gets_lf() {
        let head = LEDGER_MD.replace("# L\n", "# L\r\n");
        let (first, second) = ledger_entry_plans(&head, &[]);
        let text = &first.files["docs/ledger.md"].new_text;
        assert!(text.contains("\n# L\r\n\n## F-1"), "{text:?}");
        assert!(text.contains("x\n\n## F-2 — two\n\n"), "{text:?}");
        assert_fixed_point(&second);
    }

    /// C1-b: a recovered section after a last line with trailing spaces leaves that line as it is,
    /// so the first plan reports its citation once, as the plan after the commit does.
    #[test]
    fn a_citation_on_a_last_line_with_trailing_spaces_is_reported_once() {
        let head = format!("{LEDGER_MD}see ledger:F-9   ");
        let (first, second) =
            plan_commit_and_replan(&[("docs/ledger.md", head.as_bytes())], |cat, root| {
                add_doc(cat, root, "led", "docs/ledger.md");
                add_augmentation(
                    cat,
                    "led",
                    r#"{"entries":[{"id":"F-2","title":"two"}]}"#,
                    Some("entries"),
                );
            });
        let in_ledger = |plan: &ExportPlan| -> Vec<(Option<u32>, Status)> {
            citation_items(plan)
                .into_iter()
                .filter(|i| i.path.as_deref() == Some("docs/ledger.md"))
                .map(|i| (i.line, i.status))
                .collect()
        };
        let want = [(Some(11), Status::Unrecoverable)];
        assert_eq!(in_ledger(&first), want, "{:#?}", first.items);
        let text = &first.files["docs/ledger.md"].new_text;
        assert!(text.contains("\nsee ledger:F-9   \n\n## F-2"), "{text:?}");
        assert_fixed_point(&second);
        // The commit's frontmatter holds two more lines: the high-water mark and the declaration.
        assert_eq!(in_ledger(&second), [(Some(13), Status::Unrecoverable)]);
    }

    /// I2: a ledger whose text ends inside an unclosed code fence swallows a recovered section. The
    /// entry is reported and the text is not touched, so a rerun does not add it again.
    #[test]
    fn an_entry_that_would_land_in_an_unclosed_fence_is_unrecoverable_and_not_written() {
        let head = format!("{LEDGER_MD}\n```\nunclosed\n");
        let (first, second) =
            plan_commit_and_replan(&[("docs/ledger.md", head.as_bytes())], |cat, root| {
                add_doc(cat, root, "led", "docs/ledger.md");
                add_augmentation(
                    cat,
                    "led",
                    r#"{"entries":[{"id":"F-2","title":"two"}]}"#,
                    Some("entries"),
                );
            });
        let entries = items_of(&first, Class::ParamsEntry);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].status, Status::Unrecoverable, "{entries:?}");
        if let Some(file) = first.files.get("docs/ledger.md") {
            assert!(!file.new_text.contains("F-2"), "{}", file.new_text);
        }
        assert_fixed_point(&second);
    }

    #[test]
    fn events_and_links_of_a_frozen_or_missing_document_are_unrecoverable() {
        let plan = plan_of_files(
            &[
                ("docs/frozen.md", SINGLE_WRITER_MD.as_bytes()),
                ("src/x.rs", b"const P: &str = \"../docs/frozen.md\";\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "fro", "docs/frozen.md");
                add_doc(cat, root, "gone", "docs/gone.md");
                for doc in ["fro", "gone"] {
                    add_event(cat, &format!("e-{doc}"), doc, "note", r#"{"text":"t"}"#);
                    add_observation(cat, doc, "seen");
                    add_link(cat, doc, "fro", "depends_on");
                }
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        for class in [Class::Event, Class::Observation, Class::ManualLink] {
            let items = items_of(&plan, class);
            assert_eq!(items.len(), 2, "{class:?}");
            for item in &items {
                assert_eq!(item.status, Status::Unrecoverable, "{item:?}");
            }
            let frozen = items
                .iter()
                .find(|i| i.path.as_deref() == Some("docs/frozen.md"))
                .unwrap();
            assert!(frozen.reason.starts_with("frozen: "), "{}", frozen.reason);
            let gone = items
                .iter()
                .find(|i| i.path.as_deref() == Some("docs/gone.md"))
                .unwrap();
            assert_eq!(gone.reason, "document not in HEAD");
        }
    }

    #[test]
    fn a_document_outside_the_key_space_is_reported_as_such() {
        let plan = plan_of_files(
            &[
                (".codescout/key-space.toml", b"exclude = [\"contrib/**\"]\n"),
                ("contrib/c.md", SINGLE_WRITER_MD.as_bytes()),
                ("docs/ok.md", b"# Ok\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "c", "contrib/c.md");
                add_doc(cat, root, "ok", "docs/ok.md");
                add_event(cat, "e1", "c", "note", r#"{"text":"t"}"#);
                add_observation(cat, "c", "seen");
                add_link(cat, "c", "ok", "depends_on");
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        for class in [Class::Event, Class::Observation, Class::ManualLink] {
            let items = items_of(&plan, class);
            assert_eq!(items.len(), 1, "{class:?}");
            assert_eq!(items[0].status, Status::Unrecoverable);
            assert_eq!(items[0].reason, REASON_NOT_IN_SPACE, "{class:?}");
        }
    }

    #[test]
    fn a_target_outside_the_head_makes_the_link_unrecoverable() {
        let plan = plan_of_files(&[("docs/a.md", b"# A\n")], |cat, root| {
            add_doc(cat, root, "a", "docs/a.md");
            add_doc(cat, root, "gone", "docs/gone.md");
            add_link(cat, "a", "gone", "depends_on");
        });
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let items = items_of(&plan, Class::ManualLink);
        assert_eq!(items.len(), 1);
        for item in items {
            assert_eq!(item.status, Status::Unrecoverable);
            assert_eq!(item.reason, "the target has no name in the key space");
            assert_eq!(item.path.as_deref(), Some("docs/a.md"));
        }
    }

    #[test]
    fn a_committed_sidecar_is_compared_never_overwritten() {
        let sidecar_path = "docs/augmentations/docs-ledger.yaml";
        let with_head_sidecar = |yaml: &'static str| {
            plan_of_files(
                &[
                    ("docs/ledger.md", LEDGER_MD.as_bytes()),
                    (sidecar_path, yaml.as_bytes()),
                ],
                |cat, root| {
                    add_doc(cat, root, "led", "docs/ledger.md");
                    add_augmentation(cat, "led", "{}", Some("entries"));
                },
            )
        };
        // Equal shape, no declaration: the sidecar is untouched and the document gets the stamp.
        let plan = with_head_sidecar("prompt: keep it current\nentry_collection: entries\n");
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(files, ["docs/ledger.md"]);
        assert_eq!(
            declared_in(&plan.files["docs/ledger.md"].new_text).as_deref(),
            Some(sidecar_path)
        );
        assert_eq!(
            statuses_of(&plan, Class::AugmentationShape),
            [Status::Moved]
        );
        // A different shape: nothing is written, the drift is named.
        let plan = with_head_sidecar("prompt: an older prompt\nentry_collection: entries\n");
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 1);
        assert_eq!(shape[0].status, Status::Unrecoverable);
        assert_eq!(shape[0].reason, "shape drift: prompt");
        assert_eq!(shape[0].path.as_deref(), Some("docs/ledger.md"));
    }

    #[test]
    fn a_declared_equal_sidecar_in_head_is_already_exported() {
        let sidecar_path = "docs/augmentations/docs-ledger.yaml";
        let declared = format!(
            "---\nkind: tracker\nentry_prefix: F\nexpects_augmentation: {sidecar_path}\n---\n\n# L\n\n## F-1 — one\n\nx\n"
        );
        let plan = plan_of_files(
            &[
                ("docs/ledger.md", declared.as_bytes()),
                (sidecar_path, b"prompt: keep it current\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "led", "docs/ledger.md");
                add_augmentation(cat, "led", "{}", None);
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        assert_eq!(plan.writes(), 0);
        assert_eq!(
            statuses_of(&plan, Class::AugmentationShape),
            [Status::AlreadyExported]
        );
    }

    #[test]
    fn a_document_without_frontmatter_refuses_its_shape_and_still_gets_its_entries() {
        let plan = plan_of_files(
            &[("docs/plain.md", b"# P\n\n## F-1 \xe2\x80\x94 one\n\nx\n")],
            |cat, root| {
                add_doc(cat, root, "p", "docs/plain.md");
                add_augmentation(cat, "p", r#"{"entries":[{"id":"F-2"}]}"#, Some("entries"));
            },
        );
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 1);
        assert_eq!(shape[0].status, Status::Unrecoverable);
        assert!(
            shape[0].reason.contains("no frontmatter block"),
            "{}",
            shape[0].reason
        );
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(files, ["docs/plain.md"], "no sidecar without a declaration");
        assert!(plan.files["docs/plain.md"]
            .new_text
            .contains("\n## F-2 — F-2\n"));
        assert!(!plan.files["docs/plain.md"]
            .new_text
            .contains("entry_high_water"));
    }

    #[test]
    fn params_that_are_not_json_are_one_item_and_one_row() {
        let plan = plan_of_files(&[("docs/ledger.md", LEDGER_MD.as_bytes())], |cat, root| {
            add_doc(cat, root, "led", "docs/ledger.md");
            add_augmentation(cat, "led", "{broken", Some("entries"));
        });
        let other = items_of(&plan, Class::ParamsOther);
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].reason, "params is not JSON");
        assert_eq!(plan.population["params_other"], 1);
        assert!(items_of(&plan, Class::ParamsEntry).is_empty());
    }

    /// Two live rows may name one path (the same file under two spellings of the root). Both rows'
    /// params are reported under that path, the path's text and sidecar are planned once, and no
    /// entry is lost: an entry the first row recovered is already exported for the second.
    fn two_rows_plan(second_prompt: Option<&'static str>, reverse_rows: bool) -> ExportPlan {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let repo = repo_with(&root, &[("docs/ledger.md", FILE, LEDGER_MD.as_bytes())]);
        let old = old_catalog_with(|cat| {
            add_doc(cat, &root, "d1", "docs/ledger.md");
            add_doc_at(cat, "d2", "/old/spelling/docs/ledger.md");
            add_augmentation(
                cat,
                "d1",
                r#"{"entries":[{"id":"F-2","title":"two"}],"from_first":1}"#,
                Some("entries"),
            );
            add_augmentation(
                cat,
                "d2",
                r#"{"entries":[{"id":"F-2","title":"two"},{"id":"F-3","title":"three"}],"from_second":2}"#,
                Some("entries"),
            );
            if let Some(prompt) = second_prompt {
                let mut row = crate::librarian::catalog::augmentation::get(cat, "d2")
                    .unwrap()
                    .unwrap();
                row.prompt = prompt.into();
                crate::librarian::catalog::augmentation::upsert(cat, &row).unwrap();
            }
        });
        let map =
            RootMap::with_old_roots(root.clone(), vec![PathBuf::from("/old/spelling")]).unwrap();
        let mut rows = load(&old, &map).unwrap();
        if reverse_rows {
            rows.augmentations.reverse();
        }
        assert_eq!(rows.documents.len(), 2);
        let commit = open_commit(&repo, "HEAD").unwrap();
        plan_export(&repo, &commit, &rows).unwrap()
    }

    #[test]
    fn two_rows_naming_one_path_are_exported_once_and_both_reported() {
        let plan = two_rows_plan(None, false);
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(
            files,
            ["docs/augmentations/docs-ledger.yaml", "docs/ledger.md"]
        );
        let text = &plan.files["docs/ledger.md"].new_text;
        assert_eq!(text.matches("\n## F-2 — two\n").count(), 1, "{text}");
        assert_eq!(text.matches("\n## F-3 — three\n").count(), 1, "{text}");
        assert!(text.contains("entry_high_water_F: 3"), "{text}");
        assert_eq!(
            statuses_of(&plan, Class::ParamsEntry),
            [Status::Moved, Status::AlreadyExported, Status::Moved]
        );
        // The second row's shape equals the sidecar the first planned: nothing exported twice.
        assert_eq!(
            statuses_of(&plan, Class::AugmentationShape),
            [Status::Moved, Status::AlreadyExported]
        );
        // Both rows' other params are named under the one path.
        let other = items_of(&plan, Class::ParamsOther);
        let details: Vec<Option<&str>> = other.iter().map(|i| i.detail.as_deref()).collect();
        assert_eq!(details, [Some("from_first"), Some("from_second")]);
        assert!(other
            .iter()
            .all(|i| i.path.as_deref() == Some("docs/ledger.md")));
    }

    #[test]
    fn two_rows_naming_one_path_with_two_shapes_report_the_second_as_drift() {
        let plan = two_rows_plan(Some("another prompt"), false);
        let sidecars = plan
            .files
            .keys()
            .filter(|p| p.starts_with("docs/augmentations/"))
            .count();
        assert_eq!(sidecars, 1, "the second shape never overwrites the first");
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 2);
        assert_eq!(shape[0].status, Status::Moved);
        assert_eq!(shape[1].status, Status::Unrecoverable);
        assert_eq!(shape[1].reason, "shape drift: prompt");
        assert_eq!(items_of(&plan, Class::ParamsOther).len(), 2);
    }

    #[test]
    fn a_class_with_a_population_and_no_item_has_a_zero_row() {
        // Every event kind has an item now, so a class with rows and no item is built by hand: the
        // tally says it with zeros rather than leaving the class out.
        let mut plan = plan_for(|cat, root| {
            let _ = with_docs_slug(cat, root);
        });
        plan.population.insert("event:note".to_owned(), 1);
        let tally = plan.tally();
        let event = &tally[&Class::Event];
        assert_eq!(
            (
                event.moved,
                event.already_exported,
                event.skipped,
                event.unrecoverable
            ),
            (0, 0, 0, 0)
        );
        assert!(
            !tally.contains_key(&Class::Rename),
            "a class with no population stays out"
        );
    }

    #[test]
    fn a_committed_sidecar_that_is_a_link_is_never_replaced() {
        use crate::librarian::derived::testkit::LINK;
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let repo = repo_with(
            &root,
            &[
                ("docs/ledger.md", FILE, LEDGER_MD.as_bytes()),
                (
                    "docs/augmentations/docs-ledger.yaml",
                    LINK,
                    b"../../elsewhere.yaml",
                ),
            ],
        );
        let old = old_catalog_with(|cat| {
            add_doc(cat, &root, "led", "docs/ledger.md");
            add_augmentation(cat, "led", "{}", None);
        });
        let rows = load(&old, &RootMap::new(root.clone())).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let plan = plan_export(&repo, &commit, &rows).unwrap();
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 1);
        assert_eq!(shape[0].status, Status::Unrecoverable);
        assert_eq!(
            shape[0].reason,
            "the committed sidecar cannot be read as text"
        );
    }

    #[test]
    fn a_document_outside_the_key_space_gets_its_own_reason() {
        let plan = plan_of_files(
            &[
                (".codescout/key-space.toml", b"exclude = [\"contrib/**\"]\n"),
                ("contrib/c.md", LEDGER_MD.as_bytes()),
            ],
            |cat, root| {
                add_doc(cat, root, "c", "contrib/c.md");
                add_augmentation(cat, "c", r#"{"entries":[{"id":"F-2"}]}"#, Some("entries"));
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        for class in [Class::ParamsEntry, Class::AugmentationShape] {
            let items = items_of(&plan, class);
            assert_eq!(items.len(), 1, "{class:?}");
            assert_eq!(
                items[0].reason,
                "document is outside the key space or has no text"
            );
        }
    }

    /// The same plan, as text: items and files, for comparing two plans.
    fn plan_text(plan: &ExportPlan) -> String {
        let files: Vec<(&String, &Option<String>, &String)> = plan
            .files
            .values()
            .map(|f| (&f.path, &f.renamed_from, &f.new_text))
            .collect();
        format!("{}\n{files:?}", serde_json::to_string(&plan.items).unwrap())
    }

    #[test]
    fn rows_are_planned_in_id_order_whatever_order_the_loader_gave() {
        let forward = two_rows_plan(None, false);
        let reversed = two_rows_plan(None, true);
        assert_eq!(plan_text(&forward), plan_text(&reversed));
        let forward = two_rows_plan(Some("another prompt"), false);
        let reversed = two_rows_plan(Some("another prompt"), true);
        assert_eq!(plan_text(&forward), plan_text(&reversed));
    }

    const CLAIMED: &str = "docs/augmentations/docs-a-b.yaml";

    /// `docs/a-b.md` and `docs/a/b.md` both name `docs/augmentations/docs-a-b.yaml`.
    fn two_documents_one_sidecar(swap_ids: bool) -> ExportPlan {
        plan_of_files(
            &[
                ("docs/a-b.md", LEDGER_MD.as_bytes()),
                ("docs/a/b.md", LEDGER_MD.as_bytes()),
            ],
            |cat, root| {
                let (first, second) = if swap_ids { ("d2", "d1") } else { ("d1", "d2") };
                add_doc(cat, root, first, "docs/a-b.md");
                add_doc(cat, root, second, "docs/a/b.md");
                add_augmentation(cat, first, "{}", None);
                add_augmentation(cat, second, "{}", None);
            },
        )
    }

    #[test]
    fn two_documents_naming_one_sidecar_path_are_both_reported_and_neither_is_planned() {
        let plan = two_documents_one_sidecar(false);
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 2);
        assert!(shape.iter().all(|i| i.status == Status::Unrecoverable));
        let by_path: Vec<(&str, &str)> = shape
            .iter()
            .map(|i| (i.path.as_deref().unwrap(), i.reason.as_str()))
            .collect();
        assert_eq!(
            by_path,
            [
                (
                    "docs/a-b.md",
                    format!("sidecar path {CLAIMED} is claimed by docs/a/b.md").as_str()
                ),
                (
                    "docs/a/b.md",
                    format!("sidecar path {CLAIMED} is claimed by docs/a-b.md").as_str()
                ),
            ]
        );
        // Whichever row is read first, the plan is the same.
        assert_eq!(
            plan_text(&plan),
            plan_text(&two_documents_one_sidecar(true))
        );
    }

    #[test]
    fn a_document_naming_the_sidecar_another_document_declares_is_reported_not_overwritten() {
        let declared = LEDGER_MD.replace(
            "entry_prefix: F\n",
            &format!("entry_prefix: F\nexpects_augmentation: {CLAIMED}\n"),
        );
        let plan = plan_of_files(
            &[
                ("docs/a-b.md", LEDGER_MD.as_bytes()),
                ("docs/a/b.md", declared.as_bytes()),
                (CLAIMED, b"prompt: keep it current\n"),
            ],
            |cat, root| {
                add_doc(cat, root, "d1", "docs/a-b.md");
                add_augmentation(cat, "d1", "{}", None);
            },
        );
        assert!(plan.files.is_empty(), "{:?}", plan.files.keys());
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 1);
        assert_eq!(shape[0].status, Status::Unrecoverable);
        assert_eq!(
            shape[0].reason,
            format!("sidecar path {CLAIMED} is claimed by docs/a/b.md")
        );
    }

    /// `docs/my_notes.md` was exported by doctor: it declares and has a committed sidecar under its
    /// old name. The cutover renames it to `docs/my-notes.md`.
    fn renamed_ledger_declaring_its_old_sidecar(committed_prompt: &str) -> ExportPlan {
        let doc = "---\nkind: tracker\nentry_prefix: F\nexpects_augmentation: docs/augmentations/docs-my_notes.yaml\n---\n\n# L\n\n## F-1 — one\n\nx\n";
        let yaml = format!("prompt: {committed_prompt}\nentry_collection: entries\n");
        plan_of_files(
            &[
                ("docs/my_notes.md", doc.as_bytes()),
                ("docs/augmentations/docs-my_notes.yaml", yaml.as_bytes()),
            ],
            |cat, root| {
                add_doc(cat, root, "led", "docs/my_notes.md");
                add_augmentation(cat, "led", "{}", Some("entries"));
            },
        )
    }

    #[test]
    fn a_differing_sidecar_declared_at_the_old_name_is_drift_and_nothing_is_repointed() {
        let plan = renamed_ledger_declaring_its_old_sidecar("an older prompt");
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(files, ["docs/my-notes.md"], "the rename only");
        assert_eq!(
            declared_in(&plan.files["docs/my-notes.md"].new_text).as_deref(),
            Some("docs/augmentations/docs-my_notes.yaml")
        );
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 1);
        assert_eq!(shape[0].status, Status::Unrecoverable);
        assert_eq!(shape[0].reason, "shape drift: prompt");
        assert_eq!(shape[0].path.as_deref(), Some("docs/my-notes.md"));
    }

    #[test]
    fn an_equal_sidecar_declared_at_the_old_name_is_replaced_and_the_old_one_is_reported() {
        let plan = renamed_ledger_declaring_its_old_sidecar("keep it current");
        let files: Vec<&str> = plan.files.keys().map(String::as_str).collect();
        assert_eq!(
            files,
            ["docs/augmentations/docs-my-notes.yaml", "docs/my-notes.md"]
        );
        assert_eq!(
            declared_in(&plan.files["docs/my-notes.md"].new_text).as_deref(),
            Some("docs/augmentations/docs-my-notes.yaml")
        );
        let shape = items_of(&plan, Class::AugmentationShape);
        let statuses: Vec<Status> = shape.iter().map(|i| i.status).collect();
        assert_eq!(statuses.len(), 2, "{shape:?}");
        assert!(statuses.contains(&Status::Moved));
        let skipped = shape.iter().find(|i| i.status == Status::Skipped).unwrap();
        assert_eq!(
            skipped.reason,
            "sidecar docs/augmentations/docs-my_notes.yaml is no longer declared; remove it by hand"
        );
        assert_eq!(skipped.path.as_deref(), Some("docs/my-notes.md"));
        // Nothing deletes a file.
        assert!(plan
            .files
            .values()
            .all(|f| f.renamed_from.as_deref() != Some("docs/augmentations/docs-my_notes.yaml")));
    }

    #[test]
    fn a_declared_path_with_no_file_in_head_is_planned_as_if_there_were_no_sidecar() {
        let doc = LEDGER_MD.replace(
            "entry_prefix: F\n",
            "entry_prefix: F\nexpects_augmentation: docs/augmentations/gone.yaml\n",
        );
        let plan = plan_of_files(&[("docs/ledger.md", doc.as_bytes())], |cat, root| {
            add_doc(cat, root, "led", "docs/ledger.md");
            add_augmentation(cat, "led", "{}", None);
        });
        assert!(plan
            .files
            .contains_key("docs/augmentations/docs-ledger.yaml"));
        let shape = items_of(&plan, Class::AugmentationShape);
        assert_eq!(shape.len(), 1);
        assert_eq!(shape[0].status, Status::Moved);
    }
}
