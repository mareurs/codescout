//! End to end: the built `codescout` binary's `cutover plan`, `export` and `build`.
//!
//! The logic of each step is covered in `src/librarian/cutover/` and `src/cli/cutover.rs`. This
//! drives one repository and one old catalog through the whole sequence an operator runs, and
//! checks what only the binary does: that the subcommands are wired into `main`, that the old
//! catalog is never written, that the second `export` and the plan after the commit find nothing
//! left to do, and that `build` reads the committed text.
//!
//! The repository and the old catalog live in temporary directories. The old catalog's rows lie
//! under a root that does not exist (`/nonexistent/old-root`), so `--old-root` is what maps them
//! to the repository. Nothing here touches a real project or a real catalog.
//!
//! `CARGO_BIN_EXE_codescout` makes Cargo build the binary first (see
//! `tests/cross_process_write_lock.rs`), so there is no missing-binary skip branch.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use codescout::librarian::catalog::artifact::{self, ArtifactRow};
use codescout::librarian::catalog::augmentation::{self, AugmentationRow};
use codescout::librarian::catalog::events::{self, EventRow};
use codescout::librarian::catalog::links::{self, LinkRow};
use codescout::librarian::catalog::observations::{self, ObservationRow};
use codescout::librarian::catalog::Catalog;
use codescout::librarian::ids::artifact_id_from_abs;
use sha2::{Digest, Sha256};

const OLD_ROOT: &str = "/nonexistent/old-root";

fn git(dir: &Path, args: &[&str]) {
    git_stdout(dir, args);
}

/// Run `git` in `dir` with the one environment every call here shares, require success, and
/// return what it printed. Global and system config are off. The user's global ignore file
/// (`~/.config/git/ignore`) is not a config entry: `make_repo` sets a local `core.excludesFile`
/// to stop git from reading it.
fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// What `docs/shared.md` cites, and `docs/old/plan-x.md` holds: the id the old catalog gave
/// `plan-x.md` when it lay under `docs/old/`. No old row holds it; history does.
fn plan_x_id() -> String {
    artifact_id_from_abs(&Path::new(OLD_ROOT).join("docs/old/plan-x.md"))
}

const PLAN_X: &str = "# Plan X\n\nThe plan text. It is the same in both commits.\n";

/// The repository: commit 1 holds `docs/old/plan-x.md`; commit 2 renames it and adds the rest,
/// with the `.gitignore` line `build` needs for the checkout's catalog.
///
/// `core.excludesFile` is set to an empty file inside the git directory. Git and libgit2 read
/// `~/.config/git/ignore` when it is unset, so without this the `.gitignore` line could be
/// shadowed by the machine's own rules, and the `status` check below would prove nothing.
fn make_repo(root: &Path) {
    git(root, &["init", "-q", "."]);
    let excludes = root.join(".git/empty-excludes");
    std::fs::write(&excludes, "").unwrap();
    git(
        root,
        &["config", "core.excludesFile", excludes.to_str().unwrap()],
    );
    write(root, "docs/old/plan-x.md", PLAN_X);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "one"]);

    std::fs::remove_file(root.join("docs/old/plan-x.md")).unwrap();
    write(root, "docs/plan-x.md", PLAN_X);
    write(
        root,
        "docs/ledger.md",
        "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n## F-1 — first\n\nbody one\n",
    );
    write(
        root,
        "docs/log.md",
        "---\nsingle_writer: true\n---\n\n# Log\n",
    );
    write(
        root,
        "docs/shared.md",
        &format!(
            "# Shared\n\nThe old form ledger:F-1 names an entry.\n\nA [notes](My_Notes.md) link and \
             a wiki link [[My_Notes]].\n\nThe plan {} and the bare token F-7.\n",
            plan_x_id()
        ),
    );
    write(root, "docs/My_Notes.md", "# My notes\n\nSome text.\n");
    write(root, "docs/a/one.md", "# One\n\n## F-7 — seven\n\nx\n");
    write(root, "docs/b/two.md", "# Two\n\n## F-7 — seven\n\ny\n");
    write(
        root,
        ".gitignore",
        "/.codescout/derived.db*\n/.codescout/write.lock*\n",
    );
    write(root, "src/render_prompt.md", "# Prompt\n\nText.\n");
    write(
        root,
        "src/lib.rs",
        "pub const PROMPT: &str = include_str!(\"./render_prompt.md\");\n",
    );
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "two"]);
}

