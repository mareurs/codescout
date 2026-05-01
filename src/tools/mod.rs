//! Tool trait and registry.
//!
//! Each tool is a struct that implements the `Tool` trait. Tools are
//! registered in the MCP server at startup.

pub mod ast;
pub mod command_summary;
pub mod config;
pub mod create_file;
pub mod edit_file;
pub mod file_summary;
pub(crate) mod format;
pub mod grep;
pub mod library;
pub mod memory;
pub mod output;
pub mod output_buffer;
pub mod progress;
pub mod semantic;
pub mod symbol;
pub mod usage;
pub use usage::GetUsageStats;
pub mod markdown;
pub mod onboarding;
pub mod read_file;
pub mod run_command;
pub mod section_coverage;
pub mod tree;
pub use onboarding::Onboarding;
pub use run_command::RunCommand;

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
    pub output_buffer: Arc<output_buffer::OutputBuffer>,
    pub progress: Option<Arc<progress::ProgressReporter>>,
    /// MCP peer for sending elicitation requests to the client.
    /// `None` in tests or when the client doesn't support elicitation.
    pub peer: Option<Peer<RoleServer>>,
    /// Session-scoped markdown section read-coverage tracker.
    pub section_coverage: std::sync::Arc<std::sync::Mutex<section_coverage::SectionCoverage>>,
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

impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RecoverableError {}

/// Convenience: extract a required parameter from a JSON `Value`, returning
/// `RecoverableError` (not a fatal error) if it is missing.
pub fn require_param<'a>(
    input: &'a serde_json::Value,
    name: &str,
) -> anyhow::Result<&'a serde_json::Value> {
    input.get(name).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("missing '{}' parameter", name),
            format!("Add the required '{}' parameter to the tool call.", name),
        )
        .into()
    })
}

/// Like `require_param`, but also checks common LLM aliases for the parameter.
/// If the canonical name isn't found, tries each alias in order.
/// Returns the value from whichever name matched first.
pub fn require_param_or<'a>(
    input: &'a serde_json::Value,
    name: &str,
    aliases: &[&str],
) -> anyhow::Result<&'a serde_json::Value> {
    if let Some(v) = input.get(name) {
        return Ok(v);
    }
    for alias in aliases {
        if let Some(v) = input.get(*alias) {
            return Ok(v);
        }
    }
    Err(RecoverableError::with_hint(
        format!("missing '{}' parameter", name),
        format!("Add the required '{}' parameter to the tool call.", name),
    )
    .into())
}

/// Like `require_str_param`, but also checks common LLM aliases.
pub fn require_str_param_or<'a>(
    input: &'a serde_json::Value,
    name: &str,
    aliases: &[&str],
) -> anyhow::Result<&'a str> {
    require_param_or(input, name, aliases)?
        .as_str()
        .ok_or_else(|| {
            RecoverableError::with_hint(
                format!("'{}' must be a string", name),
                format!("Provide '{}' as a string value.", name),
            )
            .into()
        })
}

/// Convenience: extract a required string parameter from a JSON `Value`.
pub fn require_str_param<'a>(input: &'a serde_json::Value, name: &str) -> anyhow::Result<&'a str> {
    require_param(input, name)?.as_str().ok_or_else(|| {
        RecoverableError::with_hint(
            format!("'{}' must be a string", name),
            format!("Provide '{}' as a string value.", name),
        )
        .into()
    })
}

/// Convenience: extract a required u64 parameter from a JSON `Value`.
pub fn require_u64_param(input: &serde_json::Value, name: &str) -> anyhow::Result<u64> {
    let val = require_param(input, name)?;
    // Accept both JSON numbers and string-encoded integers (LLMs sometimes quote them).
    if let Some(n) = val.as_u64() {
        return Ok(n);
    }
    if let Some(s) = val.as_str() {
        if let Ok(n) = s.trim().parse::<u64>() {
            return Ok(n);
        }
    }
    Err(RecoverableError::with_hint(
        format!("'{}' must be a non-negative integer", name),
        format!("Provide '{}' as a non-negative integer.", name),
    )
    .into())
}

/// Parse a boolean parameter from a JSON `Value`.
///
/// MCP clients (including Claude Code) may serialize boolean parameters as
/// JSON strings (`"true"` / `"false"`) rather than native JSON booleans.
/// This helper accepts both representations, defaulting to `false`.
pub fn parse_bool_param(val: &serde_json::Value) -> bool {
    val.as_bool()
        .or_else(|| val.as_str().and_then(|s| s.parse::<bool>().ok()))
        .unwrap_or(false)
}

