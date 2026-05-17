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

    // Goal-tracker injection (Yak variant (b)): if this artifact's params
    // describe a goal-tracker (has `acceptance_signals` AND `children`),
    // synthesize `deterministic_child_statuses` by running
    // `goal_aggregation::child_status_pure` on each linked child. The LLM
    // reads ground truth from context rather than re-deriving rule 1.
    let is_goal_tracker = params.is_object()
        && params.get("acceptance_signals").is_some()
        && params.get("children").is_some();
    if is_goal_tracker {
        let children_tuples: Vec<(String, String, String)> = params
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let id = c.get("id")?.as_str()?.to_string();
                        let aid = c.get("artifact_id")?.as_str()?.to_string();
                        let arch = c
                            .get("archetype")
                            .and_then(|a| a.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some((id, aid, arch))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !children_tuples.is_empty() {
            let parent_signals: Vec<crate::librarian::tools::goal_aggregation::AcceptanceSignal> =
                params
                    .get("acceptance_signals")
                    .and_then(|s| serde_json::from_value(s.clone()).ok())
                    .unwrap_or_default();
            let det = crate::librarian::tools::gather::gather_goal_children(
                ctx,
                &children_tuples,
                &parent_signals,
            )?;
            context.insert("deterministic_child_statuses".to_string(), det);
        }
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

    #[tokio::test]
    async fn refresh_injects_deterministic_child_statuses_for_goal_tracker() {
        use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
        use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};

        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Helper closures (avoid module-private dependencies for sample data).
        let mk_art = |id: &str| ArtifactRow {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: "tracker".to_string(),
            status: "active".to_string(),
            title: Some(id.to_string()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "x".to_string(),
            confidence: 1.0,
        };
        let mk_aug = |aid: &str, params_json: &str| AugmentationRow {
            artifact_id: aid.to_string(),
            prompt: "p".to_string(),
            params: params_json.to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
        };

        // Two children: a failure_table (all-pass → done) and a task_list (empty → pending).
        {
            let cat = ctx.catalog.lock();
            art_upsert(&cat, &mk_art("child-a")).unwrap();
            aug_upsert(
                &cat,
                &mk_aug("child-a", r#"{"failures":[{"id":"F-1","status":"pass"}]}"#),
            )
            .unwrap();
            art_upsert(&cat, &mk_art("child-b")).unwrap();
            aug_upsert(&cat, &mk_aug("child-b", r#"{"tasks":[]}"#)).unwrap();

            // Parent goal: structurally a goal-tracker (has acceptance_signals + children).
            art_upsert(&cat, &mk_art("goal-1")).unwrap();
            let goal_params = serde_json::json!({
                "criterion": "All children done",
                "status": "active",
                "acceptance_signals": [],
                "children": [
                    {"id": "C-1", "artifact_id": "child-a", "title": "A",
                     "archetype": "failure_table", "status": "in-progress"},
                    {"id": "C-2", "artifact_id": "child-b", "title": "B",
                     "archetype": "task_list", "status": "pending"}
                ]
            });
            aug_upsert(&cat, &mk_aug("goal-1", &goal_params.to_string())).unwrap();
        }

        let result = call(&ctx, serde_json::json!({"id": "goal-1"}))
            .await
            .unwrap();

        // The context should carry deterministic_child_statuses with both children resolved.
        let det = &result["context"]["deterministic_child_statuses"];
        assert!(
            det.is_array(),
            "deterministic_child_statuses missing or not array: {result:#}"
        );
        let arr = det.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["child_id"], "C-1");
        assert_eq!(arr[0]["status"], "done");
        assert_eq!(arr[0]["basis"], "deterministic");
        assert_eq!(arr[1]["child_id"], "C-2");
        assert_eq!(arr[1]["status"], "pending");
    }

    #[tokio::test]
    async fn refresh_skips_deterministic_injection_for_non_goal_tracker() {
        use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
        use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};

        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // A regular task_list tracker — has children-shaped params but no acceptance_signals.
        // Should NOT trigger goal-tracker injection.
        let mk_art = |id: &str| ArtifactRow {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: "tracker".to_string(),
            status: "active".to_string(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "x".to_string(),
            confidence: 1.0,
        };
        {
            let cat = ctx.catalog.lock();
            art_upsert(&cat, &mk_art("plain")).unwrap();
            aug_upsert(
                &cat,
                &AugmentationRow {
                    artifact_id: "plain".to_string(),
                    prompt: "p".to_string(),
                    params: r#"{"tasks":[{"id":"T-1","status":"done"}]}"#.to_string(),
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: "2026-01-01T00:00:00.000Z".to_string(),
                    updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                    render_template: None,
                    params_schema: None,
                    append_mode: false,
                    history_cap: None,
                },
            )
            .unwrap();
        }

        let result = call(&ctx, serde_json::json!({"id": "plain"}))
            .await
            .unwrap();
        assert!(
            result["context"]["deterministic_child_statuses"].is_null()
                || !result["context"]
                    .as_object()
                    .unwrap()
                    .contains_key("deterministic_child_statuses"),
            "non-goal tracker should not receive deterministic_child_statuses: {result:#}"
        );
    }
}
