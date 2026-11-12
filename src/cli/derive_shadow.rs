//! `codescout derive-shadow` — measure how far a derived index of one commit is from the old
//! catalog, writing nothing to the catalog and nothing to the repository.
//!
//! The command resolves the git root (`lookup_git_root`; no `open_ctx`, which would open and
//! migrate the catalog), opens the old catalog read-only, builds the derived index of `--rev`
//! into a scratch file in a temporary directory (outside the repository, removed on return) from
//! git objects alone, and prints [`diff_with_rules`]'s answer: counts, and the first
//! [`MAX_PRINTED_ROWS_PER_CAUSE`] rows of each cause (`--json`: the whole diff).
//!
//! Exit codes: `0` whenever the comparison ran, whatever it found; `2` and `NOT COMPARED —
//! <reason>` on stdout when it could not run: no old catalog, one that cannot be read, no git
//! repository, a revision that names no commit. The old catalog is never created, migrated or
//! written; see `derived::diff` for what SQLite does beside a WAL database, and
//! [`refuse_if_open_would_write`] for the case this command refuses.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::json;

use crate::librarian::current_project::lookup_git_root;
use crate::librarian::derived::build::{
    build_full, key_space_from_commit, rules_from_commit, BuildInput,
};
use crate::librarian::derived::diff::{
    diff_with_rules, open_old_read_only, DiffCause, DiffRow, ShadowDiff, Subject,
};
use crate::librarian::derived::scan::open_commit;
use crate::librarian::derived::schema;

#[derive(Debug, Args)]
pub struct DeriveShadowArgs {
    /// Project root override (defaults to cwd). The git root at or above it is compared.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// The commit whose derived index is compared (a ref or a sha).
    #[arg(long, default_value = "HEAD")]
    pub rev: String,
    /// The old catalog to read (default: `catalog.db` in the `librarian` directory of the
    /// machine's local data directory).
    #[arg(long)]
    pub old_db: Option<PathBuf>,
    /// Print the whole diff as one JSON object instead of counts and samples.
    #[arg(long)]
    pub json: bool,
}

/// What a run decided and what it prints. [`run`] prints it and exits with `code`.
#[derive(Debug)]
pub struct Outcome {
    /// `0` the comparison ran, `2` it could not.
    pub code: i32,
    pub stdout: String,
}

/// How many rows of each cause the text form prints. It bounds what a person reads, not what the
/// diff holds: the counts above the rows are complete, a line under a cut sample says how many
/// rows it left out, and `--json` carries every row.
// cap-class: NOT_A_CAP — a printed sample of a report whose full counts are printed beside it; the omitted rows are counted in the text and `--json` lists every one, so nothing is presented as complete
const MAX_PRINTED_ROWS_PER_CAUSE: usize = 20;

/// The `-wal` / `-shm` file names SQLite uses beside `path`.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Does the SQLite file at `path` say it is a WAL database? Bytes 18 and 19 of the header are
/// the file format's write and read versions; `2` is WAL. `None` when the file cannot be read
/// that far (absent, short, not readable): the open reports that by itself.
fn is_wal_database(path: &Path) -> Option<bool> {
    let mut header = [0u8; 20];
    // The old catalog is a machine-global SQLite file that no repository names, so this read
    // does not go through a repository's read door. It reads twenty bytes and writes nothing.
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .ok()?;
    Some(header[18] == 2 || header[19] == 2)
}

