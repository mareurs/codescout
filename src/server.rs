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
        ServerCapabilities, ServerInfo, Tool as McpTool,
    },
    service::RequestContext,
    ErrorData as McpError, Peer, RoleServer, ServerHandler, ServiceExt,
};
use serde_json::Value;

use crate::agent::Agent;
use crate::tools::{
    approve_write::ApproveWrite,
    config::{Workspace, PROJECT_SCOPED},
    create_file::CreateFile,
    edit_file::EditFile,
    grep::Grep,
    library::Library,
    markdown::{EditMarkdown, ReadMarkdown},
    memory::Memory,
    progress,
    read_file::ReadFile,
    semantic::{Index, SemanticSearch},
    symbol::{CallGraph, EditCode, References, SymbolAt, Symbols},
    tree::Tree,
    Onboarding, RunCommand, Tool, ToolContext,
};
use crate::usage::UsageRecorder;
use crate::util::fs::to_forward_slash;

// Note: `library` (action='register') writes libraries.json but is intentionally excluded —
// it is idempotent and write-lock overhead on registration is not warranted.
// `onboarding` writes memory but is also excluded — it is infrequent and
// memory writes are small; the `memory` tool's write actions cover the
// common case.

/// Everything `CodeScoutServer::from_parts` would otherwise read from the process
/// environment, captured as data so tests can inject it.
///
/// Tests must never call `std::env::set_var`: mutating `environ` while other test
/// threads call `getenv` is UB (glibc may `realloc` it under a concurrent reader), and
/// the reader set is effectively the whole suite — every `Agent::new` reads `HOME`.
/// See `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`.
#[derive(Debug, Clone, Default)]
pub struct ServerEnv {
    /// `CODESCOUT_PROBE` — registers the oversized-description probe tool.
    pub probe: bool,
    /// `CLAUDE_CODE_SESSION_ID` — correlation id for `usage.db`. NOT the ledger
    /// key: see `session_id_explicit` / `harness_session_ids`, which resolve the
    /// ledger's identity under different collision requirements.
    pub cc_session_id: Option<String>,
    /// `CODESCOUT_SESSION_ID` — rank 1 of the ledger key chain. Trusted when set;
    /// documented as unique-per-conversation, since a value pinned in MCP config
    /// is constant across every conversation in that project.
    pub session_id_explicit: Option<String>,
    /// `(name, value)` for each of `session_key::HARNESS_SESSION_VARS` that is
    /// set, in probe order. Captured as data so tests inject without `set_var`.
    pub harness_session_ids: Vec<(&'static str, String)>,
    /// Overrides the per-user guide-hint ledger directory. `None` ⇒ derive it from
    /// `per_user_state_dir()`. Tests set this to a tempdir so they never touch — or
    /// depend on — the developer's real state directory.
    pub guide_hints_dir: Option<PathBuf>,
    /// `CODESCOUT_GUIDE_TTL_SECS` — anonymous-tier idle window. `None` ⇒ the
    /// default; `Some(0)` ⇒ no expiry at all, an explicit opt-out.
    pub guide_idle_ttl: Option<std::time::Duration>,
    /// Overrides the per-user rendezvous directory — pid-keyed slots a companion
    /// hook stamps with a fresh conversation id. `None` ⇒ derive it from
    /// `per_user_state_dir()`. Tests set this to a tempdir, same rationale as
    /// `guide_hints_dir`: no test may read, write, or garbage-collect the
    /// developer's real state directory.
    pub servers_dir: Option<PathBuf>,
    /// `CODESCOUT_PEER_ENABLED` — raw value, layered against `[peer] enabled` in
    /// project.toml by `peer_enabled_at_runtime`. `None` ⇒ not set in the
    /// environment; captured raw (not pre-parsed to bool) so the layering function
    /// stays a pure, independently-testable unit — see `parse_rerank_opt_in` for
    /// why an inline env read is avoided here too.
    #[cfg(unix)]
    pub peer_enabled: Option<String>,
    /// Inputs for the librarian runtime (workspace/db/embed/cwd).
    #[cfg(feature = "librarian")]
    pub librarian: crate::librarian::LibrarianEnv,
}

/// Ceiling for an operator-supplied `CODESCOUT_GUIDE_TTL_SECS`: 100 years, far
/// past any plausible idle window and comfortably inside the range chrono can
/// subtract from `Utc::now()` without overflowing.
///
/// `std::time::Duration::from_std` (the guard `expire_idle` already applies,
/// `src/tools/guide_ledger.rs`) only rejects values adjacent to `u64::MAX` — it
/// converts happily for anything chrono's own `TimeDelta` cannot represent
/// below that. `chrono::DateTime::sub` is `checked_sub_signed(rhs).expect(...)`,
/// which panics once the subtraction overflows chrono's range, so an
/// unclamped huge value reaches a live panic band `from_std` does not cover —
/// on the anonymous tier's per-request `tick()` path, not at startup. Clamping
/// at the parse site (rather than in `expire_idle`, which this task does not
/// own) means a malicious or fat-fingered env var degrades to "TTL effectively
/// never fires" instead of unwinding every guide-eligible call.
const MAX_GUIDE_TTL_SECS: u64 = 60 * 60 * 24 * 365 * 100;

/// Parse `CODESCOUT_GUIDE_TTL_SECS`, clamped to [`MAX_GUIDE_TTL_SECS`]. `None`
/// on anything unparseable — a typo'd env var falls back to the caller's
/// default rather than erroring.
fn parse_guide_idle_ttl(raw: &str) -> Option<std::time::Duration> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .map(|secs| std::time::Duration::from_secs(secs.min(MAX_GUIDE_TTL_SECS)))
}

impl ServerEnv {
    /// Read the real process environment. The production entry point.
    pub fn from_env() -> Self {
        Self {
            probe: std::env::var("CODESCOUT_PROBE")
                .ok()
                .filter(|v| !v.is_empty() && v != "0")
                .is_some(),
            cc_session_id: std::env::var("CLAUDE_CODE_SESSION_ID")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            session_id_explicit: std::env::var("CODESCOUT_SESSION_ID").ok(),
            harness_session_ids: crate::tools::session_key::HARNESS_SESSION_VARS
                .iter()
                .filter_map(|name| std::env::var(name).ok().map(|v| (*name, v)))
                .collect(),
            guide_hints_dir: None,
            guide_idle_ttl: std::env::var("CODESCOUT_GUIDE_TTL_SECS")
                .ok()
                .and_then(|v| parse_guide_idle_ttl(&v)),
            servers_dir: None,
            #[cfg(unix)]
            peer_enabled: std::env::var("CODESCOUT_PEER_ENABLED").ok(),
            #[cfg(feature = "librarian")]
            librarian: crate::librarian::LibrarianEnv::from_env(),
        }
    }
}

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
    /// Session-scoped set of guide topics already hinted to the model.
    /// `workspace(action="activate")` touches this conditionally, not always:
    /// a genuine project switch re-arms just the project-scoped topic when a
    /// companion rendezvous is active, falls back to clearing the whole set
    /// when it isn't (a `/clear` would otherwise be invisible to the server),
    /// and a same-project re-activation leaves it alone entirely. See
    /// `ActivateProject::call` (`PROJECT_SCOPED`, `rendezvous_active`). The
    /// ledger can also arrive non-empty straight from construction — a
    /// reconnect within one conversation — in which case
    /// `CodeScoutServer::from_parts_with_env` re-arms the project-scoped
    /// topic itself, before any `activate` runs.
    guide_hints_emitted: Arc<parking_lot::Mutex<crate::tools::guide_ledger::GuideLedger>>,
    /// This MCP server process's own id — a fresh uuid per construction.
    session_id: String,
    /// The Claude Code session's id, resolved ONCE here (env var, then the
    /// shared file, then a uuid) and handed to every consumer.
    ///
    /// Resolved once on purpose. `src/usage/mod.rs` used to re-derive this by
    /// reading `.codescout/cc_session_id` directly, which is per-PROJECT: with
    /// two Claude Code sessions open on one repo, both attributed their calls to
    /// whichever id the file held last, so telemetry could not tell them apart
    /// — exactly the case the env var exists to disambiguate.
    /// docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md
    cc_session_id: String,
    /// Resolved conversation identity for the guide ledger. Distinct from
    /// `cc_session_id`, which is usage-correlation only.
    ///
    /// Unread for now: Task 1 (this) only resolves and stores it. Tasks 2, 4
    /// and 5 of the guide-ledger-phase-b-identity plan branch on it (the
    /// Anonymous-tier ledger, idle re-arm, and eviction). `expect` (not
    /// `allow`) so `unfulfilled_lint_expectations` fires the moment a later
    /// task adds the first read, instead of riding silently forever.
    #[expect(
        dead_code,
        reason = "read by tasks 2/4/5 of guide-ledger-phase-b-identity"
    )]
    session_key: crate::tools::session_key::SessionKey,
    /// Pid-keyed rendezvous slot published at construction so a companion hook
    /// can stamp a fresh conversation id into an already-running server.
    ///
    /// Behind a `Mutex` because [`Rendezvous::poll`] memoizes the last mtime it
    /// parsed at, and the request path only ever holds `&self`.
    rendezvous: Arc<parking_lot::Mutex<crate::tools::rendezvous::Rendezvous>>,
    debug: bool,
    /// Last capabilities snapshot that was broadcast to the client via
    /// `notifications/tools/list_changed`. Used to suppress redundant broadcasts.
    last_broadcast_caps: Arc<parking_lot::Mutex<Option<crate::tools::ToolCapabilities>>>,
    /// MCP resource registry — replaceable on `activate_project` to pick up a new
    /// memory dir. Held behind `RwLock<Arc<...>>` so list/read only need a read lock
    /// while replacement takes a write lock.
    resources: Arc<tokio::sync::RwLock<Arc<crate::mcp_resources::ResourceRegistry>>>,
    /// Tracks whether the `[codescout] paths are relative to <root>` annotation
    /// has been emitted since the last activation. Replaces the per-session cap
    /// (`_path_note_count`) that the U-23 fix removed.
    ///
    /// Gate logic (`post_process`): emit only on the FIRST eligible response
    /// (tool is not `run_command` and a project root resolves) since (a)
    /// server start or (b) a successful `activate_project`. The activation
    /// reset is wired into `call_tool_inner`, immediately before THAT SAME
    /// call's own `post_process` invocation — not into the `is_activate`
    /// branch of `call_tool` (which runs `refresh_resources` /
    /// `refresh_instructions` after `call_tool_inner` returns, i.e. after
    /// this same activate response already went through `post_process`).
    /// Resetting there instead of here doubled the banner: the activate
    /// response would consume the gate, then that later reset would re-arm
    /// it in the same breath, so the very next ordinary response fired the
    /// banner again.
    ///
    /// Why a per-activation gate, not the U-23 "every-call" cadence: the
    /// cold-reader signal U-23 protected (later tools rewriting paths look
    /// like raw catalog data) is now carried by the **Active project** +
    /// **Worktree** lines in `build_server_instructions`, which compaction
    /// preserves as system-prompt content. The per-response annotation
    /// becomes redundant after the first eligible call. See U-25 in
    /// `docs/trackers/codescout-usage-frictions.md` and the bug file at
    /// `docs/issues/archive/2026-05-28-path-annotation-spam.md`.
    ///
    /// **That redundancy holds for the ROOT and not for the CONVENTION**, which is why
    /// the reset also fires on compaction. `- **Active project:** <name> at <path>` tells
    /// a reader where the project is; it never says that allowlisted path fields in
    /// responses are rendered relative to it. This banner is the only surface that says so
    /// in push form, and `/compact` discards it with the rest of the conversation.
    /// `get_guide("progressive-disclosure")` § Path-relative annotation states the
    /// convention in pull form and re-arms on compaction too, but only for an agent that
    /// re-triggers the topic. See
    /// `docs/issues/archive/2026-08-21-path-relative-banner-not-rearmed-after-compaction.md`.
    path_note_emitted_since_activation: Arc<std::sync::atomic::AtomicBool>,
    /// Tracks whether the `## Project Status (details)` block has been emitted since the
    /// last activation **or compaction**.
    ///
    /// Same novelty-gate shape as `path_note_emitted_since_activation`, and a SEPARATE
    /// flag because the two are CONSUMED against different facts — but since 2026-08-26
    /// the same reset, in one branch. This comment previously called the wider reset
    /// deliberate, on the grounds that the path note goes redundant once the persistent
    /// surface names the root. That holds for the root and not for the convention, and
    /// the two gates quietly drifting apart is what cost the banner every post-compaction
    /// session. Reset them together; consume them separately. This block is not redundant
    /// with anything either: it holds the
    /// `Substitutable` segments — languages, memories, index state, workspace topology,
    /// Kotlin known issues — which used to live in `server_instructions` and moved here
    /// because that channel is 2048 characters and they were the first thing dropped.
    ///
    /// It is conversation content, so `/compact` discards it. That is the whole reason the
    /// reset also fires on `workspace(post_compact=true)`: without the compaction re-arm,
    /// one `/compact` removes the block for the rest of the session and nothing brings it
    /// back. `guide_hints_emitted` re-arms on the same signal for the same reason
    /// (`src/tools/config/mod.rs`, `post_compact_rearms_guide_hints`).
    ///
    /// A client that never sends `post_compact` degrades to losing the block after its
    /// first compaction — recoverable, and recoverable by exactly the route that makes
    /// these segments `Substitutable` in the first place (`memory(action="list")`,
    /// `index(action="status")`, `workspace(action="status")`).
    status_block_emitted_since_activation: Arc<std::sync::atomic::AtomicBool>,
    /// Tokio-clock instant of the most recent `call_tool`, watched by the optional
    /// idle-shutdown watchdog in `run()`.
    ///
    /// Updated on **tool calls only**. `list_tools`, resource reads and pings deliberately do
    /// not count, so "idle" means *did no work* rather than *sent no traffic* — a client that
    /// polls capabilities forever would otherwise pin the server open, which is the leak this
    /// exists to close (`docs/issues/archive/2026-07-28-mcp-servers-outlive-their-clients.md`).
    ///
    /// `tokio::time::Instant`, not `std::time::Instant`: the former is virtualised under
    /// `#[tokio::test(start_paused = true)]`, so the watchdog's tests are deterministic
    /// instead of wall-clock dependent. Same substitution WIN-30's budget test needed.
    last_activity: Arc<parking_lot::Mutex<tokio::time::Instant>>,
}

impl CodeScoutServer {
    pub async fn new(agent: Agent) -> Self {
        Self::new_with_env(agent, ServerEnv::from_env()).await
    }

    /// [`Self::new`] with the environment supplied explicitly — the test seam for
    /// callers (peer-serve, `make_server_no_project`) that build through `new`
    /// rather than `from_parts` directly. See [`ServerEnv`].
    pub async fn new_with_env(agent: Agent, env: ServerEnv) -> Self {
        let lsp = match agent.project_root().await {
            Some(root) => LspManager::new_arc_with_root(root),
            None => LspManager::new_arc(),
        };
        Self::from_parts_with_env(agent, lsp, false, env).await
    }

    /// Create a server with an existing LspManager (used for HTTP multi-session).
    pub async fn from_parts(agent: Agent, lsp: Arc<dyn LspProvider>, debug: bool) -> Self {
        Self::from_parts_with_env(agent, lsp, debug, ServerEnv::from_env()).await
    }

