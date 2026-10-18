//! A root that is not an existing directory is REFUSED, by every path that would make it a
//! project, and the refusal creates NOTHING on disk.
//!
//! Before 2026-10-18 `Agent::ensure_resident` (the per-call `workspace=` pin), `Agent::activate`
//! and `Agent::new` each did `canonicalize(..).unwrap_or(raw)`: a missing path was kept as a
//! root and the first touch built a project there (`<root>/.codescout/{memories,
//! private-memories, usage.db, write.lock}` and a `.gitignore`). `activate_within_workspace`
//! did the same for a sub-project directory deleted after discovery.
//! docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
//!
//! These test the `Agent` entrances directly. The dispatch-level refusal (the one a tool call
//! meets first) is tested in `server/workspace_pin_tests.rs`; the two layers are separate guards,
//! so each has its own tests.

use super::*;
use tempfile::tempdir;

/// Every path under `root`, relative to it, sorted: "created nothing" is a claim about the tree.
fn tree_listing(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if entry.file_type().unwrap().is_dir() {
                walk(&path, root, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// The refusal must be a `RecoverableError` (so it routes to `isError: false`) that names the
/// path and says it is not an existing directory.
fn assert_refusal_names(err: &anyhow::Error, path: &Path, what: &str) {
    let recoverable = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
    assert!(
        recoverable.message.contains(&path.display().to_string()),
        "must name {}; got: {}",
        path.display(),
        recoverable.message
    );
    // WHICH argument is refused: asserted per caller, so a swapped label is a red test.
    assert!(
        recoverable.message.starts_with(&format!("{what} '")),
        "must name the argument as `{what}`; got: {}",
        recoverable.message
    );
    assert!(
        recoverable.message.contains("not an existing directory"),
        "must say what is wrong; got: {}",
        recoverable.message
    );
    assert!(
        recoverable.hint().is_some_and(|h| h.contains("absolute")),
        "must say what to do; got: {:?}",
        recoverable.hint()
    );
}

async fn agent_at_home() -> (tempfile::TempDir, PathBuf, Agent) {
    let home = tempdir().unwrap();
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let agent = Agent::new(Some(home_root.clone())).await.unwrap();
    (home, home_root, agent)
}

#[tokio::test]
async fn ensure_resident_refuses_a_missing_directory_and_creates_nothing() {
    let (_home, _home_root, agent) = agent_at_home().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let err = agent.ensure_resident(missing.clone()).await.unwrap_err();
    assert_refusal_names(&err, &missing, "workspace pin");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());

    // The pinned accessors reach it through `with_project_at`, which must propagate the same
    // refusal rather than swallow it into a default.
    let err = agent
        .with_project_at(Some(&missing), |p| Ok(p.root().to_path_buf()))
        .await
        .unwrap_err();
    assert_refusal_names(&err, &missing, "workspace pin");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
    assert!(
        !agent.inner.read().await.workspaces.contains_key(&missing),
        "a refused pin must not be resident"
    );
}

#[tokio::test]
async fn ensure_resident_refuses_a_file_and_leaves_it_untouched() {
    let (_home, _home_root, agent) = agent_at_home().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let file = host_root.join("not-a-dir.txt");
    std::fs::write(&file, "keep").unwrap();

    let err = agent.ensure_resident(file.clone()).await.unwrap_err();
    assert_refusal_names(&err, &file, "workspace pin");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep");
    assert_eq!(tree_listing(&host_root), vec!["not-a-dir.txt".to_string()]);
}

#[tokio::test]
async fn ensure_resident_still_makes_an_existing_directory_resident() {
    let (_home, home_root, agent) = agent_at_home().await;
    let other = tempdir().unwrap();
    let other_root = std::fs::canonicalize(other.path()).unwrap();

    agent.ensure_resident(other_root.clone()).await.unwrap();

    assert!(agent
        .inner
        .read()
        .await
        .workspaces
        .contains_key(&other_root));
    assert_eq!(
        agent.project_root().await,
        Some(home_root),
        "a pin never moves the session default"
    );
}

#[tokio::test]
async fn activate_refuses_a_missing_directory_and_changes_nothing() {
    let (_home, home_root, agent) = agent_at_home().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let err = agent.activate(missing.clone(), None).await.unwrap_err();
    assert_refusal_names(&err, &missing, "project root");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
    assert_eq!(
        agent.project_root().await,
        Some(home_root),
        "a refused activation must leave the session's project where it was"
    );
}

#[tokio::test]
async fn new_refuses_a_missing_directory_and_creates_nothing() {
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let err = Agent::new(Some(missing.clone()))
        .await
        .err()
        .expect("a missing --project must be refused");
    assert_refusal_names(&err, &missing, "project root");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
}

/// A sub-project is discovered from a manifest on disk, so its directory exists at discovery;
/// activating it later builds a lock file there (`create_dir_all`). If the directory was deleted
/// in between, activation must refuse rather than recreate it.
#[tokio::test]
async fn activate_within_workspace_refuses_a_sub_project_deleted_after_discovery() {
    let home = tempdir().unwrap();
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let sub = home_root.join("packages").join("api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name":"api","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();
    let agent = Agent::new(Some(home_root.clone())).await.unwrap();
    std::fs::remove_dir_all(&sub).unwrap();

    let err = agent
        .activate_within_workspace("api", None)
        .await
        .unwrap_err();
    assert_refusal_names(&err, &sub, "sub-project root");
    assert!(
        !sub.exists(),
        "a refused sub-project activation must not recreate its directory"
    );
}

/// F3 (M3): residency must not outlive the directory. A workspace that is already resident but
/// whose directory has since been deleted is refused on the next call, not served from the
/// cached entry. Residency is checked AFTER the directory, so a stale entry cannot answer.
#[tokio::test]
async fn ensure_resident_refuses_a_resident_workspace_whose_directory_was_deleted() {
    let (_home, _home_root, agent) = agent_at_home().await;
    let other = tempdir().unwrap();
    let other_root = std::fs::canonicalize(other.path()).unwrap();
    agent.ensure_resident(other_root.clone()).await.unwrap();
    std::fs::remove_dir_all(&other_root).unwrap();

    let err = agent.ensure_resident(other_root.clone()).await.unwrap_err();
    assert_refusal_names(&err, &other_root, "workspace pin");
    assert!(
        !other_root.exists(),
        "a refused pin must not re-create the deleted directory"
    );
}

/// F7: a path that cannot be REACHED is not "not an existing directory". A mode-000 parent
/// makes `canonicalize` fail with a permission error, and the old message claimed the path did
/// not exist, which sends the caller looking for a typo.
#[cfg(unix)]
#[test]
fn a_path_behind_an_unreadable_parent_is_refused_as_unreachable_not_as_missing() {
    use std::os::unix::fs::PermissionsExt;
    let host = tempdir().unwrap();
    let locked = std::fs::canonicalize(host.path()).unwrap().join("locked");
    let inner = locked.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }
    let _restore = Restore(locked.clone());
    if std::fs::read_dir(&locked).is_ok() {
        eprintln!("mode 000 does not bind this process; skipping");
        return;
    }

    let err = existing_project_dir(inner.clone(), "workspace pin").unwrap_err();
    assert!(
        err.message.contains("permission denied")
            && err.message.contains(&inner.display().to_string()),
        "must say the path cannot be reached and name it; got: {}",
        err.message
    );
    assert!(
        !err.message.contains("not an existing directory"),
        "must not claim the path is missing; got: {}",
        err.message
    );

    // The genuinely missing path keeps its own wording.
    let missing = existing_project_dir(host.path().join("nope"), "workspace pin").unwrap_err();
    assert!(
        missing.message.contains("not an existing directory"),
        "got: {}",
        missing.message
    );
}

/// F4: only a CONFIGURATION failure may point at project.toml. One case per way the classifier
/// says yes, and the filesystem failure it must say no to.
#[test]
fn only_a_configuration_failure_is_classified_as_a_project_toml_problem() {
    use anyhow::Context as _;
    let toml_err = toml::from_str::<toml::Value>("= =").unwrap_err();
    let wrapped_toml: anyhow::Error = Err::<(), _>(toml_err)
        .context("parsing project.toml")
        .unwrap_err();
    assert!(
        is_project_config_error(&wrapped_toml),
        "a TOML error under a context"
    );
    assert!(is_project_config_error(&anyhow::anyhow!(
        "invalid configuration for /p/.codescout/project.toml (merged with the global config)"
    )));
    assert!(is_project_config_error(&anyhow::anyhow!(
        "project.toml exceeds 1 MiB limit (2097152 bytes)"
    )));
    let denied: anyhow::Error = std::io::Error::from(std::io::ErrorKind::PermissionDenied).into();
    assert!(
        !is_project_config_error(&denied),
        "a permission error is not a config error"
    );
    assert!(!is_project_config_error(&anyhow::anyhow!("anything else")));
}

/// `pin_resolution_refusal` keeps an existing refusal exactly as it is (its message already
/// names the path and the remedy) and gives anything else the generic, project.toml-free hint.
#[test]
fn a_pin_refusal_is_passed_through_and_other_failures_get_the_generic_hint() {
    let root = Path::new("/p");
    let original = crate::tools::RecoverableError::with_hint("original message", "original hint");
    let passed = pin_resolution_refusal(root, original.into());
    assert_eq!(passed.message, "original message");
    assert_eq!(passed.hint(), Some("original hint"));

    let denied: anyhow::Error = std::io::Error::from(std::io::ErrorKind::PermissionDenied).into();
    let generic = pin_resolution_refusal(root, denied);
    assert!(generic
        .message
        .starts_with("workspace pin /p could not be resolved"));
    assert!(!generic.hint().unwrap().contains("project.toml"));

    let config = pin_resolution_refusal(
        root,
        anyhow::anyhow!("invalid configuration for /p/.codescout/project.toml"),
    );
    assert!(config.hint().unwrap().contains("project.toml"));
}
