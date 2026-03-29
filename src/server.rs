//! MCP server — bridges our `Tool` registry to rmcp's `ServerHandler`.

use std::path::PathBuf;
use std::sync::Arc;

use crate::lsp::{LspManager, LspProvider};

use anyhow::Result;
#[cfg(feature = "http")]
use axum::response::IntoResponse;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
        RawContent, ServerCapabilities, ServerInfo, Tool as McpTool,
    },
    service::RequestContext,
    ErrorData as McpError, Peer, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::Value;

use crate::agent::Agent;
use crate::tools::{
    config::{ActivateProject, ProjectStatus},
    file::{CreateFile, EditFile, Glob, Grep, ListDir, ReadFile},
    library::{ListLibraries, RegisterLibrary},
    markdown::{EditMarkdown, ReadMarkdown},
    memory::Memory,
    progress,
    semantic::{IndexProject, IndexStatus, SemanticSearch},
    symbol::{
        FindReferences, FindSymbol, GotoDefinition, Hover, InsertCode, ListSymbols, RemoveSymbol,
        RenameSymbol, ReplaceSymbol,
    },
    workflow::{Onboarding, RunCommand},
    Tool, ToolContext,
};
use crate::usage::UsageRecorder;

#[derive(Clone)]
pub struct CodeScoutServer {
    agent: Agent,
    lsp: Arc<dyn LspProvider>,
    output_buffer: Arc<crate::tools::output_buffer::OutputBuffer>,
    // Arc<dyn Tool>: heterogeneous collection of 23+ different tool types dispatched by name at runtime.
    tools: Vec<Arc<dyn Tool>>,
    /// Pre-computed at construction because `get_info()` is sync.
    /// Becomes stale if project state changes mid-session (e.g. after onboarding or indexing).
    /// For stdio this means instructions reflect the state at server startup;
    /// for HTTP/SSE each connection gets fresh instructions.
    instructions: String,
    section_coverage: Arc<std::sync::Mutex<crate::tools::section_coverage::SectionCoverage>>,
}

impl CodeScoutServer {
    pub async fn new(agent: Agent) -> Self {
        let lsp = match agent.project_root().await {
            Some(root) => LspManager::new_arc_with_root(root),
            None => LspManager::new_arc(),
        };
        Self::from_parts(agent, lsp).await
    }