/// Extract an optional boolean parameter with lenient coercion.
///
/// Returns `Some(bool)` if the parameter is present and coercible (native JSON
/// boolean or `"true"`/`"false"` string), `None` if absent or null. This is
/// the `Option`-returning counterpart to [`parse_bool_param`] — use it when
/// the caller needs to distinguish "not provided" from "explicitly false".
pub fn optional_bool_param(input: &serde_json::Value, name: &str) -> Option<bool> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_bool()
        .or_else(|| val.as_str().and_then(|s| s.parse::<bool>().ok()))
}

/// Extract an optional u64 parameter with lenient coercion.
///
/// Accepts both native JSON numbers and string-encoded integers (`"42"` → 42).
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_u64_param(input: &serde_json::Value, name: &str) -> Option<u64> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_u64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
}

/// Extract an optional i64 parameter with lenient coercion.
///
/// Accepts both native JSON numbers and string-encoded integers (`"-1"` → -1).
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_i64_param(input: &serde_json::Value, name: &str) -> Option<i64> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_i64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

/// Extract an optional f64 parameter with lenient coercion.
///
/// Accepts both native JSON numbers and string-encoded floats (`"0.5"` → 0.5).
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_f64_param(input: &serde_json::Value, name: &str) -> Option<f64> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    val.as_f64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

/// Extract an optional JSON array parameter with lenient coercion.
///
/// Some MCP clients serialize array-typed tool parameters as JSON strings
/// (e.g. `"[\"a\",\"b\"]"` instead of `["a","b"]`). This helper tries
/// `as_array()` first, then falls back to parsing the string as JSON.
/// Returns `None` if the parameter is absent, null, or not coercible.
pub fn optional_array_param(
    input: &serde_json::Value,
    name: &str,
) -> Option<Vec<serde_json::Value>> {
    let val = input.get(name)?;
    if val.is_null() {
        return None;
    }
    // Native JSON array — fast path
    if let Some(arr) = val.as_array() {
        return Some(arr.clone());
    }
    // String-encoded JSON array — fallback for MCP clients that stringify arrays
    if let Some(s) = val.as_str() {
        if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
            return Some(arr);
        }
    }
    None
}

/// Block write operations when git worktrees exist but the agent hasn't
/// explicitly called `activate_project` to confirm which project to write to.
///
/// Returns `Ok(())` when writes are allowed:
/// - Agent explicitly activated a project via `activate_project`
/// - No git worktrees exist (no ambiguity)
///
/// Returns `RecoverableError` when writes should be blocked:
/// - Worktrees exist AND the project was only implicitly set at startup
pub async fn guard_worktree_write(ctx: &ToolContext) -> anyhow::Result<()> {
    if ctx.agent.is_project_explicitly_activated().await {
        return Ok(());
    }
    let root = ctx.agent.require_project_root().await?;
    let worktrees = crate::util::path_security::list_git_worktrees(&root);
    if worktrees.is_empty() {
        return Ok(());
    }
    let wt_list: Vec<String> = worktrees.iter().map(|p| p.display().to_string()).collect();
    let hint = format!(
        "Call workspace(action='activate', path=\"{}\") to select the write target (or use \"{}\") for the main repo.",
        wt_list[0],
        root.display()
    );
    Err(RecoverableError::with_hint(
        format!(
            "Write blocked: git worktrees detected but workspace(action='activate') has not been called. \
             Worktrees: [{}]",
            wt_list.join(", ")
        ),
        hint,
    )
    .into())
}

