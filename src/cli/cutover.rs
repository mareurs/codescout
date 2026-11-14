//! `codescout cutover` — move the catalog-only facts of the old catalog into committed files.
//!
//! This file holds `cutover plan`, the dry run: it resolves the old catalog, refuses when a side
//! file lies beside it, opens it immutable, loads the rows under the repository, plans `HEAD`,
//! checks that the catalog did not change while it was read, and prints the report. It writes
//! nothing: not to the old catalog, not to the repository, not to a report file. See
//! `librarian::cutover` for the three subcommands and the rules every one of them follows.
//!
//! Exit codes: `0` the plan ran, whatever it found; `2` it did not run. Every reason is one line
//! and names no absolute path. (`1` is for `cutover export` when it refuses some files.)

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use clap::{Args, Subcommand};

use crate::librarian::current_project::lookup_git_root;
use crate::librarian::cutover::old_rows;
use crate::librarian::cutover::plan::{plan_export, ExportPlan};
use crate::librarian::cutover::report::{render_json, render_markdown, ReportHeader};
use crate::librarian::derived::diff::{
    file_stamp, open_old_immutable, side_files_present, NoOldRowsUnderRoot, RootMap,
};
use crate::librarian::derived::old_catalog::resolve_old_catalog;
use crate::librarian::derived::scan::open_commit;

#[derive(Debug, Args)]
pub struct CutoverArgs {
    #[command(subcommand)]
    pub action: CutoverAction,
}

#[derive(Debug, Subcommand)]
pub enum CutoverAction {
    /// Dry run: print what the cutover would move and what it could not, and write nothing.
    Plan(CommonArgs),
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
    pub git_root: PathBuf,
    pub old_db: PathBuf,
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
        git_root,
        old_db,
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

    /// A repository holding `a.md` and `sub/b.md`, and an old catalog file with rows for both,
    /// in another directory.
    fn fixture() -> Fixture {
        let repo_dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(repo_dir.path()).unwrap();
        let _repo = repo_with(
            &root,
            &[
                ("a.md", FILE, b"# A\n" as &[u8]),
                ("sub/b.md", FILE, b"# B\n"),
            ],
        );
        let catalog_dir = tempfile::tempdir().unwrap();
        let catalog_path = std::fs::canonicalize(catalog_dir.path()).unwrap();
        let db = catalog_path.join("catalog.db");
        old_catalog_file(&db, |cat| {
            add_doc(cat, &root, "a", "a.md");
            add_doc(cat, &root, "b", "sub/b.md");
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
}
