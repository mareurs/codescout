//! Runs the shell suite for `scripts/worktree-remove.sh` under `cargo test`, so the gate
//! covers it.
//!
//! `scripts/worktree-remove.sh` signals processes and removes a worktree, so its suite
//! (`tests/worktree-remove.sh`) starts real processes in its own temp worktrees and kills
//! what it starts when it ends. The suite is a bash script, not Rust, and nothing else in
//! the gate runs it: without this wrapper a regression in the stop, remove or verify
//! sequence is invisible to `cargo nextest run`.
//!
//! Asserts one thing: the suite exits zero. On failure it prints the suite's own output,
//! which names the failing case and the values it saw.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn the_worktree_remove_suite_passes() {
    let out = Command::new("bash")
        .arg("tests/worktree-remove.sh")
        .current_dir(repo_root())
        .output()
        .expect("failed to run `bash tests/worktree-remove.sh`");
    assert!(
        out.status.success(),
        "tests/worktree-remove.sh exited with {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
