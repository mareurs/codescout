//! Core types: Tool trait, ToolContext, RecoverableError, Guidance, OutputGuard-adjacent
//! constants/helpers, ToolCapabilities, and Availability.

use std::sync::Arc;

use anyhow::Result;
use rmcp::model::Content;
use rmcp::service::RoleServer;
use rmcp::Peer;
use serde_json::Value;

use crate::agent::Agent;
use crate::lsp::LspProvider;

/// Maximum estimated tokens for inline tool output.
/// Content exceeding this is buffered and summarized.
/// Token estimate: ~4 bytes per token.
pub(crate) const MAX_INLINE_TOKENS: usize = 2_500;

/// Byte equivalent of MAX_INLINE_TOKENS — used for byte-budget arithmetic
/// in truncation code (run_command buffer-only paths).
pub(crate) const TOOL_OUTPUT_BUFFER_THRESHOLD: usize = MAX_INLINE_TOKENS * 4;

/// Byte budget for auto-chunked inline content. Set to 90% of
/// TOOL_OUTPUT_BUFFER_THRESHOLD to leave headroom for the JSON envelope
/// overhead (~500-1000 bytes for content/complete/next/shown_lines keys).
pub(crate) const INLINE_BYTE_BUDGET: usize = TOOL_OUTPUT_BUFFER_THRESHOLD * 9 / 10;

/// Soft line-count nudge for markdown default reads.
///
/// Files whose line count exceeds this threshold — but whose byte size still
/// fits `INLINE_BYTE_BUDGET` — get full content plus a focused-read hint.
/// Files larger than `INLINE_BYTE_BUDGET` are buffered regardless of line count.
pub(crate) const LINE_SOFT_CAP: usize = 150;

/// Heading-count gate for escalation to MAP shape. A markdown file with more
/// than this many headings is structurally a directory — content is skim-only
/// and the caller wants to pivot. Escalates Tier 2 → Tier 3 regardless of
/// byte/line budgets. Closes Hamsa eval B1 (many-headings.md, 251 sections).
pub(crate) const HEADINGS_HARD_CAP: usize = 40;

/// Check whether content should be buffered based on estimated token count.
pub(crate) fn exceeds_inline_limit(text: &str) -> bool {
    text.len() / 4 > MAX_INLINE_TOKENS
}

/// Soft cap for compact summaries shown alongside `@tool_*` refs.
/// Truncation prefers whole-line boundaries. See [`truncate_compact`].
pub(crate) const COMPACT_SUMMARY_MAX_BYTES: usize = 2_000;
/// Hard cap — no summary will exceed this size regardless of line boundaries.
pub(crate) const COMPACT_SUMMARY_HARD_MAX_BYTES: usize = 3_000;

/// Shared context passed to every tool invocation.
///
/// Holds references to all shared resources (agent state, LSP manager,
/// and eventually parser pool, etc.). Extend this struct as new shared
/// resources are added — all tools get access automatically.
pub struct ToolContext {
    pub agent: Agent,
    // Arc<dyn LspProvider>: swapped for MockLspProvider in tests, LspManager in production (testability seam).
    pub lsp: Arc<dyn LspProvider>,
    pub output_buffer: Arc<crate::tools::output_buffer::OutputBuffer>,
    pub progress: Option<Arc<crate::tools::progress::ProgressReporter>>,
    /// MCP peer for sending elicitation requests to the client.
    /// `None` in tests or when the client doesn't support elicitation.
    pub peer: Option<Peer<RoleServer>>,
    /// Session-scoped markdown section read-coverage tracker.
    pub section_coverage:
        std::sync::Arc<std::sync::Mutex<crate::tools::section_coverage::SectionCoverage>>,
    /// Session-scoped set of guide topics already hinted to the model.
    /// Reset on workspace(action="activate").
    pub guide_hints_emitted: Arc<parking_lot::Mutex<crate::tools::guide_ledger::GuideLedger>>,
    /// Per-request workspace pin (Phase 2 plumbing for per-request workspace
    /// resolution). Populated in `call_tool_inner` from an optional `workspace`
    /// input field, canonicalized to match the registry's canonical-root keys.
    /// No tool reads it yet — Phase 3 wires the selector-aware accessors
    /// (`with_project_at`). See
    /// docs/plans/2026-05-30-per-request-workspace-pinning.md.
    pub workspace_override: Option<std::path::PathBuf>,
}

