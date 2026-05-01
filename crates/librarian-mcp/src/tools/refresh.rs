use crate::catalog::augmentation;
use crate::tools::gather::{gather_all, GatherSource};
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ArtifactRefresh;

#[derive(Deserialize)]
struct Args {
    id: String,
}

#[async_trait]
impl Tool for ArtifactRefresh {
    fn name(&self) -> &'static str {
        "artifact_refresh"
    }

    fn description(&self) -> &'static str {
        "Gather context for an augmented artifact and return a refresh package \
         (prompt + current_body + gathered context). Does NOT write anything — \
         synthesize new content from the package then call artifact_update to write back, \
         then artifact_update(id, commit_refresh=true) to record the refresh."
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

        let aug_row = {
            let cat = ctx.catalog.lock();
            augmentation::get(&cat, &a.id)?
        };

        let aug = aug_row.ok_or_else(|| {
            RecoverableError::new(format!(
                "no augmentation for artifact '{}' — call artifact_augment first",
                a.id
            ))
        })?;

        let params: Value = serde_json::from_str(&aug.params).unwrap_or_else(|_| json!({}));
        let sources: Vec<GatherSource> = params
            .get("gather_from")
            .and_then(|g| serde_json::from_value(g.clone()).ok())
            .unwrap_or_default();

        let (results, warnings) =
            gather_all(&sources, ctx, aug.last_refreshed_at.as_deref()).await?;

        let mut context: HashMap<String, Value> = HashMap::new();
        for r in results {
            context
                .entry(r.source_key.clone())
                .and_modify(|existing| {
                    if let (Value::Array(a), Value::Array(b)) = (existing, &r.data) {
                        a.extend(b.clone());
                    }
                })
                .or_insert(r.data);
        }
        if !warnings.is_empty() {
            context.insert("warnings".to_string(), json!(warnings));
        }

        let current_body = read_body(ctx, &a.id)?;

        let mut hints: Vec<String> = Vec::new();
        for (key, val) in &context {
            if key == "warnings" {
                continue;
            }
            if let Some(arr) = val.as_array() {
                hints.push(format!("{} items gathered from {key}", arr.len()));
            }
        }

        Ok(json!({
            "artifact_id": a.id,
            "prompt": aug.prompt,
            "params": params,
            "current_body": current_body,
            "context": context,
            "last_refreshed_at": aug.last_refreshed_at,
            "hints": hints,
        }))
    }
}

fn read_body(ctx: &ToolContext, artifact_id: &str) -> Result<Option<String>> {
    let cat = ctx.catalog.lock();
    let row = match crate::catalog::artifact::get(&cat, artifact_id)? {
        Some(r) => r,
        None => return Ok(None),
    };
    let root_map: HashMap<String, std::path::PathBuf> = ctx
        .workspace
        .roots
        .iter()
        .map(|r| (r.name.clone(), r.path.clone()))
        .collect();
    let Some(root) = root_map.get(&row.repo) else {
        return Ok(None);
    };
    let full_path = root.join(&row.rel_path);
    match std::fs::read_to_string(&full_path) {
        Ok(s) => Ok(Some(s)),
        Err(_) => Ok(None),
    }
}
