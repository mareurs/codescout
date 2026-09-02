use crate::librarian::catalog::{artifact, augmentation};
use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    id: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    /// Filesystem path to a JSON file holding the params payload, read
    /// server-side. Use when params are too large to pass inline (~9 KB cap).
    /// Mutually exclusive with `params`.
    #[serde(default)]
    params_path: Option<String>,
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
    #[serde(default)]
    entry_collection: Option<String>,
}

/// Validate merged params against the effective schema — the new schema if this
/// call provides one, otherwise the stored schema. No-op when neither is present.
fn validate_merged_against_schema(
    current: &Value,
    new_schema: Option<&Value>,
    stored_schema: Option<&str>,
) -> Result<()> {
    if let Some(new_schema) = new_schema {
        crate::librarian::tools::schema_validate::validate(new_schema, current).map_err(|e| {
            RecoverableError::new(format!("merged params violate params_schema: {e}"))
        })?;
    } else if let Some(schema_text) = stored_schema {
        crate::librarian::tools::schema_validate::validate_against_stored(schema_text, current)
            .map_err(|e| {
                RecoverableError::new(format!("merged params violate params_schema: {e}"))
            })?;
    }
    Ok(())
}

/// Push a shape change out to the artifact's committed sidecar, when it has one.
///
/// Errors rather than warns. On failure the catalog has moved and `docs/augmentations` has
/// not, so the committed file now describes a superseded shape — and a stale sidecar restores
/// **silently** on a fresh clone, where an absent one is reported by
/// `augmentation_declared_but_absent`. The message names both facts, because the augmentation
/// itself did succeed and a caller told only "failed" would retry a write that already landed.
///
/// A **refusal** is reported the same way and for the same reason, but it is not a failure:
/// the write-through declined to publish a field this call never authored over a committed
/// value that disagrees with it. See [`crate::librarian::augmentation_sidecar::write_through`].
fn sidecar_write_through(
    cat: &crate::librarian::catalog::Catalog,
    id: &str,
    authored: crate::librarian::augmentation_sidecar::Authored<'_>,
) -> Result<()> {
    let outcome = crate::librarian::augmentation_sidecar::write_through(cat, id, authored)
        .map_err(|e| {
            anyhow::anyhow!(
                "the augmentation WAS updated, but its committed sidecar could not be written: \
                 {e:#}. The catalog and docs/augmentations now disagree; re-run once the path is \
                 writable, or a fresh clone will restore the superseded shape and report success."
            )
        })?;

    if !outcome.refused.is_empty() {
        return Err(RecoverableError::new(format!(
            "the augmentation WAS updated, but its committed sidecar was NOT republished. \
             This call did not set {fields}, and the sidecar disagrees with the catalog on \
             {those}. One of the two is stale and nothing here can tell which — mtime cannot, \
             because a checkout stamps a file with checkout time whatever its shape's age. \
             Publishing the row would overwrite a committed value this call never spoke for, \
             which is the loss `sidecar_shape_drift` exists to prevent. Read the difference, \
             resolve it from the correct side, then re-run — the field you DID set will travel \
             then. `librarian(action=\"doctor\")` reports the drift.",
            fields = outcome.refused.join(", "),
            those = if outcome.refused.len() == 1 {
                "it"
            } else {
                "them"
            },
        )));
    }
    Ok(())
}

