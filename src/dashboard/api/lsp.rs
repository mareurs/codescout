use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::super::routes::DashboardState;

#[derive(Deserialize)]
pub struct LspParams {
    pub window: Option<String>,
}

pub async fn get_lsp(
    State(state): State<DashboardState>,
    Query(params): Query<LspParams>,
) -> Json<Value> {
    let db_path = state.project_root.join(".codescout").join("usage.db");
    if !db_path.exists() {
        return Json(json!({
            "available": false,
            "reason": "No usage data recorded yet."
        }));
    }

    let conn = match crate::usage::db::open_db(&state.project_root) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "dashboard", "usage db open failed: {e}");
            return Json(json!({
                "available": false,
                "reason": "Failed to open usage DB."
            }));
        }
    };

    let window = params.window.as_deref().unwrap_or("30d");
    match crate::usage::db::query_lsp_stats(&conn, window) {
        Ok(stats) => {
            let mut val = serde_json::to_value(stats).unwrap_or_else(|e| {
                tracing::error!(target: "dashboard", "lsp stats serialize failed: {e}");
                Value::Null
            });
            val["available"] = json!(true);
            Json(val)
        }
        Err(e) => {
            tracing::warn!(target: "dashboard", "lsp stats query failed: {e}");
            Json(json!({
                "available": false,
                "reason": "Query failed."
            }))
        }
    }
}
