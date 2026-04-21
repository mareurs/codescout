use anyhow::Result;
use axum::http::HeaderValue;
use axum::{
    http::header,
    response::Html,
    routing::{delete, get, post},
    Json, Router,
};
use std::path::{Path, PathBuf};
use tower_http::cors::CorsLayer;

use super::api;

/// Shared state passed to all handlers via axum State extractor.
#[derive(Clone)]
pub struct DashboardState {
    pub project_root: PathBuf,
}

pub fn build_router(project_root: &Path, port: u16) -> Result<Router> {
    let state = DashboardState {
        project_root: project_root.to_path_buf(),
    };

    // Restrict CORS to the exact port the dashboard is bound to. Allowing any
    // localhost port would let any local web app call memory write/delete endpoints.
    let localhost = format!("http://localhost:{port}");
    let loopback = format!("http://127.0.0.1:{port}");
    let allowed: Vec<HeaderValue> = [localhost, loopback]
        .into_iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    let router = Router::new()
        .route("/", get(serve_index))
        .route("/dashboard.css", get(serve_css))
        .route("/dashboard.js", get(serve_js))
        .route("/api/health", get(health))
        .route("/api/project", get(api::project::get_project_info))
        .route("/api/config", get(api::config::get_config))
        .route("/api/index", get(api::index::get_index))
        .route("/api/drift", get(api::index::get_drift))
        .route("/api/usage", get(api::usage::get_usage))
        .route("/api/lsp", get(api::lsp::get_lsp))
        .route("/api/errors", get(api::errors::get_errors))
        .route("/api/memories", get(api::memories::list_memories))
        .route("/api/memories/{topic}", get(api::memories::get_memory))
        .route("/api/memories/{topic}", post(api::memories::write_memory))
        .route(
            "/api/memories/{topic}",
            delete(api::memories::delete_memory),
        )
        .route("/api/libraries", get(api::libraries::get_libraries))
        .layer(
            CorsLayer::new()
                .allow_origin(allowed)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::DELETE,
                ])
                .allow_headers([header::CONTENT_TYPE]),
        )
        .with_state(state);

    Ok(router)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

#[cfg(not(debug_assertions))]
mod embedded {
    pub const INDEX_HTML: &str = include_str!("static/index.html");
    pub const DASHBOARD_CSS: &str = include_str!("static/dashboard.css");
    pub const DASHBOARD_JS: &str = include_str!("static/dashboard.js");
}

async fn serve_index() -> Html<String> {
    #[cfg(not(debug_assertions))]
    {
        Html(embedded::INDEX_HTML.to_string())
    }
    #[cfg(debug_assertions)]
    {
        let content = std::fs::read_to_string("src/dashboard/static/index.html")
            .unwrap_or_else(|_| "Dashboard HTML not found".into());
        Html(content)
    }
}

async fn serve_css() -> ([(header::HeaderName, &'static str); 1], String) {
    #[cfg(not(debug_assertions))]
    let content = embedded::DASHBOARD_CSS.to_string();
    #[cfg(debug_assertions)]
    let content = std::fs::read_to_string("src/dashboard/static/dashboard.css").unwrap_or_default();
    ([(header::CONTENT_TYPE, "text/css")], content)
}

async fn serve_js() -> ([(header::HeaderName, &'static str); 1], String) {
    #[cfg(not(debug_assertions))]
    let content = embedded::DASHBOARD_JS.to_string();
    #[cfg(debug_assertions)]
    let content = std::fs::read_to_string("src/dashboard/static/dashboard.js").unwrap_or_default();
    ([(header::CONTENT_TYPE, "application/javascript")], content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    fn test_router(root: &std::path::Path) -> Router {
        build_router(root, 3000).unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn project_info_returns_root() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/project")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["root"].as_str().is_some());
    }

    #[tokio::test]
    async fn config_returns_json() {
        let dir = tempfile::TempDir::new().unwrap();
        let ce_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&ce_dir).unwrap();
        std::fs::write(
            ce_dir.join("project.toml"),
            "[project]\nname = \"test-project\"\n",
        )
        .unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/config")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["project"]["name"], "test-project");
    }

    #[tokio::test]
    async fn index_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/index")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }

    #[tokio::test]
    async fn usage_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/usage")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }

    #[tokio::test]
    async fn errors_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/errors")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }

    #[tokio::test]
    async fn lsp_returns_not_available_without_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/lsp")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"], false);
    }

    #[tokio::test]
    async fn memories_list_returns_empty_for_fresh_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/memories")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["topics"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn libraries_returns_empty_for_fresh_project() {
        let dir = tempfile::TempDir::new().unwrap();
        let app = test_router(dir.path());
        let req = Request::builder()
            .uri("/api/libraries")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1_000_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["libraries"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cors_allows_localhost_origin() {
        let root = tempfile::TempDir::new().unwrap();
        let app = test_router(root.path());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header("Origin", "http://localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(response
            .headers()
            .contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn cors_rejects_external_origin() {
        let root = tempfile::TempDir::new().unwrap();
        let app = test_router(root.path());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header("Origin", "https://evil.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(!response
            .headers()
            .contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn cors_rejects_wrong_port() {
        let root = tempfile::TempDir::new().unwrap();
        // Router bound to port 3000 — a request from port 9999 must be rejected.
        let app = test_router(root.path());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .header("Origin", "http://localhost:9999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin"),
            "wrong-port localhost must be rejected"
        );
    }
}
