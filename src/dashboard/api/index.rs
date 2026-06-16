use super::super::routes::DashboardState;
use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Dashboard endpoint: report basic index health.
///
/// Post-L-01 the dashboard queries Qdrant via the retrieval stack rather
/// than reading the legacy sqlite db. Drift telemetry was sqlite-only and
/// is dropped; `get_drift` below returns a stub envelope for compatibility
/// with the existing UI until the dashboard catches up.
pub async fn get_index(State(state): State<DashboardState>) -> Json<Value> {
    let client = match crate::retrieval::client::RetrievalClient::from_env().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "dashboard", "retrieval stack offline: {e}");
            return Json(json!({
                "available": false,
                "reason": format!("Retrieval stack offline: {e}"),
            }));
        }
    };

    let project_id = state
        .project_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("root")
        .to_string();
    let collection = client.config.collection("code_chunks");
    match client.project_index_stats(&collection, &project_id).await {
        Ok((chunks, files)) => Json(json!({
            "available": chunks > 0,
            "file_count": files,
            "chunk_count": chunks,
            "project_id": project_id,
            "collection": collection,
        })),
        Err(e) => Json(json!({
            "available": false,
            "reason": format!("Qdrant scroll failed: {e}"),
        })),
    }
}

#[derive(Deserialize)]
pub struct DriftParams {
    #[allow(dead_code)]
    pub threshold: Option<f32>,
}

/// Drift telemetry was a sqlite-side feature of the legacy index and is
/// dropped in L-01 step 8. Return an empty envelope so existing UI code
/// that polls this endpoint still works without 404s; rewire to a
/// Qdrant-backed drift query in a future commit if telemetry shows users
/// miss it.
pub async fn get_drift(
    State(_state): State<DashboardState>,
    Query(_params): Query<DriftParams>,
) -> Json<Value> {
    Json(json!({
        "available": false,
        "files": [],
        "note": "Drift detection was removed in L-01 step 8. Track in issue if needed.",
    }))
}
