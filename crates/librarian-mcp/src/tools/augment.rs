use crate::catalog::{artifact, augmentation};
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactAugment;

#[derive(Deserialize)]

struct Args {
    id: String,
    prompt: String,
    params: Option<Value>,
    /// MiniJinja template projecting `params` into a markdown snippet.
    /// Decouples live state from prose body.
    render_template: Option<String>,
    /// JSON Schema validating future `params` merges.
    params_schema: Option<Value>,
}

#[async_trait]
impl Tool for ArtifactAugment {
    fn name(&self) -> &'static str {
        "artifact_augment"
    }

    fn description(&self) -> &'static str {
        "Attach or replace a persistent prompt + params on any artifact, enabling \
         server-assisted refresh. Idempotent — safe to call on already-augmented artifacts."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "prompt"],
            "properties": {
                "id": { "type": "string", "description": "Artifact id" },
                "prompt": {
                    "type": "string",
                    "description": "Persistent instruction: what to maintain and how to format it"
                },
                "params": {
                    "type": "object",
                    "description": "Optional gather config (gather_from, format, max_tokens). Defaults to {}."
                },
                "render_template": {
                    "type": "string",
                    "description": "Optional MiniJinja template projecting `params` into a markdown snippet rendered into librarian_context output. Decouples live state from prose body."
                },
                "params_schema": {
                    "type": "object",
                    "description": "Optional JSON Schema validating params on every merge. Initial params are also validated."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();

        if artifact::get(&cat, &a.id)?.is_none() {
            return Err(RecoverableError::new(format!(
                "artifact '{}' not found",
                a.id
            )));
        }

        let params_str = a
            .params
            .map(|p| serde_json::to_string(&p))
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        let params_schema_str = a
            .params_schema
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        // If a schema is supplied, validate the initial params against it.
        if let Some(schema) = &a.params_schema {
            let parsed_params: Value = serde_json::from_str(&params_str)?;
            crate::tools::schema_validate::validate(schema, &parsed_params).map_err(|e| {
                RecoverableError::new(format!("initial params violate params_schema: {e}"))
            })?;
        }

        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        augmentation::upsert(
            &cat,
            &augmentation::AugmentationRow {
                artifact_id: a.id.clone(),
                prompt: a.prompt,
                params: params_str,
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now.clone(),
                updated_at: now,
                render_template: a.render_template,
                params_schema: params_schema_str,
            },
        )?;

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

    fn mk_ctx() -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
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

    fn seed_artifact(ctx: &ToolContext, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        artifact::upsert(
            &cat,
            &artifact::ArtifactRow {
                id: id.to_string(),
                repo: "repo".to_string(),
                rel_path: format!("{id}.md"),
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
    }

    #[tokio::test]
    async fn creates_augmentation_row() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "art1");
        let result = ArtifactAugment
            .call(
                &ctx,
                json!({
                    "id": "art1",
                    "prompt": "Keep me updated",
                    "params": {"format": "table"}
                }),
            )
            .await
            .unwrap();
        assert_eq!(result, json!("ok"));
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "Keep me updated");
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["format"], "table");
    }

    #[tokio::test]
    async fn idempotent_update_replaces_prompt() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "art1");
        ArtifactAugment
            .call(&ctx, json!({"id": "art1", "prompt": "Old"}))
            .await
            .unwrap();
        ArtifactAugment
            .call(&ctx, json!({"id": "art1", "prompt": "New"}))
            .await
            .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "New");
    }

    #[tokio::test]
    async fn missing_artifact_returns_recoverable_error() {
        let ctx = mk_ctx();
        let err = ArtifactAugment
            .call(&ctx, json!({"id": "nope", "prompt": "Test"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn persists_render_template_and_params_schema() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "rt-art");
        ArtifactAugment
            .call(
                &ctx,
                json!({
                    "id": "rt-art",
                    "prompt": "p",
                    "render_template": "**Status:** {{ status }}",
                    "params_schema": {
                        "type": "object",
                        "properties": { "status": { "type": "string" } }
                    },
                    "params": { "status": "green" }
                }),
            )
            .await
            .unwrap();
        let row = augmentation::get(&ctx.catalog.lock(), "rt-art")
            .unwrap()
            .unwrap();
        assert_eq!(
            row.render_template.as_deref(),
            Some("**Status:** {{ status }}")
        );
        assert!(row.params_schema.as_deref().unwrap().contains("\"status\""));
    }

    #[tokio::test]
    async fn rejects_initial_params_violating_schema() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "bad-init");
        let err = ArtifactAugment
            .call(
                &ctx,
                json!({
                    "id": "bad-init",
                    "prompt": "p",
                    "params_schema": {
                        "type": "object",
                        "required": ["status"],
                        "properties": { "status": { "type": "string" } }
                    },
                    "params": {}
                }),
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("violate params_schema"),
            "got: {err}"
        );
    }
}
