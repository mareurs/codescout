use super::*;
use crate::librarian::cutover::testkit::{add_doc, add_event, old_catalog_file};
use crate::librarian::derived::testkit::{repo_with, FILE, LINK};
use crate::librarian::frontmatter;
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

/// Run `f` to the end on a runtime of its own: the inventory tests are plain `#[test]`s.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

fn inventory(f: &Fixture, paths: &[&str], kind: &str, json: bool) -> Outcome {
    block_on(evaluate(
        &inventory_args(&f.root, paths, kind, json),
        &f.root,
    ))
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
        text.contains("docs/trackers/obs-log.md: 4 lines, 1 record, 1 section, 0 index tables."),
        "{text}"
    );
    assert!(text.contains("Status values: open (1)."), "{text}");
    assert!(
        text.contains("Non-markdown files that name a source: 3 hits in 3 files. git grep alone found 1 of them."),
        "{text}"
    );
    assert!(
        text.contains("Sidecar: docs/augmentations/docs-trackers-obs-log.yaml (it has a prompt)."),
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
    let out = block_on(evaluate(
        &inventory_args(&root, &["x.md"], "file", true),
        &root,
    ));
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

// ---- apply (Task 9) ----------------------------------------------------------------

const X: &str = "docs/trackers/x.md";
const SIDECAR: &str = "docs/augmentations/docs-trackers-x.yaml";
const F1_PATH: &str = "docs/trackers/ledgers/x/2026-08-01-first-thing.md";
const F2_PATH: &str = "docs/trackers/ledgers/x/archive/2026-08-02-second-thing.md";
const README: &str = "docs/trackers/ledgers/x/readme.md";
const MAPPING: &str = ".codescout/m.toml";

/// The fixture of the planner tests (`tracker_migration/plan/tests.rs`). Lines 1-3
/// frontmatter; 4-12 preamble and index (rows F-1 at 10, F-2 at 11); F-1 13-17; F-2 18-22.
const X_TRACKER: &str = "\
---
title: X log
---
# X log

Preamble cites F-2.

| ID | Status | Title | Opened |
|---|---|---|---|
| F-1 | open | First, see F-2 | 2026-08-01 |
| F-2 | fixed | Second | 2026-08-02 |

## F-1 — First thing

**Status:** open
See F-2 and [F-2](x.md).

## F-2 — Second thing

**Status:** fixed
**Closed:** 2026-09-01
Body two.
";

/// Lines 13-17 with the bare `F-2` rewritten (the link text is left), the blank line 17
/// dropped before the row line, and the row verbatim.
const F1_BODY: &str = "## F-1 — First thing\n\n**Status:** open\nSee [[x:F-2]] and [F-2](x.md).\n\n**Index row:** | F-1 | open | First, see F-2 | 2026-08-01 |";
/// Lines 18-22 (no token to rewrite), then the row.
const F2_BODY: &str = "## F-2 — Second thing\n\n**Status:** fixed\n**Closed:** 2026-09-01\nBody two.\n\n**Index row:** | F-2 | fixed | Second | 2026-08-02 |";
/// `readme_body`, then lines 4-12 with every bare F-1 and F-2 rewritten.
const README_BODY: &str = "Findings of x.\n\n# X log\n\nPreamble cites [[x:F-2]].\n\n| ID | Status | Title | Opened |\n|---|---|---|---|\n| [[x:F-1]] | open | First, see [[x:F-2]] | 2026-08-01 |\n| [[x:F-2]] | fixed | Second | 2026-08-02 |";

/// The planner tests' mapping. F-1 is open (live), F-2 is fixed (terminal, so archive/);
/// `created` comes from the Opened column; lines 4-12 go to the readme.
const BASE: &str = r#"
readme_body = "Findings of x."
entry_kind = "finding"
status_source = "body_label"
sidecar = "retire"

[[sources]]
path = "docs/trackers/x.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["finding"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
parked = "deferred"
fixed = "terminal"

[status_map]
open = "open"
fixed = "fixed"

[columns]
opened = "created"

[[sections]]
source = "docs/trackers/x.md"
start = 4
to = "readme"
"#;

/// [`BASE`] for a collection that exists: no schema and no readme part.
const EXISTING: &str = r#"
entry_kind = "finding"
status_source = "body_label"
sidecar = "retire"

[[sources]]
path = "docs/trackers/x.md"
kind = "file"

[target]
lifecycle = "ledgers"
existing = true

[status_map]
open = "open"
fixed = "fixed"

[columns]
opened = "created"

[[sections]]
source = "docs/trackers/x.md"
start = 4
to = "drop"
"#;

/// A record appended after the merge (lines 23-27 of the flat file).
const LATE: &str = "\n## F-3 — Third thing\n\n**Status:** open\nBody three.\n";

/// A repository with a working tree; `.codescout/` (the mapping, the lock) is ignored.
struct Work {
    _dir: tempfile::TempDir,
    root: PathBuf,
    repo: git2::Repository,
}

/// Noon UTC of the date, as seconds.
fn at(y: i32, m: u32, d: u32) -> i64 {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
}

fn write_files(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let abs = root.join(path);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(abs, text).unwrap();
    }
}