/// Goal-tracker merge processing: enforce the scope-growth guard and, when the
/// status flips to `done`, evaluate the auto-close gate. Returns the `gate_check`
/// evidence to emit (Some) when the gate auto-closes; None otherwise. Errors on a
/// scope-growth violation or a blocked gate. Pure value-logic — no catalog/lock/await.
fn process_goal_tracker_merge(
    current: &Value,
    existing_params: &str,
    pre_status: Option<&str>,
) -> Result<Option<Value>> {
    let is_goal_tracker =
        current.get("acceptance_signals").is_some() && current.get("children").is_some();
    if !is_goal_tracker {
        return Ok(None);
    }

    use crate::librarian::tools::goal_aggregation::{
        evaluate_gate, validate_scope_growth, GateOutcome,
    };

    let pre_existing: Value =
        serde_json::from_str(existing_params).unwrap_or(Value::Object(Default::default()));
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

    let post_status = current.get("status").and_then(|s| s.as_str());
    if pre_status != Some("done") && post_status == Some("done") {
        match evaluate_gate(current) {
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
                    .filter(|c| c.get("status").and_then(|s| s.as_str()) == Some("done"))
                    .count();
                let signals_met = signals
                    .iter()
                    .filter(|s| s.get("met").and_then(|m| m.as_bool()) == Some(true))
                    .count();
                Ok(Some(json!({
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
                })))
            }
            GateOutcome::Block(reason) => Err(RecoverableError::new(format!(
                "goal auto-close gate blocked: {reason}"
            ))),
        }
    } else {
        Ok(None)
    }
}

/// Create/replace path (merge=false): prompt required, artifact must exist,
/// optional initial-schema validation, then upsert. Locks the catalog internally —
/// the body has no await, so the guard never crosses a suspension point.
fn create_or_replace_augmentation(ctx: &ToolContext, a: Args) -> Result<Value> {
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
            entry_collection: a.entry_collection,
            refreshed_at_commit: None,
        },
    )?;

    // Replace semantics: the caller speaks for the entire shape, including the fields it
    // omitted — those reset to None by documented design. So nothing here is unauthored.
    sidecar_write_through(
        &cat,
        &a.id,
        crate::librarian::augmentation_sidecar::Authored::All,
    )?;

    Ok(json!("ok"))
}