    /// [`Self::from_parts`] with the environment supplied explicitly — the test seam.
    /// See [`ServerEnv`] for why tests inject rather than `set_var`.
    pub async fn from_parts_with_env(
        agent: Agent,
        lsp: Arc<dyn LspProvider>,
        debug: bool,
        env: ServerEnv,
    ) -> Self {
        let status = agent.project_status().await;
        let instructions = crate::prompts::build_server_instructions(status.as_ref());
        #[cfg_attr(not(feature = "librarian"), allow(unused_mut))]
        let mut tools: Vec<Arc<dyn Tool>> = vec![
            // File tools (fully implemented)
            Arc::new(ReadFile),
            Arc::new(Tree),
            Arc::new(Grep),
            Arc::new(CreateFile),
            Arc::new(EditFile),
            Arc::new(EditMarkdown),
            Arc::new(ReadMarkdown),
            // Workflow tools
            Arc::new(RunCommand),
            Arc::new(Onboarding),
            Arc::new(ApproveWrite),
            // Symbol tools (stub — require LSP)
            Arc::new(Symbols),
            Arc::new(References),
            Arc::new(SymbolAt),
            Arc::new(CallGraph),
            Arc::new(EditCode),
            // Memory tools
            Arc::new(Memory),
            // Semantic search tools
            Arc::new(SemanticSearch),
            Arc::new(Index),
            // Config tools
            Arc::new(Workspace),
            // Library tools
            Arc::new(Library),
            // Deep-guidance tool — see docs/architecture/mcp-channel-caps.md
            Arc::new(crate::tools::guide::GetGuide::new()),
        ];
        // Peer delegation tool (Unix-only — uses Unix domain sockets) — opt-in,
        // not opt-out: see peer_enabled_at_runtime for why.
        #[cfg(unix)]
        if peer_enabled_at_runtime(
            env.peer_enabled.as_deref(),
            status.as_ref().map(|s| s.path.as_str()),
        ) {
            tools.push(Arc::new(crate::tools::peer::PeerTool));
        }
        if env.probe {
            tools.push(Arc::new(crate::tools::probe::ProbeTool));
            tracing::warn!(
                "CODESCOUT_PROBE=1 — registering __probe_description_cap__ \
                 (debug-only; ~8.8KB description with sentinel markers)"
            );
        }
        #[cfg(feature = "librarian")]
        if librarian_enabled_at_runtime(status.as_ref().map(|s| s.path.as_str())) {
            if let Some(lib_ctx) =
                crate::librarian::try_build_runtime_with(lsp.clone(), &env.librarian).await
            {
                // The markdown guard needs the catalog to recognise augmented
                // artifacts whose frontmatter carries no id. BL-33.
                crate::librarian::install_augmentation_guard_oracle(&lib_ctx);
                // The write-side twin: a direct frontmatter edit moves catalog-indexed
                // columns, and nothing else brings the row back into step. BL-48.
                crate::librarian::install_catalog_frontmatter_sync(&lib_ctx);
                tools.extend(crate::librarian::adapters_for(lib_ctx));
            }
        }
        let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));
        let section_coverage = Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        ));
        // Persisted guide-hint ledger keyed by the Claude Code conversation id,
        // so it survives /mcp restarts within one conversation instead of
        // re-injecting every guide body — except the session-opening guide,
        // which the non-empty-ledger re-arm below deliberately re-sends on
        // every reconnect. Prefer CLAUDE_CODE_SESSION_ID (set in
        // the MCP subprocess env since CC v2.1.154; per-process, so concurrent
        // CC windows don't collide), fall back to the companion's
        // .codescout/cc_session_id file, then a random uuid. See
        // docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md and
        // memory `claude-code-mcp-env`.
        let guide_project_root = agent.project_root().await;
        let cc_session_id = env
            .cc_session_id
            .clone()
            .or_else(|| {
                guide_project_root
                    .as_ref()
                    .and_then(|r| {
                        std::fs::read_to_string(r.join(".codescout").join("cc_session_id")).ok()
                    })
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        // The ledger's key is resolved separately from the usage-correlation id
        // above: usage tolerates a collision between two windows on one repo,
        // the ledger does not. See the plan's Ruling 3 and spec §1.
        let session_key = crate::tools::session_key::resolve(
            env.session_id_explicit.clone(),
            env.harness_session_ids.clone(),
        );
        if session_key.id().is_none() {
            tracing::info!(
                "no conversation id available (checked CODESCOUT_SESSION_ID and {:?}); \
                     guide ledger is in-process only and re-arms after idle",
                crate::tools::session_key::HARNESS_SESSION_VARS,
            );
        }
        // Per-USER state, not per-project: the ledger follows the conversation,
        // and one conversation can span worktrees, sub-projects and repos. Keeping
        // it under a project root made it depend on the companion plugin's
        // worktree symlink and made it silently ephemeral whenever the cwd was not
        // a project. See docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md §2.
        //
        // The `or_else` fallback below (real `per_user_state_dir()`) is deliberately
        // untested: every test constructs its server with an injected
        // `guide_hints_dir`, so no test reads, writes, or garbage-collects the real
        // per-user state directory — exercising this branch directly would mean
        // doing exactly that. The one test that spawns the real binary as a child
        // process (`tests/cross_process_write_lock.rs`) can't reach `ServerEnv`
        // injection at all — it gets there by overriding `XDG_STATE_HOME` on the
        // child's environment instead, which the fallback above reads through
        // `per_user_state_dir()`. See
        // docs/issues/archive/2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md.
        let guide_hints_dir = env.guide_hints_dir.clone().or_else(|| {
            crate::util::fs::per_user_state_dir().map(|d| d.join("codescout").join("guide_hints"))
        });
        // Published ONLY here — see `src/tools/rendezvous.rs` module doc for why
        // this must not move into any shared init path that `codescout mux` also
        // runs (W-47, docs/trackers/bug-fix-session-log.md).
        let servers_dir = env.servers_dir.clone().or_else(|| {
            crate::util::fs::per_user_state_dir().map(|d| d.join("codescout").join("servers"))
        });
        let rendezvous = Arc::new(parking_lot::Mutex::new(
            crate::tools::rendezvous::Rendezvous::publish(servers_dir, session_key.id()),
        ));
        let idle_ttl = env.guide_idle_ttl.unwrap_or(std::time::Duration::from_secs(
            crate::tools::guide_ledger::DEFAULT_IDLE_TTL_SECS,
        ));
        let guide_hints_emitted = Arc::new(parking_lot::Mutex::new(match session_key.id() {
            Some(id) => crate::tools::guide_ledger::GuideLedger::load(id, guide_hints_dir),
            // Zero is an explicit operator opt-out, accepting starvation.
            None => crate::tools::guide_ledger::GuideLedger::anonymous(
                (!idle_ttl.is_zero()).then_some(idle_ttl),
            ),
        }));
        // A non-empty ledger at construction means a prior server already served
        // this conversation. Either this is a reconnect against the same project
        // (re-arming the bootstrap costs one re-send) or against a different one
        // (re-arming is exactly right, and closes the suppression Phase A
        // knowingly created when it dropped the project from the ledger key —
        // see the guide-ledger-phase-c-rearm plan, Task 4).
        //
        // Deliberately NOT a root comparison: nothing persists the root a session
        // was last seen with (`GuideLedger::persist` serializes only `emitted`),
        // and adding one means a third on-disk shape plus migration from two
        // predecessors. See the plan's Ruling 2.
        {
            let mut led = guide_hints_emitted.lock();
            if !led.is_empty() {
                led.re_arm(PROJECT_SCOPED);
            }
        }
        let resources = Arc::new(tokio::sync::RwLock::new(Arc::new(
            build_resource_registry(&agent, Arc::clone(&lsp), &tools).await,
        )));

        // Pre-warm JVM LSP servers if the startup project contains java/kotlin.
        if let Some(root) = agent.project_root().await {
            let prewarm_langs = agent
                .with_project(|p| Ok(p.config.project.languages.clone()))
                .await
                .unwrap_or_default();
            crate::lsp::prewarm_lsp_background(Arc::clone(&lsp), root, &prewarm_langs);
        }

        Self {
            agent,
            lsp,
            output_buffer,
            tools,
            instructions: Arc::new(parking_lot::RwLock::new(instructions)),
            section_coverage,
            guide_hints_emitted,
            session_id: uuid::Uuid::new_v4().to_string(),
            cc_session_id,
            session_key,
            rendezvous,
            debug,
            last_broadcast_caps: Arc::new(parking_lot::Mutex::new(None)),
            resources,
            path_note_emitted_since_activation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            status_block_emitted_since_activation: Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            )),
            last_activity: Arc::new(parking_lot::Mutex::new(tokio::time::Instant::now())),
        }
    }

    /// Clone of the last-activity clock for the serve loop, which cannot borrow the server
    /// because `serve()` consumes it.
    pub(crate) fn last_activity_handle(&self) -> Arc<parking_lot::Mutex<tokio::time::Instant>> {
        self.last_activity.clone()
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

    // Peer-serve helper; the only non-test caller is the cfg(unix) peer module,
    // so it reads as dead in non-test Windows builds. Keep compiled (tests use it).
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name().to_string()).collect()
    }
    #[cfg_attr(not(unix), allow(dead_code))] // peer-serve helper (cfg(unix) caller)
    pub(crate) fn output_buffer_ref(
        &self,
    ) -> std::sync::Arc<crate::tools::output_buffer::OutputBuffer> {
        self.output_buffer.clone()
    }

    #[cfg_attr(not(unix), allow(dead_code))] // peer-serve helper (cfg(unix) caller)
    pub(crate) async fn project_root_string(&self) -> String {
        self.agent
            .project_root()
            .await
            .map(|r| r.display().to_string())
            .unwrap_or_default()
    }

    #[cfg_attr(not(unix), allow(dead_code))] // peer-serve helper (cfg(unix) caller)
    pub(crate) async fn project_name(&self) -> String {
        self.agent
            .project_root()
            .await
            .and_then(|r| r.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }

    #[cfg(all(test, unix))]
    pub(crate) async fn agent_security_config(
        &self,
    ) -> crate::util::path_security::PathSecurityConfig {
        self.agent.security_config().await
    }

    fn parse_input(arguments: Option<serde_json::Map<String, Value>>) -> Value {
        arguments
            .map(Value::Object)
            .unwrap_or(Value::Object(Default::default()))
    }

    async fn check_tool_access(
        &self,
        name: &str,
        workspace_override: Option<&std::path::Path>,
    ) -> std::result::Result<(), CallToolResult> {
        let security = self.agent.security_config_for(workspace_override).await;
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
            guide_hints_emitted: self.guide_hints_emitted.clone(),
            workspace_override: None,
        }
    }
    /// Phase 2: extract an optional `workspace` pin from tool input. The value
    /// is a path string, canonicalized to match the registry's canonical-root
    /// keys. No tool consumes this yet — Phase 3 wires `with_project_at`.
    /// See docs/plans/2026-05-30-per-request-workspace-pinning.md.
    fn extract_workspace_override(input: &Value) -> Option<std::path::PathBuf> {
        let raw = input.get("workspace")?.as_str()?;
        if raw.trim().is_empty() {
            return None;
        }
        let p = std::path::PathBuf::from(raw);
        Some(std::fs::canonicalize(&p).unwrap_or(p))
    }

    /// Inject the optional `workspace` pin into a pinnable tool's advertised
    /// input schema (regime-3). Optional (never added to `required`); idempotent —
    /// an existing `workspace` property is left untouched.
    fn inject_workspace_param(schema_obj: &mut serde_json::Map<String, Value>) {
        let props = schema_obj
            .entry("properties")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(props) = props.as_object_mut() {
            props.entry("workspace").or_insert_with(|| {
                serde_json::json!({
                    "type": "string",
                    "description": "Absolute workspace path to resolve this call against; omit for the active project. For concurrent subagents in different workspaces."
                })
            });
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
        // regime-3: pin the write guard to the SAME workspace the tool body will
        // resolve. A write pinned via `workspace` must acquire that project's
        // write_lock/file_lock, not the session default's — otherwise a
        // concurrent subagent's activate() steals the lock target.
        let override_root = Self::extract_workspace_override(input);
        let (mutex, fd_lock, timeout_secs) = self
            .agent
            .with_project_at(override_root.as_deref(), |p| {
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
                    // docs/issues/archive/2026-04-16-mcp-cancel-disconnect.md).
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
                    // docs/issues/archive/2026-04-16-mcp-cancel-disconnect.md).
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

    /// Append the once-per-activation `[codescout] paths are relative to <root>`
    /// banner.
    ///
    /// This method no longer transforms result text. Project-root stripping is
    /// field-aware and happens upstream, on the typed `Value`, inside
    /// `Tool::call_content` — see `src/tools/core/path_strip.rs`. Doing it here,
    /// on rendered text, meant guessing from a one-character lookbehind: it
    /// stripped path literals out of file content and collapsed root-valued
    /// fields to `""`
    /// (docs/issues/archive/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md).
    ///
    /// `run_command` needs no special case any more. Its payload is
    /// `{"exit_code", "stdout", ...}` and `stdout` is not an allowlisted path
    /// key, so raw shell bytes are left verbatim by the allowlist itself
    /// rather than by a tool-name branch. It is excluded here only from the
    /// banner, which would be noise on raw shell output.
    async fn post_process(
        &self,
        mut call_result: CallToolResult,
        tool_name: &str,
        workspace_override: Option<&std::path::Path>,
    ) -> CallToolResult {
        if tool_name == "run_command" {
            return call_result;
        }
        let Some(root) = self.agent.project_root_for(workspace_override).await else {
            return call_result;
        };

        // Novelty-gated: emit only the FIRST eligible response since server
        // start or the last `activate_project`. `call_tool_inner` resets the
        // flag right before this very call, for any `workspace(activate)`
        // request — matched on request shape only, regardless of whether the
        // call actually succeeds — so the reset can never lag behind this
        // method's own read of it. See [`path_note_emitted_since_activation`].
        let already_emitted = self
            .path_note_emitted_since_activation
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if !already_emitted {
            let root = to_forward_slash(&root);
            call_result.content.push(Content::text(format!(
                "\n[codescout] paths are relative to {root}"
            )));
        }

        // The `Substitutable` half of `## Project Status`, which the 2048-char instructions
        // channel could not carry — with custom instructions present, several of these
        // segments never arrived at all. Here they arrive whole: no fitting, no memory-name
        // cap, no trim note.
        //
        // Status is fetched only when the gate is open, so the ordinary path costs one
        // relaxed atomic load and nothing else. The gate is claimed AFTER the block is
        // known to be non-empty, mirroring the root check above: consuming it to emit
        // nothing would silently spend the one chance this session had.
        if !self
            .status_block_emitted_since_activation
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            if let Some(status) = self.agent.project_status().await {
                if let Some(block) = crate::prompts::build_status_response_block(&status) {
                    self.status_block_emitted_since_activation
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    call_result.content.push(Content::text(block));
                }
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
        let new_instructions = crate::prompts::build_server_instructions(status.as_ref());
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
        let has_embeddings = cfg!(any(
            feature = "local-embed",
            feature = "local-embed-dynamic",
            feature = "remote-embed"
        ));

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

        // shell_enabled: false only when this project sets
        // security.shell_command_mode = "disabled". Read through `with_project`
        // rather than `agent.security_config()` for the same reason
        // `has_git_remote` reads a cached field: `security_config()` also
        // populates `library_paths` from the library registry, and list_tools
        // fires frequently enough that a second registry read for one string is
        // not worth it.
        //
        // `!= "disabled"` and not `== "warn" || == "unrestricted"`: an
        // unrecognised mode must leave the tool VISIBLE, so the call lands on
        // `run_command_inner`'s `unknown shell_command_mode: '<x>'` error.
        // Whitelisting the good values would turn a config typo into a silently
        // absent tool, which tells the caller nothing.
        //
        // Defaults to `true` with no active project, matching
        // `PathSecurityConfig::default()`'s "warn".
        let shell_enabled = self
            .agent
            .with_project(|p| Ok(p.config.security.shell_command_mode != "disabled"))
            .await
            .unwrap_or(true);

        crate::tools::ToolCapabilities {
            has_lsp,
            has_embeddings,
            has_git_remote,
            has_libraries,
            shell_enabled,
        }
    }

    #[cfg_attr(not(unix), allow(dead_code))] // peer-serve dispatch entry (cfg(unix) caller)
    /// Dispatch a tool by name with raw JSON args, returning the full
    /// `CallToolResult`. Routes through `call_tool_inner`, so access checks, the
    /// write-guard, usage recording, and error routing all apply. Used by the
    /// peer-serve endpoint; carries no rmcp request/progress/peer coupling.
    ///
    /// `CallToolRequestParams` has no direct constructor in this crate (it is only
    /// ever received from rmcp), so it is rebuilt from the canonical MCP params
    /// shape via its `Deserialize` impl.
    pub(crate) async fn call_tool_by_name(
        &self,
        name: &str,
        args: Value,
    ) -> std::result::Result<CallToolResult, McpError> {
        let req: CallToolRequestParams = serde_json::from_value(serde_json::json!({
            "name": name,
            "arguments": args,
        }))
        .map_err(|e| {
            McpError::invalid_params(format!("failed to build tool request: {e}"), None)
        })?;
        self.call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
    }

    /// Notice a conversation change stamped into our rendezvous slot, and re-arm
    /// the guide ledger for the new conversation.
    ///
    /// This is the `/clear` fix: Claude Code mints a new conversation id WITHOUT
    /// respawning the MCP subprocess, so the server keeps serving the new
    /// conversation under the old key and suppresses guides it should re-send.
    /// The server has to be *told*, and the companion hook is what tells it.
    /// docs/issues/archive/2026-08-18-clear-leaves-mcp-session-id-stale.md
    ///
    /// Called from `call_tool_inner`, which is the single production funnel for
    /// every tool call — MCP requests and peer-served ones alike (see
    /// [`call_tool_by_name`](Self::call_tool_by_name)) — and therefore covers
    /// every guide-eligible request, since `Tool::call_content` (where guide
    /// delivery is decided) has no other production caller.
    ///
    /// Agent-agnostic: with no companion hook installed nothing ever stamps the
    /// slot, [`Rendezvous::poll`] stays quiet forever, and the anonymous-tier
    /// idle TTL is what eventually catches `/clear` — one interval late. The
    /// companion *adds* enforcement; the server degrades without it.
    ///
    /// Returns the rendezvous's current conversation id, if it has one, so the
    /// caller can use it for usage-telemetry attribution instead of the
    /// construction-time `cc_session_id` snapshot — which is otherwise never
    /// updated for the lifetime of the process.
    /// docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md
    fn poll_rendezvous(&self) -> Option<String> {
        // Two independent mutexes, never held together: the rendezvous guard is
        // scoped to the block below and drops before the ledger lock is taken.
        let (changed, active, current) = {
            let mut rv = self.rendezvous.lock();
            let changed = rv.poll();
            (changed, rv.is_active(), rv.current().map(str::to_string))
        };
        let mut led = self.guide_hints_emitted.lock();
        led.set_rendezvous_active(active);
        if let Some(session) = changed {
            tracing::info!(
                session = %session,
                "conversation changed via the rendezvous slot; re-arming the guide ledger"
            );
            led.rekey(&session);
        }
        current
    }

    /// Drive [`poll_rendezvous`](Self::poll_rendezvous) without routing a tool
    /// call through the whole request path.
    ///
    /// Gated with its only callers, not with a bare `#[cfg(test)]`: all three live
    /// in `guide_hint_tests`, which is `#[cfg(feature = "librarian")]`. A bare test
    /// gate compiles this under `--no-default-features --all-targets`, where
    /// nothing can call it, and `dead_code` fires. Same shape as
    /// [`os_random_auth_token`] and [`ct_eq`], which are gated with the `http` arm
    /// they serve.
    #[cfg(all(test, feature = "librarian"))]
    pub(crate) fn rendezvous_poll_for_test(&self) {
        self.poll_rendezvous();
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
        // Record the in-flight tool for the durable heartbeat (OOM forensics):
        // if RSS climbs during this call, the heartbeat line names the operation.
        crate::heartbeat::note_tool(&req.name);
        let tool_start = std::time::Instant::now();

        let tool = self.resolve_tool(&req.name)?;

        let input: Value = Self::parse_input(req.arguments);
        let workspace_override = Self::extract_workspace_override(&input);

        // A per-request workspace= pin is the caller's explicit, deliberate
        // choice of target — they named the exact path. For a write-tool
        // call, grant that pinned workspace write access on first residency
        // (or upgrade it if already resident read-only) instead of leaving
        // it at ensure_resident's read-only default. Without this, a pin to
        // a workspace that was never separately `activate`d always fails
        // "file writes disabled" below, even though the pin itself already
        // is the caller's consent — and `activate`ing it instead would clear
        // every other resident workspace (see `Agent::activate`), defeating
        // the point of pinning. Read-only calls never reach this branch, so
        // a pinned read still gets the safer read-only default.
        if let Some(root) = workspace_override.as_deref() {
            if tool.is_write(&input) {
                let _ = self
                    .agent
                    .ensure_resident(root.to_path_buf(), Some(false))
                    .await;
            }
        }

        if let Err(err) = self
            .check_tool_access(&req.name, workspace_override.as_deref())
            .await
        {
            return Ok(err);
        }

        // BEFORE the ledger this request will consult, and therefore before
        // `tool.call_content` below decides guide delivery: a `/clear` mints a
        // new conversation id without respawning us, so the ledger has to be
        // re-armed for it here rather than at construction. Position is
        // load-bearing, not stylistic — polling after the tool ran would let
        // the first post-`/clear` response answer from the stale ledger and
        // suppress a guide the new conversation never received. Guarded by
        // `guide_hint_tests::a_tool_call_polls_the_rendezvous_and_re_arms`.
        //
        // The returned id is ALSO the fix for
        // docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md:
        // `self.cc_session_id` is a construction-time snapshot that a `/clear` or a
        // subagent reusing this live process never updates, while the rendezvous is
        // polled on every call and tracks the conversation we are CURRENTLY serving.
        let rendezvous_session = self.poll_rendezvous();

        let mut ctx = self.build_context(progress, peer);
        ctx.workspace_override = workspace_override;

        let timeout_secs = if tool_skips_server_timeout(&req.name) {
            None
        } else {
            self.agent
                .with_project_at(ctx.workspace_override.as_deref(), |p| {
                    Ok(p.config.project.tool_timeout_secs)
                })
                .await
                .ok()
        };

        let recorder = UsageRecorder::new(
            self.agent.clone(),
            self.debug,
            self.session_id.clone(),
            rendezvous_session.unwrap_or_else(|| self.cc_session_id.clone()),
        );
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

        let tool_call_fut = recorder.record_content(
            &req.name,
            &input_for_record,
            ctx.workspace_override.as_deref(),
            || tool.call_content(input, &ctx),
        );

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
            Err(e) => {
                // Attach the gate CONDITION to the first refusal of each family
                // per session. This is the only point where it can be done:
                // `Tool::call_content`'s guide hook sits after `self.call(..)?`,
                // so an `Err` never reaches it, and `post_process` returns early
                // for `run_command` — which is exactly where IL-3 fires.
                //
                // Agents obey an Iron-Law refusal on the next call 96% of the
                // time and re-offend later in 47% of sessions: the message
                // teaches the CALL, never the PREDICATE. See
                // `prompts::refusal_predicate`. GF-4 / GF-5 in
                // docs/trackers/2026-08-16-iron-law-gate-firing-audit.md.
                let family = crate::usage::db::normalize_err_family(&req.name, &e.to_string());
                let mut result = route_tool_error(e);
                if let Some(text) =
                    family
                        .and_then(crate::prompts::refusal_predicate)
                        .filter(|_| {
                            // `notice_once`, not `insert`: under the opener's
                            // trigger (the `!emitted.contains(SESSION_OPENING_GUIDE)`
                            // check in `Tool::call_content`, `src/tools/core/types.rs`)
                            // a key in `emitted` only risks suppressing the
                            // opener if it collides with that literal topic
                            // string — which this refusal key does not.
                            // `notice_once` still keeps it in the separate
                            // `notices` set regardless; see `GuideLedger::notices`.
                            ctx.guide_hints_emitted
                                .lock()
                                .notice_once(&format!("refusal-predicate:{}", family.unwrap_or("")))
                        })
                {
                    result.content.push(Content::text(text));
                }
                result
            }
        };

        let ok = call_result.is_error.is_none_or(|e| !e);
        tracing::debug!(ok, "tool result");
        tracing::info!(
            tool = %req.name,
            duration_ms = tool_start.elapsed().as_millis() as u64,
            ok,
            "tool_done"
        );

        // A `workspace(activate)` request resets the path-annotation novelty
        // gate HERE, right before this same call's own `post_process` below —
        // not afterward, in `call_tool`'s `is_activate` branch (the old
        // location). Resetting afterward meant this activate response would
        // consume the gate via `post_process`, then get it re-armed one line
        // later in the caller, so the very next ordinary response fired the
        // banner again — two banners per activation. Detected from
        // `input_for_record` rather than `req.arguments` because the latter
        // was already moved into `input` above; `input_for_record` is the
        // untouched clone `record_content` only borrowed.
        let is_activate = req.name == "workspace"
            && input_for_record.get("action").and_then(|v| v.as_str()) == Some("activate");

        // Both gates below re-arm on activation AND on compaction. Matched on request
        // shape here rather than in `ProjectStatus::call`'s `post_compact` arm for a
        // mechanical reason: the flags live on `CodeScoutServer` and that handler has
        // only `ToolContext`.
        //
        // `post_compact` is read WITHOUT requiring `action` to be absent — passing it with
        // no action infers `action="status"` (`src/tools/config/mod.rs`), so a match that
        // demanded a missing action would silently never fire on the common call.
        let is_post_compact = req.name == "workspace"
            && input_for_record
                .get("post_compact")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        // Reset together, in one block, because they carry the same kind of fact:
        // something stated once INTO THE CONVERSATION, which is precisely what
        // `/compact` discards. The path banner used to re-arm on activation only while
        // the status block already re-armed on both — one clause apart in this same
        // function — so after a compaction every response carried project-relative
        // paths with nothing left in context saying they were relative. Keeping the two
        // stores adjacent is what stops them drifting apart again.
        // docs/issues/archive/2026-08-21-path-relative-banner-not-rearmed-after-compaction.md
        if is_activate || is_post_compact {
            self.path_note_emitted_since_activation
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.status_block_emitted_since_activation
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }

        let call_result = self
            .post_process(call_result, &req.name, ctx.workspace_override.as_deref())
            .await;

        Ok(call_result)
    }
}

/// Returns true for tools that manage their own timeout internally and must not
/// be wrapped by the server-level `tool_timeout_secs` guard.
///
/// - `index` / `index_library`: embedding loops that run for many minutes.
/// - `run_command`: the caller supplies `timeout_secs` in the request params; the
///   server-level timeout is unaware of that value and would fire first, making
///   the per-request `timeout_secs` parameter effectively ignored.
fn tool_skips_server_timeout(name: &str) -> bool {
    matches!(name, "index" | "index_library" | "run_command")
}

/// Whether to register the embedded librarian tool surface for this session.
///
/// Layered defaults:
/// 1. `LIBRARIAN_ENABLED=0|false|off` env var disables (overrides everything).
/// 2. `LIBRARIAN_ENABLED=1|true|on` env var enables (overrides config).
/// 3. `[librarian] enabled = true|false` in `<project>/.codescout/project.toml`.
/// 4. Default: enabled (set `LIBRARIAN_ENABLED=0` or `[librarian] enabled = false` to opt out).
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
    true
}

/// Whether to register the peer-delegation tool for this session.
///
/// Layered defaults — same shape as [`librarian_enabled_at_runtime`], opposite
/// resting state:
/// 1. `env_override` `0|false|off|no` disables (overrides everything).
/// 2. `env_override` `1|true|on|yes` enables (overrides config).
/// 3. `[peer] enabled = true|false` in `<project>/.codescout/project.toml`.
/// 4. Default: **disabled** (opt in with `CODESCOUT_PEER_ENABLED=1` or
///    `[peer] enabled = true`).
///
/// Measured 2026-08-26 across every `.codescout/usage.db` on this machine (29
/// projects, every session that has ever run here): `peer` was called twice,
/// ever — once a success with no recorded detail, once an error from a guessed
/// `action="list"` (the tool only accepts `status` for that). Opt-out was
/// exposing a schema and description nobody was using by default.
///
/// `env_override` is taken as a parameter rather than read from
/// `std::env::var` directly (contrast `librarian_enabled_at_runtime`) so this
/// function stays a pure, independently-testable unit — see
/// `retrieval::config::parse_rerank_opt_in`'s doc comment for why an inline env
/// read is UB-risky against the test suite's concurrent `getenv` readers.
#[cfg(unix)]
fn peer_enabled_at_runtime(env_override: Option<&str>, project_path: Option<&str>) -> bool {
    if let Some(v) = env_override {
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
                    .get("peer")
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
                let mut schema_obj = schema.as_object().cloned().unwrap_or_default();
                if t.pinnable() {
                    Self::inject_workspace_param(&mut schema_obj);
                }
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
        // Idle-shutdown clock: a tool call is the definition of activity. Set before any
        // early return so a rejected or cancelled call still counts as the client being alive.
        *self.last_activity.lock() = tokio::time::Instant::now();
        let is_activate = req.name == "workspace"
            && req
                .arguments
                .as_ref()
                .and_then(|m| m.get("action"))
                .and_then(|v| v.as_str())
                == Some("activate");
        // Emit progress ONLY when the client opted in via `_meta.progressToken`.
        // The old behavior synthesized a token from `req_ctx.id`, producing
        // UNSOLICITED `notifications/progress` — which crash Claude Code 2.x (it
        // closes stdin on a progress notification it never requested; BUG-038).
        // `None` makes `ctx.progress.report()` a documented no-op, the correct
        // MCP behavior for a request that carried no progress token. This mirrors
        // rmcp's own server transport (`get_meta().get_progress_token()`).
        let progress = req_ctx
            .meta
            .get_progress_token()
            .map(|token| progress::ProgressReporter::new(req_ctx.peer.clone(), token.0));
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

            // The path-annotation novelty-gate reset for this activation
            // already happened inside `call_tool_inner`, right before ITS OWN
            // `post_process` call — not here. Resetting here (after
            // `call_tool_inner` had already returned, i.e. after this same
            // activate response had already been through `post_process`)
            // used to double-fire the banner: once on the activate response,
            // then again on the very next ordinary response once this line
            // re-armed the gate. See the docstring on
            // `path_note_emitted_since_activation`.
        }

        Ok(result)
    }
}

/// Static documentation resources whose bodies ship inside the binary.
///
/// Bodies are embedded via `include_str!` so doc URIs (`doc://...`) resolve
/// identically regardless of the active project root. A regression test
/// (`static_doc_sources_all_readable`) asserts every URI in this list is
/// readable so a stale path here turns into a compile-time `include_str!`
/// failure rather than a runtime `-32603` at call time.
fn static_doc_sources() -> Vec<crate::mcp_resources::doc::DocSource> {
    use crate::mcp_resources::doc::DocSource;
    vec![
        DocSource {
            uri: "doc://progressive-disclosure".into(),
            name: "progressive-disclosure".into(),
            description: Some(
                "Output sizing, overflow hints, agent guidance for codescout tools.".into(),
            ),
            content: include_str!("../docs/PROGRESSIVE_DISCOVERABILITY.md"),
        },
        DocSource {
            uri: "doc://librarian-guide".into(),
            name: "librarian-guide".into(),
            description: Some(
                "Full reference: artifact model, filter syntax, tracker workflow, \
                 augmentation lifecycle, librarian actions."
                    .into(),
            ),
            content: include_str!("prompts/guides/librarian.md"),
        },
    ]
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
        doc::DocProvider,
        memory::MemoryProvider,
        project_summary::{AgentSummarySource, ProjectSummaryProvider},
        tool_guide::ToolGuideProvider,
        tool_usage::{AgentUsageSource, ToolUsageProvider},
        ResourceRegistry,
    };

    let mut rr = ResourceRegistry::new();

    // Static docs — always available; bodies are embedded via include_str! so
    // they resolve identically regardless of the active project root.
    let _ = rr.try_register(Box::new(DocProvider::new(static_doc_sources())));

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

    // Probe — debug-only, gated on CODESCOUT_PROBE=1.
    if std::env::var("CODESCOUT_PROBE")
        .ok()
        .filter(|v| !v.is_empty() && v != "0")
        .is_some()
    {
        let _ = rr.try_register(Box::new(crate::mcp_resources::probe::ProbeProvider));
    }

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
    } else if e.to_string().contains("code -32800") || e.to_string().contains("code -32801") {
        // Transient LSP errors:
        // - -32800 RequestCancelled: server cancelled (workspace lock, cold indexing).
        // - -32801 ContentModified: server's analysis snapshot advanced mid-request
        //   (typical post-/mcp warmup; rust-analyzer publishes diagnostics and
        //   cancels requests against the stale snapshot).
        // Treat both as recoverable so sibling parallel tool calls are not aborted.
        // Log at WARN so this is visible in diagnostic logs (otherwise it appears as ok=true).
        let code = if e.to_string().contains("code -32801") {
            "-32801"
        } else {
            "-32800"
        };
        tracing::warn!("LSP transient error ({}): {}", code, e);
        let body = serde_json::json!({
            "error": e.to_string(),
            "hint": "The LSP server returned a transient error (RequestCancelled -32800 or \
                     ContentModified -32801). The client already auto-retries idempotent \
                     methods on these codes; this surfaces only when the retry budget is \
                     exhausted or the method is non-idempotent. Common causes:\n\
                     (1) Cold indexing window — server still building its workspace index \
                     (can take 1-5 minutes after `/mcp` reconnect or fresh launch).\n\
                     (2) Workspace lock contention — another codescout instance or editor \
                     LSP holds the workspace. For kotlin-lsp, each instance needs a separate \
                     --system-path to avoid contention on the IntelliJ platform's .app.lock.\n\
                     Wait and retry; or for non-idempotent methods (rename, applyEdit) \
                     re-issue manually after confirming server state."
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
/// The idle window after which a stdio server exits, parsed from
/// `CODESCOUT_IDLE_SHUTDOWN_SECS`.
///
/// `None` — unset, blank, unparseable, or `0` — means the watchdog never fires and behaviour is
/// exactly what it was. There is deliberately **no default**, and that is a design position, not
/// an omission: how long an MCP server may sit idle is a property of the operator's workflow,
/// which codescout cannot measure for them (memory `conventions` § *Environment-Agnostic
/// Tuning*). The inert default is also the safe one — a server that exits while its client is
/// merely quiet costs the user a reconnect, which is worse than one that lingers.
///
/// Note the mux's 180 s is **not** a precedent: a mux is re-dialled transparently on the next
/// navigation call, whereas an MCP server exiting is user-visible.
///
/// Kept pure — parsing a `&str` rather than reading the environment — so it is testable without
/// `std::env::set_var`, which is UB against the suite's concurrent `getenv` readers. See
/// `ServerEnv`.
pub(crate) fn parse_idle_shutdown(raw: Option<&str>) -> Option<std::time::Duration> {
    let secs: u64 = raw?.trim().parse().ok()?;
    (secs > 0).then(|| std::time::Duration::from_secs(secs))
}

/// Resolve `parse_idle_shutdown` against the live environment. Called only from `run()`.
fn idle_shutdown_from_env() -> Option<std::time::Duration> {
    parse_idle_shutdown(
        std::env::var("CODESCOUT_IDLE_SHUTDOWN_SECS")
            .ok()
            .as_deref(),
    )
}

/// Run `fut` to completion, or give up after `deadline` and return anyway.
///
/// A shutdown step that hangs (a wedged LSP client, a stuck lock) must never keep this
/// process alive past a signal that asked it to exit — see
/// `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`:
/// a correctly-installed `SIGTERM` handler is worthless if the code it hands off to can
/// block forever. `what` names the step in the warning log so a timed-out shutdown is
/// diagnosable after the fact.
pub(crate) async fn shutdown_with_deadline<F>(fut: F, deadline: std::time::Duration, what: &str)
where
    F: std::future::Future<Output = ()>,
{
    if tokio::time::timeout(deadline, fut).await.is_err() {
        tracing::warn!(
            what,
            deadline_secs = deadline.as_secs(),
            "shutdown_deadline_exceeded_abandoning"
        );
    }
}

/// Complete once no tool call has arrived for `limit`; never complete when `limit` is `None`.
///
/// The `None` arm awaits `pending()`, so the caller's `select!` arm stays unconditional and the
/// disabled case is *structurally* inert rather than guarded by a flag someone can get wrong.
pub(crate) async fn idle_watchdog(
    limit: Option<std::time::Duration>,
    last_activity: Arc<parking_lot::Mutex<tokio::time::Instant>>,
) {
    let Some(limit) = limit else {
        std::future::pending::<()>().await;
        return;
    };
    // Poll at a tenth of the window, bounded so a multi-hour window does not wake every few
    // seconds and a short one still resolves promptly.
    let tick = (limit / 10).clamp(
        std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(60),
    );
    loop {
        tokio::time::sleep(tick).await;
        if last_activity.lock().elapsed() >= limit {
            return;
        }
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
///
/// Only reachable from the `#[cfg(feature = "http")]` transport arm, so it is
/// gated to match — otherwise `--no-default-features` builds warn dead_code.
#[cfg(feature = "http")]
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
///
/// Gated with its only caller, the `http` transport arm.
#[cfg(feature = "http")]
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

    let project_display = agent
        .project_root()
        .await
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<none>".to_string());

    if debug {
        tracing::info!(
            pid = std::process::id(),
            version = env!("CARGO_PKG_VERSION"),
            instance = %instance_tag,
            project = %project_display,
            transport = %transport,
            "codescout_start"
        );
    }

    // Always-on durable OOM-forensics heartbeat: a synchronous, SIGKILL-proof
    // RSS line per tick to <cache>/codescout/heartbeats/<pid>.log. Survives the
    // kill and the unknown-cwd discoverability gap that lost the 68 GB instance's
    // diagnostic log. See docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md.
    crate::heartbeat::spawn_durable(instance_tag.to_owned(), project_display);

    // Heartbeat: distinguishes idle from hung. (Rich fields, --debug only; rides
    // the lossy non-blocking appender — the durable sink above is the forensic one.)
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
                let mem = crate::heartbeat::read_self_memory_kb();
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

            // Opt-in idle shutdown. Neither channel codescout can watch will ever fire for a
            // session whose client has been abandoned but not exited: stdin stays open because
            // the parent holds the socketpair, and no signal arrives because the parent is
            // alive. Time since the last tool call is the only remaining observable. Unset =>
            // `None` => the watchdog below never resolves, so this is inert by default.
            let idle_after = idle_shutdown_from_env();
            let last_activity = server.last_activity_handle();
            if let Some(limit) = idle_after {
                tracing::info!(
                    instance = %instance_tag,
                    idle_shutdown_secs = limit.as_secs(),
                    "idle shutdown armed"
                );
            }

            let (stdin, stdout) = rmcp::transport::stdio();
            let service = server
                .serve((ResilientStdin::new(stdin), stdout))
                .await
                .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;

            // Wait for service to end OR shutdown signal OR the idle window to elapse
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
                _ = idle_watchdog(idle_after, last_activity) => {
                    tracing::info!(
                        instance = %instance_tag,
                        reason = "idle_timeout",
                        idle_shutdown_secs = idle_after.map(|d| d.as_secs()),
                        "service_exit"
                    );
                }
            }

            // Gracefully shut down all LSP servers. Bounded: a wedged LSP client must
            // never keep this process alive past a signal that asked it to exit — see
            // shutdown_with_deadline's doc comment.
            const LSP_SHUTDOWN_DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);
            tracing::info!("Shutting down LSP servers...");
            shutdown_with_deadline(
                lsp.shutdown_all(),
                LSP_SHUTDOWN_DEADLINE,
                "lsp_shutdown_all",
            )
            .await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use tempfile::tempdir;

    /// Test `ServerEnv` with the guide-hint ledger pinned inside `dir`, so no test
    /// ever reads, writes, or garbage-collects the real per-user state directory.
    fn test_env(dir: &std::path::Path) -> ServerEnv {
        ServerEnv {
            guide_hints_dir: Some(dir.join("guide_hints")),
            servers_dir: Some(dir.join("servers")),
            ..Default::default()
        }
    }

    /// `100_000_000_000_000` secs is inside the measured live panic band:
    /// `Duration::from_std`'s own guard only rejects values adjacent to
    /// `u64::MAX`, but feeding this unclamped value into `Utc::now() - ttl`
    /// panics inside chrono's `checked_sub_signed(...).expect(...)`. A clamp
    /// that only special-cased `u64::MAX` would let this value straight
    /// through — that split (one safe, one panicking, at opposite ends of the
    /// u64 range) is exactly the trap this test pins against.
    #[test]
    fn parse_guide_idle_ttl_clamps_a_value_from_the_chrono_panic_band() {
        let ttl = parse_guide_idle_ttl("100000000000000").expect("a valid u64 must parse");
        assert!(
            ttl <= std::time::Duration::from_secs(MAX_GUIDE_TTL_SECS),
            "must be clamped to the 100-year ceiling, got {ttl:?}"
        );

        // Prove it end-to-end: feed the clamped value through the exact chrono
        // subtraction `expire_idle` performs on every anonymous-tier `tick()`.
        // An unclamped value here panics; a clamped one does not, and correctly
        // reports nothing expired for a stamp inserted moments ago.
        let mut ledger = crate::tools::guide_ledger::GuideLedger::anonymous(Some(ttl));
        ledger.insert("librarian".to_string());
        assert_eq!(
            ledger.tick(),
            0,
            "a fresh stamp must not expire under a 100-year TTL"
        );
    }

    async fn make_server() -> (tempfile::TempDir, CodeScoutServer) {
        make_server_with_project_toml(None).await
    }

    /// `make_server`, plus an optional `.codescout/project.toml`.
    ///
    /// The file must be written BEFORE `Agent::new`, which is the only window in
    /// which it is read: `ProjectConfig::load_or_default` runs during agent
    /// construction, so a config written afterwards is invisible to the session.
    /// That ordering is the whole reason this helper exists rather than callers
    /// writing the file themselves after `make_server()`.
    async fn make_server_with_project_toml(
        project_toml: Option<&str>,
    ) -> (tempfile::TempDir, CodeScoutServer) {
        let dir = tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        let ws_path = codescout_dir.join("librarian-workspace.toml");
        std::fs::write(&ws_path, "").unwrap();
        if let Some(project_toml) = project_toml {
            std::fs::write(codescout_dir.join("project.toml"), project_toml).unwrap();
        }

        // `ServerEnv::librarian` only exists with the `librarian` feature on;
        // without the gate this helper fails to compile under
        // `--no-default-features` / `--features local-embed`.
        #[cfg(feature = "librarian")]
        let env = ServerEnv {
            librarian: crate::librarian::LibrarianEnv {
                workspace: Some(ws_path),
                db: Some(codescout_dir.join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        #[cfg(not(feature = "librarian"))]
        let env = test_env(dir.path());

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;
        (dir, server)
    }

    async fn make_server_no_project() -> (tempfile::TempDir, CodeScoutServer) {
        let dir = tempfile::tempdir().unwrap();
        let agent = Agent::new(None).await.unwrap();
        let env = test_env(dir.path());
        let server = CodeScoutServer::new_with_env(agent, env).await;
        (dir, server)
    }

    #[tokio::test]
    async fn server_registers_all_tools() {
        let (_dir, server) = make_server().await;
        // `peer` is opt-in (see peer_enabled_at_runtime) and not registered by
        // `make_server()`'s default env, on any platform.
        let expected_tools = vec![
            "read_file",
            "tree",
            "grep",
            "create_file",
            "edit_file",
            "edit_markdown",
            "read_markdown",
            "run_command",
            "onboarding",
            "approve_write",
            "symbols",
            "references",
            "call_graph",
            "edit_code",
            "symbol_at",
            "memory",
            "semantic_search",
            "index",
            "workspace",
            "library",
            "get_guide",
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

    #[tokio::test]
    async fn server_tool_count_is_l3_target() {
        let (_dir, server) = make_server().await;
        let core_count = server
            .tools
            .iter()
            .filter(|t| !is_librarian_tool(t.name()))
            .count();
        // `peer` is opt-in (see peer_enabled_at_runtime) and not registered by
        // `make_server()`'s default env, so the L3 target is 21 core tools
        // regardless of platform.
        let expected = 21;
        assert_eq!(
            core_count,
            expected,
            "L3 target is {expected} core tools; got {}: {:?}",
            core_count,
            server.tools.iter().map(|t| t.name()).collect::<Vec<_>>()
        );
    }

    /// Regression for the near-zero-adoption finding (2 calls across every
    /// `.codescout/usage.db` on the machine, one an error from a wrong action
    /// guess): `peer` must be opt-in, not opt-out, so its schema and description
    /// stop reaching every session by default.
    #[cfg(unix)]
    #[tokio::test]
    async fn peer_tool_absent_by_default() {
        let (_dir, server) = make_server().await;
        assert!(
            server.find_tool("peer").is_none(),
            "peer must not be registered without an explicit opt-in"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn peer_tool_present_when_env_enabled() {
        let dir = tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(codescout_dir.join("librarian-workspace.toml"), "").unwrap();
        let env = ServerEnv {
            peer_enabled: Some("1".to_string()),
            ..test_env(dir.path())
        };
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;
        assert!(
            server.find_tool("peer").is_some(),
            "CODESCOUT_PEER_ENABLED=1 must register peer"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn peer_tool_present_when_project_toml_enables_it() {
        let dir = tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(codescout_dir.join("librarian-workspace.toml"), "").unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[peer]\nenabled = true\n",
        )
        .unwrap();
        let env = test_env(dir.path());
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;
        assert!(
            server.find_tool("peer").is_some(),
            "[peer] enabled = true in project.toml must register peer"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn peer_tool_env_var_off_overrides_project_toml_on() {
        let dir = tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(codescout_dir.join("librarian-workspace.toml"), "").unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[peer]\nenabled = true\n",
        )
        .unwrap();
        let env = ServerEnv {
            peer_enabled: Some("0".to_string()),
            ..test_env(dir.path())
        };
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;
        assert!(
            server.find_tool("peer").is_none(),
            "CODESCOUT_PEER_ENABLED=0 must override [peer] enabled = true in project.toml"
        );
    }

    /// Gated to match the function under test. `peer_enabled_at_runtime` is
    /// `#[cfg(unix)]` — peer delegation uses Unix domain sockets — so on a Windows
    /// target an ungated `use super::…` here resolves to an item that was configured
    /// out. That is E0432, a hard compile failure of the whole test binary rather
    /// than a skipped test, and the host `cargo clippy` / `cargo test` cannot see it
    /// because the cfg erases the arm they compile.
    #[cfg(unix)]
    mod peer_enabled_at_runtime_tests {
        use super::peer_enabled_at_runtime;

        #[test]
        fn defaults_to_disabled_with_no_env_no_config() {
            assert!(!peer_enabled_at_runtime(None, None));
        }

        #[test]
        fn env_var_1_enables_regardless_of_project() {
            assert!(peer_enabled_at_runtime(Some("1"), None));
            assert!(peer_enabled_at_runtime(Some("true"), None));
            assert!(peer_enabled_at_runtime(Some("ON"), None));
        }

        #[test]
        fn env_var_0_disables_regardless_of_project() {
            assert!(!peer_enabled_at_runtime(Some("0"), None));
            assert!(!peer_enabled_at_runtime(Some("false"), None));
            assert!(!peer_enabled_at_runtime(Some("OFF"), None));
        }

        #[test]
        fn unrecognised_env_value_falls_through_to_default() {
            assert!(!peer_enabled_at_runtime(Some("banana"), None));
        }

        #[test]
        fn project_toml_enabled_true_wins_over_default() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
            std::fs::write(
                dir.path().join(".codescout").join("project.toml"),
                "[peer]\nenabled = true\n",
            )
            .unwrap();
            assert!(peer_enabled_at_runtime(
                None,
                Some(dir.path().to_str().unwrap())
            ));
        }

        #[test]
        fn missing_project_toml_is_disabled() {
            let dir = tempfile::tempdir().unwrap();
            assert!(!peer_enabled_at_runtime(
                None,
                Some(dir.path().to_str().unwrap())
            ));
        }
    }

    fn is_librarian_tool(name: &str) -> bool {
        name.starts_with("artifact_")
            || name.starts_with("librarian_")
            || name.starts_with("tracker_")
            || name == "workspace_state_at"
            || name == "tracker_design"
            || name == "artifact"
            || name == "librarian"
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

    #[tokio::test]
    async fn tool_descriptions_report_lengths() {
        // Companion to `tool_descriptions_stay_under_budget` — no assertions,
        // just prints each tool's description length sorted descending. Run
        // with `cargo test --lib tool_descriptions_report_lengths -- --nocapture`
        // to audit how close each tool is to the 300-char cap. Useful when
        // adding new tools or trimming overgrown descriptions; do NOT delete
        // this even though it has no assertions — its purpose is observability.
        let (_dir, server) = make_server().await;
        let mut lengths: Vec<(String, usize, bool)> = server
            .tools
            .iter()
            .map(|t| {
                (
                    t.name().to_string(),
                    t.description().len(),
                    is_librarian_tool(t.name()),
                )
            })
            .collect();
        lengths.sort_by_key(|b| std::cmp::Reverse(b.1));
        println!(
            "\n  len  cap  tool                                            (exempt = librarian)"
        );
        println!("  ---  ---  ---------------------------------------------");
        for (name, len, exempt) in &lengths {
            let cap = if *exempt { "  -" } else { "300" };
            let flag = if *exempt {
                ""
            } else if *len > 270 {
                "  ⚠ near cap"
            } else {
                ""
            };
            println!("  {len:>3}  {cap}  {name:<45}{flag}");
        }
    }

    // ---------- Tool surface budget (spec 2026-08-18-tool-surface-budget-design) ----------

    /// Reproduce the tool surface exactly as `list_tools` advertises it.
    ///
    /// `list_tools` does three things between a tool's raw `input_schema()` and the
    /// wire: filters on `availability(&caps)`, injects the `workspace` pin for
    /// `pinnable()` tools, and pairs each schema with its description. A budget
    /// computed from bare `input_schema()` would miss ~6.2 KB of injected `workspace`
    /// prose and count tools the client never sees — measuring a string nobody
    /// receives, which is exactly the defect
    /// `prompts::redesign_invariants::production_render_fits_the_client_channel`
    /// exists to prevent on the sibling surface. **Keep this in step with
    /// `list_tools` or the gate is decorative.**
    ///
    /// Measured against ALL capabilities true: that is the maximal advertised
    /// surface, and the only one that must be guaranteed to fit.
    ///
    /// Returns `(name, description_chars, schema_chars)` per advertised tool.
    fn advertised_surface(server: &CodeScoutServer) -> Vec<(String, usize, usize)> {
        let caps = crate::tools::ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: true,
            has_libraries: true,
            shell_enabled: true,
        };
        server
            .tools
            .iter()
            .filter(|t| t.availability(&caps).is_available(&caps))
            .map(|t| {
                let schema = t.input_schema();
                let mut schema_obj = schema.as_object().cloned().unwrap_or_default();
                if t.pinnable() {
                    CodeScoutServer::inject_workspace_param(&mut schema_obj);
                }
                let schema_chars = Value::Object(schema_obj).to_string().chars().count();
                (
                    t.name().to_string(),
                    t.description().chars().count(),
                    schema_chars,
                )
            })
            .collect()
    }

    /// Characters of authored tool text delivered on **every request of every session**.
    ///
    /// Descriptions were already capped per tool (300 chars, 1800 for the librarian
    /// family) and the surface still reached ~59K characters, because **a per-item cap
    /// does not bound a sum**: growth moved sideways into `input_schema()`, which no
    /// test had ever measured, and N items may each sit at their own limit. So the
    /// budget lives on the payload, where the cost is actually paid.
    ///
    /// The cost is recurring, not one-time. Measured 2026-08-18 across four Claude Code
    /// sessions (three models): **100.0% of input reads are cache hits**, so this block
    /// is re-read on every request for the life of a session — ~5% of a long session's
    /// cached prefix and ~10% of a short one's.
    ///
    /// Characters, not bytes — same reasoning as `CLIENT_INSTRUCTIONS_CHAR_LIMIT`, where
    /// a byte comparison over-counted an em-dash-dense surface and stayed green while
    /// shipping truncated.
    ///
    /// **Do not raise this number. Find the bytes.** When a tool needs a new parameter,
    /// pay for it — trim a param description that duplicates `get_guide`, or drop prose
    /// the schema does not need. Lower it whenever a trim frees room.
    ///
    /// Spec: `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.
    /// Measured 2026-08-18 by `tool_surface_report_lengths` against this harness:
    /// 27 tools, 8,521 description + 47,745 schema. Set as a hard ratchet at that
    /// value — there is deliberately zero headroom.
    ///
    /// It has been paid down twice. Declaring `anchor_heading` on `artifact` cost +808
    /// and breached the then-current 58,572; compressing the injected `workspace`
    /// description (225 chars to 131, times 24 pinnable tools) returned 2,232, and the
    /// constant was ratcheted to the new total rather than left slack. Then hamsa A-27
    /// cut `artifact_augment`'s five per-field restatements of the merge=false rule,
    /// returning 882 (4,436 to 3,554) and ratcheting 57,148 to 56,266. That cut is the
    /// only one here backed by a measured eval rather than by inspection: five arms,
    /// ten runs each, and the schema with ZERO statements of the rule still produced
    /// `merge=true` 10/10 with the preservation cue removed. See
    /// `augment_schema_does_not_restate_the_merge_rule_per_field` before re-adding any
    /// of it.
    ///
    /// Take this number from the report test, never from an external probe. A
    /// scratch probe that re-serialised the payload with Python's `json.dumps`
    /// (`ensure_ascii=True`) read 58,882 — it expands each em-dash into a
    /// six-character ASCII escape, so the over-count tracked prose density and
    /// every per-tool delta came out a multiple of 5. `serde_json` emits UTF-8
    /// directly, as the wire does.
    /// **Raised 2026-08-28, 56_266 → 56_519, deliberately and against this
    /// test's own advice — this is DEBT, not a clean baseline.**
    ///
    /// `memory`'s `force` param (the CM-6 shrink guard) cost ~280 chars and the
    /// surface had ~27 of headroom, so it breached on arrival. The owner chose to
    /// raise now and sweep later rather than block the data-loss fix on a
    /// prose-golf pass across 26 tools. Recorded here because a raised budget is
    /// indistinguishable from an earned one once the reason leaves the room.
    ///
    /// Set to the exact measured total, never rounded up: the ratchet still bites
    /// on the very next added byte, which is the only thing keeping this honest.
    /// The sweep that pays it back should LOWER this line, and any pass that
    /// cannot is a pass that did not happen.
    const TOOL_SURFACE_CHAR_BUDGET: usize = 56_519;

    #[tokio::test]
    async fn tool_surface_under_budget() {
        let (_dir, server) = make_server().await;
        let rows = advertised_surface(&server);
        let total: usize = rows.iter().map(|(_, d, s)| d + s).sum();
        assert!(
            total <= TOOL_SURFACE_CHAR_BUDGET,
            "advertised tool surface is {total} chars across {} tools; budget is {}. \
             Do NOT raise the budget — find the bytes. Run \
             `cargo test --lib tool_surface_report_lengths -- --nocapture` for the \
             per-tool map. See \
             docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md.",
            rows.len(),
            TOOL_SURFACE_CHAR_BUDGET,
        );
    }

    /// Companion to `tool_surface_under_budget` — no assertions, prints the per-tool
    /// map so a breach names *where* the bytes went. A budget that reports only a total
    /// tells an author to give up rather than to choose. Do NOT delete for having no
    /// assertions; observability is its purpose.
    ///
    /// `cargo test --lib tool_surface_report_lengths -- --nocapture`
    #[tokio::test]
    async fn tool_surface_report_lengths() {
        let (_dir, server) = make_server().await;
        let mut rows = advertised_surface(&server);
        rows.sort_by_key(|r| std::cmp::Reverse(r.1 + r.2));

        let desc_total: usize = rows.iter().map(|r| r.1).sum();
        let schema_total: usize = rows.iter().map(|r| r.2).sum();
        let total = desc_total + schema_total;

        println!(
            "\n  {:<22}{:>8}{:>9}{:>9}",
            "tool", "desc", "schema", "total"
        );
        println!("  {}", "-".repeat(48));
        for (name, d, s) in &rows {
            println!("  {:<22}{:>8}{:>9}{:>9}", name, d, s, d + s);
        }
        println!("  {}", "-".repeat(48));
        println!(
            "  {:<22}{:>8}{:>9}{:>9}",
            format!("TOTAL ({} tools)", rows.len()),
            desc_total,
            schema_total,
            total
        );
        println!(
            "  budget {}, headroom {}",
            TOOL_SURFACE_CHAR_BUDGET,
            TOOL_SURFACE_CHAR_BUDGET.saturating_sub(total)
        );
    }

    /// `anchor_heading` shipped implemented and **unadvertised** in `5d5ed457`, so the
    /// one `append_entry` path that structurally cannot produce an uncitable entry was
    /// reachable only by first doing it the fallible way and reading the follow-up
    /// hint. Nothing in the repo could have caught that: `all_tools_have_valid_schemas`
    /// checks `is_object` and `type == "object"` only.
    ///
    /// This pins the instance. The general form — every field the server accepts is
    /// advertised — wants the schema derived from `Args` via `schemars` (already a
    /// dependency), deferred under rule-of-three; see the spec's Revisit-when.
    ///
    /// `docs/issues/archive/2026-08-18-append-entry-body-writer-undeclared-in-artifact-schema.md`
    ///
    /// Gated on `librarian` because the `artifact` tool IS that feature: under
    /// `--no-default-features` — CI's `no-features` and `local-embed` lanes — it is never
    /// registered, and the `expect` below asserts about a tool that does not exist. Failing
    /// there said "artifact tool is registered", which reads as a regression in the
    /// registry rather than as a build config in which the tool is correctly absent.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn artifact_advertises_the_append_entry_section_writer() {
        let (_dir, server) = make_server().await;
        let artifact = server
            .find_tool("artifact")
            .expect("artifact tool is registered");
        let schema = artifact.input_schema();
        let props = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("artifact schema exposes properties");
        for field in ["title", "body", "anchor_heading"] {
            assert!(
                props.contains_key(field),
                "artifact does not advertise `{field}`, but append_entry accepts it \
                 (src/librarian/tools/append_entry.rs). All three are required together \
                 for the server to write the entry section; an agent that cannot see \
                 them can only discover the path by getting it wrong first."
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
    /// **Two-way tripwire (I-01 Phase 3 follow-on):** the test also asserts every
    /// allowlist entry actually appears backticked in at least one surface. When
    /// a section is rewritten or a token disappears from the surfaces, the
    /// allowlist must shrink — otherwise stale entries decay into permanent
    /// false-negatives that mask real drift later. Both directions are required
    /// for the allowlist to remain trustworthy.
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
        use std::collections::{HashMap, HashSet};

        let (_dir, server) = make_server().await;
        let real_tools: HashSet<&str> = server.tools.iter().map(|t| t.name()).collect();

        // Tokens that appear backticked in the surfaces but are not tool names.
        // Grow this list as prompts evolve; the unused-entry tripwire below will
        // tell you when to shrink it. Keep entries sorted.
        let allowlist_entries: &[&str] = &[
            "architecture",
            "conventions",
            "gotchas",
            "hardware",
            "model",
            "model_options",
            "protected_memories",
            "untracked",
            "url",
        ];
        let allowlist: HashSet<&str> = allowlist_entries.iter().copied().collect();
        let mut allowlist_hits: HashMap<&str, usize> = allowlist_entries
            .iter()
            .copied()
            .map(|s| (s, 0usize))
            .collect();

        let draft = crate::prompts::builders::build_system_prompt_draft(&[], &[], None, None, &[]);
        let surfaces: &[(&str, &str)] = &[
            (
                "server_instructions.md",
                crate::prompts::SERVER_INSTRUCTIONS,
            ),
            (
                "onboarding_prompt.md",
                crate::prompts::RAW_ONBOARDING_PROMPT,
            ),
            ("build_system_prompt_draft", draft.as_str()),
        ];

        let re = regex::Regex::new(r"`([a-z][a-z_0-9]{2,})`").unwrap();
        let mut drift = Vec::<String>::new();
        for (surface, body) in surfaces {
            for cap in re.captures_iter(body) {
                let ident = cap.get(1).unwrap().as_str();
                if real_tools.contains(ident) {
                    continue;
                }
                if allowlist.contains(ident) {
                    *allowlist_hits.get_mut(ident).unwrap() += 1;
                    continue;
                }
                drift.push(format!(
                    "{surface}: `{ident}` looks like a tool name but is not \
                     registered — rename the reference to a real tool, or add \
                     it to the allowlist in this test if it's a non-tool token"
                ));
            }
        }

        let mut unused: Vec<&str> = allowlist_hits
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(token, _)| *token)
            .collect();
        unused.sort();

        let mut messages = drift;
        if !unused.is_empty() {
            messages.push(format!(
                "unused allowlist entries (no longer appear backticked in any \
                 surface — remove them from the allowlist): {}",
                unused.join(", ")
            ));
        }

        assert!(
            messages.is_empty(),
            "prompt-surface drift detected:\n  {}",
            messages.join("\n  ")
        );
    }

    /// Companion plugin surfaces (`../claude-plugins/codescout-companion/hooks/*`)
    /// reference codescout tool names as plain text in matcher regexes, case
    /// statements, and message bodies. They drift independently of the codescout
    /// repo — the existing `prompt_surfaces_reference_only_real_tools` test does
    /// not cover them. This catches two kinds of drift:
    ///
    /// 1. **Positive match (matcher shape):** every `mcp__codescout__<name>` token
    ///    in companion hook scripts or `hooks.json` must name a real tool. Catches
    ///    PreToolUse matchers and case-statement filters.
    /// 2. **Stale-name sentinel:** known-removed names (`replace_symbol`,
    ///    `insert_code`, `remove_symbol`, `edit_lines`, `create_or_update_file`)
    ///    must not appear in live (non-comment) code in companion hook files.
    ///    Catches the wider text drift — message bodies that list nonexistent
    ///    tools to the model on SessionStart, BLOCKED notices, etc.
    ///
    /// Filters:
    /// - `*.test.sh` files: skip entirely — those exercise stale names on purpose
    ///   as regression sentinels for matcher coverage.
    /// - Shell comments (`#`-prefixed lines): scrub before stale-name check so
    ///   header comments can document consolidation history ("replace_symbol/
    ///   insert_code/remove_symbol were consolidated into edit_code") without
    ///   tripping the lint. Matcher-shape positive checks still run on full text.
    /// - `non_codescout_tools` allowlist: host-harness tools the companion
    ///   legitimately matches alongside codescout's own (e.g. `activate_project`
    ///   is the host equivalent of codescout's `workspace`).
    ///
    /// Skips gracefully when the sibling repo isn't present (e.g. sandbox builds
    /// of just codescout). Originating evidence: U-6 (text drift) and U-14
    /// (matcher drift causing silent worktree-write-guard failure). H-3 in the
    /// hookify ledger covers the lint extension itself.
    #[tokio::test]
    async fn companion_surfaces_reference_only_real_tools() {
        use std::collections::HashSet;
        use std::path::PathBuf;

        let hooks_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.join("claude-plugins/codescout-companion/hooks"));
        let hooks_dir = match hooks_dir {
            Some(p) if p.is_dir() => p,
            _ => {
                eprintln!(
                    "companion_surfaces_reference_only_real_tools: skipping — \
                 ../claude-plugins/codescout-companion/hooks not present"
                );
                return;
            }
        };

        let (_dir, server) = make_server().await;
        let real_tools: HashSet<&str> = server.tools.iter().map(|t| t.name()).collect();

        let non_codescout_tools: HashSet<&str> = ["activate_project"].into_iter().collect();

        let stale_names = &[
            "replace_symbol",
            "insert_code",
            "remove_symbol",
            "edit_lines",
            "create_or_update_file",
        ];

        let positive_re = regex::Regex::new(r"mcp__codescout__\(?([a-z_|]+)\)?").unwrap();
        let case_re = regex::Regex::new(r"\*__([a-z_]+)").unwrap();

        let mut stale_regexes: Vec<(&str, regex::Regex)> = Vec::new();
        for name in stale_names {
            let re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
            stale_regexes.push((name, re));
        }

        fn scrub_shell_comments(content: &str) -> String {
            content
                .lines()
                .map(|l| {
                    let trimmed = l.trim_start();
                    if trimmed.starts_with('#') {
                        ""
                    } else {
                        l
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        let mut drift = Vec::<String>::new();
        let entries: Vec<_> = std::fs::read_dir(&hooks_dir)
            .unwrap()
            .filter_map(Result::ok)
            .collect();

        for entry in entries {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "sh" | "json") {
                continue;
            }
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            if fname.ends_with(".test.sh") {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Positive matcher-shape check runs on full text (matchers in shebang
            // lines and JSON keys don't live inside shell `#` comments).
            for cap in positive_re.captures_iter(&content) {
                let group = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                for name in group.split('|') {
                    if name.is_empty() || real_tools.contains(name) {
                        continue;
                    }
                    drift.push(format!(
                        "{fname}: matcher references nonexistent tool \
                     `mcp__codescout__{name}` — update to a registered \
                     tool name or remove the alternation branch"
                    ));
                }
            }

            if ext == "sh" {
                for cap in case_re.captures_iter(&content) {
                    let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                    if name.is_empty()
                        || real_tools.contains(name)
                        || non_codescout_tools.contains(name)
                    {
                        continue;
                    }
                    drift.push(format!(
                        "{fname}: case-statement branch `*__{name}` cannot \
                     fire — no codescout tool named `{name}` is registered \
                     (add to non_codescout_tools allowlist if it is a \
                     host-harness tool)"
                    ));
                }
            }

            // Stale-name sentinel runs on comment-scrubbed text so header
            // documentation explaining consolidation history doesn't false-trip.
            let scrubbed = if ext == "sh" {
                scrub_shell_comments(&content)
            } else {
                content.clone()
            };
            for (stale, re) in &stale_regexes {
                if re.is_match(&scrubbed) {
                    drift.push(format!(
                        "{fname}: contains stale tool name `{stale}` in live \
                     (non-comment) code — replace with the live equivalent \
                     (e.g. `edit_code` consolidated \
                     replace_symbol/insert_code/remove_symbol)"
                    ));
                }
            }
        }

        assert!(
            drift.is_empty(),
            "companion-surface drift detected:\n  {}",
            drift.join("\n  ")
        );
    }

    #[tokio::test]
    async fn static_doc_sources_all_readable() {
        use crate::mcp_resources::{doc::DocProvider, ResourceProvider};
        let sources = super::static_doc_sources();
        assert!(
            !sources.is_empty(),
            "static_doc_sources() should register at least one doc URI"
        );
        let provider = DocProvider::new(sources.clone());
        for src in &sources {
            let res = provider.read(&src.uri).await;
            assert!(
                res.is_ok(),
                "doc:// URI {} failed to read: {:?}",
                src.uri,
                res.err()
            );
            assert!(
                !src.content.is_empty(),
                "doc:// URI {} embedded content is empty",
                src.uri
            );
        }
    }

    #[tokio::test]
    async fn every_tool_description_under_cap() {
        const CAP: usize = 1800;
        let (_dir, server) = make_server().await;
        let over: Vec<(String, usize)> = server
            .tools
            .iter()
            .map(|t| (t.name().to_string(), t.description().len()))
            .filter(|(_, n)| *n > CAP)
            .collect();
        assert!(
            over.is_empty(),
            "tool descriptions over the {CAP}-char cap: {:?}",
            over
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

    /// Phase 5: pinnable tools must advertise the optional `workspace` pin in
    /// their list_tools schema; session/global/librarian tools must not.
    #[tokio::test]
    async fn pinnable_tools_advertise_workspace_param() {
        let (_dir, server) = make_server().await;
        let (mut saw_pinnable, mut saw_unpinnable) = (false, false);
        for tool in &server.tools {
            if tool.pinnable() {
                let mut schema_obj = tool.input_schema().as_object().cloned().unwrap_or_default();
                CodeScoutServer::inject_workspace_param(&mut schema_obj);
                let has_ws = schema_obj
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .is_some_and(|p| p.contains_key("workspace"));
                assert!(
                    has_ws,
                    "pinnable tool '{}' must advertise the `workspace` param",
                    tool.name()
                );
                saw_pinnable = true;
            } else {
                saw_unpinnable = true;
            }
        }
        assert!(
            saw_pinnable && saw_unpinnable,
            "partition must be non-trivial"
        );

        let pinnable: std::collections::HashSet<&str> = server
            .tools
            .iter()
            .filter(|t| t.pinnable())
            .map(|t| t.name())
            .collect();
        for n in ["read_file", "edit_file", "memory", "grep"] {
            assert!(pinnable.contains(n), "{n} must be pinnable");
        }
        // `artifact` / `librarian` are only registered with the feature on, so
        // asserting them unconditionally fails the `--no-default-features` and
        // `--features local-embed` configs.
        #[cfg(feature = "librarian")]
        for n in ["artifact", "librarian"] {
            assert!(pinnable.contains(n), "{n} must be pinnable");
        }
        for n in ["workspace", "get_guide"] {
            assert!(!pinnable.contains(n), "{n} must NOT be pinnable");
        }
    }

    /// The injected `workspace` param is optional (never added to `required`),
    /// preserves existing properties, and is idempotent.
    #[test]
    fn inject_workspace_param_is_optional_and_idempotent() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"]
        })
        .as_object()
        .unwrap()
        .clone();
        CodeScoutServer::inject_workspace_param(&mut schema);
        let props = schema["properties"].as_object().unwrap();
        assert!(
            props.contains_key("workspace"),
            "workspace must be injected"
        );
        assert!(
            props.contains_key("path"),
            "existing properties must be preserved"
        );
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(
            !required.contains(&"workspace"),
            "workspace must stay optional"
        );
        // Idempotent: a second injection neither duplicates nor panics.
        CodeScoutServer::inject_workspace_param(&mut schema);
        assert_eq!(schema["properties"].as_object().unwrap().len(), 2);
    }

    /// Forward guard: pins the PRESENCE of the routing clause, because this string is
    /// the single most attractive compression target on the entire surface and the
    /// analysis that finds it cannot see why it must stay.
    ///
    /// This one description is injected into all 24 pinnable tools, so it costs 179
    /// chars x 24 = 4,296 on the wire — 7.6% of the surface for one sentence, and the
    /// only 24x multiplier there is. An n-gram redundancy sweep ranks it FIRST. hamsa
    /// A-28 nearly cut its last clause on exactly that reasoning and measured it
    /// instead:
    ///
    /// | arm | description | passed |
    /// |---|---|---|
    /// | base | 132 chars, all three claims | **10/10** |
    /// | treatment | 53 chars, routing clause dropped | 8/10 |
    /// | control-null | description removed, knob kept | 9/10 |
    /// | control-positive | + mandatory directive forbidding the pin | **0/10** |
    ///
    /// Every failure in the two cut arms was the SAME one, and none occurred in base:
    /// the model reached for `workspace(action="activate", ...)`, which is GLOBAL and
    /// clobbers a concurrently-working parent session — the exact condition the
    /// per-call pin exists for. So `For concurrent subagents in different workspaces`
    /// does not DESCRIBE the parameter, it DISPLACES a competing prior.
    ///
    /// The counts are small (10/10 vs 8/10 is Fisher p~0.47) and the disposition does
    /// not rest on them. P-4 puts the burden on the DELETION to show it does not
    /// regress; 8/10 against 10/10 does not discharge it. A cut that cannot prove
    /// safety does not ship — it need not be proven harmful.
    ///
    /// Contrast `augment_schema_does_not_restate_the_merge_rule_per_field`, whose cut
    /// DID ship: prose restating what a parameter NAME already implies is cargo cult
    /// (A-27, 882 chars, 20/20 with zero statements and no cue). The discriminator is
    /// not redundancy but function — does the sentence DESCRIBE the parameter, or
    /// DISPLACE something the model would otherwise reach for?
    ///
    /// Shortening this string needs a NEW base arm (P-3), not a byte budget.
    ///
    /// Ledger: `docs/trackers/prompt-hamsa-audit-log.md` A-28.
    /// Scenario: `prompt-engineering/scenarios/workspace-pin-routing/`.
    #[test]
    fn injected_workspace_description_keeps_the_routing_clause() {
        let mut schema = serde_json::Map::new();
        CodeScoutServer::inject_workspace_param(&mut schema);
        let desc = schema["properties"]["workspace"]["description"]
            .as_str()
            .expect("injected workspace param must carry a description");

        assert!(
            desc.contains("concurrent subagents in different workspaces"),
            "the routing clause is gone. A-28 measured its removal: 10/10 -> 8/10, and \
             every failure was the model reaching for the globally-scoped \
             workspace(action=\"activate\") instead of the per-call pin. It displaces a \
             competing prior rather than describing the parameter, so it does not come \
             out for bytes — re-cutting needs a new base arm. Got: {desc:?}"
        );
        assert!(
            desc.contains("omit for the active project"),
            "the default-behaviour clause is gone; A-28 tested dropping the ROUTING \
             clause only, so cutting this one is a separate intervention needing its \
             own arm. Got: {desc:?}"
        );
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

    /// Every observed call shape that routes to a topic which has opted into
    /// section-grain delivery must be served by some declared section, or
    /// waived with a reason. Gate 2. Replaces
    /// `every_guide_topic_is_triggered_or_declared_pull_only`, whose
    /// triggered-or-pull-only check is now subsumed: a call shape that reaches
    /// a declaring topic but no section is the section-grain form of the same
    /// defect that gate caught at whole-topic grain.
    ///
    /// Finite because call shapes are: 88 distinct rows in
    /// `src/prompts/shape_census.txt`, generated from real transcripts across
    /// every profile on this machine (script in
    /// `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` Task 9).
    ///
    /// Scoped to topics that have opted into section grain via
    /// `GUIDE_INDEX.declares(topic)` — only `librarian` today — so it is
    /// meaningful in Phase 1 and widens automatically as later phases land
    /// declarations on other topics.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn every_observed_shape_of_a_declaring_topic_has_a_section() {
        // Gate 2. Finite because call shapes are: 88 distinct across 170,465 observed
        // calls. Scoped to topics that have opted into section grain, so it is
        // meaningful in Phase 1 and widens automatically as Phases 2-3 land.
        use crate::prompts::guide_index::GUIDE_INDEX;
        let census = include_str!("prompts/shape_census.txt");
        let (_dir, server) = make_server().await;

        // Each probe stands for a RESULT SHAPE a real call returns, because
        // `relevant_guide_topic` reads the result's content to pick the topic. The
        // doctor-shaped probe is not decoration: `names_tracker_path` scans `path`
        // inside a `violations` array, and a real doctor scan names tracker and bug
        // files (measured 2026-08-31: 128 of 138), so without it the content branch
        // is unreachable from this test and every `serves: librarian.doctor`
        // declaration reads as covered while never being delivered.
        // docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md
        let probes = [
            serde_json::json!({}),
            serde_json::json!({"abs_path": "docs/issues/x.md"}),
            serde_json::json!({"abs_path": "docs/trackers/x.md"}),
            serde_json::json!({"violations": [{"path": "docs/trackers/x.md"}]}),
        ];

        for line in census
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        {
            let shape = line.split_whitespace().next().unwrap();
            let tool_name = shape.split('.').next().unwrap();
            let Some(tool) = server.tools.iter().find(|t| t.name() == tool_name) else {
                continue;
            };
            // Pair each probe with the topic IT routes to, rather than fixing one
            // topic for all of them. `find_map` took the first `Some`, and the empty
            // probe yields `Some("librarian")` unconditionally — so coverage was only
            // ever evaluated against `librarian`, and a shape whose real results route
            // elsewhere was never checked against the topic it actually reaches.
            // That is the runtime relationship this gate exists to model.
            for probe in &probes {
                let Some(topic) = tool.relevant_guide_topic(probe) else {
                    continue;
                };
                if !GUIDE_INDEX.declares(topic) {
                    continue;
                }
                let covered = !GUIDE_INDEX
                    .match_sections(topic, Some(shape), probe)
                    .is_empty();
                let waived = crate::prompts::SECTION_WAIVERS
                    .iter()
                    .any(|(t, _, r)| *t == topic && r.contains(shape));
                assert!(
                    covered || waived,
                    "call shape `{shape}` routes to declaring topic `{topic}` on a result \
                 shaped like {probe}, but no section there serves it. Add a `serves:` \
                 declaration in that topic, or a SECTION_WAIVERS entry naming the shape \
                 and saying why. An undeclared shape gets only the preamble."
                );
            }
        }
    }

    /// Every DECLARED shape must name a real tool, and a real action on that tool.
    /// Gate 6.
    ///
    /// Gate 2 above runs the opposite direction — observed shape → some section serves
    /// it — so until now nothing checked that a declaration is reachable *at all*. A
    /// shape naming a nonexistent action passes Gate 1 (`parse_shape` validates only
    /// that both halves are identifiers), passes Gate 2 (which iterates the census and
    /// never the declarations), and passes Gate 5 (which asserts `serves` is non-empty,
    /// not that it is live). The section then silently stops being delivered — the exact
    /// failure section-grain delivery exists to prevent, one level up. This is
    /// `observer-blindness:OB-7`'s shape: a declaration that is well-formed and that
    /// nothing in production reaches.
    ///
    /// The oracle is the tool REGISTRY plus each tool's `input_schema()` action enum,
    /// and deliberately **not** `shape_census.txt`. The census records what has been
    /// *called*, so it rejects every genuinely new action: measured 2026-09-01,
    /// `librarian.audit_log` is declared, real, and absent from the census. A
    /// census-based gate would have demanded a waiver for a correct declaration, which
    /// trains authors to waive rather than to fix.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn every_declared_shape_names_a_live_tool_and_action() {
        use crate::prompts::guide_index::GUIDE_INDEX;
        let (_dir, server) = make_server().await;

        let mut checked = 0usize;
        for topic in crate::prompts::GUIDE_TOPICS {
            let Some(entry) = GUIDE_INDEX.topic(topic) else {
                continue;
            };
            for sec in &entry.sections {
                for shape in &sec.serves {
                    let tool = server
                        .tools
                        .iter()
                        .find(|t| t.name() == shape.tool)
                        .unwrap_or_else(|| {
                            panic!(
                                "{topic} § {} declares `serves: {}`, but no registered tool \
                                 is named `{}`. A shape naming an unregistered tool can \
                                 never match, so that section is undeliverable.",
                                sec.heading, shape.tool, shape.tool
                            )
                        });

                    if let Some(action) = &shape.action {
                        let schema = tool.input_schema();
                        let allowed: Vec<String> = schema
                            .get("properties")
                            .and_then(|p| p.get("action"))
                            .and_then(|a| a.get("enum"))
                            .and_then(|e| e.as_array())
                            .map(|v| {
                                v.iter()
                                    .filter_map(|x| x.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default();
                        assert!(
                            !allowed.is_empty(),
                            "{topic} § {} declares `serves: {}.{action}`, but tool `{}` \
                             exposes no `action` enum in its input_schema. Either the shape \
                             should be tool-only (`serves: {}`), or the schema moved.",
                            sec.heading,
                            shape.tool,
                            shape.tool,
                            shape.tool
                        );
                        assert!(
                            allowed.iter().any(|a| a == action),
                            "{topic} § {} declares `serves: {}.{action}`, which is not one \
                             of `{}`'s real actions ({}). It parses, clears every other \
                             gate, and is delivered to nothing.",
                            sec.heading,
                            shape.tool,
                            shape.tool,
                            allowed.join(", ")
                        );
                    }
                    checked += 1;
                }
            }
        }

        // Anti-vacuity floor, not a total: the loop above is silent on an empty corpus,
        // and a gate that passes by scanning nothing is the defect it is here to catch.
        // 24 distinct shapes were declared when this landed and declarations only ever
        // grow, so this is a floor that never needs revising upward — deliberately not
        // an equality, which would turn every new declaration into a failing gate.
        assert!(
            checked >= 24,
            "the gate scanned only {checked} declared shapes — it is reading an empty or \
             truncated corpus and would otherwise pass vacuously"
        );
    }
    /// Every registered tool must supply a selector key. This is the routing
    /// PRECONDITION, and it is the direction no other gate runs in.
    ///
    /// `call_content` gates operator-rule routing on `if selector.is_some()`
    /// (`src/tools/core/types.rs`), so a tool returning `None` is unreachable by every
    /// `triggered` rule — permanently, and with no observable symptom: no error, no
    /// empty result, nothing in a log. The rule simply never fires.
    ///
    /// **Measured 2026-09-01, before `Tool::selector_key`'s default was inverted: 17 of
    /// the 21 registered tools returned `None`, and the entire routing suite was green.**
    /// It was green because the only caller exercising the path was `RoutedEchoTool`, a
    /// `#[cfg(test)]` stub *named* `"memory"` that projected the key the real tool did
    /// not — a green suite and a dead feature, consistent with each other for as long as
    /// the stub was the only caller. See
    /// `docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md`.
    ///
    /// This gate is what makes opting out *visible*. The default now opts every tool in,
    /// so an override returning `None` is the only way to leave the set, and it reds here
    /// naming the tool that left.
    ///
    /// **A selector is necessary, NOT sufficient — do not read a green here as "routing
    /// works".** `OP-4` names `edit_file` and `create_file`, both of which have supplied
    /// selectors since `2447f709`, and it still cannot fire: its `path~` predicate is
    /// matched against the RESPONSE, which carries no path. That defect is open at
    /// `docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md`. This annotation is
    /// the reason the two tools' own `selector_key` overrides could be deleted — it is
    /// where their doc comments carried that caveat, and this is the site where someone
    /// is most likely to mistake a supplied selector for a working route.
    ///
    /// The positive end-to-end proof, for the one rule that does route, is
    /// `crate::tools::memory::tests::a_real_memory_write_call_delivers_op_3`.
    #[tokio::test]
    async fn every_registered_tool_supplies_a_selector_key() {
        let (_dir, server) = make_server().await;

        let mut checked = 0usize;
        for tool in &server.tools {
            let name = tool.name().to_string();

            // Action-less shape: the key must be the bare tool name, never `None`.
            // `Shape::matches` reads `None` as "cannot match", so a tool-only rule or
            // section declaration would be permanently unmatchable.
            let bare = tool.selector_key(&serde_json::json!({}));
            assert_eq!(
                bare.as_deref(),
                Some(name.as_str()),
                "tool `{name}` returned {bare:?} for an action-less call. It is therefore \
                 unreachable by every triggered operator rule and by section-grain guide \
                 matching. If this tool genuinely must opt out, say why at the override \
                 and update this gate deliberately — do not silence it."
            );

            // A tool taking an `action` must carry it into the key, or two actions on one
            // tool are indistinguishable to a rule that means only one of them. The
            // oracle is the tool's own `input_schema` action enum, so a new action needs
            // no edit here.
            let actions: Vec<String> = tool
                .input_schema()
                .get("properties")
                .and_then(|p| p.get("action"))
                .and_then(|a| a.get("enum"))
                .and_then(|e| e.as_array())
                .map(|v| {
                    v.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            for action in &actions {
                let key = tool.selector_key(&serde_json::json!({"action": action}));
                let want = format!("{name}.{action}");
                assert_eq!(
                    key.as_deref(),
                    Some(want.as_str()),
                    "tool `{name}` projected {key:?} for action `{action}`, not `{want}`. \
                     A rule serving one action of this tool would match the wrong calls, \
                     or none."
                );
                checked += 1;
            }
            checked += 1;
        }

        // Anti-vacuity floor, not a total: the loop is silent on an empty registry, which
        // is the state this gate is least useful in and most likely to be read as green.
        // 21 tools are registered unconditionally (the librarian family and `PeerTool` are
        // feature- and runtime-conditional and only add to this), and `checked` also
        // counts one per action, so this floor holds in both the lean and default lanes
        // and never needs revising upward. Deliberately not an equality — that would red
        // on every tool or action added.
        assert!(
            checked >= 21,
            "the gate scanned only {checked} tool shapes — it is reading an empty or \
             truncated registry and would otherwise pass vacuously"
        );
    }

    /// A declaring topic must have at least one LIVE route from a real call to at least
    /// one of its declared sections. Gate 7.
    ///
    /// Gate 6 checks that a declared shape names a real tool and a real action. This
    /// checks the step after it: that some call which actually ROUTES to this topic can
    /// produce a selector key one of its shapes matches. Different failures, and this is
    /// the dangerous direction — `Shape::matches` returns `false` when `selector_key` is
    /// `None`, deliberately ("do not turn it into a wildcard"). So declaring sections on
    /// a topic whose triggering tools all opt out of `selector_key` does not merely fail
    /// to improve delivery: it REPLACES whole-topic delivery with the preamble alone,
    /// while Gates 1–6 all stay green.
    ///
    /// Measured 2026-09-01 while scoping the T-15a annotation pass, which this gate then
    /// refuted. Of the nine topics, only `librarian` and `tracker-conventions` are
    /// reachable this way. Every tool routing to `progressive-disclosure` (nine of them),
    /// `symbol-navigation` (three) and `workspace-state` (one) returns `None`, and
    /// `project-activation-bootstrap` has no `relevant_guide_topic` at all — it is the
    /// session-opening special case. Annotating any of them would have been a silent
    /// content regression, and that pass was already scoped, queued and approved.
    ///
    /// The inputs are derived from each tool's own `input_schema()` action enum — the
    /// same oracle Gate 6 uses — rather than hand-listed, so a new action is covered
    /// without editing this test.
    ///
    /// **Two caveats a reader of this gate needs, both from peer sessions on 2026-09-01.**
    ///
    /// Its live population is **one file**: `grep -l 'serves:' src/prompts/guides/*.md`
    /// returns only `librarian.md`. So the configuration this gate refuses is currently
    /// unreachable on eight of the nine topics — cheap to hold, and *thinly exercised*. A
    /// bug in the declaring path has almost nothing testing it, and a second declaring
    /// member is a deliberate act rather than something the corpus will supply. The
    /// `verified >= 1` floor below is honest about that; it is not a claim of coverage.
    ///
    /// And the reason this gate is a separate one rather than a stronger Gate 6:
    /// `reconnaissance-patterns:R-159` — *verifying a MECHANISM is not verifying its
    /// REACHABILITY, and a clean mechanism check feels like the strong form.* Gate 6
    /// establishes that a declared shape is well-formed against the registry, which reads
    /// as the thorough answer precisely because it is exact. It says nothing about whether
    /// any call arrives. That is this gate's whole subject.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn every_declaring_topic_has_a_live_route_to_a_declared_section() {
        use crate::prompts::guide_index::GUIDE_INDEX;
        let (_dir, server) = make_server().await;

        // Result shapes, not inputs: `relevant_guide_topic` reads the RESULT to pick a
        // topic. Mirrors Gate 2's probe set, including the doctor-shaped one.
        //
        // The `output_id` / `overflow` probe is deliberately INERT today, and says so here
        // rather than reading as coverage. `symbols`, `references` and `call_graph` branch
        // on those keys to route overflowing results to `progressive-disclosure` — but this
        // gate only evaluates topics that DECLARE, which is `librarian` alone, and the
        // librarian adapter's router reads `abs_path` / `violations` and never looks at
        // either key. So removing this row changes no outcome now; it is here for the case
        // where a topic reached via the overflow branch first declares a section, which is
        // exactly when a missing probe would make this gate pass without having looked.
        // Keep it, and do not cite it as evidence that overflow routing is covered.
        let result_probes = [
            serde_json::json!({}),
            serde_json::json!({"output_id": "@x", "overflow": true}),
            serde_json::json!({"abs_path": "docs/issues/x.md"}),
            serde_json::json!({"abs_path": "docs/trackers/x.md"}),
            serde_json::json!({"violations": [{"path": "docs/trackers/x.md"}]}),
        ];

        let mut verified = 0usize;
        for topic in crate::prompts::GUIDE_TOPICS {
            if !GUIDE_INDEX.declares(topic) {
                continue;
            }
            let mut route: Option<String> = None;
            'search: for tool in &server.tools {
                let mut inputs = vec![serde_json::json!({})];
                if let Some(actions) = tool
                    .input_schema()
                    .get("properties")
                    .and_then(|p| p.get("action"))
                    .and_then(|a| a.get("enum"))
                    .and_then(|e| e.as_array())
                {
                    for a in actions.iter().filter_map(|v| v.as_str()) {
                        inputs.push(serde_json::json!({"action": a}));
                    }
                }
                for result in &result_probes {
                    if tool.relevant_guide_topic(result) != Some(*topic) {
                        continue;
                    }
                    for input in &inputs {
                        let Some(sel) = tool.selector_key(input) else {
                            continue;
                        };
                        if !GUIDE_INDEX
                            .match_sections(topic, Some(&sel), result)
                            .is_empty()
                        {
                            route = Some(format!("{} -> {sel}", tool.name()));
                            break 'search;
                        }
                    }
                }
            }
            assert!(
                route.is_some(),
                "`{topic}` declares `serves:` sections, but no registered tool both routes \
                 to it AND returns a selector_key matching any of them. `Shape::matches` \
                 treats `selector_key == None` as no-match by design, so these declarations \
                 do not add delivery — they REPLACE whole-topic delivery with the preamble \
                 alone. Either implement `selector_key` on a tool that routes here (see \
                 `crate::tools::core::types::action_selector_key`), or remove the \
                 declarations. Do not waive this one: a waiver would record the regression \
                 as intentional."
            );
            verified += 1;
        }

        // Anti-vacuity: the loop is silent if nothing declares, which is the state this
        // gate is least useful in and most likely to be mistaken for green. `librarian`
        // declares today, so a floor of one is the honest assertion — deliberately not a
        // count of declaring topics, which grows.
        assert!(
            verified >= 1,
            "no topic declares any section, so this gate verified nothing — it is reading \
             an empty corpus rather than passing"
        );
    }

    /// Every registered guide topic must either fire from some tool, or be declared
    /// pull-only with a reason — and not both. This is the triggered-xor-pull-only half
    /// of the deleted `every_guide_topic_is_triggered_or_declared_pull_only` (see Gate 2
    /// above for the section-grain half, and
    /// `pull_only_guide_topics_are_registered_with_real_reasons` in `src/prompts/mod.rs`
    /// for the membership/reason-length half). Restored rather than dropped: Gate 2 is
    /// scoped to declaring topics only (`librarian` today), so it says nothing about
    /// whether the other nine topics still have a live trigger or a stale pull-only entry
    /// — that recurrence gate from
    /// `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`
    /// still applies to them and needs a running `Server` to probe `relevant_guide_topic`,
    /// which is why it lives here rather than next to the reason-length checks.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn every_guide_topic_is_triggered_xor_declared_pull_only() {
        use crate::prompts::{GUIDE_TOPICS, PULL_ONLY_GUIDE_TOPICS};

        let (_dir, server) = make_server().await;

        let probes = [
            serde_json::json!({}),
            serde_json::json!({"overflow": {"shown": 1, "total": 2}}),
            serde_json::json!({"abs_path": "docs/trackers/x.md"}),
            serde_json::json!({"abs_path": "docs/issues/x.md"}),
            serde_json::json!({"abs_path": "src/main.rs"}),
        ];
        let mut triggered: std::collections::BTreeSet<&str> =
            [crate::prompts::SESSION_OPENING_GUIDE]
                .into_iter()
                .collect();
        for tool in &server.tools {
            for probe in &probes {
                if let Some(topic) = tool.relevant_guide_topic(probe) {
                    triggered.insert(topic);
                }
            }
        }

        for topic in GUIDE_TOPICS {
            let declared = PULL_ONLY_GUIDE_TOPICS.iter().any(|(t, _)| t == topic);
            assert!(
                triggered.contains(topic) || declared,
                "guide topic `{topic}` has no `relevant_guide_topic()` trigger and is not \
                 declared pull-only. Authoring a guide nothing fires is the same as \
                 deleting it: either wire a trigger, or add it to \
                 prompts::PULL_ONLY_GUIDE_TOPICS with the reason."
            );
            assert!(
                !(triggered.contains(topic) && declared),
                "guide topic `{topic}` is declared pull-only but IS triggered — remove the \
                 stale entry from prompts::PULL_ONLY_GUIDE_TOPICS, or the list stops \
                 describing reality."
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
        let (_dir, server) = make_server_no_project().await;
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
        // Shell is allowed by default: shell_command_mode defaults to "warn"
        // and check_tool_access no longer gates run_command.
        assert_eq!(security.shell_command_mode, "warn");
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
            "use tree to explore",
        ));
        let result = route_tool_error(err);
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["hint"], "use tree to explore");
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
    fn lsp_content_modified_routes_to_recoverable_not_fatal() {
        // Code -32801 (ContentModified) — rust-analyzer cancels in-flight
        // requests when its analysis snapshot advances (typical during
        // post-restart indexer warmup). Must NOT produce isError:true,
        // same as -32800 RequestCancelled.
        let err = anyhow::anyhow!("LSP error (code -32801): content modified");
        let result = route_tool_error(err);
        assert!(
            result.is_error != Some(true),
            "LSP ContentModified must not set isError:true"
        );
        let text = &result.content[0].as_text().unwrap().text;
        let body: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(body.get("hint").is_some(), "must include retry hint");
        assert!(
            body["hint"].as_str().unwrap().contains("-32801")
                || body["hint"].as_str().unwrap().contains("ContentModified"),
            "hint should mention -32801 / ContentModified, got: {}",
            body["hint"]
        );
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
        assert!(tool_skips_server_timeout("index"));
        assert!(tool_skips_server_timeout("index_library"));
    }

    #[test]
    fn other_tools_do_not_skip_server_timeout() {
        for name in &["read_file", "edit_file", "symbols", "semantic_search"] {
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
        // Canonicalize like `Agent::new` does (src/agent/mod.rs:389) before any
        // path is rendered. On macOS `tempfile::tempdir()` returns a `/var/...`
        // path while production renders the canonicalized `/private/var/...`
        // form, so a needle built from the raw tempdir path can never match —
        // making `!text.contains(&root)` vacuously true. See the `canonical`
        // helper documented at src/agent/mod.rs:1775-1777.
        let root = std::fs::canonicalize(dir.path())
            .unwrap()
            .to_string_lossy()
            .to_string();

        let req = CallToolRequestParams::new("tree")
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
            "tree returned empty output — the strip test is not actually exercising anything"
        );
        assert!(
            !text.contains(&root),
            "Expected absolute root to be stripped, but found it in output:\n{text}"
        );
    }
    /// Regression (2026-07-18-tree-strip-bare-root-not-stripped): when a
    /// listing's sole common prefix IS the project root, `format_list_dir`
    /// renders that prefix WITHOUT a trailing slash. The old text strip needed
    /// a dedicated bare-root branch to catch that shape; field-aware stripping
    /// does not, because `entries` is allowlisted, so `common_path_prefix` runs
    /// over already-relative names and `dir_display` collapses to ".". This
    /// test guards the rendered outcome, which is unchanged — `make_server`'s
    /// tempdir needs a visible top-level entry or the listing short-circuits to
    /// "(empty directory)" and exercises nothing.
    #[tokio::test]
    async fn call_tool_strips_bare_project_root_from_list_dir_output() {
        let (dir, server) = make_server().await;
        // Canonicalize like `Agent::new` does (src/agent/mod.rs:389) — see the
        // sibling test above for why a needle built from the raw tempdir path
        // is vacuous on macOS.
        let root = std::fs::canonicalize(dir.path())
            .unwrap()
            .to_string_lossy()
            .to_string();
        std::fs::write(dir.path().join("visible.txt"), "hello").unwrap();

        let req = CallToolRequestParams::new("tree")
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
            text.contains("visible.txt"),
            "listing must show the visible entry — the bare-root branch is not \
             actually being exercised otherwise; got:\n{text}"
        );
        assert!(
            !text.contains(&root),
            "Expected absolute root to be stripped even in its bare (no trailing \
             slash) form, but found it in output:\n{text}"
        );
    }

    #[tokio::test]
    async fn no_absolute_project_paths_in_rendered_output() {
        // The PATH_KEYS allowlist in src/tools/core/path_strip.rs is a co-change
        // contract. This gate is what enforces it: a tool emitting paths under a
        // key nobody added fails here instead of silently costing tokens forever.
        //
        // Liveness guard: four of these five cases are vacuous *today* — `read_file`
        // and `read_markdown` never echo a path key at all, `symbols` pre-relativizes
        // inside the tool, and `tree` is masked by its own `common_path_prefix`. Only
        // `grep` can actually fail the negative assertion below right now. That is
        // fine — this gate is a forward-looking canary for keys added later — but a
        // canary with only a negative assertion cannot report its own death: if a
        // case stops reaching its tool (error envelope, empty text, wrong branch),
        // `!joined.contains(&needle)` passes silently and the gate advertises safety
        // it no longer provides. Each case therefore also asserts a positive token
        // that only appears if the tool actually ran and rendered real content.
        let (dir, server) = make_server().await;
        // Canonicalize before building the needle, matching `Agent::new`
        // (src/agent/mod.rs:389) — production always renders paths derived
        // from the canonicalized root, so the needle must be built the same
        // way or this gate is structurally inert on macOS (tempdir() yields
        // `/var/...` there while production renders `/private/var/...`; see
        // src/agent/mod.rs:1772-1777 and the `canonical` test helper).
        let root_fwd = to_forward_slash(&std::fs::canonicalize(dir.path()).unwrap());
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        std::fs::write(dir.path().join("notes.md"), "# Notes\n\nbody\n").unwrap();

        let cases: Vec<(&str, serde_json::Value, &str)> = vec![
            ("tree", serde_json::json!({ "path": "." }), "notes.md"),
            ("grep", serde_json::json!({ "pattern": "pub fn" }), "pub fn"),
            (
                "read_file",
                serde_json::json!({ "path": "src/lib.rs" }),
                "pub fn a",
            ),
            (
                "read_markdown",
                serde_json::json!({ "path": "notes.md" }),
                "Notes",
            ),
            (
                "symbols",
                serde_json::json!({ "path": "src/lib.rs" }),
                "Function",
            ),
        ];

        let needle = format!("{root_fwd}/");
        for (tool, input, expect_token) in cases {
            let req = CallToolRequestParams::new(tool)
                .with_arguments(serde_json::from_value(input).unwrap());
            let result = server
                .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
                .await
                .unwrap();
            let joined: String = result
                .content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                joined.contains(expect_token),
                "tool `{tool}` did not render its expected content (`{expect_token}`) — \
                 this case has stopped exercising the tool (error envelope, empty \
                 output, or wrong branch), so the absence check below would pass \
                 silently and the gate would advertise safety it no longer provides. \
                 Output:\n{joined}"
            );
            assert!(
                !joined.contains(&needle),
                "tool `{tool}` leaked an absolute project path. Either its path key \
                 is missing from PATH_KEYS in src/tools/core/path_strip.rs, or it \
                 introduced a new one. Output:\n{joined}"
            );
        }
    }

    #[tokio::test]
    async fn read_file_and_grep_show_a_path_literal_in_content_verbatim() {
        let (dir, server) = make_server().await;
        // Canonicalize before building the needle-adjacent literal — see
        // `no_absolute_project_paths_in_rendered_output` above for why the
        // raw tempdir path is the wrong thing to render from (macOS
        // /var/... vs production's canonicalized /private/var/...).
        let root_fwd = to_forward_slash(&std::fs::canonicalize(dir.path()).unwrap());
        let literal = format!("REPO = \"{root_fwd}/.worktrees/single-stage\"");
        std::fs::write(dir.path().join("probe.txt"), format!("{literal}\n")).unwrap();

        for (tool, input) in [
            ("read_file", serde_json::json!({ "path": "probe.txt" })),
            (
                "grep",
                serde_json::json!({ "pattern": "REPO", "path": "probe.txt" }),
            ),
        ] {
            let req = CallToolRequestParams::new(tool)
                .with_arguments(serde_json::from_value(input).unwrap());
            let result = server
                .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
                .await
                .unwrap();
            let joined: String = result
                .content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                joined.contains(&literal),
                "`{tool}` must show the file's path literal verbatim — an edit keyed \
                 on this text has to match the bytes on disk. Got:\n{joined}"
            );
        }
    }

    #[tokio::test]
    async fn call_tool_inner_honors_workspace_override_for_security_config() {
        // BUG (sibling of the edit_code write-path pin bug): check_tool_access
        // ran BEFORE ctx.workspace_override was even extracted from the input,
        // so it always gated against the session-default project's security
        // config. Workspace A disables writes (file_write_enabled = false);
        // the session-default project B (from make_server) allows them.
        // Pinning a write-tool call to A must be rejected using A's config —
        // proving the pin is honored before the access check runs, not after.
        let dir_a = tempdir().unwrap();
        let (_dir_b, server) = make_server().await;
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::write(
            dir_a.path().join(".codescout").join("project.toml"),
            "[project]\nname = \"pin-test-a\"\n\n[security]\nfile_write_enabled = false\n",
        )
        .unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        let req = CallToolRequestParams::new("create_file").with_arguments(
            serde_json::from_value(serde_json::json!({
                "path": "new_file.txt",
                "content": "hello",
                "workspace": root_a.to_string_lossy(),
            }))
            .unwrap(),
        );
        let result = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();

        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("")
            .to_string();
        assert_eq!(
            result.is_error,
            Some(true),
            "write call pinned to workspace A (file_write_enabled=false) must be \
             rejected using A's config, not B's (session-default, writes enabled); got: {text}"
        );
        assert!(
            !dir_a.path().join("new_file.txt").exists(),
            "file must not have been created in pinned workspace A"
        );
    }

    #[tokio::test]
    async fn post_process_annotates_against_the_pinned_root_without_mutating_text() {
        // post_process no longer strips — stripping moved to Tool::call_content.
        // What survives here is the banner, which must still name the PINNED
        // root rather than the session default.
        let dir_a = tempdir().unwrap();
        let (_dir_b, server) = make_server().await;
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();
        let root_a_fwd = to_forward_slash(&root_a);

        let literal = format!("REPO = \"{root_a_fwd}/.worktrees/x\"");
        let payload = CallToolResult::success(vec![Content::text(literal.clone())]);

        let processed = server
            .post_process(payload, "read_file", Some(&root_a))
            .await;
        let joined: String = processed
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            joined.contains(&literal),
            "post_process must not mutate result text at all; got: {joined}"
        );
        assert!(
            joined.contains(&format!("paths are relative to {root_a_fwd}")),
            "the banner must name the PINNED root A; got: {joined}"
        );
    }

    #[tokio::test]
    async fn call_tool_inner_grants_write_access_to_a_fresh_pinned_workspace() {
        // FINDING (docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md,
        // "Live-verification finding"): a workspace pin defaulted to read-only
        // on first residency (Agent::ensure_resident's documented default),
        // and Agent::activate clears every other resident workspace on every
        // call — so a per-request `workspace=` pin could never succeed at
        // writing to a workspace that was never separately `activate`d, even
        // though naming it in `workspace=` is already the caller's explicit
        // consent. A write-tool call with a pin must now upgrade that
        // workspace to writable on first touch.
        let dir_a = tempdir().unwrap();
        let (_dir_b, server) = make_server().await;
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // dir_a is NEVER explicitly activated — only referenced via the pin.
        let req = CallToolRequestParams::new("create_file").with_arguments(
            serde_json::from_value(serde_json::json!({
                "path": "new_file.txt",
                "content": "hello",
                "workspace": root_a.to_string_lossy(),
            }))
            .unwrap(),
        );
        let result = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();

        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("")
            .to_string();
        assert_ne!(
            result.is_error,
            Some(true),
            "write pinned to a never-activated workspace must succeed \
             (the pin is the caller's explicit consent); got: {text}"
        );
        assert!(
            dir_a.path().join("new_file.txt").exists(),
            "file must be created in the pinned workspace A"
        );
    }

    // `artifact` only registers with the librarian feature on.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn artifact_find_honors_workspace_pin() {
        // BUG (docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md): the
        // librarian adapter derived current_project from the session-default
        // active_project() and ignored ctx.workspace_override, so artifact(find)
        // pinned to a foreign workspace silently returned the SESSION project's
        // rows (fails silent-wrong). A pinned find must scope to the pinned
        // workspace, not the default.
        let dir_a = tempdir().unwrap();
        let (_dir_b, server) = make_server().await; // default (session) workspace B
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Seed ONE tracker in the session-default workspace B via an UNPINNED
        // create — unaffected by the bug, so the fixture is stable regardless of
        // the fix.
        let create = CallToolRequestParams::new("artifact").with_arguments(
            serde_json::from_value(serde_json::json!({
                "action": "create",
                "kind": "tracker",
                "title": "B-only tracker",
                "rel_path": "docs/trackers/b-only.md",
                "body": "seed",
            }))
            .unwrap(),
        );
        let created = server
            .call_tool_inner(
                create,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_ne!(
            created.is_error,
            Some(true),
            "seed create in the default workspace B must succeed"
        );

        // Find pinned to A (which has NO rows). It must scope to A and return
        // zero rows — NOT fall back to B and hand back B's tracker.
        let find_a = CallToolRequestParams::new("artifact").with_arguments(
            serde_json::from_value(serde_json::json!({
                "action": "find",
                "kind": "tracker",
                "workspace": root_a.to_string_lossy(),
            }))
            .unwrap(),
        );
        let res_a = server
            .call_tool_inner(
                find_a,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let text_a = res_a
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("")
            .to_string();
        let body_a: serde_json::Value = serde_json::from_str(&text_a).unwrap();
        assert_eq!(
            body_a["count"].as_u64(),
            Some(0),
            "find pinned to workspace A (no rows) must return zero, not B's rows; got: {text_a}"
        );

        // Sanity: the UNPINNED find still sees B's tracker (default preserved).
        let find_default = CallToolRequestParams::new("artifact").with_arguments(
            serde_json::from_value(serde_json::json!({
                "action": "find",
                "kind": "tracker",
            }))
            .unwrap(),
        );
        let res_d = server
            .call_tool_inner(
                find_default,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let text_d = res_d
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("")
            .to_string();
        let body_d: serde_json::Value = serde_json::from_str(&text_d).unwrap();
        assert_eq!(
            body_d["count"].as_u64(),
            Some(1),
            "unpinned find must still see the session-default workspace B's tracker; got: {text_d}"
        );
    }

    // `artifact` only registers with the librarian feature on.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn artifact_create_honors_workspace_pin() {
        // Whole-family pin (docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md):
        // resolving the pin in the adapter's call() covers writes too — a pinned
        // artifact(create) must land in the PINNED workspace, not the default.
        let dir_a = tempdir().unwrap();
        let (dir_b, server) = make_server().await; // default (session) workspace B
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        let create = CallToolRequestParams::new("artifact").with_arguments(
            serde_json::from_value(serde_json::json!({
                "action": "create",
                "kind": "tracker",
                "title": "A tracker",
                "rel_path": "docs/trackers/a.md",
                "body": "seed",
                "workspace": root_a.to_string_lossy(),
            }))
            .unwrap(),
        );
        let res = server
            .call_tool_inner(
                create,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let text = res
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("")
            .to_string();
        assert_ne!(
            res.is_error,
            Some(true),
            "create pinned to a never-activated workspace must succeed; got: {text}"
        );
        assert!(
            dir_a.path().join("docs/trackers/a.md").exists(),
            "artifact must be created in the PINNED workspace A"
        );
        assert!(
            !dir_b.path().join("docs/trackers/a.md").exists(),
            "artifact must NOT be created in the session-default workspace B"
        );
    }

    /// The `Substitutable` half of `## Project Status` rides the first eligible response.
    ///
    /// These segments used to live in `server_instructions`, where the 2048-char ceiling
    /// dropped them first — on a Kotlin project with custom instructions, several never
    /// arrived at all. The response channel has no ceiling, so they arrive whole.
    #[tokio::test]
    async fn the_status_block_rides_the_first_eligible_response_then_stays_quiet() {
        let (_dir, server) = make_server().await;
        let payload = || CallToolResult::success(vec![Content::text("{}")]);
        let joined = |r: &CallToolResult| -> String {
            r.content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let first = server.post_process(payload(), "read_file", None).await;
        let text = joined(&first);
        assert!(
            text.contains("## Project Status (details)"),
            "the first eligible response must carry the substitutable block, got:\n{text}"
        );
        assert!(
            text.contains("Semantic index:"),
            "and it must carry the segments themselves, not just a heading:\n{text}"
        );

        // Its own heading, deliberately not a second `## Project Status`: the persistent
        // half is fixed at activation while this one is re-rendered per emission, so two
        // blocks under one title would read as a contradiction the moment they disagree.
        assert!(
            !text.contains("**Active project:**"),
            "anchors stay in the persistent channel; duplicating them here spends the \
             budget the split exists to free:\n{text}"
        );

        for tool_name in ["read_file", "tree", "symbols", "grep"] {
            let again = server.post_process(payload(), tool_name, None).await;
            assert!(
                !joined(&again).contains("## Project Status (details)"),
                "tool '{tool_name}' must not re-emit it inside one activation window"
            );
        }
    }

    /// **Compaction re-arms it, and the re-armed block rides the compaction call's own
    /// response.** This is the test the whole carrier decision turns on.
    ///
    /// The block is conversation content, which is exactly what `/compact` discards —
    /// unlike `server_instructions`, which rides the system prompt and is re-sent on every
    /// request. Without this re-arm, one compaction removes the block for the rest of the
    /// session and nothing brings it back: strictly worse than the channel it moved off.
    /// `guide_hints_emitted` re-arms on the same signal for the same reason
    /// (`post_compact_rearms_guide_hints`); this makes the two agree.
    ///
    /// The re-delivery is IMMEDIATE, not deferred to the next call, and that falls out of
    /// where the reset sits: `call_tool_inner` resets the flag just before invoking
    /// `post_process` on that same call, so the `workspace(post_compact=true)` response is
    /// itself the first eligible response of the new window. Same ordering the
    /// `path_note_emitted_since_activation` comment describes for `activate` — where it was
    /// a bug to reset later, here it is exactly what you want: the agent that just told the
    /// server it compacted gets the block back in the reply.
    #[tokio::test]
    async fn a_compaction_rearms_the_status_block() {
        let (_dir, server) = make_server().await;
        let payload = || CallToolResult::success(vec![Content::text("{}")]);
        let has_block = |r: &CallToolResult| -> bool {
            r.content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .any(|t| t.contains("## Project Status (details)"))
        };

        assert!(has_block(
            &server.post_process(payload(), "read_file", None).await
        ));
        assert!(!has_block(
            &server.post_process(payload(), "read_file", None).await
        ));

        // The compaction signal, sent the way the companion hook sends it: `post_compact`
        // with no `action`, which infers `action="status"`. A reset keyed on a MISSING
        // action would never fire on this, the common call.
        let req = CallToolRequestParams::new("workspace").with_arguments(
            serde_json::from_value(serde_json::json!({"post_compact": true})).unwrap(),
        );
        let compaction_response = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();

        assert!(
            has_block(&compaction_response),
            "post_compact must re-arm the block AND carry it in its own response — \
             compaction summarized it out of context and nothing else re-delivers it"
        );
        assert!(
            !has_block(&server.post_process(payload(), "read_file", None).await),
            "and the gate is then consumed again, so the block does not repeat on every \
             call for the rest of the window"
        );
    }

    /// Both shapes of the compaction signal re-arm it.
    ///
    /// Written from an observed surviving mutation, not from a coverage argument. Adding
    /// `action.is_none()` to the reset condition — the exact trap the code comment warns
    /// about — left `a_compaction_rearms_the_status_block` green, because that test's
    /// fixture sends `post_compact` with no action and the extra clause is invisible to it.
    /// A comment describing a hazard is not a test for it.
    ///
    /// Both shapes are real: the companion hook sends `{post_compact: true}` bare, and
    /// `post_compact_rearms_guide_hints` — the sibling this mechanism is modelled on —
    /// sends `{action: "status", post_compact: true}`. The tool accepts either, inferring
    /// `action="status"` when absent, so a reset that handles only one silently drops the
    /// re-arm for half of its callers.
    #[tokio::test]
    async fn both_shapes_of_the_compaction_signal_rearm_the_status_block() {
        let has_block = |r: &CallToolResult| -> bool {
            r.content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .any(|t| t.contains("## Project Status (details)"))
        };

        for args in [
            serde_json::json!({"post_compact": true}),
            serde_json::json!({"action": "status", "post_compact": true}),
        ] {
            let (_dir, server) = make_server().await;
            let payload = || CallToolResult::success(vec![Content::text("{}")]);

            // Consume the gate.
            let _ = server.post_process(payload(), "read_file", None).await;
            assert!(!has_block(
                &server.post_process(payload(), "read_file", None).await
            ));

            let req = CallToolRequestParams::new("workspace")
                .with_arguments(serde_json::from_value(args.clone()).unwrap());
            let response = server
                .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
                .await
                .unwrap();

            assert!(
                has_block(&response),
                "compaction signalled as {args} must re-arm the status block"
            );
        }
    }

    /// `run_command` returns before the gate is consulted, so raw shell output stays
    /// verbatim — and, critically, the gate is NOT consumed. A `run_command` first call
    /// must not cost the session its one status block; the next eligible tool gets it.
    #[tokio::test]
    async fn run_command_neither_carries_the_status_block_nor_consumes_the_gate() {
        let (_dir, server) = make_server().await;
        let payload = || CallToolResult::success(vec![Content::text("{}")]);
        let has_block = |r: &CallToolResult| -> bool {
            r.content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .any(|t| t.contains("## Project Status (details)"))
        };

        assert!(
            !has_block(&server.post_process(payload(), "run_command", None).await),
            "raw shell output must stay verbatim"
        );
        assert!(
            has_block(&server.post_process(payload(), "read_file", None).await),
            "and the gate must survive it — otherwise an agent whose first call is \
             run_command silently loses the block for the whole session"
        );
    }

    #[tokio::test]
    async fn responses_emit_paths_relative_annotation_once_per_activation() {
        // Novelty-gated annotation (replaces the U-23 "every-call" cadence):
        // post_process emits `[codescout] paths are relative to <root>` only on
        // the FIRST eligible response since server start or the last
        // `activate_project`. Subsequent eligible responses skip the
        // annotation — the agent already carries the signal via the
        // `Active project` line in `build_server_instructions`, which
        // compaction preserves in the system-prompt slot.
        let (_dir, server) = make_server().await;
        // Build the payload from the server's *own* project-root form — the exact
        // string post_process strips against. On Windows the agent canonicalizes to
        // an extended-length path that differs from dir.path()'s plain/8.3 form, so
        // using dir.path() here would never match the strip prefix and the
        // annotation would never fire.
        // to_forward_slash, NOT .display() — post_process builds its strip prefix
        // with to_forward_slash, so on Windows a .display() form would never match
        // the string the code under test actually produces.
        let root = to_forward_slash(
            &server
                .agent
                .project_root()
                .await
                .expect("server has an active project root"),
        );
        let trimmed_root = root.trim_end_matches('/');

        let make_payload = || {
            CallToolResult::success(vec![Content::text(format!(
                r#"{{"file":"{root}/src/main.rs","line":1}}"#
            ))])
        };

        // First eligible response — annotation MUST appear.
        let first = server.post_process(make_payload(), "read_file", None).await;
        let joined: String = first
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains(&format!("[codescout] paths are relative to {trimmed_root}")),
            "first eligible response must carry the annotation, got:\n{joined}"
        );

        // Subsequent eligible responses across multiple tool names — annotation
        // MUST NOT re-appear within the same activation window.
        for tool_name in ["read_file", "tree", "symbols", "librarian", "grep"] {
            let processed = server.post_process(make_payload(), tool_name, None).await;
            let joined: String = processed
                .content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                !joined.contains("[codescout] paths are relative to"),
                "tool '{tool_name}' must not re-emit the annotation within the same activation window, got:\n{joined}"
            );
        }

        // Negative case: run_command is exempt from stripping, so the
        // annotation must NOT be appended even when its payload happens to
        // contain the project root. Independent of the novelty gate — the
        // run_command branch in post_process returns immediately, before the
        // gate or the banner are ever consulted.
        let processed = server
            .post_process(make_payload(), "run_command", None)
            .await;
        let joined: String = processed
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !joined.contains("[codescout] paths are relative to"),
            "run_command must not get the annotation (its raw stdout is left unstripped), got:\n{joined}"
        );

        // Activation reset: flipping the gate manually (as `call_tool_inner` does,
        // right before its own `post_process` call, for a `workspace(activate)`
        // request) must restore single-shot emission.
        server
            .path_note_emitted_since_activation
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let after_reset = server.post_process(make_payload(), "read_file", None).await;
        let joined: String = after_reset
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains(&format!("[codescout] paths are relative to {trimmed_root}")),
            "post-activation reset: next eligible response must re-emit the annotation, got:\n{joined}"
        );
    }

    #[tokio::test]
    async fn activation_and_the_next_two_calls_carry_the_banner_exactly_once() {
        // Regression (fix round 1 on the field-aware-path-strip Task 3 review):
        // the novelty gate used to get reset AFTER call_tool_inner returned, in
        // call_tool's `is_activate` branch — which runs after THIS SAME activate
        // response had already been through call_tool_inner's own post_process
        // call. That let the activate response consume the gate via
        // post_process, then the later reset re-armed it in the same breath, so
        // the very next ordinary response fired the banner again — two banners
        // per activation. Fixed by resetting the gate inside call_tool_inner,
        // right before ITS OWN post_process call, whenever the call itself is
        // a `workspace(activate)` request (matched on request shape only,
        // regardless of outcome). This test goes through
        // call_tool_inner (not post_process directly, unlike the test above) so
        // it actually exercises that ordering.
        let (dir, server) = make_server().await;

        let activate_req = CallToolRequestParams::new("workspace").with_arguments(
            serde_json::from_value(serde_json::json!({
                "action": "activate",
                "path": dir.path().to_string_lossy(),
            }))
            .unwrap(),
        );
        let activate_result = server
            .call_tool_inner(
                activate_req,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();

        let tree_req_1 = CallToolRequestParams::new("tree")
            .with_arguments(serde_json::from_value(serde_json::json!({"path": "."})).unwrap());
        let tree_result_1 = server
            .call_tool_inner(
                tree_req_1,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();

        let tree_req_2 = CallToolRequestParams::new("tree")
            .with_arguments(serde_json::from_value(serde_json::json!({"path": "."})).unwrap());
        let tree_result_2 = server
            .call_tool_inner(
                tree_req_2,
                None,
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();

        let banner = "[codescout] paths are relative to";
        let carries_banner = |r: &CallToolResult| {
            r.content
                .iter()
                .any(|c| c.as_text().is_some_and(|t| t.text.contains(banner)))
        };
        let flags = [
            carries_banner(&activate_result),
            carries_banner(&tree_result_1),
            carries_banner(&tree_result_2),
        ];

        assert_eq!(
            flags.iter().filter(|f| **f).count(),
            1,
            "exactly one of the activate response and the next two ordinary \
             responses must carry the banner; activate={}, tree_1={}, tree_2={}",
            flags[0],
            flags[1],
            flags[2],
        );
    }

    #[tokio::test]
    async fn compaction_rearms_the_path_relative_banner() {
        // The banner lives in the CONVERSATION, and `/compact` is exactly what
        // discards it. Its novelty gate used to be re-armed only by
        // `workspace(activate)` — while the sibling
        // `status_block_emitted_since_activation` gate, reset one clause later in this
        // same function, already re-armed on `post_compact` too. Two gates of identical
        // shape, adjacent in one function, disagreed; only one was right. The
        // consequence: after a compaction every later response carried project-relative
        // paths with nothing in context saying they were relative, and an agent
        // resolving one against its own cwd resolves against the wrong base.
        //
        // Reproduced live before fixing: in the 2026-08-26 session the post-compact
        // `workspace(post_compact=true)` response and every call after it went
        // bannerless until an `/mcp` restart re-constructed the server.
        // docs/issues/archive/2026-08-21-path-relative-banner-not-rearmed-after-compaction.md
        let banner = "[codescout] paths are relative to";
        let carries = |r: &CallToolResult| {
            r.content
                .iter()
                .any(|c| c.as_text().is_some_and(|t| t.text.contains(banner)))
        };

        // Both shapes, for the reason `both_shapes_of_the_compaction_signal_rearm_the_
        // status_block` documents: the companion hook sends `{post_compact: true}`
        // bare, the guide-hint sibling sends `{action: "status", post_compact: true}`,
        // and a reset handling only one silently drops half its callers. The two gates
        // now share a single `if`, which makes looping look redundant with that test —
        // it is not. "Covered by construction" is exactly the argument a mutation
        // defeats, and that test was itself written from a surviving one.
        for args in [
            serde_json::json!({"post_compact": true}),
            serde_json::json!({"action": "status", "post_compact": true}),
        ] {
            let (dir, server) = make_server().await;
            let tree = || {
                CallToolRequestParams::new("tree").with_arguments(
                    serde_json::from_value(serde_json::json!({"path": "."})).unwrap(),
                )
            };
            let token = tokio_util::sync::CancellationToken::new;

            // Spend the activation window the way a real session does.
            server
                .call_tool_inner(
                    CallToolRequestParams::new("workspace").with_arguments(
                        serde_json::from_value(serde_json::json!({
                            "action": "activate",
                            "path": dir.path().to_string_lossy(),
                        }))
                        .unwrap(),
                    ),
                    None,
                    None,
                    token(),
                )
                .await
                .unwrap();

            let before = server
                .call_tool_inner(tree(), None, None, token())
                .await
                .unwrap();
            assert!(
                !carries(&before),
                "precondition ({args}): the activation window must already be spent, so \
                 a plain call carries no banner"
            );

            // `/compact` happened; the agent reports it.
            let post_compact = server
                .call_tool_inner(
                    CallToolRequestParams::new("workspace")
                        .with_arguments(serde_json::from_value(args.clone()).unwrap()),
                    None,
                    None,
                    token(),
                )
                .await
                .unwrap();

            let after_1 = server
                .call_tool_inner(tree(), None, None, token())
                .await
                .unwrap();
            let after_2 = server
                .call_tool_inner(tree(), None, None, token())
                .await
                .unwrap();

            // Asserted as a count, not against one particular response: the contract is
            // "compaction brings the fact back, once", not "it rides on the
            // post_compact reply". Without the fix this is 0; a double-arming bug of the
            // kind `activation_and_the_next_two_calls_carry_the_banner_exactly_once`
            // guards would make it 2.
            let flags = [carries(&post_compact), carries(&after_1), carries(&after_2)];
            assert_eq!(
                flags.iter().filter(|f| **f).count(),
                1,
                "compaction signalled as {args} must re-arm the path-relative banner \
                 exactly once; post_compact={}, next={}, next2={}",
                flags[0],
                flags[1],
                flags[2],
            );
        }
    }

    #[tokio::test]
    async fn run_command_output_keeps_absolute_project_paths() {
        // Regression: docs/issues/archive/2026-05-21-run-command-strips-project-root-from-path-literals.md
        // run_command stdout is raw shell output; an absolute path literal under
        // the project root (e.g. from readlink/realpath) must be returned
        // verbatim, not silently rewritten to a relative-looking string.
        let (dir, server) = make_server().await;
        // Spell the path in the form the executing shell survives. Commands run
        // through a POSIX shell on both platforms (Git Bash on Windows), where
        // `\` is an escape character — a native `C:\a\b` literal would reach
        // `echo` as `C:ab` and this test would be asserting on shell escaping
        // rather than on the regression it guards, which is codescout rewriting
        // absolute path literals in stdout. Separator style is irrelevant to that.
        let abs =
            crate::platform::shell_path_str(&dir.path().join("some").join("nested").join("path"));

        let req = CallToolRequestParams::new("run_command").with_arguments(
            serde_json::from_value(serde_json::json!({
                "command": format!("echo {abs}"),
            }))
            .unwrap(),
        );
        let result = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();

        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("");

        // Parse the JSON response and inspect `stdout` directly — this avoids
        // matching against JSON-escaped backslashes that the serialized `text`
        // contains on Windows (`C:\\Users` in JSON is two chars per `\`).
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("run_command result should be JSON");
        let stdout = parsed["stdout"]
            .as_str()
            .expect("run_command result should expose `stdout` as a string");

        assert!(
        stdout.contains(&abs),
        "run_command stdout must keep the absolute project path verbatim.\n  expected substring: {abs}\n  actual stdout: {stdout}"
    );
    }

    #[tokio::test]
    async fn edit_failure_hint_reproduces_the_files_real_bytes() {
        // The "Nearest content" hint (src/tools/edit_file/mod.rs:196) embeds RAW
        // file bytes. Before this change it returned through the same strip as
        // read_file, so a path literal was rewritten identically in both and the
        // mismatch could not be falsified from inside the session.
        let (dir, server) = make_server().await;
        let root_fwd = to_forward_slash(dir.path());
        let literal = format!("REPO = \"{root_fwd}/.worktrees/single-stage\"");
        // A leading anchor line is required: `nearest_window_hint` only surfaces
        // a window when at least one of its lines matches `old_string` exactly
        // (after trimming) — a probe file of just the `literal` line scores zero
        // against a deliberately-mismatched `old_string` and yields no window at
        // all, which would make this test unable to observe the hint's bytes
        // regardless of stripping.
        std::fs::write(dir.path().join("probe.txt"), format!("MARKER\n{literal}\n")).unwrap();

        let req = CallToolRequestParams::new("edit_file").with_arguments(
            serde_json::from_value(serde_json::json!({
                "path": "probe.txt",
                "old_string": "MARKER\nREPO = \".worktrees/single-stage\"",
                "new_string": "x",
            }))
            .unwrap(),
        );
        let result = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();
        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("");

        // Parse the JSON response and inspect `error` directly, the same way
        // `run_command_output_keeps_absolute_project_paths` does — a raw
        // substring check against `text` would compare `literal`'s bare `"`
        // bytes against the `\"` two-byte escape `route_tool_error`'s
        // `to_string_pretty` produces for the same quote inside a JSON string,
        // which never matches regardless of whether stripping happened.
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("edit_file failure should be JSON");
        let error_msg = parsed["error"].as_str().unwrap_or("");

        assert!(
            error_msg.contains(&literal),
            "the failure must quote the file's REAL bytes so the caller can see \
             why the match failed; got: {error_msg}"
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
            shell_enabled: true,
        };
        let caps_with_lsp = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: false,
            has_libraries: false,
            shell_enabled: true,
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
        // LSP tools (symbol_at, references) must be hidden.
        if !caps.has_lsp {
            let visible: Vec<&str> = server
                .tools
                .iter()
                .filter(|t| t.availability(&caps).is_available(&caps))
                .map(|t| t.name())
                .collect();
            for lsp_tool in &["symbol_at", "references"] {
                assert!(
                    !visible.contains(lsp_tool),
                    "LSP tool '{}' should be hidden when has_lsp=false",
                    lsp_tool
                );
            }
            // Non-LSP tools must still be visible.
            for always_tool in &["read_file", "tree", "memory", "workspace"] {
                assert!(
                    visible.contains(always_tool),
                    "Always-available tool '{}' should remain visible",
                    always_tool
                );
            }
        }
    }

    /// Regression: `library` is the only surface on which a library can be registered
    /// (`register` is one of its actions), and its `availability` gated on `has_libraries`,
    /// which `current_capabilities` computes as *"at least one library is already registered"*.
    /// The tool that establishes the precondition was hidden by that precondition.
    ///
    /// Measured 2026-09-01 before the fix, in a fresh git repo with no registered libraries:
    /// **15** tools advertised and `library` absent — while `library(action="register", path=…)`
    /// dispatched normally and returned `Registered library 'codescout' (rust)`. Discovery and
    /// dispatch disagreed, and discovery is the only surface an agent can read.
    ///
    /// **The load-bearing fixture detail is `has_libraries: false`.** Flip it to `true` and
    /// this test passes against the *unfixed* code, because `true` is exactly the state the old
    /// gate admitted. The assertion discriminates in one direction only, so the `false` is what
    /// makes it a test rather than a restatement.
    #[tokio::test]
    async fn library_is_advertised_when_no_library_is_registered_yet() {
        use crate::tools::ToolCapabilities;

        let caps = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: true,
            // Load-bearing: the state the old gate hid `library` in. See doc comment.
            has_libraries: false,
            shell_enabled: true,
        };

        let (_dir, server) = make_server().await;
        let visible: Vec<&str> = server
            .tools
            .iter()
            .filter(|t| t.availability(&caps).is_available(&caps))
            .map(|t| t.name())
            .collect();

        assert!(
            visible.contains(&"library"),
            "`library` must be advertised when has_libraries=false — it is the only surface \
             that can register the first library, so gating it on one already existing hides \
             the bootstrap path in exactly the state that needs it. Visible: {visible:?}"
        );
    }

    #[tokio::test]
    async fn list_tools_shows_lsp_tools_when_has_lsp() {
        use crate::tools::ToolCapabilities;

        let caps_with_lsp = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: false,
            has_libraries: false,
            shell_enabled: true,
        };

        let (_dir, server) = make_server().await;
        let visible: Vec<&str> = server
            .tools
            .iter()
            .filter(|t| t.availability(&caps_with_lsp).is_available(&caps_with_lsp))
            .map(|t| t.name())
            .collect();

        for lsp_tool in &["symbol_at", "references"] {
            assert!(
                visible.contains(lsp_tool),
                "LSP tool '{}' should be visible when has_lsp=true",
                lsp_tool
            );
        }
    }

    /// The tool names `list_tools` would advertise for `server`'s CURRENT
    /// capabilities, applying the same filter `ServerHandler::list_tools` does
    /// (`src/server.rs`, `.filter(|t| t.availability(&caps).is_available(&caps))`).
    async fn advertised_tool_names(server: &CodeScoutServer) -> Vec<&str> {
        let caps = server.current_capabilities().await;
        server
            .tools
            .iter()
            .filter(|t| t.availability(&caps).is_available(&caps))
            .map(|t| t.name())
            .collect()
    }

    #[tokio::test]
    async fn run_command_hidden_when_shell_command_mode_is_disabled() {
        let (_dir, server) = make_server_with_project_toml(Some(
            "[project]\nname = \"t\"\n\n[security]\nshell_command_mode = \"disabled\"\n",
        ))
        .await;

        let caps = server.current_capabilities().await;
        assert!(
            !caps.shell_enabled,
            "shell_command_mode = \"disabled\" must clear shell_enabled"
        );

        let visible = advertised_tool_names(&server).await;
        assert!(
            !visible.contains(&"run_command"),
            "run_command must not be advertised when shell is disabled, got: {visible:?}"
        );
        // Hiding must be surgical: prove we removed one tool rather than
        // emptying the surface, which would make the assertion above vacuous.
        for still_there in &["read_file", "tree", "grep", "memory", "workspace"] {
            assert!(
                visible.contains(still_there),
                "'{still_there}' must stay advertised when only shell is disabled"
            );
        }
    }

    #[tokio::test]
    async fn run_command_advertised_when_shell_command_mode_is_warn() {
        // Pinned explicitly rather than relying on the default, so the test is
        // hermetic against a global ~/.config/codescout/config.toml that sets
        // shell_command_mode — project.toml overlays the global layer.
        let (_dir, server) = make_server_with_project_toml(Some(
            "[project]\nname = \"t\"\n\n[security]\nshell_command_mode = \"warn\"\n",
        ))
        .await;

        let caps = server.current_capabilities().await;
        assert!(caps.shell_enabled, "\"warn\" must leave shell_enabled set");
        assert!(
            advertised_tool_names(&server)
                .await
                .contains(&"run_command"),
            "run_command must be advertised under the default \"warn\" mode"
        );
    }

    /// An unrecognised mode must leave `run_command` VISIBLE.
    ///
    /// This is the interesting case, and it is why `shell_enabled` is derived as
    /// `mode != "disabled"` rather than by whitelisting the two good values.
    /// Whitelisting would turn a config typo into a silently absent tool — the
    /// one symptom that gives the caller nothing to act on. Left visible, the
    /// call instead reaches `run_command_inner`'s
    /// `unknown shell_command_mode: '<x>'` error, which names the bad value.
    #[tokio::test]
    async fn run_command_advertised_when_shell_command_mode_is_unrecognised() {
        let (_dir, server) = make_server_with_project_toml(Some(
            "[project]\nname = \"t\"\n\n[security]\nshell_command_mode = \"disabledd\"\n",
        ))
        .await;

        let caps = server.current_capabilities().await;
        assert!(
            caps.shell_enabled,
            "a typo'd mode must NOT be read as \"disabled\""
        );
        assert!(
            advertised_tool_names(&server)
                .await
                .contains(&"run_command"),
            "a typo'd mode must leave run_command visible so its call can report the bad value"
        );
    }

    /// `RunCommand` must actually be wired to the shell gate.
    ///
    /// Asserted on the variant rather than only through a filtered list: a
    /// regression that reverted `availability()` to the `Always` default would
    /// still pass the two positive tests above, since `Always` is also visible.
    #[tokio::test]
    async fn run_command_declares_the_shell_availability_gate() {
        use crate::tools::{Availability, Tool};

        let caps = crate::tools::ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: true,
            has_libraries: true,
            shell_enabled: false,
        };
        assert!(matches!(
            crate::tools::RunCommand.availability(&caps),
            Availability::RequiresShell
        ));
    }

    #[cfg(any(
        feature = "local-embed",
        feature = "local-embed-dynamic",
        feature = "remote-embed"
    ))]
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

    #[tokio::test]
    async fn is_write_call_classifies_plain_writes() {
        use serde_json::json;
        let (_dir, server) = make_server().await;
        assert!(server.is_write_call("edit_file", &json!({})));
        assert!(server.is_write_call("create_file", &json!({})));
        assert!(server.is_write_call("edit_code", &json!({"action": "replace"})));
        assert!(server.is_write_call("edit_code", &json!({"action": "insert"})));
        assert!(server.is_write_call("edit_code", &json!({"action": "remove"})));
        assert!(server.is_write_call("edit_code", &json!({"action": "rename"})));
        assert!(server.is_write_call("edit_markdown", &json!({})));
        assert!(server.is_write_call("index", &json!({"action": "build"})));
        assert!(!server.is_write_call("index", &json!({"action": "status"})));
        assert!(server.is_write_call("onboarding", &json!({})));
        assert!(server.is_write_call("library", &json!({"action": "register"})));
        assert!(!server.is_write_call("library", &json!({"action": "list"})));
        assert!(!server.is_write_call("read_file", &json!({})));
        assert!(!server.is_write_call("symbols", &json!({})));
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
    #[tokio::test]
    async fn call_tool_by_name_dispatches_a_read_tool() {
        let (_dir, server) = make_server().await;
        let result = server
            .call_tool_by_name("tree", serde_json::json!({ "path": "." }))
            .await
            .expect("dispatch ok");
        assert!(result.is_error.is_none_or(|e| !e), "tree should succeed");
    }

    #[tokio::test]
    async fn call_tool_by_name_rejects_unknown_tool() {
        let (_dir, server) = make_server().await;
        let err = server
            .call_tool_by_name("does_not_exist", serde_json::json!({}))
            .await;
        assert!(err.is_err(), "unknown tool must error");
    }
    #[test]
    fn parse_idle_shutdown_disables_on_anything_but_a_positive_integer() {
        // Every shape that is not a positive integer must DISABLE the watchdog rather than
        // fall back to a window we invented — see the fn's doc comment.
        for raw in [
            None,
            Some(""),
            Some("   "),
            Some("0"),
            Some("-30"),
            Some("abc"),
            Some("30s"),
            Some("1.5"),
        ] {
            assert_eq!(
                parse_idle_shutdown(raw),
                None,
                "{raw:?} must leave idle shutdown disabled"
            );
        }
        assert_eq!(
            parse_idle_shutdown(Some(" 3600 ")),
            Some(std::time::Duration::from_secs(3600)),
            "a positive integer arms the window, whitespace tolerated"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn idle_watchdog_never_resolves_when_disabled() {
        // The inert default. A day of virtual time passes and the watchdog stays pending.
        let last = Arc::new(parking_lot::Mutex::new(tokio::time::Instant::now()));
        tokio::select! {
            _ = idle_watchdog(None, last) => panic!("a disabled watchdog must never resolve"),
            _ = tokio::time::sleep(std::time::Duration::from_secs(86_400)) => {}
        }
    }

    #[tokio::test(start_paused = true)]
    async fn idle_watchdog_resolves_only_after_the_full_window() {
        let last = Arc::new(parking_lot::Mutex::new(tokio::time::Instant::now()));
        let t0 = tokio::time::Instant::now();
        idle_watchdog(Some(std::time::Duration::from_secs(600)), last).await;
        // `tokio::time::Instant` is virtualised under start_paused, so this is an exact
        // statement about the schedule rather than a tolerance for jitter.
        assert!(
            t0.elapsed() >= std::time::Duration::from_secs(600),
            "resolved early, at {:?}",
            t0.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_with_deadline_returns_after_deadline_when_inner_never_resolves() {
        let t0 = tokio::time::Instant::now();
        shutdown_with_deadline(
            std::future::pending::<()>(),
            std::time::Duration::from_secs(20),
            "test_never_resolves",
        )
        .await;
        assert!(
            t0.elapsed() >= std::time::Duration::from_secs(20),
            "resolved early, at {:?}",
            t0.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_with_deadline_returns_promptly_when_inner_resolves() {
        let t0 = tokio::time::Instant::now();
        shutdown_with_deadline(
            async {},
            std::time::Duration::from_secs(20),
            "test_resolves_immediately",
        )
        .await;
        assert!(
            t0.elapsed() < std::time::Duration::from_secs(20),
            "waited out the deadline instead of returning promptly, at {:?}",
            t0.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn activity_defers_the_idle_window() {
        // The discriminating case: a watchdog that ignored `last_activity` entirely would
        // still pass both tests above. Bumping the clock at t=300s must push the deadline
        // from t=600s to t=900s.
        let last = Arc::new(parking_lot::Mutex::new(tokio::time::Instant::now()));
        let bump = last.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            *bump.lock() = tokio::time::Instant::now();
        });
        let t0 = tokio::time::Instant::now();
        idle_watchdog(Some(std::time::Duration::from_secs(600)), last).await;
        assert!(
            t0.elapsed() >= std::time::Duration::from_secs(900),
            "activity at t=300s must defer the deadline to t=900s; resolved at {:?}",
            t0.elapsed()
        );
    }
}

#[cfg(feature = "librarian")]
#[cfg(test)]
mod guide_hint_tests {
    use super::*;
    use serde_json::{json, Value};

    // No `use serial_test::serial` and no `#[serial]` anywhere in this module: the
    // librarian workspace/db and the CC session id are INJECTED per server
    // (see `ServerEnv`), so these tests are isolated by construction rather than by
    // taking turns. They used to serialize only because they mutated process env.

    /// Test `ServerEnv` with the guide-hint ledger pinned inside `dir`, so no test
    /// ever reads, writes, or garbage-collects the real per-user state directory.
    fn test_env(dir: &std::path::Path) -> ServerEnv {
        ServerEnv {
            guide_hints_dir: Some(dir.join("guide_hints")),
            servers_dir: Some(dir.join("servers")),
            ..Default::default()
        }
    }

    /// Build a server with a per-test librarian workspace + catalog, INJECTED.
    ///
    /// This used to `set_var` LIBRARIAN_WORKSPACE / LIBRARIAN_DB behind an RAII guard.
    /// It no longer touches the environment at all — see [`ServerEnv`].
    ///
    /// The two values it injects are still load-bearing, for the reasons the old
    /// comment recorded:
    /// - **workspace:** without it, `build_tool_context` falls back to
    ///   `dirs::config_dir()/librarian/workspace.toml`, which is absent under
    ///   wine/windows-gnu — the deterministic cause of the 8-test guide_hint wine
    ///   cluster (`docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`): the
    ///   missing file fails the build, `try_build_runtime` returns None, and the
    ///   `artifact` tool never registers. An empty file is valid — `WorkspaceConfig`
    ///   is `#[derive(Default)]` with all fields `#[serde(default)]`.
    /// - **db:** without per-test isolation every test resolves the shared default
    ///   catalog (`dirs::data_local_dir()/librarian/catalog.db`) and they race on it —
    ///   intermittent on Linux (advisory locks), routine hangs on Windows (mandatory).
    ///
    /// Injecting gives strictly BETTER isolation than the env guards did: the values
    /// are scoped to this one server, so tests need no `#[serial]` to keep them apart.
    ///
    /// Also injects a `session_id_explicit` — a fresh uuid per call — so every test
    /// built through this helper gets a `SessionKey::Keyed` identity and the guide
    /// ledger persists, matching this module's pre-`session_key` assumption that a
    /// conversation id is always available. Tests of the `Anonymous` (no-identity)
    /// tier construct `ServerEnv` directly instead of going through this helper.
    async fn make_server() -> (tempfile::TempDir, CodeScoutServer) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ws_path = dir.path().join("librarian-workspace.toml");
        std::fs::write(&ws_path, "").unwrap();

        let env = ServerEnv {
            session_id_explicit: Some(uuid::Uuid::new_v4().to_string()),
            librarian: crate::librarian::LibrarianEnv {
                workspace: Some(ws_path),
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };

        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;
        (dir, server)
    }
    /// M11: nothing previously pinned that construction publishes the
    /// rendezvous slot at ALL. The module's own unit tests
    /// (`src/tools/rendezvous.rs`) call `Rendezvous::publish` directly, so they
    /// can't see whether `from_parts_with_env` ever actually calls it; every
    /// other test here builds a server and never inspects `servers_dir`.
    /// Replacing the real call with `Rendezvous::publish(None, session_key.id())`
    /// left the whole suite green.
    #[tokio::test]
    async fn from_parts_with_env_publishes_the_rendezvous_slot() {
        let (dir, _server) = make_server().await;
        let slot = dir
            .path()
            .join("servers")
            .join(format!("{}.json", std::process::id()));
        assert!(
            slot.exists(),
            "CodeScoutServer::from_parts_with_env must publish a rendezvous slot \
             at construction — Task 5's read side has nothing to read otherwise"
        );
    }

    fn tool_by_name(server: &CodeScoutServer, name: &str) -> Arc<dyn crate::tools::Tool> {
        server
            .tools
            .iter()
            .find(|t| t.name() == name)
            .unwrap_or_else(|| panic!("tool '{}' not registered", name))
            .clone()
    }

    fn shared_ctx(server: &CodeScoutServer) -> crate::tools::ToolContext {
        crate::tools::ToolContext {
            agent: server.agent.clone(),
            lsp: server.lsp.clone(),
            output_buffer: server.output_buffer.clone(),
            progress: None,
            peer: None,
            section_coverage: server.section_coverage.clone(),
            guide_hints_emitted: server.guide_hints_emitted.clone(),
            workspace_override: None,
        }
    }

    fn extract_hint(content: &[rmcp::model::Content]) -> Option<String> {
        let text = content.first()?.as_text()?.text.clone();
        let v: Value = serde_json::from_str(&text).ok()?;
        v.get("_guide_hint")
            .and_then(|h| h.as_str())
            .map(String::from)
    }

    /// Render a content payload into a failure message, truncated.
    ///
    /// A bare `assert!(hint.contains(...))` reads the same in at least four different
    /// broken worlds — the command timed out, produced no output, produced too little to
    /// overflow, or was refused by a gate. On the windows-gnu lane that ambiguity cost a
    /// CI round trip, because the only way to tell them apart is the envelope the
    /// assertion was throwing away (`docs/trackers/bug-fix-session-log.md` W-60).
    ///
    /// Truncated on purpose: a PASSING payload here is 2000 lines, and a failure message
    /// that dumps it is its own kind of unreadable.
    fn render_content(content: &[rmcp::model::Content]) -> String {
        match content.first().and_then(|c| c.as_text()) {
            Some(t) => t.text.chars().take(600).collect(),
            None => format!("<{} content item(s), none textual>", content.len()),
        }
    }

    /// Consume the session-opening guide slot.
    ///
    /// The opener fires on the first guide-eligible call of ANY session
    /// (`prompts::SESSION_OPENING_GUIDE`, dispatched from `Tool::call_content`).
    /// Tests that measure a *domain* guide's own trigger must warm the ledger
    /// first, or they measure the opener instead — which is exactly what made
    /// seven of these tests fail when the opener was widened on 2026-08-16.
    fn warm_ledger(ctx: &crate::tools::ToolContext) {
        ctx.guide_hints_emitted
            .lock()
            .insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());
    }

    /// Concatenate every content block of a result, for asserting on the
    /// second (appended) block without indexing past the end on failure.
    fn all_text(r: &CallToolResult) -> String {
        r.content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Scan every content block for the auto-injected guide-body marker for
    /// `topic`. `extract_hint` parses block 0 as JSON and returns `None`
    /// whenever the tool's primary block is text-form rather than JSON — so
    /// it cannot see the opener fire for those tools at all. This reads the
    /// guide-body block `call_content` appends instead: that marker is
    /// pushed regardless of the tool's output form, so it is the reliable
    /// probe for "did the opener actually fire."
    fn content_carries_guide_body(content: &[rmcp::model::Content], topic: &str) -> bool {
        let marker = format!("<!-- auto-injected get_guide('{topic}')");
        content
            .iter()
            .filter_map(|c| c.as_text())
            .any(|t| t.text.contains(&marker))
    }
    /// Call a tool by name against a fresh, warmed context (the session opener
    /// slot is pre-consumed so the assertion measures the tool's OWN guide
    /// delivery, not `project-activation-bootstrap`). Used by the section-grain
    /// tests (Task 8), which call several different tools/shapes against the
    /// same server and want each call's guide content in isolation.
    async fn call_tool(
        server: &CodeScoutServer,
        name: &str,
        input: Value,
    ) -> Vec<rmcp::model::Content> {
        // `shared_ctx`/`warm_ledger` touch the SAME `Arc<Mutex<GuideLedger>>`
        // the server itself dispatches with, so warming here pre-consumes the
        // opener slot for the `call_tool_by_name` call below too.
        let ctx = shared_ctx(server);
        warm_ledger(&ctx);
        // Route through the same dispatch path production traffic uses
        // (`call_tool_inner`, via `call_tool_by_name`), not a raw
        // `tool.call_content(...).await.unwrap()` — a call whose underlying
        // tool returns `RecoverableError` must come back as a graceful
        // `CallToolResult`, not panic the test harness. `call_content` itself
        // only runs the guide-injection logic on a call that actually
        // succeeds (the `?` on `self.call(...)` short-circuits past it on any
        // error, recoverable or not), so tests that need genuine guide
        // delivery must use inputs that genuinely succeed.
        server
            .call_tool_by_name(name, input)
            .await
            .expect("dispatch ok")
            .content
    }

    /// Same dispatch as `call_tool`, but asserts the call actually succeeded
    /// before returning its content. Guide injection only fires on
    /// `call_content`'s success path, so a silently-failed call produces 0 B
    /// of guide — indistinguishable from legitimate cross-call dedup unless
    /// the call is checked for BOTH failure shapes: `is_error: true` (fatal
    /// `anyhow` errors, e.g. `update`'s unknown-id path at
    /// `librarian/tools/update.rs:369`) AND a `RecoverableError`, which
    /// `route_tool_error` (this file) deliberately routes to `is_error: false`
    /// with an `{"ok": false, "error": ...}` body (e.g. `get`'s unknown-id
    /// path at `librarian/tools/get.rs:125-134`) — pinned by
    /// `recoverable_error_routes_to_success_not_is_error`. Checking `is_error`
    /// alone would silently pass a `RecoverableError`, undercounting the
    /// session's real guide draw with no test failure to show for it.
    /// `label` identifies the failing shape in the panic message.
    async fn call_tool_checked(
        server: &CodeScoutServer,
        name: &str,
        input: Value,
        label: &str,
    ) -> Vec<rmcp::model::Content> {
        let ctx = shared_ctx(server);
        warm_ledger(&ctx);
        let result = server
            .call_tool_by_name(name, input)
            .await
            .expect("dispatch ok");
        assert!(
            result.is_error.is_none_or(|e| !e),
            "{label} call must succeed for its guide bytes to count — got: {:?}",
            result.content
        );
        if let Some(primary) = result.content.first().and_then(|c| c.as_text()) {
            if let Ok(body) = serde_json::from_str::<Value>(&primary.text) {
                assert_ne!(
                    body.get("ok"),
                    Some(&Value::Bool(false)),
                    "{label} call returned a RecoverableError (isError:false, but \
                     ok:false) — its guide bytes cannot count: {body}"
                );
            }
        }
        result.content
    }

    /// Every content block after the primary (index 0) — the auto-injected
    /// guide blocks `call_content` appends, whether that is the single
    /// whole-topic block (non-declaring topic) or N section-slice blocks
    /// (declaring topic).
    fn guide_blocks(content: &[rmcp::model::Content]) -> Vec<String> {
        content
            .iter()
            .skip(1)
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect()
    }

    #[tokio::test]
    async fn a_refusal_carries_the_gate_condition_once_per_family() {
        let (_dir, server) = make_server().await;

        // IL-3: refused pre-execution, so nothing actually runs.
        let first = server
            .call_tool_by_name(
                "run_command",
                json!({"command": "cargo test | grep FAILED"}),
            )
            .await
            .expect("dispatch ok");
        assert_eq!(
            first.content.len(),
            2,
            "a refusal must carry the gate condition as a second block: {}",
            all_text(&first)
        );
        let predicate = all_text(&first);
        assert!(
            predicate.contains("IL-3 gate condition"),
            "got: {predicate}"
        );
        assert!(
            predicate.contains("--oneline"),
            "the predicate must name the EXCEPTION, which is the part a refusal \
             cannot convey: {predicate}"
        );

        // Same family again: the condition is not repeated.
        let second = server
            .call_tool_by_name("run_command", json!({"command": "cargo build | head -5"}))
            .await
            .expect("dispatch ok");
        assert_eq!(
            second.content.len(),
            1,
            "second refusal of the same family must not repeat it: {}",
            all_text(&second)
        );
    }

    #[tokio::test]
    async fn a_refusal_does_not_suppress_the_session_opening_guide() {
        // The hazard this design had to avoid. Before 2026-08-18 the opener's
        // trigger was `GuideLedger::emitted.is_empty()`, so stashing the
        // refusal key (`refusal-predicate:<family>`) in `emitted` via `insert`
        // rather than `notice_once` would have silently cost every session
        // that happens to start with a refusal its orientation guide — ANY
        // key landing in `emitted` made it non-empty. As of 2026-08-18 the
        // trigger is the `!emitted.contains(SESSION_OPENING_GUIDE)` check in
        // `Tool::call_content` (`src/tools/core/types.rs`), under which a
        // refusal key would only matter if it collided with that literal
        // topic string — which it does not. `notice_once` keeps it in the
        // separate `notices` set regardless, since that also protects the
        // topic namespace and the persisted stamp shape (see
        // `GuideLedger::notices`).
        let (_dir, server) = make_server().await;

        let refused = server
            .call_tool_by_name(
                "run_command",
                json!({"command": "cargo test | grep FAILED"}),
            )
            .await
            .expect("dispatch ok");
        assert_eq!(refused.content.len(), 2, "precondition: predicate attached");

        // Now the first SUCCESSFUL call. The opener must still fire.
        let ok = server
            .call_tool_by_name("tree", json!({"path": "."}))
            .await
            .expect("dispatch ok");
        // `content_carries_guide_body`, not a bare substring match on the topic
        // string: `SESSION_OPENING_GUIDE` ("project-activation-bootstrap") also
        // appears inside the `_guide_hint` prose, so a substring check on
        // `all_text` would pass on the hint alone, without the guide body ever
        // riding out — a green result that does not prove the opener fired.
        assert!(
            content_carries_guide_body(&ok.content, crate::prompts::SESSION_OPENING_GUIDE),
            "a prior refusal must not consume the session-opening slot: {}",
            all_text(&ok)
        );
    }

    #[tokio::test]
    async fn an_unrecognised_error_family_attaches_nothing() {
        // The table is deliberately partial — only families whose gate
        // condition is not inferable from the refusal get an entry. Everything
        // else must be untouched, so this cannot become a per-error tax.
        let (_dir, server) = make_server().await;
        let r = server
            .call_tool_by_name("read_markdown", json!({"path": "does-not-exist.md"}))
            .await
            .expect("dispatch ok");
        let text = all_text(&r);
        // Asserted on TEXT, not block count: `post_process` appends its own
        // path banner to every non-`run_command` result, so a count here would
        // measure that instead. (The two cases above CAN count, because
        // `post_process` returns early for `run_command`.)
        assert!(
            !text.contains("gate condition"),
            "an unrecognised family must attach no predicate: {text}"
        );
    }

    #[tokio::test]
    async fn first_artifact_call_emits_librarian_hint() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "artifact");
        let result = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&result)
                .unwrap_or_default()
                .contains("librarian"),
            "expected _guide_hint mentioning 'librarian' on first artifact call"
        );
    }

    /// An artifact call that touches a tracker or bug file delivers the guide about
    /// tracker conventions, not the general librarian guide.
    ///
    /// `tracker-conventions` (frontmatter, the status vocabulary,
    /// archive-through-the-catalog) was authored, cited from prose, and wired to nothing
    /// — one of the 7 of 10 topics BL-25 found firing for nobody. Two guides serve this
    /// tool and only one is delivered per call, so the choice is made from what the call
    /// actually touched.
    ///
    /// Note the coupling with `first_artifact_call_emits_librarian_hint` above: that test
    /// runs `find kind=tracker` and still expects `librarian`, which holds because its
    /// catalog is empty and `items: []` names no path. Populate that fixture and it would
    /// route here instead. Stated so the dependency is visible rather than a trap.
    ///
    /// See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
    #[tokio::test]
    async fn an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "artifact");

        let result = tool
            .call_content(
                json!({
                    "action": "create",
                    "kind": "tracker",
                    "title": "Guide routing probe",
                    "rel_path": "docs/trackers/guide-routing-probe.md",
                    "body": "probe",
                    "augment": { "prompt": "keep the probe" }
                }),
                &ctx,
            )
            .await
            .unwrap();

        let hint = extract_hint(&result).unwrap_or_default();
        assert!(
            hint.contains("tracker-conventions"),
            "a call creating a docs/trackers/ artifact must deliver the tracker guide, \
             got: {hint}"
        );
        assert!(
            !hint.contains("get_guide(\"librarian\")"),
            "one guide per call — the general librarian guide must not also fire: {hint}"
        );
    }

    /// The session opener must reach a session that never calls `workspace`.
    ///
    /// Before 2026-08-16 `project-activation-bootstrap` was triggered ONLY by
    /// the `workspace` tool, and `progressive-disclosure` is conditional on
    /// actual overflow — so a session opening with a small `run_command`
    /// received no guide at all. This pins that gap closed: the very call that
    /// `run_command_without_overflow_no_progressive_hint` asserts emits no
    /// *progressive-disclosure* hint must still open the session.
    #[tokio::test]
    async fn session_opens_with_bootstrap_from_a_non_workspace_tool() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        let tool = tool_by_name(&server, "run_command");
        let result = tool
            .call_content(json!({"command": "echo small"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&result)
                .unwrap_or_default()
                .contains(crate::prompts::SESSION_OPENING_GUIDE),
            "a session's first call must open with the orientation guide, whatever the tool"
        );
        assert_eq!(
            result.len(),
            2,
            "opener rides a second content block (primary + guide), got {}",
            result.len()
        );
        let second = result[1].as_text().expect("second block must be text");
        assert!(
            second.text.contains("ALWAYS VERIFY"),
            "the opener must carry the verification imperative that measured 100% \
             plausibility-verified as eval arm s1 — delivering the guide without it \
             would ship the trigger fix and drop the payload it exists for"
        );
    }

    /// Once per session, not once per call — the opener is subject to the same
    /// ledger dedup as every other topic.
    #[tokio::test]
    async fn session_opener_fires_once_not_on_every_call() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        let tool = tool_by_name(&server, "run_command");
        let _ = tool
            .call_content(json!({"command": "echo one"}), &ctx)
            .await
            .unwrap();
        let second = tool
            .call_content(json!({"command": "echo two"}), &ctx)
            .await
            .unwrap();
        assert!(
            !extract_hint(&second)
                .unwrap_or_default()
                .contains(crate::prompts::SESSION_OPENING_GUIDE),
            "the opener must not re-fire on later calls"
        );
    }

    /// The opener defers the calling tool's own topic by one call — it must not
    /// consume it. Without this, a session opening with `artifact` would never
    /// receive the librarian guide at all.
    #[tokio::test]
    async fn session_opener_defers_but_does_not_consume_the_tools_topic() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        let tool = tool_by_name(&server, "artifact");
        let first = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&first)
                .unwrap_or_default()
                .contains(crate::prompts::SESSION_OPENING_GUIDE),
            "first call opens the session"
        );
        let second = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&second)
                .unwrap_or_default()
                .contains("librarian"),
            "the tool's own guide must still arrive, one call later"
        );
    }

    /// The §5 predicate. A ledger holding other topics but NOT the bootstrap
    /// must still fire the opener. Under the old `is_empty()` trigger this is
    /// false — which is why a surgical re-arm would inject nothing.
    #[tokio::test]
    async fn opener_fires_when_bootstrap_absent_from_a_nonempty_set() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);

        // Seed a non-empty ledger that deliberately lacks the bootstrap topic.
        {
            let mut led = server.guide_hints_emitted.lock();
            led.insert("librarian".to_string());
            led.insert("progressive-disclosure".to_string());
            assert!(!led.is_empty());
            assert!(!led.contains(crate::prompts::SESSION_OPENING_GUIDE));
        }

        let tool = tool_by_name(&server, "run_command");
        let result = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();

        assert!(
            content_carries_guide_body(&result, crate::prompts::SESSION_OPENING_GUIDE),
            "a ledger without the bootstrap topic must re-open the session, \
             even though it is not empty"
        );
    }

    /// Retires a latent bug: an explicit get_guide as the session's first call
    /// made the set non-empty and suppressed the opener for the whole session.
    #[tokio::test]
    async fn explicit_get_guide_first_does_not_suppress_the_opener() {
        // Retires a latent bug: `GetGuide::call` inserts the requested topic
        // into the ledger BEFORE `call_content`'s opener check runs (it runs
        // via `self.call()`, which executes first in `call_content`). Under
        // the old `is_empty()` trigger that insert alone made the ledger
        // non-empty, so the opener never fired on this call — and, since
        // `is_empty()` never turns true again on its own, never fired for
        // the REST OF THE SESSION either.
        //
        // Under the new `!contains(bootstrap)` predicate the ledger is
        // non-empty (it holds "librarian") but still lacks the bootstrap
        // topic specifically, so the SAME call_content invocation that just
        // inserted "librarian" also fires the opener — deterministically,
        // because the ledger check runs after `self.call()` returns within
        // one invocation. So the guide body lands on THIS first call's own
        // response, not a later one. Verified empirically: asserting on the
        // second (`run_command`) call's content fails even with the fix
        // applied, because the opener already fired and deduped by then.
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);

        let guide = tool_by_name(&server, "get_guide");
        let first = guide
            .call_content(json!({"topic": "librarian"}), &ctx)
            .await
            .unwrap();

        assert!(
            content_carries_guide_body(&first, crate::prompts::SESSION_OPENING_GUIDE),
            "an explicit get_guide must not consume the session opener — the \
             bootstrap guide must still ride out, on this very call"
        );

        // Corollary: having fired once, it must not re-fire on the next call
        // (same dedup as every other topic).
        let tool = tool_by_name(&server, "run_command");
        let second = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();
        assert!(
            !content_carries_guide_body(&second, crate::prompts::SESSION_OPENING_GUIDE),
            "the opener must not re-fire once already delivered"
        );
    }

    /// Regression for docs/issues/archive/2026-06-01-librarian-adapter-stale-is-write.md:
    /// LibrarianAdapter::is_write matched dead tool names, so every librarian tool
    /// classified as a read and the main server's write-guard never engaged for
    /// catalog mutations. Pins the real names + per-action classification.
    #[tokio::test]
    async fn is_write_call_classifies_librarian_surface() {
        let (_dir, server) = make_server().await;
        // artifact: mutating actions write; queries read.
        assert!(server.is_write_call("artifact", &json!({"action": "create"})));
        assert!(server.is_write_call("artifact", &json!({"action": "update"})));
        assert!(server.is_write_call("artifact", &json!({"action": "move"})));
        assert!(server.is_write_call("artifact", &json!({"action": "delete"})));
        assert!(server.is_write_call("artifact", &json!({"action": "link"})));
        assert!(!server.is_write_call("artifact", &json!({"action": "find"})));
        assert!(!server.is_write_call("artifact", &json!({"action": "get"})));
        assert!(!server.is_write_call("artifact", &json!({"action": "graph"})));
        assert!(!server.is_write_call("artifact", &json!({"action": "state_at"})));
        // artifact_event: create writes, list reads.
        assert!(server.is_write_call("artifact_event", &json!({"action": "create"})));
        assert!(!server.is_write_call("artifact_event", &json!({"action": "list"})));
        // artifact_augment always writes (no read action).
        assert!(server.is_write_call("artifact_augment", &json!({"id": "x"})));
        // artifact_refresh gather/list_stale are read-only.
        assert!(!server.is_write_call("artifact_refresh", &json!({"action": "gather"})));
        assert!(!server.is_write_call("artifact_refresh", &json!({"action": "list_stale"})));
        // librarian: reindex + (default) audit_doc_refs write; the rest read.
        assert!(server.is_write_call("librarian", &json!({"action": "reindex"})));
        assert!(server.is_write_call("librarian", &json!({"action": "audit_doc_refs"})));
        assert!(!server.is_write_call(
            "librarian",
            &json!({"action": "audit_doc_refs", "emit_tracker": false})
        ));
        assert!(!server.is_write_call("librarian", &json!({"action": "context"})));
        assert!(!server.is_write_call("librarian", &json!({"action": "doctor"})));
        assert!(!server.is_write_call("librarian", &json!({"action": "tracker_design"})));
        assert!(server.is_write_call("librarian", &json!({"action": "legibility_scan"})));
        assert!(server.is_write_call(
            "librarian",
            &json!({"action": "legibility_scan", "write": true})
        ));
        assert!(!server.is_write_call(
            "librarian",
            &json!({"action": "legibility_scan", "write": false})
        ));
        // link_scan is READ-default — the polarity inverse of legibility_scan.
        assert!(!server.is_write_call("librarian", &json!({"action": "link_scan"})));
        assert!(server.is_write_call("librarian", &json!({"action": "link_scan", "write": true})));
        assert!(!server.is_write_call("librarian", &json!({"action": "link_scan", "write": false})));
    }

    #[tokio::test]
    async fn second_artifact_call_no_hint() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "artifact");
        let _ = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        let result = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&result).is_none(),
            "second call must not re-emit the hint"
        );
    }

    #[tokio::test]
    /// Updated for Task 8 (`feat(guides): emit section slices for declaring
    /// topics, preamble on no match`). `librarian` now declares `serves:`
    /// sections (Phase 1's only declaring topic), so an `artifact.find` call —
    /// which matches `## Filter Syntax`'s declaration — receives that ONE
    /// section, not the whole ~20 KB librarian body. This assertion is a
    /// deliberate behaviour change, not a relaxed regression: the whole-body
    /// append this test used to check is exactly what section grain replaces.
    async fn first_artifact_call_appends_librarian_guide_body_v2() {
        // V2 hard-injection: first call to a tool whose relevant_guide_topic
        // returns Some("librarian") gets a SECOND Content block. For a
        // declaring topic that block is the matching section, wrapped in
        // `<!-- auto-injected get_guide('librarian') § <heading> ... -->`
        // markers, not the whole guide body.
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "artifact");
        let result = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert_eq!(
                result.len(),
                2,
                "expected 2 content blocks on first librarian-topic call (primary + auto-injected guide section), got {}",
                result.len()
            );
        let second = result[1].as_text().expect("second block must be text");
        assert!(
            second
                .text
                .contains("<!-- auto-injected get_guide('librarian') § Filter Syntax"),
            "second block missing the section-scoped auto-inject opening marker: {:?}",
            &second.text[..second.text.len().min(200)]
        );
        assert!(
            second
                .text
                .contains("<!-- end auto-injected get_guide('librarian') § Filter Syntax -->"),
            "second block missing the section-scoped auto-inject closing marker"
        );
        assert!(
            second.text.contains("artifact.find"),
            "second block should contain the Filter Syntax section (mentions 'artifact.find')"
        );
        let whole = crate::prompts::topic_body("librarian").unwrap();
        assert!(
            second.text.len() < whole.len() / 4,
            "delivered {} B of a {} B guide — section grain is not engaged",
            second.text.len(),
            whole.len()
        );
    }

    #[tokio::test]
    async fn second_artifact_call_no_guide_body_block_v2() {
        // V2: dedup applies — second call within the same session does NOT
        // re-append the guide body block. Only the primary response block.
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "artifact");
        let _ = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        let result = tool
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert_eq!(
            result.len(),
            1,
            "second call must not re-inject the guide body block, got {} blocks",
            result.len()
        );
    }

    #[tokio::test]
    /// Pre-Task-8 name retained; behaviour updated deliberately for section-grain
    /// (Task 8, `feat(guides): emit section slices for declaring topics, preamble
    /// on no match`). Before that change, `librarian` was delivered as one whole
    /// topic gated on a bare `"librarian"` ledger key, so ANY second
    /// librarian-topic tool call in the same session — regardless of shape — hit
    /// the same key and got nothing. `librarian.md` declares
    /// `artifact_event.create, artifact_event.list` (a section distinct from
    /// `artifact.find`'s), so under section grain a genuinely different declared
    /// shape now legitimately delivers its own section once; only a REPEAT of the
    /// same shape delivers nothing. The old blanket "no hint" assertion is now
    /// false by design, not a regression.
    async fn a_distinct_declared_shape_delivers_its_own_section_but_a_repeat_does_not() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let artifact = tool_by_name(&server, "artifact");
        let event = tool_by_name(&server, "artifact_event");
        let _ = artifact
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        let first = event
            .call_content(
                json!({"action": "list", "artifact_id": "nonexistent"}),
                &ctx,
            )
            .await
            .unwrap();
        // `extract_hint` only proves SOMETHING shipped — the `_guide_hint`
        // field fires for a matched section AND for the preamble fallback
        // alike, so it cannot tell "artifact_event.list matched its declared
        // section" apart from "no section matched, here's the preamble".
        // Deleting `librarian.md`'s `<!-- serves: artifact_event.create,
        // artifact_event.list -->` declaration would leave this assertion
        // green while its stated claim ("delivers its own section") went
        // false. Assert on the actual section marker instead.
        let first_guide = guide_blocks(&first).join("");
        assert!(
            first_guide.contains("§ artifact_event — Event Log"),
            "a distinct declared shape (artifact_event.list) must deliver its own \
             `artifact_event — Event Log` section, even though a differently-shaped \
             librarian-topic call already fired this session; got: {}",
            first_guide.chars().take(400).collect::<String>()
        );
        let second = event
            .call_content(
                json!({"action": "list", "artifact_id": "nonexistent"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            extract_hint(&second).is_none(),
            "repeating the SAME shape must not re-emit the hint"
        );
    }

    #[tokio::test]
    /// Re-activating the SAME project must leave the ledger alone. This is the
    /// saving: today every activate wipes ~10 guide bodies out of the ledger and
    /// they all re-inject on the next call.
    ///
    /// Checks BOTH a non-project-scoped topic (`librarian`) and the
    /// project-scoped one (`SESSION_OPENING_GUIDE`) via `content_carries_guide_body`
    /// on the activate response itself — not a post-call `ledger.contains` check,
    /// which an inverted `switched` would pass anyway: if the bug wrongly re-armed
    /// the bootstrap topic, the opener would fire on THIS response and immediately
    /// re-insert it, making a post-call `contains` check blind to the mutation.
    async fn activate_same_project_keeps_hints() {
        let (dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        ctx.guide_hints_emitted.lock().set_rendezvous_active(true);
        ctx.guide_hints_emitted
            .lock()
            .insert("librarian".to_string());
        ctx.guide_hints_emitted
            .lock()
            .insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());

        let workspace = tool_by_name(&server, "workspace");
        let result = workspace
            .call_content(
                json!({"action": "activate", "path": dir.path().to_str().unwrap()}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            ctx.guide_hints_emitted.lock().contains("librarian"),
            "re-activating the SAME project must not clear the ledger"
        );
        assert!(
            !content_carries_guide_body(&result, crate::prompts::SESSION_OPENING_GUIDE),
            "re-activating the SAME project must not re-arm the project-scoped topic \
             either — a same-project activation is not a switch"
        );
    }

    #[tokio::test]
    /// A genuine project switch re-arms the project-scoped topic and NOTHING
    /// else — the tool-contract guides the model already holds must survive.
    /// The bootstrap re-arm and the session-opener check both run inside this
    /// one `call_content` invocation, so the opener's guide body must ride out
    /// on THIS activate response — asserted directly via
    /// `content_carries_guide_body`, not the `_guide_hint` field (which never
    /// reaches the wire for a text-form tool, and `workspace` renders text via
    /// `format_compact`).
    async fn activate_different_project_rearms_bootstrap_only() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        ctx.guide_hints_emitted.lock().set_rendezvous_active(true);
        ctx.guide_hints_emitted
            .lock()
            .insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());
        ctx.guide_hints_emitted
            .lock()
            .insert("librarian".to_string());

        let other = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(other.path().join(".codescout")).unwrap();

        let workspace = tool_by_name(&server, "workspace");
        let result = workspace
            .call_content(
                json!({"action": "activate", "path": other.path().to_str().unwrap()}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            content_carries_guide_body(&result, crate::prompts::SESSION_OPENING_GUIDE),
            "a genuine project switch must re-arm the project-scoped bootstrap topic, \
             and the opener must fire on this very activate response"
        );
        assert!(
            ctx.guide_hints_emitted.lock().contains("librarian"),
            "a project switch must not touch tool-contract topics the model already holds"
        );
    }

    #[tokio::test]
    /// The bare-project-id focus switch returns early via
    /// `activate_within_workspace` and must not touch the ledger at all.
    async fn subproject_focus_switch_does_not_rearm() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let sub = dir.path().join("packages").join("api");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("package.json"),
            r#"{"name":"api","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();
        let ws_path = dir.path().join("librarian-workspace.toml");
        std::fs::write(&ws_path, "").unwrap();

        let env = ServerEnv {
            session_id_explicit: Some(uuid::Uuid::new_v4().to_string()),
            librarian: crate::librarian::LibrarianEnv {
                workspace: Some(ws_path),
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        let ctx = shared_ctx(&server);
        ctx.guide_hints_emitted
            .lock()
            .insert("librarian".to_string());

        let workspace = tool_by_name(&server, "workspace");
        let _ = workspace
            .call_content(json!({"action": "activate", "path": "api"}), &ctx)
            .await
            .unwrap();

        assert!(
            ctx.guide_hints_emitted.lock().contains("librarian"),
            "a bare-project-id focus switch must not touch the ledger at all"
        );
    }

    #[tokio::test]
    /// F-52 regression: the comparand must be `default_workspace_root`, not
    /// `Agent::project_root()` (== `focused_project_root()`). Focusing a
    /// sub-project via `activate_within_workspace` never touches
    /// `default_workspace_root` — only `ws.focused` — so a later
    /// re-activation of the WORKSPACE ROOT is still the SAME project by that
    /// measure. `project_root()` would disagree (it'd report the focused
    /// sub-project as "current"), reading the root re-activation as a switch
    /// and wrongly re-arming the bootstrap topic.
    async fn root_reactivation_with_subproject_focused_does_not_rearm() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let sub = dir.path().join("packages").join("api");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("package.json"),
            r#"{"name":"api","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();
        let ws_path = dir.path().join("librarian-workspace.toml");
        std::fs::write(&ws_path, "").unwrap();

        let env = ServerEnv {
            session_id_explicit: Some(uuid::Uuid::new_v4().to_string()),
            librarian: crate::librarian::LibrarianEnv {
                workspace: Some(ws_path),
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        let ctx = shared_ctx(&server);
        let workspace = tool_by_name(&server, "workspace");

        // Focus the sub-project. Does NOT touch `default_workspace_root`.
        let _ = workspace
            .call_content(json!({"action": "activate", "path": "api"}), &ctx)
            .await
            .unwrap();

        ctx.guide_hints_emitted.lock().set_rendezvous_active(true);
        ctx.guide_hints_emitted
            .lock()
            .insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());

        // Re-activate the WORKSPACE ROOT while "api" is still focused.
        let result = workspace
            .call_content(
                json!({"action": "activate", "path": dir.path().to_str().unwrap()}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            !content_carries_guide_body(&result, crate::prompts::SESSION_OPENING_GUIDE),
            "re-activating the repo root while a sub-project is focused must \
             NOT read as a project switch"
        );
    }

    #[tokio::test]
    /// A missing/malformed `path`, or a `path` naming a nonexistent directory,
    /// must not wipe the ledger. Today both do, because the clear is the
    /// function's first statement and both checks happen later.
    async fn malformed_activate_leaves_ledger_intact() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        ctx.guide_hints_emitted
            .lock()
            .insert("librarian".to_string());

        let workspace = tool_by_name(&server, "workspace");
        let result = workspace
            .call_content(json!({"action": "activate"}), &ctx)
            .await;
        assert!(result.is_err(), "a missing path must still error");

        assert!(
            ctx.guide_hints_emitted.lock().contains("librarian"),
            "a malformed activate call must not wipe the ledger"
        );

        let result = workspace
            .call_content(
                json!({"action": "activate", "path": "/nonexistent/does-not-exist-9c3f1a"}),
                &ctx,
            )
            .await;
        assert!(result.is_err(), "a nonexistent directory must still error");

        assert!(
            ctx.guide_hints_emitted.lock().contains("librarian"),
            "a nonexistent-directory activate call must not wipe the ledger either"
        );
    }

    #[tokio::test]
    /// Without a rendezvous a `/clear` is invisible, so the precise path would
    /// starve the new conversation. The gate must fall back to the blunt clear.
    async fn without_a_rendezvous_activate_still_clears_everything() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        // Rendezvous gate left at its default (inactive) — no companion hook has
        // ever stamped this server's slot.
        //
        // Deliberately NOT `SESSION_OPENING_GUIDE`: `call_content`'s opener check
        // (`!emitted.contains(SESSION_OPENING_GUIDE)`) re-inserts that exact topic
        // on every response where it's absent, including this one — so it would
        // read back as present after the call REGARDLESS of clear vs. re-arm, and
        // could not distinguish the two paths. `librarian` and
        // `progressive-disclosure` are ordinary tool-contract topics `workspace`
        // never touches on its own, so their presence/absence isolates what the
        // re-arm predicate itself did.
        ctx.guide_hints_emitted
            .lock()
            .insert("librarian".to_string());
        ctx.guide_hints_emitted
            .lock()
            .insert("progressive-disclosure".to_string());

        let other = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(other.path().join(".codescout")).unwrap();

        let workspace = tool_by_name(&server, "workspace");
        let _ = workspace
            .call_content(
                json!({"action": "activate", "path": other.path().to_str().unwrap()}),
                &ctx,
            )
            .await
            .unwrap();

        let ledger = ctx.guide_hints_emitted.lock();
        assert!(
            !ledger.contains("librarian"),
            "without a rendezvous, activate must fall back to the blunt clear — \
             including tool-contract topics"
        );
        assert!(
            !ledger.contains("progressive-disclosure"),
            "the blunt clear must be total, not the project-scoped-only re-arm"
        );
    }

    #[tokio::test]
    /// The fix for docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md:
    /// the guide-hint ledger is persisted per CLAUDE_CODE_SESSION_ID, so a `/mcp`
    /// reconnect (which re-spawns the codescout process) reloads it instead of
    /// re-injecting every guide body the conversation already holds.
    ///
    /// No `#[serial]`, no `set_var`: the session id and librarian db are INJECTED, so
    /// this test's state cannot collide with any other's. See [`ServerEnv`].
    async fn guide_ledger_survives_mcp_restart() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        // Pin the ledger key explicitly (CODESCOUT_SESSION_ID) so both server
        // incarnations key on the same persisted file — the ledger no longer
        // falls back to a random uuid, it goes Anonymous instead, which would
        // make this test non-deterministic-by-omission rather than pass.
        let env_for = |session: &str| ServerEnv {
            session_id_explicit: Some(session.to_string()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let build = |session: &'static str| {
            let dir_path = dir.path().to_path_buf();
            let env = env_for(session);
            async move {
                let agent = crate::agent::Agent::new(Some(dir_path)).await.unwrap();
                CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await
            }
        };

        // First MCP process: record a guide topic (write-through persists to disk).
        {
            let server = build("restart-survival-session").await;
            assert!(
                !server.guide_hints_emitted.lock().contains("librarian"),
                "a fresh session starts with an empty ledger"
            );
            server
                .guide_hints_emitted
                .lock()
                .insert("librarian".to_string());
        } // server dropped — simulates a /mcp reconnect re-spawning the process.

        // Second MCP process, same project + same session id: must RELOAD the
        // persisted ledger, not re-arm. This is the regression bar.
        let server2 = build("restart-survival-session").await;
        assert!(
            server2.guide_hints_emitted.lock().contains("librarian"),
            "guide ledger must survive MCP restart within one conversation"
        );

        // A different conversation id on the same project sees a fresh ledger —
        // concurrent CC windows must not inherit each other's emitted set.
        let server3 = build("other-session").await;
        assert!(
            !server3.guide_hints_emitted.lock().contains("librarian"),
            "a different session must not inherit another session's ledger"
        );
    }
    #[tokio::test]
    /// The Phase A debt, recorded rather than patched by Phase A's whole-branch
    /// review: one conversation, MCP server restarts against a DIFFERENT
    /// `--project`. Before this task the session-keyed ledger carried the first
    /// project's topics into the second server's construction, so the reloaded
    /// ledger already `contains(SESSION_OPENING_GUIDE)` and
    /// `project-activation-bootstrap` is suppressed for a project that never
    /// received it.
    async fn a_restart_against_a_different_project_reopens_the_session() {
        let ledger_dir = tempfile::tempdir().unwrap();
        let dir_a = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();

        // Same session id, same ledger dir for both constructions — only the
        // project root differs between the two server incarnations.
        let session_id = "different-project-reopen-session";
        let env_for = |project_dir: &std::path::Path| {
            let ws_path = project_dir.join("librarian-workspace.toml");
            std::fs::write(&ws_path, "").unwrap();
            ServerEnv {
                session_id_explicit: Some(session_id.to_string()),
                guide_hints_dir: Some(ledger_dir.path().to_path_buf()),
                servers_dir: Some(ledger_dir.path().join("servers")),
                librarian: crate::librarian::LibrarianEnv {
                    workspace: Some(ws_path),
                    db: Some(project_dir.join("librarian.db")),
                    ..Default::default()
                },
                ..Default::default()
            }
        };
        let build = |project_dir: std::path::PathBuf, env: ServerEnv| async move {
            let agent = crate::agent::Agent::new(Some(project_dir)).await.unwrap();
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await
        };

        // 1. Build against dir_a, drive a call, confirm the opener fired.
        {
            let server = build(dir_a.path().to_path_buf(), env_for(dir_a.path())).await;
            let result = server
                .call_tool_by_name("tree", json!({"path": "."}))
                .await
                .expect("dispatch ok");
            assert!(
                content_carries_guide_body(&result.content, crate::prompts::SESSION_OPENING_GUIDE),
                "precondition: the first call in a fresh session must fire the opener"
            );
        } // dropped — simulates the MCP server restarting against a different --project.

        // 2. Build against dir_b, the SAME session id: reloads the session-keyed
        // ledger built up under dir_a.
        let server2 = build(dir_b.path().to_path_buf(), env_for(dir_b.path())).await;

        // 3. Before any call on server2: the reloaded ledger must NOT carry the
        // bootstrap topic forward from dir_a — that suppression is what this
        // task closes. Safe to assert on ledger state here specifically because
        // no opener has fired yet on this server.
        assert!(
            !server2
                .guide_hints_emitted
                .lock()
                .contains(crate::prompts::SESSION_OPENING_GUIDE),
            "a restart against a different project must re-open the session for \
             the bootstrap topic, not inherit the previous project's ledger state"
        );

        // And a call against the new server must re-emit the guide body.
        // Asserted on the RESPONSE, not ledger state: `Tool::call_content`
        // re-inserts the bootstrap topic as part of firing it, so a post-call
        // ledger check would pass whether or not a re-arm actually happened.
        let result2 = server2
            .call_tool_by_name("tree", json!({"path": "."}))
            .await
            .expect("dispatch ok");
        assert!(
            content_carries_guide_body(&result2.content, crate::prompts::SESSION_OPENING_GUIDE),
            "the new project's server must re-send the bootstrap guide on its first call"
        );
    }

    #[tokio::test]
    /// The accepted cost of Ruling 2, pinned so it is a decision and not a
    /// surprise: a SAME-project reconnect also re-arms the bootstrap — one
    /// guide body re-sent per `/mcp` reconnect, deliberately, rather than
    /// persisting a project root and a third on-disk ledger shape. The second
    /// assertion is what keeps this honest: if the startup path used `clear()`
    /// instead of `re_arm()`, the first assertion alone would not notice —
    /// only the survival of a tool-contract topic tells the two apart.
    async fn a_same_project_restart_also_rearms_the_bootstrap() {
        let ledger_dir = tempfile::tempdir().unwrap();
        let dir_a = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let ws_path = dir_a.path().join("librarian-workspace.toml");
        std::fs::write(&ws_path, "").unwrap();

        let session_id = "same-project-rearm-session";
        let env = ServerEnv {
            session_id_explicit: Some(session_id.to_string()),
            guide_hints_dir: Some(ledger_dir.path().to_path_buf()),
            servers_dir: Some(ledger_dir.path().join("servers")),
            librarian: crate::librarian::LibrarianEnv {
                workspace: Some(ws_path),
                db: Some(dir_a.path().join("librarian.db")),
                ..Default::default()
            },
            ..Default::default()
        };
        let build = || {
            let project_dir = dir_a.path().to_path_buf();
            let env = env.clone();
            async move {
                let agent = crate::agent::Agent::new(Some(project_dir)).await.unwrap();
                CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await
            }
        };

        // First server: fire the opener, then record a tool-contract topic the
        // model already holds — this must survive the reconnect below.
        {
            let server = build().await;
            server
                .call_tool_by_name("tree", json!({"path": "."}))
                .await
                .expect("dispatch ok");
            server
                .guide_hints_emitted
                .lock()
                .insert("librarian".to_string());
        } // dropped — simulates an /mcp reconnect against the SAME project.

        let server2 = build().await;

        // Bootstrap absent before any call — the reconnect re-opened the session.
        assert!(
            !server2
                .guide_hints_emitted
                .lock()
                .contains(crate::prompts::SESSION_OPENING_GUIDE),
            "a same-project reconnect must also re-arm the bootstrap topic"
        );
        // The tool-contract topic must survive: this is what distinguishes a
        // surgical re-arm from a blunt clear().
        assert!(
            server2.guide_hints_emitted.lock().contains("librarian"),
            "a re-arm must not touch tool-contract topics the model already holds \
             — if this fails, the startup path used clear() instead of re_arm()"
        );
    }

    #[tokio::test]
    /// A new conversation holds nothing, so a session change re-arms the WHOLE
    /// ledger — not just the project-scoped topic. This is the `/clear` fix:
    /// docs/issues/archive/2026-08-18-clear-leaves-mcp-session-id-stale.md.
    ///
    /// No `#[serial]`, no `set_var`: session id and directories are INJECTED.
    async fn session_change_rearms_everything() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let servers = tempfile::tempdir().unwrap();

        let env = ServerEnv {
            session_id_explicit: Some("conv-A".to_string()),
            servers_dir: Some(servers.path().to_path_buf()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        server
            .guide_hints_emitted
            .lock()
            .insert("librarian".to_string());
        server
            .guide_hints_emitted
            .lock()
            .insert("progressive-disclosure".to_string());

        // The companion hook stamps our slot with a DIFFERENT conversation.
        let slot = servers.path().join(format!("{}.json", std::process::id()));
        let mut entry: crate::tools::rendezvous::Entry =
            serde_json::from_str(&std::fs::read_to_string(&slot).unwrap()).unwrap();
        entry.session = Some("conv-B".to_string());
        entry.hook_at = Some(chrono::Utc::now());
        std::fs::write(&slot, serde_json::to_string(&entry).unwrap()).unwrap();
        // Deterministic rather than sleep-dependent: `poll` short-circuits on an
        // unchanged mtime, and mtime resolution is coarse on some filesystems.
        filetime::set_file_mtime(&slot, filetime::FileTime::from_unix_time(2_000_000_000, 0))
            .unwrap();

        server.rendezvous_poll_for_test();

        let ledger = server.guide_hints_emitted.lock();
        assert!(
            !ledger.contains("librarian"),
            "a new conversation re-arms every topic"
        );
        assert!(
            !ledger.contains("progressive-disclosure"),
            "re-arm must be total, not just the project-scoped topic"
        );
        // Storage has to follow the conversation too. A plain `clear()` here
        // would look identical to the two assertions above while leaving the
        // ledger writing conv-B's topics into conv-A's file — which is the
        // "degrade to SUPPRESSING" direction: resuming conv-A would then find
        // guides marked delivered that it never received.
        assert_eq!(
            ledger.path_for_test(),
            Some(dir.path().join("guide_hints").join("conv-B.json").as_path()),
            "the ledger must repoint at the new conversation's file"
        );
    }

    #[tokio::test]
    /// The Agent-Agnostic half of the contract: with no companion hook writing
    /// into the slot, the server must NOT re-arm. Guides stay suppressed for the
    /// conversation that already received them, and the anonymous-tier idle TTL
    /// is what eventually catches `/clear` — one interval late.
    ///
    /// Kills a mutation that re-arms on every poll, which no assertion in
    /// `session_change_rearms_everything` can see.
    async fn an_unstamped_slot_leaves_the_ledger_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let servers = tempfile::tempdir().unwrap();

        let env = ServerEnv {
            session_id_explicit: Some("conv-A".to_string()),
            servers_dir: Some(servers.path().to_path_buf()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        server
            .guide_hints_emitted
            .lock()
            .insert("librarian".to_string());

        server.rendezvous_poll_for_test();
        server.rendezvous_poll_for_test();

        assert!(
            server.guide_hints_emitted.lock().contains("librarian"),
            "no hook stamp ⇒ no re-arm; the server must not depend on the companion"
        );
        assert!(
            !server.guide_hints_emitted.lock().rendezvous_active(),
            "no hook stamp ⇒ the gate stays closed; a hardcoded `true` here \
             would send Task 3 down the precise path on a hookless client"
        );
    }

    #[tokio::test]
    /// The wiring itself: an ordinary tool call must poll the rendezvous.
    ///
    /// The two tests above drive `poll_rendezvous` directly, so deleting the
    /// call from `call_tool_inner` — which is the entire point of the task —
    /// leaves both of them green. This one goes through the real request path,
    /// the same funnel MCP requests and peer-served calls both use.
    async fn a_tool_call_polls_the_rendezvous_and_re_arms() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let servers = tempfile::tempdir().unwrap();

        let env = ServerEnv {
            session_id_explicit: Some("conv-A".to_string()),
            servers_dir: Some(servers.path().to_path_buf()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        server
            .guide_hints_emitted
            .lock()
            .insert("librarian".to_string());

        let slot = servers.path().join(format!("{}.json", std::process::id()));
        let mut entry: crate::tools::rendezvous::Entry =
            serde_json::from_str(&std::fs::read_to_string(&slot).unwrap()).unwrap();
        entry.session = Some("conv-B".to_string());
        entry.hook_at = Some(chrono::Utc::now());
        std::fs::write(&slot, serde_json::to_string(&entry).unwrap()).unwrap();
        filetime::set_file_mtime(&slot, filetime::FileTime::from_unix_time(2_000_000_000, 0))
            .unwrap();

        let result = server
            .call_tool_by_name("tree", json!({ "path": "." }))
            .await
            .expect("dispatch ok");
        assert!(result.is_error.is_none_or(|e| !e), "tree should succeed");

        // THE POSITION GUARD — this is the assertion that pins WHERE the poll
        // sits, and the ledger assertion below cannot do its job.
        //
        // Inspecting the ledger AFTER the call is satisfied by BOTH orderings:
        // polling before `tool.call_content` and polling after it leave the same
        // end state, which is why moving `self.poll_rendezvous()` below the tool
        // call left all 4050 lib tests green. This assertion reads THIS
        // response instead. A re-armed ledger is empty, and an empty ledger
        // always lacks `SESSION_OPENING_GUIDE` — the opener's trigger (the
        // `!emitted.contains(SESSION_OPENING_GUIDE)` check in
        // `Tool::call_content`, `src/tools/core/types.rs`) — so the opener
        // can only ride this very response if the rekey landed first.
        //
        // The off-by-one is not cosmetic: with the poll after the tool ran, the
        // first call following a `/clear` answers from the STALE conv-A ledger
        // and suppresses a guide the new conversation never received — one lost
        // re-send per `/clear`, in the degrade-to-SUPPRESSING direction the
        // phase's global constraints forbid, and the exact defect this phase
        // exists to remove.
        //
        // Probed via the APPENDED GUIDE BODY, not via `_guide_hint`. Measured,
        // not assumed: `tree` is `OutputForm::Text` with a `format_compact`
        // (src/tools/tree.rs:58-69), so its primary block is rendered text and
        // the `_guide_hint` field injected into the `Value` never reaches the
        // wire — `extract_hint` returns `None` here even when the opener fired.
        // The second block is pushed regardless of output form, and it is also
        // the stronger probe: the body is what actually costs tokens and what
        // the model reads.
        let marker = format!(
            "<!-- auto-injected get_guide('{}')",
            crate::prompts::SESSION_OPENING_GUIDE
        );
        assert!(
            all_text(&result).contains(&marker),
            "the re-arm must land BEFORE call_content, so this same response \
             carries the opener's guide body; blocks: {:?}",
            result
                .content
                .iter()
                .map(|c| c
                    .as_text()
                    .map(|t| t.text.chars().take(80).collect::<String>()))
                .collect::<Vec<_>>()
        );

        assert!(
            !server.guide_hints_emitted.lock().contains("librarian"),
            "a tool call must poll the rendezvous and re-arm for the new conversation"
        );
    }

    #[tokio::test]
    /// Regression for
    /// docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md:
    /// `cc_session_id` used to be a construction-time snapshot, so a `/clear` (or a
    /// subagent reusing this live process) kept attributing calls to the OLD
    /// conversation forever. The fix reads the rendezvous's current id per call — the
    /// same signal the ledger already polls — falling back to the snapshot only when
    /// the rendezvous has none.
    async fn a_tool_call_after_a_rendezvous_rekey_writes_the_new_cc_session_id() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let servers = tempfile::tempdir().unwrap();

        let env = ServerEnv {
            servers_dir: Some(servers.path().to_path_buf()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        let result = server
            .call_tool_by_name("tree", json!({ "path": "." }))
            .await
            .expect("dispatch ok");
        assert!(result.is_error.is_none_or(|e| !e), "tree should succeed");

        let db = dir.path().join(".codescout").join("usage.db");
        let last_cc_session_id = |db: &std::path::Path| -> String {
            rusqlite::Connection::open(db)
                .unwrap()
                .query_row(
                    "SELECT cc_session_id FROM tool_calls ORDER BY id DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
        let before = last_cc_session_id(&db);

        // The companion hook stamps our slot with a NEW conversation — the `/clear`
        // (or subagent-reusing-a-live-server) case.
        let slot = servers.path().join(format!("{}.json", std::process::id()));
        let mut entry: crate::tools::rendezvous::Entry =
            serde_json::from_str(&std::fs::read_to_string(&slot).unwrap()).unwrap();
        entry.session = Some("conv-after-clear".to_string());
        entry.hook_at = Some(chrono::Utc::now());
        std::fs::write(&slot, serde_json::to_string(&entry).unwrap()).unwrap();
        filetime::set_file_mtime(&slot, filetime::FileTime::from_unix_time(2_000_000_000, 0))
            .unwrap();

        let result = server
            .call_tool_by_name("tree", json!({ "path": "." }))
            .await
            .expect("dispatch ok");
        assert!(result.is_error.is_none_or(|e| !e), "tree should succeed");

        let after = last_cc_session_id(&db);

        assert_ne!(
            before, after,
            "a rendezvous re-key must change the id written to tool_calls, not just the ledger"
        );
        assert_eq!(
            after, "conv-after-clear",
            "the newly stamped conversation must be what gets recorded, not the \
             construction-time cc_session_id snapshot"
        );
    }

    #[tokio::test]
    /// Phase C's first production caller of `Rendezvous::is_active()`: an
    /// ordinary tool call must copy it onto the ledger, not just poll for a
    /// session change. Task 3's re-arm predicate gates on
    /// `GuideLedger::rendezvous_active()`, so this proves the wiring reaches it
    /// through the real request path, not just through a direct unit call.
    async fn a_tool_call_copies_rendezvous_activity_onto_the_ledger() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let servers = tempfile::tempdir().unwrap();

        let env = ServerEnv {
            session_id_explicit: Some("conv-A".to_string()),
            servers_dir: Some(servers.path().to_path_buf()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        assert!(
            !server.guide_hints_emitted.lock().rendezvous_active(),
            "no hook has stamped our slot yet"
        );

        let slot = servers.path().join(format!("{}.json", std::process::id()));
        let mut entry: crate::tools::rendezvous::Entry =
            serde_json::from_str(&std::fs::read_to_string(&slot).unwrap()).unwrap();
        entry.hook_at = Some(chrono::Utc::now());
        std::fs::write(&slot, serde_json::to_string(&entry).unwrap()).unwrap();
        filetime::set_file_mtime(&slot, filetime::FileTime::from_unix_time(2_000_000_000, 0))
            .unwrap();

        let result = server
            .call_tool_by_name("tree", json!({ "path": "." }))
            .await
            .expect("dispatch ok");
        assert!(result.is_error.is_none_or(|e| !e), "tree should succeed");

        assert!(
            server.guide_hints_emitted.lock().rendezvous_active(),
            "an ordinary tool call must copy Rendezvous::is_active onto the ledger"
        );
    }

    /// The ledger must NOT live under the project root any more: a git worktree,
    /// a cross-project session, and a cwd that is not a project all resolve to
    /// different roots for the same conversation, and the ledger has to follow
    /// the conversation. See the spec's §2.
    ///
    /// Both halves matter: the negative assertion alone survives the binding being
    /// hard-coded to `None`, `persist()` becoming a no-op, or `env.guide_hints_dir`
    /// being ignored in favour of the real-state fallback — all three still leave
    /// `<project>/.codescout/guide_hints` absent. The positive assertion is what
    /// catches the third case (an injection silently dropped), which the negative
    /// assertion alone lets through as a false green.
    #[tokio::test]
    async fn guide_ledger_lives_in_the_injected_dir_not_under_the_project_root() {
        let (dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        let tool = tool_by_name(&server, "run_command");
        let _ = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();

        let in_project = dir.path().join(".codescout").join("guide_hints");
        assert!(
            !in_project.exists(),
            "guide_hints must no longer be written under the project root"
        );

        let injected = dir.path().join("guide_hints");
        assert!(
            injected
                .read_dir()
                .map(|mut e| e.next().is_some())
                .unwrap_or(false),
            "the ledger must land in the injected per-user directory"
        );
    }

    /// Inverse of the test above: with no identity available at all, the ledger
    /// must stay fully in-process and must never touch the injected per-user
    /// directory. `make_server()` always injects an identity (`session_id_explicit`),
    /// so this test builds its own `ServerEnv` directly — the one path in this
    /// module that deliberately withholds one.
    ///
    /// Kills the mutation `None => GuideLedger::load("anonymous", guide_hints_dir)`:
    /// that keeps `session_key` reading `Anonymous` (so a bare identity assertion
    /// alone would not catch it) while persisting every anonymous conversation
    /// under one constant, shared file — letting one conversation's emitted set
    /// suppress another's guides, exactly what "degrade to re-sending, never to
    /// suppressing" forbids.
    #[tokio::test]
    async fn no_identity_ledger_never_touches_the_injected_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        let env = ServerEnv {
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;

        // Not asserted directly: `server.session_key` is not read anywhere yet
        // (Task 1 only stores it — see the `#[expect(dead_code)]` on the field),
        // and a test-side read here would satisfy that expectation early and
        // mask the day a later task's *production* code first reads it. The
        // ledger-placement assertions below are what this test is for.

        // Drive a guide-eligible call — the session opener fires on the first
        // call of any kind (see `Tool::call_content`), the same trigger the
        // positive-dir test above relies on.
        let ctx = shared_ctx(&server);
        let tool = tool_by_name(&server, "run_command");
        let _ = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();
        assert!(
            !server.guide_hints_emitted.lock().is_empty(),
            "the session opener must still fire in-memory for an anonymous session"
        );

        let injected = dir.path().join("guide_hints");
        assert!(
            !injected.exists(),
            "an anonymous session must never create or write the injected \
             per-user guide_hints directory"
        );
    }

    /// `:341-349` (the anonymous-arm construction) resolve `idle_ttl` from
    /// `env.guide_idle_ttl.unwrap_or(DEFAULT_IDLE_TTL_SECS)`, gated by a
    /// `!idle_ttl.is_zero()` guard, before ever calling `GuideLedger::anonymous`.
    /// `GuideLedger::anonymous`'s own unit tests (in `guide_ledger.rs`) build a
    /// `GuideLedger` directly and can't see whether *production* construction
    /// ever actually reaches it with `Some(ttl)` — this test drives the real
    /// `ServerEnv` → `CodeScoutServer::from_parts_with_env` path with NO
    /// explicit `guide_idle_ttl` override, so the 2h default must survive
    /// construction intact.
    ///
    /// Kills three independent mutations to the construction site, all of
    /// which collapse the anonymous ledger to a permanently un-expiring one:
    /// - `None => GuideLedger::anonymous(None)` (drop the ttl argument
    ///   entirely) — no TTL ever reaches the ledger.
    /// - `unwrap_or(Duration::ZERO)` — `idle_ttl` becomes zero, and the
    ///   (unmutated) guard turns a zero ttl into `None` too.
    /// - the guard inverted to `idle_ttl.is_zero().then_some(idle_ttl)` — for
    ///   the non-zero default `idle_ttl`, `is_zero()` is `false`, so the
    ///   inverted guard also yields `None`.
    ///
    /// A topic backdated 3h — past the 2h default — must re-arm under the
    /// correct implementation and must NOT re-arm under any of the three.
    #[tokio::test]
    async fn anonymous_session_default_ttl_expires_a_stamp_backdated_past_two_hours() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        let env = test_env(dir.path()); // no `guide_idle_ttl` override — exercises the default
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;

        {
            let mut ledger = server.guide_hints_emitted.lock();
            ledger.insert("librarian".to_string());
            ledger.backdate_for_test("librarian", chrono::Duration::hours(3));
        }

        let rearmed = server.guide_hints_emitted.lock().tick();
        assert_eq!(
            rearmed, 1,
            "a topic backdated 3h past the 2h default TTL must re-arm"
        );
        assert!(!server.guide_hints_emitted.lock().contains("librarian"));
    }

    /// The `CODESCOUT_GUIDE_TTL_SECS=0` opt-out (`ServerEnv.guide_idle_ttl =
    /// Some(Duration::ZERO)`) must mean "never expire, accepting starvation" —
    /// not "expire on every tick." The plan explicitly rejected renaming it to
    /// `=never` because the `=0` spelling's incidental other reading ("expire
    /// immediately") is the one behavior that must NEVER happen: it would
    /// re-arm every topic on every guide-eligible call, a token flood.
    ///
    /// Kills the guard-inverted mutation from the opposite direction the test
    /// above exercises: with a non-zero default `idle_ttl`, inversion yields
    /// `None` (caught above); with `idle_ttl` already zero, inversion instead
    /// yields `Some(Duration::ZERO)`, which is the token-flood behavior this
    /// test rules out directly.
    #[tokio::test]
    async fn anonymous_session_zero_ttl_opt_out_never_re_arms() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        let env = ServerEnv {
            guide_idle_ttl: Some(std::time::Duration::ZERO),
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let lsp = LspManager::new_arc();
        let server = CodeScoutServer::from_parts_with_env(agent, lsp, false, env).await;

        {
            let mut ledger = server.guide_hints_emitted.lock();
            ledger.insert("librarian".to_string());
            ledger.backdate_for_test("librarian", chrono::Duration::days(100));
        }

        let rearmed = server.guide_hints_emitted.lock().tick();
        assert_eq!(
            rearmed, 0,
            "the =0 opt-out must never re-arm, however stale"
        );
        assert!(server.guide_hints_emitted.lock().contains("librarian"));
    }

    /// The compaction side of the fix: `/compact` summarizes the guide bodies
    /// out of context, so `workspace(post_compact=true)` must clear the ledger to
    /// let them re-inject. A bare `/mcp` restart is not the same event: it
    /// re-arms the session-opening guide only, on any non-empty reloaded ledger
    /// (see the constructor's re-arm below); every other topic survives a
    /// restart, but compaction clears them all.
    #[tokio::test]
    async fn post_compact_rearms_guide_hints() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        let artifact = tool_by_name(&server, "artifact");
        let workspace = tool_by_name(&server, "workspace");

        // Emit once — topic is now in the ledger.
        let _ = artifact
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();

        // Compaction signal: must clear the ledger.
        let _ = workspace
            .call_content(json!({"action": "status", "post_compact": true}), &ctx)
            .await
            .unwrap();

        // Next call re-emits, because the body was summarized out of context.
        let result = artifact
            .call_content(json!({"action": "find", "kind": "tracker"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&result)
                .unwrap_or_default()
                .contains("librarian"),
            "post_compact must re-arm guide hints so they re-inject after compaction"
        );
    }

    #[tokio::test]
    async fn run_command_without_overflow_no_progressive_hint() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "run_command");
        let result = tool
            .call_content(json!({"command": "echo small"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&result).is_none(),
            "small output should not trigger progressive-disclosure hint"
        );
    }

    // Previously #[ignore]d on Windows for two reasons, both now resolved:
    // `yes`/`head` were Unix-only under cmd.exe (commands run through Git Bash
    // on both platforms now), and inject_tee's path validator rejected every
    // Windows temp path (it accepts the `:` drive letter and the `~` of 8.3
    // short names, and the path is rendered forward-slashed). Re-enabled so the
    // overflow -> progressive-disclosure-hint path is actually covered here.
    #[tokio::test]
    async fn run_command_with_overflow_emits_progressive_hint_once() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        warm_ledger(&ctx);
        let tool = tool_by_name(&server, "run_command");
        let big = tool
            .call_content(json!({"command": "yes filler | head -2000"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&big)
                .unwrap_or_default()
                .contains("progressive-disclosure"),
            "overflowing output should emit progressive-disclosure hint; got: {}",
            render_content(&big)
        );
        let second = tool
            .call_content(json!({"command": "yes filler | head -2000"}), &ctx)
            .await
            .unwrap();
        assert!(
            extract_hint(&second).is_none(),
            "second overflow must not re-emit the hint; got: {}",
            render_content(&second)
        );
    }
    // --- Section-grain delivery (Task 8) ---------------------------------
    //
    // `GUIDE_INDEX.declares(topic)` is true only for `librarian` in Phase 1.
    // These tests exercise the section-slice path for that topic and pin the
    // whole-topic path (byte-identical to pre-Task-8 behaviour) for a
    // non-declaring topic.

    /// Create a real tracker artifact via the `artifact` tool's `create` action
    /// and return its id. `append_entry`/`state_at` need a real, existing
    /// artifact to succeed — `call_content`'s guide-injection logic only
    /// runs on the underlying tool call's SUCCESS path (the `?` on
    /// `self.call(...)` short-circuits past it on any error), so an
    /// unmatched-shape or section-slice test that wants genuine guide
    /// delivery cannot use a placeholder id.
    ///
    /// Declares `entry_prefix: "T"` via `extra` — a prose-ledger
    /// `append_entry` call (no `entry_collection`) reads its id namespace
    /// from frontmatter (`allocate_entry_id`) and refuses with a
    /// `RecoverableError` JSON envelope (`ok: false`) otherwise. That
    /// envelope is itself a SUCCESSFUL `Tool::call` return (not a Rust
    /// `Err`), but `call_content`'s guide logic only fires past a genuine
    /// tool-level success — discovered by adding a temporary `eprintln!` in
    /// `guide_blocks_for` and seeing zero output for the append_entry call:
    /// the underlying `append_entry::call` was itself returning
    /// `Err(RecoverableError)` (`allocate_entry_id: ... does not declare an
    /// entry_prefix`), which `call_content`'s `?` on `self.call(...)`
    /// propagates straight past the hint computation.
    async fn create_tracker(server: &CodeScoutServer, rel_path: &str) -> String {
        let out = call_tool(
            server,
            "artifact",
            json!({
                "action": "create",
                "rel_path": rel_path,
                "kind": "tracker",
                "title": "section-grain fixture",
                "body": "fixture body",
                "extra": {"entry_prefix": "T"}
            }),
        )
        .await;
        let primary = out[0].as_text().expect("primary block is text");
        let v: Value = serde_json::from_str(&primary.text).expect("primary block is JSON");
        v["id"]
            .as_str()
            .expect("create response carries an id")
            .to_string()
    }

    #[tokio::test]
    async fn append_entry_receives_only_the_entry_sections_not_the_whole_librarian_guide() {
        let (_dir, server) = make_server().await;
        // Deliberately NOT under docs/trackers/ or docs/issues/: `names_tracker_path`
        // (src/librarian/adapter.rs) would route every subsequent call naming this
        // artifact to the `tracker-conventions` topic instead of `librarian` —
        // `tracker-conventions` declares no sections in Phase 1, so the
        // `artifact.append_entry` section this test targets would never fire.
        let id = create_tracker(&server, "docs/specs/fixture-a.md").await;
        let out = call_tool(
            &server,
            "artifact",
            json!({"action": "append_entry", "id": id, "id_prefix": "T"}),
        )
        .await;
        let guide = guide_blocks(&out).join("");
        assert!(
            guide.contains("don't hand-maintain the table"),
            "expected the append_entry section, got: {}",
            guide.chars().take(400).collect::<String>()
        );
        let whole = crate::prompts::topic_body("librarian").unwrap();
        assert!(
            guide.len() < whole.len() / 2,
            "delivered {} B of a {} B guide — section grain is not engaged",
            guide.len(),
            whole.len()
        );
        // GuideDeliveryShape::Section must produce a hint distinguishable from
        // Whole — a collapse back to the single pre-fix "Full guide auto-injected"
        // string would pass every other assertion here while re-introducing a
        // hint that lies about what shipped (a slice, not the full topic).
        let hint = extract_hint(&out).unwrap_or_default();
        assert!(
            hint.contains("Section(s) of"),
            "expected a Section-shape hint, got: {hint}"
        );
    }

    #[tokio::test]
    async fn a_second_differently_shaped_call_delivers_a_different_section() {
        let (_dir, server) = make_server().await;
        // See the rel_path comment in the append_entry test above — a docs/trackers/
        // path here would route to `tracker-conventions` (no declared sections) and
        // starve both calls of a section-grain hint.
        let id = create_tracker(&server, "docs/specs/fixture-b.md").await;
        let first = guide_blocks(
            &call_tool(
                &server,
                "artifact",
                json!({"action": "append_entry", "id": id, "id_prefix": "T"}),
            )
            .await,
        )
        .join("");
        let second =
            guide_blocks(&call_tool(&server, "artifact", json!({"action": "find"})).await).join("");
        assert!(
            !second.is_empty(),
            "per-section ledger must allow a second slice"
        );
        assert_ne!(first, second);
    }

    #[tokio::test]
    async fn the_same_shape_twice_delivers_nothing_the_second_time() {
        let (_dir, server) = make_server().await;
        let _ = call_tool(&server, "artifact", json!({"action": "find"})).await;
        let again =
            guide_blocks(&call_tool(&server, "artifact", json!({"action": "find"})).await).join("");
        assert!(again.is_empty());
    }

    /// A declared section is still reachable after the result-based topic is spent.
    ///
    /// The two topic sources disagree for any librarian call touching
    /// `docs/trackers/` or `docs/issues/`: `names_tracker_path` sends it to
    /// `tracker-conventions` (which declares nothing, so it ships WHOLE), while
    /// the call's shape is declared by a `librarian.md` section. Before the
    /// fallthrough only the first source was ever consulted, so once
    /// `tracker-conventions` was spent such calls delivered **nothing at all**
    /// and their sections were unreachable for the rest of the session.
    ///
    /// That is not a hypothetical: two tests above route around it explicitly,
    /// putting their fixtures under `docs/specs/` with the comment *"a
    /// docs/trackers/ path here would route to `tracker-conventions` ... and
    /// starve both calls of a section-grain hint"*. This test walks into it on
    /// purpose.
    ///
    /// Both halves are asserted together because each is monotone in a
    /// direction the other is blind to. Dropping the fallthrough leaves half 1
    /// passing; letting declarations WIN instead of falling through leaves half
    /// 2 passing while reverting `32736ca0` — the fix that routes tracker work
    /// to the only guide teaching `**Valid:**` and the entry-validity checks.
    ///
    /// Mutations that must kill this:
    /// - `candidates` truncated to `vec![content_topic]` → half 2 dies.
    /// - declaring topic pushed FIRST instead of appended → half 1 dies.
    #[tokio::test]
    async fn a_declared_section_still_arrives_once_the_content_topic_is_spent() {
        let (_dir, server) = make_server().await;

        // 1 — a create under docs/trackers/ must still deliver the tracker guide
        //     whole. This is the behaviour `32736ca0` bought and
        //     `an_artifact_call_naming_a_tracker_path_delivers_the_tracker_guide`
        //     protects; the fallthrough must not cost it.
        let created = call_tool(
            &server,
            "artifact",
            json!({
                "action": "create",
                "rel_path": "docs/trackers/fallthrough-probe.md",
                "kind": "tracker",
                "title": "fallthrough probe",
                "body": "probe body"
            }),
        )
        .await;
        assert!(
            content_carries_guide_body(&created, "tracker-conventions"),
            "the result-based topic must still go first and still ship whole, got: {}",
            guide_blocks(&created)
                .join("")
                .chars()
                .take(300)
                .collect::<String>()
        );
        let id = serde_json::from_str::<Value>(&created[0].as_text().unwrap().text)
            .expect("primary block is JSON")["id"]
            .as_str()
            .expect("create returns an id")
            .to_string();

        // 2 — a `get` on that same tracker. Same result-based topic, now spent,
        //     so this call used to deliver nothing. `artifact.get` is declared by
        //     librarian.md § Artifact Model, which should now arrive.
        let out = call_tool(&server, "artifact", json!({"action": "get", "id": id})).await;
        let guide = guide_blocks(&out).join("");
        assert!(
            guide.contains("`id` and `rel_path` together are the canonical identifiers"),
            "the declared section's BODY must arrive once the content topic is \
             spent — asserting body text, not the `§ Artifact Model` marker, which \
             an empty section would satisfy. got {} B: {}",
            guide.len(),
            guide.chars().take(300).collect::<String>()
        );
        assert!(
            !content_carries_guide_body(&out, "tracker-conventions"),
            "one topic per call — the spent whole guide must not ride along again"
        );
    }

    /// A call that delivers nothing must not refresh the stamps it did not use.
    ///
    /// `GuideLedger::insert` refreshes and persists on a repeat — deliberately,
    /// since the stamp means "last delivered" and that is what `expire_idle`
    /// reads. `guide_blocks_for` used it as its already-sent *test*, so an
    /// all-sections-already-sent call inserted once per matched section (the
    /// `if` gated only the `push`) and returned empty. The bill lands in a
    /// different place per tier, and neither tier pays both:
    ///
    /// - identified (`GuideLedger::load`, `path: Some`, `idle_ttl: None`) — a
    ///   staged write + rename per nothing-delivered call; the stamp itself is
    ///   never read for expiry, since `tick()` no-ops without a TTL.
    /// - anonymous (`path: None`, `idle_ttl: Some`) — `persist` returns early,
    ///   but the refresh defers re-arm, and re-arm is the only thing standing
    ///   between a second conversation on one process and permanent starvation.
    ///
    /// This asserts the tier-independent half: the stamp does not move. It is
    /// the observable both harms derive from, and it needs no TTL plumbing.
    ///
    /// Mutation that must kill this: restore `if emitted.insert(key)` as the
    /// loop's already-sent test in `guide_blocks_for`.
    #[tokio::test]
    async fn a_call_that_delivers_nothing_does_not_refresh_the_stamp() {
        let (_dir, server) = make_server().await;
        let ctx = shared_ctx(&server);

        let first =
            guide_blocks(&call_tool(&server, "artifact", json!({"action": "find"})).await).join("");
        assert!(
            !first.is_empty(),
            "setup: the first call must deliver a section for there to be a stamp to refresh"
        );

        // Age the librarian section stamps by an hour. Only those: `call_tool`
        // warms the opener slot, so an unfiltered sweep would also catch keys
        // this test is not about.
        let tracked: Vec<String> = {
            let mut led = ctx.guide_hints_emitted.lock();
            let keys: Vec<String> = led
                .stamps_for_test()
                .into_iter()
                .map(|(k, _)| k)
                .filter(|k| k.starts_with("librarian#"))
                .collect();
            assert!(
                !keys.is_empty(),
                "setup: expected at least one librarian section stamp, got none"
            );
            for k in &keys {
                led.backdate_for_test(k, chrono::Duration::hours(1));
            }
            keys
        };

        let again =
            guide_blocks(&call_tool(&server, "artifact", json!({"action": "find"})).await).join("");
        assert!(
            again.is_empty(),
            "setup: the repeat shape must deliver nothing, got {} B",
            again.len()
        );

        let led = ctx.guide_hints_emitted.lock();
        let stamps: std::collections::BTreeMap<String, _> =
            led.stamps_for_test().into_iter().collect();
        let now = chrono::Utc::now();
        for key in &tracked {
            let at = stamps
                .get(key)
                .unwrap_or_else(|| panic!("`{key}` vanished from the ledger"));
            assert!(
                now - *at > chrono::Duration::minutes(30),
                "`{key}` was refreshed by a call that delivered nothing. The stamp means \
                 \"last delivered\"; moving it on silence defers an anonymous-tier re-arm \
                 and rewrites the ledger file for zero bytes shipped."
            );
        }
    }

    #[tokio::test]
    async fn an_unmatched_shape_receives_the_preamble_not_the_whole_topic() {
        let (_dir, server) = make_server().await;
        // `state_at` is a real, low-volume action that genuinely succeeds
        // against a real artifact id; Task 6 declares nothing for it (only
        // `graft` and `state_at` are undeclared actions on `artifact`, and
        // `graft` additionally needs a SECOND real artifact to succeed —
        // `state_at` only needs one, plus a bare timestamp, since a missing
        // `commit`/`timestamp` short-circuits before the guide logic runs).
        let id = create_tracker(&server, "docs/trackers/fixture-c.md").await;
        let out = call_tool(
            &server,
            "artifact",
            json!({"action": "state_at", "artifact_id": id, "timestamp": 1_700_000_000_000i64}),
        )
        .await;
        let guide = guide_blocks(&out).join("");
        let entry = crate::prompts::guide_index::GUIDE_INDEX
            .topic("librarian")
            .unwrap();
        assert!(guide.contains(entry.preamble.trim()));
        assert!(
            guide.len() < 2000,
            "preamble fallback must be small, got {} B",
            guide.len()
        );
        // The one guard that actually catches deleting the pointer line
        // (`types.rs`'s preamble-fallback block): a bare `contains("get_guide")`
        // is vacuous here, because `librarian.md`'s own preamble text embeds
        // `see get_guide("tracker-conventions")` (lines 5-6) and is included
        // verbatim in the fallback block regardless of whether the pointer
        // line below it survives. Assert the emitted pointer sentence itself.
        assert!(
            guide.contains("Call `get_guide(\"librarian\")` for the full topic"),
            "fallback must point at the full topic with the actual emitted \
             pointer sentence, got: {}",
            guide.chars().take(600).collect::<String>()
        );
        // GuideDeliveryShape::Preamble must NOT reuse the old Whole-shape hint
        // string ("do not re-call get_guide") — that sentence is actively
        // backwards for the preamble fallback, whose entire point is that the
        // caller SHOULD re-call get_guide(topic) to get the rest.
        let hint = extract_hint(&out).unwrap_or_default();
        assert!(
            !hint.contains("do not re-call"),
            "preamble-shape hint must not tell the caller to skip get_guide, got: {hint}"
        );
    }

    #[tokio::test]
    async fn a_non_declaring_topic_is_byte_identical_to_today() {
        let (_dir, server) = make_server().await;
        // `symbols` routes to `symbol-navigation`, which has no declarations in
        // Phase 1. `path="."` (not `"src"`, which `make_server`'s fixture
        // tempdir does not contain) — any existing directory is enough for
        // the call to succeed, which is all this path needs.
        let out = call_tool(&server, "symbols", json!({"path": "."})).await;
        let guide = guide_blocks(&out);
        // `guide_blocks` is "everything after block 0" — for a fresh/unonboarded
        // tempdir, `symbols` also appends unrelated hint blocks (a
        // paths-are-relative-to notice, a project-status summary) that have
        // nothing to do with the guide-delivery system this test covers. Isolate
        // the one auto-injected guide block by its marker comment instead of
        // asserting on the raw trailing-block count, which those unrelated
        // blocks would otherwise inflate.
        let guide_shaped: Vec<&String> = guide
            .iter()
            .filter(|b| b.contains("<!-- auto-injected get_guide("))
            .collect();
        assert_eq!(
            guide_shaped.len(),
            1,
            "a non-declaring topic must ship exactly one whole-topic guide block, got {}: {:?}",
            guide_shaped.len(),
            guide
        );
        // Assert full equality against the exact wrapper `guide_block` builds
        // (`types.rs`), not a `contains()` check — `contains` passes even if
        // the wrapper text were RE-DERIVED with a one-character difference
        // from the pre-Task-8 original (the failure mode Task 8 review's
        // hazard 2 exists to catch: this branch must be a pass-through to the
        // original wrapper, never a re-implementation of it). This is the
        // sole guard on the Phase 1 containment property.
        let body = crate::prompts::topic_body("symbol-navigation").unwrap();
        let expected = format!(
            "<!-- auto-injected get_guide('symbol-navigation') — first call this session \
         that triggers the topic. Do NOT re-call get_guide for this topic. -->\n\
         \n\
         {body}\n\
         \n\
         <!-- end auto-injected get_guide('symbol-navigation') -->"
        );
        assert_eq!(*guide_shaped[0], expected);
    }

    #[tokio::test]
    async fn a_p50_session_stays_under_the_committed_guide_byte_ceiling() {
        // The p50 session issues 6 distinct artifact/librarian shapes (measured over
        // 105 main sessions). Today that draws the whole 20,545 B librarian guide on
        // the first call and nothing after. Section grain must land well under it.
        //
        // This ceiling is the mechanism that keeps the win from eroding: guides grow.
        // `tracker-conventions` gained bytes mid-study, and `iron-laws-detail` gained
        // 769 B (5d3f8ebe) during the half hour the spec was being written.
        const CEILING: usize = 12_000;

        let (_dir, server) = make_server().await;

        // A placeholder id="x" for every shape (the brief's literal sketch) does NOT
        // measure a real session: `call_content` only runs guide injection on the
        // underlying tool call's SUCCESS path (see `create_tracker`'s doc comment
        // above), and `get`/`update`/`append_entry`/`move` against a nonexistent id
        // all fail before that point — verified empirically, only `find` delivered
        // any bytes with the placeholder. A p50 session issuing these five mutating
        // shapes has necessarily just created (this session) or already has (a prior
        // session) a real artifact to target, so build one via a genuinely-succeeding
        // `create` call and thread its id through `get`/`update`/`append_entry`, with
        // `move` last since it re-keys the id. `find` needs no fixture.
        //
        // Every call below goes through `call_tool_checked`, not the plain
        // `call_tool` used elsewhere in this module: guide injection only fires on
        // `call_content`'s success path, so a silently-failed call reports 0 B —
        // character-identical to legitimate cross-call dedup. Without asserting
        // success per call, a broken `id` thread would silently zero out several
        // shapes and understate the real total.
        let mut total = 0usize;
        let mut shape_total = |out: &[rmcp::model::Content]| -> usize {
            let bytes: usize = guide_blocks(out)
                .iter()
                .filter(|b| b.contains("<!-- auto-injected get_guide("))
                .map(|b| b.len())
                .sum();
            total += bytes;
            bytes
        };

        let create_out = call_tool_checked(
            &server,
            "artifact",
            json!({
                "action": "create",
                "rel_path": "docs/specs/p50-fixture.md",
                "kind": "tracker",
                "title": "p50 ceiling fixture",
                "body": "fixture body",
                "extra": {"entry_prefix": "T"}
            }),
            "create",
        )
        .await;
        shape_total(&create_out);
        let primary = create_out[0].as_text().expect("primary block is text");
        let v: Value = serde_json::from_str(&primary.text).expect("primary block is JSON");
        let id = v["id"]
            .as_str()
            .expect("create response carries an id")
            .to_string();

        // `get` is the ONE shape expected to report 0 B here — not because the call
        // failed (checked below, same as every other shape) but because `create`'s
        // `Artifact Model` match already delivered the section `get` would draw, and
        // the guide ledger dedups within a session. Any OTHER shape reporting 0 B is
        // suspicious, not normal.
        shape_total(
            &call_tool_checked(
                &server,
                "artifact",
                json!({"action": "get", "id": id}),
                "get",
            )
            .await,
        );
        shape_total(
            &call_tool_checked(
                &server,
                "artifact",
                json!({"action": "update", "id": id, "patch": {"status": "active"}}),
                "update",
            )
            .await,
        );
        shape_total(
            &call_tool_checked(
                &server,
                "artifact",
                json!({"action": "append_entry", "id": id, "id_prefix": "T"}),
                "append_entry",
            )
            .await,
        );
        shape_total(
            &call_tool_checked(&server, "artifact", json!({"action": "find"}), "find").await,
        );
        shape_total(
                &call_tool_checked(
                    &server,
                    "artifact",
                    json!({"action": "move", "id": id, "new_rel_path": "docs/specs/p50-fixture-moved.md"}),
                    "move",
                )
                .await,
            );

        let whole = crate::prompts::topic_body("librarian").unwrap().len();
        assert!(
            total <= CEILING,
            "p50 session drew {total} B of guide (whole topic is {whole} B, ceiling \
             {CEILING} B, margin {} B). Raising CEILING is a spec amendment, not a \
             fix — it is not the remedy for this failure. The standing remedy is \
             decomposing § Body Editing Surfaces in the librarian guide, already \
             recorded in \
             `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` § Out of \
             scope for Phase 1. If that is not what happened here, check whether a \
             section grew past its own per-section cap (a separate gate/test) or a \
             `serves:` declaration is broader than intended.",
            CEILING.saturating_sub(total)
        );
        assert!(total > 0, "the session must still receive guidance");
    }
    /// The session-opener branch (`types.rs`'s `call_content`) inserts the bare
    /// `SESSION_OPENING_GUIDE` ledger key and always reports
    /// `GuideDeliveryShape::Whole`, on the assumption — stated only as a
    /// comment — that this topic never declares `##`/`###` sections in
    /// Phase 1. `guide_blocks_for` keys everything else at `topic#heading`;
    /// the two never collide TODAY only because that assumption holds. The
    /// day this topic gains a `serves:` declaration (a real Phase 3 plan),
    /// the opener would deliver the whole topic under the bare key AND
    /// `guide_blocks_for` would separately deliver its sections under
    /// `topic#heading` keys — double delivery, silently, because the two
    /// paths never meet on a shared key. Turn the comment into a gate so
    /// that day fails loudly here instead of shipping the double-delivery.
    #[test]
    fn session_opening_guide_never_declares_sections() {
        assert!(
            !crate::prompts::guide_index::GUIDE_INDEX
                .declares(crate::prompts::SESSION_OPENING_GUIDE),
            "SESSION_OPENING_GUIDE ('{}') now declares sections, but the opener branch \
             in `call_content` still keys it as a bare topic and reports \
             GuideDeliveryShape::Whole unconditionally — it will double-deliver \
             against `guide_blocks_for`'s `topic#heading` keys. Update the opener \
             branch to route through `guide_blocks_for` (or an equivalent \
             section-aware path) before removing this assertion.",
            crate::prompts::SESSION_OPENING_GUIDE
        );
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
