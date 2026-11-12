//! End to end: the built `codescout` binary's `derive-shadow` exit codes and streams.
//!
//! The logic is covered in `src/cli/derive_shadow.rs` and `src/librarian/derived/diff.rs`; this
//! checks what only the binary does: that the subcommand is wired into `main`, that it never
//! creates the old catalog it is pointed at, and that it turns an outcome into a process exit
//! code (`0` when it compared, `2` and `NOT COMPARED — <reason>` when it could not).
//!
//! `CARGO_BIN_EXE_codescout` makes Cargo build the binary first (see
//! `tests/cross_process_write_lock.rs`), so there is no missing-binary skip branch.

use std::path::Path;
use std::process::{Command, Output};

fn git(dir: &Path, args: &[&str]) {
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
}

fn derive_shadow(project: &Path, old_db: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codescout"))
        .arg("derive-shadow")
        .arg("--project")
        .arg(project)
        .arg("--old-db")
        .arg(old_db)
        .args(extra)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap()
}

#[test]
fn derive_shadow_exit_codes_through_the_binary() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    git(root, &["init", "-q", "."]);
    std::fs::write(root.join("a.md"), "# A\n").unwrap();
    git(root, &["add", "--", "a.md"]);
    git(root, &["commit", "-q", "-m", "c"]);

    // No old catalog: exit 2, one line on stdout, and the file is not created.
    let old_dir = tempfile::tempdir().unwrap();
    let missing = old_dir.path().join("catalog.db");
    let out = derive_shadow(root, &missing, &[]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("NOT COMPARED — "), "{text}");
    assert!(!missing.exists(), "the binary created the old catalog");
    assert_eq!(std::fs::read_dir(old_dir.path()).unwrap().count(), 0);

    // An old catalog with the tables the diff reads and no rows: it compared, so exit 0 even
    // though the one document the commit tracks is not in the old catalog.
    let conn = rusqlite::Connection::open(&missing).unwrap();
    conn.execute_batch(
        "CREATE TABLE artifact (id TEXT PRIMARY KEY, abs_path TEXT NOT NULL UNIQUE, \
           kind TEXT NOT NULL, status TEXT NOT NULL, title TEXT, owners TEXT NOT NULL, \
           tags TEXT NOT NULL, file_sha256 TEXT NOT NULL, slug TEXT, missing_since INTEGER);
         CREATE TABLE artifact_link (src_id TEXT NOT NULL, dst_id TEXT NOT NULL, \
           rel TEXT NOT NULL);
         CREATE TABLE entry_cite (src_slug TEXT NOT NULL, src_local TEXT NOT NULL, \
           dst_ref TEXT NOT NULL, rel TEXT NOT NULL);
         CREATE TABLE artifact_chunk (chunk_id TEXT PRIMARY KEY, artifact_id TEXT NOT NULL, \
           chunk_ix INTEGER NOT NULL);",
    )
    .unwrap();
    drop(conn);
    let bytes = std::fs::read(&missing).unwrap();
    let out = derive_shadow(root, &missing, &[]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("documents: old 0 (in scope), derived 1, on both sides 0"),
        "{text}"
    );
    assert!(text.contains("  only_new document a.md"), "{text}");
    assert_eq!(
        std::fs::read(&missing).unwrap(),
        bytes,
        "the old catalog changed"
    );

    // `--json` is one object.
    let out = derive_shadow(root, &missing, &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["compared"], true);
    assert_eq!(v["diff"]["only_new"][0]["path"], "a.md");
}
