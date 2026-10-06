use super::super::routes::DashboardState;
use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct UsageParams {
    pub window: Option<String>,
}

pub async fn get_usage(
    State(state): State<DashboardState>,
    Query(params): Query<UsageParams>,
) -> Json<Value> {
    let window = params.window.as_deref().unwrap_or("30d");
    let mut body = super::common::usage_stats_response(
        &state,
        "No usage data. Tool statistics are recorded when the MCP server runs.",
        "usage",
        window,
        crate::usage::db::query_stats,
    );
    // These counts come from `tool_calls`, which the recorder fills from codescout's
    // own MCP dispatch only. Name that beside the counts so a native tool's absence
    // is not read as "never used". Only attached when there are counts to qualify;
    // `/api/lsp` reads `lsp_events` and is deliberately left without it.
    if body["available"] == true {
        body["scope"] = Value::from(crate::usage::RECORDER_SCOPE);
    }
    Json(body)
}
