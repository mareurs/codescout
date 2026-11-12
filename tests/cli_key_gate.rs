//! End to end: the built `codescout` binary's `key-gate` exit codes and streams.
//!
//! The logic is covered in `src/cli/key_gate.rs`; this checks what only the binary does: that
//! the subcommand is wired into `main`, needs no catalog and no `.codescout` directory, and
//! turns an outcome into a process exit code.
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

fn key_gate(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codescout"))
        .arg("key-gate")
        .arg("--project")
        .arg(dir)
        .args(extra)
        // The same isolation `git()` above has: the user's git config must not change a verdict.
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap()
}

fn add(dir: &Path, rel: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "# x\n").unwrap();
    git(dir, &["add", "--", rel]);
}

#[test]
fn key_gate_exit_codes_through_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q", "."]);
    add(root, "a/notes.md");
    git(root, &["commit", "-q", "-m", "c"]);
    assert!(!root.join(".codescout").exists(), "no .codescout directory");

    // Nothing new staged: exit 0, and the scope is named.
    let clean = key_gate(root, &["--staged"]);
    assert_eq!(clean.status.code(), Some(0), "{clean:?}");
    let text = String::from_utf8_lossy(&clean.stdout);
    assert!(text.contains("1 markdown file in the key space"), "{text}");

    // A staged file that collides with a committed one: exit 1, both paths named.
    add(root, "b/notes.md");
    let bad = key_gate(root, &["--staged"]);
    assert_eq!(bad.status.code(), Some(1), "{bad:?}");
    let text = String::from_utf8_lossy(&bad.stdout);
    assert!(
        text.contains("a/notes.md") && text.contains("b/notes.md"),
        "{text}"
    );

    // A malformed key-space file: exit 2 and `NOT CHECKED`, on stdout.
    add(root, ".codescout/key-space.toml");
    std::fs::write(root.join(".codescout/key-space.toml"), "exclude = [\n").unwrap();
    git(root, &["add", "--", ".codescout/key-space.toml"]);
    let unsure = key_gate(root, &["--staged"]);
    assert_eq!(unsure.status.code(), Some(2), "{unsure:?}");
    let text = String::from_utf8_lossy(&unsure.stdout);
    assert!(text.starts_with("NOT CHECKED — "), "{text}");
    assert!(
        !root.join(".codescout/catalog.db").exists(),
        "no catalog opened"
    );
}
