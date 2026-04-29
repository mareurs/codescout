use anyhow::{anyhow, Result};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::{event_edges, events, sources};

pub struct ArtifactEventCreate;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Args {
    pub artifact_id: String,
    pub kind: String,
    pub payload: Value,
    #[serde(default)]
    pub anchor_commit: Option<String>,
    #[serde(default)]
    pub head_commit: Option<String>,
    #[serde(default)]
    pub parent_event_id: Option<String>,
    #[serde(default)]
    pub also_mutates: Option<Vec<String>>,
    #[serde(default)]
    pub resolves_intent_event_id: Option<String>,
    #[serde(default)]
    pub source: Option<SourceArg>,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SourceArg {
    pub uri: String,
    pub kind: String,
    #[serde(default)]
    pub payload: Option<Value>,
}

const ALLOWED_KINDS: &[&str] = &[
    "note",
    "reviewed",
    "status_change",
    "field_patch",
    "superseded_by",
    "external_signal",
    "intent",
    "verdict",
];

#[async_trait]
impl Tool for ArtifactEventCreate {
    fn name(&self) -> &'static str {
        "artifact_event_create"
    }

    fn description(&self) -> &'static str {
        "Append an event (note, reviewed, status_change, field_patch, superseded_by, \
         external_signal, intent, verdict) to an artifact's timeline. Anchored to git commits."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(Args)).unwrap()
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        if !ALLOWED_KINDS.contains(&a.kind.as_str()) {
            return Err(anyhow!("unknown event kind: {}", a.kind));
        }
        validate_payload(&a.kind, &a.payload)?;

        // verdict ↔ intent invariants — checked before any writes
        if let Some(ref target) = a.resolves_intent_event_id {
            if a.kind != "verdict" {
                return Err(anyhow!(
                    "resolves_intent_event_id only valid on verdict events"
                ));
            }
            let cat = ctx.catalog.lock();
            let target_kind: Option<String> = cat
                .conn
                .query_row(
                    "SELECT kind FROM events WHERE id=?1",
                    rusqlite::params![target],
                    |r| r.get(0),
                )
                .ok();
            match target_kind.as_deref() {
                Some("intent") => {}
                Some(k) => return Err(anyhow!("target event {target} is kind={k}, not intent")),
                None => return Err(anyhow!("target event {target} not found")),
            }
            if !event_edges::incoming_by_rel(&cat, target, "resolves")?.is_empty() {
                return Err(anyhow!("intent {target} already resolved"));
            }
        }

        let now = chrono::Utc::now().timestamp_millis();
        let id = ulid::Ulid::new().to_string();

        let parent_id = match &a.parent_event_id {
            Some(p) => Some(p.clone()),
            None => {
                let cat = ctx.catalog.lock();
                events::latest_for_artifact(&cat, &a.artifact_id)?.map(|e| e.id)
            }
        };

        // status_change / field_patch: round-trip change to frontmatter on disk
        if a.kind == "status_change" || a.kind == "field_patch" {
            apply_payload_to_frontmatter(ctx, &a.artifact_id, &a.kind, &a.payload)?;
        }

        // superseded_by: also create an artifact_link rel=supersedes (back-compat dual-write).
        if a.kind == "superseded_by" {
            let target_id = a
                .payload
                .get("target_artifact_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("superseded_by.target_artifact_id required"))?;
            let cat = ctx.catalog.lock();
            crate::catalog::links::insert(
                &cat,
                &crate::catalog::links::LinkRow {
                    src_id: a.artifact_id.clone(),
                    dst_id: target_id.into(),
                    rel: "supersedes".into(),
                    created_at: now,
                },
            )?;
        }

        let payload_str = serde_json::to_string(&a.payload)?;
        {
            let cat = ctx.catalog.lock();
            events::insert(
                &cat,
                &events::EventRow {
                    id: id.clone(),
                    artifact_id: a.artifact_id.clone(),
                    kind: a.kind.clone(),
                    payload: payload_str,
                    anchor_commit: a.anchor_commit.clone(),
                    head_commit: a.head_commit.clone(),
                    author: a.author.clone(),
                    created_at: now,
                },
            )?;
        }

        let mut edges: Vec<event_edges::EdgeRow> = Vec::new();

        if let Some(p) = parent_id.clone() {
            edges.push(event_edges::EdgeRow {
                src_event_id: id.clone(),
                dst_event_id: Some(p),
                dst_artifact_id: None,
                dst_source_id: None,
                rel: "parent".into(),
            });
        }

        if let Some(s) = &a.source {
            let src_id = format!("{}:{}", s.kind, s.uri);
            {
                let cat = ctx.catalog.lock();
                sources::upsert(
                    &cat,
                    &sources::SourceRow {
                        id: src_id.clone(),
                        uri: s.uri.clone(),
                        kind: s.kind.clone(),
                        payload: s.payload.as_ref().map(|p| p.to_string()),
                        ingested_at: now,
                    },
                )?;
            }
            edges.push(event_edges::EdgeRow {
                src_event_id: id.clone(),
                dst_event_id: None,
                dst_artifact_id: None,
                dst_source_id: Some(src_id),
                rel: "triggered_by".into(),
            });
        }

        for art in a.also_mutates.unwrap_or_default() {
            edges.push(event_edges::EdgeRow {
                src_event_id: id.clone(),
                dst_event_id: None,
                dst_artifact_id: Some(art),
                dst_source_id: None,
                rel: "mutates".into(),
            });
        }

        if let Some(target) = a.resolves_intent_event_id {
            edges.push(event_edges::EdgeRow {
                src_event_id: id.clone(),
                dst_event_id: Some(target),
                dst_artifact_id: None,
                dst_source_id: None,
                rel: "resolves".into(),
            });
        }

        {
            let cat = ctx.catalog.lock();
            event_edges::insert_many(&cat, &edges)?;
        }

        Ok(json!({
            "event_id": id,
            "parent_event_id": parent_id,
            "anchor_commit": a.anchor_commit,
            "head_commit": a.head_commit,
        }))
    }
}

