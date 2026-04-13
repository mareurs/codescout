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
    session_id: String,
    debug: bool,
    /// Last capabilities snapshot that was broadcast to the client via
    /// `notifications/tools/list_changed`. Used to suppress redundant broadcasts.
    last_broadcast_caps: Arc<std::sync::Mutex<Option<crate::tools::ToolCapabilities>>>,
    /// MCP resource registry — replaceable on `activate_project` to pick up a new
    /// memory dir. Held behind `RwLock<Arc<...>>` so list/read only need a read lock
    /// while replacement takes a write lock.
    resources: Arc<tokio::sync::RwLock<Arc<crate::mcp_resources::ResourceRegistry>>>,
}

impl CodeScoutServer {
    pub async fn new(agent: Agent) -> Self {
        let lsp = match agent.project_root().await {
            Some(root) => LspManager::new_arc_with_root(root),
            None => LspManager::new_arc(),
        };
        Self::from_parts(agent, lsp, false).await
    }

    /// Create a server with an existing LspManager (used for HTTP multi-session).
    pub async fn from_parts(agent: Agent, lsp: Arc<dyn LspProvider>, debug: bool) -> Self {
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
        let resources = Arc::new(tokio::sync::RwLock::new(Arc::new(
            build_resource_registry(&agent).await,
        )));
        Self {
            agent,
            lsp,
            output_buffer,
            tools,
            instructions,
            section_coverage,
            session_id: uuid::Uuid::new_v4().to_string(),
            debug,
            last_broadcast_caps: Arc::new(std::sync::Mutex::new(None)),
            resources,
        }
    }

    fn find_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    /// Replace the resource registry after an `activate_project` call that may have
    /// changed the active memory directory.
    async fn refresh_resources(&self) {
        let new_rr = build_resource_registry(&self.agent).await;
        *self.resources.write().await = Arc::new(new_rr);
    }

    /// Probe the current project state and return a snapshot of its capabilities.
    ///
    /// All probes are non-panicking — unknown or missing state falls back to `false`.
    /// Called by `list_tools` and by `call_tool` (to detect capability changes after
    /// `activate_project`).
    async fn current_capabilities(&self) -> crate::tools::ToolCapabilities {
        // has_lsp: true when any language in the active project has a registered LSP server config.
        let has_lsp = self
            .agent
            .with_project(|p| {
                let has = p
                    .config
                    .project
                    .languages
                    .iter()
                    .any(|lang| crate::lsp::servers::has_lsp_config(lang));
                Ok(has)
            })
            .await
            .unwrap_or(false);

        // has_embeddings: compile-time guard — true whenever at least one embedding backend
        // is compiled in. Both local-embed and remote-embed are in the default feature set.
        // No runtime "model loaded?" check exists without actually attempting a connection,
        // so we rely on the feature flags alone.
        let has_embeddings = cfg!(any(feature = "local-embed", feature = "remote-embed"));

        // has_git_remote: true when the active project's git repository has at least one remote.
        let has_git_remote = self
            .agent
            .project_root()
            .await
            .and_then(|root| git2::Repository::open(&root).ok())
            .and_then(|repo| repo.remotes().ok())
            .map(|remotes| !remotes.is_empty())
            .unwrap_or(false);

        // has_libraries: true when at least one library is registered for the active project.
        let has_libraries = self
            .agent
            .library_registry()
            .await
            .map(|reg| !reg.all().is_empty())
            .unwrap_or(false);

        crate::tools::ToolCapabilities {
            has_lsp,
            has_embeddings,
            has_git_remote,
            has_libraries,
        }
    }

