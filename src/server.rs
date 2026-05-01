//! MCP server — bridges our `Tool` registry to rmcp's `ServerHandler`.

use std::path::PathBuf;
use std::sync::Arc;

use crate::lsp::{LspManager, LspProvider};

use anyhow::{Context, Result};
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
    create_file::CreateFile,
    edit_file::EditFile,
    glob::Glob,
    grep::Grep,
    library::{ListLibraries, RegisterLibrary},
    list_dir::ListDir,
    markdown::{EditMarkdown, ReadMarkdown},
    memory::Memory,
    progress,
    read_file::ReadFile,
    semantic::{IndexProject, IndexStatus, SemanticSearch},
    symbol::{
        FindSymbol, GotoDefinition, Hover, InsertCode, ListSymbols, References, RemoveSymbol,
        RenameSymbol, ReplaceSymbol,
    },
    Onboarding, RunCommand, Tool, ToolContext,
};
use crate::usage::UsageRecorder;

// Note: `register_library` writes libraries.json but is intentionally excluded —
// it is idempotent and write-lock overhead on registration is not warranted.
// `onboarding` writes memory but is also excluded — it is infrequent and
// memory writes are small; the `memory` tool's write actions cover the
// common case.

#[derive(Clone)]
pub struct CodeScoutServer {
    agent: Agent,
    lsp: Arc<dyn LspProvider>,
    output_buffer: Arc<crate::tools::output_buffer::OutputBuffer>,
    // Arc<dyn Tool>: heterogeneous collection of 23+ different tool types dispatched by name at runtime.
    tools: Vec<Arc<dyn Tool>>,
    /// Pre-computed at construction, wrapped in `Arc<RwLock<>>` so that
    /// `activate_project` can refresh the string mid-session without
    /// reconstructing the server. `get_info()` is sync so we read-lock;
    /// `refresh_instructions()` write-locks after each `activate_project`.
    instructions: Arc<parking_lot::RwLock<String>>,
    section_coverage: Arc<std::sync::Mutex<crate::tools::section_coverage::SectionCoverage>>,
    session_id: String,
    debug: bool,
    /// Last capabilities snapshot that was broadcast to the client via
    /// `notifications/tools/list_changed`. Used to suppress redundant broadcasts.
    last_broadcast_caps: Arc<parking_lot::Mutex<Option<crate::tools::ToolCapabilities>>>,
    /// MCP resource registry — replaceable on `activate_project` to pick up a new
    /// memory dir. Held behind `RwLock<Arc<...>>` so list/read only need a read lock
    /// while replacement takes a write lock.
    resources: Arc<tokio::sync::RwLock<Arc<crate::mcp_resources::ResourceRegistry>>>,
    /// How many times the path-disambiguation note ("paths are relative to …") has
    /// been emitted this session. Capped at `PATH_NOTE_MAX` to avoid noise while
    /// still surfacing the context early in a session.
    path_note_count: Arc<std::sync::atomic::AtomicUsize>,
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
        #[cfg_attr(not(feature = "librarian"), allow(unused_mut))]
        let mut instructions = crate::prompts::build_server_instructions(status.as_ref());
        #[cfg(feature = "librarian")]
        if librarian_enabled_at_runtime(status.as_ref().map(|s| s.path.as_str())) {
            instructions.push_str("\n\n");
            instructions.push_str(librarian_mcp::INSTRUCTIONS);
        }
        #[cfg_attr(not(feature = "librarian"), allow(unused_mut))]
        let mut tools: Vec<Arc<dyn Tool>> = vec![
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
            Arc::new(References),
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
        #[cfg(feature = "librarian")]
        if librarian_enabled_at_runtime(status.as_ref().map(|s| s.path.as_str())) {
            if let Some(lib_ctx) = crate::librarian::try_build_runtime().await {
                tools.extend(crate::librarian::adapters_for(lib_ctx));
            }
        }
        let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));
        let section_coverage = Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        ));
        let resources = Arc::new(tokio::sync::RwLock::new(Arc::new(
            build_resource_registry(&agent, Arc::clone(&lsp), &tools).await,
        )));
        Self {
            agent,
            lsp,
            output_buffer,
            tools,
            instructions: Arc::new(parking_lot::RwLock::new(instructions)),
            section_coverage,
            session_id: uuid::Uuid::new_v4().to_string(),
            debug,
            last_broadcast_caps: Arc::new(parking_lot::Mutex::new(None)),
            resources,
            path_note_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn find_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.name() == name).cloned()
    }

    fn resolve_tool(&self, name: &str) -> std::result::Result<Arc<dyn Tool>, McpError> {
        self.find_tool(name)
            .ok_or_else(|| McpError::invalid_params(format!("unknown tool: '{}'", name), None))
    }

    /// Returns true if this tool call will mutate project state.
    ///
    /// Dispatches to `Tool::is_write(input)` on the resolved tool. Unknown
    /// tools return false — they never reach dispatch (resolve_tool rejects
    /// them first), so the answer is immaterial; returning false avoids a
    /// second lookup failure.
    fn is_write_call(&self, tool_name: &str, input: &serde_json::Value) -> bool {
        self.find_tool(tool_name)
            .map(|t| t.is_write(input))
            .unwrap_or(false)
    }

    fn parse_input(arguments: Option<serde_json::Map<String, Value>>) -> Value {
        arguments
            .map(Value::Object)
            .unwrap_or(Value::Object(Default::default()))
    }

    async fn check_tool_access(&self, name: &str) -> std::result::Result<(), CallToolResult> {
        let security = self.agent.security_config().await;
        crate::util::path_security::check_tool_access(name, &security)
            .map_err(|e| CallToolResult::error(vec![Content::text(e.to_string())]))
    }

    fn build_context(
        &self,
        progress: Option<Arc<progress::ProgressReporter>>,
        peer: Option<Peer<RoleServer>>,
    ) -> ToolContext {
        ToolContext {
            agent: self.agent.clone(),
            lsp: self.lsp.clone(),
            output_buffer: self.output_buffer.clone(),
            progress,
            peer,
            section_coverage: self.section_coverage.clone(),
        }
    }

    async fn acquire_write_guard_if_writing(
        &self,
        name: &str,
        input: &Value,
    ) -> std::result::Result<
        std::result::Result<Option<crate::agent::WriteGuard>, CallToolResult>,
        McpError,
    > {
        if !self.is_write_call(name, input) {
            return Ok(Ok(None));
        }
        let (mutex, fd_lock, timeout_secs) = self
            .agent
            .with_project(|p| {
                Ok((
                    p.write_lock.clone(),
                    p.file_lock.clone(),
                    p.config.security.write_lock_timeout_secs,
                ))
            })
            .await
            .map_err(|e| McpError::internal_error(format!("write gate: {}", e), None))?;
        match crate::agent::acquire_write_guard(
            mutex,
            fd_lock,
            std::time::Duration::from_secs(timeout_secs),
        )
        .await
        {
            Ok(g) => Ok(Ok(Some(g))),
            // Route to isError: false so sibling calls survive.
            Err(rec_err) => Ok(Err(route_tool_error(rec_err.into()))),
        }
    }

    /// Race the tool call against (a) the server-level timeout and (b) the
    /// per-request cancellation token. Cancellation is the load-bearing arm:
    /// when the user presses Escape, rmcp cancels `cancel_token`, the select!
    /// arm fires, and the tool future is dropped — which kills any spawned
    /// child via `kill_on_drop`. Without this, the future runs to completion
    /// and the late response makes Claude Code close the MCP connection.
    ///
    /// `release_on_cancel` is dropped **before** parking with `pending()`.
    /// Use this to release any guards (e.g. `WriteGuard`) that must not be
    /// held while the task is parked waiting for rmcp to drop it on disconnect.
    async fn race_against_cancel<F, G>(
        tool_call_fut: F,
        cancel_token: tokio_util::sync::CancellationToken,
        timeout_secs: Option<u64>,
        tool_name: &str,
        release_on_cancel: G,
    ) -> Result<Vec<Content>, anyhow::Error>
    where
        F: std::future::Future<Output = Result<Vec<Content>, anyhow::Error>>,
        G: Send + 'static,
    {
        if let Some(secs) = timeout_secs {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    // Suppress response after cancel: Claude Code closes the MCP
                    // stdio connection if it receives ANY response for a cancelled
                    // request (confirmed 2026-04-16 by pending() experiment — see
                    // docs/issues/2026-04-16-mcp-cancel-disconnect.md).
                    //
                    // We park the task here permanently instead. tool_call_fut was
                    // dropped by select!, so the shell child is already reaped via
                    // kill_on_drop + PgidKillGuard. Only this task's stack persists
                    // until rmcp drops it when the connection closes.
                    drop(release_on_cancel);
                    std::future::pending::<Result<Vec<Content>, anyhow::Error>>().await
                }
                res = tokio::time::timeout(
                    std::time::Duration::from_secs(secs),
                    tool_call_fut,
                ) => res.unwrap_or_else(|_| {
                    Err(anyhow::anyhow!(
                        "Tool '{}' timed out after {}s. \
                         Increase tool_timeout_secs in .codescout/project.toml if needed.",
                        tool_name,
                        secs
                    ))
                }),
            }
        } else {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    // Suppress response after cancel: Claude Code closes the MCP
                    // stdio connection if it receives ANY response for a cancelled
                    // request (confirmed 2026-04-16 by pending() experiment — see
                    // docs/issues/2026-04-16-mcp-cancel-disconnect.md).
                    //
                    // We park the task here permanently instead. tool_call_fut was
                    // dropped by select!, so the shell child is already reaped via
                    // kill_on_drop + PgidKillGuard. Only this task's stack persists
                    // until rmcp drops it when the connection closes.
                    drop(release_on_cancel);
                    std::future::pending::<Result<Vec<Content>, anyhow::Error>>().await
                }
                res = tool_call_fut => res,
            }
        }
    }

    /// Strip the absolute project root from all output to reduce token usage.
    /// Agents work exclusively within the project directory; relative paths
    /// carry all necessary information. The full root (e.g. /home/user/project)
    /// is a long repeated prefix that appears in every "file" field and error
    /// message. Buffer content (@tool_xxx refs) is covered here too: it only
    /// re-enters the pipeline through run_command, which also passes through
    /// call_tool.
    async fn post_process(&self, call_result: CallToolResult, tool_name: &str) -> CallToolResult {
        let root_prefix = self
            .agent
            .project_root()
            .await
            .map(|p| format!("{}/", p.display()))
            .unwrap_or_default();

        let (mut call_result, stripped) = strip_project_root_from_result(call_result, &root_prefix);

        // "Low hint": for tools that echo raw content (file contents, shell output),
        // append a one-liner so agents can distinguish stripped absolute paths from
        // genuine relative path values in file content. Capped at PATH_NOTE_MAX per
        // session — after that the agent has seen it enough times to remember.
        const PATH_NOTE_MAX: usize = 3;
        const PATH_NOTE_TOOLS: &[&str] = &["read_file", "run_command"];
        if stripped && PATH_NOTE_TOOLS.contains(&tool_name) {
            let prev = self
                .path_note_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if prev < PATH_NOTE_MAX {
                let root = root_prefix.trim_end_matches('/');
                call_result.content.push(Content::text(format!(
                    "[codescout] paths are relative to {root}"
                )));
            }
        }
        call_result
    }

    /// Replace the resource registry after an `activate_project` call that may have
    /// changed the active memory directory.
    async fn refresh_resources(&self) {
        let new_rr = build_resource_registry(&self.agent, Arc::clone(&self.lsp), &self.tools).await;
        *self.resources.write().await = Arc::new(new_rr);
    }

    /// Refresh the pre-computed instructions string after `activate_project`.
    /// Keeps stdio-transport clients from seeing stale project state
    /// (e.g. memories written by a just-completed onboarding run).
    async fn refresh_instructions(&self) {
        let status = self.agent.project_status().await;
        #[cfg_attr(not(feature = "librarian"), allow(unused_mut))]
        let mut new_instructions = crate::prompts::build_server_instructions(status.as_ref());
        #[cfg(feature = "librarian")]
        if librarian_enabled_at_runtime(status.as_ref().map(|s| s.path.as_str())) {
            new_instructions.push_str("\n\n");
            new_instructions.push_str(librarian_mcp::INSTRUCTIONS);
        }
        *self.instructions.write() = new_instructions;
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

        // has_git_remote: read the value cached at activation time. The original
        // implementation called `git2::Repository::open(&root)` here, which ran
        // on every `list_tools` call — list_tools fires frequently and opening
        // a repo walks parent directories looking for .git.
        let has_git_remote = self
            .agent
            .with_project(|p| Ok(p.has_git_remote))
            .await
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

        let tool = self.resolve_tool(&req.name)?;

        if let Err(err) = self.check_tool_access(&req.name).await {
            return Ok(err);
        }

        let input: Value = Self::parse_input(req.arguments);

        let ctx = self.build_context(progress, peer);

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

        // Acquire the write guard if this is a mutating call. Read calls skip
        // the lock entirely. The guard is passed into race_against_cancel and
        // dropped there — either naturally when the tool future completes, or
        // explicitly before parking if the request is cancelled. This ensures
        // the cross-process write lock is released even when the task is parked
        // waiting for rmcp to drop it on connection close.
        let write_guard = match self
            .acquire_write_guard_if_writing(&req.name, &input_for_record)
            .await?
        {
            Ok(g) => g,
            Err(result) => return Ok(result),
        };

        let tool_call_fut = recorder.record_content(&req.name, &input_for_record, || {
            tool.call_content(input, &ctx)
        });

        let result = Self::race_against_cancel(
            tool_call_fut,
            cancel_token,
            timeout_secs,
            &req.name,
            write_guard,
        )
        .await;

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

        let call_result = self.post_process(call_result, &req.name).await;

        Ok(call_result)
    }
}

