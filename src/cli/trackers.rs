//! `codescout trackers` — move a flat tracker into a lifecycle collection (Plan L-E1).
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`. Three subcommands:
//! `inventory` (this file, Task 6) prints what one or more tracker sources hold and who names
//! them; `apply` and `check` come in Tasks 9 and 10 and say "not built yet" until then.
//!
//! Exit codes (P-R5): `0` done, `1` refused, `2` could not run. A `2` prints one line that names
//! the reason and no absolute path.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use clap::{Args, Subcommand};

use crate::librarian::augmentation_sidecar::rel_path_for;
use crate::librarian::current_project::lookup_git_root;
use crate::librarian::cutover::code_refs::{CodeRefs, AUDIT_DIR, DATA_EXTENSIONS, LINK_MODE};
use crate::librarian::derived::build::key_space_from_commit;
use crate::librarian::derived::scan::{blob_text, open_commit, scan_commit, Skip};
use crate::librarian::identity::key_space::KeySpace;
use crate::librarian::tracker_migration::inventory::{
    build_inventory, merge_code_hits, Inventory, SidecarInfo,
};
use crate::librarian::tracker_migration::mapping::StatusSource;
use crate::librarian::tracker_migration::records::SourceKind;

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
    /// Not built yet (Task 9).
    Apply {
        /// The mapping file.
        mapping: PathBuf,
        /// Project root override (defaults to cwd).
        #[arg(long)]
        project: Option<PathBuf>,
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
pub fn evaluate(args: &TrackersArgs, cwd: &Path) -> Outcome {
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
        TrackersAction::Apply { json, .. } => {
            did_not_run(*json, "codescout trackers apply is not built yet")
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
    let start = cwd.join(req.project.unwrap_or(Path::new(".")));
    let start =
        std::fs::canonicalize(start).map_err(|_| anyhow!("the project path cannot be resolved"))?;
    let git_root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("the project is not inside a git repository"))?;
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

    let space = key_space_from_commit(&repo, &commit)?;
    let mut other_markdown = BTreeMap::new();
    for file in scan_commit(&repo, &commit, &space)? {
        if sources.iter().any(|(p, _, _)| *p == file.rel_path) {
            continue;
        }
        let text = match file.text {
            Ok(text) => Some(text),
            // The key space decides what is catalogued, not what can name a source. A file
            // that cannot be read (a symlink, not UTF-8) is skipped, as it is inside the space.
            Err(Skip::OutsideKeySpace) => blob_text(&repo, &commit, &file.rel_path).ok().flatten(),
            Err(Skip::NotUtf8 | Skip::Symlink) => None,
        };
        if let Some(text) = text {
            other_markdown.insert(file.rel_path, text);
        }
    }

    let code_refs = CodeRefs::load(&repo, &commit, &space)?;
    let mut refs_hits = Vec::new();
    for (path, _, _) in &sources {
        for mention in code_refs.mentions(path) {
            if let Some((file, line)) = mention.rsplit_once(':') {
                if let Ok(line) = line.parse::<u32>() {
                    refs_hits.push((file.to_string(), line));
                }
            }
        }
    }
    let grep_hits = grep_commit(&repo, &commit, &space, &needles)?;
    let code_hits = merge_code_hits(&refs_hits, &grep_hits);

    // The inventory holds one sidecar: a second one is a stop, never a silent drop.
    let mut sidecar: Option<(&str, SidecarInfo)> = None;
    for (path, _, _) in &sources {
        let rel = rel_path_for(Path::new(""), Path::new(path));
        if let Some(text) = blob_text(&repo, &commit, &rel)? {
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

    let status = match req.status_column {
        Some(header) => StatusSource::IndexColumn(header.to_string()),
        None => StatusSource::BodyLabel,
    };
    // Spec § Path mentions: `CLAUDE.md` is frozen even when no code names it.
    let frozen = |file: &str| code_refs.frozen(file).is_some() || file_name(file) == "CLAUDE.md";
    Ok(build_inventory(
        &sources,
        &status,
        &other_markdown,
        &frozen,
        code_hits,
        sidecar,
    ))
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

/// Evaluate with the current directory, print, and exit with the code when it is not `0`.
pub async fn run(args: TrackersArgs) -> Result<()> {
    use std::io::Write;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let out = evaluate(&args, &cwd);
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(out.stdout.as_bytes());
    let _ = stdout.flush();
    if out.code != 0 {
        std::process::exit(out.code);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::derived::testkit::{repo_with, FILE, LINK};
    use serde_json::{json, Value};

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    /// A repository with one commit holding `files`; its working tree is empty, so every read
    /// that finds a file found it at `HEAD`.
    fn repo(files: &[(&str, &str)]) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|(p, t)| (*p, FILE, t.as_bytes()))
            .collect();
        repo_with(&root, &entries);
        Fixture { _dir: dir, root }
    }

    fn inventory_args(root: &Path, paths: &[&str], kind: &str, json: bool) -> TrackersArgs {
        TrackersArgs {
            action: TrackersAction::Inventory {
                paths: paths.iter().map(|p| p.to_string()).collect(),
                kind: kind.into(),
                level: None,
                status_column: None,
                project: Some(root.to_path_buf()),
                json,
            },
        }
    }

    fn inventory(f: &Fixture, paths: &[&str], kind: &str, json: bool) -> Outcome {
        evaluate(&inventory_args(&f.root, paths, kind, json), &f.root)
    }

    fn no_absolute_path(f: &Fixture, out: &Outcome) {
        assert!(
            !out.stdout.contains(f.root.to_str().unwrap()),
            "the output names no absolute path:\n{}",
            out.stdout
        );
    }

    const TRACKER: &str = "# Obs log\n\n## F-1 — first\n**Status:** open\n";

    /// The repository of the reader tests. `scripts/read.py` (code) and `config/x.yaml`
    /// (configuration) name the tracker path, so `CodeRefs` and `git grep` both find them;
    /// `scripts/x.json` names it too, but `CodeRefs` reads JSON as data, so only `git grep`
    /// finds it (line 2).
    ///
    /// The key space excludes `.codescout/memories`. Files that name the tracker but are no
    /// hit for either search: an audit record, a `.log` and a `.txt` data file, and a YAML
    /// file in the excluded folder. Markdown that mentions it: `docs/notes.md` (prose),
    /// `CLAUDE.md` at the root (no code names it) and `.codescout/memories/m.md` (excluded
    /// from the key space, under a dot directory).
    fn readers_repo() -> Fixture {
        repo(&[
            ("docs/trackers/obs-log.md", TRACKER),
            ("scripts/read.py", "open('docs/trackers/obs-log.md')\n"),
            ("config/x.yaml", "path: docs/trackers/obs-log.md\n"),
            (
                "scripts/x.json",
                "{\n  \"log\": \"docs/trackers/obs-log.md\"\n}\n",
            ),
            ("docs/notes.md", "See docs/trackers/obs-log.md.\n"),
            (
                "CLAUDE.md",
                "Read docs/trackers/obs-log.md before you start.\n",
            ),
            (
                ".codescout/key-space.toml",
                "exclude = [\".codescout/memories\"]\n",
            ),
            // Under a dot directory: `CodeRefs::frozen` freezes it. Outside the key space.
            (
                ".codescout/memories/m.md",
                "Read docs/trackers/obs-log.md.\n",
            ),
            (
                ".codescout/memories/index.yaml",
                "log: docs/trackers/obs-log.md\n",
            ),
            (
                ".codescout/audit/x.jsonl",
                "{\"path\": \"docs/trackers/obs-log.md\"}\n",
            ),
            ("docs/evals/data/y.log", "read docs/trackers/obs-log.md\n"),
            ("z.txt", "obs-log\n"),
            (
                "docs/augmentations/docs-trackers-obs-log.yaml",
                "prompt: Keep it short.\n",
            ),
        ])
    }

    /// The JSON of an inventory run of `docs/trackers/obs-log.md` in `f`, which must exit 0.
    fn inventory_json(f: &Fixture) -> Value {
        let out = inventory(f, &["docs/trackers/obs-log.md"], "file", true);
        assert_eq!(out.code, 0, "{}", out.stdout);
        serde_json::from_str(out.stdout.trim()).unwrap()
    }

    /// The path mention of `file` in `v`, which must be listed.
    fn path_hit<'a>(v: &'a Value, file: &str) -> &'a Value {
        v["mentions"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["file"] == file)
            .unwrap_or_else(|| panic!("no path mention of {file} in {}", v["mentions"]["paths"]))
    }

    /// Spec § Path mentions names `CLAUDE.md` as frozen, and `CodeRefs::frozen` freezes it
    /// only when code names it. Breaks if the CLI's frozen test drops the `CLAUDE.md` rule,
    /// or freezes every file (`docs/notes.md` must stay not frozen).
    #[test]
    fn claude_md_is_frozen_and_a_docs_note_is_not() {
        let v = inventory_json(&readers_repo());
        assert_eq!(path_hit(&v, "CLAUDE.md")["frozen"], true);
        assert_eq!(path_hit(&v, "docs/notes.md")["frozen"], false);
    }

    /// A markdown file the key space excludes can still name a source. Breaks if the CLI
    /// drops markdown outside the key space (the `OutsideKeySpace` arm returns nothing): the
    /// memory's path mention would vanish.
    #[test]
    fn markdown_outside_the_key_space_still_names_a_source() {
        let v = inventory_json(&readers_repo());
        assert_eq!(
            path_hit(&v, ".codescout/memories/m.md"),
            &json!({"file": ".codescout/memories/m.md", "line": 1, "in_code": false, "frozen": true})
        );
    }

    /// A symlinked or non-UTF-8 markdown file outside the key space is skipped, as the same
    /// file inside it is. Breaks if reading it is an error that stops the run (exit 2).
    #[test]
    fn an_unreadable_markdown_file_outside_the_key_space_does_not_stop_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        repo_with(
            &root,
            &[
                ("docs/trackers/obs-log.md", FILE, TRACKER.as_bytes()),
                (
                    ".codescout/key-space.toml",
                    FILE,
                    b"exclude = [\"notes\"]\n",
                ),
                ("notes/bad.md", FILE, b"docs/trackers/obs-log.md \xff\xfe\n"),
                ("notes/link.md", LINK, b"../docs/trackers/obs-log.md"),
                ("notes/ok.md", FILE, b"See docs/trackers/obs-log.md.\n"),
            ],
        );
        let f = Fixture { _dir: dir, root };
        let v = inventory_json(&f);
        let files: Vec<&str> = v["mentions"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["file"].as_str().unwrap())
            .collect();
        assert_eq!(files, ["notes/ok.md"]);
    }

    /// The grep skips the audit trail, data files other than JSON (`jsonl`, `txt`, `csv`,
    /// `log`) and paths outside the key space; JSON and YAML readers stay. Breaks if any of
    /// `.codescout/audit/x.jsonl`, `docs/evals/data/y.log`, `z.txt` or
    /// `.codescout/memories/index.yaml` is a hit, or if `scripts/x.json` is no longer a
    /// grep-only hit.
    #[test]
    fn the_grep_skips_audit_data_and_excluded_files_but_keeps_json() {
        let v = inventory_json(&readers_repo());
        assert_eq!(
            v["mentions"]["code"],
            json!([
                {"file": "config/x.yaml", "line": 1, "origin": "code_refs"},
                {"file": "scripts/read.py", "line": 1, "origin": "code_refs"},
                {"file": "scripts/x.json", "line": 2, "origin": "grep"}
            ])
        );
    }

    /// Two sources that each have a sidecar stop the run: the inventory holds one sidecar,
    /// and a second one must not be dropped without a word. One source with a sidecar beside
    /// one without is reported as before. Breaks if the run keeps the first sidecar and goes
    /// on, or if the reason does not name both sources.
    #[test]
    fn a_second_source_with_a_sidecar_exits_2_naming_both() {
        let f = repo(&[
            ("docs/trackers/a.md", TRACKER),
            ("docs/trackers/b.md", TRACKER),
            ("docs/trackers/c.md", TRACKER),
            ("docs/augmentations/docs-trackers-a.yaml", "prompt: A.\n"),
            ("docs/augmentations/docs-trackers-b.yaml", "prompt: B.\n"),
        ]);
        let out = inventory(
            &f,
            &["docs/trackers/a.md", "docs/trackers/b.md"],
            "file",
            false,
        );
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert_eq!(
            out.stdout,
            "The command did not run: sources docs/trackers/a.md and docs/trackers/b.md both \
         have a sidecar; fold sources that have a sidecar one at a time.\n"
        );

        let out = inventory(
            &f,
            &["docs/trackers/c.md", "docs/trackers/a.md"],
            "file",
            true,
        );
        assert_eq!(out.code, 0, "{}", out.stdout);
        let v: Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(
            v["sidecar"],
            json!({"path": "docs/augmentations/docs-trackers-a.yaml", "prompt": "A."})
        );
    }

    /// Breaks if: the JSON reader is dropped (the grep is not run, or it skips JSON); a hit
    /// both searches find is reported as `grep` or twice; the grep reads the working tree
    /// (`scripts/wt.json` exists only there); markdown is grepped (`docs/notes.md` must be a
    /// path mention, not a code hit); the sidecar or its prompt is not read at HEAD; `frozen`
    /// is not taken from `CodeRefs::frozen` (the memory under `.codescout/` must be frozen).
    #[test]
    fn inventory_json_lists_a_json_reader_that_code_refs_misses() {
        let f = readers_repo();
        std::fs::create_dir_all(f.root.join("scripts")).unwrap();
        std::fs::write(
            f.root.join("scripts/wt.json"),
            "\"docs/trackers/obs-log.md\"\n",
        )
        .unwrap();

        let out = inventory(&f, &["docs/trackers/obs-log.md"], "file", true);

        assert_eq!(out.code, 0, "{}", out.stdout);
        no_absolute_path(&f, &out);
        let v: Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(
            v["mentions"]["code"],
            json!([
                {"file": "config/x.yaml", "line": 1, "origin": "code_refs"},
                {"file": "scripts/read.py", "line": 1, "origin": "code_refs"},
                {"file": "scripts/x.json", "line": 2, "origin": "grep"}
            ])
        );
        assert_eq!(
            v["mentions"]["paths"],
            json!([
                {"file": ".codescout/memories/m.md", "line": 1, "in_code": false, "frozen": true},
                {"file": "CLAUDE.md", "line": 1, "in_code": false, "frozen": true},
                {"file": "docs/notes.md", "line": 1, "in_code": false, "frozen": false}
            ])
        );
        assert_eq!(
            v["sidecar"],
            json!({"path": "docs/augmentations/docs-trackers-obs-log.yaml", "prompt": "Keep it short."})
        );
        assert_eq!(v["sources"][0]["path"], "docs/trackers/obs-log.md");
        assert_eq!(v["sources"][0]["status_values"], json!({"open": 1}));
        assert_eq!(v["sources"][0]["records"][0]["token"], "F-1");
    }

    /// The text form is a short summary with counts and repo-relative paths. Breaks if the
    /// summary drops the source line, the record count, the grep-only count or the sidecar.
    #[test]
    fn inventory_text_is_a_short_summary() {
        let f = readers_repo();
        let out = inventory(&f, &["docs/trackers/obs-log.md"], "file", false);
        assert_eq!(out.code, 0, "{}", out.stdout);
        no_absolute_path(&f, &out);
        let text = &out.stdout;
        assert!(
            text.contains(
                "docs/trackers/obs-log.md: 4 lines, 1 record, 1 section, 0 index tables."
            ),
            "{text}"
        );
        assert!(text.contains("Status values: open (1)."), "{text}");
        assert!(
            text.contains("Non-markdown files that name a source: 3 hits in 3 files. git grep alone found 1 of them."),
            "{text}"
        );
        assert!(
            text.contains(
                "Sidecar: docs/augmentations/docs-trackers-obs-log.yaml (it has a prompt)."
            ),
            "{text}"
        );
    }

    /// Breaks if a missing source is skipped instead of stopping the run, or if the reason
    /// spans lines or names an absolute path.
    #[test]
    fn a_source_missing_at_head_exits_2_with_one_line() {
        let f = repo(&[("docs/trackers/obs-log.md", TRACKER)]);
        // In the working tree only: HEAD is what counts.
        std::fs::create_dir_all(f.root.join("docs/trackers")).unwrap();
        std::fs::write(f.root.join("docs/trackers/new.md"), TRACKER).unwrap();
        let out = inventory(&f, &["docs/trackers/new.md"], "file", false);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert_eq!(out.stdout.lines().count(), 1, "{}", out.stdout);
        assert!(
            out.stdout.contains("docs/trackers/new.md is not in HEAD"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out);
    }

    /// Breaks if a directory with no git repository at or above it is not a `2`.
    #[test]
    fn a_project_that_is_not_a_git_repository_exits_2() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let out = evaluate(&inventory_args(&root, &["x.md"], "file", true), &root);
        assert_eq!(out.code, 2, "{}", out.stdout);
        let v: Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(v["ran"], false);
        assert_eq!(v["reason"], "the project is not inside a git repository");
    }

    /// `--kind files` expands a folder at HEAD into one whole source per markdown file, in
    /// path order, and leaves other files out. Breaks if the folder is read as a file, a
    /// non-markdown file is a source, or a nested folder's file is taken.
    #[test]
    fn kind_files_expands_a_folder_into_one_whole_source_per_markdown_file() {
        let f = repo(&[
            ("docs/trackers/clusters/IC-2.md", "# IC-2 — two\ntext\n"),
            ("docs/trackers/clusters/IC-1.md", "# IC-1 — one\n"),
            ("docs/trackers/clusters/notes.txt", "x\n"),
            ("docs/trackers/clusters/sub/IC-3.md", "# IC-3 — three\n"),
        ]);
        let out = inventory(&f, &["docs/trackers/clusters"], "files", true);
        assert_eq!(out.code, 0, "{}", out.stdout);
        let v: Value = serde_json::from_str(out.stdout.trim()).unwrap();
        let paths: Vec<&str> = v["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "docs/trackers/clusters/IC-1.md",
                "docs/trackers/clusters/IC-2.md"
            ]
        );
        let records: Vec<usize> = v["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["records"].as_array().unwrap().len())
            .collect();
        assert_eq!(records, [1, 1]);
    }

    /// Breaks if `--kind sections` runs without a level instead of stopping with a reason.
    #[test]
    fn kind_sections_without_a_level_exits_2() {
        let f = repo(&[("docs/trackers/obs-log.md", TRACKER)]);
        let out = inventory(&f, &["docs/trackers/obs-log.md"], "sections", false);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("--kind sections needs --level"),
            "{}",
            out.stdout
        );
    }

    /// `apply` and `check` exist and say they are not built yet, with exit 2.
    #[test]
    fn apply_and_check_are_not_built_yet() {
        let f = repo(&[("docs/trackers/obs-log.md", TRACKER)]);
        for action in [
            TrackersAction::Apply {
                mapping: "m.toml".into(),
                project: None,
                json: false,
            },
            TrackersAction::Check {
                mapping: "m.toml".into(),
                project: None,
                json: false,
            },
        ] {
            let out = evaluate(&TrackersArgs { action }, &f.root);
            assert_eq!(out.code, 2, "{}", out.stdout);
            assert_eq!(out.stdout.lines().count(), 1, "{}", out.stdout);
            assert!(out.stdout.contains("not built yet"), "{}", out.stdout);
        }
    }
}