    /// Create a server with an existing LspManager (used for HTTP multi-session).
    pub async fn from_parts(agent: Agent, lsp: Arc<dyn LspProvider>) -> Self {
        let status = agent.project_status().await;
        let instructions = crate::prompts::build_server_instructions(status.as_ref());
        let tools: Vec<Arc<dyn Tool>> = vec![
            // File tools (fully implemented)
            Arc::new(ReadFile),
            Arc::new(ListDir),
            Arc::new(Grep),
            Arc::new(CreateFile),
            Arc::new(Glob),
            Arc::new(EditFile),
            Arc::new(EditMarkdown),
            Arc::new(ReadMarkdown),
            // Workflow tools
            Arc::new(RunCommand),
            Arc::new(Onboarding),
            // Symbol tools (stub — require LSP)
            Arc::new(FindSymbol),
            Arc::new(FindReferences),
            Arc::new(GotoDefinition),
            Arc::new(Hover),
            Arc::new(ListSymbols),
            Arc::new(ReplaceSymbol),
            Arc::new(RemoveSymbol),
            Arc::new(InsertCode),
            Arc::new(RenameSymbol),
            // Memory tools
            Arc::new(Memory),
            // Semantic search tools
            Arc::new(SemanticSearch),
            Arc::new(IndexProject),
            Arc::new(IndexStatus),
            // Config tools
            Arc::new(ActivateProject),
            Arc::new(ProjectStatus),
            // Library tools
            Arc::new(ListLibraries),
            Arc::new(RegisterLibrary),
        ];
        let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));
        let section_coverage = Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        ));
        Self {
            agent,
            lsp,
            output_buffer,
            tools,
            instructions,
            section_coverage,
        }
    }

    fn find_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    /// Core tool dispatch, separated from the MCP trait method so tests can
    /// call it without constructing a `RequestContext`.
    #[tracing::instrument(skip_all, fields(tool = %req.name))]
    async fn call_tool_inner(
        &self,
        req: CallToolRequestParams,
        progress: Option<Arc<progress::ProgressReporter>>,
        peer: Option<Peer<RoleServer>>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!(args = ?req.arguments, "tool call");

        let arg_keys: Vec<&str> = req
            .arguments
            .as_ref()
            .map(|m| m.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();
        tracing::info!(tool = %req.name, ?arg_keys, "tool_call");
        let tool_start = std::time::Instant::now();

        let tool = self.find_tool(&req.name).ok_or_else(|| {
            McpError::invalid_params(format!("unknown tool: '{}'", req.name), None)
        })?;

        // Check tool access before dispatching
        let security = self.agent.security_config().await;
        if let Err(e) = crate::util::path_security::check_tool_access(&req.name, &security) {
            return Ok(CallToolResult::error(vec![Content::text(e.to_string())]));
        }

        let input: Value = req
            .arguments
            .map(Value::Object)
            .unwrap_or(Value::Object(Default::default()));

        let ctx = ToolContext {
            agent: self.agent.clone(),
            lsp: self.lsp.clone(),
            output_buffer: self.output_buffer.clone(),
            progress,
            peer,
            section_coverage: self.section_coverage.clone(),
        };

        let timeout_secs = if tool_skips_server_timeout(&req.name) {
            None
        } else {
            self.agent
                .with_project(|p| Ok(p.config.project.tool_timeout_secs))
                .await
                .ok()
        };

        let recorder = UsageRecorder::new(self.agent.clone());

        let result = if let Some(secs) = timeout_secs {
            tokio::time::timeout(
                std::time::Duration::from_secs(secs),
                recorder.record_content(&req.name, || tool.call_content(input, &ctx)),
            )
            .await
            .unwrap_or_else(|_| {
                Err(anyhow::anyhow!(
                    "Tool '{}' timed out after {}s. \
                     Increase tool_timeout_secs in .codescout/project.toml if needed.",
                    req.name,
                    secs
                ))
            })
        } else {
            recorder
                .record_content(&req.name, || tool.call_content(input, &ctx))
                .await
        };

        // Assemble the result — success or error both produce a CallToolResult
        // so we can apply post-processing in one place.
        let call_result = match result {
            Ok(blocks) => CallToolResult::success(blocks),
            Err(e) => route_tool_error(e),
        };

        let ok = call_result.is_error.map_or(true, |e| !e);
        tracing::debug!(ok, "tool result");
        tracing::info!(
            tool = %req.name,
            duration_ms = tool_start.elapsed().as_millis() as u64,
            ok,
            "tool_done"
        );

        // Strip the absolute project root from all output to reduce token usage.
        // Agents work exclusively within the project directory; relative paths
        // carry all necessary information. The full root (e.g. /home/user/project)
        // is a long repeated prefix that appears in every "file" field and error
        // message. Buffer content (@tool_xxx refs) is covered here too: it only
        // re-enters the pipeline through run_command, which also passes through
        // call_tool.
        let root_prefix = self
            .agent
            .project_root()
            .await
            .map(|p| format!("{}/", p.display()))
            .unwrap_or_default();

        Ok(strip_project_root_from_result(call_result, &root_prefix))
    }
}

/// Returns true for tools that manage their own timeout internally and must not
/// be wrapped by the server-level `tool_timeout_secs` guard.
///
/// - `index_project` / `index_library`: embedding loops that run for many minutes.
/// - `run_command`: the caller supplies `timeout_secs` in the request params; the
///   server-level timeout is unaware of that value and would fire first, making
///   the per-request `timeout_secs` parameter effectively ignored.
fn tool_skips_server_timeout(name: &str) -> bool {
    matches!(name, "index_project" | "index_library" | "run_command")
}

