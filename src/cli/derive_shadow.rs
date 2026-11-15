//! `codescout derive-shadow` — measure how far a derived index of one commit is from the old
//! catalog. Nothing is written to the old catalog or its `-wal`, and nothing to the repository;
//! SQLite itself updates read marks in an existing `-shm` (see `derived::diff`).
//!
//! The command resolves the git root (`lookup_git_root`; no `open_ctx`, which would open and
//! migrate the catalog), opens the old catalog read-only, builds the derived index of `--rev`
//! into a scratch file in a temporary directory (outside the repository, removed on return) from
//! git objects alone, and prints [`diff_with_rules`]'s answer: counts, and the first
//! [`MAX_PRINTED_ROWS_PER_CAUSE`] rows of each cause (`--json`: the whole diff).
//!
//! Exit codes: `0` whenever the comparison ran, whatever it found; `2` and `NOT COMPARED —
//! <reason>` on stdout when it could not run (every error `compare` returns becomes one):
//!
//! - the old catalog: no `--old-db`, no `LIBRARIAN_DB` and no local data directory to look for
//!   `librarian/catalog.db` in ([`resolve_old_catalog`]); a file that does not exist (it is never
//!   created) or cannot be opened or read (not a database, a table or column the diff reads is
//!   missing, a query fails); a `-wal`, `-shm` or `-journal` file beside it that is not the pair
//!   a running server keeps ([`open_old`]); a catalog with rows of which none lies under the git
//!   root or an `--old-root` (a root spelled differently on the two sides); an `--old-root` that
//!   is not an absolute path;
//! - the repository: a `--project` that cannot be resolved; no git repository at or above it, or
//!   one that cannot be opened; a `--rev` that names no commit; a `.codescout/librarian.toml` or
//!   `.codescout/key-space.toml` in that commit that does not parse or is a symlink;
//! - the scratch index: a temporary directory that cannot be resolved or created, or that would
//!   lie inside the repository (point `TMPDIR` elsewhere); a derived index that cannot be
//!   created or built (a `.codescout/repo.toml` in the commit that does not parse or is a
//!   symlink, for one);
//! - the report: it cannot be serialised for `--json`.
//!
//! The old catalog is never created or migrated; see `derived::diff` for what SQLite does beside
//! a WAL database.

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use serde::Serialize;
use serde_json::json;

use crate::librarian::current_project::lookup_git_root;
use crate::librarian::derived::build::{build_full, BuildInput};
use crate::librarian::derived::diff::{
    diff_with_rules, open_old_immutable, open_old_read_only, DiffCause, DiffRow, OldCatalog,
    RootMap, ShadowDiff, Subject,
};
use crate::librarian::derived::old_catalog::{resolve_old_catalog, side_file_present};
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
    /// The old catalog to read (default: `LIBRARIAN_DB`, else `catalog.db` in the `librarian`
    /// directory of the machine's local data directory).
    #[arg(long)]
    pub old_db: Option<PathBuf>,
    /// Another spelling of the git root that old rows carry, as an absolute path (repeatable).
    /// The old indexer stores each path as its caller held it: through a symlink, or in a
    /// checkout that has moved.
    #[arg(long = "old-root")]
    pub old_root: Vec<PathBuf>,
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

