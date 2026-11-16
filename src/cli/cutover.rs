//! `codescout cutover` — move the catalog-only facts of the old catalog into committed files.
//!
//! This file holds `cutover plan`, `cutover export` and `cutover build`. `plan` is the dry run:
//! it resolves the old catalog, refuses when a side file lies beside it, opens it immutable,
//! loads the rows under the repository, plans `HEAD`, checks that the catalog did not change while
//! it was read, and prints the report. It writes nothing: not to the old catalog, not to the
//! repository, not to a report file. `export` does the same, then (in D-R8 and D-R30's order)
//! refuses when the new catalog (`.codescout/derived.db` in the checkout) exists or the project
//! write lock is held, re-checks the old catalog, applies the plan to the working tree
//! (`librarian::cutover::apply`), and keeps the report beside the old catalog. `build` writes the
//! new catalog from `HEAD`, in the git root of the checkout it runs in; it refuses a link, a
//! tracked file, a hard link (on Unix), or a repository that does not ignore the file (E1-R1 to
//! E1-R4, E1-R12). See `librarian::cutover` for the three subcommands and the rules every one of
//! them follows.
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
use crate::librarian::derived::build::{build_full, BuildInput};
use crate::librarian::derived::diff::{
    file_stamp, open_old_immutable, side_files_present, NoOldRowsUnderRoot, RootMap,
};
use crate::librarian::derived::old_catalog::resolve_old_catalog;
use crate::librarian::derived::scan::open_commit;
use crate::librarian::derived::schema::{self, derived_catalog_in, DERIVED_CATALOG_REL};
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
    /// Build the new catalog (`.codescout/derived.db` in this checkout, git-ignored) from `HEAD`.
    /// Refuses while a plan of `HEAD` would still change a file: run `export` and commit first.
    /// A second run rebuilds the same file.
    Build(CommonArgs),
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
/// budget that tries the lock. `acquire` retries every 50 ms until the budget is spent (six
/// tries), so a held lock is a refusal after a quarter of a second, not a wait.
const LOCK_ATTEMPT: Duration = Duration::from_millis(250);

/// Why `take_at` did not give the lock, on one line that names no path. Two failures differ: the
/// lock file cannot be opened (an `io::Error`, whose text names the path: only its kind is kept),
/// or another process holds the lock (a `RecoverableError` with the holder record and a hint,
/// which name no path).
fn lock_refusal(e: &anyhow::Error) -> anyhow::Error {
    match e.downcast_ref::<std::io::Error>() {
        Some(io) => anyhow!(
            "the project write lock file cannot be opened: {}",
            io.kind()
        ),
        None => anyhow!("the project write lock is not free: {}", one_line(e)),
    }
}

/// Run `cutover export`: plan `HEAD`, refuse when the new catalog exists or the project write
/// lock is held, re-check the old catalog, write the plan into the working tree, and keep the
/// report beside the old catalog. It returns no error: whatever stops the run becomes an outcome
/// with code 2, and a run that refused some files, or could not keep a report file, has code 1.
///
/// The order is D-R8 and D-R30. The side-file refusal is the primary guard (an idle server holds
/// no lock); the lock keeps another CLI writer out, with a budget of [`LOCK_ATTEMPT`] so a held
/// lock is a refusal and not a wait.
pub async fn evaluate_export(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
) -> Outcome {
    export_with(args, cwd, librarian_db, data_dir, &|_| {}, &|_| {}).await
}