impl ServerHandler for CodeScoutServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(self.instructions.clone())
    }

    async fn list_tools(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let tools = self
            .tools
            .iter()
            .map(|t| {
                let schema = t.input_schema();
                let schema_obj = schema.as_object().cloned().unwrap_or_default();
                McpTool::new(t.name().to_owned(), t.description().to_owned(), schema_obj)
            })
            .collect();

        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        req_ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let progress = Some(progress::ProgressReporter::new(
            req_ctx.peer.clone(),
            req_ctx.id.clone(),
        ));
        let peer = Some(req_ctx.peer.clone());
        self.call_tool_inner(req, progress, peer).await
    }
}

/// Route a tool `Err(e)` to the appropriate `CallToolResult`.
///
/// - [`RecoverableError`] → `isError: false` with a JSON body
///   `{"error": "...", "hint": "..."}`.  The LLM sees the problem and a
///   suggestion but sibling parallel calls are **not** aborted by the client.
/// - Any other error → `isError: true` (fatal; something truly broke).
///
/// [`RecoverableError`]: crate::tools::RecoverableError
fn route_tool_error(e: anyhow::Error) -> CallToolResult {
    if let Some(rec) = e.downcast_ref::<crate::tools::RecoverableError>() {
        let mut body = serde_json::json!({ "ok": false, "error": rec.message });
        if let Some(hint) = &rec.hint {
            body["hint"] = serde_json::json!(hint);
        }
        let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
        CallToolResult::success(vec![Content::text(text)])
    } else if e.to_string().contains("code -32800") {
        // LSP RequestCancelled — treat as recoverable so sibling parallel tool calls are not aborted.
        // Log at WARN so this is visible in diagnostic logs (otherwise it appears as ok=true).
        tracing::warn!("LSP RequestCancelled (-32800): {}", e);
        let body = serde_json::json!({
            "error": e.to_string(),
            "hint": "The LSP server cancelled this request (code -32800). Common causes:\n\
                     (1) Another process holds the workspace lock — e.g. another codescout instance \
                     or an editor (VS Code, IntelliJ) running a language server for the same project. \
                     For kotlin-lsp, each instance needs a separate --system-path to avoid lock \
                     contention on the IntelliJ platform's .app.lock file.\n\
                     (2) The server just started and is still running background indexing \
                     (can take 1-5 minutes on first run). Wait and retry the call."
        });
        let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string());
        CallToolResult::success(vec![Content::text(text)])
    } else {
        CallToolResult::error(vec![Content::text(e.to_string())])
    }
}

/// Entry point: start the MCP server with the chosen transport.
/// Generate a bearer token for HTTP transport authentication.
///
/// Combines high-resolution timestamp with process ID to produce a unique,
/// hard-to-guess hex string. This is NOT cryptographically secure — it is a
/// convenience default when the operator does not supply `--auth-token`.
pub fn generate_auth_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u64;
    // Mask to 64 bits each so the total is exactly 32 hex characters.
    let hi = nanos as u64;
    let lo = pid.wrapping_mul(0x517cc1b727220a95);
    format!("{:016x}{:016x}", hi, lo)
}

/// Wait for SIGINT (Ctrl-C), SIGTERM, or SIGHUP and return the signal name.
///
/// SIGHUP is sent when the parent process (e.g. Claude Code) exits abruptly without
/// sending SIGTERM first. Without this handler, codescout dies silently with no log entry.
pub(crate) async fn shutdown_signal() -> &'static str {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
        "SIGINT"
    };

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
            .expect("failed to install SIGHUP handler");
        tokio::select! {
            v = ctrl_c => v,
            _ = sigterm.recv() => "SIGTERM",
            _ = sighup.recv() => "SIGHUP",
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await
    }
}