/// MCP client identity resolved from the `initialize` handshake's `clientInfo`.
/// This is the protocol-proper, agent-agnostic source — every MCP client sends
/// it. Verified live for Claude Code: name="claude-code", version="2.1.177"
/// (see codescout memory `claude-code-mcp-env`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    pub name: String,
    pub version: Option<String>,
}

/// Policy: does a client `name` indicate Claude-Code-style subagent-spawning
/// capability? Conservative — only the Claude family today. Pure + testable.
pub(crate) fn is_subagent_capable_name(name: Option<&str>) -> bool {
    name.is_some_and(|n| n.to_lowercase().contains("claude"))
}

impl ToolContext {
    /// The connected MCP client's identity, from the `initialize` handshake's
    /// `clientInfo`. `None` when no peer is attached (e.g. tests). The
    /// `AI_AGENT` env fallback is deliberately NOT consulted here: on a real
    /// tool call the handshake has completed so `clientInfo` is always present,
    /// and reading the env would poison tests (the test runner itself runs
    /// under Claude Code). The pre-handshake `AI_AGENT` path belongs to the
    /// instructions-cap work, which is deferred.
    pub fn client_identity(&self) -> Option<ClientIdentity> {
        self.peer
            .as_ref()
            .and_then(|p| p.peer_info())
            .map(|info| ClientIdentity {
                name: info.client_info.name.clone(),
                version: {
                    let v = info.client_info.version.clone();
                    (!v.is_empty()).then_some(v)
                },
            })
    }

    /// Convenience: the connected client's name, if known.
    pub fn client_name(&self) -> Option<String> {
        self.client_identity().map(|c| c.name)
    }

    /// Whether the connected client supports subagent spawning (Claude family).
    pub fn is_subagent_capable(&self) -> bool {
        is_subagent_capable_name(self.client_name().as_deref())
    }
}

