use super::super::routes::DashboardState;
use crate::memory::MemoryStore;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

fn internal_error(context: &str, err: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    tracing::warn!(target: "dashboard", "{context}: {err}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
}

pub async fn list_memories(State(state): State<DashboardState>) -> Json<Value> {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(_) => return Json(json!({ "topics": [] })),
    };
    let topics = store.list().unwrap_or_default();
    Json(json!({ "topics": topics }))
}

pub async fn get_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (get)", e),
    };
    match store.read(&topic) {
        Ok(Some(content)) => (
            StatusCode::OK,
            Json(json!({ "topic": topic, "content": content })),
        ),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "Not found" }))),
        Err(e) => internal_error("memory read", e),
    }
}

#[derive(Deserialize)]
pub struct WriteMemoryBody {
    pub content: String,
}

pub async fn write_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
    Json(body): Json<WriteMemoryBody>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (write)", e),
    };
    match store.write(&topic, &body.content) {
        Ok(()) => (StatusCode::OK, Json(json!("ok"))),
        Err(e) => internal_error("memory write", e),
    }
}

pub async fn delete_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (delete)", e),
    };
    match store.delete(&topic) {
        Ok(()) => (StatusCode::OK, Json(json!("ok"))),
        Err(e) => internal_error("memory delete", e),
    }
}