/// Wraps `tokio::io::Stdin` to absorb transient `WouldBlock`/`EAGAIN` errors.
///
/// rmcp's `AsyncRwTransport::receive()` converts *any* IO error into `None`
/// (stream closed), causing the service loop to exit with `QuitReason::Closed`.
/// A transient `EAGAIN` (os error 11) on stdin — observed when Claude Code's
/// Node.js runtime temporarily sets the pipe to non-blocking mode — kills the
/// entire MCP server.
///
/// This wrapper intercepts `WouldBlock` at the `AsyncRead` level and converts
/// it to `Poll::Pending`, which is the correct async semantic: "not ready yet,
/// wake me when data arrives." This prevents rmcp from ever seeing the error.
struct ResilientStdin {
    inner: tokio::io::Stdin,
}

impl ResilientStdin {
    fn new(stdin: tokio::io::Stdin) -> Self {
        Self { inner: stdin }
    }
}

impl tokio::io::AsyncRead for ResilientStdin {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match std::pin::Pin::new(&mut self.inner).poll_read(cx, buf) {
            std::task::Poll::Ready(Err(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                tracing::warn!("stdin EAGAIN intercepted — converting to Pending");
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
            other => other,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    project: Option<PathBuf>,
    transport: &str,
    host: &str,
    port: u16,
    auth_token: Option<String>,
    debug: bool,
    diagnostic: bool,
    instance_id: Option<String>,
) -> Result<()> {
    // If no --project given, auto-detect from CWD (Claude Code launches servers from the project dir).
    // Canonicalize early so every downstream consumer (Agent, LspManager) sees the same
    // absolute path.  Without this, a relative `--project .` would store `home_root = "."`
    // while `activate_project(".")` later canonicalizes to `/abs/path`, making `is_home()`
    // return false and causing path-form drift across the system.
    let project = project
        .or_else(|| std::env::current_dir().ok())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p));
    let lsp = match project.clone() {
        Some(root) => LspManager::new_arc_with_root(root),
        None => LspManager::new_arc(),
    };
    let agent = Agent::new(project).await?;

    let instance_tag = instance_id.as_deref().unwrap_or("----");

    if diagnostic {
        let project_display = agent
            .project_root()
            .await
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string());
        tracing::info!(
            pid = std::process::id(),
            version = env!("CARGO_PKG_VERSION"),
            instance = %instance_tag,
            project = %project_display,
            transport = %transport,
            "codescout_start"
        );
    }

