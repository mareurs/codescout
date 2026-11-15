//! `codescout cutover` — move the catalog-only facts of the old catalog into committed files.
//!
//! This file holds `cutover plan` and `cutover export`. `plan` is the dry run: it resolves the old
//! catalog, refuses when a side file lies beside it, opens it immutable, loads the rows under the
//! repository, plans `HEAD`, checks that the catalog did not change while it was read, and prints
//! the report. It writes nothing: not to the old catalog, not to the repository, not to a report
//! file. `export` does the same, then (in D-R8 and D-R30's order) refuses when `derived.db`
//! exists or the project write lock is held, re-checks the old catalog, applies the plan to the
//! working tree (`librarian::cutover::apply`), and keeps the report beside the old catalog. See
//! `librarian::cutover` for the three subcommands and the rules every one of them follows.
//!
//! Exit codes: `0` the run did what it was asked, whatever it found; `1` `cutover export` refused
//! some files (or could not keep its report); `2` it did not run. Every reason is one line and
//! names no absolute path.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use clap::{Args, Subcommand};

use crate::cli::write_lock::{lock_root_for, take_at};
use crate::librarian::current_project::lookup_git_root;
use crate::librarian::cutover::apply::apply;
use crate::librarian::cutover::old_rows;
use crate::librarian::cutover::plan::{plan_export, ExportPlan};
use crate::librarian::cutover::report::{
    render_apply_markdown, render_json, render_markdown, ReportHeader,
};
use crate::librarian::derived::diff::{
    file_stamp, open_old_immutable, side_files_present, NoOldRowsUnderRoot, RootMap,
};
use crate::librarian::derived::old_catalog::resolve_old_catalog;
use crate::librarian::derived::scan::open_commit;
use crate::librarian::derived::schema::{derived_catalog_beside, DERIVED_CATALOG_FILE};
use crate::librarian::util::write_catalogued_file;

#[derive(Debug, Args)]
pub struct CutoverArgs {
    #[command(subcommand)]
    pub action: CutoverAction,
}

#[derive(Debug, Subcommand)]
pub enum CutoverAction {
    /// Dry run: print what the cutover would move and what it could not, and write nothing.
    Plan(CommonArgs),
    /// Write the plan into the working tree, one atomic write per file, and keep the report
    /// beside the old catalog. Never commits.
    Export(CommonArgs),
}

/// The options every `cutover` subcommand takes.
#[derive(Debug, Args)]
pub struct CommonArgs {
    /// Project root override (defaults to cwd). The git root at or above it is planned.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// The old catalog to read (default: `LIBRARIAN_DB`, else `catalog.db` in the `librarian`
    /// directory of the machine's local data directory).
    #[arg(long)]
    pub old_db: Option<PathBuf>,
    /// Another spelling of the git root that old rows carry, as an absolute path (repeatable).
    #[arg(long = "old-root")]
    pub old_root: Vec<PathBuf>,
    /// Print the report as one JSON object instead of markdown.
    #[arg(long)]
    pub json: bool,
}

/// What a run decided and what it prints. [`run`] prints it and exits with `code`.
#[derive(Debug)]
pub struct Outcome {
    /// `0` the run did what it was asked, `1` export refused some files, `2` it did not run.
    pub code: i32,
    pub stdout: String,
}

/// What a planning run produced.
pub struct Loaded {
    /// The directory the run started from, resolved (`--project`, else the working directory).
    pub start: PathBuf,
    pub git_root: PathBuf,
    pub old_db: PathBuf,
    /// Length and modification time of the old catalog when it was opened, for `export`'s last
    /// re-check before it writes.
    pub stamp: (u64, std::time::SystemTime),
    pub plan: ExportPlan,
}

/// Resolve, refuse on side files, open, load rows, plan `HEAD`, then re-check side files and the
/// stamp.
pub(crate) fn load_and_plan(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
) -> Result<Loaded> {
    load_and_plan_between(args, cwd, librarian_db, data_dir, &|_| {})
}

