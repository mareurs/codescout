use crate::catalog::augmentation;
use crate::tools::{Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactRefreshCommit;

#[derive(Deserialize)]
struct Args {
    id: String,
}

#[async_trait]
impl Tool for ArtifactRefreshCommit {
    fn name(&self) -> &'static str {
        "artifact_refresh_commit"
    }

    fn description(&self) -> &'static str {
        "Signal that a refresh cycle is complete. Increments refresh_count and sets \
         last_refreshed_at. Call this after artifact_update in every refresh cycle. \
         No-ops gracefully if the augmentation row has been deleted."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "Artifact id" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();
        let found = augmentation::commit_refresh(&cat, &a.id)?;
        Ok(json!({ "committed": found }))
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

    fn seed(cat: &Catalog, id: &str) {
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
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn increments_count_and_sets_timestamp() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let ctx = mk_ctx(cat);
        let r = ArtifactRefreshCommit
            .call(&ctx, json!({"id": "a1"}))
            .await
            .unwrap();
        assert_eq!(r["committed"], true);
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a1").unwrap().unwrap();
        assert_eq!(row.refresh_count, 1);
        assert!(row.last_refreshed_at.is_some());
    }

    #[tokio::test]
    async fn missing_row_returns_committed_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        let r = ArtifactRefreshCommit
            .call(&ctx, json!({"id": "nope"}))
            .await
            .unwrap();
        assert_eq!(r["committed"], false);
    }
}