/// Open the old catalog without creating or writing a file beside it.
///
/// The path is resolved through symlinks first, because SQLite keeps `-wal` and `-shm` beside the
/// target, and every decision below looks there.
///
/// A WAL database that a codescout server holds open has its `-wal` and `-shm`, and no
/// non-empty `-journal`: it is read through [`open_old_read_only`], which writes nothing to the
/// catalog or its `-wal` (SQLite updates read marks in the `-shm`, see `derived::diff`) and sees
/// the rows that lie in the `-wal`. Every other catalog is opened immutable
/// ([`open_old_immutable`]): a WAL catalog that no process holds opens and nothing appears beside
/// it, and a catalog with one side file, or a non-empty `-journal`, is refused, because an
/// immutable open would ignore the rows in its `-wal`. The server can still start while this
/// command runs, and the answer is then out of date; `derived::diff` states the cost.
fn open_old(path: &Path) -> Result<OldCatalog> {
    let absolute =
        std::path::absolute(path).with_context(|| format!("cannot resolve {}", path.display()))?;
    let resolved = std::fs::canonicalize(&absolute).unwrap_or(absolute);
    let held = is_wal_database(&resolved) == Some(true)
        && side_file_present(&resolved, "-wal")
        && side_file_present(&resolved, "-shm")
        && !side_file_present(&resolved, "-journal");
    if held {
        return open_old_read_only(&resolved);
    }
    open_old_immutable(&resolved).with_context(|| format!("old catalog {}", path.display()))
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
            "no --old-db was given, LIBRARIAN_DB is not set, and this machine has no local data \
             directory to look for librarian/catalog.db in"
        )
    })?;
    if let Some(relative) = args.old_root.iter().find(|p| !p.is_absolute()) {
        bail!(
            "--old-root takes an absolute path, and {} is not one",
            relative.display()
        );
    }
    let old = open_old(old_path)?;

    let start = args.project.clone().unwrap_or_else(|| cwd.to_path_buf());
    let start = std::fs::canonicalize(&start)
        .with_context(|| format!("cannot resolve {}", start.display()))?;
    let root = lookup_git_root(&start)
        .ok_or_else(|| anyhow!("{} is not inside a git repository", start.display()))?;
    // Judged before anything is built: an old root above the git root is refused here.
    let roots = RootMap::with_old_roots(root.clone(), args.old_root.clone())?;
    let repo = git2::Repository::open(&root)
        .with_context(|| format!("opening the git repository at {}", root.display()))?;
    let commit = open_commit(&repo, &args.rev)?;
    let input = BuildInput::from_commit(&repo, &commit, &root)?;

    // The scratch index lives in a temporary directory outside the repository and goes with it.
    // Where it may go is judged BEFORE anything is created there.
    let parent = std::fs::canonicalize(scratch_parent)
        .with_context(|| format!("cannot resolve {}", scratch_parent.display()))?;
    if parent.starts_with(&root) {
        bail!(
            "the scratch directory would lie inside the repository {} (its parent {} does); \
             point TMPDIR elsewhere",
            root.display(),
            parent.display()
        );
    }
    let scratch = tempfile::tempdir_in(&parent)
        .with_context(|| format!("creating a scratch directory in {}", parent.display()))?;
    let mut conn = schema::open(&scratch.path().join("derived.db"))
        .context("creating the scratch derived index")?;
    build_full(&mut conn, &input)
        .with_context(|| format!("building the derived index of {}", args.rev))?;
    let shadow = diff_with_rules(&old, &conn, &roots, input.rules())?;
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
        "documents: old {} (in scope), derived {}, on both sides {}\n  only old {}, only derived {}, changed {} (one row per differing field)\n  old rows ignored: {} in other roots, {} under .worktrees, {} with missing_since set\n  old paths that collide: {} (two rows naming one file; the lowest id is kept)\n",
        c.old_documents, c.new_documents, c.matched_documents, c.only_old, c.only_new, c.changed,
        c.ignored_other_roots, c.ignored_worktrees, c.ignored_missing, c.old_path_collisions
    ));
    out.push_str(&format!(
        "edges, file grain: old {}, derived {}; entry grain: old {}, derived {}\n  note: a missing citation gives a file_edge row and an entry_edge row (the grains overlap)\n  not compared, an end is not a document on both sides: old {}, derived {}\n  derived entry-grain edges not compared: no slug {}, outside any entry {}, inside their own entry {}\n  old edge rows ignored, an end is out of scope: {}\n\n",
        c.old_file_edges, c.new_file_edges, c.old_entry_edges, c.new_entry_edges,
        c.old_edges_not_compared, c.new_edges_not_compared,
        c.new_edges_not_compared_no_slug, c.new_edges_not_compared_outside_an_entry,
        c.new_edges_not_compared_own_entry, c.ignored_old_edges
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
    let default_old_db = resolve_old_catalog(
        None,
        std::env::var_os("LIBRARIAN_DB"),
        dirs::data_local_dir(),
    );
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
            old_root: Vec::new(),
            json,
        }
    }

    /// [`args`] with `--old-root` given once per entry of `roots`.
    fn args_with_old_roots(old_db: PathBuf, roots: &[&str]) -> DeriveShadowArgs {
        DeriveShadowArgs {
            old_root: roots.iter().map(PathBuf::from).collect(),
            ..args(Some(old_db), false)
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
    fn a_wal_catalog_no_process_holds_open_is_compared_and_nothing_appears_beside_it() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = f.old_dir.path().join("catalog.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            fill_old_catalog(&conn, &[old(&f, "id-a", "a.md")], &[], &[], &[]);
        }
        assert_eq!(
            listing(f.old_dir.path()),
            vec!["catalog.db".to_owned()],
            "a cleanly closed WAL database leaves no side files (the premise of this test)"
        );
        let bytes = std::fs::read(&path).unwrap();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

        let out = eval(&f, &args(Some(path.clone()), false), None);

        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("documents: old 1 (in scope), derived 1, on both sides 1"),
            "{}",
            out.stdout
        );
        assert_eq!(listing(f.old_dir.path()), vec!["catalog.db".to_owned()]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);
    }

    #[test]
    fn a_wal_catalog_with_one_side_file_is_not_compared() {
        for suffix in ["-wal", "-shm"] {
            let f = fixture(&[("a.md", "# A\n")]);
            let path = f.old_dir.path().join("catalog.db");
            {
                let conn = Connection::open(&path).unwrap();
                conn.pragma_update(None, "journal_mode", "WAL").unwrap();
                fill_old_catalog(&conn, &[old(&f, "id-a", "a.md")], &[], &[], &[]);
            }
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            std::fs::write(&side, b"").unwrap();
            let before = listing(f.old_dir.path());
            assert_eq!(before.len(), 2, "{suffix}: the premise");

            let out = eval(&f, &args(Some(path.clone()), false), None);

            assert_eq!(out.code, 2, "{suffix}: {}", out.stdout);
            assert!(out.stdout.starts_with("NOT COMPARED — "), "{}", out.stdout);
            assert!(
                out.stdout.contains("Stop every codescout process"),
                "{suffix}: {}",
                out.stdout
            );
            assert_eq!(listing(f.old_dir.path()), before, "{suffix}");
        }
    }

    #[test]
    fn a_leftover_journal_is_not_compared() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[old(&f, "id-a", "a.md")]);
        // A journal with bytes may be a hot journal: reading around it could miss a rollback.
        std::fs::write(f.old_dir.path().join("catalog.db-journal"), b"x").unwrap();
        let before = listing(f.old_dir.path());

        let out = eval(&f, &args(Some(path), false), None);

        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout.contains("Stop every codescout process"),
            "{}",
            out.stdout
        );
        assert_eq!(listing(f.old_dir.path()), before);
    }

    #[test]
    fn a_zero_length_journal_is_not_a_hot_journal_and_the_catalog_is_compared() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[old(&f, "id-a", "a.md")]);
        std::fs::write(f.old_dir.path().join("catalog.db-journal"), b"").unwrap();
        let before = listing(f.old_dir.path());

        let out = eval(&f, &args(Some(path), false), None);

        assert_eq!(out.code, 0, "{}", out.stdout);
        assert_eq!(listing(f.old_dir.path()), before);
    }

    #[test]
    fn an_old_root_above_the_git_root_is_not_compared_and_names_the_flag() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[old(&f, "id-a", "a.md")]);
        let above = f.root.parent().unwrap().to_str().unwrap().to_owned();
        let out = eval(&f, &args_with_old_roots(path, &[above.as_str()]), None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(out.stdout.starts_with("NOT COMPARED — "), "{}", out.stdout);
        assert!(out.stdout.contains("--old-root"), "{}", out.stdout);
        assert!(
            !out.stdout.contains(&above),
            "the refusal names no path: {}",
            out.stdout
        );
    }

    /// The server keeps `-wal` and `-shm` beside the symlink's target; the decision whether a
    /// server holds the catalog must look there.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_catalog_a_server_holds_open_is_compared_through_its_target() {
        let f = fixture(&[("a.md", "# A\n")]);
        let real_dir = tempfile::tempdir().unwrap();
        let real = real_dir.path().join("catalog.db");
        {
            let conn = Connection::open(&real).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            fill_old_catalog(&conn, &[old(&f, "id-a", "a.md")], &[], &[], &[]);
        }
        let link = f.old_dir.path().join("catalog.db");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        // The server opened it through the link, and its second row lies only in the `-wal`.
        let holder = Connection::open(&link).unwrap();
        holder.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        holder
            .execute(
                "INSERT INTO artifact (id, abs_path, kind, status, owners, tags, file_sha256) \
                 VALUES ('id-w', ?1, 'doc', 'active', '[]', '[]', 'x')",
                [format!("{}/w.md", f.root.display())],
            )
            .unwrap();
        assert_eq!(listing(f.old_dir.path()), vec!["catalog.db".to_owned()]);

        let out = eval(&f, &args(Some(link), false), None);

        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("documents: old 2 (in scope), derived 1, on both sides 1"),
            "the row only the -wal holds is read: {}",
            out.stdout
        );
    }

    #[test]
    fn rows_under_an_old_root_are_compared() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[old_row("id-a", "/old/spelling/a.md")]);

        // Without the flag the run cannot say where the row belongs.
        let without = eval(&f, &args(Some(path.clone()), false), None);
        assert_eq!(without.code, 2, "{}", without.stdout);

        let out = eval(&f, &args_with_old_roots(path, &["/old/spelling"]), None);
        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("documents: old 1 (in scope), derived 1, on both sides 1"),
            "{}",
            out.stdout
        );
        assert!(
            out.stdout.contains("old rows ignored: 0 in other roots"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn an_old_root_that_maps_no_row_is_not_compared_and_names_the_flag() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[old_row("id-x", "/elsewhere/other-repo/a.md")]);
        let out = eval(&f, &args_with_old_roots(path, &["/old/spelling"]), None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert_eq!(
            out.stdout,
            "NOT COMPARED — no old row lies under the git root or under any of the 1 \
             --old-root paths given (1 row lies under other roots)\n"
        );
    }

    #[test]
    fn a_relative_old_root_is_refused() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[]);
        let out = eval(&f, &args_with_old_roots(path, &["old/spelling"]), None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(out.stdout.starts_with("NOT COMPARED — "), "{}", out.stdout);
        assert!(out.stdout.contains("--old-root"), "{}", out.stdout);
        assert!(out.stdout.contains("absolute"), "{}", out.stdout);
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
        let wal_path = f.old_dir.path().join("catalog.db-wal");
        let wal_bytes = std::fs::read(&wal_path).unwrap();

        let out = eval(&f, &args(Some(path.clone()), false), None);

        assert_eq!(out.code, 0, "{}", out.stdout);
        assert!(
            out.stdout
                .contains("documents: old 1 (in scope), derived 1, on both sides 1"),
            "{}",
            out.stdout
        );
        // What is pinned: no new file, and the catalog and its `-wal` are byte-identical. The
        // `-shm` is deliberately not compared: SQLite updates read marks there (`derived::diff`).
        assert_eq!(listing(f.old_dir.path()), before);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            main_bytes,
            "the main file changed"
        );
        assert_eq!(
            std::fs::read(&wal_path).unwrap(),
            wal_bytes,
            "the -wal file changed"
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

    #[test]
    fn an_old_catalog_whose_rows_all_lie_under_another_root_is_not_compared() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[old_row("id-x", "/elsewhere/other-repo/a.md")]);
        let out = eval(&f, &args(Some(path), false), None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert_eq!(
            out.stdout,
            "NOT COMPARED — no old row lies under the git root (1 row lies under other \
             roots; is the root spelled through a symlink? pass --old-root <the other \
             spelling>)\n"
        );
        assert!(
            !out.stdout.contains(&f.root.display().to_string()),
            "the refusal names no path: {}",
            out.stdout
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_project_indexed_through_a_symlink_is_not_compared_instead_of_called_all_new() {
        let f = fixture(&[("a.md", "# A\n")]);
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("proj");
        std::os::unix::fs::symlink(&f.root, &link).unwrap();
        // The old indexer stored the path as its caller held it: through the link.
        let path = old_file(&f, &[old_row("id-a", &format!("{}/a.md", link.display()))]);
        let out = eval(&f, &args(Some(path.clone()), false), None);
        assert_eq!(out.code, 2, "{}", out.stdout);
        assert!(
            out.stdout
                .starts_with("NOT COMPARED — no old row lies under "),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("symlink"), "{}", out.stdout);
        assert!(out.stdout.contains("--old-root"), "{}", out.stdout);
        assert!(
            !out.stdout.contains(&f.root.display().to_string()),
            "the refusal names no path: {}",
            out.stdout
        );
        // The flag is the remedy the text names: the link spelling maps the row.
        let fixed = eval(
            &f,
            &args_with_old_roots(path, &[link.to_str().unwrap()]),
            None,
        );
        assert_eq!(fixed.code, 0, "{}", fixed.stdout);
    }

    #[test]
    fn an_empty_old_catalog_still_compares_and_every_document_is_only_derived() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[]);
        let out = eval(&f, &args(Some(path), false), None);
        assert_eq!(out.code, 0, "{}", out.stdout);
        let text = &out.stdout;
        assert!(
            text.contains("documents: old 0 (in scope), derived 1, on both sides 0"),
            "{text}"
        );
        assert!(text.contains("file_set_walk_vs_commit (1 rows)"), "{text}");
        assert!(text.contains("  only_new document a.md"), "{text}");
    }

    #[test]
    fn the_text_header_names_the_edge_grains_and_every_not_compared_counter() {
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[]);
        let text = eval(&f, &args(Some(path), false), None).stdout;
        assert!(
            text.contains("a missing citation gives a file_edge row and an entry_edge row"),
            "{text}"
        );
        assert!(
            text.contains(
                "derived entry-grain edges not compared: no slug 0, outside any entry 0, \
                 inside their own entry 0"
            ),
            "{text}"
        );
        assert!(text.contains("old paths that collide: 0"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn a_scratch_parent_inside_the_repository_is_refused_before_a_directory_is_made_in_it() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture(&[("a.md", "# A\n")]);
        let path = old_file(&f, &[]);
        let parent = f.root.join("scratch");
        std::fs::create_dir(&parent).unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555)).unwrap();
        let restore = || {
            std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        // A process that may write anyway (root) cannot tell the two orders apart.
        if std::fs::File::create(parent.join("probe")).is_ok() {
            restore();
            return;
        }
        let out = evaluate(&args(Some(path), false), &f.root, None, &parent);
        restore();
        assert_eq!(out.code, 2, "{}", out.stdout);
        // The refusal is about WHERE, not about a directory that could not be made.
        assert!(
            out.stdout.contains("inside the repository"),
            "{}",
            out.stdout
        );
        assert!(
            !out.stdout.contains("creating a scratch directory"),
            "{}",
            out.stdout
        );
    }
}