/// `doc(action="augment", id=…, augment={prompt, params, …})` dispatches here via
/// `artifact.rs`'s `flatten_augment_args`, which lifts the nested `augment` object
/// and carries `id`/`merge` into it — so this function still reads a flat `Args`.
pub(crate) async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let mut a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(
            format!("doc(action=\"augment\") requires 'id': {e}"),
            "e.g. doc(action=\"augment\", id=\"<16-hex>\", augment={prompt: \"...\"}). Get an id from doc(action=\"find\", ...). Pass merge=true to patch an existing augmentation — merge=false (the default) REPLACES all seven shape fields, silently resetting any you omit.",
        )
    })?;

    // params_path: read the params JSON from a filesystem path server-side.
    // A large params array (≳9 KB) can't be round-tripped through the model
    // to rebuild the inline `params` argument — the MCP result buffer caps
    // inline reads, so every read-back of the file buffers. Writing the JSON
    // to a file and pointing here sidesteps that. Filesystem path only (the
    // librarian ToolContext has no output_buffer, so @ref buffers are not
    // resolvable here). See get_guide("progressive-disclosure").
    if let Some(path) = a.params_path.take() {
        if a.params.is_some() {
            return Err(RecoverableError::new(
                "pass at most one of `params` or `params_path`",
            ));
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| RecoverableError::new(format!("params_path: reading {path}: {e}")))?;
        let parsed: Value = serde_json::from_str(&raw).map_err(|e| {
            RecoverableError::new(format!("params_path content is not valid JSON: {e}"))
        })?;
        // The schema's `"type": "object"` constrains only the INLINE `params`
        // argument — `params_path` bypasses that boundary entirely. Without
        // this check a bare top-level array is valid JSON, reaches
        // `apply_merge_patch`, misses its `(Object, Object)` arm, and is
        // discarded while the call reports "ok".
        // See docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md
        if !parsed.is_object() {
            let shape = match &parsed {
                Value::Array(_) => "array",
                Value::String(_) => "string",
                Value::Number(_) => "number",
                Value::Bool(_) => "boolean",
                Value::Null => "null",
                Value::Object(_) => unreachable!(),
            };
            return Err(RecoverableError::with_hint(
                format!(
                    "params_path: top-level JSON must be an object, found {shape}"
                ),
                format!(
                    "Wrap it under the key it belongs to, e.g. {{\"<entry_collection>\": <your {shape}>}}. \
                     A bare {shape} cannot be merge-patched into params and would be silently discarded."
                ),
            ));
        }
        a.params = Some(parsed);
    }

    {
        let mut cat = ctx.catalog.lock();
        a.id = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
    }

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
            let mut patched_siblings = false;
            if let Some(existing) = augmentation::get(&cat, &a.id)? {
                let mut current: Value = serde_json::from_str(&existing.params)
                    .unwrap_or(Value::Object(Default::default()));
                let pre_status = current
                    .get("status")
                    .and_then(|s| s.as_str())
                    .map(String::from);
                augmentation::apply_merge_patch(&mut current, &patch);

                // F-5: validate merged params against the EFFECTIVE schema —
                // the new one if this call provides it, otherwise the stored one.
                validate_merged_against_schema(
                    &current,
                    a.params_schema.as_ref(),
                    existing.params_schema.as_deref(),
                )?;

                // Goal-tracker merge: scope-growth guard + auto-close gate.
                // Evidence (if any) is emitted as a note event after the lock drops.
                gate_check_evidence =
                    process_goal_tracker_merge(&current, &existing.params, pre_status.as_deref())?;

                // F-5: when this call also provides sibling fields (prompt /
                // params_schema / render_template / entry_collection / flags),
                // patch them onto the existing row here via a full upsert that
                // PRESERVES every field the caller did not provide. Removes the
                // merge=false foot-gun where an omitted field silently resets to
                // None — merge=true now patches whatever you pass, keeps the rest.
                if a.prompt.is_some()
                    || a.params_schema.is_some()
                    || a.render_template.is_some()
                    || a.entry_collection.is_some()
                    || a.append_mode.is_some()
                    || a.history_cap.is_some()
                {
                    let now = chrono::Utc::now()
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string();
                    let params_schema_str = match a.params_schema.as_ref() {
                        Some(s) => Some(serde_json::to_string(s)?),
                        None => existing.params_schema.clone(),
                    };
                    augmentation::upsert(
                        &cat,
                        &augmentation::AugmentationRow {
                            artifact_id: a.id.clone(),
                            prompt: a.prompt.clone().unwrap_or_else(|| existing.prompt.clone()),
                            params: serde_json::to_string(&current)?,
                            last_refreshed_at: existing.last_refreshed_at.clone(),
                            refresh_count: existing.refresh_count,
                            created_at: existing.created_at.clone(),
                            updated_at: now,
                            render_template: a
                                .render_template
                                .clone()
                                .or_else(|| existing.render_template.clone()),
                            params_schema: params_schema_str,
                            append_mode: a.append_mode.unwrap_or(existing.append_mode),
                            history_cap: a.history_cap.map(|v| v as i64).or(existing.history_cap),
                            entry_collection: a
                                .entry_collection
                                .clone()
                                .or_else(|| existing.entry_collection.clone()),
                            refreshed_at_commit: existing.refreshed_at_commit.clone(),
                        },
                    )?;
                    // Merge semantics: the upsert above PRESERVED every field this call
                    // did not pass, so only the passed ones were authored here. Anything
                    // else must not be republished over a sidecar that disagrees.
                    let mut authored: Vec<&'static str> = Vec::new();
                    if a.prompt.is_some() {
                        authored.push("prompt");
                    }
                    if a.params_schema.is_some() {
                        authored.push("params_schema");
                    }
                    if a.render_template.is_some() {
                        authored.push("render_template");
                    }
                    if a.entry_collection.is_some() {
                        authored.push("entry_collection");
                    }
                    if a.append_mode.is_some() {
                        authored.push("append_mode");
                    }
                    if a.history_cap.is_some() {
                        authored.push("history_cap");
                    }
                    sidecar_write_through(
                        &cat,
                        &a.id,
                        crate::librarian::augmentation_sidecar::Authored::Only(&authored),
                    )?;
                    patched_siblings = true;
                }
            }
            if !patched_siblings {
                let found = augmentation::merge_params(&cat, &a.id, &patch)?.found;
                if !found {
                    return Err(RecoverableError::new(format!(
                        "no augmentation for artifact '{}' — call doc(action=\"augment\") first",
                        a.id
                    )));
                }
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

    create_or_replace_augmentation(ctx, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, augmentation, Catalog};
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        TestToolContextBuilder::new(cat).build()
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
        let result = call(
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
        call(&ctx, json!({"id": "art1", "prompt": "Old"}))
            .await
            .unwrap();
        call(&ctx, json!({"id": "art1", "prompt": "New"}))
            .await
            .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "New");
    }

    #[tokio::test]
    async fn missing_artifact_returns_recoverable_error() {
        let ctx = mk_ctx();
        let err = call(&ctx, json!({"id": "nope", "prompt": "Test"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn persists_render_template_and_params_schema() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "rt-art");
        call(
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
        let err = call(
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
        call(
            &ctx,
            json!({"id": "aug-1", "prompt": "do stuff", "params": {"a": 1, "b": 2}}),
        )
        .await
        .unwrap();

        // Now merge-patch: add c, delete b
        call(
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
    async fn merge_true_patches_sibling_fields_preserving_rest() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "aug-sib");
        // Initial full augmentation with every caller-controlled field set.
        call(
            &ctx,
            json!({
                "id": "aug-sib",
                "prompt": "keep the list",
                "params": {"items": [{"id": "X-1", "status": "open"}]},
                "params_schema": {
                    "type": "object",
                    "properties": {"items": {"type": "array", "items": {
                        "type": "object",
                        "properties": {"status": {"enum": ["open", "done"]}}
                    }}}
                },
                "render_template": "ORIGINAL TEMPLATE",
                "entry_collection": "items"
            }),
        )
        .await
        .unwrap();

        // F-5: widen the schema enum (add "blocked") AND add an item using it, in
        // ONE merge=true call. Pre-fix this needed a full merge=false re-send of
        // prompt/render_template/entry_collection or they'd reset to None.
        call(
            &ctx,
            json!({
                "id": "aug-sib",
                "merge": true,
                "params": {"items": [
                    {"id": "X-1", "status": "open"},
                    {"id": "X-2", "status": "blocked"}
                ]},
                "params_schema": {
                    "type": "object",
                    "properties": {"items": {"type": "array", "items": {
                        "type": "object",
                        "properties": {"status": {"enum": ["open", "done", "blocked"]}}
                    }}}
                }
            }),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let aug = crate::librarian::catalog::augmentation::get(&cat, "aug-sib")
            .unwrap()
            .unwrap();
        // Fields NOT provided in the merge call are preserved (not reset to None).
        assert_eq!(aug.prompt, "keep the list", "prompt preserved");
        assert_eq!(
            aug.render_template.as_deref(),
            Some("ORIGINAL TEMPLATE"),
            "render_template preserved"
        );
        assert_eq!(
            aug.entry_collection.as_deref(),
            Some("items"),
            "entry_collection preserved"
        );
        // The provided schema was written (now accepts "blocked").
        let schema = aug.params_schema.expect("schema present");
        assert!(schema.contains("blocked"), "schema widened: {schema}");
        let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(
            params["items"].as_array().unwrap().len(),
            2,
            "second item merged in"
        );
    }

    #[tokio::test]
    async fn merge_true_without_existing_augmentation_errors() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "aug-2");
        let err = call(
            &ctx,
            json!({"id": "aug-2", "merge": true, "params": {"x": 1}}),
        )
        .await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(
            msg.contains("doc(action=\"augment\")"),
            "error must tell the caller how to create the augmentation it's missing: {msg}"
        );
    }

    #[tokio::test]
    async fn non_merge_without_prompt_errors() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "aug-3");
        let err = call(&ctx, json!({"id": "aug-3", "params": {"x": 1}})).await;
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("prompt"), "error must mention prompt");
    }

    #[tokio::test]
    async fn persists_append_mode_and_history_cap() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "a99");
        call(
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
        call(
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

    /// F1 (2026-09-02 fix round on 5da2537d): `doc.augment`'s schema promises
    /// "Unknown keys are REJECTED, not ignored" but `Args` had no
    /// `#[serde(deny_unknown_fields)]`, so a misspelled shape field (e.g.
    /// `render_tempalte`) silently deserialized as absent instead of refusing —
    /// writing a half-configured shape while reporting success. Mirrors
    /// `create::tests::create_augment_rejects_an_unknown_field`, whose
    /// `AugmentSpec` already carries the attribute.
    #[tokio::test]
    async fn rejects_an_unknown_field_instead_of_silently_dropping_it() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "a101");
        let err = call(
            &ctx,
            serde_json::json!({"id": "a101", "prompt": "p", "render_tempalte": "typo"}),
        )
        .await
        .expect_err("a misspelled shape field must be rejected, not silently dropped");
        let msg = err.to_string();
        assert!(
            msg.contains("render_tempalte") || msg.contains("unknown field"),
            "error must name the unrecognised field so the typo is findable: {msg}"
        );
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
        let _ = call(
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
        call(
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
        call(
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
        let res = call(
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
    #[tokio::test]
    async fn persists_entry_collection() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "ec-tool");
        call(
            &ctx,
            json!({
                "id": "ec-tool",
                "prompt": "maintain the failures list",
                "params": { "failures": [] },
                "entry_collection": "failures"
            }),
        )
        .await
        .unwrap();
        let row = {
            let cat = ctx.catalog.lock();
            augmentation::get(&cat, "ec-tool").unwrap().unwrap()
        };
        assert_eq!(row.entry_collection.as_deref(), Some("failures"));
    }
    #[tokio::test]
    async fn params_path_reads_params_from_file() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-art");
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let payload =
            serde_json::to_string(&json!({"findings": [{"uf": "UF-1"}, {"uf": "UF-2"}]})).unwrap();
        std::fs::write(tmp.path(), &payload).unwrap();
        let result = call(
            &ctx,
            json!({
                "id": "pp-art",
                "prompt": "keep findings",
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap();
        assert_eq!(result, json!("ok"));
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "pp-art").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["findings"].as_array().unwrap().len(), 2);
        assert_eq!(params["findings"][0]["uf"], "UF-1");
    }

    // Mirrors the MRV-poc scenario: a large array patched via merge + params_path.
    #[tokio::test]
    async fn params_path_works_with_merge() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-merge");
        call(
            &ctx,
            json!({
                "id": "pp-merge",
                "prompt": "p",
                "params": {"findings": [{"uf": "UF-1", "dev_status": "open"}]}
            }),
        )
        .await
        .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let payload = serde_json::to_string(
            &json!({"findings": [{"uf": "UF-1", "dev_status": "fixed-verified"}]}),
        )
        .unwrap();
        std::fs::write(tmp.path(), &payload).unwrap();
        call(
            &ctx,
            json!({
                "id": "pp-merge",
                "merge": true,
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "pp-merge").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["findings"][0]["dev_status"], "fixed-verified");
    }

    #[tokio::test]
    async fn params_and_params_path_conflict_errors() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-conflict");
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "{}").unwrap();
        let err = call(
            &ctx,
            json!({
                "id": "pp-conflict",
                "prompt": "x",
                "params": {"a": 1},
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("at most one of"),
            "expected mutual-exclusion error, got: {err}"
        );
    }

    #[tokio::test]
    async fn params_path_invalid_json_errors() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-bad");
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "{not json").unwrap();
        let err = call(
            &ctx,
            json!({
                "id": "pp-bad",
                "prompt": "p",
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("not valid JSON"),
            "expected JSON parse error, got: {err}"
        );
    }

    /// Regression: docs/issues/archive/2026-07-02-artifact-augment-params-path-bare-array-silent-noop.md
    /// A bare top-level array is valid JSON, so it slipped past the only two
    /// guards (mutual exclusion + JSON validity) and reached
    /// `apply_merge_patch`, whose `(Object, Object)` match arm silently fell
    /// through — reporting success while discarding the entire payload.
    /// The schema's `"type": "object"` constrains only the INLINE `params`
    /// argument; `params_path` bypasses that boundary entirely.
    #[tokio::test]
    async fn params_path_bare_array_is_refused_not_silently_dropped() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-arr");
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"[{"id": "F-1"}, {"id": "F-2"}]"#).unwrap();
        let err = call(
            &ctx,
            json!({
                "id": "pp-arr",
                "prompt": "p",
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "must be recoverable so the caller can retry with a wrapped object"
        );
        assert!(
            err.to_string().contains("array"),
            "the error must name the actual shape so the fix is obvious: {err}"
        );
    }

    /// The merge path is where the silent drop actually bit — a bare array
    /// under `merge=true` is the exact reproduction in the bug report.
    #[tokio::test]
    async fn params_path_bare_array_is_refused_on_the_merge_path_too() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-arr-merge");
        call(
            &ctx,
            json!({"id": "pp-arr-merge", "prompt": "p", "params": {"keep": 1}}),
        )
        .await
        .unwrap();

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"["a", "b"]"#).unwrap();
        let err = call(
            &ctx,
            json!({
                "id": "pp-arr-merge",
                "merge": true,
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());

        // A refused call must not disturb what was already stored.
        let cat = ctx.catalog.lock();
        let row = crate::librarian::catalog::augmentation::get(&cat, "pp-arr-merge")
            .unwrap()
            .unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["keep"], 1);
    }

    /// Scalars are the same class of mistake and must not fall through either.
    #[tokio::test]
    async fn params_path_scalar_is_refused() {
        let ctx = mk_ctx();
        seed_artifact(&ctx, "pp-scalar");
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "42").unwrap();
        let err = call(
            &ctx,
            json!({
                "id": "pp-scalar",
                "prompt": "p",
                "params_path": tmp.path().to_str().unwrap()
            }),
        )
        .await
        .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    /// A real repo on disk: `.git`, an artifact declaring its sidecar, and the catalog row.
    ///
    /// `write_through` resolves the git root from the artifact's OWN path, so the synthetic
    /// `/test/repo/...` path `seed_artifact` uses makes it a no-op — which is why none of the
    /// tests above changed behaviour when the write-through landed. Returns the sidecar path,
    /// deliberately NOT creating the file: creation is the export's job.
    fn seed_declared_on_disk(
        ctx: &ToolContext,
        dir: &std::path::Path,
        id: &str,
    ) -> std::path::PathBuf {
        use crate::librarian::augmentation_sidecar as sc;
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let art = dir.join("t.md");
        let rel = sc::rel_path_for(dir, &art);
        std::fs::write(
            &art,
            format!("---\nkind: tracker\nexpects_augmentation: {rel}\n---\n\nbody\n"),
        )
        .unwrap();
        let cat = ctx.catalog.lock();
        artifact::upsert(
            &cat,
            &artifact::TestArtifactRowBuilder::new(id)
                .with_abs_path(art.to_str().unwrap())
                .with_kind("tracker")
                .build(),
        )
        .unwrap();
        dir.join(&rel)
    }

    /// Writes the sidecar the way the export leaves it, so a test starts from the real
    /// post-export state rather than an invented one.
    fn export_sidecar(ctx: &ToolContext, sidecar: &std::path::Path, id: &str) {
        use crate::librarian::augmentation_sidecar as sc;
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, id).unwrap().unwrap();
        sc::write(sidecar, &sc::AugmentationSidecar::from_row(&row)).unwrap();
    }

    /// The first live shape edit after the sidecar mechanism shipped found that NOTHING could
    /// update a committed sidecar: the export skips an already-exported artifact (idempotent,
    /// by its own pinned test), `reindex` attaches only when a row is absent (repair, not
    /// sync), and this tool did not touch the file. Each is correct alone; together they were a
    /// one-way door. A stale sidecar is worse than an absent one — absence is reported by
    /// `augmentation_declared_but_absent`, staleness restores clean and reports success.
    #[tokio::test]
    async fn a_shape_change_writes_through_to_the_committed_sidecar() {
        use crate::librarian::augmentation_sidecar as sc;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx();
        let sidecar = seed_declared_on_disk(&ctx, tmp.path(), "art1");

        call(&ctx, json!({"id": "art1", "prompt": "before"}))
            .await
            .unwrap();
        export_sidecar(&ctx, &sidecar, "art1");
        assert_eq!(sc::read(&sidecar).unwrap().prompt, "before");

        call(&ctx, json!({"id": "art1", "prompt": "after"}))
            .await
            .unwrap();

        assert_eq!(
            sc::read(&sidecar).unwrap().prompt,
            "after",
            "the committed sidecar must follow the catalog, or a fresh clone restores the \
             superseded shape and reports success"
        );
    }

    /// `merge=true` with a sibling field is the OTHER shape-writing path. Hooking only
    /// `create_or_replace_augmentation` would leave this one stale, and it is the path the
    /// documented `doc(action="augment", merge=true, ...)` recipe in CLAUDE.md actually uses.
    #[tokio::test]
    async fn a_merge_true_sibling_change_writes_through_too() {
        use crate::librarian::augmentation_sidecar as sc;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx();
        let sidecar = seed_declared_on_disk(&ctx, tmp.path(), "art1");

        call(&ctx, json!({"id": "art1", "prompt": "p"}))
            .await
            .unwrap();
        export_sidecar(&ctx, &sidecar, "art1");
        assert_eq!(sc::read(&sidecar).unwrap().entry_collection, None);

        call(
            &ctx,
            json!({"id": "art1", "merge": true, "entry_collection": "rows"}),
        )
        .await
        .unwrap();

        assert_eq!(
            sc::read(&sidecar).unwrap().entry_collection.as_deref(),
            Some("rows"),
            "a sibling-field patch changes the shape, so the committed shape must follow"
        );
    }

    /// The plan's requirement, and the reason the write is guarded by a byte comparison rather
    /// than by where the call happens to sit.
    #[tokio::test]
    async fn a_params_only_merge_leaves_the_committed_sidecar_byte_identical() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx();
        let sidecar = seed_declared_on_disk(&ctx, tmp.path(), "art1");

        call(
            &ctx,
            json!({"id": "art1", "prompt": "p", "params": {"a": 1}}),
        )
        .await
        .unwrap();
        export_sidecar(&ctx, &sidecar, "art1");

        // A trailing comment no serializer emits. Without it this test would pass even for an
        // unconditional rewrite, since `params` are not part of the rendering and canonical
        // output would be byte-identical anyway — the assertion would be unable to fail.
        // With it, any rewrite at all drops the comment and the test dies.
        let hand = std::fs::read_to_string(&sidecar).unwrap()
            + "# hand-edited, as the 2a8decc5 repair had to be\n";
        std::fs::write(&sidecar, &hand).unwrap();

        call(
            &ctx,
            json!({"id": "art1", "merge": true, "params": {"a": 2}}),
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&sidecar).unwrap(),
            hand,
            "a params-only merge must not touch the committed file — params are live state \
             and deliberately do not travel"
        );
        // Positive control: without this the assertion above would also pass if the merge had
        // silently done nothing at all.
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert!(
            row.params.contains("\"a\":2"),
            "the merge itself must still have happened: {}",
            row.params
        );
    }

    /// Creation stays `doctor(fix="export_augmentations")`'s job. If this tool created one, a
    /// repo that never asked for sidecars would start growing committed files as a side effect
    /// of an unrelated augment call.
    #[tokio::test]
    async fn write_through_never_creates_a_sidecar_that_does_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx();
        let sidecar = seed_declared_on_disk(&ctx, tmp.path(), "art1");

        call(&ctx, json!({"id": "art1", "prompt": "p"}))
            .await
            .unwrap();

        assert!(
            !sidecar.exists(),
            "declared-but-absent is the export's case and doctor reports it; creating the file \
             here would make every augment call a repo-writing side effect"
        );
    }

    /// A merge call must not silently republish a shape field it did not set.
    ///
    /// The write-through stands in front of `sidecar_shape_drift`, whose entire design
    /// position is that when the catalog and the committed sidecar disagree, **the
    /// direction is undecidable without a human** — its own detail says so, and warns
    /// "if the SIDECAR is right … do NOT export — that would overwrite their shape with
    /// your stale row". Resolving that automatically, catalog-wards, on a call that
    /// named a different field, is exactly the move that check exists to prevent.
    ///
    /// Measured instance: a `--merge --params-schema` call republished a stale
    /// `render_template` over the correct committed one, and it was recovered only
    /// because a pre-integration catalog backup happened to exist
    /// (`docs/issues/archive/2026-08-31-artifact-augment-write-through-republishes-the-whole-row.md`).
    #[tokio::test]
    async fn a_merge_call_refuses_to_republish_a_shape_field_it_did_not_set() {
        use crate::librarian::augmentation_sidecar as sc;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx();
        let sidecar = seed_declared_on_disk(&ctx, tmp.path(), "art1");

        call(&ctx, json!({"id": "art1", "prompt": "p"}))
            .await
            .unwrap();
        export_sidecar(&ctx, &sidecar, "art1");

        // The load-bearing fixture detail: the committed sidecar holds a
        // `render_template` the catalog row does not. That asymmetry IS the case under
        // test — it is the shape of the measured incident, where the on-disk value was
        // the correct one and the row's was the loss-window leftover. Make the two
        // agree and this test passes while testing nothing.
        let mut committed = sc::read(&sidecar).unwrap();
        committed.render_template = Some("the correct, human-authored template".into());
        sc::write(&sidecar, &committed).unwrap();

        let err = call(
            &ctx,
            json!({"id": "art1", "merge": true, "entry_collection": "rows"}),
        )
        .await
        .expect_err("republishing an unnamed field over a disagreeing sidecar must refuse");

        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "a refusal is recoverable — sibling calls must survive it: {err:#}"
        );
        let msg = format!("{err:#}");
        assert!(
            msg.contains("render_template"),
            "the refusal must NAME the field it declined to overwrite, or the operator \
                 cannot act on it: {msg}"
        );

        assert_eq!(
            sc::read(&sidecar).unwrap().render_template.as_deref(),
            Some("the correct, human-authored template"),
            "and it must not have been overwritten on the way to refusing — a refusal \
                 that already destroyed the value is not a refusal: {msg}"
        );
    }

    /// A REPLACE call authors the whole shape, so nothing in it is unnamed.
    ///
    /// `merge=false` resets omitted fields by documented design — that is the foot-gun
    /// `merge=true` exists to avoid, not an accident. So the refusal above must not fire
    /// here, or the guard would block the one path whose caller really did intend to
    /// speak for every field.
    #[tokio::test]
    async fn a_replace_call_publishes_the_whole_shape_including_fields_it_omitted() {
        use crate::librarian::augmentation_sidecar as sc;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx();
        let sidecar = seed_declared_on_disk(&ctx, tmp.path(), "art1");

        call(&ctx, json!({"id": "art1", "prompt": "p"}))
            .await
            .unwrap();
        export_sidecar(&ctx, &sidecar, "art1");

        let mut committed = sc::read(&sidecar).unwrap();
        committed.render_template = Some("about to be replaced, deliberately".into());
        sc::write(&sidecar, &committed).unwrap();

        call(&ctx, json!({"id": "art1", "prompt": "p2"}))
            .await
            .expect("a replace call speaks for the whole shape and must not be refused");

        assert_eq!(
            sc::read(&sidecar).unwrap().render_template,
            None,
            "replace semantics reset the omitted field, and the sidecar must follow the \
                 row rather than keep a value the row no longer holds"
        );
    }
}
