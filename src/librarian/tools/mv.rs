use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::artifact;

#[derive(Deserialize)]
struct Args {
    id: String,
    new_rel_path: String,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        super::RecoverableError::new(format!("move requires 'id' and 'new_rel_path': {e}"))
    })?;

    // Defense-in-depth: new_rel_path must stay within the resolved root. Reject
    // absolute paths and `..` segments so a move can never escape the project
    // even if root resolution is wrong (1a5acfc0).
    if a.new_rel_path.is_empty()
        || std::path::Path::new(&a.new_rel_path).components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(super::RecoverableError::new(format!(
            "new_rel_path '{}' must be a non-empty relative path with no '..' or absolute segments",
            a.new_rel_path
        )));
    }

    let cat = ctx.catalog.lock();
    let row = artifact::get(&cat, &a.id)?
        .ok_or_else(|| super::RecoverableError::new(format!("unknown id `{}`", a.id)))?;

    // Find the managed root that contains this artifact — a workspace
    // `[[roots]]` entry or the active project. `new_rel_path` is interpreted
    // relative to that root. See `super::managed_roots`.
    let roots = super::managed_roots(ctx);
    let root_path = super::containing_root(&roots, &row.abs_path)
        .ok_or_else(|| anyhow::anyhow!("no managed root contains {}", row.abs_path.display()))?;

    let old_full = row.abs_path.clone();
    let new_full = root_path.join(&a.new_rel_path);

    if new_full.exists() {
        return Err(super::RecoverableError::new(format!(
            "destination '{}' already exists — choose a different path or delete it first",
            a.new_rel_path
        )));
    }

    if let Some(parent) = new_full.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(&old_full, &new_full)?;

    let now = chrono::Utc::now().timestamp_millis();
    let file_mtime = std::fs::metadata(&new_full)
        .ok()
        .and_then(|m| {
            m.modified().ok().and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as i64)
            })
        })
        .unwrap_or(now);
    let content = std::fs::read_to_string(&new_full)?;
    let file_sha256 = crate::librarian::util::sha_of_bytes(content.as_bytes());

    let updated_row = crate::librarian::catalog::artifact::ArtifactRow {
        abs_path: new_full.clone(),
        updated_at: now,
        file_mtime,
        file_sha256,
        ..row.clone()
    };
    artifact::upsert(&cat, &updated_row)?;

    Ok(json!({
        "id": a.id,
        "old_abs_path": old_full.display().to_string(),
        "new_abs_path": new_full.display().to_string(),
        "moved": true
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::librarian::{
        catalog::{artifact, artifact::ArtifactRow, Catalog},
        tools::{mv, ToolContext},
        workspace::{Root, WorkspaceConfig},
    };

    fn mk_ctx(tmp: &std::path::Path) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();

        let row = ArtifactRow {
            id: "aabbccdd11223344".into(),
            abs_path: tmp.join("docs/trackers/foo.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Foo Tracker".into()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        };
        artifact::upsert(&cat, &row).unwrap();

        let src = tmp.join("docs/trackers/foo.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: aabbccdd11223344\nkind: tracker\n---\n# Foo\n",
        )
        .unwrap();

        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "test-repo".into(),
                    path: tmp.to_path_buf(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            artifact_store: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn move_renames_file_and_updates_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(result["old_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/trackers/foo.md"));
        assert!(result["new_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/archive/foo.md"));

        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!tmp.path().join("docs/trackers/foo.md").exists());

        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, "aabbccdd11223344").unwrap().unwrap();
        assert!(row.abs_path.ends_with("docs/archive/foo.md"));
    }

    #[tokio::test]
    async fn move_errors_if_destination_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let dst = tmp.path().join("docs/archive/foo.md");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&dst, "already here").unwrap();

        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn move_errors_on_unknown_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "deadbeefdeadbeef",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unknown id"));
    }

    #[tokio::test]
    async fn move_succeeds_for_active_project_absent_from_legacy_roots() {
        // Regression for docs/issues/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md
        // (mv shares delete's guard): under the `[[project]]` model the active project is in
        // `current_project`, not `workspace.roots`. `new_rel_path` must resolve relative to the
        // active project's git_root.
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = mk_ctx(tmp.path());
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = Some(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: tmp.path().to_path_buf(),
                git_root: tmp.path().to_path_buf(),
                umbrella: None,
            },
        ));

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!tmp.path().join("docs/trackers/foo.md").exists());
        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, "aabbccdd11223344").unwrap().unwrap();
        assert!(row.abs_path.ends_with("docs/archive/foo.md"));
    }

    #[tokio::test]
    async fn move_resolves_under_nested_project_not_ancestor_root() {
        // 1a5acfc0: active project nested under an ancestor [[roots]] entry.
        // The move must resolve against the nested project, not the ancestor.
        let tmp = tempfile::tempdir().unwrap();
        let ancestor = tmp.path().to_path_buf();
        let child = ancestor.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let mut ctx = mk_ctx(&child); // seeds artifact at child/docs/trackers/foo.md

        // Workspace registers the ANCESTOR as a legacy [[roots]] entry; the
        // active project is the nested child (its own repo), absent from roots.
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "ancestor".into(),
                path: ancestor.clone(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = Some(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: child.clone(),
                git_root: child.clone(),
                umbrella: None,
            },
        ));

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(
            child.join("docs/archive/foo.md").exists(),
            "move resolved under the nested active project"
        );
        assert!(
            !ancestor.join("docs/archive/foo.md").exists(),
            "move did NOT escape to the ancestor [[roots]] entry"
        );
    }

    #[tokio::test]
    async fn move_rejects_new_rel_path_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "../escape/foo.md"
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("..") || err.to_string().contains("relative"),
            "got: {err}"
        );
    }
}