fn row(path: &str) -> ArtifactRow {
    let abs = Path::new(OLD_ROOT).join(path);
    ArtifactRow {
        id: artifact_id_from_abs(&abs),
        abs_path: abs,
        kind: "doc".to_owned(),
        status: "active".to_owned(),
        title: None,
        owners: vec![],
        tags: vec![],
        topic: None,
        time_scope: None,
        source: None,
        created_at: 0,
        updated_at: 0,
        file_mtime: 0,
        file_sha256: String::new(),
        confidence: 1.0,
    }
}

fn id_of(path: &str) -> String {
    row(path).id
}

fn note(id: &str, path: &str, text: &str) -> EventRow {
    EventRow {
        id: id.to_owned(),
        artifact_id: id_of(path),
        kind: "note".to_owned(),
        payload: serde_json::json!({ "text": text }).to_string(),
        anchor_commit: None,
        head_commit: None,
        author: None,
        created_at: 1_000,
    }
}

/// The old catalog, made through the real catalog code, closed, and left as one plain file.
fn make_old_catalog(db: &Path) {
    {
        let cat = Catalog::open(db).unwrap();
        for path in [
            "docs/ledger.md",
            "docs/log.md",
            "docs/shared.md",
            // The file the cutover renames: the row lets a run after the commit see the rename.
            "docs/My_Notes.md",
        ] {
            artifact::upsert(&cat, &row(path)).unwrap();
        }
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: id_of("docs/ledger.md"),
                prompt: "keep it current".to_owned(),
                params: r#"{"entries":[{"id":"F-1"},{"id":"F-2","title":"second"}],"gather_from":"somewhere"}"#
                    .to_owned(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("entries".to_owned()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        // The note and the observation cite old forms: the export writes them rewritten (D-R9).
        events::insert(
            &cat,
            &note("ev-log", "docs/log.md", "decided to wait for ledger:F-1"),
        )
        .unwrap();
        events::insert(
            &cat,
            &note("ev-shared", "docs/shared.md", "a note nobody holds"),
        )
        .unwrap();
        observations::insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: id_of("docs/log.md"),
                text: "seen once, in a meeting; see [notes](My_Notes.md)".to_owned(),
                source: None,
                created_at: 2_000,
            },
        )
        .unwrap();
        links::insert(
            &cat,
            &LinkRow {
                src_id: id_of("docs/log.md"),
                dst_id: id_of("docs/ledger.md"),
                rel: "depends_on".to_owned(),
                created_at: 3_000,
            },
        )
        .unwrap();
    }
    // A catalog that left a side file is not the state an operator starts from.
    let side = |suffix: &str| {
        let mut name = db.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    };
    if side("-wal").exists() || side("-shm").exists() {
        let conn = rusqlite::Connection::open(db).unwrap();
        conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
    }
    assert!(
        !side("-wal").exists() && !side("-shm").exists() && !side("-journal").exists(),
        "the fixture catalog is one plain file"
    );
}

fn cutover(root: &Path, db: &Path, action: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codescout"))
        .arg("cutover")
        .arg(action)
        .arg("--project")
        .arg(root)
        .arg("--old-root")
        .arg(OLD_ROOT)
        .arg("--old-db")
        .arg(db)
        .arg("--json")
        .env_remove("LIBRARIAN_DB")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap()
}

/// Run `action`, require `code`, and parse the one JSON object it printed.
fn run(root: &Path, db: &Path, action: &str, code: i32) -> serde_json::Value {
    let out = cutover(root, db, action);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(
        out.status.code(),
        Some(code),
        "cutover {action}: {text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !text.contains(root.to_str().unwrap())
            && !text.contains(OLD_ROOT)
            && !text.contains(db.parent().unwrap().to_str().unwrap()),
        "an absolute path in the output of {action}:\n{text}"
    );
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("{action}: {e}\n{text}"))
}

/// `class -> [moved, already_exported, skipped, unrecoverable]`, every class the tally names.
fn tally(report: &serde_json::Value) -> BTreeMap<String, [u64; 4]> {
    report["tally"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(class, t)| {
            let n = |k: &str| t[k].as_u64().unwrap();
            (
                class.clone(),
                [
                    n("moved"),
                    n("already_exported"),
                    n("skipped"),
                    n("unrecoverable"),
                ],
            )
        })
        .collect()
}

fn expected(rows: &[(&str, [u64; 4])]) -> BTreeMap<String, [u64; 4]> {
    rows.iter().map(|(c, n)| ((*c).to_owned(), *n)).collect()
}

/// The items of one class and status.
fn items<'a>(
    report: &'a serde_json::Value,
    class: &str,
    status: &str,
) -> Vec<&'a serde_json::Value> {
    report["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["class"] == class && i["status"] == status)
        .collect()
}