    // Heartbeat: in debug mode or diagnostic mode — distinguishes idle from hung.
    if debug || diagnostic {
        let agent_hb = agent.clone();
        let lsp_hb = lsp.clone();
        let start = tokio::time::Instant::now();
        let instance_tag_hb = instance_tag.to_owned();
        let _heartbeat = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            interval.tick().await; // Skip the immediate first tick
            loop {
                interval.tick().await;
                let uptime_secs = start.elapsed().as_secs();
                let lsp_servers = lsp_hb.active_languages().await;
                let active_projects: usize = if agent_hb.project_root().await.is_some() {
                    1
                } else {
                    0
                };
                tracing::info!(
                    instance = %instance_tag_hb,
                    uptime_secs,
                    active_projects,
                    ?lsp_servers,
                    "heartbeat"
                );
            }
        });
    }

    match transport {
        "stdio" => {
            if auth_token.is_some() {
                tracing::warn!("--auth-token is ignored for stdio transport");
            }
            tracing::info!("codescout MCP server ready (stdio)");
            let server = CodeScoutServer::from_parts(agent, lsp.clone()).await;
            let (stdin, stdout) = rmcp::transport::stdio();
            let service = server
                .serve((ResilientStdin::new(stdin), stdout))
                .await
                .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;

            // Wait for service to end OR shutdown signal
            tokio::select! {
                result = service.waiting() => {
                    match result {
                        Ok(reason) => tracing::info!(instance = %instance_tag, ?reason, "service_exit"),
                        Err(e) => {
                            tracing::info!(instance = %instance_tag, %e, "service_exit join_error");
                            return Err(anyhow::anyhow!("MCP server exited: {}", e));
                        }
                    }
                }
                reason = shutdown_signal() => {
                    tracing::info!(instance = %instance_tag, reason, "service_exit");
                }
            }

            // Gracefully shut down all LSP servers
            tracing::info!("Shutting down LSP servers...");
            lsp.shutdown_all().await;
            tracing::info!("All LSP servers shut down");
            Ok(())
        }
        #[cfg(feature = "http")]
        "http" => {
            use rmcp::transport::streamable_http_server::{
                session::local::LocalSessionManager, StreamableHttpServerConfig,
                StreamableHttpService,
            };

            // Build the server once (async), then clone per session.
            let server = CodeScoutServer::from_parts(agent, lsp.clone()).await;

            let ct = tokio_util::sync::CancellationToken::new();
            let service = StreamableHttpService::new(
                move || Ok(server.clone()),
                LocalSessionManager::default().into(),
                StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
            );

            // Bearer token auth middleware
            let token = auth_token.unwrap_or_else(|| {
                let mut buf = [0u8; 32];
                // /dev/urandom is always available on Linux/macOS — no crate needed.
                if let Ok(bytes) = std::fs::read("/dev/urandom") {
                    for (i, b) in bytes.iter().take(32).enumerate() {
                        buf[i] = *b;
                    }
                } else {
                    // Fallback: hash of pid + timestamp (not cryptographic, but usable)
                    let seed = std::process::id() as u64
                        ^ std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as u64;
                    for (i, b) in seed.to_le_bytes().iter().cycle().take(32).enumerate() {
                        buf[i] = b.wrapping_add(i as u8);
                    }
                }
                let generated: String = buf
                    .iter()
                    .map(|b| {
                        let idx = b % 62;
                        match idx {
                            0..=9 => (b'0' + idx) as char,
                            10..=35 => (b'a' + idx - 10) as char,
                            _ => (b'A' + idx - 36) as char,
                        }
                    })
                    .collect();
                eprintln!("Generated auth token: {generated}");
                generated
            });

            let router =
                axum::Router::new()
                    .nest_service("/mcp", service)
                    .layer(axum::middleware::from_fn(
                        move |req: axum::extract::Request, next: axum::middleware::Next| {
                            let expected = format!("Bearer {token}");
                            async move {
                                match req.headers().get("authorization") {
                                    Some(v) if v == expected.as_str() => next.run(req).await,
                                    _ => axum::http::StatusCode::UNAUTHORIZED.into_response(),
                                }
                            }
                        },
                    ));

            let bind_addr = format!("{host}:{port}");
            let tcp_listener = tokio::net::TcpListener::bind(&bind_addr)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to bind {bind_addr}: {e}"))?;

            tracing::info!(
                %bind_addr,
                instance = %instance_tag,
                "codescout MCP server ready (HTTP)"
            );
            eprintln!("codescout listening on http://{bind_addr}/mcp");

            let ct_shutdown = ct.clone();
            let instance_tag_http = instance_tag.to_owned();
            axum::serve(tcp_listener, router)
                .with_graceful_shutdown(async move {
                    let reason = shutdown_signal().await;
                    tracing::info!(instance = %instance_tag_http, reason, "service_exit");
                    ct_shutdown.cancel();
                })
                .await
                .map_err(|e| anyhow::anyhow!("HTTP server error: {e}"))?;

            // Gracefully shut down all LSP servers
            tracing::info!("Shutting down LSP servers...");
            lsp.shutdown_all().await;
            tracing::info!("All LSP servers shut down");
            Ok(())
        }
        #[cfg(not(feature = "http"))]
        "http" => {
            let _ = (host, port, auth_token);
            anyhow::bail!(
                "HTTP transport is not available in this build. \
                 Build with `--features http` to enable it."
            );
        }
        other => anyhow::bail!("Unknown transport '{}'. Use 'stdio' or 'http'.", other),
    }
}