/// Snapshot of `/proc/self/status` memory fields, in kB.
///
/// Logged from the heartbeat task so OOM forensics has a per-instance time
/// series. All four fields default to 0 on platforms that don't expose
/// `/proc/self/status` (Windows, macOS) or on parse failure — heartbeat
/// continues regardless.
#[derive(Default, Copy, Clone)]
struct SelfMemoryKb {
    vm_size_kb: u64,
    vm_rss_kb: u64,
    vm_data_kb: u64,
    vm_peak_kb: u64,
}

fn read_self_memory_kb() -> SelfMemoryKb {
    let mut out = SelfMemoryKb::default();
    let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
        return out;
    };
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value_kb = rest
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        match key {
            "VmSize" => out.vm_size_kb = value_kb,
            "VmRSS" => out.vm_rss_kb = value_kb,
            "VmData" => out.vm_data_kb = value_kb,
            "VmPeak" => out.vm_peak_kb = value_kb,
            _ => {}
        }
    }
    out
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

/// Whether to register the embedded librarian tool surface for this session.
///
/// Layered defaults:
/// 1. `LIBRARIAN_ENABLED=0|false|off` env var disables (overrides everything).
/// 2. `LIBRARIAN_ENABLED=1|true|on` env var enables (overrides config).
/// 3. `[librarian] enabled = true|false` in `<project>/.codescout/project.toml`.
/// 4. Default: disabled (experimental — set `LIBRARIAN_ENABLED=1` to opt in).
#[cfg(feature = "librarian")]
fn librarian_enabled_at_runtime(project_path: Option<&str>) -> bool {
    if let Ok(v) = std::env::var("LIBRARIAN_ENABLED") {
        let v = v.trim().to_ascii_lowercase();
        if matches!(v.as_str(), "0" | "false" | "off" | "no") {
            return false;
        }
        if matches!(v.as_str(), "1" | "true" | "on" | "yes") {
            return true;
        }
    }
    if let Some(root) = project_path {
        let cfg = std::path::Path::new(root)
            .join(".codescout")
            .join("project.toml");
        if let Ok(text) = std::fs::read_to_string(&cfg) {
            if let Ok(parsed) = toml::from_str::<toml::Value>(&text) {
                if let Some(v) = parsed
                    .get("librarian")
                    .and_then(|t| t.get("enabled"))
                    .and_then(|v| v.as_bool())
                {
                    return v;
                }
            }
        }
    }
    false
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
        .with_instructions(self.instructions.read().clone())
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
        let is_activate = req.name == crate::tools::config::ActivateProject::NAME;
        let progress = Some(progress::ProgressReporter::new(
            req_ctx.peer.clone(),
            req_ctx.id.clone(),
        ));
        let peer = Some(req_ctx.peer.clone());
        // `req_ctx.ct` is rmcp's per-request CancellationToken. It is cancelled
        // when the client sends a CancelledNotification (Escape in Claude Code).
        // Hand it to call_tool_inner so the tool future can be aborted instead
        // of running to completion and triggering a connection close.
        let result = self
            .call_tool_inner(req, progress, peer, req_ctx.ct.clone())
            .await?;

        // After a successful activate_project, check whether the capability set has
        // changed. If it has, emit notifications/tools/list_changed so the client
        // can refresh its tool list without a full reconnect.
        if is_activate {
            let new_caps = self.current_capabilities().await;
            let caps_changed = {
                let mut last = self.last_broadcast_caps.lock();
                let changed = last.as_ref() != Some(&new_caps);
                if changed {
                    *last = Some(new_caps);
                }
                changed
            };
            if caps_changed {
                let _ = req_ctx.peer.notify_tool_list_changed().await;
            }

            // Rebuild the resource registry to pick up the new memory dir,
            // and refresh instructions so stdio clients see current project state.
            self.refresh_resources().await;
            self.refresh_instructions().await;
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
///
/// `tools` is passed so that the tool-guide resource is always current; each
/// `refresh_resources` call simply re-registers with the same tool slice.
async fn build_resource_registry(
    agent: &Agent,
    lsp: Arc<dyn LspProvider>,
    tools: &[Arc<dyn Tool>],
) -> crate::mcp_resources::ResourceRegistry {
    use crate::mcp_resources::{
        doc::{DocProvider, DocSource},
        memory::MemoryProvider,
        project_summary::{AgentSummarySource, ProjectSummaryProvider},
        tool_guide::ToolGuideProvider,
        tool_usage::{AgentUsageSource, ToolUsageProvider},
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
        AgentSummarySource::new(agent.clone(), lsp),
    )));

    // Tool guide — always registered; renders long_docs() for each registered tool.
    let _ = rr.try_register(Box::new(ToolGuideProvider::new(tools.to_vec())));

    // Tool usage doctor — reports per-tool call counts and prune candidates.
    // Always registered; returns empty snapshot when usage.db is absent.
    let _ = rr.try_register(Box::new(ToolUsageProvider::new(AgentUsageSource::new(
        agent.clone(),
        tools.to_vec(),
    ))));

    rr
}

