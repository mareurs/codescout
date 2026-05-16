use crate::librarian::catalog::augmentation;
use crate::librarian::tools::gather::{gather_all, GatherSource};
use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Deserialize)]
struct Args {
    id: String,
}

fn read_body(ctx: &ToolContext, artifact_id: &str) -> Result<Option<String>> {
    let cat = ctx.catalog.lock();
    let row = match crate::librarian::catalog::artifact::get(&cat, artifact_id)? {
        Some(r) => r,
        None => return Ok(None),
    };
    let full_path = row.abs_path.clone();
    match std::fs::read_to_string(&full_path) {
        Ok(s) => Ok(Some(s)),
        Err(_) => Ok(None),
    }
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
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

    let (results, warnings) = gather_all(&sources, ctx, aug.last_refreshed_at.as_deref()).await?;

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

    let mut out = json!({
        "artifact_id": a.id,
        "prompt": aug.prompt,
        "params": params,
        "current_body": current_body,
        "context": context,
        "last_refreshed_at": aug.last_refreshed_at,
        "hints": hints,
    });
    if aug.append_mode {
        out["append_mode"] = json!(true);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::Tool;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: tmp_root,
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
    async fn refresh_includes_append_mode_hint_when_set() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "hint_test.md",
                "kind": "spec",
                "title": "hint test",
                "body": "body",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        crate::librarian::tools::augment::ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": id,
                    "prompt": "track",
                    "append_mode": true,
                }),
            )
            .await
            .unwrap();

        let result = call(&ctx, serde_json::json!({"id": id})).await.unwrap();
        assert_eq!(result["append_mode"], serde_json::json!(true));
    }
}
