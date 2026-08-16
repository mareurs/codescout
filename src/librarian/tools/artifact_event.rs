use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ArtifactEvent;

#[async_trait]
impl Tool for ArtifactEvent {
    fn name(&self) -> &'static str {
        "artifact_event"
    }

    fn description(&self) -> &'static str {
        "Artifact event log. action: create | list. \
         Events are immutable append-only records anchored to git commits — \
         distinct from field patches (use artifact(update) for those)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list"],
                    "description": "Operation: create appends an event; list returns events newest-first."
                },
                "artifact_id": { "type": "string", "description": "create/list: artifact id" },
                "kind": {
                    "type": "string",
                    "description": "create: event kind (note, reviewed, status_change, field_patch, superseded_by, external_signal, intent, verdict)"
                },
                "payload": {
                    "type": "object",
                    "description": format!(
                        "create: event payload (a JSON object). {}",
                        super::event_create::payload_requirements_sentence()
                    )
                },
                "anchor_commit": { "type": "string", "description": "create: git commit to anchor event to" },
                "head_commit": { "type": "string", "description": "create: HEAD commit at write time" },
                "parent_event_id": { "type": "string", "description": "create: parent event id for threading" },
                "author": { "type": "string", "description": "create: event author" },
                "also_mutates": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create: additional artifact ids mutated by this event"
                },
                "resolves_intent_event_id": { "type": "string", "description": "create: intent event id this verdict resolves" },
                "source": {
                    "type": "object",
                    "description": "create: external signal source {uri, kind, payload?}",
                    "properties": {
                        "uri": { "type": "string" },
                        "kind": { "type": "string" },
                        "payload": {}
                    },
                    "required": ["uri", "kind"]
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "list: filter to these event kinds"
                },
                "limit": { "type": "integer", "default": 50, "description": "list: max results" },
                "since": { "type": "integer", "format": "int64", "description": "list: return events after this ms epoch" },
                "until": { "type": "integer", "format": "int64", "description": "list: return events before this ms epoch" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| RecoverableError::new("action required — one of: create, list"))?;
        match action {
            "create" => super::event_create::call(ctx, args).await,
            "list" => super::timeline::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: create, list"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = ArtifactEvent
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "bogus", "artifact_id": "x"}),
            )
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn list_action_routes_correctly() {
        let v = ArtifactEvent
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "list", "artifact_id": "nonexistent"}),
            )
            .await
            .unwrap();
        // timeline returns {items:[...], count, truncated} even for unknown ids
        assert!(v["items"].is_array(), "expected items array, got {v}");
        assert_eq!(v["truncated"], serde_json::json!(false));
    }

    #[test]
    fn payload_schema_declares_object_type() {
        // Regression: docs/issues/archive/2026-05-21-artifact-event-create-payload-rejected.md
        // A `payload` property with no declared type caused MCP clients to
        // transport the value as a stringified JSON, which the server's
        // `.as_object()` guard then rejected with "payload must be object".
        let schema = ArtifactEvent.input_schema();
        assert_eq!(
            schema["properties"]["payload"]["type"], "object",
            "payload must declare type=object so clients send an object, not a JSON string"
        );
    }

    /// Every per-kind payload requirement must be both **enforced** and **advertised**.
    ///
    /// `artifact_event` ran a 50% error rate over the 2026-07 window and every failure was
    /// a missing per-kind payload field, while the schema said only "event payload (a JSON
    /// object)". Those errors carry no `err_family`, so the family-based sweep that caught
    /// the sibling instances (`edit_code.body`, `artifact.patch`) was blind to them; TU-9
    /// in `docs/trackers/2026-08-15-tool-usage-investigation.md` found them by a different
    /// route and asked for this.
    ///
    /// The first half executes `validate_payload` rather than reading
    /// `REQUIRED_PAYLOAD_FIELDS`, so the table cannot drift from the validator: drop a
    /// check there and this fails instead of the schema confidently describing a rule that
    /// is no longer enforced.
    ///
    /// See `docs/issues/archive/2026-08-15-conditionally-required-params-advertised-optional.md`.
    #[test]
    fn every_required_payload_field_is_enforced_and_advertised() {
        use crate::librarian::tools::event_create::REQUIRED_PAYLOAD_FIELDS;

        let desc = ArtifactEvent.input_schema()["properties"]["payload"]["description"]
            .as_str()
            .expect("payload must carry a description")
            .to_string();

        for (kind, fields) in REQUIRED_PAYLOAD_FIELDS {
            if fields.is_empty() {
                assert!(
                    desc.contains(kind),
                    "`{kind}` has no required keys and the description must say so: {desc}"
                );
                continue;
            }
            assert!(
                desc.contains(kind),
                "the description must name kind `{kind}`: {desc}"
            );

            for omitted in *fields {
                // Every OTHER required field present, so the failure is unambiguously the
                // omitted one and not simply the first check in the arm.
                let mut payload = serde_json::Map::new();
                for f in *fields {
                    if f != omitted {
                        payload.insert((*f).to_string(), serde_json::json!("x"));
                    }
                }
                let err = crate::librarian::tools::event_create::validate_payload(
                    kind,
                    &serde_json::Value::Object(payload),
                )
                .expect_err("a missing required payload field must be refused");

                assert_eq!(
                    err.to_string(),
                    format!("{kind}.{omitted} required"),
                    "the refusal must name kind and field, which is the shape \
                     usage::db::normalize_err_family classifies on"
                );
                assert!(
                    desc.contains(omitted),
                    "`{kind}.{omitted}` is enforced but never advertised: {desc}"
                );
            }
        }
    }
}