/// Route a tool `Err(e)` to the appropriate `CallToolResult`.
///
/// - [`RecoverableError`] → `isError: false` with a JSON body containing
///   `"error"`, optional guidance under its variant-named key
///   (`hint` / `warning` / `must_follow`), and any `extra` fields spliced
///   in at the top level.  Sibling parallel calls are **not** aborted.
/// - Any other error → `isError: true` (fatal; something truly broke).
///   The full `anyhow` context chain is logged server-side via
///   `tracing::error!`; only the outermost message goes over the wire. This
///   keeps `with_context` chains (which often include absolute paths) out
///   of MCP responses for the HTTP transport, where error oracles can leak
///   filesystem layout to an authenticated-but-untrusted client.
///
/// [`RecoverableError`]: crate::tools::RecoverableError
fn route_tool_error(e: anyhow::Error) -> CallToolResult {
    if let Some(rec) = e.downcast_ref::<crate::tools::RecoverableError>() {
        let mut body = serde_json::json!({ "ok": false, "error": rec.message });
        if let Some(g) = &rec.guidance {
            body[g.field_name()] = serde_json::json!(g.text());
        }
        if let Some(obj) = body.as_object_mut() {
            for (k, v) in rec.extra.iter() {
                obj.insert(k.clone(), v.clone());
            }
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
        // Log the full context chain server-side (`{:#}` walks `.source()`
        // chain). Only the outermost message crosses the wire.
        tracing::error!(error = format!("{e:#}"), "tool error");
        CallToolResult::error(vec![Content::text(e.to_string())])
    }
}

/// Entry point: start the MCP server with the chosen transport.
/// Generate a bearer token for HTTP transport authentication.
///
/// # Deprecated
///
/// Uses timestamp + PID, which is NOT cryptographically secure. Kept only
/// for external callers that may reference this symbol. New code should call
/// `os_random_auth_token()` (private) or pass `--auth-token` explicitly.
#[deprecated(
    since = "0.9.0",
    note = "Not cryptographically secure. Use os_random_auth_token() internally or pass --auth-token."
)]
pub fn generate_auth_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u64;
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
/// On `WouldBlock`, this wrapper arms a short timer and returns `Poll::Pending`
/// with the waker registered via tokio's timer reactor. This avoids both failure
/// modes: "hang forever" (no waker registered) and "CPU spin" (`wake_by_ref()`
/// immediately reschedules the task, causing a tight busy-loop when EAGAIN is
/// persistent).
struct ResilientStdin {
    inner: tokio::io::Stdin,
    /// Short sleep armed on `WouldBlock` to prevent CPU spinning.
    backoff: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
}

