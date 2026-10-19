#[cfg(feature = "dashboard")]
mod api;
#[cfg(feature = "dashboard")]
mod routes;

use anyhow::Result;
use std::path::PathBuf;

/// Launch the dashboard HTTP server.
///
/// Reads project data from `.codescout/` and serves a web UI.
/// Does NOT start the MCP server, LSP, or tool machinery.
#[cfg(feature = "dashboard")]
pub async fn serve(
    project_root: PathBuf,
    host: String,
    port: u16,
    open_browser: bool,
) -> Result<()> {
    // A typo'd `--project` used to be stored unvalidated, and the first `/api/memories`
    // request built `<typo>/.codescout/memories` through `MemoryStore::open`. Refuse it
    // before anything binds or reads. Done here, not in the CLI arm, so every caller of
    // `serve` is covered.
    let project_root = crate::agent::existing_project_dir(project_root, "project root")?;
    let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("Dashboard server starting at http://{}", addr);

    let router = routes::build_router(&project_root, port)?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    eprintln!("Dashboard: http://{}", actual_addr);

    if open_browser {
        let url = format!("http://{}", actual_addr);
        if let Err(e) = open::that(&url) {
            tracing::warn!("Failed to open browser: {}", e);
        }
    }

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = crate::server::shutdown_signal().await;
        })
        .await?;

    Ok(())
}

#[cfg(all(test, feature = "dashboard"))]
mod tests {
    use super::*;
    use std::time::Duration;

    /// F6: `codescout dashboard --project <typo>` stored the root unvalidated, and the first
    /// `/api/memories` request built `<typo>/.codescout/memories` through `MemoryStore::open`.
    /// The server must refuse a root that is not an existing directory BEFORE it binds, and
    /// create nothing. The timeout is the red state: unfixed, `serve` starts serving and never
    /// returns, which is itself the defect.
    #[tokio::test]
    async fn serve_refuses_a_project_root_that_is_not_a_directory_and_creates_nothing() {
        let host = tempfile::tempdir().unwrap();
        let host_root = std::fs::canonicalize(host.path()).unwrap();
        let typo = host_root.join("typo");

        let outcome = tokio::time::timeout(
            Duration::from_secs(5),
            serve(typo.clone(), "127.0.0.1".into(), 0, false),
        )
        .await
        .expect("serve must refuse a missing root, not start serving it");
        let err = outcome.expect_err("a missing project root must be refused");
        let refusal = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
        assert!(
            refusal.message.starts_with("project root '")
                && refusal.message.contains("not an existing directory"),
            "got: {}",
            refusal.message
        );
        assert!(
            std::fs::read_dir(&host_root).unwrap().next().is_none(),
            "nothing may be created under the directory the typo pointed into"
        );
    }

    /// The other side, POSITIVELY: a root that exists is served, proved by connecting and being
    /// answered (an earlier version only observed a timeout, which any hang also produces).
    #[tokio::test]
    async fn serve_still_serves_an_existing_project_root() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let host = tempfile::tempdir().unwrap();
        // A free port, found by binding and releasing it: `serve` binds its own listener.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let server = tokio::spawn(serve(
            host.path().to_path_buf(),
            "127.0.0.1".into(),
            port,
            false,
        ));

        let mut reply = None;
        for _ in 0..60 {
            if server.is_finished() {
                break;
            }
            if let Ok(mut stream) = tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
                stream
                    .write_all(
                        b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
                let mut body = String::new();
                stream.read_to_string(&mut body).await.unwrap();
                reply = Some(body);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let finished_early = server.is_finished();
        server.abort();
        assert!(
            !finished_early,
            "serve returned instead of serving an existing root"
        );
        let reply = reply.expect("the dashboard never accepted a connection");
        assert!(reply.starts_with("HTTP/1.1 200"), "got: {reply}");
        assert!(reply.contains("\"status\":\"ok\""), "got: {reply}");
    }
}
