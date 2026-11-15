//! `codescout hot-view` through the built binary: the exit code is what a session-start hook reads,
//! so this is the one test that the subcommand is wired into `main` and that `run` turns code 2
//! into the process exit status.

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

fn hot_view(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codescout"))
        .arg("hot-view")
        .args(extra)
        .arg("--project")
        .arg(dir)
        // The same isolation `git()` above has: the user's git config must not change a verdict.
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap()
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn repo_with_ledger() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "."]);
    write(
        dir.path(),
        "docs/trackers/ledgers/l/readme.md",
        "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, fixed: terminal}\ninitial_status: open\n---\n",
    );
    write(
        dir.path(),
        "docs/trackers/ledgers/l/new.md",
        "---\nentry_kind: f\ntitle: t\nstatus: open\nstatus_since: 2026-11-10\ncreated: 2026-11-10\n---\n",
    );
    dir
}

#[test]
fn an_unknown_key_exits_2_and_says_not_shown() {
    let dir = repo_with_ledger();
    let out = hot_view(dir.path(), &["nope"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("NOT SHOWN — "), "{text}");
}

#[test]
fn a_good_collection_exits_0_and_prints_the_header() {
    let dir = repo_with_ledger();
    let out = hot_view(dir.path(), &["l"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with("hot view: l (docs/trackers/ledgers/l): "),
        "{text}"
    );
    assert!(text.contains("[[new]]"), "{text}");
    // A hook can rely on stdout alone: nothing is written to stderr on the success path.
    assert!(
        out.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_project_that_is_not_a_git_repository_exits_2_and_says_not_shown() {
    let dir = tempfile::tempdir().unwrap();
    if dir.path().ancestors().any(|a| a.join(".git").exists()) {
        eprintln!(
            "skipped: the temp dir {} lies inside a git work tree, so it is not a non-repo",
            dir.path().display()
        );
        return;
    }
    let out = hot_view(dir.path(), &["l"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("NOT SHOWN — "), "{text}");
}
