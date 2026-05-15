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

/// Display renders only `message`. The structured `hint` and `recovery_steps`
/// are intentionally omitted here so existing `to_string().contains(...)` test
/// assertions stay stable. Production callers surface the full payload via
/// `route_tool_error` (see `src/tools/mod.rs`), which emits `hint`/steps as
/// dedicated JSON keys. If you need the hint programmatically, downcast to
/// `RecoverableError` and call `.hint()` — do not parse it out of Display.
impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
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

/// A single MCP tool exposed to the LLM.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Tool name as exposed over MCP (e.g. "symbols")
    fn name(&self) -> &str;

    /// Short description shown to the LLM
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
    /// in an `@tool_*` buffer. The default (`"$.field"`) is a generic placeholder;
    /// override to guide agents directly to the right extraction path (e.g.
    /// `"$.symbols[0].body"` for `symbols` with `include_body=true`).
    fn json_path_hint(&self, _val: &Value) -> String {
        "$.field".to_string()
    }

    /// Returns MCP content blocks for this tool call.
    ///
    /// Large output (> threshold) is stored in the output buffer and a compact
    /// summary is returned. Small output is returned as pretty-printed JSON.
    /// Override directly for full control over content blocks.
    async fn call_content(&self, input: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
        let val = self.call(input, ctx).await?;
        let json = serde_json::to_string(&val).unwrap_or_else(|_| val.to_string());

        if exceeds_inline_limit(&json) {
            let json_len = json.len();
            let ref_id = ctx.output_buffer.store_tool(self.name(), json);
            let raw_summary = self
                .format_compact(&val)
                .unwrap_or_else(|| format!("Result stored in {} ({} bytes)", ref_id, json_len));
            let summary = truncate_compact(
                &raw_summary,
                COMPACT_SUMMARY_MAX_BYTES,
                COMPACT_SUMMARY_HARD_MAX_BYTES,
            );

            // Return a *structured* JSON response so agents consistently look for
            // the `output_id` field — the same field name `run_command` uses for its
            // `@cmd_*` refs.  The previous prose format ("summary\nFull result: @ref")
            // caused agents to either miss the ref or confuse it with the summary text.
            let jp = self.json_path_hint(&val);
            let hint = format!(
                "read_file(\"{ref_id}\", json_path=\"{jp}\") to extract a specific field, \
                 or read_file(\"{ref_id}\", start_line=N, end_line=M) to browse sections"
            );
            let buffered = serde_json::json!({
                "output_id": ref_id,
                "summary": summary,
                "hint": hint,
            });
            return Ok(vec![Content::text(
                serde_json::to_string_pretty(&buffered)
                    .unwrap_or_else(|_| format!("{{\"output_id\":\"{ref_id}\"}}")),
            )]);
        }

        // Small output — return pretty JSON to the assistant.
        // TODO(#13600/#3174): emit self.format_for_user_channel(&val) to user channel here.
        Ok(vec![Content::text(
            serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string()),
        )])
    }
}
