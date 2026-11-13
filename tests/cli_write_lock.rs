//! The CLI write commands take the project write lock (`src/cli/write_lock.rs`).
//!
//! Each test spawns the real `codescout` binary against a throwaway project and holds
//! `.codescout/write.lock` from THIS process, the way `tests/cross_process_write_lock.rs` does for
//! the MCP server. `CARGO_BIN_EXE_codescout` makes Cargo build the binary before the test runs.
//!
//! The environment is handed to the child through `Command::env`, never set on the test process
//! (`tests/env_mutation_isolation.rs`).
//!
//! Run with: `cargo test --test cli_write_lock`

use fs4::fs_std::FileExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A project the CLI resolves (`.git/` marks the repository root) and a scratch state directory
/// that holds everything the child would otherwise write under the developer's home.
struct Fixture {
    project: tempfile::TempDir,
    state: tempfile::TempDir,
}

/// `write_lock_timeout_secs` is the budget the child waits for the lock before it refuses.
fn fixture(write_lock_timeout_secs: u64) -> Fixture {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join(".git")).unwrap();
    std::fs::create_dir_all(project.path().join(".codescout")).unwrap();
    std::fs::write(
        project.path().join(".codescout/project.toml"),
        format!("[project]\nname = \"t\"\n[security]\nwrite_lock_timeout_secs = {write_lock_timeout_secs}\n"),
    )
    .unwrap();
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("workspace.toml"), "").unwrap();
    Fixture { project, state }
}

/// The `codescout` binary with the environment redirected into the fixture. Without the
/// redirection the child would write the developer's real guide-hint ledger, librarian workspace
/// file and catalog (`tests/cross_process_write_lock.rs`, "Spawn the binary").
fn codescout(fx: &Fixture, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_codescout"));
    cmd.args(args)
        .env("XDG_STATE_HOME", fx.state.path())
        .env(
            "LIBRARIAN_WORKSPACE",
            fx.state.path().join("workspace.toml"),
        )
        .env("LIBRARIAN_DB", fx.state.path().join("catalog.db"))
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .stdin(Stdio::null());
    cmd
}

/// Take the project's OS-level flock from this process, then write a holder record naming
/// `holder`, in the order `write_guard::acquire` uses. Dropping the returned file releases it.
fn hold_the_lock(project: &Path, holder: &str) -> std::fs::File {
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(project.join(".codescout/write.lock"))
        .unwrap();
    lock.try_lock_exclusive()
        .expect("the test process takes the lock uncontested");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    std::fs::write(
        project.join(".codescout/write.lock.holder"),
        format!("{now_ms}\t{holder}"),
    )
    .unwrap();
    lock
}

// mutation: remove the `take` call from `cli::doc::run_create`; the child then exits 0 and
// `docs/t.md` exists.
#[test]
fn cli_doc_create_waits_for_the_project_lock_and_names_its_holder() {
    let fx = fixture(1);
    let dir = fx.project.path().to_str().unwrap().to_owned();
    let lock = hold_the_lock(fx.project.path(), "codescout:sid-beta append_entry");

    let out = codescout(
        &fx,
        &[
            "doc",
            "create",
            "--project",
            &dir,
            "--kind",
            "note",
            "--title",
            "T",
            "--rel-path",
            "docs/t.md",
        ],
    )
    .output()
    .unwrap();

    assert!(
        !out.status.success(),
        "a held lock refuses the CLI; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("codescout:sid-beta append_entry"),
        "the refusal names the holder record, got stderr:\n{stderr}"
    );
    assert!(
        !fx.project.path().join("docs/t.md").exists(),
        "a refused create writes nothing"
    );
    drop(lock);
}

// mutation: remove the `take` call from `cli::doc::run_update`; step 3 then sees the child
// exited, or `status: done` already on disk.
#[test]
fn a_cli_update_waits_for_the_lock_and_keeps_the_other_writers_bytes() {
    let fx = fixture(5);
    let dir = fx.project.path().to_str().unwrap().to_owned();
    let doc = fx.project.path().join("docs/t.md");

    // (0) Create docs/t.md through the CLI with no lock held.
    let created = codescout(
        &fx,
        &[
            "doc",
            "create",
            "--project",
            &dir,
            "--kind",
            "note",
            "--title",
            "T",
            "--rel-path",
            "docs/t.md",
            "--json",
        ],
    )
    .output()
    .unwrap();
    assert!(
        created.status.success(),
        "an unlocked create succeeds; stderr:\n{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let created_json: serde_json::Value =
        serde_json::from_slice(&created.stdout).unwrap_or_else(|e| {
            panic!(
                "create --json prints JSON ({e}):\n{}",
                String::from_utf8_lossy(&created.stdout)
            )
        });
    let id = created_json["id"]
        .as_str()
        .unwrap_or_else(|| panic!("create --json names the document id:\n{created_json}"))
        .to_owned();
    assert!(doc.exists(), "the create wrote docs/t.md");

    // (1) This process takes the project lock.
    let lock = hold_the_lock(fx.project.path(), "codescout:sid-beta append_entry");

    // (2) Spawn the update.
    let mut child = codescout(
        &fx,
        &["doc", "update", &id, "--status", "done", "--project", &dir],
    )
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();

    // (3) For ~1.5 s the child is still running and has written nothing. `try_wait` is the
    // assertion; no sleep stands in for one.
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(1500) {
        assert!(
            child.try_wait().unwrap().is_none(),
            "the update must still be waiting for the project lock"
        );
        assert!(
            !std::fs::read_to_string(&doc)
                .unwrap()
                .contains("status: done"),
            "the update wrote while another writer held the lock"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    // (4) The other writer appends its bytes and releases the lock.
    let mut text = std::fs::read_to_string(&doc).unwrap();
    text.push_str("peer line\n");
    std::fs::write(&doc, text).unwrap();
    FileExt::unlock(&lock).expect("the test releases the lock");
    drop(lock);

    // (5) The update then runs on top of those bytes.
    let status = child.wait().unwrap();
    assert!(
        status.success(),
        "the update exits 0 once the lock is free: {status}"
    );
    let text = std::fs::read_to_string(&doc).unwrap();
    assert!(
        text.contains("peer line") && text.contains("status: done"),
        "the update keeps the other writer's bytes and applies its own:\n{text}"
    );
}
