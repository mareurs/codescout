//! Independent review probes. Original acceptance stays in worktree_write_scope.rs.
use codescout::agent::Agent;
use codescout::lsp::LspManager;
use codescout::tools::approve_write::ApproveWrite;
use codescout::tools::create_file::CreateFile;
use codescout::tools::edit_file::EditFile;
use codescout::tools::{Tool, ToolContext};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::{tempdir, TempDir};

async fn fixture(worktrees: bool) -> (TempDir, ToolContext, PathBuf, PathBuf) {
    let temp = tempdir().unwrap();
    let root = temp.path().join("main");
    let linked = temp.path().join("linked");
    let outside = temp.path().join("scratch");
    for path in [&root, &linked, &outside, &root.join("member")] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(
        root.join("member/Cargo.toml"),
        "[package]\nname='member'\nversion='0.1.0'\n",
    )
    .unwrap();
    if worktrees {
        let entry = root.join(".git/worktrees/linked");
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(entry.join("gitdir"), format!("{}/.git\n", linked.display())).unwrap();
    }
    let agent = Agent::new(Some(root.clone())).await.unwrap();
    assert!(!agent.is_project_chosen_this_session().await);
    assert!(agent
        .discovered_projects()
        .await
        .iter()
        .any(|p| p.relative_root == Path::new("member")));
    let context = ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer: Arc::new(codescout::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(
            codescout::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    (temp, context, outside, linked)
}

async fn invoke(
    operation: &str,
    target: &Path,
    key: &str,
    context: &ToolContext,
) -> anyhow::Result<String> {
    let mut input = match operation {
        "create" => json!({"content": "after", "overwrite": true}),
        "edit" => json!({"old_string": "before", "new_string": "after"}),
        "approve" => json!({}),
        _ => unreachable!(),
    };
    input[key] = json!(target);
    let result = match operation {
        "create" => CreateFile.call_content(input, context).await,
        "edit" => EditFile.call_content(input, context).await,
        "approve" => ApproveWrite.call_content(input, context).await,
        _ => unreachable!(),
    }?;
    Ok(serde_json::to_string(&result).unwrap())
}

async fn must_guard(operation: &str, target: &Path, context: &ToolContext) {
    let result = invoke(operation, target, "path", context).await;
    assert!(
        result
            .as_ref()
            .err()
            .is_some_and(|e| e.to_string().contains("worktrees detected")),
        "{operation} must retain project guard at {}: {result:?}; leaf_symlink={}; bytes={:?}",
        target.display(),
        target.is_symlink(),
        std::fs::read_to_string(target)
    );
}

#[tokio::test]
async fn production_aliases_work_outside_projects() {
    for worktrees in [false, true] {
        let (_temp, context, outside, _) = fixture(worktrees).await;
        for operation in ["create", "edit"] {
            for alias in ["file_path", "relative_path", "file"] {
                let target = outside.join(format!("{operation}-{alias}.txt"));
                if operation == "edit" {
                    std::fs::write(&target, "before").unwrap();
                }
                let result = invoke(operation, &target, alias, &context).await;
                assert!(
                    result.is_ok(),
                    "{operation}/{alias}/{worktrees}: {result:?}"
                );
                assert_eq!(std::fs::read_to_string(target).unwrap(), "after");
            }
        }
        assert!(!context.agent.is_project_chosen_this_session().await);
    }
}

#[tokio::test]
async fn ordinary_discovered_member_stays_guarded() {
    let (temp, context, _, _) = fixture(true).await;
    let member = temp.path().join("main/member");
    for operation in ["create", "edit", "approve"] {
        let target = if operation == "approve" {
            member.clone()
        } else {
            member.join("file.txt")
        };
        if operation == "edit" {
            std::fs::write(&target, "before").unwrap();
        }
        must_guard(operation, &target, &context).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn relocated_known_member_stays_guarded() {
    let (temp, context, _, _) = fixture(true).await;
    let old_member = temp.path().join("main/member");
    let moved_member = temp.path().join("moved-member");
    std::fs::rename(&old_member, &moved_member).unwrap();
    std::os::unix::fs::symlink(&moved_member, &old_member).unwrap();
    assert!(context
        .agent
        .discovered_projects()
        .await
        .iter()
        .any(|p| p.relative_root == Path::new("member")));
    for operation in ["create", "edit", "approve"] {
        let target = if operation == "approve" {
            moved_member.clone()
        } else {
            moved_member.join("file.txt")
        };
        if operation == "edit" {
            std::fs::write(&target, "before").unwrap();
        }
        must_guard(operation, &target, &context).await;
    }
}

#[cfg(unix)]
async fn leaf_replacement_must_guard(operation: &str, linked_target: bool) {
    let (temp, context, outside, linked) = fixture(true).await;
    let destination = outside.join("destination.txt");
    std::fs::write(&destination, "before").unwrap();
    let root = if linked_target {
        linked
    } else {
        temp.path().join("main")
    };
    let target = root.join("link.txt");
    std::os::unix::fs::symlink(&destination, &target).unwrap();
    must_guard(operation, &target, &context).await;
    assert!(target.is_symlink());
    assert_eq!(std::fs::read_to_string(destination).unwrap(), "before");
}

#[cfg(unix)]
#[tokio::test]
async fn create_must_guard_project_leaf_replacement() {
    leaf_replacement_must_guard("create", false).await;
}

#[cfg(unix)]
#[tokio::test]
async fn edit_must_guard_project_leaf_replacement() {
    leaf_replacement_must_guard("edit", false).await;
}

#[cfg(unix)]
#[tokio::test]
async fn create_must_guard_linked_leaf_replacement() {
    leaf_replacement_must_guard("create", true).await;
}

#[cfg(unix)]
#[tokio::test]
async fn edit_must_guard_linked_leaf_replacement() {
    leaf_replacement_must_guard("edit", true).await;
}

#[cfg(unix)]
#[tokio::test]
async fn missing_descendants_under_directory_alias_stay_guarded() {
    let (temp, context, _, linked) = fixture(true).await;
    let alias = temp.path().join("directory-alias");
    std::os::unix::fs::symlink(&linked, &alias).unwrap();
    must_guard("create", &alias.join("missing/deep/file.txt"), &context).await;
    must_guard("approve", &alias.join("missing/deep"), &context).await;
    assert!(!linked.join("missing").exists());
}

#[tokio::test]
async fn opaque_and_malformed_paths_keep_conservative_guard() {
    let (_temp, context, _, _) = fixture(true).await;
    for path in [
        Value::Null,
        json!(""),
        json!("\u{0}"),
        json!("@ack_opaque_review"),
    ] {
        for operation in ["create", "edit", "approve"] {
            let input = json!({"path": path, "content": "after", "old_string": "before", "new_string": "after"});
            let result = match operation {
                "create" => CreateFile.call_content(input, &context).await,
                "edit" => EditFile.call_content(input, &context).await,
                "approve" => ApproveWrite.call_content(input, &context).await,
                _ => unreachable!(),
            };
            assert!(
                result
                    .as_ref()
                    .err()
                    .is_some_and(|e| e.to_string().contains("worktrees detected")),
                "{operation}/{path}: {result:?}"
            );
        }
    }
}

#[cfg(unix)]
async fn outside_leaf_replacement(operation: &str) {
    let (temp, context, outside, _) = fixture(true).await;
    let referent = temp.path().join("main/referent.txt");
    std::fs::write(&referent, "before").unwrap();
    let target = outside.join("link.txt");
    std::os::unix::fs::symlink(&referent, &target).unwrap();
    let result = invoke(operation, &target, "path", &context).await;
    assert!(
        result.is_ok(),
        "outside leaf replacement must be independent of project referent: {result:?}"
    );
    assert!(!target.is_symlink());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "after");
    assert_eq!(std::fs::read_to_string(&referent).unwrap(), "before");
    assert!(!context.agent.is_project_chosen_this_session().await);
}

#[cfg(unix)]
#[tokio::test]
async fn create_outside_leaf_can_replace_without_touching_project_referent() {
    outside_leaf_replacement("create").await;
}

#[cfg(unix)]
#[tokio::test]
async fn edit_outside_leaf_can_replace_without_touching_project_referent() {
    outside_leaf_replacement("edit").await;
}
