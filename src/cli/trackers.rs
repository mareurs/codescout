//! `codescout trackers` — move a flat tracker into a lifecycle collection (Plan L-E1).
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`. Three subcommands:
//! `inventory` (Task 6) prints what one or more tracker sources hold and who names them;
//! `apply` (Task 9) writes the collection a mapping describes and deletes the sources, under the
//! project write lock, refusing before its first write when a precondition fails; `check` comes
//! in Task 10 and says "not built yet" until then. The tests are in `trackers/tests.rs`.
//!
//! Exit codes (P-R5): `0` done, `1` refused, `2` could not run. A `2` prints one line that names
//! the reason and no absolute path.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use chrono::NaiveDate;
use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::cutover::{load_and_plan, CommonArgs};
use crate::cli::write_lock::{lock_root_for, take_at};
use crate::librarian::augmentation_sidecar::rel_path_for;
use crate::librarian::current_project::lookup_git_root;
use crate::librarian::cutover::code_refs::{CodeRefs, AUDIT_DIR, DATA_EXTENSIONS, LINK_MODE};
use crate::librarian::cutover::plan::Status as CutoverStatus;
use crate::librarian::derived::build::key_space_from_commit;
use crate::librarian::derived::old_catalog::resolve_old_catalog;
use crate::librarian::derived::scan::{blob_text, open_commit, scan_commit, Skip};
use crate::librarian::frontmatter;
use crate::librarian::identity::key::key_of;
use crate::librarian::identity::key_space::KeySpace;
use crate::librarian::lifecycle::collection::{
    load_collection, load_collection_dir, CollectionLoadError,
};
use crate::librarian::lifecycle::layout::{ARCHIVE_DIR, TRACKERS_DIR};
use crate::librarian::lifecycle::schema::CollectionSchema;
use crate::librarian::read_door::{ReadDoor, ReadError};
use crate::librarian::tracker_migration::dates::blame_dates;
use crate::librarian::tracker_migration::inventory::{
    build_inventory, merge_code_hits, Inventory, SidecarInfo,
};
use crate::librarian::tracker_migration::mapping::{
    mapping_problems, parse_mapping, Mapping, MentionClass, StatusSource,
};
use crate::librarian::tracker_migration::plan::{
    plan_migration, ExistingEntry, MigrationInput, MigrationPlan, Review, SourceText,
};
use crate::librarian::tracker_migration::records::{heading_token, SourceKind};
use crate::librarian::util::{
    refuse_catalogued_path_outside_repo, refuse_write_through_symlink, write_catalogued_file,
};

#[derive(Debug, Args)]
pub struct TrackersArgs {
    #[command(subcommand)]
    pub action: TrackersAction,
}

#[derive(Debug, Subcommand)]
pub enum TrackersAction {
    /// Print what each tracker source holds at HEAD: records, sections, possible records,
    /// status values, index rows, and every file that names a source. Writes nothing.
    Inventory {
        /// The tracker sources, relative to the project directory (a folder for `--kind files`).
        #[arg(required = true)]
        paths: Vec<String>,
        /// What a record is: `file` (a record per `TOKEN — title` heading), `sections` (a record
        /// per heading of `--level`), `whole` (the file is one record) or `files` (each markdown
        /// file of the folder is one record).
        #[arg(long, default_value = "file", value_parser = ["file", "sections", "whole", "files"])]
        kind: String,
        /// The heading level of a `sections` source.
        #[arg(long)]
        level: Option<usize>,
        /// Read each record's status from this index column instead of its `**Status:**` line.
        #[arg(long = "status-column")]
        status_column: Option<String>,
        /// Project root override (defaults to cwd). The git root at or above it is read.
        #[arg(long)]
        project: Option<PathBuf>,
        /// Print the inventory as one JSON object instead of a summary.
        #[arg(long)]
        json: bool,
    },
    /// Write the collection that the mapping describes, and delete the sources. Reads every
    /// source at HEAD, takes the project write lock, and refuses before the first write when a
    /// precondition fails. A second run with the same mapping writes nothing and says
    /// "already applied". Never commits.
    Apply {
        /// The mapping file.
        mapping: PathBuf,
        /// Project root override (defaults to cwd).
        #[arg(long)]
        project: Option<PathBuf>,
        /// This repository never had an old catalog: run without the catalog-facts check when
        /// this machine has no old catalog.
        #[arg(long = "no-old-catalog")]
        no_old_catalog: bool,
        /// Print the result as one JSON object.
        #[arg(long)]
        json: bool,
    },
    /// Not built yet (Task 10).
    Check {
        /// The mapping file.
        mapping: PathBuf,
        /// Project root override (defaults to cwd).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Print the result as one JSON object.
        #[arg(long)]
        json: bool,
    },
}

/// What a run decided and what it prints. [`run`] prints it and exits with `code`.
#[derive(Debug)]
pub struct Outcome {
    /// `0` done, `1` refused, `2` could not run.
    pub code: i32,
    pub stdout: String,
}

/// Decide the outcome of `args`, resolving `--project` against `cwd`. It returns no error:
/// whatever stops the run becomes an outcome with code 2 and a one-line reason.
pub async fn evaluate(args: &TrackersArgs, cwd: &Path) -> Outcome {
    match &args.action {
        TrackersAction::Inventory {
            paths,
            kind,
            level,
            status_column,
            project,
            json,
        } => {
            let request = InventoryRequest {
                paths,
                kind,
                level: *level,
                status_column: status_column.as_deref(),
                project: project.as_deref(),
            };
            let printed = inventory_run(&request, cwd).and_then(|inv| {
                Ok(if *json {
                    format!("{}\n", serde_json::to_string(&inv)?)
                } else {
                    render_summary(&inv)
                })
            });
            match printed {
                Ok(stdout) => Outcome { code: 0, stdout },
                Err(e) => did_not_run(*json, &one_line(&e)),
            }
        }
        TrackersAction::Apply {
            mapping,
            project,
            no_old_catalog,
            json,
        } => {
            let request = ApplyRequest {
                mapping,
                project: project.as_deref(),
                no_old_catalog: *no_old_catalog,
                json: *json,
            };
            apply_outcome(&request, cwd, &ApplyEnv::from_machine()).await
        }
        TrackersAction::Check { json, .. } => {
            did_not_run(*json, "codescout trackers check is not built yet")
        }
    }
}

/// The options of one `inventory` run.
struct InventoryRequest<'a> {
    paths: &'a [String],
    kind: &'a str,
    level: Option<usize>,
    status_column: Option<&'a str>,
    project: Option<&'a Path>,
}

/// Read every source, every other tracked markdown file and the sidecar at `HEAD`, search the
/// non-markdown files of `HEAD` for each source (`CodeRefs` and a fixed-string grep for each
/// stem), and build the inventory. Every text comes from a git blob; nothing is read from the
/// working tree.
fn inventory_run(req: &InventoryRequest, cwd: &Path) -> Result<Inventory> {
    match (req.kind, req.level) {
        ("sections", None) => bail!("--kind sections needs --level"),
        ("sections", Some(_)) | (_, None) => {}
        (_, Some(_)) => bail!("--level works only with --kind sections"),
    }
    let (start, git_root) = project_roots(req.project, cwd)?;
    let repo = git2::Repository::open(&git_root)
        .map_err(|_| anyhow!("the git repository cannot be opened"))?;
    let commit =
        open_commit(&repo, "HEAD").map_err(|_| anyhow!("the repository has no commit at HEAD"))?;
    let base = start.strip_prefix(&git_root).unwrap_or(Path::new(""));

    let mut sources: Vec<(String, String, SourceKind)> = Vec::new();
    let mut needles: BTreeSet<String> = BTreeSet::new();
    for arg in req.paths {
        let rel = repo_relative(base, arg)?;
        let files = if req.kind == "files" {
            needles.insert(file_name(&rel).to_string());
            markdown_files_in(&repo, &commit, &rel)?
        } else {
            vec![rel]
        };
        for path in files {
            if sources.iter().any(|(p, _, _)| *p == path) {
                continue;
            }
            let text = blob_text(&repo, &commit, &path)?
                .ok_or_else(|| anyhow!("{path} is not in HEAD. Commit it, then run this again"))?;
            let kind = match (req.kind, req.level) {
                ("sections", Some(level)) => SourceKind::Sections { level },
                ("whole" | "files", _) => SourceKind::Whole,
                _ => SourceKind::File,
            };
            needles.insert(stem_of(&path).to_string());
            sources.push((path, text, kind));
        }
    }

    let status = match req.status_column {
        Some(header) => StatusSource::IndexColumn(header.to_string()),
        None => StatusSource::BodyLabel,
    };
    Ok(inventory_at(&repo, &commit, &sources, &needles, &status)?.0)
}

/// The project directory (`--project` resolved against `cwd`) and the git root at or above it,
/// both resolved.
fn project_roots(project: Option<&Path>, cwd: &Path) -> Result<(PathBuf, PathBuf)> {
    let start = cwd.join(project.unwrap_or(Path::new(".")));
    let start =
        std::fs::canonicalize(start).map_err(|_| anyhow!("the project path cannot be resolved"))?;
    let git_root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("the project is not inside a git repository"))?;
    Ok((start, git_root))
}

/// The inventory of `sources` (path, text at `commit`, kind) and every other markdown file of
/// `commit` (path to text) that it read. It searches the non-markdown files of `commit` for each
/// source (`CodeRefs`, and a fixed-string grep for each of `needles`) and reads each source's
/// sidecar at `commit`. `inventory` and `apply` both build their inventory here.
fn inventory_at(
    repo: &git2::Repository,
    commit: &git2::Commit,
    sources: &[(String, String, SourceKind)],
    needles: &BTreeSet<String>,
    status: &StatusSource,
) -> Result<(Inventory, BTreeMap<String, String>)> {
    let space = key_space_from_commit(repo, commit)?;
    let mut other_markdown = BTreeMap::new();
    for file in scan_commit(repo, commit, &space)? {
        if sources.iter().any(|(p, _, _)| *p == file.rel_path) {
            continue;
        }
        let text = match file.text {
            Ok(text) => Some(text),
            // The key space decides what is catalogued, not what can name a source. A file
            // that cannot be read (a symlink, not UTF-8) is skipped, as it is inside the space.
            Err(Skip::OutsideKeySpace) => blob_text(repo, commit, &file.rel_path).ok().flatten(),
            Err(Skip::NotUtf8 | Skip::Symlink) => None,
        };
        if let Some(text) = text {
            other_markdown.insert(file.rel_path, text);
        }
    }

    let code_refs = CodeRefs::load(repo, commit, &space)?;
    let mut refs_hits = Vec::new();
    for (path, _, _) in sources {
        for mention in code_refs.mentions(path) {
            if let Some((file, line)) = mention.rsplit_once(':') {
                if let Ok(line) = line.parse::<u32>() {
                    refs_hits.push((file.to_string(), line));
                }
            }
        }
    }
    let grep_hits = grep_commit(repo, commit, &space, needles)?;
    let code_hits = merge_code_hits(&refs_hits, &grep_hits);

    // The inventory holds one sidecar: a second one is a stop, never a silent drop.
    let mut sidecar: Option<(&str, SidecarInfo)> = None;
    for (path, _, _) in sources {
        let rel = rel_path_for(Path::new(""), Path::new(path));
        if let Some(text) = blob_text(repo, commit, &rel)? {
            if let Some((first, _)) = &sidecar {
                bail!(
                    "sources {first} and {path} both have a sidecar; fold sources that have a \
                     sidecar one at a time."
                );
            }
            let info = SidecarInfo {
                prompt: prompt_of(&text),
                path: rel,
            };
            sidecar = Some((path.as_str(), info));
        }
    }
    let sidecar = sidecar.map(|(_, info)| info);

    // Spec § Path mentions: `CLAUDE.md` is frozen even when no code names it.
    let frozen = |file: &str| code_refs.frozen(file).is_some() || file_name(file) == "CLAUDE.md";
    let inventory = build_inventory(
        sources,
        status,
        &other_markdown,
        &frozen,
        code_hits,
        sidecar,
    );
    Ok((inventory, other_markdown))
}

/// `arg` (relative to the project directory `base`, itself relative to the git root) as a
/// normalised repo-relative path. An absolute path, a path that leaves the repository and the
/// repository root itself are refused; the reason does not repeat an absolute path.
fn repo_relative(base: &Path, arg: &str) -> Result<String> {
    let path = Path::new(arg);
    if path.is_absolute() {
        bail!("give each source path relative to the project directory, not as an absolute path");
    }
    let mut parts: Vec<String> = base
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    for c in path.components() {
        match c {
            Component::Normal(s) => parts.push(
                s.to_str()
                    .ok_or_else(|| anyhow!("a source path is not UTF-8"))?
                    .to_string(),
            ),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    bail!("{arg} lies outside the repository");
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("{arg} is not a relative path")
            }
        }
    }
    if parts.is_empty() {
        bail!("{arg} names the repository root, not a tracker");
    }
    Ok(parts.join("/"))
}

/// The last component of a repo-relative path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The file name of `path` without its extension.
fn stem_of(path: &str) -> &str {
    let name = file_name(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// The markdown files (extension `md`, any case) directly inside `folder` at `commit`, sorted.
/// A symlink is not a file here.
fn markdown_files_in(
    repo: &git2::Repository,
    commit: &git2::Commit,
    folder: &str,
) -> Result<Vec<String>> {
    let tree = commit.tree()?;
    let entry = tree
        .get_path(Path::new(folder))
        .map_err(|_| anyhow!("{folder} is not in HEAD. Commit it, then run this again"))?;
    if entry.kind() != Some(git2::ObjectType::Tree) {
        bail!("{folder} is not a folder at HEAD, and --kind files takes a folder");
    }
    let sub = repo.find_tree(entry.id())?;
    let mut out: Vec<String> = sub
        .iter()
        .filter(|e| e.kind() == Some(git2::ObjectType::Blob) && e.filemode() != LINK_MODE)
        .filter_map(|e| e.name().map(str::to_owned))
        .filter(|name| {
            name.len() > 3 && name.as_bytes()[name.len() - 3..].eq_ignore_ascii_case(b".md")
        })
        .map(|name| format!("{folder}/{name}"))
        .collect();
    out.sort();
    if out.is_empty() {
        bail!("{folder} holds no markdown file at HEAD");
    }
    Ok(out)
}

/// What `git grep -n -F <needle> HEAD -- ':!*.md'` finds for any of `needles`: `(path, line)` of
/// every line of a non-markdown blob of `commit` that holds one of them, JSON and YAML
/// included. Like git, it compares bytes (a blob need not be UTF-8), excludes paths ending in
/// `.md` (case-sensitive, as git's pathspec), and skips a binary blob (a NUL byte in its first
/// 8000 bytes). A symlink and a submodule are not searched.
///
/// It also skips what names documents as data, as [`CodeRefs`] does ([`is_grep_data`]): the
/// audit trail, a path outside the key space, and a data or log file other than JSON. A JSON
/// file stays, because a program can read a tracker path from it.
fn grep_commit(
    repo: &git2::Repository,
    commit: &git2::Commit,
    space: &KeySpace,
    needles: &BTreeSet<String>,
) -> Result<Vec<(String, u32)>> {
    let needles: Vec<&[u8]> = needles
        .iter()
        .filter(|n| !n.is_empty())
        .map(|n| n.as_bytes())
        .collect();
    let mut hits = Vec::new();
    if needles.is_empty() {
        return Ok(hits);
    }
    let mut failure: Option<String> = None;
    let walked = commit
        .tree()?
        .walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() != Some(git2::ObjectType::Blob) || entry.filemode() == LINK_MODE {
                return git2::TreeWalkResult::Ok;
            }
            let Some(name) = entry.name() else {
                return git2::TreeWalkResult::Ok;
            };
            let path = format!("{dir}{name}");
            if path.ends_with(".md") || is_grep_data(&path, space) {
                return git2::TreeWalkResult::Ok;
            }
            let blob = match repo.find_blob(entry.id()) {
                Ok(blob) => blob,
                Err(_) => {
                    failure = Some(path);
                    return git2::TreeWalkResult::Abort;
                }
            };
            let bytes = blob.content();
            if bytes[..bytes.len().min(8000)].contains(&0) {
                return git2::TreeWalkResult::Ok;
            }
            for (i, line) in bytes.split(|&b| b == b'\n').enumerate() {
                if needles
                    .iter()
                    .any(|n| line.windows(n.len()).any(|w| w == *n))
                {
                    hits.push((path.clone(), i as u32 + 1));
                }
            }
            git2::TreeWalkResult::Ok
        });
    if let Some(path) = failure {
        bail!("{path} cannot be read at HEAD");
    }
    walked?;
    Ok(hits)
}

/// Whether the grep skips `path` as data: it lies under [`AUDIT_DIR`], the key space excludes
/// it, or its extension (ASCII case ignored) is in [`DATA_EXTENSIONS`] and is not `json`.
fn is_grep_data(path: &str, space: &KeySpace) -> bool {
    let name = file_name(path);
    let data = name.rsplit_once('.').is_some_and(|(_, ext)| {
        !ext.eq_ignore_ascii_case("json")
            && DATA_EXTENSIONS.iter().any(|d| ext.eq_ignore_ascii_case(d))
    });
    data || path.starts_with(AUDIT_DIR) || space.is_outside(path)
}

/// The `prompt` of a sidecar's YAML text; `None` when it has none or does not parse.
fn prompt_of(text: &str) -> Option<String> {
    let value: serde_yml::Value = serde_yml::from_str(text).ok()?;
    value.get("prompt")?.as_str().map(str::to_owned)
}

/// How many items of a list the text summary names before it says how many more there are.
const LISTED_IN_TEXT: usize = 10;

/// `items` joined with `, `, at most [`LISTED_IN_TEXT`] of them, then `and N more`.
fn listed(items: &[String]) -> String {
    let mut text = items
        .iter()
        .take(LISTED_IN_TEXT)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > LISTED_IN_TEXT {
        text.push_str(&format!(" and {} more", items.len() - LISTED_IN_TEXT));
    }
    text
}

/// `1 record`, `2 records`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The short STE summary of an inventory. Line numbers and every hit are in `--json`.
fn render_summary(inv: &Inventory) -> String {
    let mut out = format!(
        "Inventory at HEAD: {}.\n",
        count(inv.sources.len(), "source", "sources")
    );
    for s in &inv.sources {
        out.push_str(&format!(
            "\n{}: {}, {}, {}, {}.\n",
            s.path,
            count(s.line_count as usize, "line", "lines"),
            count(s.records.len(), "record", "records"),
            count(s.sections.len(), "section", "sections"),
            count(s.tables.len(), "index table", "index tables"),
        ));
        let values: Vec<String> = s
            .status_values
            .iter()
            .map(|(value, n)| format!("{value} ({n})"))
            .collect();
        let values = if values.is_empty() {
            "none".to_string()
        } else {
            listed(&values)
        };
        out.push_str(&format!("- Status values: {values}.\n"));
        let mut line = |label: &str, items: Vec<String>| {
            if !items.is_empty() {
                out.push_str(&format!("- {label}: {}.\n", listed(&items)));
            }
        };
        line("Records with no status", s.no_status.clone());
        line(
            "Possible records",
            s.possibles
                .iter()
                .map(|p| format!("{} (line {})", p.token, p.line))
                .collect(),
        );
        line(
            "Tokens on more than one heading",
            s.repeats
                .iter()
                .map(|r| {
                    let how = if r.nested { "nested" } else { "not nested" };
                    format!(
                        "{} (lines {} and {}, {how})",
                        r.token, r.first_line, r.other_line
                    )
                })
                .collect(),
        );
        line("Index rows with no record heading", s.row_only.clone());
        if !s.tables.is_empty() {
            line("Records with no index row", s.no_row.clone());
        }
        line(
            "Tokens with more than one index row",
            s.duplicate_rows.clone(),
        );
        line(
            "The body and the index disagree on the status of",
            s.disagreements
                .iter()
                .map(|d| format!("{} (body {}, index {})", d.token, d.body, d.index))
                .collect(),
        );
    }

    let m = &inv.mentions;
    let in_code = m.paths.iter().filter(|p| p.in_code).count();
    let frozen = m.paths.iter().filter(|p| p.frozen).count();
    out.push_str(&format!(
        "\nMentions in other markdown files: {}, {} ({in_code} in code, {frozen} in frozen files), {}.\n",
        count(m.citations.len(), "citation", "citations"),
        count(m.paths.len(), "path mention", "path mentions"),
        count(m.ids.len(), "id mention", "id mentions"),
    ));
    let files: BTreeSet<&str> = m.code.iter().map(|h| h.file.as_str()).collect();
    let grep_only = m.code.iter().filter(|h| h.origin == "grep").count();
    out.push_str(&format!(
        "Non-markdown files that name a source: {} in {}. git grep alone found {grep_only} of them.\n",
        count(m.code.len(), "hit", "hits"),
        count(files.len(), "file", "files"),
    ));
    match &inv.sidecar {
        Some(sidecar) => {
            let prompt = if sidecar.prompt.is_some() {
                "it has a prompt"
            } else {
                "it has no prompt"
            };
            out.push_str(&format!("Sidecar: {} ({prompt}).\n", sidecar.path));
        }
        None => out.push_str("Sidecar: none.\n"),
    }
    out.push_str("Use --json for every line number and every hit.\n");
    out
}

/// The reason on one line: a context chain spans several.
fn one_line(e: &anyhow::Error) -> String {
    format!("{e:#}")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn did_not_run(json: bool, reason: &str) -> Outcome {
    let stdout = if json {
        format!(
            "{}\n",
            serde_json::json!({ "ran": false, "reason": reason })
        )
    } else {
        format!("The command did not run: {reason}\n")
    };
    Outcome { code: 2, stdout }
}

/// The options of one `apply` run.
struct ApplyRequest<'a> {
    mapping: &'a Path,
    project: Option<&'a Path>,
    no_old_catalog: bool,
    json: bool,
}

/// What `apply` takes from the machine, so that a test can set it.
struct ApplyEnv {
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    /// The later of the local date and the UTC date (plan P-R6).
    now: NaiveDate,
    lock_wait: Duration,
}

impl ApplyEnv {
    fn from_machine() -> Self {
        let local = chrono::Local::now().date_naive();
        let utc = chrono::Utc::now().date_naive();
        ApplyEnv {
            librarian_db: std::env::var_os("LIBRARIAN_DB"),
            data_dir: dirs::data_local_dir(),
            now: local.max(utc),
            lock_wait: LOCK_WAIT,
        }
    }
}

/// How long `apply` waits for the project write lock before it stops (plan Task 9).
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// The refusal when this machine has no old catalog and `--no-old-catalog` is not given.
const NO_OLD_CATALOG: &str = "No old catalog is on this machine: LIBRARIAN_DB names no file, \
     and the local data directory has no librarian/catalog.db. So apply cannot make sure that \
     the catalog facts of the sources are in files. If this repository never had an old \
     catalog, run this again with --no-old-catalog.";

/// The refusal for a shallow clone (spec § Dates).
const SHALLOW: &str = "this clone is shallow, so blame dates would be wrong. Run git fetch \
     --unshallow, then run this again.";

/// Run `apply`. It returns no error: a refusal is code 1, a run that could not start is code 2.
async fn apply_outcome(req: &ApplyRequest<'_>, cwd: &Path, env: &ApplyEnv) -> Outcome {
    match apply_run(req, cwd, env).await {
        Ok(done) => Outcome {
            code: 0,
            stdout: render_applied(&done, req.json),
        },
        Err(Stop::Refused(reasons)) => refused(req.json, &reasons),
        Err(Stop::CouldNotRun(reason)) => did_not_run(req.json, &reason),
    }
}

/// Why `apply` stopped before its first write.
enum Stop {
    /// A precondition failed: one line per reason, exit 1.
    Refused(Vec<String>),
    /// The run could not start (not a git repository, a source missing at HEAD, the lock not
    /// free): one line, exit 2.
    CouldNotRun(String),
}

impl From<anyhow::Error> for Stop {
    fn from(e: anyhow::Error) -> Self {
        Stop::CouldNotRun(one_line(&e))
    }
}

/// What `apply` did. Every path is repo-relative, each list in path order.
#[derive(Debug, Serialize)]
struct Applied {
    written: Vec<String>,
    deleted: Vec<String>,
    /// Planned writes whose file already holds the planned text, and planned deletes of a
    /// file that is at HEAD and already gone from the working tree.
    already_applied: Vec<String>,
    review: Review,
}

/// The steps of `apply`, in the order of plan Task 9. Every refusal is decided before the first
/// write: the writes and deletes happen only in [`carry_out`], after all reasons are known.
async fn apply_run(req: &ApplyRequest<'_>, cwd: &Path, env: &ApplyEnv) -> Result<Applied, Stop> {
    // 1. The git root and the project write lock, held to the end of the run.
    let (start, git_root) = project_roots(req.project, cwd)?;
    let _lock = take_at(
        &lock_root_for(&start, &git_root),
        env.lock_wait,
        "trackers apply",
    )
    .await
    .map_err(|e| Stop::CouldNotRun(lock_text(&e)))?;

    // 2. The mapping, the inventory of its sources at HEAD, and what the mapping leaves open.
    let mapping = read_mapping(&cwd.join(req.mapping))?;
    let repo = git2::Repository::open(&git_root)
        .map_err(|_| anyhow!("the git repository cannot be opened"))?;
    let commit =
        open_commit(&repo, "HEAD").map_err(|_| anyhow!("the repository has no commit at HEAD"))?;
    let work = worktree_changes(&git_root)?;
    let head = read_head(&repo, &commit, &mapping, &work.untracked)?;
    let mut reasons = mapping_problems(&mapping, &head.inventory);

    // 3. Preconditions (a) catalog facts, (b) readers, (d) not shallow.
    reasons.extend(catalog_facts(&head, &start, cwd, env, req.no_old_catalog)?);
    reasons.extend(readers(&mapping, &head.inventory));
    if is_shallow(&repo) {
        reasons.push(SHALLOW.to_string());
        return Err(Stop::Refused(dedup(reasons)));
    }

    // 4-5. The plan from HEAD and its problems.
    let plan = match plan_from_head(&repo, &commit, &git_root, &mapping, &head, env.now) {
        Ok(plan) => plan,
        Err(Stop::Refused(more)) => {
            reasons.extend(more);
            return Err(Stop::Refused(dedup(reasons)));
        }
        Err(stop) => return Err(stop),
    };
    reasons.extend(plan.problems.iter().cloned());

    // 3(c) needs the plan's paths, and step 6 decides each path before anything is written.
    let dirty = dirty_paths(&git_root, &work.changed, &plan);
    let refused_paths: BTreeSet<String> = dirty.iter().map(|(p, _)| p.clone()).collect();
    reasons.extend(dirty.into_iter().map(|(_, reason)| reason));
    let steps = decide(
        &repo,
        &commit,
        &git_root,
        &plan,
        &head,
        &refused_paths,
        &mut reasons,
    );
    let reasons = dedup(reasons);
    if !reasons.is_empty() {
        return Err(Stop::Refused(reasons));
    }

    // 6. Only now: write, then delete.
    carry_out(&git_root, &plan, steps)
}

/// Why `take_at` did not give the lock, on one line that names no path: the lock file cannot be
/// opened (only the kind of the `io::Error` is kept, its text names the path), or another
/// process holds the lock (the holder record names no path).
fn lock_text(e: &anyhow::Error) -> String {
    match e.downcast_ref::<std::io::Error>() {
        Some(io) => format!(
            "the project write lock file cannot be opened: {}",
            io.kind()
        ),
        None => format!("the project write lock is not free: {}", one_line(e)),
    }
}

/// Read the mapping through the read door and parse it. A read failure means the run cannot
/// start; a parse problem is a refusal.
fn read_mapping(path: &Path) -> Result<Mapping, Stop> {
    let text = ReadDoor::new(Vec::new())
        .text(path)
        .map_err(|e| match e {
            ReadError::Io(io) => format!("the mapping file cannot be read: {}", io.kind()),
            ReadError::Refused(_) => {
                "the mapping file cannot be read: it is a link out of its folder, or not a \
                 regular file"
                    .to_string()
            }
        })
        .map_err(Stop::CouldNotRun)?;
    parse_mapping(&text).map_err(Stop::Refused)
}

/// The uncommitted state of the working tree, from `git status`.
#[derive(Default)]
struct WorkTree {
    /// Every path with a staged, unstaged or untracked change, sorted.
    changed: Vec<String>,
    /// The untracked paths.
    untracked: Vec<String>,
}

/// `git status --porcelain=v1 -z` with every untracked file listed one by one (an untracked
/// folder would otherwise be one entry) and no rename pairs (a rename is a delete and an add).
fn worktree_changes(git_root: &Path) -> Result<WorkTree> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(git_root)
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--no-renames",
        ])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(|_| anyhow!("git cannot be started, and apply needs git status"))?;
    if !out.status.success() {
        bail!("git status failed in the repository");
    }
    let text = String::from_utf8(out.stdout)
        .map_err(|_| anyhow!("git status gave a path that is not UTF-8"))?;
    let mut work = WorkTree::default();
    for record in text.split('\0').filter(|r| !r.is_empty()) {
        let (Some(code), Some(path)) = (record.get(..2), record.get(3..)) else {
            bail!("git status gave a line that apply cannot read");
        };
        if code == "??" {
            work.untracked.push(path.to_string());
        }
        work.changed.push(path.to_string());
    }
    work.changed.sort();
    work.changed.dedup();
    Ok(work)
}

/// One source of the mapping, read at HEAD. A `files` source is one of these per markdown file.
struct HeadSource {
    path: String,
    text: String,
    kind: SourceKind,
    /// The lowercase file stem of a `files` source (plan P-R2).
    fixed_stem: Option<String>,
}

/// What one mapping reads at HEAD before it plans. `apply` builds it with [`read_head`], and
/// `check` (Task 10) builds it the same way before [`plan_from_head`].
struct HeadView {
    sources: Vec<HeadSource>,
    /// Every other markdown file of HEAD, path to text.
    other_markdown: BTreeMap<String, String>,
    /// Each key to its paths: every tracked path of HEAD and every untracked file under
    /// `docs/`, so the entries of an earlier run are seen with their paths.
    key_paths: BTreeMap<String, BTreeSet<String>>,
    /// Every file path of the HEAD tree.
    tracked: BTreeSet<String>,
    inventory: Inventory,
}

/// Read the sources of `m` at `commit` (a `files` folder expanded into one whole source per
/// markdown file), build their inventory with the mapping's status source, and collect the key
/// space with paths. `untracked` comes from `git status`.
fn read_head(
    repo: &git2::Repository,
    commit: &git2::Commit,
    m: &Mapping,
    untracked: &[String],
) -> Result<HeadView, Stop> {
    let mut sources: Vec<HeadSource> = Vec::new();
    let mut needles: BTreeSet<String> = BTreeSet::new();
    let mut bad_paths: Vec<String> = Vec::new();
    for spec in &m.sources {
        match repo_relative(Path::new(""), &spec.path) {
            Ok(normal) if normal == spec.path => {}
            Ok(normal) => {
                bad_paths.push(format!(
                    "source {} is not a normalised repo-relative path. Write it as {normal}.",
                    spec.path
                ));
                continue;
            }
            Err(e) => {
                bad_paths.push(format!("source {}: {}", spec.path, one_line(&e)));
                continue;
            }
        }
        let files: Vec<(String, SourceKind, Option<String>)> = match spec.kind.as_str() {
            "files" => {
                needles.insert(file_name(&spec.path).to_string());
                markdown_files_in(repo, commit, &spec.path)?
                    .into_iter()
                    .map(|path| {
                        let stem = stem_of(&path).to_ascii_lowercase();
                        (path, SourceKind::Whole, Some(stem))
                    })
                    .collect()
            }
            "sections" => {
                let Some(level) = spec.level else {
                    bad_paths.push(format!(
                        "source {} has kind \"sections\" but no level",
                        spec.path
                    ));
                    continue;
                };
                vec![(spec.path.clone(), SourceKind::Sections { level }, None)]
            }
            "whole" => vec![(spec.path.clone(), SourceKind::Whole, None)],
            _ => vec![(spec.path.clone(), SourceKind::File, None)],
        };
        for (path, kind, fixed_stem) in files {
            if sources.iter().any(|s| s.path == path) {
                continue;
            }
            let text = blob_text(repo, commit, &path)?
                .ok_or_else(|| anyhow!("{path} is not in HEAD. Commit it, then run this again"))?;
            needles.insert(stem_of(&path).to_string());
            sources.push(HeadSource {
                path,
                text,
                kind,
                fixed_stem,
            });
        }
    }
    if !bad_paths.is_empty() {
        return Err(Stop::Refused(bad_paths));
    }

    let triples: Vec<(String, String, SourceKind)> = sources
        .iter()
        .map(|s| (s.path.clone(), s.text.clone(), s.kind.clone()))
        .collect();
    // Decision 3: the inventory reads statuses where the planner reads them.
    let (inventory, other_markdown) =
        inventory_at(repo, commit, &triples, &needles, &m.status_source)?;
    let (key_paths, tracked) = key_space_paths(commit, untracked)?;
    Ok(HeadView {
        sources,
        other_markdown,
        key_paths,
        tracked,
        inventory,
    })
}

/// Each key (`key_of`) to the paths that have it, over every file of the HEAD tree and every
/// untracked path under `docs/`; and the set of tracked file paths.
#[allow(clippy::type_complexity)]
fn key_space_paths(
    commit: &git2::Commit,
    untracked: &[String],
) -> Result<(BTreeMap<String, BTreeSet<String>>, BTreeSet<String>)> {
    let mut tracked: BTreeSet<String> = BTreeSet::new();
    commit
        .tree()?
        .walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                if let Some(name) = entry.name() {
                    tracked.insert(format!("{dir}{name}"));
                }
            }
            git2::TreeWalkResult::Ok
        })?;
    let mut keys: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let docs = untracked.iter().filter(|p| p.starts_with("docs/"));
    for path in tracked.iter().chain(docs) {
        if let Ok(key) = key_of(path) {
            keys.entry(key.as_str().to_string())
                .or_default()
                .insert(path.clone());
        }
    }
    Ok((keys, tracked))
}

/// Build the planner's input from HEAD and plan. This is the one place that does it: `check`
/// (Task 10) re-plans with this function, so it compares against the plan `apply` wrote.
///
/// Each source's text is the HEAD text in `head`, and its blame is taken at `commit` with the
/// path relative to the git root (the project may sit below it). In `existing` mode the
/// collection is loaded and each entry's text is read from the working tree through the read
/// door. `Refused` when `existing` names no collection that loads at the planned folder;
/// `CouldNotRun` when a blame or an entry read fails.
fn plan_from_head(
    repo: &git2::Repository,
    commit: &git2::Commit,
    git_root: &Path,
    m: &Mapping,
    head: &HeadView,
    now: NaiveDate,
) -> Result<MigrationPlan, Stop> {
    let mut sources = Vec::new();
    for s in &head.sources {
        let blame = blame_dates(repo, commit, &s.path).map_err(|_| {
            Stop::CouldNotRun(format!(
                "{} has no blame at HEAD; commit it, then run this again",
                s.path
            ))
        })?;
        sources.push(SourceText {
            path: s.path.clone(),
            kind: s.kind.clone(),
            text: s.text.clone(),
            blame,
            fixed_stem: s.fixed_stem.clone(),
        });
    }
    let existing = if m.target.existing {
        Some(existing_collection(git_root, m)?)
    } else {
        None
    };
    let input = MigrationInput {
        mapping: m,
        sources,
        other_markdown: &head.other_markdown,
        key_paths: &head.key_paths,
        existing: existing
            .as_ref()
            .map(|(schema, template, entries)| (schema, template.as_deref(), entries.clone())),
        now,
    };
    Ok(plan_migration(&input))
}

/// The collection folder the planner writes into: `docs/trackers/<lifecycle>/[archive/]<key>`.
/// The same rule as `plan_migration`, which does not export it.
fn collection_dir(m: &Mapping, key: &str) -> String {
    let lifecycle = &m.target.lifecycle;
    if m.target.archived {
        format!("{TRACKERS_DIR}/{lifecycle}/{ARCHIVE_DIR}/{key}")
    } else {
        format!("{TRACKERS_DIR}/{lifecycle}/{key}")
    }
}

/// The collection key of `m`: `target.key`, else the lowercase stem of the first source (the
/// planner's rule).
fn collection_key(m: &Mapping) -> String {
    if let Some(key) = &m.target.key {
        return key.clone();
    }
    let first = m.sources.first().map_or("", |s| s.path.as_str());
    let name = file_name(first.trim_end_matches('/'));
    let stem = if name.to_ascii_lowercase().ends_with(".md") {
        &name[..name.len() - 3]
    } else {
        name
    };
    stem.to_ascii_lowercase()
}

/// The schema, the entry template and the entries of the collection `existing` mode adds to.
/// Each entry's text is read from the working tree through the read door.
#[allow(clippy::type_complexity)]
fn existing_collection(
    git_root: &Path,
    m: &Mapping,
) -> Result<(CollectionSchema, Option<String>, Vec<ExistingEntry>), Stop> {
    let key = collection_key(m);
    let dir = collection_dir(m, &key);
    let loaded = if m.target.archived {
        load_collection_dir(git_root, &dir)
    } else {
        load_collection(git_root, &key)
    };
    let loaded = loaded.map_err(|e| {
        let why = match e {
            CollectionLoadError::NotFound(_) | CollectionLoadError::NotACollectionDir(_) => {
                "the folder has no readme".to_string()
            }
            CollectionLoadError::Unreadable(_) => "a file of it cannot be read".to_string(),
            other => other.to_string(),
        };
        Stop::Refused(vec![format!(
            "target.existing is true, but {dir} holds no collection that loads: {why}"
        )])
    })?;
    if loaded.dir != dir {
        return Err(Stop::Refused(vec![format!(
            "target.existing is true, but the collection {key} is in {}, not in {dir}. Set \
             target.lifecycle and target.archived to match it.",
            loaded.dir
        )]));
    }
    let node = loaded.node();
    let door = ReadDoor::new(vec![git_root.to_path_buf()]);
    let mut entries = Vec::new();
    let paths = node
        .entries
        .iter()
        .map(|e| e.rel_path.clone())
        .chain(node.unparsed.iter().cloned());
    for rel in paths {
        let text = door
            .text(&git_root.join(&rel))
            .map_err(|_| Stop::CouldNotRun(format!("{rel} cannot be read")))?;
        let token = first_heading_token(&text);
        entries.push(ExistingEntry {
            rel_path: rel,
            text,
            token,
        });
    }
    Ok((loaded.schema().clone(), loaded.template.clone(), entries))
}

/// The record token of the first heading of an entry's body.
fn first_heading_token(text: &str) -> Option<String> {
    let body = frontmatter::parse(text).map_or(text, |(_, body)| body);
    body.lines()
        .find(|l| l.starts_with('#'))
        .and_then(heading_token)
        .map(|(token, _)| token)
}

/// Precondition (a): the identity cutover has nothing left to do for a source or its sidecar: a
/// cutover plan of HEAD writes neither and moves no item of either (spec § Preconditions, plan
/// P-R4). A plan that still rewrites an old citation form in a source refuses too: the
/// cutover has not run there. With no old catalog on this machine it is a refusal unless
/// `no_old_catalog`; when a catalog exists the check runs whatever the flag says.
fn catalog_facts(
    head: &HeadView,
    start: &Path,
    cwd: &Path,
    env: &ApplyEnv,
    no_old_catalog: bool,
) -> Result<Vec<String>, Stop> {
    let found = resolve_old_catalog(None, env.librarian_db.clone(), env.data_dir.clone())
        .filter(|path| std::fs::metadata(path).is_ok());
    if found.is_none() {
        return Ok(if no_old_catalog {
            Vec::new()
        } else {
            vec![NO_OLD_CATALOG.to_string()]
        });
    }
    let args = CommonArgs {
        project: Some(start.to_path_buf()),
        old_db: None,
        old_root: Vec::new(),
        json: false,
    };
    let loaded = load_and_plan(&args, cwd, env.librarian_db.clone(), env.data_dir.clone())?;
    let mut watched: BTreeSet<String> = BTreeSet::new();
    for s in &head.sources {
        watched.insert(s.path.clone());
        watched.insert(rel_path_for(Path::new(""), Path::new(&s.path)));
    }
    let mut hit: BTreeSet<String> = BTreeSet::new();
    for (path, change) in &loaded.plan.files {
        for p in std::iter::once(path).chain(change.renamed_from.as_ref()) {
            if watched.contains(p) {
                hit.insert(p.clone());
            }
        }
    }
    for item in &loaded.plan.items {
        if item.status == CutoverStatus::Moved {
            if let Some(p) = item.path.as_ref().filter(|p| watched.contains(*p)) {
                hit.insert(p.clone());
            }
        }
    }
    Ok(hit
        .into_iter()
        .map(|p| {
            format!(
                "{p}: the identity cutover still has catalog facts or old citation forms to move \
                 into this file. Run codescout cutover export, commit the result, then run this \
                 again."
            )
        })
        .collect())
}

/// Precondition (b): each non-markdown file that names a source and is classed `reader`.
/// (A file with no class is a `mapping_problems` line.)
fn readers(m: &Mapping, inv: &Inventory) -> Vec<String> {
    let files: BTreeSet<&str> = inv
        .mentions
        .code
        .iter()
        .map(|h| h.file.as_str())
        .filter(|f| m.code_mentions.get(*f) == Some(&MentionClass::Reader))
        .collect();
    files
        .into_iter()
        .map(|f| {
            format!(
                "{f} reads a source (code_mentions class reader). Switch that reader to the \
                 collection first, then run this again."
            )
        })
        .collect()
}

/// Precondition (d). A linked worktree keeps `shallow` in the common dir, where libgit2's
/// `is_shallow` does not look (as `blame_dates` checks it). `metadata` reads no content.
fn is_shallow(repo: &git2::Repository) -> bool {
    let common = std::fs::metadata(repo.commondir().join("shallow"))
        .map(|m| m.len() > 0)
        .unwrap_or(false);
    repo.is_shallow() || common
}

/// Precondition (c): every changed path under `docs/trackers/` is one the plan writes or
/// deletes (a previous run's output), and no other path the plan writes has a change, so a
/// restore of apply's list cannot destroy another session's work. A changed path outside
/// `docs/trackers/` whose working text already equals the planned text is a previous run's
/// output too (`already applied`, decided in [`decide`]). Each refused path with its reason.
fn dirty_paths(git_root: &Path, changed: &[String], plan: &MigrationPlan) -> Vec<(String, String)> {
    let trackers = format!("{TRACKERS_DIR}/");
    let door = ReadDoor::new(vec![git_root.to_path_buf()]);
    let mut out = Vec::new();
    for p in changed {
        let touched = plan.writes.contains_key(p) || plan.deletes.contains(p);
        if p.starts_with(&trackers) {
            if !touched {
                out.push((
                    p.clone(),
                    format!(
                        "{p} has an uncommitted change, and this plan does not write or delete \
                         it. Commit or restore it, then run this again."
                    ),
                ));
            }
        } else if let Some(planned) = plan.writes.get(p) {
            let applied = door
                .text(&git_root.join(p))
                .is_ok_and(|on_disk| same_text(&on_disk, planned));
            if !applied {
                out.push((
                    p.clone(),
                    format!(
                        "{p} has an uncommitted change, and apply would rewrite it. Commit or \
                         restore it, then run this again."
                    ),
                ));
            }
        }
    }
    out
}

/// What `apply` does with each planned path, decided before the first write.
#[derive(Default)]
struct Steps {
    /// Planned writes whose file is absent, or is another markdown file that still holds its
    /// HEAD text (the key-change rewrite).
    write: Vec<String>,
    /// Planned writes whose file already holds the planned text.
    equal: Vec<String>,
    /// Planned deletes whose file is there and equals its HEAD text.
    delete: Vec<String>,
    /// Planned deletes of a file that is at HEAD and gone from the working tree.
    gone: Vec<String>,
}

/// Decide each planned path, pushing a reason for each one apply must not touch. A path in
/// `refused` already has its reason (the clean-tree check) and gets no second one.
///
/// * A write target that is a link or lies outside the repository (the write door would refuse
///   it after other writes), or exists with other text. Another markdown file that still holds
///   its HEAD text is rewritten; with any third text it is refused too.
/// * A delete target that is a link or lies outside the repository, or whose text is not its
///   HEAD text: git could not restore what the delete removes.
fn decide(
    repo: &git2::Repository,
    commit: &git2::Commit,
    git_root: &Path,
    plan: &MigrationPlan,
    head: &HeadView,
    refused: &BTreeSet<String>,
    reasons: &mut Vec<String>,
) -> Steps {
    let roots = vec![git_root.to_path_buf()];
    let door = ReadDoor::new(roots.clone());
    let mut steps = Steps::default();
    for (path, text) in plan.writes.iter().filter(|(p, _)| !refused.contains(*p)) {
        let abs = git_root.join(path);
        if refuse_write_through_symlink(&abs).is_err()
            || refuse_catalogued_path_outside_repo(&roots, &abs, "write").is_err()
        {
            reasons.push(format!(
                "{path} is a symbolic link, or lies outside the repository. Apply does not write \
                 through it."
            ));
            continue;
        }
        match std::fs::symlink_metadata(&abs) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => steps.write.push(path.clone()),
            Err(e) => reasons.push(format!("{path} cannot be inspected: {}", e.kind())),
            Ok(_) => match door.text(&abs) {
                Ok(on_disk) if same_text(&on_disk, text) => steps.equal.push(path.clone()),
                Ok(on_disk)
                    if head
                        .other_markdown
                        .get(path)
                        .is_some_and(|at_head| same_text(&on_disk, at_head)) =>
                {
                    steps.write.push(path.clone())
                }
                Ok(_) => reasons.push(format!(
                    "{path} exists and holds other text than apply would write. Move it aside, \
                     or make it equal, then run this again."
                )),
                Err(_) => reasons.push(format!(
                    "{path} exists and cannot be read. Move it aside, then run this again."
                )),
            },
        }
    }
    for path in plan.deletes.iter().filter(|p| !refused.contains(*p)) {
        let abs = git_root.join(path);
        match std::fs::symlink_metadata(&abs) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if head.tracked.contains(path) {
                    steps.gone.push(path.clone());
                }
            }
            Err(e) => reasons.push(format!("{path} cannot be inspected: {}", e.kind())),
            Ok(_) => {
                if refuse_write_through_symlink(&abs).is_err()
                    || refuse_catalogued_path_outside_repo(&roots, &abs, "delete").is_err()
                {
                    reasons.push(format!(
                        "{path} is a symbolic link, or lies outside the repository. Apply does \
                         not delete through it."
                    ));
                    continue;
                }
                let at_head = blob_text(repo, commit, path).ok().flatten();
                match (door.text(&abs), at_head) {
                    (Ok(now), Some(then)) if same_text(&now, &then) => {
                        steps.delete.push(path.clone())
                    }
                    (_, None) => reasons.push(format!(
                        "{path} is not in HEAD, and apply would delete it, so git could not \
                         restore it. Commit it or move it aside, then run this again."
                    )),
                    _ => reasons.push(format!(
                        "{path} differs from its text at HEAD, and apply would delete it. Commit \
                         it first (apply reads sources at HEAD), then run this again."
                    )),
                }
            }
        }
    }
    steps
}

/// Two texts are equal when they differ at most in `\r\n` against `\n`.
fn same_text(a: &str, b: &str) -> bool {
    a.replace("\r\n", "\n") == b.replace("\r\n", "\n")
}

/// Write each decided path through the write door (after its folder is made), then delete each
/// decided path after the door's two guards. A failure here stops the run with what was done
/// before it: a second run completes the first, because an equal file is already applied.
fn carry_out(git_root: &Path, plan: &MigrationPlan, steps: Steps) -> Result<Applied, Stop> {
    let roots = vec![git_root.to_path_buf()];
    let mut written: Vec<String> = Vec::new();
    let mut deleted: Vec<String> = Vec::new();
    for path in &steps.write {
        let abs = git_root.join(path);
        let made = match abs.parent() {
            Some(dir) => std::fs::create_dir_all(dir).map_err(anyhow::Error::from),
            None => Ok(()),
        };
        made.and_then(|()| write_catalogued_file(&roots, &abs, &plan.writes[path]))
            .map_err(|e| stopped(path, "written", &e, written.len(), deleted.len()))?;
        written.push(path.clone());
    }
    for path in &steps.delete {
        let abs = git_root.join(path);
        refuse_write_through_symlink(&abs)
            .and_then(|()| refuse_catalogued_path_outside_repo(&roots, &abs, "delete"))
            .and_then(|()| std::fs::remove_file(&abs).map_err(anyhow::Error::from))
            .map_err(|e| stopped(path, "deleted", &e, written.len(), deleted.len()))?;
        deleted.push(path.clone());
    }
    let mut already_applied: Vec<String> = steps.equal.into_iter().chain(steps.gone).collect();
    already_applied.sort();
    Ok(Applied {
        written,
        deleted,
        already_applied,
        review: plan.review.clone(),
    })
}

/// The one-line stop of a write or delete that failed, without the error text (it can name an
/// absolute path).
fn stopped(path: &str, what: &str, e: &anyhow::Error, written: usize, deleted: usize) -> Stop {
    let cause = match e.downcast_ref::<std::io::Error>() {
        Some(io) => io.kind().to_string(),
        None => "the write door refused it".to_string(),
    };
    Stop::CouldNotRun(format!(
        "{path} could not be {what} ({cause}). Before it, apply wrote {written} and deleted \
         {deleted} files. Fix the cause, then run this again: the next run completes this one."
    ))
}

/// `reasons` without a repeat, in first-seen order: `mapping_problems` and the planner word a
/// missing sections entry and an unsettled possible record the same.
fn dedup(reasons: Vec<String>) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    reasons
        .into_iter()
        .filter(|r| seen.insert(r.clone()))
        .collect()
}

/// The outcome of a refusal: one line per reason (a reason that spans lines is joined), exit 1.
fn refused(json: bool, reasons: &[String]) -> Outcome {
    let lines: Vec<String> = reasons
        .iter()
        .map(|r| {
            r.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let stdout = if json {
        format!("{}\n", serde_json::json!({ "refused": lines }))
    } else {
        lines.iter().map(|l| format!("{l}\n")).collect()
    };
    Outcome { code: 1, stdout }
}

/// What a successful run prints: the JSON object, or one line per group with its count and the
/// review counts.
fn render_applied(done: &Applied, json: bool) -> String {
    if json {
        return format!(
            "{}\n",
            serde_json::to_string(done).unwrap_or_else(|_| "{}".to_string())
        );
    }
    let group = |label: &str, items: &[String]| {
        if items.is_empty() {
            format!("{label}: 0 files.\n")
        } else {
            format!(
                "{label}: {}: {}.\n",
                count(items.len(), "file", "files"),
                listed(items)
            )
        }
    };
    let r = &done.review;
    let mut out = String::new();
    out.push_str(&group("Written", &done.written));
    out.push_str(&group("Deleted", &done.deleted));
    out.push_str(&group("Already applied", &done.already_applied));
    out.push_str(&format!(
        "Review: {}, {}, {}, {}, {}, {}.\n",
        count(r.derived_dates.len(), "derived date", "derived dates"),
        count(r.dropped.len(), "dropped section", "dropped sections"),
        count(
            r.skipped_rewrites.len(),
            "record with skipped rewrites",
            "records with skipped rewrites"
        ),
        count(r.row_only.len(), "row-only record", "row-only records"),
        count(
            r.no_row.len(),
            "record with no index row",
            "records with no index row"
        ),
        count(
            r.disagreements.len(),
            "status disagreement",
            "status disagreements"
        ),
    ));
    let long = [&done.written, &done.deleted, &done.already_applied]
        .iter()
        .any(|l| l.len() > LISTED_IN_TEXT);
    if long {
        out.push_str("Use --json for every path.\n");
    }
    out
}

/// Evaluate with the current directory, print, and exit with the code when it is not `0`.
pub async fn run(args: TrackersArgs) -> Result<()> {
    use std::io::Write;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let out = evaluate(&args, &cwd).await;
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(out.stdout.as_bytes());
    let _ = stdout.flush();
    if out.code != 0 {
        std::process::exit(out.code);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
