//! The project write lock, taken by the CLI commands that write.
//!
//! The MCP server serialises every write on `<project root>/.codescout/write.lock`; the CLI
//! locks the same kind of file, so a `codescout doc update` run from a shell waits for an agent's
//! `append_entry` instead of rewriting a ledger under it (a lost update).
//!
//! **Which file.** The CLI locks `<lock_root>/.codescout/write.lock`, where [`lock_root`] is the
//! nearest directory, from the project the CLI resolved up to its git root, that holds a
//! `.codescout/project.toml` (the git root when none does). MCP locks the directory it was
//! launched, activated or pinned at, with no walk upward. The two reach the SAME file when MCP's
//! activated directory is the nearest one holding a `project.toml`. They reach DIFFERENT files
//! when MCP is activated at a directory without a `project.toml`, or for an umbrella member, and
//! the two writers then do not exclude each other. Plan E's per-ledger file lock is the fix; this
//! module is the per-project lock only.
//!
//! **When.** The lock is taken after `open_ctx` and held until the tool body returns, so a
//! refusal costs no catalog work and the order matches MCP (flock before SQLite). A command that
//! does not write takes none.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::agent::{acquire_write_guard, holder_record_path, open_lock_file, WriteGuard};
use crate::librarian::current_project::CurrentProject;

/// The directory whose `.codescout/write.lock` the CLI locks for a project that starts at `start`
/// inside the git root `git_root`: the nearest directory from `start` up to and including
/// `git_root` that holds `.codescout/project.toml`, else `git_root`.
pub(crate) fn lock_root_for(start: &Path, git_root: &Path) -> PathBuf {
    for dir in start.ancestors() {
        if dir.join(".codescout").join("project.toml").is_file() {
            return dir.to_path_buf();
        }
        if dir == git_root {
            break;
        }
    }
    git_root.to_path_buf()
}

/// The directory whose `.codescout/write.lock` the CLI locks for `cp`: the nearest directory from
/// `cp.abs_path` up to and including `cp.git_root` that holds `.codescout/project.toml`, else
/// `cp.git_root`.
pub(crate) fn lock_root(cp: &CurrentProject) -> PathBuf {
    lock_root_for(&cp.abs_path, &cp.git_root)
}

/// Take the project write lock for `command`, waiting up to the project's
/// `security.write_lock_timeout_secs`. The returned guard releases the lock when dropped.
///
/// A CLI process that resolved no project has nothing to lock: that is an error naming
/// `--project`, never a command that runs unlocked.
pub(crate) async fn take(
    ctx: &crate::librarian::tools::ToolContext,
    command: &str,
) -> Result<WriteGuard> {
    let cp = ctx.current_project.as_deref().ok_or_else(|| {
        anyhow!(
            "no project to lock for this write; run it inside a project or pass `--project <dir>`"
        )
    })?;
    let root = lock_root(cp);
    let config = crate::config::project::ProjectConfig::load_or_default(&root)
        .context("reading the project's write-lock timeout")?;
    take_at(
        &root,
        Duration::from_secs(config.security.write_lock_timeout_secs),
        command,
    )
    .await
}