/// Strips the absolute project root prefix from all text content blocks in a
/// `CallToolResult`. This normalises tool output to relative paths, reducing
/// token usage — the prefix (e.g. "/home/user/work/project/") is identical in
/// every response and carries no information since agents always operate within
/// the project directory.
///
/// `root_prefix` must end with `/`. Pass an empty string when no project is
/// active; the replace becomes a no-op.
///
/// Note: values like `"project_root": "/abs/path"` in `activate_project` / `get_config`
/// responses are intentionally not stripped — they use a bare absolute path without a
/// trailing slash, so they do not match `root_prefix` and pass through unchanged.
/// Agents that need to call `activate_project` again can still use those values as-is.
///
/// Buffer content (`@tool_xxx` refs) is covered automatically: it only
/// re-enters the pipeline through `run_command`, which also passes through
/// `call_tool` and gets stripped there.
fn strip_project_root_from_result(mut result: CallToolResult, root_prefix: &str) -> CallToolResult {
    if root_prefix.is_empty() {
        return result;
    }
    debug_assert!(
        root_prefix.ends_with('/'),
        "root_prefix must end with '/' to avoid stripping partial path components"
    );
    for block in &mut result.content {
        if let RawContent::Text(ref mut t) = block.raw {
            t.text = t.text.replace(root_prefix, "");
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use tempfile::tempdir;

    async fn make_server() -> (tempfile::TempDir, CodeScoutServer) {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let server = CodeScoutServer::new(agent).await;
        (dir, server)
    }

    async fn make_server_no_project() -> CodeScoutServer {
        let agent = Agent::new(None).await.unwrap();
        CodeScoutServer::new(agent).await
    }

    #[tokio::test]
    async fn server_registers_all_tools() {
        let (_dir, server) = make_server().await;
        let expected_tools = [
            "read_file",
            "list_dir",
            "grep",
            "create_file",
            "glob",
            "edit_file",
            "edit_markdown",
            "read_markdown",
            "run_command",
            "onboarding",
            "find_symbol",
            "find_references",
            "list_symbols",
            "replace_symbol",
            "insert_code",
            "rename_symbol",
            "remove_symbol",
            "goto_definition",
            "hover",
            "memory",
            "semantic_search",
            "index_project",
            "index_status",
            "activate_project",
            "project_status",
            "list_libraries",
            "register_library",
        ];
        assert_eq!(
            server.tools.len(),
            expected_tools.len(),
            "tool count mismatch: expected {}, got {}\nregistered: {:?}",
            expected_tools.len(),
            server.tools.len(),
            server.tools.iter().map(|t| t.name()).collect::<Vec<_>>()
        );
        for name in &expected_tools {
            assert!(
                server.find_tool(name).is_some(),
                "tool '{}' not found in server",
                name
            );
        }
    }

    #[tokio::test]
    async fn find_tool_returns_none_for_unknown() {
        let (_dir, server) = make_server().await;
        assert!(server.find_tool("nonexistent_tool").is_none());
        assert!(server.find_tool("").is_none());
        assert!(server.find_tool("READ_FILE").is_none()); // case-sensitive
    }

    #[tokio::test]
    async fn tool_names_are_unique() {
        let (_dir, server) = make_server().await;
        let mut names: Vec<&str> = server.tools.iter().map(|t| t.name()).collect();
        let original_len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), original_len, "duplicate tool names found");
    }

    #[tokio::test]
    async fn all_tools_have_valid_schemas() {
        let (_dir, server) = make_server().await;
        for tool in &server.tools {
            let schema = tool.input_schema();
            assert!(
                schema.is_object(),
                "tool '{}' schema is not an object",
                tool.name()
            );
            // Every schema should have "type": "object" at minimum
            assert_eq!(
                schema["type"],
                "object",
                "tool '{}' schema missing type:object",
                tool.name()
            );
        }
    }

    #[tokio::test]
    async fn all_tools_have_descriptions() {
        let (_dir, server) = make_server().await;
        for tool in &server.tools {
            let desc = tool.description();
            assert!(
                !desc.is_empty(),
                "tool '{}' has empty description",
                tool.name()
            );
        }
    }

    #[tokio::test]
    async fn get_info_contains_instructions() {
        let (_dir, server) = make_server().await;
        let info = server.get_info();
        assert!(info.instructions.is_some());
        let instructions = info.instructions.unwrap();
        assert!(!instructions.is_empty());
    }

    #[tokio::test]
    async fn get_info_without_project_still_works() {
        let server = make_server_no_project().await;
        let info = server.get_info();
        assert!(info.instructions.is_some());
    }

    #[tokio::test]
    async fn server_instructions_mention_project_when_active() {
        let (_dir, server) = make_server().await;
        let info = server.get_info();
        let instructions = info.instructions.unwrap();
        // When a project is active, instructions should reference it
        assert!(
            instructions.contains("Project:") || instructions.contains("project"),
            "instructions should mention the active project"
        );
    }

    #[test]
    fn generate_auth_token_produces_nonempty_hex() {
        let token = super::generate_auth_token();
        assert!(!token.is_empty(), "token must not be empty");
        assert_eq!(token.len(), 32, "token should be 32 hex chars");
        assert!(
            token.chars().all(|c| c.is_ascii_hexdigit()),
            "token must be valid hex: {}",
            token
        );
    }

    #[test]
    fn generate_auth_token_is_unique_across_calls() {
        let t1 = super::generate_auth_token();
        let t2 = super::generate_auth_token();
        // The nanos component changes between calls, so tokens should differ.
        // In the astronomically unlikely case of a collision, the test is still
        // correct — but practically this always passes.
        assert_ne!(t1, t2, "consecutive tokens should differ");
    }

    #[tokio::test]
    async fn shell_tool_allowed_by_default() {
        let (_dir, server) = make_server().await;
        let security = server.agent.security_config().await;
        assert!(security.shell_enabled);
        assert!(crate::util::path_security::check_tool_access("run_command", &security).is_ok());
    }

    // ── route_tool_error ───────────────────────────────────────────────────

    #[test]
    fn recoverable_error_routes_to_success_not_is_error() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::new("path not found"));
        let result = route_tool_error(err);
        assert!(
            result.is_error != Some(true),
            "RecoverableError must not set isError:true"
        );
    }

    #[test]
    fn recoverable_error_body_has_ok_false() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::new("old_string not found"));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            body["ok"],
            serde_json::Value::Bool(false),
            "RecoverableError body must include ok:false so models cannot confuse it with the success string \"ok\""
        );
    }

    #[test]
    fn recoverable_error_body_has_error_key() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::new(
            "path not found: foo/bar",
        ));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["error"], "path not found: foo/bar");
    }

    #[test]
    fn recoverable_error_body_includes_hint_when_present() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::with_hint(
            "not found",
            "use list_dir to explore",
        ));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["hint"], "use list_dir to explore");
    }

    #[test]
    fn recoverable_error_without_hint_omits_hint_from_body() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::new("not found"));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(body.get("hint").is_none(), "hint key must be absent");
    }

    #[test]
    fn plain_anyhow_error_routes_to_is_error_true() {
        let err = anyhow::anyhow!("LSP crashed unexpectedly");
        let result = route_tool_error(err);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn lsp_request_cancelled_routes_to_recoverable_not_fatal() {
        // Kotlin-lsp (and other IntelliJ-based servers) send code -32800 when
        // they cancel a request due to concurrent load.  This must NOT produce
        // isError:true, otherwise Claude Code aborts all sibling parallel calls.
        let err = anyhow::anyhow!("LSP error (code -32800): cancelled");
        let result = route_tool_error(err);
        assert!(
            result.is_error != Some(true),
            "LSP RequestCancelled must not set isError:true"
        );
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(body.get("hint").is_some(), "must include retry hint");
    }

    #[test]
    fn other_lsp_errors_still_route_to_is_error_true() {
        // Only -32800 gets the recoverable treatment; other LSP errors are fatal.
        let err = anyhow::anyhow!("LSP error (code -32603): internal error");
        let result = route_tool_error(err);
        assert_eq!(result.is_error, Some(true));
    }

    // ── timeout dispatch ───────────────────────────────────────────────────

    #[test]
    fn run_command_skips_server_timeout() {
        // Regression: run_command accepts a per-request timeout_secs parameter.
        // The server-level tool_timeout_secs (default 60s) must not wrap it,
        // otherwise the server fires first and the per-request value is ignored.
        assert!(
            tool_skips_server_timeout("run_command"),
            "run_command must not be wrapped by the server-level timeout"
        );
    }

    #[test]
    fn indexing_tools_skip_server_timeout() {
        assert!(tool_skips_server_timeout("index_project"));
        assert!(tool_skips_server_timeout("index_library"));
    }

    #[test]
    fn other_tools_do_not_skip_server_timeout() {
        for name in &["read_file", "edit_file", "find_symbol", "semantic_search"] {
            assert!(
                !tool_skips_server_timeout(name),
                "tool '{}' should be subject to the server-level timeout",
                name
            );
        }
    }

    #[tokio::test]
    async fn call_tool_strips_project_root_from_output() {
        let (dir, server) = make_server().await;
        let root = dir.path().to_string_lossy().to_string();

        let req = CallToolRequestParams::new("list_dir")
            .with_arguments(serde_json::from_value(serde_json::json!({"path": "."})).unwrap());
        let result = server.call_tool_inner(req, None, None).await.unwrap();

        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("");

        assert!(
            !text.is_empty(),
            "list_dir returned empty output — the strip test is not actually exercising anything"
        );
        assert!(
            !text.contains(&root),
            "Expected absolute root to be stripped, but found it in output:\n{text}"
        );
    }

    #[test]
    fn strip_project_root_removes_prefix_from_text_content() {
        let prefix = "/home/user/myproject/";
        let result = CallToolResult::success(vec![Content::text(
            r#"{"file":"/home/user/myproject/src/foo.rs","line":1}"#,
        )]);
        let stripped = strip_project_root_from_result(result, prefix);
        let text = extract_text(&stripped);
        assert_eq!(text, r#"{"file":"src/foo.rs","line":1}"#);
    }

    #[test]
    fn strip_project_root_no_op_when_prefix_empty() {
        let result = CallToolResult::success(vec![Content::text("some output")]);
        let stripped = strip_project_root_from_result(result, "");
        assert_eq!(extract_text(&stripped), "some output");
    }

    #[test]
    fn strip_project_root_no_op_when_prefix_absent() {
        let prefix = "/home/user/myproject/";
        let result = CallToolResult::success(vec![Content::text("no paths here")]);
        let stripped = strip_project_root_from_result(result, prefix);
        assert_eq!(extract_text(&stripped), "no paths here");
    }

    fn extract_text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.clone()))
            .unwrap_or_default()
    }
}

