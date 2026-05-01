use crate::catalog::augmentation;
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactUpdateParams;

#[derive(Deserialize)]
struct Args {
    id: String,
    params: Value,
}

#[async_trait]
impl Tool for ArtifactUpdateParams {
    fn name(&self) -> &'static str {
        "artifact_update_params"
    }

    fn description(&self) -> &'static str {
        "Merge-patch the params JSON of an augmented artifact (RFC 7396). \
         Keys set to null are deleted; present keys are merged. \
         Call this mid-session to tune gather sources without touching the prompt or body."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "params"],
            "properties": {
                "id": { "type": "string" },
                "params": {
                    "type": "object",
                    "description": "Partial params to merge. Set a key to null to delete it."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();
        let found = augmentation::merge_params(&cat, &a.id, &a.params)?;
        if !found {
            return Err(RecoverableError::new(format!(
                "no augmentation for artifact '{}' — call artifact_augment first",
                a.id
            )));
        }
        Ok(json!("ok"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{artifact, augmentation, Catalog};
    use crate::workspace::WorkspaceConfig;
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    fn seed(cat: &Catalog, id: &str, params: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        artifact::upsert(
            cat,
            &artifact::ArtifactRow {
                id: id.to_string(),
                repo: "r".to_string(),
                rel_path: format!("{id}.md"),
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: None,
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
        augmentation::upsert(
            cat,
            &augmentation::AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "p".to_string(),
                params: params.to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn merge_adds_key() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1", r#"{"format":"bullets"}"#);
        let ctx = mk_ctx(cat);
        ArtifactUpdateParams
            .call(&ctx, json!({"id": "a1", "params": {"max_tokens": 2000}}))
            .await
            .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a1").unwrap().unwrap();
        let p: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(p["format"], "bullets");
        assert_eq!(p["max_tokens"], 2000);
    }

    #[tokio::test]
    async fn null_deletes_key() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1", r#"{"format":"table","max_tokens":3000}"#);
        let ctx = mk_ctx(cat);
        ArtifactUpdateParams
            .call(&ctx, json!({"id": "a1", "params": {"format": null}}))
            .await
            .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a1").unwrap().unwrap();
        let p: Value = serde_json::from_str(&row.params).unwrap();
        assert!(p.get("format").is_none());
        assert_eq!(p["max_tokens"], 3000);
    }

    #[tokio::test]
    async fn missing_augmentation_returns_recoverable() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        let err = ArtifactUpdateParams
            .call(&ctx, json!({"id": "nope", "params": {}}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