/// [`evaluate_export`] with two hooks. `before_writes` runs after the lock is taken and before the
/// last re-check of the old catalog: a test uses it to change the catalog at the one moment the
/// re-check exists for. `at_end` runs after the report files are written, while the lock is still
/// held, with the directory the lock is taken at: a test uses it to show the lock is held to
/// the end of the run.
async fn export_with(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    before_writes: &dyn Fn(&Path),
    at_end: &dyn Fn(&Path),
) -> Outcome {
    match export_run(args, cwd, librarian_db, data_dir, before_writes, at_end).await {
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
    at_end: &dyn Fn(&Path),
) -> Result<Outcome> {
    let loaded = load_and_plan(args, cwd, librarian_db, data_dir)?;
    if std::fs::symlink_metadata(derived_catalog_in(&loaded.git_root)).is_ok() {
        bail!(
            "the new catalog ({DERIVED_CATALOG_REL}) already exists in this checkout, and export \
             runs before it is built. Delete it (git can rebuild it), then run export again"
        );
    }
    let lock_root = lock_root_for(&loaded.start, &loaded.git_root);
    let _lock = take_at(&lock_root, LOCK_ATTEMPT, "cutover export")
        .await
        .map_err(|e| lock_refusal(&e))?;
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
    at_end(&lock_root);
    Ok(Outcome { code, stdout })
}

/// Run `cutover build`: plan `HEAD`, refuse while the plan still writes a file, then build
/// `.codescout/derived.db` from `HEAD` in the git root of the checkout. It returns no error:
/// whatever stops the run becomes an outcome with code 2. A second run rebuilds the same file.
///
/// The build reads git objects alone, so it does not take the project write lock: it writes no
/// file of the repository that git tracks. The old catalog is read for the plan and never written.
pub async fn evaluate_build(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
) -> Outcome {
    build_with(args, cwd, librarian_db, data_dir, &|_| {})
}

/// [`evaluate_build`] with a hook that runs after the plan and before the build reads `HEAD`
/// again, with the git root: a test uses it to move `HEAD` at the one moment the re-check exists
/// for.
fn build_with(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    after_plan: &dyn Fn(&Path),
) -> Outcome {
    match build_run(args, cwd, librarian_db, data_dir, after_plan) {
        Ok(outcome) => outcome,
        Err(e) => did_not_run(args.json, &one_line(&e)),
    }
}

/// The line that makes git ignore the new catalog and the three side files SQLite keeps beside it.
const IGNORE_LINE: &str = "/.codescout/derived.db*";

/// The new catalog and its three side files, relative to the git root. SQLite opens a side file by
/// name, so a link or a tracked file at any of them is as unsafe as one at the catalog.
const CATALOG_NAMES: [&str; 4] = [
    ".codescout/derived.db",
    ".codescout/derived.db-wal",
    ".codescout/derived.db-shm",
    ".codescout/derived.db-journal",
];

/// Why the new catalog could not be opened, on one line that names no absolute path: the text of
/// `schema::open` names the file as the caller spelled it.
fn open_refusal(e: &rusqlite::Error) -> anyhow::Error {
    use rusqlite::ErrorCode;
    match e.sqlite_error_code() {
        Some(ErrorCode::NotADatabase) => anyhow!(
            "{DERIVED_CATALOG_REL} is not a new catalog: it is not a database, or it holds \
             another database's tables. Move it away, then run this again"
        ),
        Some(ErrorCode::SchemaChanged) => {
            anyhow!("{DERIVED_CATALOG_REL} was made by a newer codescout than this one")
        }
        Some(ErrorCode::CannotOpen) => anyhow!(
            "{DERIVED_CATALOG_REL} cannot be created or opened: check that this checkout's \
             .codescout directory is writable"
        ),
        Some(code) => anyhow!("{DERIVED_CATALOG_REL} cannot be opened: {code:?}"),
        None => anyhow!("{DERIVED_CATALOG_REL} cannot be opened"),
    }
}

/// True when `canonical_dir`, the resolved `.codescout` of a checkout, is the one directory of that
/// name in the git root. A belt behind the link check: it needs `git_root` to be canonical.
fn catalog_dir_is_the_checkouts(git_root: &Path, canonical_dir: &Path) -> bool {
    canonical_dir == git_root.join(".codescout")
}

/// Refuse before anything is opened when a write of the new catalog could land outside the
/// checkout or on a committed file (E1-R12): a link at `.codescout`, a `.codescout` that is not a
/// directory or resolves to another directory, a link, a directory, a hard link (on Unix) or an
/// uninspectable file at the catalog or one of its side files, or one of the four names in the git
/// index (any stage). The ignore rules cannot see any of these: they match patterns only. A hard
/// link is a name for a file that another path also names, so a write through it changes that
/// other file (E1-R12 says "link"; a hard link is one). On a platform without a link count, a hard
/// link is not refused (see [`has_other_hard_links`]).
fn refuse_unsafe_catalog_path(repo: &git2::Repository, git_root: &Path) -> Result<()> {
    let dir = git_root.join(".codescout");
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_symlink() => bail!(
            ".codescout in this checkout is a link, and build writes only inside the checkout. \
             Replace the link with a directory, then run this again"
        ),
        Ok(meta) if !meta.is_dir() => bail!(
            ".codescout in this checkout is not a directory. Replace it with a directory, then \
             run this again"
        ),
        Ok(_) => {
            let canonical = std::fs::canonicalize(&dir)
                .map_err(|_| anyhow!(".codescout in this checkout cannot be resolved"))?;
            if !catalog_dir_is_the_checkouts(git_root, &canonical) {
                bail!(
                    ".codescout in this checkout resolves to another directory; build writes \
                     only inside the checkout"
                );
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => bail!(
            ".codescout in this checkout cannot be inspected: {}",
            e.kind()
        ),
    }
    for name in CATALOG_NAMES {
        match std::fs::symlink_metadata(git_root.join(name)) {
            Ok(meta) if meta.file_type().is_symlink() => bail!(
                "{name} is a link, and build does not write through a link. Remove it, then run \
                 this again"
            ),
            Ok(meta) if meta.is_dir() => bail!(
                "{name} is a directory, and build writes a file at that name. Remove the \
                 directory, then run this again"
            ),
            Ok(meta) if has_other_hard_links(&meta) => bail!(
                "{name} has more than one hard link, and build would write into the same file \
                 under its other name. Remove it, then run this again"
            ),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => bail!("{name} cannot be inspected: {}", e.kind()),
        }
    }
    let index = repo
        .index()
        .map_err(|_| anyhow!("git cannot read the index of this checkout"))?;
    for name in CATALOG_NAMES {
        // Stage 0 is a tracked file; stages 1 to 3 are a conflicted merge entry.
        if (0..=3).any(|stage| index.get_path(Path::new(name), stage).is_some()) {
            bail!(
                "{name} is tracked by git, and build would overwrite a committed file. Run git \
                 rm --cached {name}, commit, then run this again"
            );
        }
    }
    Ok(())
}

/// True when the file has another name. On a platform without a link count in its metadata the
/// answer is always `false`: a hard link at the catalog is not refused there (known gap).
fn has_other_hard_links(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() > 1
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// Refuse unless git ignores the new catalog and its three side files, from any source
/// (`.gitignore`, the repository's `info/exclude`, `core.excludesFile`). `build` writes them in the
/// working tree, and a file that git does not ignore can be committed by the next `git add -A`.
fn refuse_unless_ignored(repo: &git2::Repository) -> Result<()> {
    for name in CATALOG_NAMES {
        let ignored = repo.is_path_ignored(name).map_err(|_| {
            anyhow!("git cannot tell whether {DERIVED_CATALOG_REL} is ignored in this checkout")
        })?;
        if !ignored {
            bail!(
                "git does not ignore {DERIVED_CATALOG_REL} and its side files in this checkout, \
                 and build writes them there. Add the line {IGNORE_LINE} to .gitignore or to the \
                 repository's info/exclude (git rev-parse --git-path info/exclude prints it), then \
                 run this again"
            );
        }
    }
    Ok(())
}

/// The steps of `build`; an `Err` is a refusal, or a build that failed. A refusal leaves the new
/// catalog as it was. A build that fails on a file that did not exist leaves the schema with no
/// rows (the build is one transaction, so a failure never leaves a part of the rows); `export`
/// refuses a catalog file that exists, and the reason says to delete it.
///
/// The catalog is the git root's own file: no other checkout reads it, so a rebuild destroys
/// nothing another checkout uses (E1-R3). Every check that can refuse runs before the file is
/// opened (E1-R12).
fn build_run(
    args: &CommonArgs,
    cwd: &Path,
    librarian_db: Option<OsString>,
    data_dir: Option<PathBuf>,
    after_plan: &dyn Fn(&Path),
) -> Result<Outcome> {
    let loaded = load_and_plan(args, cwd, librarian_db, data_dir)?;
    let writes = loaded.plan.writes();
    if writes > 0 {
        bail!(
            "the export is not committed: {writes} files would change. Run codescout cutover \
             export, commit, then run this again"
        );
    }
    after_plan(&loaded.git_root);
    let repo = git2::Repository::open(&loaded.git_root)
        .map_err(|_| anyhow!("the git repository cannot be opened"))?;
    let commit = open_commit(&repo, "HEAD")?;
    // The plan above judged one commit; the build must read that one.
    if commit.id().to_string() != loaded.plan.commit {
        bail!("HEAD changed while the plan was made; run this again");
    }
    refuse_unsafe_catalog_path(&repo, &loaded.git_root)?;
    refuse_unless_ignored(&repo)?;
    let input = BuildInput::from_commit(&repo, &commit, &loaded.git_root)?;
    let new_catalog = derived_catalog_in(&loaded.git_root);
    let mut conn = schema::open(&new_catalog).map_err(|e| open_refusal(&e))?;
    let built = build_full(&mut conn, &input)
        .map_err(|e| anyhow!("building the new catalog failed: {}", one_line(&e)))?;
    drop(conn);

    let commit12: String = loaded.plan.commit.chars().take(12).collect();
    let sentence = format!(
        "Built from {commit12}; the new catalog is {DERIVED_CATALOG_REL} in this checkout."
    );
    let stdout = if args.json {
        format!(
            "{}\n",
            serde_json::json!({
                "ran": true,
                "commit": loaded.plan.commit,
                "catalog": DERIVED_CATALOG_REL,
                "built": {
                    "documents": built.documents,
                    "entries": built.entries,
                    "edges": built.edges,
                    "chunks": built.chunks,
                    "skipped": built.skipped,
                    "problems": built.problems,
                },
            })
        )
    } else {
        format!(
            "# Cutover build — {} at {commit12}\n\n- documents: {}\n- entries: {}\n- edges: {}\n\
             - chunks: {}\n- skipped: {}\n- problems: {}\n\n{sentence}\n",
            loaded.plan.repo_name,
            built.documents,
            built.entries,
            built.edges,
            built.chunks,
            built.skipped,
            built.problems
        )
    };
    Ok(Outcome { code: 0, stdout })
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
        CutoverAction::Build(common) => {
            evaluate_build(
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
    /// rewrites it), `d.md` and a `.gitignore` that ignores the new catalog and its side files,
    /// with its working tree equal to `HEAD`, and an old catalog file with rows for them in another
    /// directory.
    fn fixture() -> Fixture {
        let repo_dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(repo_dir.path()).unwrap();
        let entries: Vec<(&str, u32, &[u8])> = vec![
            ("a.md", FILE, b"# A\n"),
            ("sub/b.md", FILE, b"# B\n"),
            // `c.md` cites the id of `d.md`: the plan rewrites it, so `export` has a file to write.
            ("c.md", FILE, b"# C\n\nsee 0123456789abcdef.\n"),
            ("d.md", FILE, b"# D\n"),
            (".gitignore", FILE, b"/.codescout/derived.db*\n"),
        ];
        let repo = repo_with(&root, &entries);
        no_global_excludes(&repo);
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

    /// Point the repository's local `core.excludesFile` at an empty file inside its git directory.
    /// libgit2 reads `~/.config/git/ignore` when the setting is unset, so a test that asks git
    /// whether a path is ignored would otherwise depend on the machine it runs on.
    fn no_global_excludes(repo: &git2::Repository) {
        let empty = repo.path().join("empty-excludes");
        std::fs::write(&empty, b"").unwrap();
        repo.config()
            .unwrap()
            .set_str("core.excludesfile", empty.to_str().unwrap())
            .unwrap();
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

    // mutation: remove the refusal; check the old catalog's directory instead of the checkout.
    #[tokio::test]
    async fn export_refuses_when_the_new_catalog_exists() {
        let f = fixture();
        std::fs::create_dir_all(f.root.join(".codescout")).unwrap();
        std::fs::write(f.root.join(DERIVED_CATALOG_REL), b"x").unwrap();
        // A `derived.db` beside the old catalog is not the new catalog: it stops nothing.
        std::fs::write(f.catalog_dir.join("derived.db"), b"y").unwrap();
        let repo_before = snapshot(&f.root);
        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                "the new catalog (.codescout/derived.db) already exists in this checkout, and \
                 export runs before it is built. Delete it (git can rebuild it), then run export \
                 again"
            ),
            "the refusal says why and what to do: {}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!(snapshot(&f.root), repo_before, "no file is written");
        assert_eq!(report_files(&f), (vec![], vec![]), "no report is kept");

        // The old directory alone does not stop export.
        std::fs::remove_file(f.root.join(DERIVED_CATALOG_REL)).unwrap();
        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
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
            out.stdout.contains("the project write lock is not free"),
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

    // mutation: forward the error chain of `take_at` for an open failure (it names the path), or
    // call every lock failure "not free".
    #[cfg(unix)]
    #[tokio::test]
    async fn export_refuses_a_lock_file_it_cannot_open_and_names_no_path() {
        // A link at `.codescout/write.lock` (committed content can plant one), then a directory.
        for plant in ["link", "directory"] {
            let f = fixture();
            let dir = f.root.join(".codescout");
            std::fs::create_dir_all(&dir).unwrap();
            match plant {
                "link" => {
                    std::os::unix::fs::symlink("elsewhere.txt", dir.join("write.lock")).unwrap()
                }
                _ => std::fs::create_dir(dir.join("write.lock")).unwrap(),
            }
            let (a, c) = (read(&f, "a.md"), read(&f, "c.md"));
            let out = evaluate_export(&args(&f), &f.root, None, None).await;
            assert_eq!(out.code, 2, "{plant}: {}", out.stdout);
            assert!(
                out.stdout
                    .contains("the project write lock file cannot be opened"),
                "{plant}: {}",
                out.stdout
            );
            assert!(
                !out.stdout.contains("not free"),
                "{plant}: a lock nobody holds is not called held: {}",
                out.stdout
            );
            no_absolute_path(&f, &out.stdout);
            assert!(
                !out.stdout.contains("write.lock"),
                "{plant}: {}",
                out.stdout
            );
            assert_eq!((read(&f, "a.md"), read(&f, "c.md")), (a, c), "{plant}");
            assert_eq!(report_files(&f), (vec![], vec![]), "{plant}");
        }
    }

    // mutation: `let _lock = take_at(..)` becomes `let _ = take_at(..)` (the guard drops at once).
    #[tokio::test]
    async fn export_holds_the_lock_to_the_end_of_the_run() {
        use fs4::fs_std::FileExt;
        let f = fixture();
        let probe = std::cell::Cell::new(None);
        let at_end = |root: &Path| {
            let file = crate::agent::open_lock_file(root).unwrap();
            // A second open file description conflicts with the run's own, so a free lock
            // would let this succeed.
            probe.set(Some(file.try_lock_exclusive().is_err()));
        };
        let out = export_with(&args(&f), &f.root, None, None, &|_| {}, &at_end).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert_eq!(
            probe.get(),
            Some(true),
            "the hook ran after the writes and found the lock held"
        );
        // And the run released it: a new holder gets it now.
        let again = take_at(
            &lock_root_for(&f.root, &f.root),
            std::time::Duration::from_secs(1),
            "test holder",
        )
        .await;
        assert!(again.is_ok(), "the lock is free after the run");
    }

    // mutation: drop the failure notes; return code 0 when only the report failed; put the notes
    // after the next-step line.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_report_that_cannot_be_written_is_said_and_exits_1() {
        use std::os::unix::fs::PermissionsExt;
        // Root ignores directory modes, so the write would succeed.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        struct Restore(PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        for json in [false, true] {
            let f = fixture();
            let _restore = Restore(f.catalog_dir.clone());
            std::fs::set_permissions(&f.catalog_dir, std::fs::Permissions::from_mode(0o555))
                .unwrap();
            let mut a = args(&f);
            a.json = json;
            let out = evaluate_export(&a, &f.root, None, None).await;
            assert_eq!(out.code, 1, "json={json}: {}", out.stdout);
            assert_eq!(
                read(&f, "c.md"),
                "# C\n\nsee [[d]].\n",
                "the files were written; only the report was not kept"
            );
            no_absolute_path(&f, &out.stdout);
            if json {
                let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
                let errors = v["report_errors"].as_array().unwrap();
                assert_eq!(errors.len(), 2, "{errors:?}");
                assert!(errors[0].as_str().unwrap().contains("could not be written"));
            } else {
                let lines: Vec<&str> = out.stdout.lines().collect();
                assert_eq!(lines.last(), Some(&NEXT_STEP), "{}", out.stdout);
                let notes: Vec<usize> = lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.contains("could not be written"))
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(notes.len(), 2, "one note per report file:\n{}", out.stdout);
                assert!(
                    notes.iter().all(|i| *i < lines.len() - 1),
                    "the notes come before the last line"
                );
            }
        }
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
        let out = export_with(&args(&f), &f.root, None, None, &create, &|_| {}).await;
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
        let out = export_with(&args(&f), &f.root, None, None, &touch, &|_| {}).await;
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
    // ---- cutover build ----

    /// Commit every file of the working tree on top of `HEAD`, as the operator does after `export`.
    fn commit_all(f: &Fixture, message: &str) -> git2::Oid {
        let repo = git2::Repository::open(&f.root).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let sig = crate::librarian::derived::testkit::fixed_sig();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
            .unwrap()
    }

    /// The state `build` runs from: the export written and committed.
    async fn exported_and_committed(f: &Fixture) -> git2::Oid {
        let out = evaluate_export(&args(f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        commit_all(f, "export")
    }

    fn derived(f: &Fixture) -> rusqlite::Connection {
        rusqlite::Connection::open(f.root.join(DERIVED_CATALOG_REL)).unwrap()
    }

    fn count(conn: &rusqlite::Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    // mutation: remove the `writes() > 0` refusal.
    #[tokio::test]
    async fn build_refuses_while_the_export_is_not_committed() {
        let f = fixture();
        let planned = load_and_plan(&args(&f), &f.root, None, None).unwrap();
        let n = planned.plan.writes();
        assert!(n > 0, "the fixture needs a file the export changes");
        let want = format!(
            "the export is not committed: {n} files would change. Run codescout cutover export, \
             commit, then run this again"
        );
        let catalog_before = snapshot(&f.catalog_dir);
        let repo_before = snapshot(&f.root);

        // Before `export`, and after it with nothing committed: a plan of `HEAD` still writes.
        for step in ["before the export", "after the export, uncommitted"] {
            let out = evaluate_build(&args(&f), &f.root, None, None).await;
            assert_eq!(out.code, 2, "{step}: {}", out.stdout);
            assert!(out.stdout.contains(&want), "{step}: {}", out.stdout);
            no_absolute_path(&f, &out.stdout);
            assert!(
                !f.root.join(DERIVED_CATALOG_REL).exists()
                    && !f.catalog_dir.join("derived.db").exists(),
                "{step}: no new catalog is created"
            );
            if step == "before the export" {
                assert_eq!(snapshot(&f.catalog_dir), catalog_before);
                assert_eq!(snapshot(&f.root), repo_before);
                let exported = evaluate_export(&args(&f), &f.root, None, None).await;
                assert_eq!(exported.code, 0, "{}", exported.stdout);
            }
        }

        let mut json = args(&f);
        json.json = true;
        let out = evaluate_build(&json, &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(v["ran"], false);
        assert_eq!(v["reason"], want);
    }

    // mutation: build from `HEAD~1`; build from the working tree's files (a fresh `git add`);
    // build beside the old catalog.
    #[tokio::test]
    async fn build_writes_the_new_catalog_from_head() {
        use sha2::{Digest, Sha256};
        let f = fixture();
        let head = exported_and_committed(&f).await;
        let catalog_before = std::fs::read(f.db.clone()).unwrap();
        // The working tree differs from `HEAD`: an edit and a new file. Neither is in the build.
        std::fs::write(f.root.join("a.md"), "# A edited after the commit\n").unwrap();
        std::fs::write(f.root.join("z.md"), "# Z\n").unwrap();

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("the new catalog is .codescout/derived.db in this checkout"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);

        assert!(f.root.join(DERIVED_CATALOG_REL).is_file());
        assert!(
            !f.catalog_dir.join("derived.db").exists(),
            "nothing is built beside the old catalog"
        );
        assert_eq!(
            std::fs::read(&f.db).unwrap(),
            catalog_before,
            "the old catalog is not written"
        );
        let conn = derived(&f);
        let commit_sha: String = conn
            .query_row("SELECT commit_sha FROM checkout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(commit_sha, head.to_string(), "built from HEAD");
        let paths: Vec<String> = conn
            .prepare("SELECT rel_path FROM document ORDER BY rel_path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            paths,
            ["a.md", "c.md", "d.md", "sub/b.md"],
            "the files of HEAD, and not z.md"
        );
        let a_sha: String = conn
            .query_row(
                "SELECT sha256 FROM document WHERE rel_path='a.md'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            a_sha,
            hex::encode(Sha256::digest(b"# A\n")),
            "a.md as HEAD has it, not as the working tree has it"
        );
        let exported_dst: Option<String> = conn
            .query_row(
                "SELECT dst_key FROM edge WHERE src_rel_path='c.md' AND kind='file'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            exported_dst.as_deref(),
            Some("d"),
            "the citation the export rewrote is read from the committed text"
        );

        // The counts printed are the rows stored.
        for (label, table) in [
            ("documents", "document"),
            ("entries", "entry"),
            ("edges", "edge"),
            ("chunks", "chunk"),
            ("skipped", "skipped"),
            ("problems", "problem"),
        ] {
            assert!(
                out.stdout
                    .contains(&format!("- {label}: {}\n", count(&conn, table))),
                "{label}: {}",
                out.stdout
            );
        }

        // `--json` carries the same counts. The reader above is closed first, so the JSON run
        // reads a closed file.
        drop(conn);
        let mut json = args(&f);
        json.json = true;
        let out = evaluate_build(&json, &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).unwrap();
        assert_eq!(v["ran"], true);
        assert_eq!(v["built"]["documents"], 4);
        assert_eq!(v["catalog"], ".codescout/derived.db");
        assert!(v.get("location").is_none(), "{v}");
        no_absolute_path(&f, &out.stdout);
    }

    // mutation: skip the side-file refusal of the old catalog's open.
    #[tokio::test]
    async fn build_refuses_when_a_side_file_lies_beside_the_old_catalog() {
        let f = fixture();
        exported_and_committed(&f).await;
        std::fs::write(f.catalog_dir.join("catalog.db-wal"), b"x").unwrap();
        let before = snapshot(&f.catalog_dir);
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("lies beside the old catalog"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert!(!f.root.join(DERIVED_CATALOG_REL).exists());
        assert_eq!(snapshot(&f.catalog_dir), before);
    }

    // mutation: refuse when `derived.db` exists; leave the old rows in place (no `DELETE`).
    #[tokio::test]
    async fn a_second_build_rebuilds_in_place() {
        let f = fixture();
        exported_and_committed(&f).await;
        let first = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(first.code, 0, "{}", first.stdout);
        {
            // A row no build writes: a rebuild in place removes it.
            let conn = derived(&f);
            conn.execute(
                "INSERT INTO problem (kind, \"key\", rel_paths) VALUES ('stale', 'x', '[]')",
                [],
            )
            .unwrap();
            assert_eq!(count(&conn, "problem"), 1);
        }
        // The second build writes the same file: a file held open since before it keeps its
        // inode (a delete-and-recreate would leave the held handle on an unlinked file, and the
        // open handle also stops the file system from giving the old inode to the new file).
        #[cfg(unix)]
        let held = std::fs::File::open(f.root.join(DERIVED_CATALOG_REL)).unwrap();
        let second = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(second.code, 0, "{}", second.stdout);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                held.metadata().unwrap().ino(),
                std::fs::metadata(f.root.join(DERIVED_CATALOG_REL))
                    .unwrap()
                    .ino(),
                "the second build rebuilds the file in place"
            );
        }
        assert_eq!(
            second.stdout, first.stdout,
            "the same commit, the same counts"
        );
        {
            let conn = derived(&f);
            assert_eq!(count(&conn, "problem"), 0, "the stale row is gone");
            assert_eq!(count(&conn, "document"), 4);
            assert_eq!(count(&conn, "checkout"), 1, "one checkout row, not two");
        }
        let names: Vec<String> = std::fs::read_dir(f.root.join(".codescout"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("derived.db"))
            .collect();
        assert_eq!(names, ["derived.db"], "no side file is left");
    }

    // mutation: refuse a new catalog that has no `checkout` row.
    #[tokio::test]
    async fn build_over_a_new_catalog_with_no_build_in_it_builds() {
        let f = fixture();
        exported_and_committed(&f).await;
        // The schema and no rows: what a failed first build leaves.
        drop(schema::open(&f.root.join(DERIVED_CATALOG_REL)).unwrap());
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert_eq!(count(&derived(&f), "document"), 4);
    }

    // mutation: remove the comparison of `HEAD` with the commit the plan judged.
    #[tokio::test]
    async fn build_refuses_when_head_moves_after_the_plan() {
        let f = fixture();
        exported_and_committed(&f).await;
        let move_head = |root: &Path| {
            let repo = git2::Repository::open(root).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            let sig = crate::librarian::derived::testkit::fixed_sig();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "moved",
                &head.tree().unwrap(),
                &[&head],
            )
            .unwrap();
        };
        let out = build_with(&args(&f), &f.root, None, None, &move_head);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("HEAD changed while the plan was made"),
            "{}",
            out.stdout
        );
        assert!(!f.root.join(DERIVED_CATALOG_REL).exists());
    }

    // mutation: forward the error chain of `schema::open` (it names the path).
    #[tokio::test]
    async fn build_refuses_a_file_that_is_not_a_new_catalog_and_names_no_path() {
        let f = fixture();
        exported_and_committed(&f).await;
        let foreign = f.root.join(DERIVED_CATALOG_REL);
        std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        {
            let conn = rusqlite::Connection::open(&foreign).unwrap();
            conn.execute_batch("CREATE TABLE somebody_elses (x TEXT);")
                .unwrap();
        }
        let before = std::fs::read(&foreign).unwrap();
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains(".codescout/derived.db is not a new catalog"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!(std::fs::read(&foreign).unwrap(), before, "left as it was");
    }

    // mutation: refuse with `SQLITE_SCHEMA` (the text says "newer").
    #[tokio::test]
    async fn build_refuses_an_older_catalog_it_cannot_rebuild_and_never_says_newer() {
        let f = fixture();
        exported_and_committed(&f).await;
        let older = f.root.join(DERIVED_CATALOG_REL);
        std::fs::create_dir_all(older.parent().unwrap()).unwrap();
        {
            let conn = rusqlite::Connection::open(&older).unwrap();
            conn.execute_batch(
                "CREATE TABLE checkout (x); CREATE TABLE somebody_elses (x TEXT); \
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let before = std::fs::read(&older).unwrap();
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains(".codescout/derived.db is not a new catalog"),
            "{}",
            out.stdout
        );
        assert!(!out.stdout.contains("newer"), "{}", out.stdout);
        no_absolute_path(&f, &out.stdout);
        assert_eq!(std::fs::read(&older).unwrap(), before, "left as it was");
    }

    /// A directory that the test owns and `build` has no business writing in.
    fn outside_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = std::fs::canonicalize(dir.path()).unwrap();
        (dir, path)
    }

    /// Every `derived.db*` file under `root`, found without following a link.
    #[cfg(unix)]
    fn catalog_files(root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let kind = std::fs::symlink_metadata(&path).unwrap().file_type();
                if kind.is_dir() && path.file_name().is_some_and(|n| n != ".git") {
                    pending.push(path);
                } else if path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("derived.db"))
                {
                    found.push(path);
                }
            }
        }
        found
    }

    // mutation: skip `refuse_unless_ignored`; check `derived.db` only, not its side files.
    #[tokio::test]
    async fn build_refuses_when_git_does_not_ignore_the_new_catalog() {
        let f = fixture();
        exported_and_committed(&f).await;
        // `fixture` pointed `core.excludesFile` at an empty file: only the rules below count.
        std::fs::remove_file(f.root.join(".gitignore")).unwrap();
        let new_catalog = f.root.join(DERIVED_CATALOG_REL);
        let refused = |out: &Outcome, step: &str| {
            assert_eq!(out.code, 2, "{step}: {}", out.stdout);
            assert!(
                out.stdout.contains("/.codescout/derived.db*")
                    && out.stdout.contains("info/exclude"),
                "{step}: the refusal names the line and where to put it: {}",
                out.stdout
            );
            no_absolute_path(&f, &out.stdout);
            assert!(!new_catalog.exists(), "{step}: nothing is created");
        };

        // (a) No rule at all.
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        refused(&out, "no rule");

        // (b) A rule for the file alone: its `-wal`, `-shm` and `-journal` are not ignored.
        let exclude = f.root.join(".git/info/exclude");
        std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        std::fs::write(&exclude, "/.codescout/derived.db\n").unwrap();
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        refused(&out, "a rule for the file alone");

        // (c) The rule the refusal names, in the repository's own `info/exclude`.
        std::fs::write(&exclude, "/.codescout/derived.db*\n").unwrap();
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(new_catalog.is_file());
    }

    // mutation: write to the main checkout's root for a worktree.
    #[tokio::test]
    async fn every_checkout_builds_its_own_catalog_and_leaves_the_others_unchanged() {
        use sha2::{Digest, Sha256};
        let f = fixture();
        exported_and_committed(&f).await;
        let first = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(first.code, 0, "{}", first.stdout);
        let main_file = f.root.join(DERIVED_CATALOG_REL);
        let main_sha = Sha256::digest(std::fs::read(&main_file).unwrap());

        // A linked worktree and a clone, both beside the main checkout, never inside it.
        let (_wt_dir, wt_base) = outside_dir();
        let wt_root = wt_base.join("wt");
        let main = git2::Repository::open(&f.root).unwrap();
        main.worktree("wt", &wt_root, None).unwrap();
        let (_clone_dir, clone_base) = outside_dir();
        let clone_root = clone_base.join("clone");
        let cloned = git2::Repository::clone(f.root.to_str().unwrap(), &clone_root).unwrap();
        no_global_excludes(&cloned);

        for (name, root) in [("worktree", &wt_root), ("clone", &clone_root)] {
            // The old rows lie under the main checkout; this checkout lies elsewhere.
            let own = CommonArgs {
                project: Some(root.clone()),
                old_db: Some(f.db.clone()),
                old_root: vec![f.root.clone()],
                json: false,
            };
            let out = evaluate_build(&own, root, None, None).await;
            assert_eq!(out.code, 0, "{name}: {}", out.stdout);
            let file = root.join(DERIVED_CATALOG_REL);
            assert!(file.is_file(), "{name}: the checkout has its own catalog");
            let recorded: String = rusqlite::Connection::open(&file)
                .unwrap()
                .query_row("SELECT root FROM checkout", [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                recorded,
                std::fs::canonicalize(root).unwrap().to_str().unwrap(),
                "{name}: the catalog records its own root"
            );
            assert_eq!(
                Sha256::digest(std::fs::read(&main_file).unwrap()),
                main_sha,
                "{name}: the main checkout's catalog is byte for byte as it was"
            );
        }
        let main_root: String = derived(&f)
            .query_row("SELECT root FROM checkout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(main_root, f.root.to_str().unwrap());
    }

    // mutation: drop the `CannotOpen` arm of `open_refusal`.
    #[cfg(unix)]
    #[tokio::test]
    async fn build_in_a_checkout_it_cannot_write_refuses_and_names_no_path() {
        use std::os::unix::fs::PermissionsExt;
        struct Restore(PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        let f = fixture();
        exported_and_committed(&f).await;
        // Export's lock made the directory; this checkout can no longer write in it.
        let dir = f.root.join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        let _restore = Restore(dir.clone());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(dir.join("probe"), b"x").is_ok() {
            return; // running as root: a mode does not stop the write
        }

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                ".codescout/derived.db cannot be created or opened: check that this checkout's \
                 .codescout directory is writable"
            ),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
    }

    // mutation: drop the directory link check.
    #[cfg(unix)]
    #[tokio::test]
    async fn build_refuses_a_linked_codescout_directory_and_writes_nothing_outside() {
        let f = fixture();
        exported_and_committed(&f).await;
        let (_outside, outside) = outside_dir();
        let dir = f.root.join(".codescout");
        std::fs::remove_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(&outside, &dir).unwrap();

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                ".codescout in this checkout is a link, and build writes only inside the \
                 checkout. Replace the link with a directory, then run this again"
            ),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert!(
            !out.stdout.contains(outside.to_str().unwrap()),
            "{}",
            out.stdout
        );
        assert_eq!(
            std::fs::read_dir(&outside).unwrap().count(),
            0,
            "nothing is written through the link"
        );
        assert_eq!(catalog_files(&f.root), Vec::<PathBuf>::new());
    }

    // mutation: check only the file and not its side files (the second name reds).
    #[cfg(unix)]
    #[tokio::test]
    async fn build_refuses_a_linked_catalog_file_and_writes_nothing_outside() {
        let f = fixture();
        exported_and_committed(&f).await;
        let (_outside, outside) = outside_dir();
        let dir = f.root.join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["derived.db", "derived.db-wal"] {
            let target = outside.join(format!("{name}.target"));
            let link = dir.join(name);
            std::os::unix::fs::symlink(&target, &link).unwrap();

            let out = evaluate_build(&args(&f), &f.root, None, None).await;
            assert_eq!(out.code, 2, "{name}: {}", out.stdout);
            assert!(
                out.stdout.contains(&format!(
                    ".codescout/{name} is a link, and build does not write through a link. \
                     Remove it, then run this again"
                )),
                "{name}: {}",
                out.stdout
            );
            no_absolute_path(&f, &out.stdout);
            assert!(
                !out.stdout.contains(outside.to_str().unwrap()),
                "{}",
                out.stdout
            );
            assert!(
                !target.exists(),
                "{name}: nothing is created through the link"
            );
            assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0, "{name}");
            std::fs::remove_file(&link).unwrap();
        }
        assert_eq!(catalog_files(&f.root), Vec::<PathBuf>::new());
    }

    // mutation: drop the index check (the ignore check alone passes: libgit2 matches patterns
    // only, so it calls a tracked file ignored); limit the index check to the first name.
    #[tokio::test]
    async fn build_refuses_a_tracked_catalog_file_and_leaves_it_unchanged() {
        let f = fixture();
        exported_and_committed(&f).await;
        let repo = git2::Repository::open(&f.root).unwrap();
        for name in CATALOG_NAMES {
            let file = f.root.join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            let bytes = b"a committed file that is not a database";
            std::fs::write(&file, bytes).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new(name)).unwrap();
            index.write().unwrap();

            let out = evaluate_build(&args(&f), &f.root, None, None).await;
            assert_eq!(out.code, 2, "{name}: {}", out.stdout);
            assert!(
                out.stdout.contains(&format!(
                    "{name} is tracked by git, and build would overwrite a committed file. Run \
                     git rm --cached {name}, commit, then run this again"
                )),
                "{name}: {}",
                out.stdout
            );
            no_absolute_path(&f, &out.stdout);
            assert_eq!(
                std::fs::read(&file).unwrap(),
                bytes,
                "{name}: left as it was"
            );

            // The next name starts from a clean index.
            index.remove_path(Path::new(name)).unwrap();
            index.write().unwrap();
            std::fs::remove_file(&file).unwrap();
        }
    }

    // mutation: return `true`.
    #[test]
    fn the_catalog_directory_must_be_the_checkouts_own() {
        assert!(catalog_dir_is_the_checkouts(
            Path::new("/r"),
            Path::new("/r/.codescout")
        ));
        assert!(!catalog_dir_is_the_checkouts(
            Path::new("/r"),
            Path::new("/elsewhere/.codescout")
        ));
        assert!(!catalog_dir_is_the_checkouts(
            Path::new("/r"),
            Path::new("/r/sub/.codescout")
        ));
    }

    // mutation: restore the refusal of a side file beside the new catalog.
    #[tokio::test]
    async fn build_over_a_catalog_with_a_leftover_side_file_rebuilds() {
        let f = fixture();
        exported_and_committed(&f).await;
        let file = f.root.join(DERIVED_CATALOG_REL);
        drop(schema::open(&file).unwrap());
        // What a build that was killed leaves: SQLite recovers it, so it is not a refusal.
        std::fs::write(f.root.join(".codescout/derived.db-wal"), b"x").unwrap();
        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert_eq!(count(&derived(&f), "document"), 4);
    }

    /// Hard-link `target` at `link`; `false` when the file system refuses, and the test returns.
    #[cfg(unix)]
    fn hard_link_made(target: &Path, link: &Path) -> bool {
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::fs::hard_link(target, link).is_ok()
    }

    #[cfg(unix)]
    const HARD_LINK_REFUSAL: &str =
        "has more than one hard link, and build would write into the same file under its other \
          name. Remove it, then run this again";

    // mutation: drop the hard-link check.
    #[cfg(unix)]
    #[tokio::test]
    async fn build_refuses_a_hard_linked_catalog_file_and_writes_nothing_outside() {
        let f = fixture();
        exported_and_committed(&f).await;
        let (_outside, outside) = outside_dir();
        let target = outside.join("other.db");
        std::fs::write(&target, b"outside").unwrap();
        if !hard_link_made(&target, &f.root.join(DERIVED_CATALOG_REL)) {
            return;
        }

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains(&format!(".codescout/derived.db {HARD_LINK_REFUSAL}")),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert!(
            !out.stdout.contains(outside.to_str().unwrap()),
            "{}",
            out.stdout
        );
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"outside",
            "left as it was"
        );
    }

    // mutation: drop the hard-link check.
    #[cfg(unix)]
    #[tokio::test]
    async fn build_refuses_a_catalog_hard_linked_from_another_checkout_and_leaves_it_unchanged() {
        use sha2::{Digest, Sha256};
        let f = fixture();
        exported_and_committed(&f).await;
        let first = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(first.code, 0, "{}", first.stdout);
        let main_file = f.root.join(DERIVED_CATALOG_REL);
        let main_sha = Sha256::digest(std::fs::read(&main_file).unwrap());

        // A clone made with `cp -al` shares the catalog's file with the main checkout.
        let (_clone_dir, clone_base) = outside_dir();
        let clone_root = clone_base.join("clone");
        let cloned = git2::Repository::clone(f.root.to_str().unwrap(), &clone_root).unwrap();
        no_global_excludes(&cloned);
        if !hard_link_made(&main_file, &clone_root.join(DERIVED_CATALOG_REL)) {
            return;
        }
        let own = CommonArgs {
            project: Some(clone_root.clone()),
            old_db: Some(f.db.clone()),
            old_root: vec![f.root.clone()],
            json: false,
        };

        let out = evaluate_build(&own, &clone_root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains(&format!(".codescout/derived.db {HARD_LINK_REFUSAL}")),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert!(
            !out.stdout.contains(clone_root.to_str().unwrap()),
            "{}",
            out.stdout
        );
        assert_eq!(
            Sha256::digest(std::fs::read(&main_file).unwrap()),
            main_sha,
            "the other checkout's catalog is byte for byte as it was"
        );
    }

    // mutation: check the hard link count of `derived.db` only, not of its side files.
    #[cfg(unix)]
    #[tokio::test]
    async fn build_refuses_a_hard_linked_side_file_and_writes_nothing_outside() {
        let f = fixture();
        exported_and_committed(&f).await;
        let (_outside, outside) = outside_dir();
        for name in &CATALOG_NAMES[1..] {
            let target = outside.join("other");
            std::fs::write(&target, b"outside").unwrap();
            let link = f.root.join(name);
            if !hard_link_made(&target, &link) {
                return;
            }

            let out = evaluate_build(&args(&f), &f.root, None, None).await;
            assert_eq!(out.code, 2, "{name}: {}", out.stdout);
            assert!(
                out.stdout.contains(&format!("{name} {HARD_LINK_REFUSAL}")),
                "{name}: {}",
                out.stdout
            );
            no_absolute_path(&f, &out.stdout);
            assert_eq!(std::fs::read(&target).unwrap(), b"outside", "{name}");
            std::fs::remove_file(&link).unwrap();
            std::fs::remove_file(&target).unwrap();
        }
        assert_eq!(catalog_files(&f.root), Vec::<PathBuf>::new());
    }

    // mutation: read stage 0 of the index only.
    #[tokio::test]
    async fn build_refuses_a_conflicted_catalog_entry() {
        let f = fixture();
        exported_and_committed(&f).await;
        let repo = git2::Repository::open(&f.root).unwrap();
        let mut index = repo.index().unwrap();
        // A merge that stopped on this file leaves it at stages 2 and 3, and no stage 0.
        for (stage, bytes) in [(2u16, &b"ours"[..]), (3, b"theirs")] {
            let entry = git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: 0o100644,
                uid: 0,
                gid: 0,
                file_size: bytes.len() as u32,
                id: repo.blob(bytes).unwrap(),
                flags: stage << 12,
                flags_extended: 0,
                path: DERIVED_CATALOG_REL.as_bytes().to_vec(),
            };
            index.add(&entry).unwrap();
        }
        index.write().unwrap();
        let index = git2::Repository::open(&f.root).unwrap().index().unwrap();
        let stages: Vec<i32> = (0..=3)
            .filter(|s| index.get_path(Path::new(DERIVED_CATALOG_REL), *s).is_some())
            .collect();
        assert_eq!(
            stages,
            [2, 3],
            "the index holds a conflict and no stage 0 entry"
        );

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains(".codescout/derived.db is tracked by git, and build would overwrite"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert!(!f.root.join(DERIVED_CATALOG_REL).exists());
    }

    // mutation: drop the `is_dir` arm for `.codescout`.
    #[tokio::test]
    async fn build_refuses_a_codescout_that_is_a_file_and_leaves_it_unchanged() {
        let f = fixture();
        exported_and_committed(&f).await;
        let path = f.root.join(".codescout");
        std::fs::remove_dir_all(&path).unwrap();
        std::fs::write(&path, b"not a directory").unwrap();

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                ".codescout in this checkout is not a directory. Replace it with a directory, \
                 then run this again"
            ),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!(std::fs::read(&path).unwrap(), b"not a directory");
    }

    // mutation: treat an error other than NotFound at a catalog name as "absent".
    #[cfg(unix)]
    #[tokio::test]
    async fn build_refuses_a_catalog_name_it_cannot_inspect() {
        use std::os::unix::fs::PermissionsExt;
        struct Restore(PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        let f = fixture();
        exported_and_committed(&f).await;
        let dir = f.root.join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        let _restore = Restore(dir.clone());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        let denied = std::fs::symlink_metadata(dir.join("derived.db"))
            .is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
        if !denied {
            return; // running as root: a mode does not stop the stat
        }

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .contains(".codescout/derived.db cannot be inspected: permission denied"),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
    }

    // mutation: change a name or the ignore line without the other (each follows the one path).
    #[test]
    fn the_catalog_names_and_the_ignore_line_follow_the_one_relative_path() {
        use crate::librarian::derived::old_catalog::SIDE_FILE_SUFFIXES;
        assert_eq!(CATALOG_NAMES[0], DERIVED_CATALOG_REL);
        let sides: Vec<String> = SIDE_FILE_SUFFIXES
            .iter()
            .map(|suffix| format!("{DERIVED_CATALOG_REL}{suffix}"))
            .collect();
        assert_eq!(&CATALOG_NAMES[1..], sides.as_slice());
        assert_eq!(IGNORE_LINE, format!("/{DERIVED_CATALOG_REL}*"));
    }

    // mutation: change one character of the `.gitignore` line or of `IGNORE_LINE`.
    #[test]
    fn the_repositorys_gitignore_holds_the_ignore_line() {
        let gitignore = include_str!("../../.gitignore");
        assert!(
            gitignore.lines().any(|line| line == IGNORE_LINE),
            "the root .gitignore has no line equal to {IGNORE_LINE}: build refuses in this \
             repository until git ignores the new catalog"
        );
    }

    // mutation: run the refusals and `derived_catalog_in` on `loaded.start` instead of
    // `loaded.git_root` (the nested link then refuses, or the build writes through it).
    #[cfg(unix)]
    #[tokio::test]
    async fn build_from_a_project_below_the_git_root_builds_the_git_roots_catalog() {
        let f = fixture();
        exported_and_committed(&f).await;
        assert!(
            f.root.join(".codescout").is_dir(),
            "the git root has its own .codescout"
        );
        let (_outside, outside) = outside_dir();
        let sub = f.root.join("sub");
        let nested = sub.join(".codescout");
        std::fs::create_dir_all(&nested).unwrap();
        // A dangling link in the nested `.codescout`: a write through it lands outside.
        let target = outside.join("target.db");
        std::os::unix::fs::symlink(&target, nested.join("derived.db")).unwrap();
        let below = CommonArgs {
            project: Some(sub.clone()),
            ..args(&f)
        };

        let out = evaluate_build(&below, &f.root, None, None).await;
        assert_eq!(out.code, 0, "{}", out.stdout);
        no_absolute_path(&f, &out.stdout);
        assert!(
            f.root.join(DERIVED_CATALOG_REL).is_file(),
            "the git root's catalog is built"
        );
        let recorded: String = derived(&f)
            .query_row("SELECT root FROM checkout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            recorded,
            f.root.to_str().unwrap(),
            "the catalog records the git root"
        );
        let in_nested: Vec<_> = std::fs::read_dir(&nested)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(
            in_nested,
            vec!["derived.db"],
            "the subdirectory gets no catalog"
        );
        assert_eq!(
            std::fs::read_link(nested.join("derived.db")).unwrap(),
            target,
            "the nested link is left as it was"
        );
        assert_eq!(
            std::fs::read_dir(&outside).unwrap().count(),
            0,
            "nothing is written outside the checkout"
        );
    }

    // mutation: drop the `is_dir` arm for a catalog name (then tmpfs says "hard link" and btrfs
    // says "writable").
    #[tokio::test]
    async fn build_refuses_a_catalog_name_that_is_a_directory_and_leaves_it_unchanged() {
        let f = fixture();
        exported_and_committed(&f).await;
        for name in ["derived.db", "derived.db-wal"] {
            let path = f.root.join(".codescout").join(name);
            std::fs::create_dir_all(&path).unwrap();

            let out = evaluate_build(&args(&f), &f.root, None, None).await;
            assert_eq!(out.code, 2, "{name}: {}", out.stdout);
            assert!(
                out.stdout.contains(&format!(
                    ".codescout/{name} is a directory, and build writes a file at that name. \
                     Remove the directory, then run this again"
                )),
                "{name}: {}",
                out.stdout
            );
            no_absolute_path(&f, &out.stdout);
            assert!(path.is_dir(), "{name}: the directory stays");
            assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0, "{name}");
            std::fs::remove_dir(&path).unwrap();
            assert!(
                !f.root.join(DERIVED_CATALOG_REL).exists(),
                "{name}: no catalog is created"
            );
        }
    }

    // mutation: treat an error other than NotFound at `.codescout` as "absent".
    #[cfg(unix)]
    #[test]
    fn a_codescout_that_cannot_be_inspected_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        struct Restore(PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }
        let f = fixture();
        let repo = git2::Repository::open(&f.root).unwrap();
        // A root that cannot be searched: its `.codescout` entry cannot be inspected.
        let (_locked_dir, locked) = outside_dir();
        let _restore = Restore(locked.clone());
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let denied = std::fs::symlink_metadata(locked.join(".codescout"))
            .is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
        if !denied {
            return; // running as root: a mode does not stop the stat
        }

        let err = refuse_unsafe_catalog_path(&repo, &locked).unwrap_err();
        assert_eq!(
            err.to_string(),
            ".codescout in this checkout cannot be inspected: permission denied"
        );
    }

    // mutation: check existence with `exists()`, which follows a link: a dangling link then
    // counts as absent.
    #[cfg(unix)]
    #[tokio::test]
    async fn export_refuses_a_dangling_link_at_the_new_catalog() {
        let f = fixture();
        let (_outside, outside) = outside_dir();
        std::fs::create_dir_all(f.root.join(".codescout")).unwrap();
        let target = outside.join("target.db");
        std::os::unix::fs::symlink(&target, f.root.join(DERIVED_CATALOG_REL)).unwrap();
        let c_before = read(&f, "c.md");

        let out = evaluate_export(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                "the new catalog (.codescout/derived.db) already exists in this checkout"
            ),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert_eq!(read(&f, "c.md"), c_before, "no file is written");
        assert_eq!(report_files(&f), (vec![], vec![]), "no report is kept");
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    }

    // mutation: read a git error as "ignored" (`unwrap_or(true)`).
    #[tokio::test]
    async fn build_refuses_when_git_cannot_tell_whether_the_new_catalog_is_ignored() {
        let f = fixture();
        exported_and_committed(&f).await;
        // libgit2 reads `core.ignorecase` for every ignore lookup and fails on a value that is
        // not a boolean; opening the repository and reading the index do not fail on it.
        git2::Repository::open(&f.root)
            .unwrap()
            .config()
            .unwrap()
            .set_str("core.ignorecase", "maybe")
            .unwrap();

        let out = evaluate_build(&args(&f), &f.root, None, None).await;
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains(
                "git cannot tell whether .codescout/derived.db is ignored in this checkout"
            ),
            "{}",
            out.stdout
        );
        no_absolute_path(&f, &out.stdout);
        assert!(!f.root.join(DERIVED_CATALOG_REL).exists());
    }
}
