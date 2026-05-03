use super::*;
use crate::agent::Agent;

use std::sync::Arc;
use tempfile::tempdir;

fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
    crate::lsp::LspManager::new_arc()
}

#[tokio::test]
async fn activate_and_get_config() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // No project initially
    assert!(ProjectStatus.call(json!({}), &ctx).await.is_err());

    // Activate
    let result = ActivateProject
        .call(
            json!({
                "path": dir.path().to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");

    // Now project_status works
    let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    assert!(!status["project_root"].as_str().unwrap().is_empty());
    assert!(status["languages"].is_array());
    assert!(status["embeddings_model"].is_string());
}

#[tokio::test]
async fn activate_surfaces_project_hints_from_cargo_toml() {
    // Agents that never call `onboarding` should still see primary language,
    // manifest, entry points, and build commands in the activate response.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let hints = &result["project_hints"];
    assert_eq!(hints["primary_language"], "rust");
    assert_eq!(hints["manifest"], "Cargo.toml");
    assert_eq!(hints["entry_points"], json!(["src/main.rs"]));
    assert!(
        hints["build_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "cargo test"),
        "hints must include cargo test: {hints:?}"
    );
    assert_eq!(hints["onboarded"], false);
}

#[tokio::test]
async fn activate_hints_empty_for_unrecognised_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // No manifest file.

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let hints = &result["project_hints"];
    assert!(hints["primary_language"].is_null());
    assert!(hints["manifest"].is_null());
    assert_eq!(hints["entry_points"], json!([]));
    assert_eq!(hints["build_commands"], json!([]));
}

#[tokio::test]
async fn activate_nonexistent_path_errors() {
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(
            json!({
                "path": "/nonexistent/path/xyz"
            }),
            &ctx,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn activate_replaces_previous_project() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(dir1.path().to_path_buf())).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Activate dir2
    ActivateProject
        .call(
            json!({
                "path": dir2.path().to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();

    let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let root = status["project_root"].as_str().unwrap();
    assert!(root.contains(dir2.path().file_name().unwrap().to_str().unwrap()));
}

#[tokio::test]
async fn project_status_returns_all_sections() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let tool = ProjectStatus;
    let result = tool.call(json!({}), &ctx).await.unwrap();
    assert!(result["project_root"].is_string(), "missing project_root");
    assert!(result["languages"].is_array(), "missing languages field");
    assert!(
        result["embeddings_model"].is_string(),
        "missing embeddings_model field"
    );
    assert!(result.get("index").is_some(), "missing index section");
    assert!(
        result.get("libraries").is_some(),
        "missing libraries section"
    );
}

#[tokio::test]
async fn project_status_compact_shape() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    // Flat config fields — no blob
    assert!(result["languages"].is_array(), "missing languages");
    assert!(
        result["embeddings_model"].is_string(),
        "missing embeddings_model"
    );
    assert!(
        result.get("config").is_none(),
        "config blob must be removed"
    );

    // Index section has status field, no drift
    assert!(
        result["index"]["status"].is_string(),
        "index.status must be present"
    );
    assert!(
        result["index"].get("drift").is_none(),
        "drift must not appear in project_status"
    );

    // Libraries section still present
    assert!(result["libraries"].is_object(), "libraries section missing");
}

#[tokio::test]
async fn project_status_includes_memory_staleness() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Create memories dir and a memory file
    let memories_dir = dir.path().join(".codescout/memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(memories_dir.join("architecture.md"), "# Arch").unwrap();

    // Create anchored file and sidecar
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/server.rs"), "fn main() {}").unwrap();

    let anchors =
        crate::memory::anchors::seed_anchors(dir.path(), "Uses `src/server.rs`.").unwrap();
    crate::memory::anchors::write_anchor_file(
        &memories_dir.join("architecture.anchors.toml"),
        &anchors,
    )
    .unwrap();

    // Before change — should be fresh
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let staleness = &result["memory_staleness"];
    assert!(staleness["stale"].as_array().unwrap().is_empty());
    assert!(staleness["fresh"]
        .as_array()
        .unwrap()
        .contains(&json!("architecture")));

    // Modify the anchored file
    std::fs::write(dir.path().join("src/server.rs"), "fn changed() {}").unwrap();

    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let staleness = &result["memory_staleness"];
    let stale = staleness["stale"].as_array().unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0]["topic"], "architecture");
    assert!(stale[0]["changed_files"]
        .as_array()
        .unwrap()
        .contains(&json!("src/server.rs")));
}

#[tokio::test]
async fn activate_includes_cwd_hint() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(None).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let input = json!({ "path": dir.path().to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(
        hint.starts_with("CWD: "),
        "hint should start with CWD: but was: {hint}"
    );
    assert!(hint.contains(dir.path().to_str().unwrap()));
}

#[tokio::test]
async fn activate_hint_shows_switched_when_away_from_home() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let input = json!({ "path": dir2.path().to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    // Non-home default is RO: "Browsing … (read-only). CWD: … — remember to workspace(action='activate', …)"
    assert!(
        hint.contains("remember to workspace"),
        "hint should warn to switch back: {hint}"
    );
    assert!(
        hint.contains(dir2.path().to_str().unwrap()),
        "should contain new path: {hint}"
    );
    assert!(
        hint.contains(dir1.path().to_str().unwrap()),
        "should contain home path: {hint}"
    );
}

#[tokio::test]
async fn activate_hint_shows_returned_when_back_home() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    // Switch away
    ActivateProject
        .call(json!({ "path": dir2.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    // Return home
    let result = ActivateProject
        .call(json!({ "path": dir1.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(hint.contains("Returned to home project"), "hint: {hint}");
    assert!(hint.contains(dir1.path().to_str().unwrap()));
}

#[tokio::test]
async fn project_status_shows_workspace_projects() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create multi-project structure
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    // Create workspace.toml
    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["test"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = ProjectStatus
        .call(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let ws = result.get("workspace");
    assert!(
        ws.is_some(),
        "project_status should include workspace section"
    );
    let projects = ws.unwrap().get("projects").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 2);
}

#[tokio::test]
async fn activate_project_switches_focus_by_id() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create multi-project structure
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Initially focused on root project
    let root_path = ctx.agent.require_project_root().await.unwrap();
    assert_eq!(root_path, root.to_path_buf());

    // Switch focus to mcp-server by ID
    let result = ActivateProject
        .call(serde_json::json!({"path": "mcp-server"}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");

    // Now focused on mcp-server
    let new_root = ctx.agent.require_project_root().await.unwrap();
    assert_eq!(new_root, root.join("mcp-server"));
}

#[tokio::test]
async fn activate_project_unknown_id_with_no_slash_returns_error() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"test\"\n").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // "unknown-project" has no slash and does not exist as a project ID or a path
    let result = ActivateProject
        .call(serde_json::json!({"path": "unknown-project"}), &ctx)
        .await;
    // Should fail: not a known project ID, and not a valid directory path
    assert!(
        result.is_err() || result.as_ref().unwrap().get("error").is_some(),
        "expected error or error field, got: {:?}",
        result
    );
}

#[tokio::test]
async fn post_compact_flushes_lsp_clients_and_returns_flushed() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // post_compact=true should return flushed:true without the normal status fields
    let result = ProjectStatus
        .call(json!({"post_compact": true}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["flushed"], json!(true), "expected flushed:true");
    assert!(result["hint"].is_string(), "expected hint string");
    // Normal status fields must NOT be present in the compact-flush response
    assert!(
        result.get("project_root").is_none(),
        "post_compact response must not include project_root"
    );

    // post_compact=false (or absent) should return the normal status response
    let result = ProjectStatus
        .call(json!({"post_compact": false}), &ctx)
        .await
        .unwrap();
    assert!(
        result["project_root"].is_string(),
        "normal call must include project_root"
    );
}

#[test]
fn format_activate_project_rw_compact() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch", "conventions", "gotchas"],
        "index": {"status": "not_indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert_eq!(
        compact,
        "activated · my-project (rw) · 3 memories · index: not_indexed"
    );
}

#[test]
fn format_activate_project_ro_with_workspace() {
    let result = json!({
        "status": "ok",
        "project": "sub-lib",
        "project_root": "/home/user/mono/sub-lib",
        "read_only": true,
        "memories": [],
        "index": {"status": "indexed"},
        "workspace": [
            {"id": "main", "root": ".", "languages": ["rust"]},
            {"id": "sub-lib", "root": "libs/sub-lib", "languages": ["rust"]},
        ],
        "hint": "Browsing sub-lib (read-only)."
    });
    let compact = format_activate_project(&result);
    assert_eq!(
        compact,
        "activated · sub-lib (ro) · 0 memories · index: indexed · 2 workspace projects"
    );
}

#[test]
fn format_activate_project_with_auto_libs() {
    let result = json!({
        "status": "ok",
        "project": "web",
        "project_root": "/home/user/web",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "auto_registered_libs": {"count": 12, "without_source": 3},
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert_eq!(compact, "activated · web (rw) · 1 memories · index: not_indexed · auto-registered 12 libs (3 without source)");
}

#[test]
fn format_activate_project_auto_libs_all_with_source() {
    let result = json!({
        "status": "ok",
        "project": "app",
        "project_root": "/home/user/app",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "auto_registered_libs": {"count": 5, "without_source": 0},
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert_eq!(
        compact,
        "activated · app (rw) · 0 memories · index: indexed · auto-registered 5 libs"
    );
}

#[tokio::test]
async fn activate_project_rw_includes_security_fields() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(
            json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(
        result["security_profile"].is_string(),
        "RW should include security_profile"
    );
    assert!(
        !result["shell_enabled"].is_null(),
        "RW should include shell_enabled"
    );
}

#[tokio::test]
async fn activate_project_ro_excludes_security_fields() {
    let home = tempdir().unwrap();
    let other = tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(other.path().join(".codescout")).unwrap();
    // Start with a home project (always RW)
    let ctx = ToolContext {
        agent: Agent::new(Some(home.path().to_path_buf())).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    // Now activate another project as RO
    let result = ActivateProject
        .call(
            json!({"path": other.path().to_str().unwrap(), "read_only": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(
        result["security_profile"].is_null(),
        "RO should not include security_profile"
    );
    assert!(
        result["shell_enabled"].is_null(),
        "RO should not include shell_enabled"
    );
}

#[tokio::test]
async fn activate_project_includes_memories_and_index() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(
        result["memories"].is_array(),
        "should include memories array"
    );
    assert!(result["index"].is_object(), "should include index object");
    assert!(
        result["index"]["status"].is_string(),
        "index should have status"
    );
}

#[tokio::test]
async fn activate_project_rw_hint_promotes_project_status() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(
            json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
            &ctx,
        )
        .await
        .unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(
        hint.contains("workspace(action='status')"),
        "RW hint should promote workspace status, got: {hint}"
    );
}

#[tokio::test]
async fn activate_project_single_project_no_workspace() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(
        result["workspace"].is_null(),
        "single-project should have null workspace"
    );
}

#[tokio::test]
async fn activate_project_focus_switch_returns_full_response() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a sub-project
    let sub = root.join("packages").join("api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name":"api","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(root)).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Focus-switch by ID
    let result = ActivateProject
        .call(json!({"path": "api"}), &ctx)
        .await
        .unwrap();

    assert_eq!(result["status"], "ok");
    assert!(result["project"].is_string(), "should have project name");
    assert!(result["languages"].is_array(), "should have languages");
    assert!(result["memories"].is_array(), "should have memories");
    assert!(result["index"].is_object(), "should have index");
    assert!(!result["read_only"].is_null(), "should have read_only");
}

#[tokio::test]
async fn activate_project_workspace_includes_depends_on() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let sub_a = root.join("packages").join("core");
    let sub_b = root.join("packages").join("web");
    std::fs::create_dir_all(&sub_a).unwrap();
    std::fs::create_dir_all(&sub_b).unwrap();
    std::fs::write(
        sub_a.join("package.json"),
        r#"{"name":"core","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();
    std::fs::write(
        sub_b.join("package.json"),
        r#"{"name":"web","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(root)).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    if let Some(ws) = result["workspace"].as_array() {
        for entry in ws {
            assert!(
                entry["depends_on"].is_array(),
                "each workspace entry should have depends_on"
            );
        }
    }
}

#[tokio::test]
async fn activate_project_ro_hint_warns_switch_back() {
    let home = tempdir().unwrap();
    let other = tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(other.path().join(".codescout")).unwrap();

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Activate home first
    ActivateProject
        .call(json!({"path": home.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    // Activate other as RO
    let result = ActivateProject
        .call(
            json!({"path": other.path().to_str().unwrap(), "read_only": true}),
            &ctx,
        )
        .await
        .unwrap();

    let hint = result["hint"].as_str().unwrap();
    assert!(
        hint.contains("remember to workspace"),
        "RO hint should warn about switching back, got: {hint}"
    );
    assert!(
        hint.contains("read-only"),
        "RO hint should mention read-only, got: {hint}"
    );
}

#[test]
fn activate_project_auto_libs_is_summary_not_array() {
    let result = json!({
        "status": "ok",
        "project": "test",
        "project_root": "/tmp/test",
        "read_only": false,
        "memories": [],
        "index": {"status": "not_indexed"},
        "auto_registered_libs": {"count": 5, "without_source": 2},
    });
    assert!(result["auto_registered_libs"].is_object());
    assert_eq!(result["auto_registered_libs"]["count"], 5);
    assert_eq!(result["auto_registered_libs"]["without_source"], 2);
}

#[tokio::test]
async fn activate_project_memories_graceful_on_error() {
    // A project with no .codescout dir should still activate with memories: []
    let dir = tempdir().unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    let memories = result["memories"].as_array().unwrap();
    assert!(
        memories.is_empty(),
        "empty project should have empty memories array"
    );
}

#[tokio::test]
async fn workspace_action_activate_dispatches_to_activate_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = Workspace
        .call(
            json!({
                "action": "activate",
                "path": dir.path().to_str().unwrap(),
                "read_only": false,
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(result.get("project_hints").is_some());
}

#[tokio::test]
async fn workspace_action_status_dispatches_to_project_status() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let result = Workspace
        .call(json!({ "action": "status" }), &ctx)
        .await
        .unwrap();
    assert!(result["project_root"].is_string());
    assert!(result["languages"].is_array());
    assert!(result["index"].is_object());
}

#[tokio::test]
async fn workspace_action_list_projects_returns_workspace_field() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let result = Workspace
        .call(json!({ "action": "list_projects" }), &ctx)
        .await
        .unwrap();
    // The result must contain the "workspace" key (value may be null when no
    // workspace.toml is present — that's still a successful list_projects call).
    assert!(result.as_object().unwrap().contains_key("workspace"));
    // And no other fields should leak through.
    assert_eq!(result.as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn workspace_action_unknown_errors() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let err = Workspace
        .call(json!({ "action": "wat" }), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("unknown workspace action"),
        "expected unknown action error, got: {err}"
    );
}

#[tokio::test]
async fn activation_response_includes_stale_warning_when_no_stored_version() {
    // No project.toml → onboarding_version = None → stale
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let stale = &result["system_prompt_stale"];
    assert!(
        stale.is_object(),
        "system_prompt_stale missing; got: {result}"
    );
    assert!(
        stale["stored_version"].is_null(),
        "stored_version should be null for None"
    );
    assert_eq!(
        stale["current_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION as u64
    );
    assert!(
        stale["action"].as_str().unwrap().contains("refresh_prompt"),
        "action should mention refresh_prompt"
    );
}

#[tokio::test]
async fn activation_response_no_stale_warning_when_version_current() {
    let dir = tempdir().unwrap();
    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    // Write project.toml with current onboarding version
    std::fs::write(
        cs_dir.join("project.toml"),
        format!(
            "[project]\nname = \"test\"\nlanguages = []\nonboarding_version = {}\n",
            crate::tools::onboarding::ONBOARDING_VERSION
        ),
    )
    .unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    assert!(
        result["system_prompt_stale"].is_null(),
        "system_prompt_stale should be absent; got: {result}"
    );
}

#[tokio::test]
async fn activation_response_includes_stale_warning_when_version_outdated() {
    let dir = tempdir().unwrap();
    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    // Write project.toml with an outdated onboarding version
    std::fs::write(
        cs_dir.join("project.toml"),
        format!(
            "[project]\nname = \"test\"\nlanguages = []\nonboarding_version = {}\n",
            crate::tools::onboarding::ONBOARDING_VERSION.saturating_sub(1)
        ),
    )
    .unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let stale = &result["system_prompt_stale"];
    assert!(
        stale.is_object(),
        "system_prompt_stale missing; got: {result}"
    );
    assert_eq!(
        stale["stored_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION.saturating_sub(1) as u64,
        "stored_version should reflect the outdated version"
    );
    assert_eq!(
        stale["current_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION as u64
    );
}

#[test]
fn format_activate_project_prepends_warning_when_stale() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "system_prompt_stale": {
            "stored_version": 20,
            "current_version": 22,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.starts_with("⚠ SYSTEM PROMPT STALE (v20 → v22):"),
        "compact should start with stale warning but was: {compact}"
    );
    assert!(
        compact.contains("activated · my-project (rw)"),
        "compact should still contain activation summary but was: {compact}"
    );
}

#[test]
fn format_activate_project_no_warning_when_current() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        !compact.contains("STALE"),
        "no stale warning expected but was: {compact}"
    );
    assert_eq!(
        compact,
        "activated · my-project (rw) · 1 memories · index: not_indexed"
    );
}

#[test]
fn format_activate_project_prepends_warning_with_none_stored_version() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": [],
        "index": {"status": "not_indexed"},
        "system_prompt_stale": {
            "stored_version": null,
            "current_version": 22,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.starts_with("⚠ SYSTEM PROMPT STALE (none → v22):"),
        "should show 'none' not 'v0' for null stored_version; got: {compact}"
    );
}