fn validate_payload(kind: &str, p: &Value) -> Result<()> {
    let obj = p
        .as_object()
        .ok_or_else(|| anyhow!("payload must be object"))?;
    match kind {
        "note" => {
            obj.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("note.text required"))?;
        }
        "reviewed" => { /* both fields optional */ }
        "status_change" => {
            obj.get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("status_change.to required"))?;
        }
        "field_patch" => {
            obj.get("field")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("field_patch.field required"))?;
            obj.get("to")
                .ok_or_else(|| anyhow!("field_patch.to required"))?;
        }
        "superseded_by" => {
            obj.get("target_artifact_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("superseded_by.target_artifact_id required"))?;
        }
        "external_signal" => {
            obj.get("source_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("external_signal.source_id required"))?;
            obj.get("summary")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("external_signal.summary required"))?;
        }
        "intent" => {
            obj.get("hypothesis")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("intent.hypothesis required"))?;
        }
        "verdict" => {
            let outcome = obj
                .get("outcome")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("verdict.outcome required"))?;
            if !matches!(outcome, "confirmed" | "refuted" | "partial" | "abandoned") {
                return Err(anyhow!(
                    "verdict.outcome must be confirmed|refuted|partial|abandoned"
                ));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn apply_payload_to_frontmatter(
    ctx: &ToolContext,
    artifact_id: &str,
    kind: &str,
    payload: &Value,
) -> Result<()> {
    match kind {
        "status_change" => {
            let to = payload.get("to").and_then(|v| v.as_str()).unwrap(); // already validated above
            crate::tools::update::write_field_to_frontmatter(
                ctx,
                artifact_id,
                "status",
                &Value::String(to.into()),
            )?;
        }
        "field_patch" => {
            let field = payload.get("field").and_then(|v| v.as_str()).unwrap(); // already validated above
            let to = payload.get("to").unwrap(); // already validated above
            crate::tools::update::write_field_to_frontmatter(ctx, artifact_id, field, to)?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_insert, ArtifactRow};
    use crate::tools::observe::tests::mk_ctx;
    use tempfile::TempDir;

    fn art(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "spec".into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    #[tokio::test]
    async fn note_event_round_trip() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a1")).unwrap();
        }
        let result = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a1",
                    "kind": "note",
                    "payload": {"text": "hi"}
                }),
            )
            .await
            .unwrap();
        let event_id = result["event_id"].as_str().unwrap();
        assert!(!event_id.is_empty());
    }

    #[tokio::test]
    async fn rejects_unknown_kind() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a1",
                    "kind": "bogus",
                    "payload": {}
                }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown event kind"));
    }

    #[tokio::test]
    async fn verdict_resolves_intent_emits_edge() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a2")).unwrap();
        }

        // Write intent event
        let intent_result = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a2",
                    "kind": "intent",
                    "payload": {"hypothesis": "X causes Y"}
                }),
            )
            .await
            .unwrap();
        let intent_id = intent_result["event_id"].as_str().unwrap().to_string();

        // Write verdict resolving the intent
        let verdict_result = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a2",
                    "kind": "verdict",
                    "payload": {"outcome": "confirmed"},
                    "resolves_intent_event_id": intent_id
                }),
            )
            .await
            .unwrap();
        let verdict_id = verdict_result["event_id"].as_str().unwrap().to_string();

        let cat = ctx.catalog.lock();
        let edges = event_edges::outgoing(&cat, &verdict_id).unwrap();
        let resolves_edge = edges.iter().find(|e| e.rel == "resolves");
        assert!(resolves_edge.is_some());
        assert_eq!(
            resolves_edge.unwrap().dst_event_id.as_deref(),
            Some(intent_id.as_str())
        );
    }

    #[tokio::test]
    async fn cannot_resolve_intent_twice() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a3")).unwrap();
        }

        // Write intent event
        let intent_result = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a3",
                    "kind": "intent",
                    "payload": {"hypothesis": "P implies Q"}
                }),
            )
            .await
            .unwrap();
        let intent_id = intent_result["event_id"].as_str().unwrap().to_string();

        // First verdict — should succeed
        ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a3",
                    "kind": "verdict",
                    "payload": {"outcome": "refuted"},
                    "resolves_intent_event_id": intent_id
                }),
            )
            .await
            .unwrap();

        // Second verdict against the same intent — should fail
        let err = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a3",
                    "kind": "verdict",
                    "payload": {"outcome": "confirmed"},
                    "resolves_intent_event_id": intent_id
                }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already resolved"));
    }

    #[tokio::test]
    async fn event_create_superseded_by_creates_link() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("src-art")).unwrap();
            art_insert(&cat, &art("dst-art")).unwrap();
        }

        ArtifactEventCreate
            .call(
                &ctx,
                serde_json::json!({
                    "artifact_id": "src-art",
                    "kind": "superseded_by",
                    "payload": {"target_artifact_id": "dst-art"}
                }),
            )
            .await
            .unwrap();

        // Expect an artifact_link with rel=supersedes from src-art → dst-art.
        let count: i64 = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_link WHERE src_id='src-art' AND dst_id='dst-art' AND rel='supersedes'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "superseded_by event must create an artifact_link rel=supersedes"
        );
    }
}