/// Refuse to open a WAL database that has no `-wal` and `-shm` beside it. SQLite does not refuse
/// a read-only open of one in a writable directory: it creates both files, and a command that
/// promises to write nothing beside the catalog must not do that. The case is a catalog no
/// process has open (a process that has it open keeps both files). Measured with the bundled
/// SQLite: the open succeeds and leaves `catalog.db-wal` and `catalog.db-shm` behind; in a
/// read-only directory it is refused with `attempt to write a readonly database`.
fn refuse_if_open_would_write(path: &Path) -> Result<()> {
    if is_wal_database(path) != Some(true) {
        return Ok(());
    }
    if sidecar(path, "-wal").exists() && sidecar(path, "-shm").exists() {
        return Ok(());
    }
    bail!(
        "{path} is a WAL database and {path}-wal and {path}-shm are not both beside it (no \
         process has it open): SQLite would create them to read it, and this command writes \
         nothing beside the catalog. Run it while a codescout server has the catalog open",
        path = path.display()
    )
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

fn not_compared(json: bool, reason: &str) -> Outcome {
    let stdout = if json {
        format!(
            "{}\n",
            json!({"compared": false, "exit_code": 2, "not_compared": reason})
        )
    } else {
        format!("NOT COMPARED — {reason}\n")
    };
    Outcome { code: 2, stdout }
}

/// Decide the outcome of `args`, resolving `--project` against `cwd`, the old catalog against
/// `default_old_db` when `--old-db` is not given, and building the scratch index under
/// `scratch_parent`. It returns no error: whatever stops the comparison from running becomes a
/// `NOT COMPARED` outcome with code 2.
pub fn evaluate(
    args: &DeriveShadowArgs,
    cwd: &Path,
    default_old_db: Option<&Path>,
    scratch_parent: &Path,
) -> Outcome {
    compare(args, cwd, default_old_db, scratch_parent)
        .unwrap_or_else(|e| not_compared(args.json, &one_line(&e)))
}

/// What a run compared, for the report's header.
#[derive(Serialize)]
struct Compared<'a> {
    compared: bool,
    old_catalog: String,
    git_root: String,
    rev: &'a str,
    commit: String,
    diff: &'a ShadowDiff,
}

fn compare(
    args: &DeriveShadowArgs,
    cwd: &Path,
    default_old_db: Option<&Path>,
    scratch_parent: &Path,
) -> Result<Outcome> {
    // The old catalog first: it is the cheap check, and a missing one needs no build.
    let old_path = args.old_db.as_deref().or(default_old_db).ok_or_else(|| {
        anyhow!(
            "no --old-db was given and this machine has no local data directory to look for \
             librarian/catalog.db in"
        )
    })?;
    refuse_if_open_would_write(old_path)?;
    let old = open_old_read_only(old_path)?;

    let start = args.project.clone().unwrap_or_else(|| cwd.to_path_buf());
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot resolve {}", start.display()))?;
    let root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("{} is not inside a git repository", start.display()))?;
    let repo = git2::Repository::open(&root)
        .with_context(|| format!("opening the git repository at {}", root.display()))?;
    let commit = open_commit(&repo, &args.rev)?;
    let rules = rules_from_commit(&repo, &commit)?;
    let space = key_space_from_commit(&repo, &commit)?;

    // The scratch index lives in a temporary directory outside the repository and goes with it.
    let scratch = tempfile::tempdir_in(scratch_parent).with_context(|| {
        format!(
            "creating a scratch directory in {}",
            scratch_parent.display()
        )
    })?;
    let scratch_path = std::fs::canonicalize(scratch.path())
        .with_context(|| format!("cannot resolve {}", scratch.path().display()))?;
    if scratch_path.starts_with(&root) {
        bail!(
            "the scratch directory {} would lie inside the repository {}; point TMPDIR elsewhere",
            scratch_path.display(),
            root.display()
        );
    }
    let mut conn = schema::open(&scratch_path.join("derived.db"))
        .context("creating the scratch derived index")?;
    build_full(
        &mut conn,
        &BuildInput {
            repo: &repo,
            commit: &commit,
            root: &root,
            rules: &rules,
            space: &space,
        },
    )
    .with_context(|| format!("building the derived index of {}", args.rev))?;
    let shadow = diff_with_rules(&old, &conn, &root, &rules)?;
    drop(conn);

    let compared = Compared {
        compared: true,
        old_catalog: old_path.display().to_string(),
        git_root: root.display().to_string(),
        rev: &args.rev,
        commit: commit.id().to_string(),
        diff: &shadow,
    };
    let stdout = if args.json {
        format!("{}\n", serde_json::to_string(&compared)?)
    } else {
        render_text(&compared)
    };
    Ok(Outcome { code: 0, stdout })
}

/// Every row of the diff with the list it came from.
fn labelled(shadow: &ShadowDiff) -> impl Iterator<Item = (&'static str, &DiffRow)> {
    shadow
        .only_old
        .iter()
        .map(|r| ("only_old", r))
        .chain(shadow.only_new.iter().map(|r| ("only_new", r)))
        .chain(shadow.changed.iter().map(|r| ("changed", r)))
}

