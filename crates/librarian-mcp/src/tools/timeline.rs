use anyhow::Result;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::{event_edges, events};

pub struct ArtifactTimeline;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Args {
    pub artifact_id: String,
    #[serde(default)]
    pub since: Option<i64>,
    #[serde(default)]
    pub until: Option<i64>,
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

#[async_trait]
impl Tool for ArtifactTimeline {
    fn name(&self) -> &'static str {
        "artifact_timeline"
    }

    fn description(&self) -> &'static str {
        "Return events for an artifact, newest first. Each event includes resolved \
         parent_event_id, triggered_by_source, mutates_artifacts, resolves_intent_id, \
         resolved_by_verdict_id. \
         Ordering is `created_at DESC, id DESC` (id is ULID, time-ordered to ms). \
         Within the same millisecond, ULID's random tail dominates so order may not \
         match strict creation order — pin lookups by event id rather than array position."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(Args)).unwrap()
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let kinds_owned: Option<Vec<String>> = a.kinds.clone();
        let kinds_refs: Option<Vec<&str>> = kinds_owned
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect());
        let mut rows = {
            let cat = ctx.catalog.lock();
            events::timeline_for_artifact(
                &cat,
                &a.artifact_id,
                kinds_refs.as_deref(),
                a.until,
                a.limit,
            )?
        };
        if let Some(since) = a.since {
            rows.retain(|e| e.created_at >= since);
        }

        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            let cat = ctx.catalog.lock();
            let edges = event_edges::outgoing(&cat, &r.id)?;
            let parent = edges
                .iter()
                .find(|e| e.rel == "parent")
                .and_then(|e| e.dst_event_id.clone());
            let triggered_by = edges
                .iter()
                .find(|e| e.rel == "triggered_by")
                .and_then(|e| e.dst_source_id.clone());
            let mutates: Vec<String> = edges
                .iter()
                .filter(|e| e.rel == "mutates")
                .filter_map(|e| e.dst_artifact_id.clone())
                .collect();
            let resolves_intent_id = edges
                .iter()
                .find(|e| e.rel == "resolves")
                .and_then(|e| e.dst_event_id.clone());
            let resolved_by_verdict_id = event_edges::incoming_by_rel(&cat, &r.id, "resolves")?
                .into_iter()
                .next()
                .map(|e| e.src_event_id);
            let payload: Value = serde_json::from_str(&r.payload).unwrap_or(Value::Null);
            out.push(json!({
                "id": r.id,
                "kind": r.kind,
                "payload": payload,
                "anchor_commit": r.anchor_commit,
                "head_commit": r.head_commit,
                "author": r.author,
                "created_at": r.created_at,
                "parent_event_id": parent,
                "triggered_by_source": triggered_by,
                "mutates_artifacts": mutates,
                "resolves_intent_id": resolves_intent_id,
                "resolved_by_verdict_id": resolved_by_verdict_id,
            }));
        }
        Ok(Value::Array(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_insert, ArtifactRow};
    use crate::tools::event_create::ArtifactEventCreate;
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

    fn seed_artifact(ctx: &ToolContext, id: &str) {
        let cat = ctx.catalog.lock();
        art_insert(&cat, &art(id)).unwrap();
    }

    #[tokio::test]
    async fn returns_events_newest_first() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed_artifact(&ctx, "a");
        for i in 1..=3 {
            ArtifactEventCreate
                .call(
                    &ctx,
                    json!({
                        "artifact_id": "a",
                        "kind": "note",
                        "payload": {"text": format!("n{i}")}
                    }),
                )
                .await
                .unwrap();
        }
        let res = ArtifactTimeline
            .call(&ctx, json!({"artifact_id": "a"}))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        // Newest first: payload.text == "n3" first
        assert_eq!(arr[0]["payload"]["text"], "n3");
        assert_eq!(arr[2]["payload"]["text"], "n1");
    }

    #[tokio::test]
    async fn since_filter_excludes_older() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed_artifact(&ctx, "a");
        ArtifactEventCreate
            .call(
                &ctx,
                json!({"artifact_id": "a", "kind": "note", "payload": {"text": "old"}}),
            )
            .await
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let mid_ts = chrono::Utc::now().timestamp_millis();
        std::thread::sleep(std::time::Duration::from_millis(10));
        ArtifactEventCreate
            .call(
                &ctx,
                json!({"artifact_id": "a", "kind": "note", "payload": {"text": "new"}}),
            )
            .await
            .unwrap();
        let res = ArtifactTimeline
            .call(&ctx, json!({"artifact_id": "a", "since": mid_ts}))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["payload"]["text"], "new");
    }

    #[tokio::test]
    async fn intent_verdict_pair_flattens_resolves_edges() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed_artifact(&ctx, "a");
        let intent_id = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a",
                    "kind": "intent",
                    "payload": {"hypothesis": "h"}
                }),
            )
            .await
            .unwrap()["event_id"]
            .as_str()
            .unwrap()
            .to_string();
        let verdict_id = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a",
                    "kind": "verdict",
                    "payload": {"outcome": "confirmed", "summary": "s"},
                    "resolves_intent_event_id": intent_id.clone()
                }),
            )
            .await
            .unwrap()["event_id"]
            .as_str()
            .unwrap()
            .to_string();
        let res = ArtifactTimeline
            .call(&ctx, json!({"artifact_id": "a"}))
            .await
            .unwrap();
        let arr = res.as_array().unwrap();
        // verdict (newest) first
        assert_eq!(arr[0]["id"], verdict_id);
        assert_eq!(arr[0]["resolves_intent_id"], intent_id);
        // intent shows it was resolved
        assert_eq!(arr[1]["id"], intent_id);
        assert_eq!(arr[1]["resolved_by_verdict_id"], verdict_id);
    }
}