impl ToolContext {
    /// Request structured input from the user via MCP elicitation.
    ///
    /// Returns `Ok(Some(T))` if the user provided data, `Ok(None)` if elicitation
    /// is unavailable (no peer, client doesn't support it, or user provided no content),
    /// or an error if the user declined/cancelled.
    ///
    /// `UserDeclined` and `UserCancelled` are wrapped as [`RecoverableError`] so they
    /// route to `isError: false` — these are expected user-driven outcomes, not fatal
    /// tool failures. The LLM can handle them gracefully without aborting sibling calls.
    pub async fn elicit<T>(&self, message: impl Into<String>) -> anyhow::Result<Option<T>>
    where
        T: rmcp::service::ElicitationSafe + for<'de> serde::Deserialize<'de>,
    {
        let Some(ref peer) = self.peer else {
            return Ok(None);
        };
        match peer.elicit::<T>(message).await {
            Ok(data) => Ok(data),
            Err(rmcp::service::ElicitationError::CapabilityNotSupported) => Ok(None),
            Err(rmcp::service::ElicitationError::NoContent) => Ok(None),
            Err(rmcp::service::ElicitationError::UserDeclined) => {
                Err(RecoverableError::with_hint(
                    "User declined the elicitation request",
                    "Re-issue the call with a more specific argument to avoid the disambiguation prompt",
                )
                .into())
            }
            Err(rmcp::service::ElicitationError::UserCancelled) => {
                Err(RecoverableError::with_hint(
                    "User cancelled the elicitation request",
                    "Re-issue the call with a more specific argument to avoid the disambiguation prompt",
                )
                .into())
            }
            Err(rmcp::service::ElicitationError::ParseError { error, data }) => {
                Err(RecoverableError::with_hint(
                    format!("Could not parse elicitation response: {error}"),
                    format!("Received data: {data}"),
                )
                .into())
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Severity-tagged guidance attached to a [`RecoverableError`].
///
/// Serialized into the response body under the variant-named key
/// (`hint` / `warning` / `must_follow`). The field name itself carries the
/// register — agents scan JSON responses and react to the key, not the prose.
#[derive(Debug, Clone)]
pub enum Guidance {
    /// Optional narrowing — "you could try X".
    Hint(String),
    /// Off-golden-path — "reconsider before proceeding".
    Warning(String),
    /// Binding, iron-law-grade rule — violating produces wrong results or
    /// wastes significant context. Cite the specific rule where applicable
    /// (e.g. "IRON LAW #6: ...").
    MustFollow(String),
}

impl Guidance {
    /// JSON field name the variant serializes under.
    pub fn field_name(&self) -> &'static str {
        match self {
            Self::Hint(_) => "hint",
            Self::Warning(_) => "warning",
            Self::MustFollow(_) => "must_follow",
        }
    }

    /// The guidance text.
    pub fn text(&self) -> &str {
        match self {
            Self::Hint(s) | Self::Warning(s) | Self::MustFollow(s) => s.as_str(),
        }
    }
}

/// A recoverable tool error: the LLM gave bad input and can self-correct.
///
/// When a tool returns this error type, the MCP server serialises it as
/// `isError: false` with a JSON body containing `"error"`, optional
/// guidance (under one of `hint` / `warning` / `must_follow`), and any
/// structured `extra` fields spliced in at the top level.  This prevents
/// Claude Code from aborting sibling parallel tool calls (which it does
/// when it sees `isError: true`).
///
/// Use this for **expected, input-driven failures**: path not found,
/// unsupported file type, empty glob match, no index built yet, etc.
///
/// Keep returning plain `anyhow` errors (→ `isError: true`) for genuine
/// failures: panics, security violations, LSP crashes.
#[derive(Debug)]
pub struct RecoverableError {
    /// Human-readable description of what went wrong.
    pub message: String,
    /// Optional severity-tagged guidance for how to correct the call.
    pub guidance: Option<Guidance>,
    /// Structured payload spliced into the response body at the top level
    /// (e.g. `file_id`, `section_map`, `next_actions`). Boxed to keep the
    /// struct size below clippy's `result_large_err` threshold.
    pub extra: Box<serde_json::Map<String, serde_json::Value>>,
}

impl RecoverableError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: None,
            extra: Box::new(serde_json::Map::new()),
        }
    }

    pub fn with_hint(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: Some(Guidance::Hint(hint.into())),
            extra: Box::new(serde_json::Map::new()),
        }
    }

    pub fn with_warning(message: impl Into<String>, warning: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: Some(Guidance::Warning(warning.into())),
            extra: Box::new(serde_json::Map::new()),
        }
    }

    pub fn with_must_follow(message: impl Into<String>, must_follow: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            guidance: Some(Guidance::MustFollow(must_follow.into())),
            extra: Box::new(serde_json::Map::new()),
        }
    }

    /// Attach a structured field to the response body. Chainable.
    pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }

    /// Back-compat accessor: returns the text of the attached `Hint` variant,
    /// or `None` for other variants or no guidance.
    pub fn hint(&self) -> Option<&str> {
        match &self.guidance {
            Some(Guidance::Hint(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// Display renders `message`, followed by `" — {field}: {text}"` when
/// `guidance` is `Some` — the hint IS included, not omitted. Existing
/// `to_string().contains(...)` test assertions rely on this append; don't
/// remove it without auditing them first. Production callers additionally
/// surface the full structured payload via `route_tool_error` (see
/// `src/tools/mod.rs`), which emits `hint`/steps as dedicated JSON keys — use
/// `.hint()` when you need the hint value alone, without the message prefix.
impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(g) = &self.guidance {
            write!(f, " — {}: {}", g.field_name(), g.text())?;
        }
        Ok(())
    }
}

impl std::error::Error for RecoverableError {}

/// Returns the largest byte offset `<= n` that lands on a UTF-8 char boundary.
/// Prevents `&str[..n]` panics when `n` points into a multi-byte character.
pub(crate) fn floor_char_boundary(s: &str, n: usize) -> usize {
    let n = n.min(s.len());
    (0..=n).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0)
}

/// Return `&s[..max_bytes]` rounded down to a UTF-8 char boundary.
///
/// Safe alternative to `&s[..n]` which panics when `n` falls inside a
/// multi-byte character.
pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    &s[..floor_char_boundary(s, max_bytes)]
}