/// Stage every change of the working tree (deletions too) and commit it at `secs`.
fn commit_all(repo: &git2::Repository, secs: i64) {
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.update_all(["*"].iter(), None).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig =
        git2::Signature::new("Test", "test@example.invalid", &git2::Time::new(secs, 0)).unwrap();
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, "commit", &tree, &parents)
        .unwrap();
}

/// `files` and a `.gitignore`, committed on 2026-08-10, the working tree equal to HEAD.
fn work_repo(files: &[(&str, &str)]) -> Work {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let repo = git2::Repository::init(&root).unwrap();
    write_files(&root, &[(".gitignore", ".codescout/\n")]);
    write_files(&root, files);
    commit_all(&repo, at(2026, 8, 10));
    Work {
        _dir: dir,
        root,
        repo,
    }
}

/// The tracker, its sidecar and an unrelated tracker. No non-markdown file holds an `x`, so
/// the grep for the stem finds no code mention.
fn x_repo() -> Work {
    work_repo(&[
        (X, X_TRACKER),
        (SIDECAR, "prompt: Keep it short.\n"),
        ("docs/trackers/other.md", "# Other\n"),
    ])
}

fn test_env() -> ApplyEnv {
    ApplyEnv {
        librarian_db: None,
        data_dir: None,
        now: NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        lock_wait: Duration::from_millis(200),
    }
}

/// Write `mapping` to [`MAPPING`] and run apply from the root.
async fn apply_with(
    w: &Work,
    mapping: &str,
    json: bool,
    no_old_catalog: bool,
    env: &ApplyEnv,
) -> Outcome {
    write_files(&w.root, &[(MAPPING, mapping)]);
    let path = PathBuf::from(MAPPING);
    let req = ApplyRequest {
        mapping: &path,
        project: None,
        no_old_catalog,
        json,
    };
    apply_outcome(&req, &w.root, env).await
}

/// [`apply_with`] for a repository that never had an old catalog.
async fn run_apply(w: &Work, mapping: &str, json: bool) -> Outcome {
    apply_with(w, mapping, json, true, &test_env()).await
}

fn read(w: &Work, rel: &str) -> String {
    std::fs::read_to_string(w.root.join(rel)).unwrap()
}

fn exists(w: &Work, rel: &str) -> bool {
    w.root.join(rel).exists()
}

fn entry_body(text: &str) -> String {
    frontmatter::parse(text)
        .unwrap()
        .1
        .trim_matches('\n')
        .to_string()
}

fn json_of(out: &Outcome) -> Value {
    serde_json::from_str(out.stdout.trim()).unwrap_or_else(|e| panic!("{e}: {}", out.stdout))
}

