use crate::librarian::catalog::{artifact, augmentation};
use crate::librarian::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactAugment;

#[derive(Deserialize)]
struct Args {
    id: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    render_template: Option<String>,
    #[serde(default)]
    params_schema: Option<Value>,
    #[serde(default)]
    merge: bool,
    #[serde(default)]
    append_mode: Option<bool>,
    #[serde(default)]
    history_cap: Option<usize>,
}

#[async_trait]
impl Tool for ArtifactAugment {
    fn name(&self) -> &'static str {
        "artifact_augment"
    }

    fn description(&self) -> &'static str {
        "Attach or replace a persistent prompt + params on any artifact (merge=false, default), \
         or RFC 7396 merge-patch params only without changing the prompt (merge=true). \
         Idempotent — safe to call on already-augmented artifacts. \
         Replaces artifact_update_params."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "Artifact id" },
                "prompt": {
                    "type": "string",
                    "description": "Required when merge=false. Persistent instruction: what to maintain and how to format it."
                },
                "params": {
                    "type": "object",
                    "description": "The data params payload on the augmentation row. On merge=false (default — create/replace), fully replaces existing params. On merge=true, RFC 7396 merge-patched into existing params. NOT gather config — gather behavior is controlled by gather_from/format/max_tokens fields written into the params payload itself by callers that need them."
                },
                "render_template": {
                    "type": "string",
                    "description": "Optional MiniJinja template projecting `params` into a markdown snippet rendered into librarian_context output. Decouples live state from prose body."
                },
                "params_schema": {
                    "type": "object",
                    "description": "Optional JSON Schema validating params on every merge. Initial params are also validated."
                },
                "merge": {
                    "type": "boolean",
                    "description": "When true, apply RFC 7396 merge-patch to params only — prompt is not required. Requires an existing augmentation."
                },
                "append_mode": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true, artifact_update prepends a new dated section instead of replacing the body. Prompt should instruct the LLM to write only the new delta block."
                },
                "history_cap": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Max number of dated ## YYYY-MM-DD sections to retain. Oldest sections beyond cap are dropped on each append."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        // D11: when the gate ran and passed, capture evidence to emit a
        // `note` event after the catalog lock is released (event_create is
        // async and acquires its own lock).
        let mut gate_check_evidence: Option<Value> = None;

        if a.merge {
            // Scope the catalog lock so it's dropped before the async
            // event_create call below (parking_lot MutexGuard is !Send).
            {
                let cat = ctx.catalog.lock();
                let patch = a
                    .params
                    .as_ref()
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                if let Some(existing) = augmentation::get(&cat, &a.id)? {
                    let mut current: Value = serde_json::from_str(&existing.params)
                        .unwrap_or(Value::Object(Default::default()));
                    let pre_status = current
                        .get("status")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    augmentation::apply_merge_patch(&mut current, &patch);

                    if let Some(schema_text) = existing.params_schema.as_deref() {
                        crate::librarian::tools::schema_validate::validate_against_stored(
                            schema_text,
                            &current,
                        )
                        .map_err(|e| {
                            RecoverableError::new(format!(
                                "merged params violate params_schema: {e}"
                            ))
                        })?;
                    }

                    let post_status = current.get("status").and_then(|s| s.as_str());
                    let is_goal_tracker = current.get("acceptance_signals").is_some()
                        && current.get("children").is_some();

                    if is_goal_tracker {
                        use crate::librarian::tools::goal_aggregation::validate_scope_growth;
                        let pre_existing: Value = serde_json::from_str(&existing.params)
                            .unwrap_or(Value::Object(Default::default()));
                        let empty_vec: Vec<Value> = Vec::new();
                        let prior_children: &[Value] = pre_existing
                            .get("children")
                            .and_then(|c| c.as_array())
                            .map(Vec::as_slice)
                            .unwrap_or(&empty_vec);
                        let submitted_children: &[Value] = current
                            .get("children")
                            .and_then(|c| c.as_array())
                            .map(Vec::as_slice)
                            .unwrap_or(&empty_vec);
                        if let Err(e) = validate_scope_growth(prior_children, submitted_children) {
                            return Err(RecoverableError::new(format!("{e}")));
                        }
                    }

                    if is_goal_tracker
                        && pre_status.as_deref() != Some("done")
                        && post_status == Some("done")
                    {
                        use crate::librarian::tools::goal_aggregation::{
                            evaluate_gate, GateOutcome,
                        };
                        match evaluate_gate(&current) {
                            GateOutcome::AutoClose => {
                                let children = current
                                    .get("children")
                                    .and_then(|c| c.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                let signals = current
                                    .get("acceptance_signals")
                                    .and_then(|s| s.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                let children_done = children
                                    .iter()
                                    .filter(|c| {
                                        c.get("status").and_then(|s| s.as_str()) == Some("done")
                                    })
                                    .count();
                                let signals_met = signals
                                    .iter()
                                    .filter(|s| {
                                        s.get("met").and_then(|m| m.as_bool()) == Some(true)
                                    })
                                    .count();
                                gate_check_evidence = Some(json!({
                                    "tag": "gate_check",
                                    "gate_passed": true,
                                    "text": format!(
                                        "auto-close gate passed: {}/{} children done, {}/{} signals met",
                                        children_done, children.len(),
                                        signals_met, signals.len()
                                    ),
                                    "evidence": {
                                        "children_count": children.len(),
                                        "children_done": children_done,
                                        "signal_count_total": signals.len(),
                                        "signal_count_met": signals_met,
                                    },
                                    "refresh_at": chrono::Utc::now().to_rfc3339(),
                                }));
                            }
                            GateOutcome::Block(reason) => {
                                return Err(RecoverableError::new(format!(
                                    "goal auto-close gate blocked: {reason}"
                                )));
                            }
                        }
                    }
                }
                let found = augmentation::merge_params(&cat, &a.id, &patch)?;
                if !found {
                    return Err(RecoverableError::new(format!(
                        "no augmentation for artifact '{}' — call artifact_augment first",
                        a.id
                    )));
                }
            } // cat dropped here

            // D11 — emit gate_check note event after the catalog lock is
            // released. Best-effort: if event emission fails, the augment
            // itself still succeeded.
            if let Some(payload) = gate_check_evidence {
                let _ = crate::librarian::tools::event_create::call(
                    ctx,
                    json!({
                        "artifact_id": &a.id,
                        "kind": "note",
                        "payload": payload,
                    }),
                )
                .await;
            }

            return Ok(json!("ok"));
        }

        let cat = ctx.catalog.lock();

        // Create/replace path — prompt is required
        let prompt = a.prompt.ok_or_else(|| {
            RecoverableError::new("prompt is required (set merge=true to patch params only)")
        })?;

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

        if let Some(schema) = &a.params_schema {
            let parsed_params: Value = serde_json::from_str(&params_str)?;
            crate::librarian::tools::schema_validate::validate(schema, &parsed_params).map_err(
                |e| RecoverableError::new(format!("initial params violate params_schema: {e}")),
            )?;
        }

        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        augmentation::upsert(
            &cat,
            &augmentation::AugmentationRow {
                artifact_id: a.id.clone(),
                prompt,
                params: params_str,
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now.clone(),
                updated_at: now,
                render_template: a.render_template,
                params_schema: params_schema_str,
                append_mode: a.append_mode.unwrap_or(false),
                history_cap: a.history_cap.map(|v| v as i64),
            },
        )?;

        Ok(json!("ok"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, augmentation, Catalog};
    use crate::librarian::workspace::WorkspaceConfig;
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
                abs_path: std::path::PathBuf::from(format!("/test/repo/{id}.md")),
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

    #[tokio::test]
    async fn merge_true_patches_params_without_touching_prompt() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "aug-1");
        // First, augment with a prompt and initial params
        ArtifactAugment
            .call(
                &ctx,
                json!({"id": "aug-1", "prompt": "do stuff", "params": {"a": 1, "b": 2}}),
            )
            .await
            .unwrap();

        // Now merge-patch: add c, delete b
        ArtifactAugment
            .call(
                &ctx,
                json!({"id": "aug-1", "merge": true, "params": {"c": 3, "b": null}}),
            )
            .await
            .unwrap();

        let cat = ctx.catalog.lock();
        let aug = crate::librarian::catalog::augmentation::get(&cat, "aug-1")
            .unwrap()
            .unwrap();
        assert_eq!(aug.prompt, "do stuff", "prompt must be unchanged");
        let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["a"], 1, "a must survive merge");
        assert_eq!(params["c"], 3, "c must be added");
        assert!(
            params.get("b").map(|v| v.is_null()).unwrap_or(true),
            "b must be deleted"
        );
    }

    #[tokio::test]
    async fn merge_true_without_existing_augmentation_errors() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "aug-2");
        let err = ArtifactAugment
            .call(
                &ctx,
                json!({"id": "aug-2", "merge": true, "params": {"x": 1}}),
            )
            .await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.contains("artifact_augment"),
            "error must mention artifact_augment"
        );
    }

    #[tokio::test]
    async fn non_merge_without_prompt_errors() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "aug-3");
        let err = ArtifactAugment
            .call(&ctx, json!({"id": "aug-3", "params": {"x": 1}}))
            .await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("prompt"), "error must mention prompt");
    }

    #[tokio::test]
    async fn persists_append_mode_and_history_cap() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "a99");
        ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": "a99",
                    "prompt": "track me",
                    "append_mode": true,
                    "history_cap": 10,
                }),
            )
            .await
            .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a99").unwrap().unwrap();
        assert!(row.append_mode);
        assert_eq!(row.history_cap, Some(10));
    }

    #[tokio::test]
    async fn append_mode_defaults_to_false_when_absent() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "a100");
        ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({"id": "a100", "prompt": "no append"}),
            )
            .await
            .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a100").unwrap().unwrap();
        assert!(!row.append_mode);
        assert_eq!(row.history_cap, None);
    }

    // =================================================================
    // D11 — gate_check note event emission
    // =================================================================

    #[tokio::test]
    async fn gate_check_note_event_emitted_on_autoclose() {
        let ctx = mk_ctx();
        // Seed goal with two done children + two met signals, status=active.
        let goal_id = "g-pass";
        seed_artifact(&ctx, goal_id);
        let _ = ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": goal_id,
                    "prompt": "p",
                    "params": {
                        "criterion": "x",
                        "status": "active",
                        "acceptance_signals": [
                            {"description":"A","met":true,"kind":"freeform"},
                            {"description":"B","met":true,"kind":"freeform"}
                        ],
                        "children": [
                            {"id":"C-1","artifact_id":"a","title":"A","archetype":"task_list","status":"done"},
                            {"id":"C-2","artifact_id":"b","title":"B","archetype":"task_list","status":"done"}
                        ]
                    }
                }),
            )
            .await
            .unwrap();

        // Flip status to done — gate passes, note event must emit.
        ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": goal_id,
                    "merge": true,
                    "params": {"status": "done"}
                }),
            )
            .await
            .unwrap();

        // Inspect events for this artifact.
        use crate::librarian::catalog::events::timeline_for_artifact;
        let cat = ctx.catalog.lock();
        let events = timeline_for_artifact(&cat, goal_id, None, None, 50).unwrap();
        let gate_notes: Vec<_> = events
            .iter()
            .filter(|e| {
                e.kind == "note"
                    && serde_json::from_str::<serde_json::Value>(&e.payload)
                        .ok()
                        .and_then(|p| p.get("tag").and_then(|t| t.as_str()).map(String::from))
                        .as_deref()
                        == Some("gate_check")
            })
            .collect();
        assert_eq!(
            gate_notes.len(),
            1,
            "expected exactly one gate_check note event"
        );
        let payload: serde_json::Value = serde_json::from_str(&gate_notes[0].payload).unwrap();
        assert_eq!(payload["gate_passed"], true);
        assert_eq!(payload["evidence"]["children_count"], 2);
        assert_eq!(payload["evidence"]["children_done"], 2);
        assert_eq!(payload["evidence"]["signal_count_total"], 2);
        assert_eq!(payload["evidence"]["signal_count_met"], 2);
    }

    #[tokio::test]
    async fn gate_check_event_not_emitted_when_gate_blocks() {
        let ctx = mk_ctx();
        let goal_id = "g-block";
        seed_artifact(&ctx, goal_id);
        // Seed with 1 child (too few — D9 blocks the gate).
        ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": goal_id,
                    "prompt": "p",
                    "params": {
                        "criterion": "x",
                        "status": "active",
                        "acceptance_signals": [{"description":"A","met":true,"kind":"freeform"}],
                        "children": [
                            {"id":"C-1","artifact_id":"a","title":"A","archetype":"task_list","status":"done"}
                        ]
                    }
                }),
            )
            .await
            .unwrap();

        // Attempt to flip status to done — gate blocks.
        let res = ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": goal_id,
                    "merge": true,
                    "params": {"status": "done"}
                }),
            )
            .await;
        assert!(res.is_err(), "expected gate to block status flip");

        use crate::librarian::catalog::events::timeline_for_artifact;
        let cat = ctx.catalog.lock();
        let events = timeline_for_artifact(&cat, goal_id, None, None, 50).unwrap();
        let gate_notes: Vec<_> = events
            .iter()
            .filter(|e| {
                e.kind == "note"
                    && serde_json::from_str::<serde_json::Value>(&e.payload)
                        .ok()
                        .and_then(|p| p.get("tag").and_then(|t| t.as_str()).map(String::from))
                        .as_deref()
                        == Some("gate_check")
            })
            .collect();
        assert_eq!(
            gate_notes.len(),
            0,
            "expected NO gate_check note event when gate blocks: {gate_notes:?}"
        );

        // Suppress unused warning.
        let _: i32 = 0;
    }
}