impl ResilientStdin {
    fn new(stdin: tokio::io::Stdin) -> Self {
        Self {
            inner: stdin,
            backoff: None,
        }
    }
}

impl tokio::io::AsyncRead for ResilientStdin {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        use std::future::Future;
        let this = self.get_mut();

        // Drain any active backoff sleep before attempting a read.
        // The sleep registers the waker via tokio's timer reactor, so we are
        // woken after the delay rather than spinning immediately.
        if let Some(ref mut sleep) = this.backoff {
            if sleep.as_mut().poll(cx).is_pending() {
                return std::task::Poll::Pending;
            }
            this.backoff = None;
        }

        match std::pin::Pin::new(&mut this.inner).poll_read(cx, buf) {
            std::task::Poll::Ready(Err(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // EAGAIN from stdin: Node.js briefly set the pipe O_NONBLOCK.
                // Returning Poll::Pending without a registered waker would hang
                // the task forever. Calling wake_by_ref() immediately would spin
                // at scheduler rate (the original BUG-047 flaw). Instead, arm a
                // 1ms sleep — polling it registers the waker via the timer
                // reactor so the task resumes after the delay, not immediately.
                tracing::trace!("stdin EAGAIN — backing off 1ms before retry");
                let mut sleep = Box::pin(tokio::time::sleep(std::time::Duration::from_millis(1)));
                let _ = sleep.as_mut().poll(cx);
                this.backoff = Some(sleep);
                std::task::Poll::Pending
            }
            other => other,
        }
    }
}