/// `git status --porcelain=v1 --untracked-files=all`, one line per path, sorted.
fn git_changes(w: &Work) -> Vec<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&w.root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
        .unwrap();
    let mut lines: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

/// Every file of the working tree outside `.git` and `.codescout`, path to bytes.
fn tree_files(w: &Work) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(&w.root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != ".codescout")
    {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(&w.root).unwrap();
            out.insert(
                rel.to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            );
        }
    }
    out
}

fn no_root_in(w: &Work, out: &Outcome) {
    assert!(
        !out.stdout.contains(w.root.to_str().unwrap()),
        "the output names no absolute path:\n{}",
        out.stdout
    );
}

/// The written entries and readme hold the planner's bodies, F-2 is in `archive/`, and the
/// source and its sidecar are gone; git sees nothing else. Breaks if a write or a delete is
/// skipped, a body is not the planner's, or apply changes a path the plan does not list.
#[tokio::test]
async fn apply_writes_the_collection_and_deletes_the_source() {
    let w = x_repo();
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert_eq!(entry_body(&read(&w, F1_PATH)), F1_BODY);
    assert_eq!(entry_body(&read(&w, F2_PATH)), F2_BODY);
    assert_eq!(entry_body(&read(&w, README)), README_BODY);
    let f1 = frontmatter::parse(&read(&w, F1_PATH)).unwrap().0.unwrap();
    assert_eq!(f1.status.as_deref(), Some("open"));
    assert_eq!(f1.extra["created"], json!("2026-08-01"));
    assert_eq!(
        git_changes(&w),
        vec![
            " D docs/augmentations/docs-trackers-x.yaml",
            " D docs/trackers/x.md",
            "?? docs/trackers/ledgers/x/2026-08-01-first-thing.md",
            "?? docs/trackers/ledgers/x/archive/2026-08-02-second-thing.md",
            "?? docs/trackers/ledgers/x/readme.md",
        ]
    );
}

