use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::artifact;

#[derive(Deserialize)]
struct Args {
    id: String,
}

/// Delete an artifact: remove its file from disk and its catalog row.
///
/// The catalog delete cascades (FK `ON DELETE CASCADE`, with
/// `PRAGMA foreign_keys = ON`) to the artifact's augmentation, links,
/// observations, and events, and the `artifact_vec` trigger drops its
/// embedding — so no orphaned rows remain (closes metadata-filtering F-6,
/// which noted that `rm` + `reindex` left the catalog-only augmentation
/// behind). The artifact must live under a managed workspace root; out-of-tree
/// paths are refused. A missing file is not fatal — the catalog row is still
/// dropped, so `delete` also repairs a stale entry for an already-removed file.
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)
        .map_err(|e| super::RecoverableError::new(format!("delete requires 'id': {e}")))?;

    let cat = ctx.catalog.lock();
    let row = artifact::get(&cat, &a.id)?
        .ok_or_else(|| super::RecoverableError::new(format!("unknown id `{}`", a.id)))?;

    // Guard: only delete artifacts under a managed workspace root (mirrors mv).
    let abs_path = row.abs_path.clone();
    if !ctx
        .workspace
        .roots
        .iter()
        .any(|r| abs_path.starts_with(&r.path))
    {
        return Err(super::RecoverableError::new(format!(
            "artifact '{}' is outside every workspace root — refusing to delete {}",
            a.id,
            abs_path.display()
        )));
    }

    // Remove the file. A missing file is not fatal — still drop the catalog row
    // so a stale entry for an already-deleted file is cleaned up.
    match std::fs::remove_file(&abs_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(anyhow::anyhow!(
                "failed to remove {}: {e}",
                abs_path.display()
            ))
        }
    }

    let existed = artifact::delete(&cat, &a.id)?;

    Ok(json!({
        "id": a.id,
        "deleted_abs_path": abs_path.display().to_string(),
        "deleted": existed,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::librarian::{
        catalog::{
            artifact,
            artifact::ArtifactRow,
            augmentation::{self, AugmentationRow},
            Catalog,
        },
        tools::{delete, ToolContext},
        workspace::{Root, WorkspaceConfig},
    };

    const ID: &str = "dddd11112222eeee";

    fn mk_ctx(tmp: &std::path::Path) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();

        let row = ArtifactRow {
            id: ID.into(),
            abs_path: tmp.join("docs/trackers/doomed.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Doomed Tracker".into()),
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

        // Attach an augmentation to prove the FK ON DELETE CASCADE drops it.
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: ID.into(),
                prompt: "maintain".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "0".into(),
                updated_at: "0".into(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
            },
        )
        .unwrap();

        let src = tmp.join("docs/trackers/doomed.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: dddd11112222eeee\nkind: tracker\n---\n# Doomed\n",
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
            current_project: None,
        }
    }

    #[tokio::test]
    async fn delete_removes_file_catalog_row_and_augmentation() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let file = tmp.path().join("docs/trackers/doomed.md");
        assert!(file.exists());

        let result = delete::call(&ctx, serde_json::json!({"action": "delete", "id": ID}))
            .await
            .unwrap();
        assert_eq!(result["deleted"], true);
        assert!(result["deleted_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/trackers/doomed.md"));

        assert!(!file.exists(), "file should be removed");
        let cat = ctx.catalog.lock();
        assert!(
            artifact::get(&cat, ID).unwrap().is_none(),
            "catalog row should be gone"
        );
        assert!(
            augmentation::get(&cat, ID).unwrap().is_none(),
            "augmentation should cascade-delete"
        );
    }

    #[tokio::test]
    async fn delete_missing_file_still_drops_catalog_row() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        std::fs::remove_file(tmp.path().join("docs/trackers/doomed.md")).unwrap();

        let result = delete::call(&ctx, serde_json::json!({"id": ID}))
            .await
            .unwrap();
        assert_eq!(result["deleted"], true);
        let cat = ctx.catalog.lock();
        assert!(artifact::get(&cat, ID).unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_unknown_id_is_recoverable_error() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let err = delete::call(&ctx, serde_json::json!({"id": "nope"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown id"), "got: {err}");
    }
}