/// [`take`] below the project lookup: lock `<root>/.codescout/write.lock` within `timeout`.
///
/// The in-process mutex is fresh, because a CLI process runs one command and nothing else in it
/// contends; the cross-process flock is what excludes MCP and other CLI processes. A refusal
/// carries the holder record (for example `codescout:sid-beta append_entry`) in its message.
pub(crate) async fn take_at(root: &Path, timeout: Duration, command: &str) -> Result<WriteGuard> {
    let file = open_lock_file(root).context("opening the project write lock file")?;
    let guard = acquire_write_guard(
        Arc::new(tokio::sync::Mutex::new(())),
        file,
        holder_record_path(root),
        timeout,
        &format!("codescout-cli {command}"),
    )
    .await?;
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::audit_doc_refs::AuditArgs;
    use crate::cli::doctor::DoctorArgs;
    use clap::Parser;

    #[derive(Parser)]
    struct DoctorCli {
        #[command(flatten)]
        args: DoctorArgs,
    }

    #[derive(Parser)]
    struct AuditCli {
        #[command(flatten)]
        args: AuditArgs,
    }

    fn doctor(argv: &[&str]) -> DoctorArgs {
        DoctorCli::try_parse_from(std::iter::once("doctor").chain(argv.iter().copied()))
            .expect("the doctor argv parses")
            .args
    }

    fn audit(argv: &[&str]) -> AuditArgs {
        AuditCli::try_parse_from(std::iter::once("audit-doc-refs").chain(argv.iter().copied()))
            .expect("the audit-doc-refs argv parses")
            .args
    }

    fn project(abs_path: &Path, git_root: &Path) -> CurrentProject {
        CurrentProject {
            abs_path: abs_path.to_path_buf(),
            git_root: git_root.to_path_buf(),
            main_root: None,
            umbrella: None,
        }
    }

    // mutation: make `lock_root_for` return `git_root`. (This test also pins that `lock_root` agrees
    // with it; a `lock_root` that stopped calling it and kept its own loop would still pass.)
    #[test]
    fn lock_root_for_matches_lock_root() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        // A nested project: its `project.toml` is nearer than the git root.
        let nested = repo.join("packages/app");
        std::fs::create_dir_all(nested.join(".codescout")).unwrap();
        std::fs::write(
            nested.join(".codescout/project.toml"),
            "[project]\nname = \"app\"\n",
        )
        .unwrap();
        let deep = nested.join("src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        let bare = repo.join("docs");
        std::fs::create_dir_all(&bare).unwrap();

        for start in [&deep, &nested, &bare, &repo] {
            assert_eq!(
                lock_root_for(start, &repo),
                lock_root(&project(start, &repo)),
                "the same root for a start of {start:?}"
            );
        }
        assert_eq!(lock_root_for(&deep, &repo), nested);
        assert_eq!(lock_root_for(&bare, &repo), repo);
    }

    // mutation: make `lock_root` return `cp.abs_path.clone()`.
    #[test]
    fn the_lock_root_is_the_nearest_project_toml_within_the_git_root() {
        let tmp = tempfile::tempdir().unwrap();

        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join(".codescout")).unwrap();
        std::fs::write(
            repo.join(".codescout/project.toml"),
            "[project]\nname = \"t\"\n",
        )
        .unwrap();
        let deep = repo.join("sub/dir");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            lock_root(&project(&deep, &repo)),
            repo,
            "a project.toml above the working directory, inside the git root, names the root"
        );

        let repo2 = tmp.path().join("repo2");
        std::fs::create_dir_all(repo2.join(".git")).unwrap();
        let x = repo2.join("x");
        std::fs::create_dir_all(&x).unwrap();
        assert_eq!(
            lock_root(&project(&x, &repo2)),
            repo2,
            "with no project.toml anywhere the git root is the lock root"
        );
    }

    // mutation: open the lock file under `root.join("sub")` in `take_at`.
    #[tokio::test]
    async fn a_held_lock_refuses_the_cli_and_names_the_holder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let held = acquire_write_guard(
            Arc::new(tokio::sync::Mutex::new(())),
            open_lock_file(root).unwrap(),
            holder_record_path(root),
            Duration::from_secs(1),
            "codescout:sid-beta append_entry",
        )
        .await
        .unwrap();

        let err = take_at(root, Duration::from_millis(200), "doc update")
            .await
            .err()
            .expect("a held lock refuses the CLI");
        assert!(
            format!("{err:#}").contains("codescout:sid-beta append_entry"),
            "the refusal names the holder record, got: {err:#}"
        );

        drop(held);
        assert!(
            take_at(root, Duration::from_millis(200), "doc update")
                .await
                .is_ok(),
            "the lock is free again once the holder drops it"
        );
    }

    // mutation: make `doctor::takes_the_write_lock` return `true` always.
    #[test]
    fn doctor_takes_the_lock_only_with_a_fix() {
        assert!(!crate::cli::doctor::takes_the_write_lock(&doctor(&[])));
        assert!(!crate::cli::doctor::takes_the_write_lock(&doctor(&[
            "--fail-on-violations"
        ])));
        assert!(crate::cli::doctor::takes_the_write_lock(&doctor(&[
            "--fix",
            "export_augmentations"
        ])));
    }

    // mutation: make `audit_doc_refs::takes_the_write_lock` return `false` always.
    #[test]
    fn audit_doc_refs_takes_the_lock_unless_it_emits_no_tracker() {
        assert!(crate::cli::audit_doc_refs::takes_the_write_lock(
            &audit(&[])
        ));
        assert!(!crate::cli::audit_doc_refs::takes_the_write_lock(&audit(
            &["--no-emit-tracker"]
        )));
    }
}