/// Truncate a compact summary to fit within output size limits, preserving line structure.
///
/// Returns `text` verbatim when `text.len() <= soft_max`. Otherwise, finds the last `\n`
/// within `hard_max` bytes and truncates there (keeping whole lines). When no newline
/// exists within `hard_max`, truncates at `hard_max` bytes directly.
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
fn truncate_compact(text: &str, soft_max: usize, hard_max: usize) -> String {
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

/// Returns true if the input looks like it was intended as a regex pattern
/// rather than a plain symbol name or literal text.
// Used by symbols and search_pattern.
pub(crate) fn is_regex_like(s: &str) -> bool {
    // Alternation: `foo|bar` but not `|leading` or `trailing|`
    if s.contains('|') {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.iter().filter(|p| !p.is_empty()).count() >= 2 {
            return true;
        }
    }
    // Quantified wildcard: .* .+ .?
    if s.contains(".*") || s.contains(".+") || s.contains(".?") {
        return true;
    }
    // Anchors
    if s.starts_with('^') || s.ends_with('$') {
        return true;
    }
    // Character class with range: [A-Z] but not [u8]
    // Note: only inspects the first [...] pair in the string.
    if let Some(open) = s.find('[') {
        if let Some(close) = s[open..].find(']') {
            let inside = &s[open + 1..open + close];
            if inside.contains('-') && inside.len() > 2 {
                return true;
            }
        }
    }
    // Regex escape sequences
    if s.contains(r"\b") || s.contains(r"\w") || s.contains(r"\d") || s.contains(r"\s") {
        return true;
    }
    // Grouping: ( followed by )
    if let Some(open) = s.find('(') {
        if s[open..].contains(')') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_context_has_progress_field() {
        // Compile-only test: ensures the progress field exists and has the right type.
        fn _check_progress_field_type(_ctx: &ToolContext) {
            let _p: &Option<std::sync::Arc<progress::ProgressReporter>> = &_ctx.progress;
        }
    }

    #[test]
    fn parse_bool_param_handles_all_variants() {
        use serde_json::json;
        // Native JSON booleans
        assert!(parse_bool_param(&json!(true)));
        assert!(!parse_bool_param(&json!(false)));
        // String booleans (sent by Claude Code MCP client)
        assert!(parse_bool_param(&json!("true")));
        assert!(!parse_bool_param(&json!("false")));
        // Missing / null / wrong type → false
        assert!(!parse_bool_param(&json!(null)));
        assert!(!parse_bool_param(&json!(42)));
        assert!(!parse_bool_param(&json!("yes")));
    }

    #[test]
    fn optional_bool_param_returns_none_when_absent() {
        use serde_json::json;
        assert_eq!(optional_bool_param(&json!({}), "flag"), None);
        assert_eq!(optional_bool_param(&json!({"flag": null}), "flag"), None);
    }

    #[test]
    fn optional_bool_param_coerces_strings() {
        use serde_json::json;
        assert_eq!(optional_bool_param(&json!({"x": true}), "x"), Some(true));
        assert_eq!(optional_bool_param(&json!({"x": false}), "x"), Some(false));
        assert_eq!(optional_bool_param(&json!({"x": "true"}), "x"), Some(true));
        assert_eq!(
            optional_bool_param(&json!({"x": "false"}), "x"),
            Some(false)
        );
        assert_eq!(optional_bool_param(&json!({"x": "yes"}), "x"), None);
        assert_eq!(optional_bool_param(&json!({"x": 42}), "x"), None);
    }

    #[test]
    fn optional_u64_param_coerces_strings() {
        use serde_json::json;
        assert_eq!(optional_u64_param(&json!({}), "n"), None);
        assert_eq!(optional_u64_param(&json!({"n": null}), "n"), None);
        assert_eq!(optional_u64_param(&json!({"n": 42}), "n"), Some(42));
        assert_eq!(optional_u64_param(&json!({"n": "42"}), "n"), Some(42));
        assert_eq!(optional_u64_param(&json!({"n": " 7 "}), "n"), Some(7));
        assert_eq!(optional_u64_param(&json!({"n": "abc"}), "n"), None);
        assert_eq!(optional_u64_param(&json!({"n": "-1"}), "n"), None);
    }

    #[test]
    fn optional_i64_param_coerces_strings() {
        use serde_json::json;
        assert_eq!(optional_i64_param(&json!({}), "n"), None);
        assert_eq!(optional_i64_param(&json!({"n": -5}), "n"), Some(-5));
        assert_eq!(optional_i64_param(&json!({"n": "-5"}), "n"), Some(-5));
        assert_eq!(optional_i64_param(&json!({"n": "abc"}), "n"), None);
    }

    #[test]
    fn optional_f64_param_coerces_strings() {
        use serde_json::json;
        assert_eq!(optional_f64_param(&json!({}), "t"), None);
        assert_eq!(optional_f64_param(&json!({"t": 0.5}), "t"), Some(0.5));
        assert_eq!(optional_f64_param(&json!({"t": "0.5"}), "t"), Some(0.5));
        assert_eq!(optional_f64_param(&json!({"t": "abc"}), "t"), None);
    }

    #[test]
    fn optional_array_param_coerces_strings() {
        use serde_json::json;
        // Absent → None
        assert_eq!(optional_array_param(&json!({}), "a"), None);
        // Null → None
        assert_eq!(optional_array_param(&json!({"a": null}), "a"), None);
        // Native array → Some
        assert_eq!(
            optional_array_param(&json!({"a": ["x", "y"]}), "a"),
            Some(vec![json!("x"), json!("y")])
        );
        // String-encoded array → Some (MCP client workaround)
        assert_eq!(
            optional_array_param(&json!({"a": "[\"x\",\"y\"]"}), "a"),
            Some(vec![json!("x"), json!("y")])
        );
        // String-encoded array of objects
        assert_eq!(
            optional_array_param(&json!({"a": "[{\"k\":1},{\"k\":2}]"}), "a"),
            Some(vec![json!({"k": 1}), json!({"k": 2})])
        );
        // Non-array string → None
        assert_eq!(
            optional_array_param(&json!({"a": "not an array"}), "a"),
            None
        );
        // String-encoded non-array JSON → None
        assert_eq!(optional_array_param(&json!({"a": "{}"}), "a"), None);
        // Number → None
        assert_eq!(optional_array_param(&json!({"a": 42}), "a"), None);
    }

    #[test]
    fn recoverable_error_stores_message() {
        let e = RecoverableError::new("path not found");
        assert_eq!(e.message, "path not found");
        assert!(e.hint().is_none());
    }

    #[test]
    fn recoverable_error_stores_hint() {
        let e = RecoverableError::with_hint("path not found", "use tree to explore");
        assert_eq!(e.message, "path not found");
        assert_eq!(e.hint(), Some("use tree to explore"));
    }

    #[test]
    fn recoverable_error_display_shows_message() {
        let e = RecoverableError::with_hint("file missing", "check the path");
        assert_eq!(e.to_string(), "file missing");
    }

    #[test]
    fn require_u64_param_accepts_integer() {
        let input = serde_json::json!({ "n": 42 });
        assert_eq!(require_u64_param(&input, "n").unwrap(), 42);
    }

    #[test]
    fn require_u64_param_accepts_string_encoded_integer() {
        // LLMs sometimes quote integers — we must tolerate this.
        let input = serde_json::json!({ "n": "11" });
        assert_eq!(require_u64_param(&input, "n").unwrap(), 11);
    }

    #[test]
    fn require_u64_param_rejects_non_numeric_string() {
        let input = serde_json::json!({ "n": "abc" });
        assert!(require_u64_param(&input, "n").is_err());
    }

    #[test]
    fn require_u64_param_rejects_negative_string() {
        let input = serde_json::json!({ "n": "-5" });
        assert!(require_u64_param(&input, "n").is_err());
    }
    #[test]
    fn recoverable_error_downcasts_from_anyhow() {
        let e: anyhow::Error = RecoverableError::new("test error").into();
        assert!(
            e.downcast_ref::<RecoverableError>().is_some(),
            "must be recoverable via downcast"
        );
    }

    #[test]
    fn recoverable_error_with_warning_stores_warning_variant() {
        let e = RecoverableError::with_warning("too many results", "narrow with path=");
        assert_eq!(e.message, "too many results");
        assert!(matches!(e.guidance, Some(Guidance::Warning(ref s)) if s == "narrow with path="));
    }

    #[test]
    fn recoverable_error_with_must_follow_stores_must_follow_variant() {
        let e =
            RecoverableError::with_must_follow("heading too large", "IRON LAW #6: use @file_xxx");
        assert_eq!(e.message, "heading too large");
        assert!(
            matches!(e.guidance, Some(Guidance::MustFollow(ref s)) if s == "IRON LAW #6: use @file_xxx")
        );
    }

    #[test]
    fn recoverable_error_with_hint_still_produces_hint_variant() {
        let e = RecoverableError::with_hint("not found", "check path");
        assert!(matches!(e.guidance, Some(Guidance::Hint(ref s)) if s == "check path"));
        assert_eq!(e.hint(), Some("check path"));
    }

    #[test]
    fn recoverable_error_extra_fields_roundtrip() {
        let mut e = RecoverableError::new("heading too large");
        e.extra
            .insert("file_id".into(), serde_json::json!("@file_abc"));
        e.extra.insert(
            "section_map".into(),
            serde_json::json!([{"level": 2, "text": "## X", "line": 10}]),
        );
        assert_eq!(e.extra["file_id"], "@file_abc");
        assert_eq!(e.extra["section_map"][0]["line"], 10);
    }

    #[test]
    fn is_regex_like_detects_alternation() {
        assert!(is_regex_like("foo|bar"));
        assert!(is_regex_like("foo|bar|baz"));
    }

    #[test]
    fn is_regex_like_detects_wildcards() {
        assert!(is_regex_like("foo.*bar"));
        assert!(is_regex_like("foo.+bar"));
        assert!(is_regex_like("foo.?bar"));
    }

    #[test]
    fn is_regex_like_detects_anchors() {
        assert!(is_regex_like("^main"));
        assert!(is_regex_like("name$"));
    }

    #[test]
    fn is_regex_like_detects_character_classes_with_range() {
        assert!(is_regex_like("[A-Z]foo"));
        assert!(is_regex_like("bar[0-9]"));
    }

    #[test]
    fn is_regex_like_detects_escape_sequences() {
        assert!(is_regex_like(r"\bword"));
        assert!(is_regex_like(r"foo\d+"));
        assert!(is_regex_like(r"\w+bar"));
        assert!(is_regex_like(r"foo\s"));
    }

    #[test]
    fn is_regex_like_detects_grouping() {
        assert!(is_regex_like("(foo|bar)"));
        assert!(is_regex_like("some(thing)"));
    }

    #[test]
    fn is_regex_like_rejects_plain_identifiers() {
        assert!(!is_regex_like("my_function"));
        assert!(!is_regex_like("MyStruct/method"));
        assert!(!is_regex_like("some-name"));
        assert!(!is_regex_like("CamelCase"));
        assert!(!is_regex_like("foo.bar"));
        assert!(!is_regex_like("Vec<String>"));
        assert!(!is_regex_like(""));
    }

    #[test]
    fn is_regex_like_rejects_lone_pipe() {
        assert!(!is_regex_like("|leading"));
        assert!(!is_regex_like("trailing|"));
    }

    #[test]
    fn is_regex_like_rejects_brackets_without_range() {
        assert!(!is_regex_like("[u8]"));
        assert!(!is_regex_like("[i32; 4]"));
    }

    // ---- call_content auto-buffering tests ----

    async fn bare_ctx() -> ToolContext {
        ToolContext {
            agent: crate::agent::Agent::new(None).await.unwrap(),
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                section_coverage::SectionCoverage::new(),
            )),
        }
    }

    struct EchoTool {
        result: Value,
        user_summary: Option<String>,
    }

    #[async_trait::async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo_tool"
        }
        fn description(&self) -> &str {
            "test"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({})
        }
        async fn call(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<Value> {
            Ok(self.result.clone())
        }
        fn format_compact(&self, _result: &Value) -> Option<String> {
            self.user_summary.clone()
        }
    }

    #[tokio::test]
    async fn call_content_passthrough_small_output() {
        let ctx = bare_ctx().await;
        let result = serde_json::json!({"key": "value"});
        let tool = EchoTool {
            result: result.clone(),
            user_summary: None,
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        // Small output: no buffering — content should contain the JSON
        assert_eq!(content.len(), 1, "small output should not be buffered");
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(text.contains("key"));
    }

    #[tokio::test]
    async fn call_content_small_output_ignores_format_compact() {
        // Even when format_compact returns Some, call_content must return exactly
        // 1 block with pretty JSON — the compact text is NOT injected into small outputs.
        let ctx = bare_ctx().await;
        let result = serde_json::json!({"key": "value"});
        let tool = EchoTool {
            result: result.clone(),
            user_summary: Some("compact summary".to_string()),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert_eq!(
            content.len(),
            1,
            "small output must produce exactly 1 block, got: {:?}",
            content
        );
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            text.contains("key"),
            "block must contain the JSON key, got: {}",
            text
        );
        assert!(
            !text.contains("compact summary"),
            "compact summary must NOT appear in small-output block, got: {}",
            text
        );
    }

    #[tokio::test]
    async fn call_content_buffers_large_output() {
        let ctx = bare_ctx().await;
        // Build a Value that serializes to >> 5_000 bytes (well above the buffer threshold)
        let big_array: Vec<Value> = (0..500)
            .map(|i| {
                serde_json::json!({
                    "index": i,
                    "name": format!("symbol_{}", i),
                    "file": "src/tools/symbol.rs"
                })
            })
            .collect();
        let result = serde_json::json!({ "symbols": big_array });
        let tool = EchoTool {
            result,
            user_summary: None,
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        // Must return exactly 1 Content item
        assert_eq!(content.len(), 1);
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        // Contains a @tool_ ref handle
        assert!(text.contains("@tool_"), "expected @tool_ ref in: {}", text);
    }

    #[tokio::test]
    async fn call_content_uses_format_compact_in_buffer_summary() {
        let ctx = bare_ctx().await;
        let big_array: Vec<Value> = (0..500)
            .map(|i| {
                serde_json::json!({
                    "index": i,
                    "name": format!("symbol_{}", i)
                })
            })
            .collect();
        let result = serde_json::json!({ "symbols": big_array });
        let tool = EchoTool {
            result,
            user_summary: Some("Found 500 symbols".to_string()),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            text.contains("Found 500 symbols"),
            "expected summary in: {}",
            text
        );
        assert!(text.contains("@tool_"), "expected ref handle in: {}", text);
    }

    #[tokio::test]
    async fn call_content_generic_fallback_without_format_compact() {
        let ctx = bare_ctx().await;
        let big_array: Vec<Value> = (0..500)
            .map(|i| {
                serde_json::json!({
                    "index": i,
                    "name": format!("symbol_{}", i)
                })
            })
            .collect();
        let result = serde_json::json!({ "symbols": big_array });
        let tool = EchoTool {
            result,
            user_summary: None,
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        // No format_compact → generic fallback message with byte count and ref
        assert!(
            text.contains("bytes") || text.contains("stored"),
            "expected fallback in: {}",
            text
        );
        assert!(text.contains("@tool_"), "expected ref handle in: {}", text);
    }

    // ---- threshold + summary-cap tests ----

    #[tokio::test]
    async fn call_content_buffers_at_token_threshold() {
        // Build a Value whose JSON is ~12 KB — above MAX_INLINE_TOKENS (2500 tokens ≈ 10 KB).
        let ctx = bare_ctx().await;
        let items: Vec<Value> = (0..150)
            .map(|i| {
                serde_json::json!({
                    "file": format!("src/tools/file_{}.rs", i),
                    "line": i,
                    "content": format!("let x_{} = some_function_call_{};\n", i, i)
                })
            })
            .collect();
        let result = serde_json::json!({ "matches": items, "total": items.len() });

        // Sanity: confirm the JSON exceeds the token-based threshold (~10 KB)
        let json_len = serde_json::to_string(&result).unwrap().len();
        assert!(
            json_len > MAX_INLINE_TOKENS * 4,
            "test data must exceed token threshold ({} bytes), got {} bytes",
            MAX_INLINE_TOKENS * 4,
            json_len
        );

        let tool = EchoTool {
            result,
            user_summary: Some("150 matches".to_string()),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            text.contains("@tool_"),
            "output exceeding token limit must be buffered, got: {}",
            &text[..text.len().min(200)]
        );
    }

    #[tokio::test]
    async fn call_content_does_not_buffer_under_token_limit() {
        // ~2 KB result — well under MAX_INLINE_TOKENS, must stay inline (no @tool_ ref)
        let ctx = bare_ctx().await;
        let items: Vec<Value> = (0..30)
            .map(|i| serde_json::json!({ "file": format!("src/a_{}.rs", i), "line": i }))
            .collect();
        let result = serde_json::json!({ "matches": items });

        let json_len = serde_json::to_string(&result).unwrap().len();
        assert!(
            json_len < 5_000,
            "test data must be < 5 KB, got {} bytes",
            json_len
        );

        let tool = EchoTool {
            result,
            user_summary: Some("30 matches".to_string()),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            !text.contains("@tool_"),
            "small output must not be buffered, got: {}",
            &text[..text.len().min(200)]
        );
    }

    #[tokio::test]
    async fn call_content_caps_compact_summary() {
        // format_compact returns a 4 KB summary — must be truncated to ≤ 3 KB (hard max)
        let ctx = bare_ctx().await;
        let items: Vec<Value> = (0..200)
            .map(|i| serde_json::json!({ "idx": i, "name": "x".repeat(50) }))
            .collect();
        let result = serde_json::json!({ "items": items });

        // Summary deliberately larger than hard cap
        let big_summary = format!("{}\n", "summary line ".repeat(300)); // ~3.9 KB
        assert!(
            big_summary.len() > 3_000,
            "summary must be > hard cap for this test"
        );

        let tool = EchoTool {
            result,
            user_summary: Some(big_summary),
        };
        let content = tool
            .call_content(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

        // Output is now a JSON object — parse it to check individual fields
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("call_content must return valid JSON");
        assert!(
            parsed["output_id"]
                .as_str()
                .unwrap_or("")
                .starts_with("@tool_"),
            "must have output_id: {parsed}"
        );
        // The summary field must be capped. truncate_compact appends "\n… (truncated)"
        // (~15 bytes) after the hard-max boundary, so allow a small suffix slack.
        let summary = parsed["summary"].as_str().unwrap_or("");
        assert!(
            summary.len() <= COMPACT_SUMMARY_HARD_MAX_BYTES + 20,
            "summary must be capped; got {} bytes",
            summary.len()
        );
        assert!(
            summary.contains("truncated"),
            "must include truncation note: {}",
            &summary[..summary.len().min(200)]
        );
        // hint must be present and reference the output_id
        let hint = parsed["hint"].as_str().unwrap_or("");
        assert!(
            hint.contains("@tool_"),
            "hint must reference the output_id: {hint}"
        );
    }

    // ---- truncate_compact tests ----

    #[test]
    fn truncate_compact_under_soft_cap_returns_verbatim() {
        let text = "line1\nline2\nline3";
        assert_eq!(super::truncate_compact(text, 2_000, 3_000), text);
    }

    #[test]
    fn truncate_compact_exact_soft_cap_returns_verbatim() {
        // Exactly at the soft cap — no truncation
        let text = "x".repeat(2_000);
        assert_eq!(super::truncate_compact(&text, 2_000, 3_000), text);
    }

    #[test]
    fn truncate_compact_at_line_boundary() {
        // Line 1 is 1,800 bytes; line 2 is 600 bytes → total 2,401 (> soft_max=2_000)
        // Last '\n' is at byte 1,800, which is ≤ hard_max=3_000 → truncate there
        let line1 = "a".repeat(1_800);
        let line2 = "b".repeat(600);
        let text = format!("{}\n{}", line1, line2);

        let result = super::truncate_compact(&text, 2_000, 3_000);

        assert!(result.starts_with(&line1), "should keep line1 intact");
        assert!(!result.contains(&line2), "should drop line2");
        assert!(
            result.contains("… (truncated)"),
            "should append truncation note"
        );
    }

    #[test]
    fn truncate_compact_no_newlines_uses_hard_cap() {
        // Single 5,000-byte line — no '\n' → hard-cap at 3,000 bytes
        let text = "x".repeat(5_000);
        let result = super::truncate_compact(&text, 2_000, 3_000);

        assert!(
            result.starts_with(&"x".repeat(3_000)),
            "should keep first 3,000 bytes"
        );
        assert!(result.ends_with("… (truncated)"), "should append note");
        // Sanity check: result is not longer than hard_max + note
        assert!(result.len() <= 3_000 + 20);
    }

    #[test]
    fn truncate_compact_preserves_text_exactly_at_hard_cap() {
        // Text is 2,500 bytes (> soft) with a single newline at position 2,400.
        // Line boundary (2,400) is between soft (2,000) and hard (3,000) — use it.
        let line1 = "a".repeat(2_400);
        let line2 = "b".repeat(99);
        let text = format!("{}\n{}", line1, line2);

        let result = super::truncate_compact(&text, 2_000, 3_000);

        assert!(result.starts_with(&line1), "should keep line1");
        assert!(!result.contains(&line2), "should not include line2");
        assert!(result.contains("… (truncated)"));
    }

    #[test]
    fn truncate_compact_unicode_does_not_panic() {
        // Regression test for the read_file crash on docs/ARCHITECTURE.md.
        // Box-drawing chars (─, │, ┌, etc.) are 3 bytes each in UTF-8.
        // A hard_max that lands mid-char must NOT cause a panic.
        let box_line: String = "─".repeat(700); // 2100 bytes
        let prefix = "x".repeat(100);
        let text = format!("{}\n{}", prefix, box_line); // >2000 bytes, no newline after 101

        // Must not panic regardless of where hard_max falls inside multi-byte chars.
        let result = super::truncate_compact(&text, 2_000, 3_000);
        assert!(result.contains("… (truncated)"), "should be truncated");
        // Result must be valid UTF-8 (no mid-char slices)
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn floor_char_boundary_lands_on_boundary() {
        let s = "ab─cd"; // 'a'=1, 'b'=1, '─'=3 bytes (E2 94 80), 'c'=1, 'd'=1
                         // bytes: 0='a', 1='b', 2-4='─', 5='c', 6='d'
        assert_eq!(super::floor_char_boundary(s, 0), 0);
        assert_eq!(super::floor_char_boundary(s, 2), 2); // before '─'
        assert_eq!(super::floor_char_boundary(s, 3), 2); // inside '─' → back to 2
        assert_eq!(super::floor_char_boundary(s, 4), 2); // inside '─' → back to 2
        assert_eq!(super::floor_char_boundary(s, 5), 5); // after '─'
        assert_eq!(super::floor_char_boundary(s, 6), 6);
        assert_eq!(super::floor_char_boundary(s, 100), s.len()); // clamp to len
    }

    #[test]
    fn safe_truncate_avoids_mid_char_split() {
        let s = "ab\u{2500}cd"; // 'a'=1, 'b'=1, '\u{2500}'=3 bytes, 'c'=1, 'd'=1
        assert_eq!(super::safe_truncate(s, 0), "");
        assert_eq!(super::safe_truncate(s, 2), "ab");
        assert_eq!(super::safe_truncate(s, 3), "ab"); // inside 3-byte char → round down
        assert_eq!(super::safe_truncate(s, 4), "ab"); // still inside
        assert_eq!(super::safe_truncate(s, 5), "ab\u{2500}");
        assert_eq!(super::safe_truncate(s, 100), s); // clamp to len
    }

    // ---- elicitation tests ----

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct TestConfirm {
        confirm: bool,
    }
    rmcp::elicit_safe!(TestConfirm);

    #[tokio::test]
    async fn elicit_returns_none_when_no_peer() {
        let ctx = bare_ctx().await;
        let result = ctx.elicit::<TestConfirm>("Test?").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn elicit_user_declined_is_recoverable_error() {
        // UserDeclined must produce a RecoverableError (isError: false at MCP level),
        // not a plain anyhow error (isError: true). We verify this by constructing the
        // error the same way the elicit() match arm does and checking the downcast.
        let e: anyhow::Error = RecoverableError::with_hint(
            "User declined the elicitation request",
            "Re-issue the call with a more specific argument to avoid the disambiguation prompt",
        )
        .into();
        assert!(
            e.downcast_ref::<RecoverableError>().is_some(),
            "UserDeclined must be a RecoverableError so it routes to isError:false"
        );
    }

    #[test]
    fn elicit_user_cancelled_is_recoverable_error() {
        // UserCancelled must produce a RecoverableError (isError: false at MCP level),
        // not a plain anyhow error (isError: true).
        let e: anyhow::Error = RecoverableError::with_hint(
            "User cancelled the elicitation request",
            "Re-issue the call with a more specific argument to avoid the disambiguation prompt",
        )
        .into();
        assert!(
            e.downcast_ref::<RecoverableError>().is_some(),
            "UserCancelled must be a RecoverableError so it routes to isError:false"
        );
    }
}

#[cfg(test)]
mod availability_tests {
    use super::*;
    use serde_json::Value;

    struct AlwaysTool;
    #[async_trait::async_trait]
    impl Tool for AlwaysTool {
        fn name(&self) -> &str {
            "always"
        }
        fn description(&self) -> &str {
            ""
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({})
        }
        async fn call(&self, _i: Value, _c: &ToolContext) -> anyhow::Result<Value> {
            Ok(serde_json::json!({}))
        }
    }

    #[test]
    fn default_availability_is_always() {
        let t = AlwaysTool;
        let caps = ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false,
        };
        assert!(t.availability(&caps).is_available(&ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false
        }));
        assert!(matches!(t.availability(&caps), Availability::Always));
    }

    #[test]
    fn availability_gates_toggle_correctly() {
        let off = ToolCapabilities {
            has_lsp: false,
            has_embeddings: false,
            has_git_remote: false,
            has_libraries: false,
        };
        let on = ToolCapabilities {
            has_lsp: true,
            has_embeddings: true,
            has_git_remote: true,
            has_libraries: true,
        };
        assert!(!Availability::RequiresLsp.is_available(&off));
        assert!(Availability::RequiresLsp.is_available(&on));
        assert!(Availability::Always.is_available(&off));
    }
}
