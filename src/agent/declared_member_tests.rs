//! A `[[project]]` entry in `workspace.toml` makes a directory a workspace member, even when
//! the directory holds no build manifest.
//!
//! Before this, `Agent::new` and `Agent::activate` built the member list from the manifest walk
//! alone and read only `discovery_max_depth` and `exclude_projects` from the file, so a declared
//! entry was parsed and then ignored.
//! docs/issues/2026-10-28-a-nested-git-repo-cannot-join-a-workspace-as-a-project-and-pinned-onboarding-mixes-in-the-home-workspace.md

use super::*;
use tempfile::tempdir;

/// A workspace root with one manifest-less directory and a `workspace.toml` declaring it.
fn workspace_declaring_a_manifestless_member() -> (tempfile::TempDir, PathBuf) {
    let host = tempdir().unwrap();
    let root = std::fs::canonicalize(host.path()).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"home\"\n").unwrap();
    std::fs::create_dir_all(root.join("remote-fixes/addon")).unwrap();
    std::fs::write(root.join("remote-fixes/addon/addon.py"), "print('x')").unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(
        crate::config::workspace::workspace_config_path(&root),
        "[workspace]\nname = \"home\"\n\n\
         [[project]]\nid = \"thecrew-module\"\nroot = \"remote-fixes/addon\"\nlanguages = [\"python\"]\n",
    )
    .unwrap();
    (host, root)
}

#[tokio::test]
async fn startup_lists_a_declared_manifestless_member() {
    let (_host, root) = workspace_declaring_a_manifestless_member();

    let agent = Agent::new(Some(root)).await.unwrap();

    let ids: Vec<String> = agent
        .discovered_projects()
        .await
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert!(
        ids.contains(&"thecrew-module".to_string()),
        "the declared member is missing: {ids:?}"
    );
}

#[tokio::test]
async fn an_ensure_resident_workspace_lists_a_declared_manifestless_member() {
    let (_host, declaring_root) = workspace_declaring_a_manifestless_member();
    let home = tempdir().unwrap();
    let agent = Agent::new(Some(home.path().to_path_buf())).await.unwrap();

    agent.ensure_resident(declaring_root.clone()).await.unwrap();

    let ids: Vec<String> = agent
        .discovered_projects_at(Some(&declaring_root))
        .await
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert!(
        ids.contains(&"thecrew-module".to_string()),
        "the declared member is missing: {ids:?}"
    );
}