// ── ResilientStdin ────────────────────────────────────────────────────

/// A mock reader that returns WouldBlock on the first poll, then data.
#[allow(dead_code)]
struct WouldBlockThenData {
    returned_eagain: bool,
}

impl tokio::io::AsyncRead for WouldBlockThenData {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if !self.returned_eagain {
            self.returned_eagain = true;
            std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "EAGAIN",
            )))
        } else {
            buf.put_slice(b"hello");
            std::task::Poll::Ready(Ok(()))
        }
    }
}

/// Verifies that WouldBlock from the inner reader is converted to Pending,
/// not surfaced as an error that would kill the rmcp service loop.
#[tokio::test]
async fn resilient_stdin_absorbs_would_block() {
    use tokio::io::AsyncReadExt;

    // We can't wrap WouldBlockThenData in ResilientStdin directly since
    // ResilientStdin is hard-coded to tokio::io::Stdin.  Instead, test
    // the same logic inline with a generic version.
    struct ResilientReader<R> {
        inner: R,
    }
    impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for ResilientReader<R> {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            match std::pin::Pin::new(&mut self.inner).poll_read(cx, buf) {
                std::task::Poll::Ready(Err(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
                other => other,
            }
        }
    }

    let mock = WouldBlockThenData {
        returned_eagain: false,
    };
    let mut reader = ResilientReader { inner: mock };
    let mut buf = [0u8; 16];
    // This would fail with WouldBlock if the wrapper didn't absorb it.
    let n = reader.read(&mut buf).await.expect("should not error");
    assert_eq!(&buf[..n], b"hello");
}
