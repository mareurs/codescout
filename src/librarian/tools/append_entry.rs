use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::augmentation;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    id: String,
    entry_collection: String,
    id_prefix: String,
    #[serde(default = "default_entry")]
    entry: Value,
}

fn default_entry() -> Value {
    json!({})
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    if !a.entry.is_object() {
        return Err(RecoverableError::new(
            "append_entry: `entry` must be a JSON object",
        ));
    }
    let mut cat = ctx.catalog.lock();
    let id =
        augmentation::append_entry(&mut cat, &a.id, &a.entry_collection, &a.id_prefix, a.entry)?;
    Ok(json!({"id": id}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            lsp: crate::lsp::MockLspProvider::with_client(crate::lsp::MockLspClient::default()),
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
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

    fn seed(ctx: &ToolContext, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        art_upsert(
            &cat,
            &ArtifactRow {
                id: id.to_string(),
                abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: Some("T".to_string()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "x".to_string(),
                confidence: 1.0,
            },
        )
        .unwrap();
        aug_upsert(
            &cat,
            &AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "test".to_string(),
                params: r#"{"failures":[]}"#.to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("failures".to_string()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn call_assigns_and_returns_next_id() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let result = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {"status": "fail"}
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-1");
    }

    #[tokio::test]
    async fn call_rejects_non_object_entry() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let err = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": "not an object"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn call_missing_artifact_returns_recoverable_error() {
        let ctx = mk_ctx();

        let err = call(
            &ctx,
            json!({
                "id": "nope",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {}
            }),
        )
        .await
        .unwrap_err();

        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