    /// Core tool dispatch, separated from the MCP trait method so tests can
    /// call it without constructing a `RequestContext`.
    ///
    /// `cancel_token` carries the per-request cancellation signal from rmcp
    /// (driven by `CancelledNotification` from the client when the user presses
    /// Escape). When the token fires we drop the in-flight tool future, which
    /// cascades through to `kill_on_drop` on any spawned shell child — so the
    /// process tree is reaped instead of running to completion in the background
    /// while Claude Code closes the MCP connection. Tests that don't care about
    /// cancellation can pass `tokio_util::sync::CancellationToken::new()` —
    /// a fresh token never fires.
    #[tracing::instrument(skip_all, fields(tool = %req.name))]
    async fn call_tool_inner(
        &self,
        req: CallToolRequestParams,
        progress: Option<Arc<progress::ProgressReporter>>,
        peer: Option<Peer<RoleServer>>,
        cancel_token: tokio_util::sync::CancellationToken,
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

        let recorder = UsageRecorder::new(self.agent.clone(), self.debug, self.session_id.clone());
        let input_for_record = input.clone();

        // Race the tool call against (a) the server-level timeout and (b) the
        // per-request cancellation token. Cancellation is the load-bearing arm:
        // when the user presses Escape, rmcp cancels `cancel_token`, the select!
        // arm fires, and the tool future is dropped — which kills any spawned
        // child via `kill_on_drop`. Without this, the future runs to completion
        // and the late response makes Claude Code close the MCP connection.
        let tool_call_fut = recorder.record_content(&req.name, &input_for_record, || {
            tool.call_content(input, &ctx)
        });

        let result = if let Some(secs) = timeout_secs {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    Err(anyhow::anyhow!("request cancelled by client"))
                }
                res = tokio::time::timeout(
                    std::time::Duration::from_secs(secs),
                    tool_call_fut,
                ) => res.unwrap_or_else(|_| {
                    Err(anyhow::anyhow!(
                        "Tool '{}' timed out after {}s. \
                         Increase tool_timeout_secs in .codescout/project.toml if needed.",
                        req.name,
                        secs
                    ))
                }),
            }
        } else {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    Err(anyhow::anyhow!("request cancelled by client"))
                }
                res = tool_call_fut => res,
            }
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
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .build(),
        )
        .with_instructions(self.instructions.clone())
    }

    async fn list_tools(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let caps = self.current_capabilities().await;
        let tools = self
            .tools
            .iter()
            .filter(|t| t.availability(&caps).is_available(&caps))
            .map(|t| {
                let schema = t.input_schema();
                let schema_obj = schema.as_object().cloned().unwrap_or_default();
                McpTool::new(t.name().to_owned(), t.description().to_owned(), schema_obj)
            })
            .collect();

        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn list_resources(
        &self,
        _req: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<rmcp::model::ListResourcesResult, McpError> {
        use rmcp::model::{AnnotateAble as _, RawResource};
        let rr = self.resources.read().await.clone();
        let resources = rr
            .list()
            .into_iter()
            .map(|d| {
                let mut raw = RawResource::new(d.uri, d.name);
                if let Some(desc) = d.description {
                    raw = raw.with_description(desc);
                }
                raw = raw.with_mime_type(d.mime_type);
                raw.no_annotation()
            })
            .collect();
        Ok(rmcp::model::ListResourcesResult {
            meta: None,
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        req: rmcp::model::ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<rmcp::model::ReadResourceResult, McpError> {
        use crate::mcp_resources::{ResourceBytes, ResourceError};
        use rmcp::model::{ReadResourceResult, ResourceContents};
        let rr = self.resources.read().await.clone();
        match rr.read(&req.uri).await {
            Ok(ResourceBytes::Text(t)) => {
                Ok(ReadResourceResult::new(vec![ResourceContents::text(
                    t, &req.uri,
                )]))
            }
            // Blob resources are not yet produced by any current provider;
            // callers should not encounter this in practice.
            Ok(ResourceBytes::Blob(_)) => Err(McpError::internal_error(
                "blob resource encoding not supported in this build",
                None,
            )),
            Err(ResourceError::NotFound(u)) => Err(McpError::resource_not_found(
                format!("resource not found: {u}"),
                None,
            )),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        req_ctx: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let is_activate = req.name == "activate_project";
        let progress = Some(progress::ProgressReporter::new(
            req_ctx.peer.clone(),
            req_ctx.id.clone(),
        ));
        let peer = Some(req_ctx.peer.clone());
        let result = self.call_tool_inner(req, progress, peer).await?;

        // After a successful activate_project, check whether the capability set has
        // changed. If it has, emit notifications/tools/list_changed so the client
        // can refresh its tool list without a full reconnect.
        if is_activate {
            let new_caps = self.current_capabilities().await;
            let caps_changed = {
                let mut last = self.last_broadcast_caps.lock().unwrap();
                let changed = last.as_ref() != Some(&new_caps);
                if changed {
                    *last = Some(new_caps);
                }
                changed
            };
            if caps_changed {
                let _ = req_ctx.peer.notify_tool_list_changed().await;
            }

            // Rebuild the resource registry to pick up the new memory dir.
            self.refresh_resources().await;
        }

        Ok(result)
    }
}

/// Build a fresh [`crate::mcp_resources::ResourceRegistry`] from the current agent state.
///
/// Called at server construction and again after each `activate_project` to pick up
/// the new memory directory.  Any provider that can't be constructed (e.g. the project
/// root is not yet set) is silently skipped — the registry is always valid even when
/// empty.
async fn build_resource_registry(agent: &Agent) -> crate::mcp_resources::ResourceRegistry {
    use crate::mcp_resources::{
        doc::{DocProvider, DocSource},
        memory::MemoryProvider,
        project_summary::{AgentSummarySource, ProjectSummaryProvider},
        ResourceRegistry,
    };

    let mut rr = ResourceRegistry::new();

    // Static docs — register only when the project root is known so the paths exist.
    if let Some(project_root) = agent.project_root().await {
        let _ = rr.try_register(Box::new(DocProvider::new(vec![
            DocSource {
                uri: "doc://progressive-disclosure".into(),
                name: "progressive-disclosure".into(),
                description: Some(
                    "Output sizing, overflow hints, agent guidance for codescout tools.".into(),
                ),
                path: project_root.join("docs/PROGRESSIVE_DISCOVERABILITY.md"),
            },
            DocSource {
                uri: "doc://tool-misbehaviors".into(),
                name: "tool-misbehaviors".into(),
                description: Some("Living log of observed codescout tool bugs.".into()),
                path: project_root.join("docs/TODO-tool-misbehaviors.md"),
            },
        ])));
    }

    // Memory dir — derived from the active project's MemoryStore.
    if let Ok(memory_dir) = agent
        .with_project(|p| Ok(p.memory.dir().to_path_buf()))
        .await
    {
        let _ = rr.try_register(Box::new(MemoryProvider::new(memory_dir)));
    }

    // Project summary — always registered; falls back gracefully when no project is active.
    let _ = rr.try_register(Box::new(ProjectSummaryProvider::new(
        AgentSummarySource::new(agent.clone()),
    )));

    rr
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

    if debug {
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

    // Heartbeat: distinguishes idle from hung.
    if debug {
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
            let server = CodeScoutServer::from_parts(agent, lsp.clone(), debug).await;
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
            let server = CodeScoutServer::from_parts(agent, lsp.clone(), debug).await;

            let ct = tokio_util::sync::CancellationToken::new();
            let service = StreamableHttpService::new(
                move || {
                    let mut s = server.clone();
                    s.session_id = uuid::Uuid::new_v4().to_string();
                    Ok(s)
                },
                LocalSessionManager::default().into(),
                StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
            );

            // Bearer token auth middleware
            let token = auth_token.unwrap_or_else(|| {
                let mut buf = [0u8; 32];
                // Read exactly 32 bytes from /dev/urandom.  Must use File::open +
                // read_exact — std::fs::read tries to read the entire "file" which
                // is undefined for device nodes.
                use std::io::Read;
                let ok = std::fs::File::open("/dev/urandom")
                    .and_then(|mut f| f.read_exact(&mut buf))
                    .is_ok();
                if !ok {
                    // Fallback: mix pid, thread id, timestamp, and address-space
                    // entropy.  Not cryptographic, but unpredictable enough for a
                    // local-only convenience token.
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::process::id().hash(&mut hasher);
                    std::thread::current().id().hash(&mut hasher);
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                        .hash(&mut hasher);
                    // Hash twice with different seeds to fill 32 bytes.
                    let h1 = hasher.finish().to_le_bytes();
                    h1[0].hash(&mut hasher);
                    let h2 = hasher.finish().to_le_bytes();
                    buf[..8].copy_from_slice(&h1);
                    buf[8..16].copy_from_slice(&h2);
                    h2[0].hash(&mut hasher);
                    let h3 = hasher.finish().to_le_bytes();
                    h3[0].hash(&mut hasher);
                    let h4 = hasher.finish().to_le_bytes();
                    buf[16..24].copy_from_slice(&h3);
                    buf[24..32].copy_from_slice(&h4);
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
/// token usage — the prefix (e.g. `"/home/user/work/project/"`) is identical
/// in every response and carries no information since agents always operate
/// within the project directory.
///
/// `root_prefix` must end with `/`. Pass an empty string when no project is
/// active; the replace becomes a no-op.
///
/// **Disambiguation note**: when stripping fires, a trailing content block is
/// appended:
///   `[codescout] paths are relative to /abs/project/root`
/// This prevents LLMs from misreading stripped paths in file *content* (e.g.
/// a `"statusLine"` value in `settings.json`) as evidence that the file stores
/// a relative path. The note only appears when stripping actually changes
/// something — navigation tools whose output never contained absolute paths
/// get no note.
///
/// Note: values like `"project_root": "/abs/path"` in `activate_project` /
/// `get_config` responses are intentionally not stripped — they use a bare
/// absolute path without a trailing slash, so they do not match `root_prefix`
/// and pass through unchanged. Agents that need to call `activate_project`
/// again can still use those values as-is.
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
    let mut stripped = false;
    for block in &mut result.content {
        if let RawContent::Text(ref mut t) = block.raw {
            let replaced = t.text.replace(root_prefix, "");
            if replaced != t.text {
                stripped = true;
                t.text = replaced;
            }
        }
    }
    // Append a one-liner so agents can distinguish stripped absolute paths
    // (which appear as bare relative paths) from genuine relative path values
    // stored in file content. Without this note, an LLM reading a config file
    // that stores an absolute path as a string value would see a relative path
    // and incorrectly conclude the file needs to be "fixed".
    if stripped {
        let root = root_prefix.trim_end_matches('/');
        result.content.push(Content::text(format!(
            "[codescout] paths are relative to {root}"
        )));
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
        let result = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();

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

    #[tokio::test]
    async fn call_tool_cancellation_kills_long_running_run_command() {
        // Regression for the "codescout disconnects after Escape on long
        // run_command" bug: when the per-request CancellationToken fires (rmcp
        // received CancelledNotification), the tool future must drop and the
        // spawned shell child must be reaped — instead of running to completion
        // and triggering Claude Code to close the MCP connection.
        //
        // The test runs `sleep 5 && touch <marker>` with timeout_secs=30,
        // cancels after 200ms, asserts the call returns within ~1s, and
        // verifies the marker file is *never* created (proving the sleep was
        // killed before reaching `touch`).
        let (dir, server) = make_server().await;
        let marker = dir.path().join("cancel-test-marker");
        let marker_str = marker.to_string_lossy().to_string();

        let req = CallToolRequestParams::new("run_command").with_arguments(
            serde_json::from_value(serde_json::json!({
                "command": format!("sleep 5 && touch '{}'", marker_str),
                "timeout_secs": 30u64,
            }))
            .unwrap(),
        );

        let ct = tokio_util::sync::CancellationToken::new();
        let server_clone = server.clone();
        let ct_clone = ct.clone();
        let handle = tokio::spawn(async move {
            server_clone
                .call_tool_inner(req, None, None, ct_clone)
                .await
        });

        // Let the shell child actually start before cancelling.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let cancel_at = std::time::Instant::now();
        ct.cancel();

        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("call_tool_inner must return within 2s of cancel — it kept running")
            .expect("spawned task panicked");

        let elapsed = cancel_at.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(1000),
            "expected return within 1s of cancel, took {elapsed:?}"
        );

        let r = result.unwrap();
        let body = r
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("");
        assert!(
            body.contains("cancelled") || r.is_error == Some(true),
            "expected cancellation indication in result, got: {body}"
        );

        // Wait past the original sleep duration — if the child wasn't killed,
        // the marker file would now exist.
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        assert!(
            !marker.exists(),
            "marker file {marker:?} exists — sleep child was NOT killed by cancel"
        );
    }

    #[test]
    fn strip_project_root_removes_prefix_from_text_content() {
        let prefix = "/home/user/myproject/";
        let result = CallToolResult::success(vec![Content::text(
            r#"{"file":"/home/user/myproject/src/foo.rs","line":1}"#,
        )]);
        let stripped = strip_project_root_from_result(result, prefix);
        // First block: prefix replaced
        assert_eq!(extract_text(&stripped), r#"{"file":"src/foo.rs","line":1}"#);
        // Trailing block: disambiguation note with the absolute root
        assert_eq!(stripped.content.len(), 2);
        let note = stripped.content[1]
            .as_text()
            .map(|t| t.text.as_str())
            .unwrap_or("");
        assert!(
            note.contains("/home/user/myproject"),
            "note should carry the absolute root, got: {note}"
        );
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
        // Content unchanged, and no disambiguation note appended since nothing was stripped
        assert_eq!(extract_text(&stripped), "no paths here");
        assert_eq!(
            stripped.content.len(),
            1,
            "no note should be added when nothing was stripped"
        );
    }

    #[tokio::test]
    async fn list_tools_hides_lsp_tools_when_no_lsp() {
        use crate::tools::{Availability, ToolCapabilities};

        // Verify Availability filtering logic directly — no LSP config in the
        // temp project, so has_lsp should be false and LSP tools should be hidden.
        let caps_no_lsp = ToolCapabilities {
            has_lsp: false,
            has_embeddings: true,
            has_git_remote: false,
            has_libraries: false,
        };
        let caps_with_lsp = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: false,
            has_libraries: false,
        };

        assert!(
            !Availability::RequiresLsp.is_available(&caps_no_lsp),
            "RequiresLsp should not be available when has_lsp=false"
        );
        assert!(
            Availability::RequiresLsp.is_available(&caps_with_lsp),
            "RequiresLsp should be available when has_lsp=true"
        );

        // Verify server-level filtering: build a server and check that the tool
        // names returned by current_capabilities + filter match expectations.
        let (_dir, server) = make_server().await;
        let caps = server.current_capabilities().await;

        // In a fresh temp dir with no languages configured, has_lsp should be false.
        // LSP tools (hover, goto_definition, find_references, rename_symbol) must be hidden.
        if !caps.has_lsp {
            let visible: Vec<&str> = server
                .tools
                .iter()
                .filter(|t| t.availability(&caps).is_available(&caps))
                .map(|t| t.name())
                .collect();
            for lsp_tool in &[
                "hover",
                "goto_definition",
                "find_references",
                "rename_symbol",
            ] {
                assert!(
                    !visible.contains(lsp_tool),
                    "LSP tool '{}' should be hidden when has_lsp=false",
                    lsp_tool
                );
            }
            // Non-LSP tools must still be visible.
            for always_tool in &["read_file", "list_dir", "memory", "activate_project"] {
                assert!(
                    visible.contains(always_tool),
                    "Always-available tool '{}' should remain visible",
                    always_tool
                );
            }
        }
    }

    #[tokio::test]
    async fn list_tools_shows_lsp_tools_when_has_lsp() {
        use crate::tools::ToolCapabilities;

        let caps_with_lsp = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: false,
            has_libraries: false,
        };

        let (_dir, server) = make_server().await;
        let visible: Vec<&str> = server
            .tools
            .iter()
            .filter(|t| t.availability(&caps_with_lsp).is_available(&caps_with_lsp))
            .map(|t| t.name())
            .collect();

        for lsp_tool in &[
            "hover",
            "goto_definition",
            "find_references",
            "rename_symbol",
        ] {
            assert!(
                visible.contains(lsp_tool),
                "LSP tool '{}' should be visible when has_lsp=true",
                lsp_tool
            );
        }
    }

    #[tokio::test]
    async fn current_capabilities_returns_without_panic() {
        // Smoke test: current_capabilities must not panic even for a fresh project.
        let (_dir, server) = make_server().await;
        let caps = server.current_capabilities().await;
        // has_embeddings is compile-time — must be true in default feature set.
        assert!(
            caps.has_embeddings,
            "has_embeddings should be true when local-embed or remote-embed feature is active"
        );
    }

    // -------------------------------------------------------------------------
    // Resource registry tests (T7)
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn list_resources_includes_doc_and_summary() {
        let (_dir, server) = make_server().await;
        let rr = server.resources.read().await.clone();
        let uris: Vec<String> = rr.list().into_iter().map(|d| d.uri).collect();

        assert!(
            uris.iter().any(|u| u.starts_with("doc://")),
            "expected at least one doc:// URI, got: {uris:?}"
        );
        assert!(
            uris.contains(&"project://summary".to_string()),
            "expected project://summary URI, got: {uris:?}"
        );
    }

    #[tokio::test]
    async fn read_resource_roundtrips_project_summary() {
        let (_dir, server) = make_server().await;
        let rr = server.resources.read().await.clone();
        let bytes = rr.read("project://summary").await.unwrap();
        let text = match bytes {
            crate::mcp_resources::ResourceBytes::Text(t) => t,
            _ => panic!("expected text resource"),
        };
        let json: serde_json::Value =
            serde_json::from_str(&text).expect("project://summary must be valid JSON");
        for key in ["active_project", "index_status", "language", "lsp_ready"] {
            assert!(
                json.get(key).is_some(),
                "missing key '{}' in summary JSON",
                key
            );
        }
    }

    #[tokio::test]
    async fn read_resource_unknown_returns_not_found() {
        let (_dir, server) = make_server().await;
        let rr = server.resources.read().await.clone();
        let err = rr
            .read("doc://does-not-exist")
            .await
            .expect_err("reading unknown URI must fail");
        assert!(
            matches!(err, crate::mcp_resources::ResourceError::NotFound(_)),
            "expected NotFound, got: {err}"
        );
    }

    #[tokio::test]
    async fn get_info_advertises_resources_capability() {
        let (_dir, server) = make_server().await;
        let info = server.get_info();
        assert!(
            info.capabilities.resources.is_some(),
            "server must advertise resources capability"
        );
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
