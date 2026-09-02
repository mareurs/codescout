use anyhow::Result;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::{event_edges, events};

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
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("doc(action=\"event_list\") requires 'id': {e}"), "e.g. doc(action=\"event_list\", id=\"<16-hex>\"). Add kinds=[...] to filter, or since/until (ms epoch) to bound the window.")
    })?;
    let kinds_owned: Option<Vec<String>> = a.kinds.clone();
    let kinds_refs: Option<Vec<&str>> = kinds_owned
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    // Overfetch one past `limit` so a full-but-complete page is distinguishable
    // from a truncated one; without the `truncated` flag an agent reads a capped
    // page as the complete event history (silent-cap family — see
    // docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md).
    let mut rows = {
        let cat = ctx.catalog.lock();
        events::timeline_for_artifact(
            &cat,
            &a.artifact_id,
            kinds_refs.as_deref(),
            a.until,
            a.limit.saturating_add(1),
        )?
    };
    let truncated = rows.len() > a.limit;
    rows.truncate(a.limit);
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

    let count = out.len();
    let mut result = json!({
        "items": out,
        "count": count,
        "truncated": truncated,
    });
    if truncated {
        result["truncated_hint"] = json!(
            "more events match than were returned; raise `limit` (or narrow `kinds`/`since`/`until`)"
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{
        upsert as art_insert, ArtifactRow, TestArtifactRowBuilder,
    };
    use crate::librarian::tools::event_create::tests::mk_ctx;
    use tempfile::TempDir;

    fn art(id: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id).build()
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
            crate::librarian::tools::event_create::call(
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
        let res = call(&ctx, json!({"artifact_id": "a"})).await.unwrap();
        let arr = res["items"].as_array().unwrap();
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
        crate::librarian::tools::event_create::call(
            &ctx,
            json!({"artifact_id": "a", "kind": "note", "payload": {"text": "old"}}),
        )
        .await
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let mid_ts = chrono::Utc::now().timestamp_millis();
        std::thread::sleep(std::time::Duration::from_millis(10));
        crate::librarian::tools::event_create::call(
            &ctx,
            json!({"artifact_id": "a", "kind": "note", "payload": {"text": "new"}}),
        )
        .await
        .unwrap();
        let res = call(&ctx, json!({"artifact_id": "a", "since": mid_ts}))
            .await
            .unwrap();
        let arr = res["items"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["payload"]["text"], "new");
    }

    #[tokio::test]
    async fn intent_verdict_pair_flattens_resolves_edges() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed_artifact(&ctx, "a");
        let intent_id = crate::librarian::tools::event_create::call(
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
        let verdict_id = crate::librarian::tools::event_create::call(
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
        let res = call(&ctx, json!({"artifact_id": "a"})).await.unwrap();
        let arr = res["items"].as_array().unwrap();
        // verdict (newest) first
        assert_eq!(arr[0]["id"], verdict_id);
        assert_eq!(arr[0]["resolves_intent_id"], intent_id);
        // intent shows it was resolved
        assert_eq!(arr[1]["id"], intent_id);
        assert_eq!(arr[1]["resolved_by_verdict_id"], verdict_id);
    }

    #[tokio::test]
    async fn truncation_signals_capped_page() {
        // Silent-cap regression: a limit-capped timeline must flag that more
        // events exist, so an agent does not read the page as the complete
        // history. docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed_artifact(&ctx, "a");
        for i in 1..=3 {
            crate::librarian::tools::event_create::call(
                &ctx,
                json!({"artifact_id": "a", "kind": "note", "payload": {"text": format!("n{i}")}}),
            )
            .await
            .unwrap();
        }
        let res = call(&ctx, json!({"artifact_id": "a", "limit": 2}))
            .await
            .unwrap();
        assert_eq!(
            res["truncated"],
            json!(true),
            "3 events, limit 2 -> truncated"
        );
        assert_eq!(
            res["items"].as_array().unwrap().len(),
            2,
            "page capped to limit"
        );
        // Boundary: a page that holds everything must not flag truncation.
        let full = call(&ctx, json!({"artifact_id": "a", "limit": 10}))
            .await
            .unwrap();
        assert_eq!(
            full["truncated"],
            json!(false),
            "3 events, limit 10 -> not truncated"
        );
        assert_eq!(full["items"].as_array().unwrap().len(), 3);
    }
}