/// [`load_and_plan`] with a hook that runs between the plan and the re-check. A test uses it to
/// change the old catalog at the one moment the re-check exists for.
pub(crate) fn load_and_plan_between(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    between: &dyn Fn(&Path),
) -> Result<Loaded> {
    // The old catalog first: it is the cheap check, and a missing one needs no plan. No text
    // below names a path: the operator typed each one, and the report is kept and shared.
    let old_db =
        resolve_old_catalog(args.old_db.as_deref(), librarian_db, data_dir).ok_or_else(|| {
            anyhow!(
            "no --old-db was given, LIBRARIAN_DB is not set, and this machine has no local data \
             directory to look for librarian/catalog.db in"
        )
        })?;
    if args.old_root.iter().any(|p| !p.is_absolute()) {
        bail!("an --old-root must be an absolute path");
    }
    let absolute = std::path::absolute(&old_db)
        .map_err(|_| anyhow!("the path of the old catalog cannot be resolved"))?;
    // SQLite keeps its side files beside the target of a symlink: look and stamp there.
    let old_db = std::fs::canonicalize(&absolute).unwrap_or(absolute);
    let old = open_old_immutable(&old_db)?;
    let stamp = file_stamp(&old_db).map_err(|_| anyhow!("the old catalog cannot be inspected"))?;

    let start = args.project.clone().unwrap_or_else(|| cwd.to_path_buf());
    let start =
        std::fs::canonicalize(start).map_err(|_| anyhow!("the project path cannot be resolved"))?;
    let git_root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("the project is not inside a git repository"))?;
    let roots = RootMap::with_old_roots(git_root.clone(), args.old_root.clone())?;
    let repo = git2::Repository::open(&git_root)
        .map_err(|_| anyhow!("the git repository cannot be opened"))?;
    let commit = open_commit(&repo, "HEAD")?;

    let rows = old_rows::load(&old, &roots)?;
    // Rows exist and none lies under the root: the root is probably spelled another way on the
    // old side, and a plan would move nothing from no evidence.
    if rows.documents.is_empty() && rows.out_of_scope.other_roots > 0 {
        return Err(NoOldRowsUnderRoot {
            old_roots: args.old_root.len(),
            other_roots: rows.out_of_scope.other_roots,
        }
        .into());
    }
    let plan = plan_export(&repo, &commit, &rows)?;

    // An immutable open cannot see a writer that started after the side-file check. A side file,
    // or a file that grew, shrank or was touched since the open, means the rows are out of date.
    between(&old_db);
    if side_files_present(&old_db) || file_stamp(&old_db).ok() != Some(stamp) {
        bail!(
            "the old catalog changed while it was read; stop every codescout process and run \
             this again"
        );
    }
    Ok(Loaded {
        start,
        git_root,
        old_db,
        stamp,
        plan,
    })
}

/// Run `cutover plan`. It returns no error: whatever stops the plan becomes an outcome with
/// code 2.
pub fn evaluate_plan(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
) -> Outcome {
    plan_outcome(args, load_and_plan(args, cwd, librarian_db, data_dir))
}

/// [`evaluate_plan`] with the hook of [`load_and_plan_between`].
#[cfg(test)]
pub(crate) fn evaluate_plan_between(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    between: &dyn Fn(&Path),
) -> Outcome {
    plan_outcome(
        args,
        load_and_plan_between(args, cwd, librarian_db, data_dir, between),
    )
}

/// What `cutover plan` prints for the result of planning.
fn plan_outcome(args: &CommonArgs, planned: Result<Loaded>) -> Outcome {
    match planned {
        Ok(loaded) => {
            let header = ReportHeader {
                phase: "plan",
                old_roots_given: args.old_root.len(),
            };
            let stdout = if args.json {
                format!("{}\n", render_json(&loaded.plan, &header))
            } else {
                render_markdown(&loaded.plan, &header)
            };
            Outcome { code: 0, stdout }
        }
        Err(e) => did_not_run(args.json, &one_line(&e)),
    }
}