/// Always appends `"\n… (truncated)"` when content is cut.
pub(crate) fn truncate_compact(text: &str, soft_max: usize, hard_max: usize) -> String {
    if text.len() <= soft_max {
        return text.to_string();
    }

    // Find the last newline within hard_max bytes — prefer to break at a line boundary.
    // floor_char_boundary ensures the slice never starts mid-char (e.g. box-drawing chars).
    let search_end = floor_char_boundary(text, hard_max);
    if let Some(nl_pos) = text[..search_end].rfind('\n') {
        return format!("{}\n… (truncated)", &text[..nl_pos]);
    }

    // No newline within hard_max — hard-truncate at a char boundary.
    format!("{}… (truncated)", &text[..search_end])
}

/// Snapshot of project capabilities that tool `availability()` can inspect.
///
/// Built by `CodeScoutServer::current_capabilities()` at each `list_tools`
/// call. Cheap (Copy); do not hold references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolCapabilities {
    pub has_lsp: bool,
    pub has_embeddings: bool,
    pub has_git_remote: bool,
    pub has_libraries: bool,
}

/// Conditional-exposure constraint for a `Tool`.
#[derive(Debug, Clone, Copy)]
pub enum Availability {
    Always,
    RequiresLsp,
    RequiresEmbeddings,
    RequiresGitRemote,
    RequiresLibraries,
}

/// Wire-format preference for `Tool::call_content`.
///
/// `Json` (default): inline output is pretty-printed JSON; only buffered
/// overflow uses `format_compact`. Right for tools whose result has named
/// fields callers want to access (`output_id`, `summary`, etc).
///
/// `Text`: inline AND buffered output use `format_compact`. Right for bulk
/// locator tools (grep, references, tree glob) where the compact text form
/// is ripgrep-style `file\n  N: content` — strictly more compact than the
/// equivalent JSON and trivially parseable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputForm {
    Json,
    Text,
}

impl Availability {
    pub fn is_available(self, c: &ToolCapabilities) -> bool {
        match self {
            Availability::Always => true,
            Availability::RequiresLsp => c.has_lsp,
            Availability::RequiresEmbeddings => c.has_embeddings,
            Availability::RequiresGitRemote => c.has_git_remote,
            Availability::RequiresLibraries => c.has_libraries,
        }
    }
}

/// Derive a recovery `json_path` from a buffered payload's actual shape.
///
/// The old default was the constant `"$.field"`, which models recovery as a
/// single-value extraction. But the results that actually overflow are
/// overwhelmingly *lists of records*, where the useful projection is "this field
/// from every element" — so the hint recommended a shape the payload did not
/// have. Measured 2026-08-15 across 13 usage.db files: `[*]`, which the parser
/// then rejected, was 73% of all rejected segments, and agents fell back to
/// shelling out. A hint that cannot work for the result it is attached to is
/// worse than no hint: it converts a lookup into a failed call.
///
/// Picks the **largest array anywhere within a bounded depth**, not merely a
/// top-level one. Envelopes routinely carry a short array at the top and the
/// payload worth projecting further down — measured live 2026-08-16, an
/// `artifact(get)` result advertised `$.tags[*]` (4 strings) for a buffer whose
/// point was `$.augmentation.params.tasks[*]` (18 records).
/// docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
pub(crate) fn default_json_path_hint(val: &Value) -> String {
    if val.is_array() {
        return "$[*]".to_string();
    }
    let mut best: Option<(String, usize)> = None;
    find_largest_array(val, "$", 0, &mut best);
    best.map(|(path, _)| format!("{path}[*]"))
        .unwrap_or_else(|| "$.field".to_string())
}