fn sha256_of(path: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
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
fn the_cutover_exports_every_class_and_leaves_the_old_catalog_unchanged() {
    let repo = tempfile::Builder::new()
        .prefix("cutover-proj-")
        .tempdir()
        .unwrap();
    let root = std::fs::canonicalize(repo.path()).unwrap();
    make_repo(&root);
    let catalog_dir = tempfile::tempdir().unwrap();
    let catalog_dir = std::fs::canonicalize(catalog_dir.path()).unwrap();
    let db = catalog_dir.join("catalog.db");
    make_old_catalog(&db);
    let (db_sha, dir_before) = (sha256_of(&db), listing(&catalog_dir));

    // ---- plan: the dry run writes nothing ----
    let planned = run(&root, &db, "plan", 0);
    assert_eq!(planned["phase"], "plan");
    assert_eq!(listing(&catalog_dir), dir_before, "plan keeps no file");
    assert!(
        !root.join("docs/my-notes.md").exists(),
        "plan writes nothing"
    );
    assert!(root.join("docs/My_Notes.md").exists());

    // ---- export ----
    let exported = run(&root, &db, "export", 0);
    assert_eq!(exported["phase"], "export");
    assert_eq!(
        tally(&planned),
        tally(&exported),
        "the plan and the export tell one tally"
    );
    let first = expected(&[
        ("params_entry", [1, 1, 0, 0]),
        ("params_other", [0, 0, 0, 1]),
        ("augmentation_shape", [1, 0, 0, 0]),
        ("event", [1, 0, 1, 0]),
        ("observation", [1, 0, 0, 0]),
        ("manual_link", [1, 0, 0, 0]),
        ("rename", [1, 0, 0, 1]),
        ("citation", [6, 0, 0, 1]),
    ]);
    assert_eq!(tally(&exported), first, "{exported:#}");

    // The files.
    let ledger = read(&root, "docs/ledger.md");
    assert!(ledger.contains("## F-2 — "), "{ledger}");
    assert!(ledger.contains("expects_augmentation:"), "{ledger}");
    assert!(root.join("docs/augmentations/docs-ledger.yaml").is_file());
    let log = read(&root, "docs/log.md");
    assert!(log.contains("ev-log"), "the note section: {log}");
    assert!(
        log.contains("observation-1"),
        "the observation section: {log}"
    );
    assert!(log.contains("**Depends on:** [[ledger]]"), "{log}");
    assert!(
        log.contains("> decided to wait for [[ledger:F-1]]\n"),
        "{log}"
    );
    assert!(
        log.contains("> seen once, in a meeting; see [[my-notes|notes]]\n"),
        "{log}"
    );
    assert!(root.join("docs/my-notes.md").is_file());
    assert!(!root.join("docs/My_Notes.md").exists());
    let shared = read(&root, "docs/shared.md");
    for want in [
        "[[ledger:F-1]]",
        "[[my-notes|notes]]",
        "[[my-notes]]",
        "[[plan-x]]",
        "the bare token F-7.",
    ] {
        assert!(shared.contains(want), "{want}: {shared}");
    }
    assert_eq!(
        read(&root, "src/render_prompt.md"),
        "# Prompt\n\nText.\n",
        "the file code reads is unchanged"
    );

    // What was not moved, and why.
    let other = items(&exported, "params_other", "unrecoverable");
    assert_eq!(other.len(), 1);
    assert_eq!(other[0]["detail"], "gather_from");
    let skipped = items(&exported, "event", "skipped");
    assert_eq!(
        skipped[0]["reason"],
        "no committed home yet: not a single-writer document"
    );
    let frozen = items(&exported, "rename", "unrecoverable");
    assert_eq!(frozen[0]["path"], "src/render_prompt.md");
    let reason = frozen[0]["reason"].as_str().unwrap();
    assert!(reason.starts_with("frozen:"), "{reason}");
    assert!(reason.contains("src/lib.rs:1"), "{reason}");
    let bare = items(&exported, "citation", "unrecoverable");
    assert_eq!(bare[0]["path"], "docs/shared.md");
    assert_eq!(
        bare[0]["reason"], "ambiguous: several documents qualify",
        "F-7 is defined in two documents"
    );
    assert!(
        bare[0]["detail"].as_str().unwrap().contains("F-7"),
        "{:?}",
        bare[0]
    );

    // The old catalog is unchanged, and the only new files beside it are the two reports.
    assert_eq!(sha256_of(&db), db_sha, "the old catalog is unchanged");
    let mut with_reports = dir_before.clone();
    with_reports.push(exported["report"]["markdown"].as_str().unwrap().to_owned());
    with_reports.push(exported["report"]["json"].as_str().unwrap().to_owned());
    with_reports.sort();
    assert_eq!(listing(&catalog_dir), with_reports);

    // ---- a second export finds every file done ----
    let again = run(&root, &db, "export", 0);
    let apply = &again["apply"];
    assert_eq!(apply["written"], serde_json::json!([]), "{again:#}");
    assert_eq!(apply["renamed"], serde_json::json!([]));
    assert_eq!(apply["refused"], serde_json::json!([]));
    assert!(!apply["unchanged"].as_array().unwrap().is_empty());
    assert_eq!(
        apply["unchanged"].as_array().unwrap().len(),
        again["files"].as_array().unwrap().len(),
        "every planned file is reported unchanged"
    );
    assert_eq!(read(&root, "docs/shared.md"), shared);

    // `build` before the commit: a plan of HEAD still writes.
    let refused = run(&root, &db, "build", 2);
    assert_eq!(refused["ran"], false);
    assert!(
        refused["reason"]
            .as_str()
            .unwrap()
            .starts_with("the export is not committed: "),
        "{refused}"
    );
    assert!(!root.join(".codescout/derived.db").exists());

    // ---- commit, then plan again ----
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "export"]);
    let after = run(&root, &db, "plan", 0);
    assert_eq!(
        after["files"],
        serde_json::json!([]),
        "a plan of the committed export writes nothing"
    );
    assert_eq!(
        tally(&after),
        expected(&[
            ("params_entry", [0, 2, 0, 0]),
            ("params_other", [0, 0, 0, 1]),
            ("augmentation_shape", [0, 1, 0, 0]),
            ("event", [0, 1, 1, 0]),
            ("observation", [0, 1, 0, 0]),
            ("manual_link", [0, 1, 0, 0]),
            ("rename", [0, 1, 0, 1]),
            ("citation", [0, 0, 0, 1]),
        ]),
        "{after:#}"
    );

    // ---- build ----
    let built = run(&root, &db, "build", 0);
    assert_eq!(built["ran"], true, "{built}");
    assert_eq!(built["catalog"], ".codescout/derived.db");
    assert!(
        built.get("location").is_none(),
        "the output names no location: {built}"
    );
    let derived = root.join(".codescout/derived.db");
    assert!(derived.is_file());
    assert_eq!(
        git_stdout(&root, &["status", "--porcelain"]),
        "",
        "git ignores the built catalog and its side files"
    );
    assert_eq!(
        git_stdout(&root, &["ls-files", ".codescout"]),
        "",
        "git tracks nothing in .codescout: the lock files and the catalog stay out"
    );
    assert_eq!(
        sha256_of(&db),
        db_sha,
        "build leaves the old catalog unchanged"
    );
    // Beside the old catalog: what it began with and the reports of the two exports. The new
    // catalog is in the checkout, so a build that wrote beside the old one would show here.
    let mut with_new = dir_before.clone();
    for report in [&exported, &again] {
        with_new.push(report["report"]["markdown"].as_str().unwrap().to_owned());
        with_new.push(report["report"]["json"].as_str().unwrap().to_owned());
    }
    with_new.sort();
    assert_eq!(listing(&catalog_dir), with_new);
    assert_eq!(
        built["built"]["problems"], 1,
        "the one problem is the frozen file's key: {built}"
    );

    let conn = rusqlite::Connection::open(&derived).unwrap();
    let problems: Vec<(String, Option<String>, String)> = conn
        .prepare("SELECT kind, \"key\", rel_paths FROM problem ORDER BY rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        problems,
        [(
            "invalid_key".to_owned(),
            Some("render_prompt".to_owned()),
            r#"["src/render_prompt.md"]"#.to_owned()
        )],
        "the frozen file keeps its invalid key, and nothing else is a problem"
    );
    let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(
        one("SELECT count(*) FROM entry WHERE rel_path='docs/ledger.md' AND token='F-2'"),
        1,
        "the entry the export wrote is derived"
    );
    assert_eq!(
        one(
            "SELECT count(*) FROM edge WHERE src_rel_path='docs/shared.md' AND kind='entry' \
             AND dst_key='ledger' AND target_token='F-1'"
        ),
        1,
        "shared.md cites F-1 of the ledger"
    );
    assert_eq!(
        one(
            "SELECT count(*) FROM edge WHERE src_rel_path='docs/log.md' AND kind='file' \
             AND dst_key='ledger'"
        ),
        1,
        "log.md depends on the ledger"
    );
    assert_eq!(
        one("SELECT count(*) FROM edge WHERE src_rel_path='docs/shared.md' AND kind='malformed'"),
        0,
        "no citation of shared.md is malformed"
    );
}