/// A second run finds every file as the first run left it: it writes and deletes nothing
/// and lists each planned path as already applied. Breaks if an equal target is written
/// again or refused, if the untracked entries of the first run count as key collisions, or
/// if a delete the first run did is not reported.
#[tokio::test]
async fn a_second_apply_reports_already_applied() {
    let w = x_repo();
    let first = run_apply(&w, BASE, true).await;
    assert_eq!(first.code, 0, "{}", first.stdout);
    let before = tree_files(&w);
    let out = run_apply(&w, BASE, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    let v = json_of(&out);
    assert_eq!(v["written"], json!([]));
    assert_eq!(v["deleted"], json!([]));
    assert_eq!(
        v["already_applied"],
        json!([SIDECAR, F1_PATH, F2_PATH, README, X])
    );
    assert_eq!(tree_files(&w), before);
}

/// A run that stopped after some writes (F-1 missing, the source and the sidecar still
/// there) is completed by the next run. Breaks if apply refuses a partial state, rewrites
/// what is already there, or skips the missing entry or the deletes.
#[tokio::test]
async fn apply_after_a_partial_run_completes() {
    let w = x_repo();
    assert_eq!(run_apply(&w, BASE, true).await.code, 0);
    std::fs::remove_file(w.root.join(F1_PATH)).unwrap();
    write_files(
        &w.root,
        &[(X, X_TRACKER), (SIDECAR, "prompt: Keep it short.\n")],
    );
    let out = run_apply(&w, BASE, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    let v = json_of(&out);
    assert_eq!(v["written"], json!([F1_PATH]));
    assert_eq!(v["deleted"], json!([SIDECAR, X]));
    assert_eq!(v["already_applied"], json!([F2_PATH, README]));
    assert_eq!(entry_body(&read(&w, F1_PATH)), F1_BODY);
    assert!(!exists(&w, X) && !exists(&w, SIDECAR));
}

/// A target that holds other text is a refusal that names it, and nothing is written or
/// deleted. The readme is the LAST path of the plan's writes, so this breaks if a refusal
/// is decided inside the write loop (both entries would already be on disk), if the other
/// text is overwritten, or if it is reported as already applied.
#[tokio::test]
async fn apply_refuses_a_target_that_holds_other_text() {
    let w = x_repo();
    write_files(&w.root, &[(README, "# Someone else's readme\n")]);
    let line = format!(
        "{README} exists and holds other text than apply would write. Move it aside, or make \
         it equal, then run this again."
    );
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout, format!("{line}\n"));
    assert!(!exists(&w, F1_PATH) && !exists(&w, F2_PATH));
    assert_eq!(read(&w, README), "# Someone else's readme\n");
    assert_eq!(read(&w, X), X_TRACKER);
    assert!(exists(&w, SIDECAR));
    let out = run_apply(&w, BASE, true).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(json_of(&out), json!({ "refused": [line] }));
}

/// An uncommitted change under `docs/trackers/` on a path the plan does not touch (one
/// modified, one untracked) is a refusal per path, in path order, before any write. Breaks
/// if the clean-tree check is skipped, accepts every change under `docs/trackers/`, or
/// misses an untracked file.
#[tokio::test]
async fn apply_refuses_a_dirty_unrelated_tracker() {
    let w = x_repo();
    write_files(
        &w.root,
        &[
            ("docs/trackers/other.md", "# Other\n\nEdited.\n"),
            ("docs/trackers/new.md", "# New\n"),
        ],
    );
    let before = tree_files(&w);
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(
        out.stdout,
        "docs/trackers/new.md has an uncommitted change, and this plan does not write or \
         delete it. Commit or restore it, then run this again.\n\
         docs/trackers/other.md has an uncommitted change, and this plan does not write or \
         delete it. Commit or restore it, then run this again.\n"
    );
    assert_eq!(tree_files(&w), before);
}

/// A key change rewrites `[[x:…]]` in `docs/notes.md`, outside `docs/trackers/`. While that
/// file has an uncommitted change apply refuses, so a restore of apply's list cannot destroy
/// it; once it is clean, apply rewrites it. Breaks if the clean-tree check skips the writes
/// outside `docs/trackers/`, or if the other-markdown rewrite is refused as "other text".
#[tokio::test]
async fn apply_refuses_a_dirty_file_it_would_rewrite_outside_docs_trackers() {
    let w = work_repo(&[
        (X, X_TRACKER),
        ("docs/notes.md", "See F-1 and [[x:F-1]].\n"),
    ]);
    let m = BASE.replace(
        "lifecycle = \"ledgers\"\n",
        "lifecycle = \"ledgers\"\nkey = \"y\"\n",
    );
    write_files(
        &w.root,
        &[("docs/notes.md", "See F-1 and [[x:F-1]], edited.\n")],
    );
    let before = tree_files(&w);
    let out = run_apply(&w, &m, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(
        out.stdout,
        "docs/notes.md has an uncommitted change, and apply would rewrite it. Commit or \
         restore it, then run this again.\n"
    );
    assert_eq!(tree_files(&w), before);

    write_files(&w.root, &[("docs/notes.md", "See F-1 and [[x:F-1]].\n")]);
    let out = run_apply(&w, &m, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert_eq!(read(&w, "docs/notes.md"), "See F-1 and [[y:F-1]].\n");
    let written = json_of(&out)["written"].clone();
    assert!(
        written
            .as_array()
            .unwrap()
            .contains(&json!("docs/notes.md")),
        "{written}"
    );
}

/// A second run after a key change finds `docs/notes.md` dirty (` M`), holding exactly the text
/// the plan writes: it is already applied, not a foreign edit. Breaks if the clean-tree check
/// refuses a dirty path in `writes` before comparing its text with the planned text.
#[tokio::test]
async fn a_second_apply_after_a_key_change_reports_the_rewritten_file_already_applied() {
    let w = work_repo(&[
        (X, X_TRACKER),
        ("docs/notes.md", "See F-1 and [[x:F-1]].\n"),
    ]);
    let m = BASE.replace(
        "lifecycle = \"ledgers\"\n",
        "lifecycle = \"ledgers\"\nkey = \"y\"\n",
    );
    let first = run_apply(&w, &m, true).await;
    assert_eq!(first.code, 0, "{}", first.stdout);
    assert_eq!(read(&w, "docs/notes.md"), "See F-1 and [[y:F-1]].\n");
    let out = run_apply(&w, &m, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    let v = json_of(&out);
    assert_eq!(v["written"], json!([]));
    assert_eq!(v["deleted"], json!([]));
    assert_eq!(
        v["already_applied"],
        json!([
            "docs/notes.md",
            "docs/trackers/ledgers/y/2026-08-01-first-thing.md",
            "docs/trackers/ledgers/y/archive/2026-08-02-second-thing.md",
            "docs/trackers/ledgers/y/readme.md",
            X
        ])
    );
}

/// A code mention classed `reader` is a refusal that names the file; `comment` is not.
/// Breaks if the reader check is skipped or blocks every class.
#[tokio::test]
async fn apply_refuses_a_reader() {
    let w = work_repo(&[
        (X, X_TRACKER),
        ("scripts/read.py", "open('docs/trackers/x.md')\n"),
    ]);
    let reader = format!("{BASE}\n[code_mentions]\n\"scripts/read.py\" = \"reader\"\n");
    let out = run_apply(&w, &reader, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(
        out.stdout,
        "scripts/read.py reads a source (code_mentions class reader). Switch that reader to \
         the collection first, then run this again.\n"
    );
    assert!(!exists(&w, F1_PATH) && exists(&w, X));

    let comment = format!("{BASE}\n[code_mentions]\n\"scripts/read.py\" = \"comment\"\n");
    let out = run_apply(&w, &comment, false).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
}

/// With no old catalog on the machine, apply refuses and says so, unless
/// `--no-old-catalog` says the repository never had one. Breaks if a missing catalog is
/// read as "no facts", if the flag is ignored, or if the text names the path looked at.
#[tokio::test]
async fn apply_without_an_old_catalog_refuses_unless_the_flag_is_given() {
    let w = x_repo();
    let none = tempfile::tempdir().unwrap();
    let env = ApplyEnv {
        librarian_db: Some(none.path().join("catalog.db").into()),
        data_dir: Some(none.path().to_path_buf()),
        ..test_env()
    };
    let out = apply_with(&w, BASE, false, false, &env).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout, format!("{NO_OLD_CATALOG}\n"));
    assert!(!out.stdout.contains(none.path().to_str().unwrap()));
    assert!(!exists(&w, F1_PATH) && exists(&w, X));

    let out = apply_with(&w, BASE, false, true, &env).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
}

/// An old catalog whose rows all lie under another repository's root is "no old catalog" for
/// this repository: a refusal that names `--no-old-catalog` (not the cutover's `--old-root`,
/// which apply does not take), and with the flag apply runs. Breaks if `NoOldRowsUnderRoot`
/// stops the run (exit 2), or if the flag is ignored because a catalog file exists.
#[tokio::test]
async fn an_old_catalog_with_rows_only_for_another_repository_counts_as_none() {
    let w = x_repo();
    let other = tempfile::tempdir().unwrap();
    let other_root = std::fs::canonicalize(other.path()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = std::fs::canonicalize(dir.path())
        .unwrap()
        .join("catalog.db");
    old_catalog_file(&db, |cat| {
        add_doc(cat, &other_root, "o1", "a.md");
    });
    let env = ApplyEnv {
        librarian_db: Some(db.into()),
        ..test_env()
    };
    let out = apply_with(&w, BASE, false, false, &env).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout, format!("{NO_OLD_ROWS}\n"));
    assert!(!exists(&w, F1_PATH) && exists(&w, X));

    let out = apply_with(&w, BASE, false, true, &env).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
}

/// An old catalog that still holds a fact of the source that the cutover writes into it (a
/// note on a `single_writer` document is spliced into the file) is a refusal; one that holds
/// only the document row is not. That source has no old link form (`[F-2](x.md)` is
/// replaced, and `title` gives way to `single_writer` on the same line), because the cutover
/// also rewrites an old link form, and that alone refuses: the last case. Breaks if the
/// cutover plan is not consulted, or if it is not scoped to the sources.
#[tokio::test]
async fn apply_refuses_a_source_with_catalog_only_facts() {
    let line = "docs/trackers/x.md: the identity cutover still has catalog facts or old \
                citation forms to move into this file. Run codescout cutover export, commit \
                the result, then run this again.\n";
    let single_writer = X_TRACKER
        .replace("[F-2](x.md)", "F-2 again")
        .replace("title: X log", "single_writer: true");
    let with_catalog = |w: &Work, event: bool| {
        let dir = tempfile::tempdir().unwrap();
        let db = std::fs::canonicalize(dir.path())
            .unwrap()
            .join("catalog.db");
        old_catalog_file(&db, |cat| {
            add_doc(cat, &w.root, "x1", X);
            if event {
                add_event(cat, "e1", "x1", "note", r#"{"text": "hello"}"#);
            }
        });
        let env = ApplyEnv {
            librarian_db: Some(db.into()),
            ..test_env()
        };
        (dir, env)
    };

    let w = work_repo(&[(X, &single_writer)]);
    let (_dir, env) = with_catalog(&w, true);
    let out = apply_with(&w, BASE, false, false, &env).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout, line);
    assert!(!exists(&w, F1_PATH) && exists(&w, X));

    let w = work_repo(&[(X, &single_writer)]);
    let (_dir, env) = with_catalog(&w, false);
    let out = apply_with(&w, BASE, false, false, &env).await;
    assert_eq!(out.code, 0, "{}", out.stdout);

    let w = work_repo(&[(X, X_TRACKER)]);
    let (_dir, env) = with_catalog(&w, false);
    let out = apply_with(&w, BASE, false, false, &env).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout, line);
}

/// The repository after a first apply, committed on 2026-09-01.
async fn migrated() -> Work {
    let w = x_repo();
    let out = run_apply(&w, BASE, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    commit_all(&w.repo, at(2026, 9, 1));
    w
}

/// The flat file comes back with F-3 appended, in a merge committed on 2026-09-05. Apply in
/// existing mode writes F-3 only (its lines were last changed on 2026-09-05, so that is its
/// created date), leaves F-1 as it is, and deletes the file again. Breaks if existing
/// entries are not read from the working tree (F-1 and F-2 would be planned again and
/// collide), or if apply reads anything but HEAD for the source.
#[tokio::test]
async fn a_late_record_after_a_committed_merge_is_added_in_existing_mode() {
    let w = migrated().await;
    let f1_before = read(&w, F1_PATH);
    write_files(&w.root, &[(X, &format!("{X_TRACKER}{LATE}"))]);
    commit_all(&w.repo, at(2026, 9, 5));
    let out = run_apply(&w, EXISTING, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    let f3 = "docs/trackers/ledgers/x/2026-09-05-third-thing.md";
    let v = json_of(&out);
    assert_eq!(v["written"], json!([f3]));
    assert_eq!(v["deleted"], json!([X]));
    assert_eq!(v["already_applied"], json!([]));
    assert_eq!(
        entry_body(&read(&w, f3)),
        "## F-3 — Third thing\n\n**Status:** open\nBody three."
    );
    assert_eq!(read(&w, F1_PATH), f1_before);
    assert!(!exists(&w, X));
}

/// In the merged file F-2's body changed: apply refuses with the planner's line and writes
/// nothing, not even F-3. Breaks if the planner's problems do not stop the writes.
#[tokio::test]
async fn a_late_merge_that_changes_a_record_is_refused() {
    let w = migrated().await;
    let merged = format!("{X_TRACKER}{LATE}").replace("Body two.", "Body two, edited.");
    write_files(&w.root, &[(X, &merged)]);
    commit_all(&w.repo, at(2026, 9, 5));
    let out = run_apply(&w, EXISTING, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(
        out.stdout,
        format!(
            "F-2: entry {F2_PATH} exists and its body is not the body of the record at line \
             18 of docs/trackers/x.md. Make the two equal, or remove the entry.\n"
        )
    );
    assert!(!exists(
        &w,
        "docs/trackers/ledgers/x/2026-09-05-third-thing.md"
    ));
    assert!(exists(&w, X));
}

/// The JSON lists every written, deleted and already-applied path, repo-relative and in
/// path order, and the planner's review; the text gives one line per group with counts.
/// Breaks if a path is absolute, a group is missing or out of order, or the review is not
/// the plan's.
#[tokio::test]
async fn apply_output_lists_exact_paths() {
    let w = x_repo();
    let out = run_apply(&w, BASE, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    no_root_in(&w, &out);
    assert_eq!(
        json_of(&out),
        json!({
            "written": [F1_PATH, F2_PATH, README],
            "deleted": [SIDECAR, X],
            "already_applied": [],
            "review": {
                "derived_dates": ["F-1: status_since", "F-2: status_since"],
                "dropped": [],
                "skipped_rewrites": [
                    "F-1: 1 rewritten, skipped 1 in link text, 0 in URLs, 0 next to ( or !"
                ],
                "row_only": [],
                "no_row": [],
                "disagreements": [],
                "path_mentions": [],
                "time_based": []
            }
        })
    );

    let w = x_repo();
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    no_root_in(&w, &out);
    assert_eq!(
        out.stdout,
        format!(
            "Written: 3 files: {F1_PATH}, {F2_PATH}, {README}.\n\
             Deleted: 2 files: {SIDECAR}, {X}.\n\
             Already applied: 0 files.\n\
             Review: 2 derived dates, 0 dropped sections, 1 record with skipped rewrites, \
             0 row-only records, 0 records with no index row, 0 status disagreements.\n"
        )
    );
}

/// A shallow clone is a refusal before any write: blame dates would be the boundary date.
/// Breaks if the shallow check is skipped.
#[tokio::test]
async fn apply_refuses_a_shallow_clone() {
    let w = x_repo();
    let head = w.repo.head().unwrap().target().unwrap();
    std::fs::write(w.repo.path().join("shallow"), format!("{head}\n")).unwrap();
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout, format!("{SHALLOW}\n"));
    assert!(!exists(&w, F1_PATH) && exists(&w, X));
}

/// While another process holds the project write lock, apply does not run (exit 2, one
/// line naming the holder) and writes nothing; once it is free, apply runs. Breaks if apply
/// does not take the lock.
#[tokio::test]
async fn apply_does_not_run_while_the_write_lock_is_held() {
    let w = x_repo();
    let held = take_at(
        &lock_root_for(&w.root, &w.root),
        Duration::from_secs(1),
        "test holder",
    )
    .await
    .unwrap();
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 2, "{}", out.stdout);
    assert_eq!(out.stdout.lines().count(), 1, "{}", out.stdout);
    assert!(
        out.stdout
            .starts_with("The command did not run: the project write lock is not free:"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("test holder"), "{}", out.stdout);
    assert!(!exists(&w, F1_PATH) && exists(&w, X));
    drop(held);
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
}

/// A section with no `sections` entry is reported by `mapping_problems` and by the planner
/// in the same words; apply prints it once. Breaks if the two lists are joined without the
/// order-preserving dedup.
#[tokio::test]
async fn a_problem_both_checks_find_is_printed_once() {
    let w = x_repo();
    let m = BASE.replace(
        "[[sections]]\nsource = \"docs/trackers/x.md\"\nstart = 4\nto = \"readme\"\n",
        "",
    );
    let out = run_apply(&w, &m, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    let line = "section at line 4 of docs/trackers/x.md (\"# X log\") has no sections entry";
    assert_eq!(
        out.stdout.lines().filter(|l| *l == line).count(),
        1,
        "{}",
        out.stdout
    );
    assert_eq!(out.stdout.lines().next(), Some(line), "{}", out.stdout);
    assert!(!exists(&w, F1_PATH) && exists(&w, X));
}

/// With `status_source = { index_column = "Status" }` the inventory reads the status from
/// the index, as the planner does, so a record with no `**Status:**` line has a status.
/// Breaks if the inventory reads body labels (it would ask for `status_none`).
#[tokio::test]
async fn the_inventory_reads_the_status_column_the_mapping_names() {
    let tracker = "# X log\n\n| ID | Status | Title |\n|---|---|---|\n| F-1 | open | First |\n\n## F-1 — First thing\n\nBody one.\n";
    let w = work_repo(&[(X, tracker)]);
    let m = r#"
readme_body = "Findings of x."
entry_kind = "finding"
status_source = { index_column = "Status" }

[[sources]]
path = "docs/trackers/x.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["finding"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
fixed = "terminal"

[status_map]
open = "open"

[[sections]]
source = "docs/trackers/x.md"
start = 1
to = "readme"
"#;
    let out = run_apply(&w, m, true).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    // No date in the record or its row: created is the blame date of its lines.
    assert_eq!(
        json_of(&out)["written"],
        json!(["docs/trackers/ledgers/x/2026-08-10-first-thing.md", README])
    );
}

/// From a project below the git root (`sub/` holds `.codescout/project.toml`), apply locks
/// at `sub/`, reads the mapping's repo-relative paths at the git root, and blames them
/// there. Breaks if a path is taken relative to the project, or the lock is taken at the
/// git root.
#[tokio::test]
async fn apply_from_a_project_below_the_git_root_reads_repo_relative_paths() {
    let w = x_repo();
    write_files(
        &w.root,
        &[
            ("sub/.codescout/project.toml", "[project]\nname = \"sub\"\n"),
            (MAPPING, BASE),
        ],
    );
    let sub = w.root.join("sub");
    let mapping = PathBuf::from("../.codescout/m.toml");
    let req = ApplyRequest {
        mapping: &mapping,
        project: None,
        no_old_catalog: true,
        json: true,
    };
    let out = apply_outcome(&req, &sub, &test_env()).await;
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert_eq!(json_of(&out)["written"], json!([F1_PATH, F2_PATH, README]));
    assert_eq!(
        frontmatter::parse(&read(&w, F1_PATH))
            .unwrap()
            .0
            .unwrap()
            .extra["created"],
        json!("2026-08-01")
    );
    assert!(sub.join(".codescout/write.lock").exists());
    assert!(!w.root.join(".codescout/write.lock").exists());
}

/// A source whose working-tree text differs from HEAD would be deleted with work git cannot
/// restore: apply refuses. Breaks if a delete does not compare the file with HEAD.
#[tokio::test]
async fn apply_refuses_a_source_with_uncommitted_changes() {
    let w = x_repo();
    write_files(&w.root, &[(X, &format!("{X_TRACKER}{LATE}"))]);
    let before = tree_files(&w);
    let out = run_apply(&w, BASE, false).await;
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(
        out.stdout,
        "docs/trackers/x.md differs from its text at HEAD, and apply would delete it. Commit \
         it first (apply reads sources at HEAD), then run this again.\n"
    );
    assert_eq!(tree_files(&w), before);
}
