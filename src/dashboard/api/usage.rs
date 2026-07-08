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
    Json(super::common::usage_stats_response(
        &state,
        "No usage data. Tool statistics are recorded when the MCP server runs.",
        "usage",
        window,
        crate::usage::db::query_stats,
    ))
}