/// Record the largest array reachable through object keys, with its full path.
///
/// Descends through objects only, never into arrays: an array's *elements* are
/// what a projection returns, so recursing into them would suggest addressing one
/// row rather than the set. Depth-bounded so a deeply nested payload costs a fixed
/// walk rather than a full traversal.
fn find_largest_array(v: &Value, path: &str, depth: usize, best: &mut Option<(String, usize)>) {
    const MAX_DEPTH: usize = 4;
    let Some(map) = v.as_object() else {
        return;
    };
    for (key, child) in map {
        let child_path = format!("{path}.{key}");
        match child {
            Value::Array(items) => {
                if best.as_ref().is_none_or(|(_, n)| items.len() > *n) {
                    *best = Some((child_path, items.len()));
                }
            }
            Value::Object(_) if depth < MAX_DEPTH => {
                find_largest_array(child, &child_path, depth + 1, best);
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Tool name as exposed over MCP (e.g. "symbols")
    fn name(&self) -> &str;

    /// Short description shown to the LLM.
    ///
    /// **Hard cap: 300 chars** — enforced by
    /// `server::tests::tool_descriptions_stay_under_budget`
    /// (`src/server.rs:1493-1508`). Test fails the build if any tool's
    /// description exceeds the cap. Librarian tools are exempt because
    /// they expose feature-rich CRUD surfaces with multiple actions.
    ///
    /// Rationale: Claude Code (the primary consumer) silently truncates
    /// per-tool `description` fields at ~2000 bytes / ~500 tokens per
    /// the MCP channel inventory (`docs/architecture/mcp-channel-caps.md`).
    /// The 300-char self-cap is well inside the platform truncation
    /// boundary AND leaves room for tool count to grow without the LLM
    /// drowning in description text at session start.
    ///
    /// Style: lead with the action verb, name the most-used arguments,
    /// keep examples short. If the description starts approaching the cap,
    /// prefer pointing the LLM at `get_guide(topic)` (large-budget
    /// channel — ~100 KB cap on tool results) over packing prose here.
    fn description(&self) -> &str;

    /// Extended usage documentation for `doc://codescout-tool-guide`.
    ///
    /// The short `description()` (capped at 300 chars) goes in the MCP tool list
    /// and is re-sent every turn. Long examples and "when to use this vs. that"
    /// prose live here and are only paid for when the agent explicitly reads the
    /// tool-guide resource.
    fn long_docs(&self) -> Option<&str> {
        None
    }

    /// JSON Schema for the input parameters
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given input (already parsed from JSON)
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value>;

    /// Optional human-readable formatting for the tool result.
    /// When Some, call_content() emits dual-audience blocks:
    ///   1. Compact JSON (audience: assistant)
    ///   2. Formatted plain text (audience: user)
    /// Compact plain-text summary used in the buffer path alongside `@tool_*` refs.
    /// Return `None` for the generic "Result stored in @tool_xxx (N bytes)" fallback.
    fn format_compact(&self, _result: &Value) -> Option<String> {
        None
    }

    /// Human-readable display text for the MCP user-facing channel.
    ///
    /// Defaults to `format_compact()`. Override for richer display when the user
    /// channel differs from the buffer summary.
    ///
    /// Not yet called — wire in `call_content` at the TODO comment when either
    /// Claude Code issue #13600 (audience filtering) or #3174 (notifications/message)
    /// ships.
    fn format_for_user_channel(&self, result: &Value) -> Option<String> {
        self.format_compact(result)
    }

    /// Conditional-exposure gate for `ServerHandler::list_tools`.
    /// Defaults to `Always`; override in tools that require LSP, embeddings, etc.
    fn availability(&self, _caps: &ToolCapabilities) -> Availability {
        Availability::Always
    }

    /// Whether this tool resolves a project per-request and therefore accepts
    /// the optional `workspace` pin (regime-3). Drives central `workspace`-param
    /// schema injection in `ServerHandler::list_tools`.
    ///
    /// Default: every tool is pinnable EXCEPT session/global/registry tools —
    /// those resolve no per-request project and must not advertise a pin they
    /// ignore. The librarian family (`artifact`, `artifact_event`,
    /// `artifact_refresh`, `artifact_augment`, `librarian`) IS pinnable: its
    /// adapter resolves `ctx.workspace_override` before deriving each call's
    /// `current_project` (see `LibrarianAdapter::call`), so a `workspace=` pin
    /// scopes catalog reads and writes to the named workspace.
    /// See docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md.
    fn pinnable(&self) -> bool {
        !matches!(
            self.name(),
            "workspace"
                | "activate_project"
                | "onboarding"
                | "get_usage_stats"
                | "get_guide"
                | "__probe_description_cap__"
        )
    }

    /// Wire-format preference for `call_content`.
    ///
    /// Defaults to `OutputForm::Json` (pretty-printed inline, compact summary only on
    /// buffered overflow). Override to `OutputForm::Text` for bulk-locator tools
    /// (grep, references, tree glob) whose `format_compact` output is strictly
    /// more compact than the equivalent JSON.
    fn output_form(&self) -> OutputForm {
        OutputForm::Json
    }

    /// When true, this tool's output is never diverted to the `@tool_*` output
    /// buffer on overflow — the full result is always returned inline, no matter
    /// how large. Default false (large output buffers, per progressive-disclosure).
    ///
    /// Reserve for tools whose entire value IS the payload the caller asked to
    /// read and that cannot be meaningfully paginated or field-extracted — e.g.
    /// `get_guide`, where a buffer handle defeats the purpose (the agent asked to
    /// READ the guide, not to receive a reference it must then fetch).
    fn force_inline(&self) -> bool {
        false
    }

    /// Returns true if this tool call will mutate project state and therefore
    /// must acquire the cross-process write lock before dispatch.
    ///
    /// Defaults to `false` (read-only). Override on every mutating tool.
    /// For tools whose write-ness depends on input (e.g. `memory` switches on
    /// `action`), inspect `input` to decide. The server calls this after
    /// argument parsing, so `input` is already the same `Value` that `call()`
    /// will receive.
    fn is_write(&self, _input: &Value) -> bool {
        false
    }

    /// Returns the JSON path to the most useful field in a buffered result.
    ///
    /// Used to build a specific, actionable hint when the tool result is stored
    /// in an `@tool_*` buffer. Override to guide agents directly to the right
    /// extraction path (e.g. `"$.symbols[0].body"` for `symbols` with
    /// `include_body=true`); the default derives one from the payload's shape.
    fn json_path_hint(&self, val: &Value) -> String {
        default_json_path_hint(val)
    }

    /// Returns MCP content blocks for this tool call.
    ///
    /// Large output (> threshold) is stored in the output buffer and a compact
    /// summary is returned. Small output is returned as pretty-printed JSON
    /// (`OutputForm::Json`, default) or as the compact text form
    /// (`OutputForm::Text`, for bulk locators like grep/references/tree-glob).
    /// Override directly for full control over content blocks.
    ///
    /// **V2 hard-injection** (added 2026-05-28): when this is the first call
    /// in the session that triggers `relevant_guide_topic()` for some topic,
    /// the response gets a SECOND `Content::text` block containing the full
    /// guide body for that topic, wrapped in `<!-- auto-injected get_guide
    /// ('TOPIC') ... -->` markers. The tool's actual response block stays
    /// FIRST so the model sees its answer before the supplementary guide.
    /// The legacy `_guide_hint` JSON field is also retained on the first
    /// block (backward compat for tests + clients that only render text;
    /// soft pointer to the second-block guide content). Compliance with the
    /// pointer was empirically ~1.3% (F-3 in
    /// `docs/trackers/prompt-guide-refactor-session-log.md`); V2 closes that
    /// gap by delivering the content directly.
    async fn call_content(&self, input: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
        let mut val = self.call(input, ctx).await?;

        // Field-aware project-root stripping. Runs HERE, on the typed Value,
        // and therefore BEFORE `exceeds_inline_limit`, the `@tool_*` buffer
        // payload, and `format_compact` — the ordering is load-bearing: a
        // summary built from an unstripped Value leaks absolute paths through
        // the serialized envelope, which is how 85% of the pre-fix leaks
        // escaped. The root resolves from the same `ctx` the tool body used,
        // so a `workspace=` pin cannot be mismatched here the way it could
        // when `post_process` had to be handed the pin separately
        // (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md).
        let root_prefix = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await
            .map(|p| format!("{}/", crate::util::fs::to_forward_slash(&p)))
            .unwrap_or_default();
        crate::tools::core::path_strip::strip_paths_in_value(&mut val, &root_prefix);
        let val = val;
        let form = self.output_form();
        let json = serde_json::to_string(&val).unwrap_or_else(|_| val.to_string());

        // Compute potential hint topic + whether it should fire on this call.
        let hint_topic: Option<String> = {
            let mut emitted = ctx.guide_hints_emitted.lock();
            if emitted.is_empty() {
                // Session opener. An empty ledger means this is the session's
                // first guide-eligible call — or the first since an `activate`
                // or post-compact re-arm cleared it — so the orientation guide
                // goes out now, whatever tool was called.
                //
                // Before 2026-08-16 this fired only for `workspace`, so a
                // session opening with symbols/grep/read_file never received
                // it. `progressive-disclosure` does not cover the gap: it is
                // conditional on actual overflow, so a session with small
                // results got no guide at all.
                //
                // The calling tool's own topic is deliberately NOT marked
                // emitted here, so it still fires on a later call. The cost is
                // a one-response delay, and a tool called exactly once in a
                // session forfeits its guide — an acceptable trade against
                // 18 KB of librarian guide landing on a one-shot `artifact`
                // call. See `prompts::SESSION_OPENING_GUIDE`.
                let topic = crate::prompts::SESSION_OPENING_GUIDE;
                emitted.insert(topic.to_string());
                Some(topic.to_string())
            } else if let Some(topic) = self.relevant_guide_topic() {
                if emitted.contains(topic) {
                    None
                } else {
                    let should = match topic {
                        "progressive-disclosure" => {
                            // Either the default-path buffering kicked in (large JSON),
                            // or the tool itself pre-buffered (e.g. run_command storing
                            // a `@cmd_*` ref and returning a small envelope with
                            // `output_id`). Both signal the agent should learn the
                            // progressive-disclosure pattern.
                            exceeds_inline_limit(&json)
                                || val
                                    .as_object()
                                    .and_then(|o| o.get("output_id"))
                                    .and_then(|v| v.as_str())
                                    .is_some()
                        }
                        _ => true,
                    };
                    if should {
                        emitted.insert(topic.to_string());
                        Some(topic.to_string())
                    } else {
                        None
                    }
                }
            } else {
                None
            }
        };

        fn inject_hint(val: &mut Value, topic: &str) {
            if let Some(obj) = val.as_object_mut() {
                obj.insert(
                    "_guide_hint".to_string(),
                    Value::String(format!(
                        "First call this session for topic '{topic}'. \
                         Full guide auto-injected as a separate content \
                         block below; do not re-call get_guide(\"{topic}\")."
                    )),
                );
            }
        }

        /// Build the second-block content for V2 hard-injection. Returns
        /// `None` when the topic's body is not registered (defensive — the
        /// `_guide_hint` field still ships in that case so the model knows
        /// to call `get_guide(topic)` manually).
        fn guide_block(topic: &str) -> Option<Content> {
            let body = crate::prompts::topic_body(topic)?;
            let wrapped = format!(
                "<!-- auto-injected get_guide('{topic}') — first call this session \
                 that triggers the topic. Do NOT re-call get_guide for this topic. -->\n\
                 \n\
                 {body}\n\
                 \n\
                 <!-- end auto-injected get_guide('{topic}') -->"
            );
            Some(Content::text(wrapped))
        }

        // Build the primary response block (the tool's actual output).
        // `force_inline` tools (e.g. get_guide) opt out of overflow buffering:
        // their full payload is always returned inline regardless of size.
        let primary = if exceeds_inline_limit(&json) && !self.force_inline() {
            let json_len = json.len();
            let ref_id = ctx.output_buffer.store_tool(self.name(), json);
            // The fallback describes the payload rather than restating the envelope. A
            // bare "Result stored in @tool_x (N bytes)" repeats `output_id` — which the
            // caller already has — and spends the only informative slot on nothing, so
            // the call answers no question and a second round-trip is needed to learn
            // what is in there. See `format::describe_payload_shape`.
            let raw_summary = self.format_compact(&val).unwrap_or_else(|| {
                let head = format!("Result stored in {ref_id} ({json_len} bytes)");
                match crate::tools::format::describe_payload_shape(&val) {
                    Some(shape) => format!("{head}\n  {shape}"),
                    None => head,
                }
            });
            let summary = truncate_compact(
                &raw_summary,
                COMPACT_SUMMARY_MAX_BYTES,
                COMPACT_SUMMARY_HARD_MAX_BYTES,
            );

            let jp = self.json_path_hint(&val);
            let hint = format!(
                "read_file(\"{ref_id}\", json_path=\"{jp}\") to extract a specific field, \
                 or read_file(\"{ref_id}\", start_line=N, end_line=M) to browse sections"
            );
            let mut buffered = serde_json::json!({
                "output_id": ref_id,
                "summary": summary,
                "hint": hint,
                "buffered_bytes": json_len,
            });
            if let Some(topic) = &hint_topic {
                inject_hint(&mut buffered, topic);
            }
            Content::text(
                serde_json::to_string_pretty(&buffered)
                    .unwrap_or_else(|_| format!("{{\"output_id\":\"{ref_id}\"}}")),
            )
        } else {
            // Small output path.
            let mut val = val;
            if let Some(topic) = &hint_topic {
                inject_hint(&mut val, topic);
            }
            if form == OutputForm::Text {
                if let Some(text) = self.format_compact(&val) {
                    Content::text(text)
                } else {
                    Content::text(
                        serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string()),
                    )
                }
            } else {
                Content::text(
                    serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string()),
                )
            }
        };

        // V2 hard-injection: append the full guide body as a second block
        // when a hint fired. Topic body lookup may return None (unregistered
        // topic) — in that case we ship only the primary block with the
        // soft `_guide_hint` pointer.
        let mut blocks = vec![primary];
        if let Some(topic) = &hint_topic {
            if let Some(block) = guide_block(topic) {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }

    /// Topic name this tool's discipline depends on, for the first-call
    /// hint mechanism. When the model first calls a tool with
    /// Some("librarian"), Tool::call_content injects a one-shot
    /// _guide_hint pointing to get_guide("librarian"). Topic dedup is
    /// session-wide; calling a different tool with the same topic in
    /// the same session does NOT re-emit the hint. The set is cleared
    /// on workspace(action="activate").
    ///
    /// Default None — most tools' rules fit in their description and
    /// don't need the hint.
    fn relevant_guide_topic(&self) -> Option<&str> {
        None
    }
}

#[cfg(test)]
mod json_path_hint_tests {
    use super::default_json_path_hint;
    use serde_json::json;

    #[test]
    fn array_payload_projects_the_root() {
        assert_eq!(default_json_path_hint(&json!([{"a": 1}])), "$[*]");
    }

    #[test]
    fn object_payload_names_its_largest_array() {
        let v = json!({ "meta": "x", "rows": [1, 2, 3], "tags": ["a"] });
        assert_eq!(default_json_path_hint(&v), "$.rows[*]");
    }

    /// The shape that matters most in practice. An `artifact(get)` envelope keeps
    /// its useful payload three levels down while carrying a short `tags` array at
    /// the top; a top-level-only scan names `tags` — real, but not what the caller
    /// wants projected. Measured live 2026-08-16: the hint said `$.tags[*]` (4
    /// strings) for a buffer whose point was `$.augmentation.params.tasks[*]` (18
    /// records).
    #[test]
    fn nested_payload_beats_a_shallow_but_smaller_array() {
        let v = json!({
            "tags": ["a", "b", "c", "d"],
            "augmentation": { "params": { "tasks": [1, 2, 3, 4, 5, 6, 7, 8] } }
        });
        assert_eq!(default_json_path_hint(&v), "$.augmentation.params.tasks[*]");
    }

    #[test]
    fn scalar_shaped_payload_keeps_the_generic_placeholder() {
        assert_eq!(
            default_json_path_hint(&json!({ "content": "text", "lines": 4 })),
            "$.field"
        );
    }
}