fn render_row(bucket: &str, row: &DiffRow) -> String {
    let subject = match row.subject {
        Subject::Document => "document",
        Subject::FileEdge => "file_edge",
        Subject::EntryEdge => "entry_edge",
    };
    let shown = |v: &Option<String>| v.clone().unwrap_or_else(|| "<none>".to_owned());
    let detail = match (&row.field, row.subject) {
        (Some(field), _) => format!(" {field}: {} -> {}", shown(&row.old), shown(&row.new)),
        (None, Subject::Document) => row
            .new
            .as_ref()
            .map(|note| format!(" ({note})"))
            .unwrap_or_default(),
        (None, _) => format!(
            " -> {}",
            row.old
                .as_ref()
                .or(row.new.as_ref())
                .unwrap_or(&String::new())
        ),
    };
    format!("  {bucket} {subject} {}{detail}\n", row.path)
}

fn render_text(report: &Compared) -> String {
    let c = &report.diff.counts;
    let mut out = String::new();
    out.push_str(&format!(
        "old catalog: {}\nderived index: {} ({}) of {}\n\n",
        report.old_catalog, report.rev, report.commit, report.git_root
    ));
    out.push_str(&format!(
        "documents: old {} (in scope), derived {}, on both sides {}\n  only old {}, only derived {}, changed {} (one row per differing field)\n  old rows ignored: {} in other roots, {} under .worktrees, {} with missing_since set\n",
        c.old_documents, c.new_documents, c.matched_documents, c.only_old, c.only_new, c.changed,
        c.ignored_other_roots, c.ignored_worktrees, c.ignored_missing
    ));
    out.push_str(&format!(
        "edges, file grain: old {}, derived {}; entry grain: old {}, derived {}\n  not compared, an end is not a document on both sides: old {}, derived {}\n  old edge rows ignored, an end is out of scope: {}\n\n",
        c.old_file_edges, c.new_file_edges, c.old_entry_edges, c.new_entry_edges,
        c.old_edges_not_compared, c.new_edges_not_compared, c.ignored_old_edges
    ));
    out.push_str("rows by cause:");
    for cause in DiffCause::ALL {
        out.push_str(&format!(
            " {} {},",
            cause.label(),
            c.by_cause.get(&cause).copied().unwrap_or(0)
        ));
    }
    out.pop();
    out.push('\n');
    for cause in DiffCause::ALL {
        let total = c.by_cause.get(&cause).copied().unwrap_or(0);
        if total == 0 {
            continue;
        }
        out.push_str(&format!("\n{} ({total} rows)\n", cause.label()));
        for (bucket, row) in labelled(report.diff)
            .filter(|(_, r)| r.cause == cause)
            .take(MAX_PRINTED_ROWS_PER_CAUSE)
        {
            out.push_str(&render_row(bucket, row));
        }
        if total > MAX_PRINTED_ROWS_PER_CAUSE {
            out.push_str(&format!(
                "  ... {} more rows not shown; --json lists every row\n",
                total - MAX_PRINTED_ROWS_PER_CAUSE
            ));
        }
    }
    out
}