/// The outcome of a run that stopped before it had a plan.
fn did_not_run(json: bool, reason: &str) -> Outcome {
    let stdout = if json {
        format!(
            "{}\n",
            serde_json::json!({ "ran": false, "reason": reason })
        )
    } else {
        format!("The cutover did not run: {reason}\n")
    };
    Outcome { code: 2, stdout }
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

/// The last line of an `export` report: what the operator does next.
const NEXT_STEP: &str = "Commit the changed files, then run: codescout cutover build";

/// How long `export` tries for the project write lock before it refuses. D-R30 names timeout 0,
/// but `write_guard::acquire` treats a zero budget as already spent and refuses even a free lock
/// (measured: "timed out before checking cross-process write lock"), so this is the smallest
/// budget that tries the lock: one attempt, one 50 ms retry slot, no real wait.
const LOCK_ATTEMPT: Duration = Duration::from_millis(250);

/// Run `cutover export`: plan `HEAD`, refuse when the new catalog exists or the project write
/// lock is held, re-check the old catalog, write the plan into the working tree, and keep the
/// report beside the old catalog. It returns no error: whatever stops the run becomes an outcome
/// with code 2, and a run that refused some files has code 1.
///
/// The order is D-R8 and D-R30. The side-file refusal is the primary guard (an idle server holds
/// no lock); the lock keeps another CLI writer out, with timeout 0 so a held lock is a refusal and
/// not a wait.
pub async fn evaluate_export(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
) -> Outcome {
    export_with(args, cwd, librarian_db, data_dir, &|_| {}).await
}

/// [`evaluate_export`] with a hook that runs after the lock is taken and before the last
/// re-check of the old catalog. A test uses it to change the catalog at the one moment the
/// re-check exists for.
async fn export_with(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    before_writes: &dyn Fn(&Path),
) -> Outcome {
    match export_run(args, cwd, librarian_db, data_dir, before_writes).await {
        Ok(outcome) => outcome,
        Err(e) => did_not_run(args.json, &one_line(&e)),
    }
}

/// The steps of `export`; an `Err` is a refusal before the first write.
async fn export_run(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    before_writes: &dyn Fn(&Path),
) -> Result<Outcome> {
    let loaded = load_and_plan(args, cwd, librarian_db, data_dir)?;
    if std::fs::symlink_metadata(derived_catalog_beside(&loaded.old_db)).is_ok() {
        bail!(
            "the new catalog ({DERIVED_CATALOG_FILE}) already exists beside the old one, and \
             export runs before it is built"
        );
    }
    let _lock = take_at(
        &lock_root_for(&loaded.start, &loaded.git_root),
        LOCK_ATTEMPT,
        "cutover export",
    )
    .await
    .map_err(|e| anyhow!("the project write lock is not free: {}", one_line(&e)))?;
    before_writes(&loaded.old_db);
    if side_files_present(&loaded.old_db) || file_stamp(&loaded.old_db).ok() != Some(loaded.stamp) {
        bail!(
            "the old catalog changed while it was read; stop every codescout process and run \
             this again"
        );
    }

    let applied = apply(&loaded.plan, &loaded.git_root);
    let header = ReportHeader {
        phase: "export",
        old_roots_given: args.old_root.len(),
    };
    let dir = loaded
        .old_db
        .parent()
        .ok_or_else(|| anyhow!("the old catalog has no directory to keep the report in"))?;
    let (md_path, json_path) = report_paths(dir, &loaded.plan.repo_name, &loaded.plan.commit);
    let name_of = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let (md_name, json_name) = (name_of(&md_path), name_of(&json_path));

    let mut markdown = render_markdown(&loaded.plan, &header);
    markdown.push('\n');
    markdown.push_str(&render_apply_markdown(&applied));
    markdown.push_str(&format!(
        "\nReport files, kept beside the old catalog: {md_name} and {json_name}\n{NEXT_STEP}\n"
    ));
    let mut json = render_json(&loaded.plan, &header);
    json["apply"] = serde_json::to_value(&applied)?;
    json["report"] = serde_json::json!({ "markdown": md_name, "json": json_name });
    json["next"] = serde_json::Value::from(NEXT_STEP);

    // Both files are written even when files were refused: the report is the record of the run.
    let mut report_failures = Vec::new();
    let json_text = format!("{}\n", serde_json::to_string_pretty(&json)?);
    for (path, name, text) in [
        (&md_path, &md_name, &markdown),
        (&json_path, &json_name, &json_text),
    ] {
        if let Err(e) = write_catalogued_file(&[], path, text) {
            let why = match e.downcast_ref::<std::io::Error>() {
                Some(io) => io.kind().to_string(),
                None => "the write was refused".to_owned(),
            };
            report_failures.push(format!("{name} could not be written: {why}"));
        }
    }

    let code = i32::from(!applied.refused.is_empty() || !report_failures.is_empty());
    let stdout = if args.json {
        if !report_failures.is_empty() {
            json["report_errors"] = serde_json::json!(report_failures);
        }
        format!("{json}\n")
    } else if report_failures.is_empty() {
        markdown
    } else {
        let mut text = markdown;
        let last = text.len() - NEXT_STEP.len() - 1;
        let notes: String = report_failures.iter().map(|f| format!("{f}\n")).collect();
        text.insert_str(last, &notes);
        text
    };
    Ok(Outcome { code, stdout })
}

/// The first of `cutover-report-<name>-<commit12>.md`, `…-2.md`, `…-3.md`, … in `dir` that does
/// not exist, with its `.json` twin: a report is never overwritten (D-R17). Only the name and the
/// first 12 characters of `commit` are used, and every character of the repository name that is
/// not a letter, a digit, `-`, `_` or `.` becomes `_`, so the result is always a file in `dir`.
pub(crate) fn report_paths(dir: &Path, repo_name: &str, commit: &str) -> (PathBuf, PathBuf) {
    let name: String = repo_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = if name.is_empty() {
        "repo".to_owned()
    } else {
        name
    };
    let commit12: String = commit.chars().take(12).collect();
    let stem = format!("cutover-report-{name}-{commit12}");
    let mut n = 1u32;
    loop {
        let numbered = if n == 1 {
            stem.clone()
        } else {
            format!("{stem}-{n}")
        };
        let md = dir.join(format!("{numbered}.md"));
        let json = dir.join(format!("{numbered}.json"));
        if std::fs::symlink_metadata(&md).is_err() && std::fs::symlink_metadata(&json).is_err() {
            return (md, json);
        }
        n += 1;
    }
}

pub async fn run(args: CutoverArgs) -> Result<()> {
    use std::io::Write;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let out = match &args.action {
        CutoverAction::Plan(common) => evaluate_plan(
            common,
            &cwd,
            std::env::var_os("LIBRARIAN_DB"),
            dirs::data_local_dir(),
        ),
        CutoverAction::Export(common) => {
            evaluate_export(
                common,
                &cwd,
                std::env::var_os("LIBRARIAN_DB"),
                dirs::data_local_dir(),
            )
            .await
        }
    };
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
    use crate::librarian::cutover::testkit::*;
    use crate::librarian::derived::testkit::{repo_with, FILE};
    use std::collections::BTreeMap;

    struct Fixture {
        _repo_dir: tempfile::TempDir,
        _catalog_dir: tempfile::TempDir,
        root: PathBuf,
        catalog_dir: PathBuf,
        db: PathBuf,
    }

    /// A repository holding `a.md`, `sub/b.md`, `c.md` (which cites the id of `d.md`, so the plan
    /// rewrites it) and `d.md`, with its working tree equal to `HEAD`, and an old catalog file
    /// with rows for them in another directory.
    fn fixture() -> Fixture {
        let repo_dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(repo_dir.path()).unwrap();
        let repo = repo_with(
            &root,
            &[
                ("a.md", FILE, b"# A\n" as &[u8]),
                ("sub/b.md", FILE, b"# B\n"),
                // `c.md` cites the id of `d.md`: the plan rewrites it, so `export` has a file to write.
                ("c.md", FILE, b"# C\n\nsee 0123456789abcdef.\n"),
                ("d.md", FILE, b"# D\n"),
            ],
        );
        // The working tree equals HEAD, the state `export` is run from.
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        repo.checkout_head(Some(&mut checkout)).unwrap();
        let catalog_dir = tempfile::tempdir().unwrap();
        let catalog_path = std::fs::canonicalize(catalog_dir.path()).unwrap();
        let db = catalog_path.join("catalog.db");
        old_catalog_file(&db, |cat| {
            add_doc(cat, &root, "a", "a.md");
            add_doc(cat, &root, "b", "sub/b.md");
            add_doc(cat, &root, "0123456789abcdef", "d.md");
            add_event(cat, "e1", "a", "field_patch", "{}");
            add_event(cat, "e2", "b", "note", "{}");
            add_augmentation(cat, "a", r#"{"k":1}"#, None);
        });
        Fixture {
            _repo_dir: repo_dir,
            _catalog_dir: catalog_dir,
            root,
            catalog_dir: catalog_path,
            db,
        }
    }

    fn args(f: &Fixture) -> CommonArgs {
        CommonArgs {
            project: Some(f.root.clone()),
            old_db: Some(f.db.clone()),
            old_root: Vec::new(),
            json: false,
        }
    }

    /// Every file under `dir` (relative path to bytes), so a test can tell that nothing changed.
    fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(base, &path, out);
                } else {
                    let rel = path
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    out.insert(rel, std::fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(dir, dir, &mut out);
        out
    }

    #[test]
    fn plan_writes_nothing_and_prints_the_report() {
        let f = fixture();
        let repo_before = snapshot(&f.root);
        let catalog_before = snapshot(&f.catalog_dir);
        assert!(!catalog_before.is_empty());

        let out = evaluate_plan(&args(&f), &f.root, None, None);

        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout.starts_with("# Cutover report — repo at "),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("\nPhase: plan\n"));
        assert!(
            out.stdout.contains("| event:field_patch | 1 |"),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("- git history of the file holds it: 1"));
        assert!(
            !out.stdout.contains(f.root.to_str().unwrap())
                && !out.stdout.contains(f.catalog_dir.to_str().unwrap()),
            "the report names no absolute path:\n{}",
            out.stdout
        );
        assert_eq!(
            snapshot(&f.root),
            repo_before,
            "the repository is unchanged"
        );
        assert_eq!(
            snapshot(&f.catalog_dir),
            catalog_before,
            "nothing is written beside the old catalog, and not the catalog"
        );
    }

    #[test]
    fn plan_prints_json_with_the_same_counts() {
        let f = fixture();
        let mut a = args(&f);
        a.json = true;
        let out = evaluate_plan(&a, &f.root, None, None);
        assert_eq!(out.code, 0, "{}", out.stdout);
        let json: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(json["phase"], "plan");
        assert_eq!(json["population"]["event:field_patch"], 1);
        assert_eq!(json["population"]["augmentation_shape"], 1);
    }
    #[test]
    fn plan_refuses_when_no_old_row_lies_under_the_root() {
        let f = fixture();
        let other = tempfile::tempdir().unwrap();
        let other_db = std::fs::canonicalize(other.path())
            .unwrap()
            .join("catalog.db");
        old_catalog_file(&other_db, |cat| {
            add_doc_at(cat, "x", "/elsewhere/proj/a.md");
            add_doc_at(cat, "y", "/elsewhere/proj/b.md");
        });
        let mut a = args(&f);
        a.old_db = Some(other_db);
        let out = evaluate_plan(&a, &f.root, None, None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("no old row lies under the git root (2 rows lie under other roots"),
            "{}",
            out.stdout
        );
        assert!(
            !out.stdout.contains(f.root.to_str().unwrap()),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn plan_refuses_when_a_side_file_lies_beside_the_old_catalog() {
        let f = fixture();
        std::fs::write(f.catalog_dir.join("catalog.db-wal"), b"x").unwrap();
        let before = snapshot(&f.catalog_dir);
        let out = evaluate_plan(&args(&f), &f.root, None, None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("lies beside the old catalog"),
            "{}",
            out.stdout
        );
        assert!(
            !out.stdout.contains("# Cutover report"),
            "no report is printed"
        );
        assert!(
            !out.stdout.contains(f.catalog_dir.to_str().unwrap()),
            "the refusal names no absolute path:\n{}",
            out.stdout
        );
        assert_eq!(snapshot(&f.catalog_dir), before);
    }

    #[test]
    fn plan_refuses_when_the_catalog_changed_during_the_load() {
        let f = fixture();
        let touch = |db: &Path| {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().append(true).open(db).unwrap();
            file.write_all(b"x").unwrap();
        };
        let out = evaluate_plan_between(&args(&f), &f.root, None, None, &touch);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("the old catalog changed while it was read"),
            "{}",
            out.stdout
        );
        assert!(
            !out.stdout.contains("# Cutover report"),
            "no report is printed"
        );
    }

    #[test]
    fn plan_refuses_when_a_side_file_appears_during_the_load() {
        let f = fixture();
        let create = |db: &Path| {
            let mut side = db.as_os_str().to_owned();
            side.push("-wal");
            std::fs::write(PathBuf::from(side), b"x").unwrap();
        };
        let out = evaluate_plan_between(&args(&f), &f.root, None, None, &create);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("the old catalog changed while it was read"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn plan_refuses_a_relative_old_root_and_a_missing_catalog_without_a_path() {
        let f = fixture();
        let mut a = args(&f);
        a.old_root = vec![PathBuf::from("relative/root")];
        let out = evaluate_plan(&a, &f.root, None, None);
        assert_eq!(out.code, 2);
        assert!(out.stdout.contains("absolute"), "{}", out.stdout);

        let mut b = args(&f);
        b.old_db = None;
        let out = evaluate_plan(&b, &f.root, None, None);
        assert_eq!(out.code, 2);
        assert!(out.stdout.contains("--old-db"), "{}", out.stdout);

        let mut c = args(&f);
        c.old_db = Some(f.catalog_dir.join("none.db"));
        let out = evaluate_plan(&c, &f.root, None, None);
        assert_eq!(out.code, 2);
        assert!(out.stdout.contains("does not exist"), "{}", out.stdout);
        assert!(
            !out.stdout.contains(f.catalog_dir.to_str().unwrap()),
            "{}",
            out.stdout
        );
        assert!(!f.catalog_dir.join("none.db").exists(), "never created");
    }

    // ---- cutover export ----

    /// The report files beside the old catalog: `(markdown names, json names)`, sorted.
    fn report_files(f: &Fixture) -> (Vec<String>, Vec<String>) {
        let mut md = Vec::new();
        let mut json = Vec::new();
        for entry in std::fs::read_dir(&f.catalog_dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            if name.starts_with("cutover-report-") && name.ends_with(".md") {
                md.push(name);
            } else if name.starts_with("cutover-report-") && name.ends_with(".json") {
                json.push(name);
            }
        }
        md.sort();
        json.sort();
        (md, json)
    }

    fn read(f: &Fixture, rel: &str) -> String {
        std::fs::read_to_string(f.root.join(rel)).unwrap()
    }

    fn no_absolute_path(f: &Fixture, text: &str) {
        assert!(
            !text.contains(f.root.to_str().unwrap())
                && !text.contains(f.catalog_dir.to_str().unwrap()),
            "an absolute path in the output:\n{text}"
        );
    }

    // mutation: remove the `derived_catalog_beside(..).exists()` refusal.
    #[tokio::test]
    async fn export_refuses_when_the_new_catalog_exists() {
        let f = fixture();
        std::fs::write(f.catalog_dir.join("derived.db"), b"x").unwrap();
        let repo_before = snapshot(&f.root);
        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(out.stdout.contains("derived.db"), "{}", out.stdout);
        assert!(
            out.stdout.contains("already exists"),
            "the refusal says why: {}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!(snapshot(&f.root), repo_before, "no file is written");
        assert_eq!(report_files(&f), (vec![], vec![]), "no report is kept");
    }

    // mutation: skip `take_at`, or give it a budget of seconds (the run would then wait).
    #[tokio::test]
    async fn export_refuses_while_another_process_holds_the_write_lock() {
        let f = fixture();
        let held = take_at(
            &lock_root_for(&f.root, &f.root),
            std::time::Duration::from_secs(1),
            "test holder",
        )
        .await
        .unwrap();
        let (a, b) = (read(&f, "a.md"), read(&f, "c.md"));
        // The bound is far above the plan's own cost under load and far below the 30 s the
        // project's configured lock timeout would give: only a run that waits for the lock misses it.
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            evaluate_export(&args(&f), &f.root, None, None),
        )
        .await
        .expect("export refuses a held lock; it does not wait for it");
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("write lock"),
            "the refusal names the lock: {}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!((read(&f, "a.md"), read(&f, "c.md")), (a, b));
        assert_eq!(report_files(&f), (vec![], vec![]));
        drop(held);

        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "the lock is free again: {}", out.stdout);
    }

    // mutation: drop the `side_files_present` half of the re-check; drop the `file_stamp` half.
    #[tokio::test]
    async fn export_refuses_when_a_side_file_appears_before_the_writes() {
        let f = fixture();
        let (a, b) = (read(&f, "a.md"), read(&f, "c.md"));

        let create = |db: &Path| {
            let mut side = db.as_os_str().to_owned();
            side.push("-wal");
            std::fs::write(PathBuf::from(side), b"x").unwrap();
        };
        let out = export_with(&args(&f), &f.root, None, None, &create).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("the old catalog changed while it was read"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!((read(&f, "a.md"), read(&f, "c.md")), (a.clone(), b.clone()));
        assert_eq!(report_files(&f), (vec![], vec![]));
        std::fs::remove_file(f.catalog_dir.join("catalog.db-wal")).unwrap();

        // The catalog file itself changes (length and mtime), with no side file.
        let touch = |db: &Path| {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().append(true).open(db).unwrap();
            file.write_all(b"x").unwrap();
        };
        let out = export_with(&args(&f), &f.root, None, None, &touch).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("the old catalog changed while it was read"),
            "{}",
            out.stdout
        );
        assert_eq!((read(&f, "a.md"), read(&f, "c.md")), (a, b));
        assert_eq!(report_files(&f), (vec![], vec![]));
    }

    // mutation: write the report to the repository root; print an absolute path; skip the .json.
    #[tokio::test]
    async fn export_writes_the_report_beside_the_old_catalog_and_prints_no_absolute_path() {
        let f = fixture();
        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout.starts_with("# Cutover report — "),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("\nPhase: export\n"), "{}", out.stdout);
        assert!(out.stdout.contains("## Export result"), "{}", out.stdout);
        assert!(out.stdout.contains("\n- c.md written\n"), "{}", out.stdout);
        assert_eq!(
            out.stdout.lines().last(),
            Some(NEXT_STEP),
            "the last line is the next step: {}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);

        let (md, json) = report_files(&f);
        assert_eq!((md.len(), json.len()), (1, 1), "{md:?} {json:?}");
        assert!(md[0].ends_with(".md") && json[0].ends_with(".json"));
        assert_eq!(
            md[0].trim_end_matches(".md"),
            json[0].trim_end_matches(".json"),
            "one name, two extensions"
        );
        assert!(
            out.stdout.contains(&md[0]) && out.stdout.contains(&json[0]),
            "the output names the report files: {}",
            out.stdout
        );
        let kept = std::fs::read_to_string(f.catalog_dir.join(&md[0])).unwrap();
        assert_eq!(kept, out.stdout, "the markdown file is the printed report");
        no_absolute_path(&f, &kept);
        let kept_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(f.catalog_dir.join(&json[0])).unwrap())
                .unwrap();
        assert_eq!(kept_json["phase"], "export");
        assert!(
            snapshot(&f.root)
                .keys()
                .all(|k| !k.starts_with("cutover-report-")),
            "no report lies in the repository"
        );
        assert_eq!(
            read(&f, "c.md"),
            "# C\n\nsee [[d]].\n",
            "the planned file was written"
        );
    }

    // mutation: always use the first free name `…-<commit12>.md`; overwrite; skip the .json twin.
    #[tokio::test]
    async fn a_second_export_keeps_the_first_report() {
        let f = fixture();
        let first = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(first.code, 0, "{}", first.stdout);
        let (md1, json1) = report_files(&f);
        assert_eq!((md1.len(), json1.len()), (1, 1));
        let first_md = std::fs::read(f.catalog_dir.join(&md1[0])).unwrap();
        let first_json = std::fs::read(f.catalog_dir.join(&json1[0])).unwrap();

        // The first run left the files written and uncommitted: the second finds them done.
        let second = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(second.code, 0, "{}", second.stdout);
        let (md2, json2) = report_files(&f);
        assert_eq!((md2.len(), json2.len()), (2, 2), "{md2:?} {json2:?}");
        let stem = md1[0].trim_end_matches(".md");
        assert!(
            md2.contains(&format!("{stem}-2.md")) && json2.contains(&format!("{stem}-2.json")),
            "{md2:?} {json2:?}"
        );
        assert_eq!(
            std::fs::read(f.catalog_dir.join(&md1[0])).unwrap(),
            first_md
        );
        assert_eq!(
            std::fs::read(f.catalog_dir.join(&json1[0])).unwrap(),
            first_json
        );
        assert_ne!(
            first.stdout, second.stdout,
            "the second report says the files were already written"
        );

        let third = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(third.code, 0);
        let (md3, _) = report_files(&f);
        assert!(md3.contains(&format!("{stem}-3.md")), "{md3:?}");
    }

    // mutation: leave the `apply` result out of the JSON; print a trailing line after the object.
    #[tokio::test]
    async fn the_json_report_parses_and_carries_the_tally() {
        let f = fixture();
        let planned = evaluate_plan(
            &CommonArgs {
                json: true,
                ..args(&f)
            },
            &f.root,
            None,
            None,
        );
        let planned: serde_json::Value = serde_json::from_str(planned.stdout.trim()).unwrap();

        let mut a = args(&f);
        a.json = true;
        let out = evaluate_export(&a, &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        let json: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(json["phase"], "export");
        assert_eq!(json["tally"], planned["tally"], "the tally of the plan");
        assert_eq!(json["population"], planned["population"]);
        let written: Vec<&str> = json["apply"]["written"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(written.contains(&"c.md"), "{written:?}");
        assert_eq!(json["apply"]["refused"].as_array().unwrap().len(), 0);
        assert_eq!(json["next"], NEXT_STEP);
        no_absolute_path(&f, &out.stdout);

        let (md, json_files) = report_files(&f);
        assert_eq!((md.len(), json_files.len()), (1, 1));
        let kept: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(f.catalog_dir.join(&json_files[0])).unwrap(),
        )
        .unwrap();
        assert_eq!(kept, json, "the json file is the printed report");
    }

    // mutation: return 0 when `apply` refused a file; leave the report unwritten on a refusal.
    #[tokio::test]
    async fn a_run_with_refused_files_exits_1() {
        let f = fixture();
        std::fs::write(f.root.join("c.md"), "# C\nedited by hand\n").unwrap();
        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 1, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                "- c.md refused — changed since HEAD: commit or revert it, then run again"
            ),
            "{}",
            out.stdout
        );
        assert_eq!(read(&f, "c.md"), "# C\nedited by hand\n", "left as it was");
        no_absolute_path(&f, &out.stdout);
        let (md, json) = report_files(&f);
        assert_eq!(
            (md.len(), json.len()),
            (1, 1),
            "the report is kept on a refusal"
        );

        let mut a = args(&f);
        a.json = true;
        let out = evaluate_export(&a, &f.root, None, None).await;
        assert_eq!(out.code, 1, "{}", out.stdout);
        let json: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(json["apply"]["refused"][0][0], "c.md");
    }

    // mutation: skip the `.json` twin in the existence test; skip the `-<n>` loop; leave the name
    // unsanitised.
    #[test]
    fn report_paths_picks_the_first_free_name_and_keeps_the_name_a_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let commit = "0123456789abcdef0123456789abcdef01234567";
        let (md, json) = report_paths(dir.path(), "proj", commit);
        assert_eq!(md, dir.path().join("cutover-report-proj-0123456789ab.md"));
        assert_eq!(
            json,
            dir.path().join("cutover-report-proj-0123456789ab.json")
        );

        std::fs::write(&md, b"x").unwrap();
        let (md, json) = report_paths(dir.path(), "proj", commit);
        assert_eq!(md, dir.path().join("cutover-report-proj-0123456789ab-2.md"));
        assert_eq!(
            json,
            dir.path().join("cutover-report-proj-0123456789ab-2.json")
        );

        // A lone `.json` twin takes the name too.
        std::fs::write(&json, b"x").unwrap();
        let (md, _) = report_paths(dir.path(), "proj", commit);
        assert_eq!(md, dir.path().join("cutover-report-proj-0123456789ab-3.md"));

        let (md, json) = report_paths(dir.path(), "a/b c..", commit);
        assert_eq!(md.parent(), Some(dir.path()), "no path separator survives");
        assert_eq!(json.parent(), Some(dir.path()));
        assert!(
            md.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("cutover-report-a_b_c"),
            "{md:?}"
        );
    }
}
