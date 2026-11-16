//! End to end: the built `codescout` binary's `trackers inventory`, `apply` and `check` on the
//! tracker shapes that real repositories hold (plan L-E1 Task 11).
//!
//! The logic of each step is covered in `src/librarian/tracker_migration/` and
//! `src/cli/trackers/tests.rs`. Each test here builds a temporary git repository, commits fixture
//! trackers on 2026-08-10, writes a mapping into the ignored `.codescout/` folder, and runs the
//! binary with `--json`, as the skill does. It checks what only the binary run shows: that the
//! subcommands are wired into `main`, the exit codes (0 done or clean, 1 refused or not clean),
//! the JSON shapes, and the files on disk and in `git status`.
//!
//! `LIBRARIAN_DB` names a file that does not exist and every apply passes `--no-old-catalog`, so
//! no run reads the catalog of this machine. `CARGO_BIN_EXE_codescout` makes Cargo build the binary
//! first, so there is no missing-binary skip branch.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

/// The mapping path, relative to the repository root. `.codescout/` is in `.gitignore`.
const MAPPING: &str = ".codescout/m.toml";

/// A repository with one commit, and a catalog path that does not exist.
struct Repo {
    _dir: tempfile::TempDir,
    _db_dir: tempfile::TempDir,
    root: PathBuf,
    no_db: PathBuf,
}