pub async fn run(args: DeriveShadowArgs) -> Result<()> {
    use std::io::Write;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let default_old_db = dirs::data_local_dir().map(|d| d.join("librarian").join("catalog.db"));
    let out = evaluate(
        &args,
        &cwd,
        default_old_db.as_deref(),
        &std::env::temp_dir(),
    );
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
    use crate::librarian::derived::testkit::{fill_old_catalog, old_row, repo_with, OldRow, FILE};
    use rusqlite::Connection;

    struct Fixture {
        /// The repository the command is run in (its commit holds the markdown files).
        _repo: tempfile::TempDir,
        root: PathBuf,
        /// Where the command may build its scratch index.
        scratch: tempfile::TempDir,
        /// Where the old catalog lives.
        old_dir: tempfile::TempDir,
    }

    fn fixture(files: &[(&str, &str)]) -> Fixture {
        let repo_dir = tempfile::tempdir().unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|&(path, text)| (path, FILE, text.as_bytes()))
            .collect();
        repo_with(repo_dir.path(), &entries);
        Fixture {
            root: std::fs::canonicalize(repo_dir.path()).unwrap(),
            _repo: repo_dir,
            scratch: tempfile::tempdir().unwrap(),
            old_dir: tempfile::tempdir().unwrap(),
        }
    }

    fn args(old_db: Option<PathBuf>, json: bool) -> DeriveShadowArgs {
        DeriveShadowArgs {
            project: None,
            rev: "HEAD".to_owned(),
            old_db,
            json,
        }
    }

    fn eval(f: &Fixture, args: &DeriveShadowArgs, default: Option<&Path>) -> Outcome {
        evaluate(args, &f.root, default, f.scratch.path())
    }

    /// An old row for `rel` under the fixture's git root.
    fn old(f: &Fixture, id: &str, rel: &str) -> OldRow {
        old_row(id, &format!("{}/{rel}", f.root.display()))
    }

    /// A rollback-journal old catalog file holding `rows`.
    fn old_file(f: &Fixture, rows: &[OldRow]) -> PathBuf {
        let path = f.old_dir.path().join("catalog.db");
        let conn = Connection::open(&path).unwrap();
        fill_old_catalog(&conn, rows, &[], &[], &[]);
        path
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_missing_old_catalog_exits_2_says_not_compared_and_creates_nothing() {
        let f = fixture(&[("a.md", "# A\n")]);
        let missing = f.old_dir.path().join("catalog.db");
        let out = eval(&f, &args(Some(missing.clone()), false), None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(out.stdout.starts_with("NOT COMPARED — "), "{}", out.stdout);
        assert_eq!(out.stdout.lines().count(), 1, "one line: {}", out.stdout);
        assert!(
            out.stdout.contains(&missing.display().to_string()),
            "names the path: {}",
            out.stdout
        );
        assert!(!missing.exists(), "the command created the old catalog");
        assert!(listing(f.old_dir.path()).is_empty());
        assert!(
            listing(f.scratch.path()).is_empty(),
            "no build for a missing catalog"
        );

        // The same, as JSON.
        let json = eval(&f, &args(Some(missing), true), None);
        assert_eq!(json.code, 2);
        let v: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
        assert_eq!(v["compared"], false);
        assert_eq!(v["exit_code"], 2);
        assert!(v["not_compared"].as_str().unwrap().contains("catalog.db"));
    }

    #[test]
    fn no_old_db_and_no_default_location_is_not_compared_and_a_default_is_used_when_given() {
        let f = fixture(&[("a.md", "# A\n")]);
        let none = eval(&f, &args(None, false), None);
        assert_eq!(none.code, 2);
        assert!(none.stdout.contains("--old-db"), "{}", none.stdout);

        let default = old_file(&f, &[]);
        let used = eval(&f, &args(None, false), Some(&default));
        assert_eq!(used.code, 0, "{}", used.stdout);
        assert!(used.stdout.contains(&default.display().to_string()));
    }

    #[test]
    fn a_comparison_with_differences_exits_0_prints_counts_and_samples_and_leaves_no_trace() {
        let f = fixture(&[("a.md", "# A\n"), ("b.md", "# B\n")]);
        let mut a = old(&f, "id-a", "a.md");
        a.kind = "plan".to_owned();
        let ghost = old(&f, "id-ghost", "untracked.md");
        let path = old_file(&f, &[a, ghost]);
        let bytes = std::fs::read(&path).unwrap();
        let before = listing(f.old_dir.path());

        let out = eval(&f, &args(Some(path.clone()), false), None);

        assert_eq!(out.code, 0, "differences are not a failure: {}", out.stdout);
        let text = &out.stdout;
        assert!(
            text.contains("documents: old 2 (in scope), derived 2, on both sides 1"),
            "{text}"
        );
        assert!(
            text.contains("only old 1, only derived 1, changed"),
            "{text}"
        );
        assert!(text.contains("file_set_walk_vs_commit (2 rows)"), "{text}");
        assert!(text.contains("  only_old document untracked.md"), "{text}");
        assert!(text.contains("  only_new document b.md"), "{text}");
        assert!(text.contains("unexplained ("), "{text}");
        assert!(
            text.contains("  changed document a.md kind: plan -> unknown"),
            "{text}"
        );
        // Nothing beside the old catalog changed, the repository holds nothing the run made,
        // and the scratch index is gone.
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "the old catalog's bytes changed"
        );
        assert_eq!(listing(f.old_dir.path()), before);
        assert_eq!(listing(&f.root), vec![".git".to_owned()]);
        assert!(
            listing(f.scratch.path()).is_empty(),
            "the scratch directory was not removed"
        );
    }

    #[test]
    fn json_carries_every_row_where_the_text_shows_twenty_per_cause_and_says_what_it_left_out() {
        let names: Vec<String> = (0..25).map(|i| format!("doc-{i:02}.md")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "# T\n")).collect();
        let f = fixture(&files);
        let path = old_file(&f, &[]);

        let text = eval(&f, &args(Some(path.clone()), false), None).stdout;
        assert!(text.contains("file_set_walk_vs_commit (25 rows)"), "{text}");
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("  only_new document "))
                .count(),
            20,
            "{text}"
        );
        assert!(
            text.contains("  ... 5 more rows not shown; --json lists every row"),
            "{text}"
        );
        assert!(
            text.contains("only derived 25"),
            "the counts are complete: {text}"
        );

        let json = eval(&f, &args(Some(path), true), None);
        assert_eq!(json.code, 0);
        let v: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
        assert_eq!(v["compared"], true);
        assert_eq!(v["diff"]["only_new"].as_array().unwrap().len(), 25);
        assert_eq!(v["diff"]["counts"]["only_new"], 25);
        assert_eq!(
            v["diff"]["counts"]["by_cause"]["file_set_walk_vs_commit"],
            25
        );
        assert_eq!(v["diff"]["counts"]["by_cause"]["unexplained"], 0);
        assert_eq!(v["diff"]["only_new"][0]["cause"], "file_set_walk_vs_commit");
        assert_eq!(v["diff"]["only_new"][0]["subject"], "document");
        assert_eq!(v["rev"], "HEAD");
    }

    #[test]
    fn a_wal_catalog_no_process_holds_open_is_not_compared_and_nothing_appears_beside_it() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = f.old_dir.path().join("catalog.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            fill_old_catalog(&conn, &[], &[], &[], &[]);
        }
        assert_eq!(
            listing(f.old_dir.path()),
            vec!["catalog.db".to_owned()],
            "a cleanly closed WAL database leaves no side files (the premise of this test)"
        );
        let bytes = std::fs::read(&path).unwrap();

        let out = eval(&f, &args(Some(path.clone()), false), None);

        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(out.stdout.starts_with("NOT COMPARED — "), "{}", out.stdout);
        assert!(out.stdout.contains("WAL database"), "{}", out.stdout);
        assert_eq!(listing(f.old_dir.path()), vec!["catalog.db".to_owned()]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn a_wal_catalog_another_process_holds_open_is_compared_without_writing_to_it() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = f.old_dir.path().join("catalog.db");
        // This connection stands for a running codescout server: it keeps the side files.
        let holder = Connection::open(&path).unwrap();
        holder.pragma_update(None, "journal_mode", "WAL").unwrap();
        fill_old_catalog(&holder, &[old(&f, "id-a", "a.md")], &[], &[], &[]);
        let before = listing(f.old_dir.path());
        assert_eq!(
            before,
            vec![
                "catalog.db".to_owned(),
                "catalog.db-shm".to_owned(),
                "catalog.db-wal".to_owned()
            ],
            "the premise: a live WAL database has both side files"
        );
        let main_bytes = std::fs::read(&path).unwrap();

        let out = eval(&f, &args(Some(path.clone()), false), None);

        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("documents: old 1 (in scope), derived 1, on both sides 1"),
            "{}",
            out.stdout
        );
        assert_eq!(listing(f.old_dir.path()), before);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            main_bytes,
            "the main file changed"
        );
        let rows: i64 = holder
            .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "the holder still sees its data");
    }

    #[test]
    fn a_revision_that_names_no_commit_is_not_compared() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[]);
        let mut a = args(Some(path), false);
        a.rev = "no-such-ref".to_owned();
        let out = eval(&f, &a, None);
        assert_eq!(out.code, 2);
        assert!(out.stdout.starts_with("NOT COMPARED — "), "{}", out.stdout);
        assert!(out.stdout.contains("no-such-ref"), "{}", out.stdout);
    }

    #[test]
    fn a_scratch_directory_inside_the_repository_is_refused() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[]);
        let out = evaluate(&args(Some(path), false), &f.root, None, &f.root);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("inside the repository"),
            "{}",
            out.stdout
        );
        assert_eq!(
            listing(&f.root),
            vec![".git".to_owned()],
            "the refused scratch directory is gone"
        );
    }
}
