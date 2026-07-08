use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::super::routes::DashboardState;

#[derive(Deserialize)]
pub struct LspParams {
    pub window: Option<String>,
}

pub async fn get_lsp(
    State(state): State<DashboardState>,
    Query(params): Query<LspParams>,
) -> Json<Value> {
    let window = params.window.as_deref().unwrap_or("30d");
    Json(super::common::usage_stats_response(
        &state,
        "No usage data recorded yet.",
        "lsp",
        window,
        crate::usage::db::query_lsp_stats,
    ))
}