/// Generate a random bearer token for HTTP transport auth.
///
/// Uses the OS CSPRNG exclusively. Aborts startup on failure rather than
/// falling back to a weak token — a predictable bearer on a network-reachable
/// endpoint is equivalent to no auth.
fn os_random_auth_token() -> Result<String> {
    let mut buf = [0u8; 32];
    // File::open + read_exact, not std::fs::read — device nodes have no EOF.
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .map_err(|e| anyhow::anyhow!("Failed to read /dev/urandom for auth token: {e}"))?;
    Ok(hex::encode(buf))
}

/// Constant-time bearer string comparison. Prevents timing oracles that let
/// an attacker enumerate valid token bytes by measuring response latency.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
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
    let project = match project.or_else(|| std::env::current_dir().ok()) {
        Some(p) => Some(std::fs::canonicalize(&p).with_context(|| {
            format!(
                "failed to canonicalize project path {} — check it exists and is readable",
                p.display()
            )
        })?),
        None => None,
    };
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
                let mem = read_self_memory_kb();
                tracing::info!(
                    instance = %instance_tag_hb,
                    uptime_secs,
                    active_projects,
                    ?lsp_servers,
                    vm_size_kb = mem.vm_size_kb,
                    vm_rss_kb = mem.vm_rss_kb,
                    vm_data_kb = mem.vm_data_kb,
                    vm_peak_kb = mem.vm_peak_kb,
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
            let token = match auth_token {
                Some(t) => t,
                None => {
                    let t = os_random_auth_token()?;
                    eprintln!("Generated auth token: {t}");
                    t
                }
            };

            let router =
                axum::Router::new()
                    .nest_service("/mcp", service)
                    .layer(axum::middleware::from_fn(
                        move |req: axum::extract::Request, next: axum::middleware::Next| {
                            let expected = format!("Bearer {token}");
                            async move {
                                let ok = req
                                    .headers()
                                    .get("authorization")
                                    .map(|v| ct_eq(v.as_bytes(), expected.as_bytes()))
                                    .unwrap_or(false);
                                if ok {
                                    next.run(req).await
                                } else {
                                    axum::http::StatusCode::UNAUTHORIZED.into_response()
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
/// Returns `(result, stripped)` where `stripped` is true when at least one
/// content block was modified. The caller decides whether to append a
/// disambiguation note (see `PATH_NOTE_MAX` / `PATH_NOTE_TOOLS`).
///
/// **Stripping heuristic**: only occurrences preceded by a non-path character
/// (e.g. `"`, ` `, `:`, `\n`) or at the very start of the text are replaced.
/// This avoids mangling embedded path literals inside file content where the
/// project root happens to appear mid-string (e.g. inside a test assertion).
///
/// Note: values like `"project_root": "/abs/path"` in `activate_project` /
/// `get_config` responses are intentionally not stripped — they use a bare
/// absolute path without a trailing slash, so they do not match `root_prefix`
/// and pass through unchanged.
///
/// Buffer content (`@tool_xxx` refs) is covered automatically: it only
/// re-enters the pipeline through `run_command`, which also passes through
/// `call_tool` and gets stripped there.
fn strip_project_root_from_result(
    mut result: CallToolResult,
    root_prefix: &str,
) -> (CallToolResult, bool) {
    if root_prefix.is_empty() {
        return (result, false);
    }
    debug_assert!(
        root_prefix.ends_with('/'),
        "root_prefix must end with '/' to avoid stripping partial path components"
    );
    let mut stripped = false;
    for block in &mut result.content {
        if let RawContent::Text(ref mut t) = block.raw {
            let new_text = strip_prefix_from_text(&t.text, root_prefix);
            if new_text != t.text {
                stripped = true;
                t.text = new_text;
            }
        }
    }
    (result, stripped)
}

/// Replace occurrences of `prefix` in `text` only when they appear in a
/// path-value context — i.e. preceded by a non-path character or at the start
/// of the string. This avoids stripping the prefix when it appears embedded
/// inside longer strings such as code literals or comments.
///
/// A "path character" here means anything that could legitimately precede a
/// path component: `/`, alphanumerics, `-`, `_`, `.`. Everything else (quotes,
/// spaces, colons, brackets, newlines…) signals a value boundary.
fn strip_prefix_from_text(text: &str, prefix: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last = 0;
    for (pos, _) in text.match_indices(prefix) {
        let is_value_boundary = pos == 0
            || {
                // SAFETY: pos is a byte offset from match_indices, so text[..pos] is valid UTF-8.
                // next_back() returns the last char before the match.
                let prev = text[..pos].chars().next_back();
                !matches!(prev, Some(c) if c == '/' || c == '.' || c == '-' || c == '_' || c.is_ascii_alphanumeric())
            };
        if is_value_boundary {
            result.push_str(&text[last..pos]);
            last = pos + prefix.len();
        }
    }
    result.push_str(&text[last..]);
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
            "references",
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
        let core_count = server
            .tools
            .iter()
            .filter(|t| !is_librarian_tool(t.name()))
            .count();
        assert_eq!(
            core_count,
            expected_tools.len(),
            "core tool count mismatch: expected {}, got {}\nregistered: {:?}",
            expected_tools.len(),
            core_count,
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

    fn is_librarian_tool(name: &str) -> bool {
        name.starts_with("artifact_")
            || name.starts_with("librarian_")
            || name == "workspace_state_at"
    }

    #[tokio::test]
    async fn tool_descriptions_stay_under_budget() {
        let (_dir, server) = make_server().await;
        for t in &server.tools {
            if is_librarian_tool(t.name()) {
                continue;
            }
            let d = t.description();
            assert!(
                d.len() <= 300,
                "tool `{}` description is {} chars (cap 300): {:?}",
                t.name(),
                d.len(),
                d
            );
        }
    }

    /// Guard against prompt-surface drift: every backticked snake_case identifier
    /// in `server_instructions.md`, `onboarding_prompt.md`, and the generated
    /// `build_system_prompt_draft` output must resolve to a real registered tool
    /// name or appear in the known-non-tool allowlist below. When you rename or
    /// remove a tool, the compiler won't catch stale prompt mentions — this test
    /// does.
    ///
    /// Scope: **snake_case tokens only** (regex `[a-z][a-z_0-9]{2,}`). This
    /// deliberately skips PascalCase identifiers — host-harness tool names
    /// (`EnterWorktree`, `TaskCreate`), Rust type names, and tree-sitter node
    /// kinds use PascalCase and would explode the allowlist with non-codescout
    /// tokens. Codescout's own tool names are all snake_case, so this coverage
    /// matches the drift surface we care about. If a codescout tool is ever
    /// added in PascalCase (none today), widen the regex and grow the allowlist.
    #[tokio::test]
    async fn prompt_surfaces_reference_only_real_tools() {
        use std::collections::HashSet;

        let (_dir, server) = make_server().await;
        let real_tools: HashSet<&str> = server.tools.iter().map(|t| t.name()).collect();

        // Tokens that appear backticked in the surfaces but are not tool names.
        // Grow this list as prompts evolve; shrink it when a token disappears
        // from the surfaces. Keep entries sorted.
        let allowlist: HashSet<&str> = [
            "acknowledge_risk",
            "architecture",
            "by_file",
            "class",
            "code",
            "conventions",
            "cwd",
            "detail_level",
            "domain_glossary",
            "end_line",
            "features_md",
            "file_id",
            "files",
            "fn",
            "gotchas",
            "hardware",
            "include_body",
            "json_path",
            "kind",
            "language_patterns",
            "limit",
            "model",
            "model_options",
            "name",
            "name_path",
            "new_body",
            "new_string",
            "next",
            "offset",
            "old_string",
            "output_id",
            "path",
            "pattern",
            "project_overview",
            "protected_memories",
            "query",
            "read_only",
            "replace_all",
            "run_in_background",
            "scope",
            "sed",
            "start_line",
            "struct",
            "symbol",
            "system_prompt",
            "timeout_secs",
            "toml_key",
            "untracked",
            "url",
        ]
        .into_iter()
        .collect();

        let draft = crate::prompts::builders::build_system_prompt_draft(&[], &[], None, None, &[]);
        let surfaces: &[(&str, &str)] = &[
            (
                "server_instructions.md",
                include_str!("prompts/server_instructions.md"),
            ),
            (
                "onboarding_prompt.md",
                include_str!("prompts/onboarding_prompt.md"),
            ),
            ("build_system_prompt_draft", draft.as_str()),
        ];

        let re = regex::Regex::new(r"`([a-z][a-z_0-9]{2,})`").unwrap();
        let mut drift = Vec::<String>::new();
        for (surface, body) in surfaces {
            for cap in re.captures_iter(body) {
                let ident = cap.get(1).unwrap().as_str();
                if real_tools.contains(ident) || allowlist.contains(ident) {
                    continue;
                }
                drift.push(format!(
                    "{surface}: `{ident}` looks like a tool name but is not \
                     registered — rename the reference to a real tool, or add \
                     it to the allowlist in this test if it's a non-tool token"
                ));
            }
        }

        assert!(
            drift.is_empty(),
            "prompt-surface drift detected:\n  {}",
            drift.join("\n  ")
        );
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
    #[allow(deprecated)]
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
    #[allow(deprecated)]
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
    fn recoverable_error_body_serializes_warning_under_warning_key() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::with_warning(
            "too many results",
            "narrow with path=",
        ));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["warning"], "narrow with path=");
        assert!(body.get("hint").is_none());
        assert!(body.get("must_follow").is_none());
    }

    #[test]
    fn recoverable_error_body_serializes_must_follow_under_must_follow_key() {
        let err = anyhow::Error::new(crate::tools::RecoverableError::with_must_follow(
            "heading too large",
            "IRON LAW #6: use @file_xxx",
        ));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["must_follow"], "IRON LAW #6: use @file_xxx");
        assert!(body.get("hint").is_none());
        assert!(body.get("warning").is_none());
    }

    #[test]
    fn recoverable_error_body_splices_extra_fields_at_top_level() {
        let err_struct =
            crate::tools::RecoverableError::with_must_follow("heading too large", "IRON LAW #6")
                .with_extra("file_id", serde_json::json!("@file_abc"))
                .with_extra(
                    "section_map",
                    serde_json::json!([{"level": 2, "text": "## X", "line": 10}]),
                );
        let err: anyhow::Error = err_struct.into();
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["file_id"], "@file_abc");
        assert_eq!(body["section_map"][0]["line"], 10);
        assert_eq!(body["ok"], serde_json::Value::Bool(false));
        assert_eq!(body["error"], "heading too large");
        assert_eq!(body["must_follow"], "IRON LAW #6");
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
        // run_command" bug.
        //
        // When the per-request CancellationToken fires, the tool future is
        // dropped (killing the child via kill_on_drop + PgidKillGuard) and
        // call_tool_inner parks on pending() — no response is ever sent.
        // Sending a response for a cancelled request causes Claude Code to
        // close the MCP stdio connection (confirmed 2026-04-16).
        //
        // This test verifies the child-reaping half: run `sleep 5 && touch
        // <marker>` with timeout_secs=30, cancel after 200ms, confirm the
        // marker is never created (sleep was killed before reaching touch).
        // We abort the task after checking since it parks permanently.
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
        ct.cancel();

        // Give kill_on_drop + PgidKillGuard time to reap the child, then
        // abort the handler task (it parks on pending() by design — no
        // response is sent for cancelled requests).
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        handle.abort();

        // Wait past the original sleep window. If the child survived the
        // cancel, touch would have run and the marker would exist by now.
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
        let (stripped, did_strip) = strip_project_root_from_result(result, prefix);
        assert!(did_strip, "should report that stripping occurred");
        assert_eq!(extract_text(&stripped), r#"{"file":"src/foo.rs","line":1}"#);
        // Note is NOT appended by this function — call_tool_inner does that
        // with the session-capped logic. Verify only 1 content block here.
        assert_eq!(stripped.content.len(), 1);
    }

    #[test]
    fn strip_project_root_no_op_when_prefix_empty() {
        let result = CallToolResult::success(vec![Content::text("some output")]);
        let (stripped, did_strip) = strip_project_root_from_result(result, "");
        assert!(!did_strip);
        assert_eq!(extract_text(&stripped), "some output");
    }

    #[test]
    fn strip_project_root_no_op_when_prefix_absent() {
        let prefix = "/home/user/myproject/";
        let result = CallToolResult::success(vec![Content::text("no paths here")]);
        let (stripped, did_strip) = strip_project_root_from_result(result, prefix);
        assert!(!did_strip);
        assert_eq!(extract_text(&stripped), "no paths here");
        assert_eq!(stripped.content.len(), 1);
    }

    #[test]
    fn strip_prefix_only_at_value_boundary() {
        // Prefix preceded by a quote (JSON string value) — should strip.
        let prefix = "/home/user/proj/";
        assert_eq!(
            strip_prefix_from_text("\"/home/user/proj/src/lib.rs\"", prefix),
            "\"src/lib.rs\""
        );
        // Prefix at start of string — should strip.
        assert_eq!(
            strip_prefix_from_text("/home/user/proj/src/lib.rs", prefix),
            "src/lib.rs"
        );
        // Prefix after a space (error message) — should strip.
        assert_eq!(
            strip_prefix_from_text("could not open /home/user/proj/foo.rs", prefix),
            "could not open foo.rs"
        );
        // Prefix after newline — should strip.
        assert_eq!(
            strip_prefix_from_text("files:\n/home/user/proj/a.rs\n/home/user/proj/b.rs", prefix),
            "files:\na.rs\nb.rs"
        );
    }

    #[test]
    fn strip_prefix_not_inside_longer_path() {
        // Prefix embedded after another path component — should NOT strip.
        // e.g. a symlink or nested repo that happens to share the suffix.
        let prefix = "/home/user/proj/";
        let text = "/other/home/user/proj/src/lib.rs";
        assert_eq!(strip_prefix_from_text(text, prefix), text);
    }

    #[test]
    fn strip_prefix_not_inside_code_string_preceded_by_path_char() {
        // Prefix after an alphanumeric character — not a boundary, should NOT strip.
        let prefix = "/home/user/proj/";
        let text = "foo/home/user/proj/bar";
        assert_eq!(strip_prefix_from_text(text, prefix), text);
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
        // LSP tools (hover, goto_definition, references, rename_symbol) must be hidden.
        if !caps.has_lsp {
            let visible: Vec<&str> = server
                .tools
                .iter()
                .filter(|t| t.availability(&caps).is_available(&caps))
                .map(|t| t.name())
                .collect();
            for lsp_tool in &["hover", "goto_definition", "references", "rename_symbol"] {
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

        for lsp_tool in &["hover", "goto_definition", "references", "rename_symbol"] {
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

    #[tokio::test]
    async fn is_write_call_classifies_plain_writes() {
        use serde_json::json;
        let (_dir, server) = make_server().await;
        assert!(server.is_write_call("edit_file", &json!({})));
        assert!(server.is_write_call("create_file", &json!({})));
        assert!(server.is_write_call("replace_symbol", &json!({})));
        assert!(server.is_write_call("insert_code", &json!({})));
        assert!(server.is_write_call("remove_symbol", &json!({})));
        assert!(server.is_write_call("rename_symbol", &json!({})));
        assert!(server.is_write_call("edit_markdown", &json!({})));
        assert!(server.is_write_call("index_project", &json!({})));
        assert!(server.is_write_call("onboarding", &json!({})));
        assert!(server.is_write_call("register_library", &json!({})));
        assert!(!server.is_write_call("read_file", &json!({})));
        assert!(!server.is_write_call("find_symbol", &json!({})));
    }

    #[tokio::test]
    async fn is_write_call_memory_depends_on_action() {
        use serde_json::json;
        let (_dir, server) = make_server().await;
        assert!(server.is_write_call("memory", &json!({"action": "write"})));
        assert!(server.is_write_call("memory", &json!({"action": "remember"})));
        assert!(server.is_write_call("memory", &json!({"action": "forget"})));
        assert!(server.is_write_call("memory", &json!({"action": "delete"})));
        assert!(server.is_write_call("memory", &json!({"action": "refresh_anchors"})));
        assert!(!server.is_write_call("memory", &json!({"action": "read"})));
        assert!(!server.is_write_call("memory", &json!({"action": "list"})));
        assert!(!server.is_write_call("memory", &json!({"action": "recall"})));
        assert!(!server.is_write_call("memory", &json!({})));
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
///
/// Mirrors the production `ResilientStdin` backoff pattern (BUG-047): on
/// EAGAIN, arm a 1ms sleep, poll it to register the waker via the timer
/// reactor, return Pending. Production cannot be tested directly because
/// `ResilientStdin` is hard-coded to `tokio::io::Stdin`; this generic
/// version mirrors the state machine so regressions in the pattern are
/// caught by test.
#[tokio::test]
async fn resilient_stdin_absorbs_would_block() {
    use std::future::Future;
    use tokio::io::AsyncReadExt;

    struct ResilientReader<R> {
        inner: R,
        backoff: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
    }
    impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for ResilientReader<R> {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let this = self.get_mut();

            if let Some(ref mut sleep) = this.backoff {
                if sleep.as_mut().poll(cx).is_pending() {
                    return std::task::Poll::Pending;
                }
                this.backoff = None;
            }

            match std::pin::Pin::new(&mut this.inner).poll_read(cx, buf) {
                std::task::Poll::Ready(Err(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    let mut sleep =
                        Box::pin(tokio::time::sleep(std::time::Duration::from_millis(1)));
                    let _ = sleep.as_mut().poll(cx);
                    this.backoff = Some(sleep);
                    std::task::Poll::Pending
                }
                other => other,
            }
        }
    }

    let mock = WouldBlockThenData {
        returned_eagain: false,
    };
    let mut reader = ResilientReader {
        inner: mock,
        backoff: None,
    };
    let mut buf = [0u8; 16];
    // Would surface WouldBlock as an error without the wrapper.
    // With the backoff pattern, the first EAGAIN arms a sleep, the timer
    // reactor fires, the task resumes, and the second poll returns data.
    let n = reader.read(&mut buf).await.expect("should not error");
    assert_eq!(&buf[..n], b"hello");
}
