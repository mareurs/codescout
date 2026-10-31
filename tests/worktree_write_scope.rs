//! Regression contract for writes outside the project-selection guard's scope.
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
    for path in [&root, &linked, &outside] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    if worktrees {
        let entry = root.join(".git/worktrees/linked");
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(entry.join("gitdir"), format!("{}/.git\n", linked.display())).unwrap();
    }
    let agent = Agent::new(Some(root.clone())).await.unwrap();
    assert!(!agent.is_project_chosen_this_session().await);
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

async fn invoke(operation: &str, target: &Path, context: &ToolContext) -> anyhow::Result<Value> {
    match operation {
        "create" => {
            CreateFile
                .call(json!({"path": target, "content": "after"}), context)
                .await
        }
        "edit" => {
            EditFile
                .call(
                    json!({"path": target, "old_string": "before", "new_string": "after"}),
                    context,
                )
                .await
        }
        "approve" => ApproveWrite.call(json!({"path": target}), context).await,
        _ => unreachable!(),
    }
}

async fn outside_control(operation: &str, worktrees: bool) {
    let (_temp, context, outside, _) = fixture(worktrees).await;
    let target = if operation == "approve" {
        outside.clone()
    } else {
        outside.join("file.rs")
    };
    if operation == "edit" {
        std::fs::write(&target, "before").unwrap();
    }
    let result = invoke(operation, &target, &context).await;
    assert!(
        result.is_ok(),
        "{operation} outside roots, worktrees={worktrees}: {result:?}"
    );
    if operation == "approve" {
        assert_eq!(
            result.unwrap()["approved"],
            target.to_string_lossy().as_ref()
        );
    } else {
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "after");
    }
    assert!(!context.agent.is_project_chosen_this_session().await);
}

#[tokio::test]
async fn create_outside_project_is_independent_of_linked_worktrees() {
    outside_control("create", false).await;
    outside_control("create", true).await;
}

#[tokio::test]
async fn edit_outside_project_is_independent_of_linked_worktrees() {
    outside_control("edit", false).await;
    outside_control("edit", true).await;
}

#[tokio::test]
async fn approve_outside_project_is_independent_of_linked_worktrees() {
    outside_control("approve", false).await;
    outside_control("approve", true).await;
}

async fn still_protects_projects(operation: &str) {
    let (temp, context, _, linked) = fixture(true).await;
    let root = temp.path().join("main");
    let mut roots = vec![root, linked.clone()];
    #[cfg(unix)]
    {
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&linked, &alias).unwrap();
        roots.push(alias);
    }
    for root in roots {
        let target = if operation == "approve" {
            root.clone()
        } else {
            root.join("file.rs")
        };
        if operation == "edit" {
            std::fs::write(&target, "before").unwrap();
        }
        let error = invoke(operation, &target, &context)
            .await
            .expect_err("unchosen project path must remain guarded");
        assert!(
            error.to_string().contains("worktrees detected"),
            "{operation} at {}: {error:#}",
            target.display()
        );
        if operation == "edit" {
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "before");
        } else if operation == "create" {
            assert!(!target.exists());
        }
    }
}

#[tokio::test]
async fn create_keeps_project_and_linked_worktree_protection() {
    still_protects_projects("create").await;
}

#[tokio::test]
async fn edit_keeps_project_and_linked_worktree_protection() {
    still_protects_projects("edit").await;
}

#[tokio::test]
async fn approve_keeps_project_and_linked_worktree_protection() {
    still_protects_projects("approve").await;
}