/// `git` in `dir` with the user's configuration shut out. Returns stdout.
fn git(dir: &Path, args: &[&str]) -> String {
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
        .env("GIT_AUTHOR_DATE", "2026-08-10T12:00:00+00:00")
        .env("GIT_COMMITTER_DATE", "2026-08-10T12:00:00+00:00")
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

fn read(r: &Repo, rel: &str) -> String {
    std::fs::read_to_string(r.root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// `files` and a `.gitignore` for `.codescout/`, committed on 2026-08-10 (so every blame date is
/// 2026-08-10); the working tree equals HEAD.
fn repo(files: &[(&str, &str)]) -> Repo {
    let dir = tempfile::Builder::new()
        .prefix("trackers-proj-")
        .tempdir()
        .unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let db_dir = tempfile::tempdir().unwrap();
    let no_db = db_dir.path().join("catalog.db");
    git(&root, &["init", "-q", "."]);
    write(&root, ".gitignore", ".codescout/\n");
    for (rel, text) in files {
        write(&root, rel, text);
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "trackers"]);
    Repo {
        _dir: dir,
        _db_dir: db_dir,
        root,
        no_db,
    }
}

/// Run `codescout trackers <args> --json` from the repository root. Returns the exit code and the
/// one JSON object printed. The output must name no absolute path.
fn trackers(r: &Repo, args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_codescout"))
        .current_dir(&r.root)
        .arg("trackers")
        .args(args)
        .arg("--json")
        .env("LIBRARIAN_DB", &r.no_db)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        !text.contains(r.root.to_str().unwrap()),
        "an absolute path in the output of {args:?}:\n{text}"
    );
    let code = out.status.code().expect("the binary exited by a signal");
    let v = serde_json::from_str(text.trim()).unwrap_or_else(|e| {
        panic!(
            "{args:?}: {e}\nstdout: {text}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (code, v)
}

/// `inventory <sources>`, which must exit 0.
fn inventory(r: &Repo, sources: &[&str]) -> Value {
    let mut args = vec!["inventory"];
    args.extend_from_slice(sources);
    let (code, v) = trackers(r, &args);
    assert_eq!(code, 0, "inventory: {v}");
    v
}

/// Write `mapping` to [`MAPPING`] and run `apply` (this repository never had an old catalog).
fn apply(r: &Repo, mapping: &str) -> (i32, Value) {
    write(&r.root, MAPPING, mapping);
    trackers(r, &["apply", MAPPING, "--no-old-catalog"])
}

/// Write `mapping` to [`MAPPING`] and run `check`.
fn check(r: &Repo, mapping: &str) -> (i32, Value) {
    write(&r.root, MAPPING, mapping);
    trackers(r, &["check", MAPPING])
}

/// `check` must exit 0 and report clean, with no blocking line.
fn assert_clean(r: &Repo, mapping: &str) {
    let (code, v) = check(r, mapping);
    assert_eq!(code, 0, "check: {v}");
    assert_eq!(v["clean"], json!(true), "{v}");
    assert_eq!(v["blocking"], json!([]), "{v}");
}

/// The strings of a JSON array.
fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap_or_else(|| panic!("not an array: {v}"))
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

/// `git status --porcelain=v1 --untracked-files=all`, one line per path, sorted.
fn changes(r: &Repo) -> Vec<String> {
    let mut lines: Vec<String> = git(
        &r.root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
    .lines()
    .map(str::to_owned)
    .collect();
    lines.sort();
    lines
}

/// The entries among `written`: every path but the readme and other markdown outside `prefix`.
fn entries(written: &[String], prefix: &str) -> Vec<String> {
    written
        .iter()
        .filter(|p| p.starts_with(prefix) && !p.ends_with("/readme.md"))
        .cloned()
        .collect()
}

/// The record tokens of every source of an inventory, in source order.
fn record_tokens(inv: &Value) -> Vec<String> {
    inv["sources"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["records"].as_array().unwrap().iter())
        .map(|r| r["token"].as_str().unwrap().to_string())
        .collect()
}

// ---- a session log: frictions, wins and an index table ------------------------------------

const SESSION_LOG_PATH: &str = "docs/trackers/session-log.md";

/// Lines 1-3 frontmatter; 4-7 the preamble; 8-15 the index (rows at 12-14); F-1 16-20; F-2
/// 21-26; W-1 27-30.
const SESSION_LOG: &str = "\
---
title: Session log
---
# Session log

Frictions and wins of the agent sessions.

## Index

| ID | Status | Title | Date |
|---|---|---|---|
| F-1 | open | Slow grep | 2026-07-01 |
| F-2 | fixed | Lost buffer | 2026-07-03 |
| W-1 | noted | Fast symbols | 2026-07-05 |

## F-1 — Slow grep

**Status:** open
The grep over the tree took long. W-1 was the fix path.

## F-2 — Lost buffer

**Status:** fixed
**Closed:** 2026-07-10
A buffer was lost after F-1.

## W-1 — Fast symbols

**Status:** noted
Symbols answered at once.
";

/// A markdown file outside the tracker that cites one of its records.
const NOTES: &str = "# Notes\n\nSee [[session-log:F-1]].\n";

/// Frictions and wins by token prefix; the Date column gives `created`; the preamble and the
/// index go to the readme. `fixed` and `noted` are terminal, so F-2 and W-1 go to `archive/`.
const SESSION_MAPPING: &str = r#"
readme_body = "Frictions and wins of the agent sessions."
status_source = "body_label"

[entry_kind]
F = "friction"
W = "win"

[[sources]]
path = "docs/trackers/session-log.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["friction", "win"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
fixed = "terminal"
noted = "terminal"

[status_map]
open = "open"
fixed = "fixed"
noted = "noted"

[columns]
date = "created"

[[sections]]
source = "docs/trackers/session-log.md"
start = 4
to = "readme"

[[sections]]
source = "docs/trackers/session-log.md"
start = 8
to = "readme"
"#;

const S_F1: &str = "docs/trackers/ledgers/session-log/2026-07-01-slow-grep.md";
const S_F2: &str = "docs/trackers/ledgers/session-log/archive/2026-07-03-lost-buffer.md";
const S_W1: &str = "docs/trackers/ledgers/session-log/archive/2026-07-05-fast-symbols.md";
const S_README: &str = "docs/trackers/ledgers/session-log/readme.md";

#[test]
fn trackers_help_names_no_unbuilt_subcommand() {
    let out = Command::new(env!("CARGO_BIN_EXE_codescout"))
        .args(["trackers", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "trackers --help failed:\n{text}");
    assert!(!text.contains("not built yet"), "stale help text:\n{text}");
    assert!(text.contains("apply"), "help omits apply:\n{text}");
    assert!(text.contains("check"), "help omits check:\n{text}");
}

/// The skill's loop on a session log: inventory, a mapping, apply, check. Breaks if the
/// inventory misses a record, the index or a section; if apply writes an entry per index row or
/// per section instead of per record, misses a record, files a terminal entry live, changes a
/// path it does not list, or drops the index row; or if check does not pass what apply wrote.
#[test]
fn a_session_log_with_frictions_wins_and_an_index_round_trips() {
    let r = repo(&[(SESSION_LOG_PATH, SESSION_LOG), ("docs/notes.md", NOTES)]);

    let inv = inventory(&r, &[SESSION_LOG_PATH]);
    let src = &inv["sources"][0];
    assert_eq!(record_tokens(&inv), ["F-1", "F-2", "W-1"], "{inv}");
    assert_eq!(src["line_count"], json!(30));
    let starts: Vec<&Value> = src["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["start"])
        .collect();
    assert_eq!(starts, [&json!(4), &json!(8)], "{inv}");
    assert_eq!(src["tables"][0]["section_start"], json!(8));
    assert_eq!(src["tables"][0]["rows"], json!(3));
    assert_eq!(
        src["status_values"],
        json!({"fixed": 1, "noted": 1, "open": 1})
    );
    assert_eq!(src["possibles"], json!([]));
    assert_eq!(src["row_only"], json!([]));
    assert_eq!(src["no_row"], json!([]));
    assert_eq!(
        inv["mentions"]["citations"],
        json!([{"file": "docs/notes.md", "line": 3, "text": "[[session-log:F-1]]"}])
    );

    let (code, out) = apply(&r, SESSION_MAPPING);
    assert_eq!(code, 0, "apply: {out}");
    let written = strs(&out["written"]);
    assert_eq!(written, [S_F1, S_F2, S_W1, S_README], "{out}");
    assert_eq!(strs(&out["deleted"]), [SESSION_LOG_PATH]);
    assert_eq!(out["already_applied"], json!([]));

    // One entry per record: not per index row plus heading, not per section.
    let collection = "docs/trackers/ledgers/session-log/";
    assert_eq!(
        entries(&written, collection).len(),
        record_tokens(&inv).len()
    );

    // git sees exactly the listed paths, and nothing else (notes.md keeps its citation: the key
    // is reused).
    let mut expected: Vec<String> = written.iter().map(|p| format!("?? {p}")).collect();
    expected.push(format!(" D {SESSION_LOG_PATH}"));
    expected.sort();
    assert_eq!(changes(&r), expected);
    assert_eq!(read(&r, "docs/notes.md"), NOTES);

    let f1 = read(&r, S_F1);
    assert!(f1.contains("\nentry_kind: friction\n"), "{f1}");
    assert!(f1.contains("\ncreated: 2026-07-01\n"), "{f1}");
    assert!(f1.contains("\nstatus: open\n"), "{f1}");
    assert!(
        f1.contains(
            "## F-1 — Slow grep\n\n**Status:** open\n\
             The grep over the tree took long. [[session-log:W-1]] was the fix path.\n\n\
             **Index row:** | F-1 | open | Slow grep | 2026-07-01 |"
        ),
        "{f1}"
    );
    let w1 = read(&r, S_W1);
    assert!(w1.contains("\nentry_kind: win\n"), "{w1}");
    assert!(w1.contains("\nstatus: noted\n"), "{w1}");
    let f2 = read(&r, S_F2);
    assert!(f2.contains("\nclosed: 2026-07-10\n"), "{f2}");
    assert!(
        f2.contains("A buffer was lost after [[session-log:F-1]]."),
        "{f2}"
    );
    let readme = read(&r, S_README);
    assert!(
        readme.contains("| [[session-log:W-1]] | noted | Fast symbols | 2026-07-05 |"),
        "{readme}"
    );

    let before = changes(&r);
    assert_clean(&r, SESSION_MAPPING);
    assert_eq!(changes(&r), before, "check writes nothing");
}

// ---- an archived split file folds into the live tracker's collection ----------------------

const REVIEW_LOG_PATH: &str = "docs/trackers/review-log.md";
const REVIEW_ARCHIVE_PATH: &str = "docs/trackers/review-log-archive.md";

/// The live tracker: R-57 cites R-55, which only the archive file defines.
const REVIEW_LOG: &str = "\
# Review log

Findings of the code reviews. Older findings are in review-log-archive.md.

## R-57 — Lock dropped early

**Status:** open
The guard is dropped at once. It follows R-55.

## R-58 — Missing test

**Status:** open
No test pins the order.
";

/// The split archive file: `## R-56b` (line 13) is a heading with a suffixed token, so it is a
/// possible record, not a definition.
const REVIEW_ARCHIVE: &str = "\
# Review log archive

## R-55 — Old finding

**Status:** fixed
Fixed long ago.

## R-56 — Split finding

**Status:** fixed
The first half.

## R-56b — Hit: late split

**Status:** fixed
The second half, split from R-56.
";

/// Both files into one collection keyed by the first source. No `records_extra`.
const REVIEW_MAPPING: &str = r#"
readme_body = "Findings of the code reviews."
entry_kind = "finding"
status_source = "body_label"

[[sources]]
path = "docs/trackers/review-log.md"
kind = "file"

[[sources]]
path = "docs/trackers/review-log-archive.md"
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
fixed = "fixed"

[[sections]]
source = "docs/trackers/review-log.md"
start = 1
to = "readme"

[[sections]]
source = "docs/trackers/review-log-archive.md"
start = 1
to = "drop"
"#;

/// The user names the `R-56b` heading a record.
const R56B_EXTRA: &str = r#"
[[records_extra]]
source = "docs/trackers/review-log-archive.md"
line = 13
token = "R-56b"
"#;

/// Two sources, one collection. Without `records_extra` the `R-56b` heading blocks; with it,
/// R-56b is an entry of the same collection, a token of one source cited in the other is
/// rewritten to the collection key, and both sources are deleted. Breaks if a second source
/// gets a collection of its own, if `records_extra` is ignored (R-56b would stay in the dropped
/// section or block), or if a source is left on disk.
#[test]
fn the_archived_split_file_folds_into_its_collection() {
    let r = repo(&[
        (REVIEW_LOG_PATH, REVIEW_LOG),
        (REVIEW_ARCHIVE_PATH, REVIEW_ARCHIVE),
    ]);

    let inv = inventory(&r, &[REVIEW_LOG_PATH, REVIEW_ARCHIVE_PATH]);
    assert_eq!(
        record_tokens(&inv),
        ["R-57", "R-58", "R-55", "R-56"],
        "{inv}"
    );
    assert_eq!(
        inv["sources"][1]["possibles"],
        json!([{"line": 13, "token": "R-56b", "enclosing": null}])
    );

    let (code, out) = apply(&r, REVIEW_MAPPING);
    assert_eq!(code, 1, "apply without records_extra: {out}");
    let refused = strs(&out["refused"]);
    assert!(
        refused.contains(
            &"possible record R-56b at line 13 of docs/trackers/review-log-archive.md is in a \
              section. Add it to records_extra, or to sections with confirm = true."
                .to_string()
        ),
        "{out}"
    );
    assert_eq!(
        changes(&r),
        Vec::<String>::new(),
        "a refusal writes nothing"
    );

    let mapping = format!("{REVIEW_MAPPING}{R56B_EXTRA}");
    let (code, out) = apply(&r, &mapping);
    assert_eq!(code, 0, "apply: {out}");
    let c = "docs/trackers/ledgers/review-log";
    let written = strs(&out["written"]);
    assert_eq!(
        written,
        [
            format!("{c}/2026-08-10-lock-dropped-early.md"),
            format!("{c}/2026-08-10-missing-test.md"),
            format!("{c}/archive/2026-08-10-hit-late-split.md"),
            format!("{c}/archive/2026-08-10-old-finding.md"),
            format!("{c}/archive/2026-08-10-split-finding.md"),
            format!("{c}/readme.md"),
        ],
        "{out}"
    );
    assert_eq!(
        strs(&out["deleted"]),
        [REVIEW_ARCHIVE_PATH, REVIEW_LOG_PATH]
    );
    // Four definitions and the one records_extra heading.
    assert_eq!(entries(&written, c).len(), record_tokens(&inv).len() + 1);

    let late = read(&r, &format!("{c}/archive/2026-08-10-hit-late-split.md"));
    assert!(
        late.contains(
            "## R-56b — Hit: late split\n\n**Status:** fixed\n\
             The second half, split from [[review-log:R-56]]."
        ),
        "{late}"
    );
    let r57 = read(&r, &format!("{c}/2026-08-10-lock-dropped-early.md"));
    assert!(r57.contains("It follows [[review-log:R-55]]."), "{r57}");
    assert!(
        strs(&out["review"]["dropped"]).contains(
            &"docs/trackers/review-log-archive.md lines 1-2 (\"# Review log archive\")".to_string()
        ),
        "{out}"
    );

    assert_clean(&r, &mapping);
}

// ---- a repeated token as a subsection of its own record ------------------------------------

const AUDIT_LOG_PATH: &str = "docs/trackers/prompt-audit-log.md";

/// The `prompt-hamsa-audit-log` `A-38` shape: `### A-38 — Follow-up` (line 13) repeats the token
/// of the record that holds it.
const AUDIT_LOG: &str = "\
# Prompt audit log

## A-37 — Tool list too long

**Status:** done
Cut the list.

## A-38 — Hint names a missing tool

**Status:** open
The hint names a tool that was removed.

### A-38 — Follow-up

The same hint came back after the rebase.

## A-39 — Guide overlaps the hint

**Status:** open
See A-38.
";

const AUDIT_MAPPING: &str = r#"
readme_body = "Audits of the prompts."
entry_kind = "finding"
status_source = "body_label"

[[sources]]
path = "docs/trackers/prompt-audit-log.md"
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
done = "terminal"

[status_map]
open = "open"
done = "done"

[[sections]]
source = "docs/trackers/prompt-audit-log.md"
start = 1
to = "drop"
"#;

/// The nested repeat is reported as nested and stays inside A-38's entry: three records, three
/// entries, and the follow-up only in A-38. Breaks if the repeat is refused, split into an entry
/// of its own, cut from A-38 (check would find the gap), or copied into a second entry.
#[test]
fn a_repeated_subsection_token_stays_inside_its_record() {
    let r = repo(&[(AUDIT_LOG_PATH, AUDIT_LOG)]);

    let inv = inventory(&r, &[AUDIT_LOG_PATH]);
    assert_eq!(record_tokens(&inv), ["A-37", "A-38", "A-39"], "{inv}");
    assert_eq!(
        inv["sources"][0]["repeats"],
        json!([{"token": "A-38", "first_line": 8, "other_line": 13, "nested": true}])
    );
    assert_eq!(inv["sources"][0]["records"][1]["last"], json!(16));

    let (code, out) = apply(&r, AUDIT_MAPPING);
    assert_eq!(code, 0, "apply: {out}");
    let c = "docs/trackers/ledgers/prompt-audit-log";
    let written = strs(&out["written"]);
    let entry_paths = entries(&written, c);
    assert_eq!(entry_paths.len(), 3, "{out}");

    let a38_path = format!("{c}/2026-08-10-hint-names-a-missing-tool.md");
    assert!(entry_paths.contains(&a38_path), "{out}");
    let a38 = read(&r, &a38_path);
    assert!(
        a38.contains(
            "## A-38 — Hint names a missing tool\n\n**Status:** open\n\
             The hint names a tool that was removed.\n\n\
             ### A-38 — Follow-up\n\n\
             The same hint came back after the rebase.\n"
        ),
        "{a38}"
    );
    for p in entry_paths.iter().filter(|p| **p != a38_path) {
        let text = read(&r, p);
        assert!(!text.contains("Follow-up"), "{p}:\n{text}");
    }
    let a39 = read(&r, &format!("{c}/2026-08-10-guide-overlaps-the-hint.md"));
    assert!(a39.contains("See [[prompt-audit-log:A-38]]."), "{a39}");

    assert_clean(&r, AUDIT_MAPPING);
}

// ---- a possible record nested in a record --------------------------------------------------

const SRI_LOG_PATH: &str = "docs/trackers/sri-log.md";

/// `### T-15a` (line 8) lies inside SRI-15 (lines 3-11). Its token has a suffix, so it is not a
/// definition: a possible record.
const SRI_LOG: &str = "\
# Search ranking issues

## SRI-15 — Exact match ranked low

**Status:** open
An exact symbol match ranks below a fuzzy one.

### T-15a — Reproduce on the fixture

Run the query on the small fixture first.

## SRI-16 — Stale index

**Status:** open
The index lags one commit.
";

const SRI_MAPPING: &str = r#"
readme_body = "Search ranking issues."
entry_kind = "issue"
status_source = "body_label"

[[sources]]
path = "docs/trackers/sri-log.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["issue"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
fixed = "terminal"

[status_map]
open = "open"

[[sections]]
source = "docs/trackers/sri-log.md"
start = 1
to = "readme"
"#;

/// Settlement one: T-15a stays inside SRI-15.
const T15A_INSIDE: &str = r#"
[[possibles_inside]]
source = "docs/trackers/sri-log.md"
line = 8
"#;

/// Settlement two: T-15a is a record of its own. It has no `**Status:**` line, so it needs one.
const T15A_RECORD: &str = r#"
[[records_extra]]
source = "docs/trackers/sri-log.md"
line = 8
token = "T-15a"

[overrides.T-15a]
status = "open"
"#;

const SRI15: &str = "docs/trackers/ledgers/sri-log/2026-08-10-exact-match-ranked-low.md";
const SRI16: &str = "docs/trackers/ledgers/sri-log/2026-08-10-stale-index.md";
const T15A: &str = "docs/trackers/ledgers/sri-log/2026-08-10-reproduce-on-the-fixture.md";
const SRI_README: &str = "docs/trackers/ledgers/sri-log/readme.md";

/// Until the mapping settles T-15a, apply refuses with exactly that line and writes nothing, and
/// check is not clean. Either settlement then applies and checks clean: `possibles_inside` keeps
/// it in SRI-15's entry, `records_extra` makes it an entry of its own and takes it out of SRI-15.
/// Breaks if the nested possible record is carried silently (either way), or if a settlement is
/// ignored.
#[test]
fn a_nested_possible_record_blocks_until_settled() {
    let r = repo(&[(SRI_LOG_PATH, SRI_LOG)]);

    let inv = inventory(&r, &[SRI_LOG_PATH]);
    assert_eq!(record_tokens(&inv), ["SRI-15", "SRI-16"], "{inv}");
    assert_eq!(
        inv["sources"][0]["possibles"],
        json!([{"line": 8, "token": "T-15a", "enclosing": "SRI-15"}])
    );

    let blocked = "possible record T-15a at line 8 of docs/trackers/sri-log.md is inside record \
                   SRI-15. Add it to records_extra or possibles_inside.";
    let (code, out) = apply(&r, SRI_MAPPING);
    assert_eq!(code, 1, "apply: {out}");
    assert_eq!(out, json!({ "refused": [blocked] }));
    assert_eq!(
        changes(&r),
        Vec::<String>::new(),
        "a refusal writes nothing"
    );

    let (code, out) = check(&r, SRI_MAPPING);
    assert_eq!(code, 1, "check: {out}");
    assert_eq!(out["clean"], json!(false));
    assert!(
        strs(&out["blocking"]).contains(&format!(
            "The plan from HEAD has a problem, so apply refuses it: {blocked}"
        )),
        "{out}"
    );

    // Settlement one: inside SRI-15.
    let inside = format!("{SRI_MAPPING}{T15A_INSIDE}");
    let (code, out) = apply(&r, &inside);
    assert_eq!(code, 0, "apply: {out}");
    assert_eq!(strs(&out["written"]), [SRI15, SRI16, SRI_README], "{out}");
    let sri15 = read(&r, SRI15);
    assert!(
        sri15.contains(
            "An exact symbol match ranks below a fuzzy one.\n\n\
             ### T-15a — Reproduce on the fixture\n\n\
             Run the query on the small fixture first.\n"
        ),
        "{sri15}"
    );
    assert_clean(&r, &inside);

    // Settlement two, on a fresh copy: an entry of its own.
    let r = repo(&[(SRI_LOG_PATH, SRI_LOG)]);
    let record = format!("{SRI_MAPPING}{T15A_RECORD}");
    let (code, out) = apply(&r, &record);
    assert_eq!(code, 0, "apply: {out}");
    assert_eq!(
        strs(&out["written"]),
        [SRI15, T15A, SRI16, SRI_README],
        "{out}"
    );
    let t15a = read(&r, T15A);
    assert!(
        t15a.contains(
            "### T-15a — Reproduce on the fixture\n\nRun the query on the small fixture first."
        ),
        "{t15a}"
    );
    assert!(t15a.contains("\nstatus: open\n"), "{t15a}");
    let sri15 = read(&r, SRI15);
    assert!(!sri15.contains("T-15a"), "{sri15}");
    assert_clean(&r, &record);
}

// ---- a JSON file that reads the tracker ----------------------------------------------------

const EVAL_LOG_PATH: &str = "docs/trackers/eval-log.md";

const EVAL_LOG: &str = "\
# Eval log

## E-1 — Baseline run

**Status:** open
The first baseline.
";

/// A dashboard configuration that reads the tracker. `CodeRefs` reads JSON as data, so only the
/// plain grep of the inventory finds it.
const DASHBOARD: &str = "\
{
  \"panels\": [
    { \"title\": \"Evals\", \"source\": \"docs/trackers/eval-log.md\" }
  ]
}
";

const EVAL_MAPPING: &str = r#"
readme_body = "Eval runs."
entry_kind = "run"
status_source = "body_label"

[[sources]]
path = "docs/trackers/eval-log.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["run"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
done = "terminal"

[status_map]
open = "open"

[[sections]]
source = "docs/trackers/eval-log.md"
start = 1
to = "readme"
"#;

/// The inventory lists the JSON file as a grep hit. Apply refuses while it has no class and while
/// it is classed `reader`, writing nothing either time; classed `fixture` the same mapping
/// applies and checks clean, so the class is what blocked. Breaks if the inventory's grep skips
/// JSON, or if apply goes ahead with an unclassed or a reader mention.
#[test]
fn a_json_reader_blocks_apply() {
    let r = repo(&[
        (EVAL_LOG_PATH, EVAL_LOG),
        ("config/dashboard.json", DASHBOARD),
    ]);

    let inv = inventory(&r, &[EVAL_LOG_PATH]);
    assert_eq!(
        inv["mentions"]["code"],
        json!([{"file": "config/dashboard.json", "line": 3, "origin": "grep"}])
    );

    let (code, out) = apply(&r, EVAL_MAPPING);
    assert_eq!(code, 1, "apply: {out}");
    assert_eq!(
        out,
        json!({"refused": ["code mention config/dashboard.json has no class in code_mentions"]})
    );
    assert_eq!(
        changes(&r),
        Vec::<String>::new(),
        "a refusal writes nothing"
    );

    let reader =
        format!("{EVAL_MAPPING}\n[code_mentions]\n\"config/dashboard.json\" = \"reader\"\n");
    let (code, out) = apply(&r, &reader);
    assert_eq!(code, 1, "apply: {out}");
    assert_eq!(
        out,
        json!({"refused": [
            "config/dashboard.json reads a source (code_mentions class reader). Switch that \
             reader to the collection first, then run this again."
        ]})
    );
    assert_eq!(
        changes(&r),
        Vec::<String>::new(),
        "a refusal writes nothing"
    );

    let fixture =
        format!("{EVAL_MAPPING}\n[code_mentions]\n\"config/dashboard.json\" = \"fixture\"\n");
    let (code, out) = apply(&r, &fixture);
    assert_eq!(code, 0, "apply: {out}");
    assert_eq!(strs(&out["deleted"]), [EVAL_LOG_PATH]);
    assert_eq!(read(&r, "config/dashboard.json"), DASHBOARD);
    assert_clean(&r, &fixture);
}

// ---- a re-run after an interrupted apply ---------------------------------------------------

/// [`SESSION_MAPPING`] with a new key, so apply also rewrites `docs/notes.md` (outside
/// `docs/trackers/`), the first path it writes.
fn renamed_mapping() -> String {
    SESSION_MAPPING.replace(
        "lifecycle = \"ledgers\"\n",
        "lifecycle = \"ledgers\"\nkey = \"sessions\"\n",
    )
}

/// Every file under the root but `.git` and `.codescout`, path to bytes.
fn tree(r: &Repo) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![r.root.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let e = e.unwrap();
            let name = e.file_name();
            if name == ".git" || name == ".codescout" {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            } else {
                let rel = e.path().strip_prefix(&r.root).unwrap().to_owned();
                out.insert(
                    rel.to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                );
            }
        }
    }
    out
}

/// An apply stopped after its first two writes (it writes in path order and deletes last): the
/// rewritten `docs/notes.md` and the F-1 entry are on disk, the other writes are not, the source
/// is still there. Check is not clean. A re-run with the same mapping writes the rest, lists the
/// two finished writes as already applied (the dirty `docs/notes.md` holds the planned text, the
/// Task 9 fix 1 rule), deletes the source, and leaves the tree byte-equal to an uninterrupted
/// run; check is then clean. Breaks if a re-run refuses a previous run's output, writes a
/// finished file again, or ends with a different tree.
#[test]
fn a_rerun_after_an_interrupted_apply_completes() {
    let mapping = renamed_mapping();
    let c = "docs/trackers/ledgers/sessions";
    let (f1, f2, w1, readme) = (
        format!("{c}/2026-07-01-slow-grep.md"),
        format!("{c}/archive/2026-07-03-lost-buffer.md"),
        format!("{c}/archive/2026-07-05-fast-symbols.md"),
        format!("{c}/readme.md"),
    );

    // The uninterrupted run, for the expected end state.
    let whole = repo(&[(SESSION_LOG_PATH, SESSION_LOG), ("docs/notes.md", NOTES)]);
    let (code, out) = apply(&whole, &mapping);
    assert_eq!(code, 0, "apply: {out}");
    assert_eq!(
        strs(&out["written"]),
        [
            "docs/notes.md",
            f1.as_str(),
            f2.as_str(),
            w1.as_str(),
            readme.as_str()
        ],
        "{out}"
    );
    assert_eq!(
        read(&whole, "docs/notes.md"),
        "# Notes\n\nSee [[sessions:F-1]].\n"
    );
    let end_tree = tree(&whole);
    let end_changes = changes(&whole);

    // The interrupted run: the same first run, then its later writes and its delete undone.
    let r = repo(&[(SESSION_LOG_PATH, SESSION_LOG), ("docs/notes.md", NOTES)]);
    let (code, out) = apply(&r, &mapping);
    assert_eq!(code, 0, "apply: {out}");
    git(&r.root, &["checkout", "HEAD", "--", SESSION_LOG_PATH]);
    for p in [&f2, &w1, &readme] {
        std::fs::remove_file(r.root.join(p)).unwrap();
    }
    assert_eq!(
        changes(&r),
        [" M docs/notes.md".to_string(), format!("?? {f1}")]
    );

    let (code, out) = check(&r, &mapping);
    assert_eq!(code, 1, "check after the interruption: {out}");
    assert!(
        strs(&out["blocking"]).contains(
            &"docs/trackers/session-log.md is still on disk. Apply deletes every source."
                .to_string()
        ),
        "{out}"
    );

    let (code, out) = apply(&r, &mapping);
    assert_eq!(code, 0, "the re-run: {out}");
    assert_eq!(
        strs(&out["written"]),
        [&f2, &w1, &readme].map(String::as_str),
        "{out}"
    );
    assert_eq!(strs(&out["deleted"]), [SESSION_LOG_PATH], "{out}");
    assert_eq!(
        strs(&out["already_applied"]),
        ["docs/notes.md", f1.as_str()],
        "{out}"
    );
    assert_eq!(changes(&r), end_changes);
    assert_eq!(tree(&r), end_tree);
    assert_clean(&r, &mapping);

    // A third run finds nothing to do.
    let (code, out) = apply(&r, &mapping);
    assert_eq!(code, 0, "the third run: {out}");
    assert_eq!(out["written"], json!([]), "{out}");
    assert_eq!(out["deleted"], json!([]), "{out}");
    let listed: BTreeSet<String> = strs(&out["already_applied"]).into_iter().collect();
    for p in [
        "docs/notes.md",
        f1.as_str(),
        f2.as_str(),
        w1.as_str(),
        readme.as_str(),
        SESSION_LOG_PATH,
    ] {
        assert!(listed.contains(p), "{p} not already applied: {out}");
    }
}

// ---- final review: a `files` source whose files carry frontmatter -------------------------

/// Two cluster files with the frontmatter that every real `issue-clusters/IC-*.md` carries.
const IC1_PATH: &str = "docs/trackers/clusters/IC-1-blast-radius.md";
const IC1: &str = "\
---
kind: tracker
status: active
title: 'blast radius exceeds visibility'
owners:
- marius
tags:
- defect-classes
- blast-radius
topic: issue clusters
---

## IC-1 — blast radius exceeds visibility

Body line one.
";
const IC2_PATH: &str = "docs/trackers/clusters/IC-2-gate.md";
const IC2: &str = "\
---
kind: tracker
status: active
title: 'gate keyed on unobservable event'
tags:
- defect-classes
---

## IC-2 — gate keyed on unobservable event

Body two.
";

const CLUSTERS_MAPPING: &str = r#"
readme_body = "Issue clusters."
entry_kind = "cluster"
status_source = { constant = "open" }
hot_view_none = "clusters are reference material"

[[sources]]
path = "docs/trackers/clusters"
kind = "files"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["cluster"]
initial_status = "open"

[schema.statuses]
open = "open"
closed = "terminal"
"#;

/// Final review C1 (scenario A). The entry frontmatter replaces the source frontmatter, so
/// inventory, apply and check each list every source key that no entry keeps, with its value,
/// and the check stays clean. Breaks if a key is dropped silently on any of the three, `kind`,
/// `status` and `title` included.
#[test]
fn a_files_source_lists_the_frontmatter_keys_it_drops() {
    let r = repo(&[(IC1_PATH, IC1), (IC2_PATH, IC2)]);
    let (code, inv) = trackers(
        &r,
        &["inventory", "docs/trackers/clusters", "--kind", "files"],
    );
    assert_eq!(code, 0, "inventory: {inv}");
    assert_eq!(
        inv["sources"][0]["dropped_frontmatter"],
        json!([
            {"key": "kind", "value": "tracker"},
            {"key": "owners", "value": ["marius"]},
            {"key": "status", "value": "active"},
            {"key": "tags", "value": ["defect-classes", "blast-radius"]},
            {"key": "title", "value": "blast radius exceeds visibility"},
            {"key": "topic", "value": "issue clusters"},
        ]),
        "{inv}"
    );
    assert_eq!(
        inv["sources"][1]["dropped_frontmatter"],
        json!([
            {"key": "kind", "value": "tracker"},
            {"key": "status", "value": "active"},
            {"key": "tags", "value": ["defect-classes"]},
            {"key": "title", "value": "gate keyed on unobservable event"},
        ]),
        "{inv}"
    );

    let dropped = [
        format!("{IC1_PATH} frontmatter kind: \"tracker\""),
        format!("{IC1_PATH} frontmatter owners: [\"marius\"]"),
        format!("{IC1_PATH} frontmatter status: \"active\""),
        format!("{IC1_PATH} frontmatter tags: [\"defect-classes\",\"blast-radius\"]"),
        format!("{IC1_PATH} frontmatter title: \"blast radius exceeds visibility\""),
        format!("{IC1_PATH} frontmatter topic: \"issue clusters\""),
        format!("{IC2_PATH} frontmatter kind: \"tracker\""),
        format!("{IC2_PATH} frontmatter status: \"active\""),
        format!("{IC2_PATH} frontmatter tags: [\"defect-classes\"]"),
        format!("{IC2_PATH} frontmatter title: \"gate keyed on unobservable event\""),
    ];
    let (code, out) = apply(&r, CLUSTERS_MAPPING);
    assert_eq!(code, 0, "apply: {out}");
    assert_eq!(strs(&out["review"]["dropped"]), dropped, "{out}");

    let (code, out) = check(&r, CLUSTERS_MAPPING);
    assert_eq!(code, 0, "check: {out}");
    assert_eq!(out["clean"], json!(true), "{out}");
    assert_eq!(strs(&out["review"]["dropped"]), dropped, "{out}");
}

// ---- final review: two folded sources with an index section at the same line --------------

const REV_PATH: &str = "docs/trackers/rev.md";
const REV: &str = "\
# Rev

## Index

| Token | Status |
|---|---|
| F-1 | open |
| F-2 | open |

## F-2 — Two

**Status:** open
Two.
";
const REV_ARCHIVE_PATH: &str = "docs/trackers/rev-archive.md";
const REV_ARCHIVE: &str = "\
# Rev archive

## Index

| Token | Status |
|---|---|
| F-1 | fixed |

## F-1 — One

**Status:** open
One.
";

/// The scenario E mapping, with `{index_table}` in its first line.
const REV_MAPPING: &str = r#"{index_table}
entry_kind = "finding"
status_source = "body_label"
hot_view_none = "test"
row_title_column = "Status"

[[sources]]
path = "docs/trackers/rev.md"
kind = "file"

[[sources]]
path = "docs/trackers/rev-archive.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["finding"]
initial_status = "open"

[schema.statuses]
open = "open"
fixed = "terminal"

[status_map]
open = "open"

[[sections]]
source = "docs/trackers/rev.md"
start = 1
to = "readme"

[[sections]]
source = "docs/trackers/rev.md"
start = 3
to = "readme"

[[sections]]
source = "docs/trackers/rev-archive.md"
start = 1
to = "readme"

[[sections]]
source = "docs/trackers/rev-archive.md"
start = 3
to = "readme"
"#;

/// Final review I1 (scenario E). Both sources have `## Index` at line 3 and F-1 has a row in
/// each. With no `index_table` or with the bare `3`, apply refuses and names the section line
/// of each table; with the table form it picks the row of the named source, and check is clean.
/// Breaks if no value settles the token, if the bare line picks a silent winner, or if the
/// refusal names the line of the table header.
#[test]
fn index_table_can_name_one_of_two_sources_with_a_table_at_one_line() {
    let r = repo(&[(REV_PATH, REV), (REV_ARCHIVE_PATH, REV_ARCHIVE)]);
    for index_table in ["", "index_table = 3"] {
        let (code, out) = apply(&r, &REV_MAPPING.replace("{index_table}", index_table));
        assert_eq!(code, 1, "{index_table:?}: {out}");
        assert!(
            strs(&out["refused"]).contains(
                &"token F-1 has rows in the index tables of the sections at line 3 of \
                  docs/trackers/rev.md and line 3 of docs/trackers/rev-archive.md. Set \
                  index_table to { source = \"<path>\", start = <line> }: the source and the \
                  first line of the section that holds the table to use."
                    .to_string()
            ),
            "{index_table:?}: {out}"
        );
    }
    let mapping = REV_MAPPING.replace(
        "{index_table}",
        "index_table = { source = \"docs/trackers/rev-archive.md\", start = 3 }",
    );
    let (code, out) = apply(&r, &mapping);
    assert_eq!(code, 0, "apply: {out}");
    let one = read(&r, "docs/trackers/ledgers/rev/2026-08-10-one.md");
    assert!(
        one.contains("**Status:** open\nOne.\n\n**Index row:** | F-1 | fixed |"),
        "{one}"
    );
    assert_clean(&r, &mapping);
}

// ---- final review: an archived target with a record that is not terminal ------------------

const OLD_PATH: &str = "docs/trackers/old.md";
const OLD: &str = "\
# Old

## F-1 — First

**Status:** open
One.

## F-2 — Second

**Status:** fixed
**Closed:** 2026-08-05
Two.
";

const OLD_MAPPING: &str = r#"
entry_kind = "friction"
status_source = "body_label"
hot_view_none = "archive"

[[sources]]
path = "docs/trackers/old.md"
kind = "file"

[target]
lifecycle = "ledgers"
archived = true

[schema]
entry_kinds = ["friction"]
initial_status = "open"

[schema.statuses]
open = "open"
fixed = "terminal"

[status_map]
open = "open"
fixed = "fixed"

[[sections]]
source = "docs/trackers/old.md"
start = 1
to = "readme"
"#;

/// Final review I2 (scenario F). An archived target holds only terminal entries, so apply
/// refuses the open record before any write, naming the keys that settle it, and the source
/// stays. With an override to a terminal status, apply writes and check is clean. Breaks if
/// apply writes an invalid collection and deletes the source.
#[test]
fn an_archived_target_refuses_a_record_that_is_not_terminal() {
    let r = repo(&[(OLD_PATH, OLD)]);
    let (code, out) = apply(&r, OLD_MAPPING);
    assert_eq!(code, 1, "apply: {out}");
    assert_eq!(
        strs(&out["refused"]),
        [
            "F-1: target.archived is true, and the status \"open\" is not terminal. Map it to a \
          terminal status in status_map, or set overrides.F-1.status."
        ],
        "{out}"
    );
    assert_eq!(
        changes(&r),
        Vec::<String>::new(),
        "a refusal writes nothing"
    );

    let mapping = format!("{OLD_MAPPING}\n[overrides.F-1]\nstatus = \"fixed\"\n");
    let (code, out) = apply(&r, &mapping);
    assert_eq!(code, 0, "apply: {out}");
    assert_clean(&r, &mapping);
}

/// Final review M-a: the skill keeps the mapping outside the tracked tree, and a relative path
/// to it can climb with `..`. Apply and check read it; the read door still judges the path.
/// Breaks if a `..` path is refused as a link out of its folder.
#[test]
fn a_mapping_path_with_dot_dot_is_read() {
    let r = repo(&[(OLD_PATH, OLD)]);
    let outside = tempfile::Builder::new()
        .prefix("trackers-map-")
        .tempdir()
        .unwrap();
    let outside_root = std::fs::canonicalize(outside.path()).unwrap();
    assert_eq!(outside_root.parent(), r.root.parent(), "sibling folders");
    let name = outside_root.file_name().unwrap().to_str().unwrap();
    let mapping = format!("{OLD_MAPPING}\n[overrides.F-1]\nstatus = \"fixed\"\n");
    std::fs::write(outside_root.join("m.toml"), &mapping).unwrap();
    let rel = format!("../{name}/m.toml");

    let (code, out) = trackers(&r, &["apply", &rel, "--no-old-catalog"]);
    assert_eq!(code, 0, "apply: {out}");
    let (code, out) = trackers(&r, &["check", &rel]);
    assert_eq!(code, 0, "check: {out}");
    assert_eq!(out["clean"], json!(true), "{out}");
}
