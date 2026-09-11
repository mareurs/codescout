//! Core types: Tool trait, ToolContext, RecoverableError, Guidance, OutputGuard-adjacent
//! constants/helpers, ToolCapabilities, and Availability.

use std::sync::Arc;

use anyhow::Result;
use rmcp::model::{Content, ToolAnnotations};
use rmcp::service::RoleServer;
use rmcp::Peer;
use serde_json::Value;

use crate::agent::Agent;
use crate::lsp::LspProvider;
use crate::tools::core::guide_emit::inject_hint;

/// Maximum estimated tokens for inline tool output.
/// Content exceeding this is buffered and summarized.
/// Token estimate: ~4 bytes per token.
// cap-class: RESULT_CAP tool_output.inline_tokens — probed
pub(crate) const MAX_INLINE_TOKENS: usize = 2_500;

/// Byte equivalent of MAX_INLINE_TOKENS — used for byte-budget arithmetic
/// in truncation code (run_command buffer-only paths).
// cap-class: RESULT_CAP run_command.inline_bytes — probed
pub(crate) const TOOL_OUTPUT_BUFFER_THRESHOLD: usize = MAX_INLINE_TOKENS * 4;

/// Byte budget for auto-chunked inline content. Set to 90% of
/// TOOL_OUTPUT_BUFFER_THRESHOLD to leave headroom for the JSON envelope
/// overhead (~500-1000 bytes for content/complete/next/shown_lines keys).
// cap-class: RESULT_CAP tool_output.inline_byte_budget — probed
pub(crate) const INLINE_BYTE_BUDGET: usize = TOOL_OUTPUT_BUFFER_THRESHOLD * 9 / 10;

/// Soft line-count nudge for markdown default reads.
///
/// Files whose line count exceeds this threshold — but whose byte size still
/// fits `INLINE_BYTE_BUDGET` — get full content plus a focused-read hint.
/// Files larger than `INLINE_BYTE_BUDGET` are buffered regardless of line count.
// cap-class: NOT_A_CAP — soft nudge: above it the caller still receives full content plus a focused-read hint, so nothing is withheld
pub(crate) const LINE_SOFT_CAP: usize = 150;

/// Heading-count gate for escalation to MAP shape. A markdown file with more
/// than this many headings is structurally a directory — content is skim-only
/// and the caller wants to pivot. Escalates Tier 2 → Tier 3 regardless of
/// byte/line budgets. Closes Hamsa eval B1 (many-headings.md, 251 sections).
// cap-class: RESULT_CAP markdown.headings_hard_cap — probed
pub(crate) const HEADINGS_HARD_CAP: usize = 40;

/// Check whether content should be buffered based on estimated token count.
pub(crate) fn exceeds_inline_limit(text: &str) -> bool {
    text.len() / 4 > MAX_INLINE_TOKENS
}

/// Soft cap for compact summaries shown alongside `@tool_*` refs.
/// Truncation prefers whole-line boundaries. See [`truncate_compact`].
// cap-class: RESULT_CAP tool_output.compact_summary_bytes — probed
pub(crate) const COMPACT_SUMMARY_MAX_BYTES: usize = 2_000;
/// Hard cap — no summary will exceed this size regardless of line boundaries.
// cap-class: RESULT_CAP tool_output.compact_summary_hard_bytes — probed
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
    /// `workspace(action="activate")` touches this conditionally, not always:
    /// a genuine project switch re-arms just the project-scoped topic when a
    /// companion rendezvous is active, falls back to clearing the whole set
    /// when it isn't (a `/clear` would otherwise be invisible to the server),
    /// and a same-project re-activation leaves it alone entirely. See
    /// `ActivateProject::call` (`PROJECT_SCOPED`, `rendezvous_active`).
    pub guide_hints_emitted: Arc<parking_lot::Mutex<crate::tools::guide_ledger::GuideLedger>>,
    /// Per-request workspace pin (Phase 2 plumbing for per-request workspace
    /// resolution). Populated in `call_tool_inner` from an optional `workspace`
    /// input field, canonicalized to match the registry's canonical-root keys.
    /// No tool reads it yet — Phase 3 wires the selector-aware accessors
    /// (`with_project_at`). See
    /// docs/plans/2026-05-30-per-request-workspace-pinning.md.
    pub workspace_override: Option<std::path::PathBuf>,
}

/// Notice that reads are resolving against a tree the caller never chose.
///
/// `guard_worktree_write` refuses WRITES on exactly these two facts. Reads had
/// no equivalent, so after a session switched into a linked worktree, `symbols`,
/// `grep` and `read_file` kept answering from the main checkout and said nothing
/// about it — a silent wrong answer, which is the expensive kind: the agent reads
/// plausible code from the wrong tree and acts on it. The asymmetry was the bug;
/// the write guard proved the condition is cheaply detectable and then spent the
/// detection on half the surface.
///
/// **A notice, not a refusal.** Refusing reads would fire while the agent is
/// still orienting — exactly when it cannot yet know which tree it wants — and a
/// guard that fires before the caller can plausibly satisfy it trains callers to
/// route around it. Same philosophy as `removed_attributes`: the operation is
/// allowed, and it says what it did.
///
/// **Gated on the CONDITION, never on novelty — this is load-bearing and was
/// once the other way.** Until 2026-09-02 the last gate here was
/// `notice_once(WORKTREE_READ_NOTICE)`, one-shot per conversation, on the stated
/// grounds that "a notice on every call is noise". The measured consequence was
/// worse than noise: the notice fired once, early, while the agent was still
/// orienting and had no worktree work to do — and was then **silent for the rest
/// of the conversation**, including across every `/mcp` reconnect. A reconnect
/// clears `project_chosen_this_session` and so *recreates* the exact state this
/// notice describes, but re-arms nothing: only `GuideLedger::clear` (an
/// `activate`, or a post-compact) and `rekey` restore the key, and an `activate`
/// is precisely the act that makes the notice unnecessary. So the one-shot was
/// structurally guaranteed to be spent on the wrong episode.
///
/// Measured on a live transcript, `docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`:
/// an unpinned read and a *refused* write in the same window — the refusal proving
/// the slot was empty and worktrees present — carried no notice, while the first
/// read after a ledger clear carried one. Six reconnects, zero notices; one clear,
/// one notice.
///
/// The condition is self-limiting, which is why emitting on every call is
/// affordable: it requires linked worktrees to EXIST and no tree to have been
/// chosen, so a repo without worktrees never sees it, and the two documented
/// remedies — `workspace(action='activate')`, or passing `workspace=` per call —
/// each silence it permanently. The correct path ends in a quiet state, so
/// compliance leaves nothing armed.
///
/// **A pinned call is silent**, because it already named the tree it meant.
/// `workspace_override` is the per-call form of the choice `activate` makes for
/// the session; warning a caller who supplied it would be reporting a
/// resolution that cannot be wrong.
///
/// **Agent-agnostic by construction.** Both facts are git/filesystem
/// observations; nothing here learns any harness's worktree tool by name.
///
/// Ordered cheapest-check-first.
///
/// docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md
async fn worktree_read_notice(ctx: &ToolContext, root: Option<&std::path::Path>) -> Option<String> {
    if ctx.workspace_override.is_some() {
        // The call named its own tree. Nothing was resolved by default, so there
        // is no default to disclose.
        return None;
    }
    if ctx.agent.is_project_chosen_this_session().await {
        // The caller already made the choice this notice would ask for.
        //
        // Deliberately NOT `is_project_explicitly_activated`: that one is set at
        // startup from the `current_dir()` fallback, so it is true in essentially
        // every session and would suppress this notice everywhere.
        // `guard_worktree_write` used to make that same mistake — fixed 2026-08-17,
        // see docs/issues/archive/2026-08-16-worktree-write-guard-is-dead-code-in-production.md —
        // and now gates on this same flag, for this same reason.
        return None;
    }
    let root = root?;
    let worktrees = crate::util::path_security::list_git_worktrees(root);
    if worktrees.is_empty() {
        return None;
    }
    let list: Vec<String> = worktrees.iter().map(|p| p.display().to_string()).collect();
    Some(format!(
        "Reads are resolving against \"{}\". This repo also has linked git \
         worktrees [{}] and no project has been explicitly activated, so results \
         describe the main checkout even if you are working in a worktree. Call \
         workspace(action='activate', path=\"{}\") to pin the tree you mean, or \
         pass workspace=\"<abs path>\" on a single call.",
        root.display(),
        list.join(", "),
        list[0],
    ))
}

/// Tools whose `is_write` is true but whose write does not land at a path
/// resolved against the project root, so naming a checkout would mislead:
/// `approve_write` grants a scope rather than writing a file, and the library
/// registry is global rather than per-project.
const WRITE_ROOT_ANNOTATION_EXEMPT: &[&str] = &["approve_write", "register_library", "library"];

/// Name the checkout an UNPINNED write landed in, when the repo has linked
/// worktrees.
///
/// The third member of the family with [`guard_worktree_write`] and
/// [`worktree_read_notice`], and the one covering the case both of those miss.
/// Each of them gates on `is_project_chosen_this_session`, so both fall silent
/// the moment a session calls `activate` — which is exactly when the leak this
/// closes happens. A subagent pinning its writes per call drops the pin on one;
/// that write resolves against the session default and answers with a bare
/// `status: ok`, indistinguishable from the calls that landed correctly.
///
/// **A notice, not a refusal** — same philosophy as `worktree_read_notice`, and
/// here the choice is measured rather than argued. Over 195,126 tool calls
/// (2026-03-21..2026-08-27), a *refusal* armed by prior pin use in the session
/// would have blocked 9,914 writes — 30.3% of every write in the corpus — to
/// catch at most 18 genuine misroutes: ~551 false refusals per true catch. Two
/// reasons it was that bad, both worth not rediscovering: `list_git_worktrees`
/// is a property of the REPO, not the session, so a stale worktree on disk arms
/// it in sessions that never touch one; and 63% of the arming pins named an
/// unrelated repo, after which every ordinary home-project write is refused. An
/// annotation withholds nothing, so it has no false-positive cost at all.
/// docs/issues/archive/2026-08-27-edit-code-writes-to-session-default-not-pinned-workspace.md
///
/// Silent unless the repo genuinely has linked worktrees, so the overwhelmingly
/// common single-checkout case is untouched — including every test tempdir,
/// which is why promoting a bare `"ok"` to an object here disturbs the no-echo
/// write convention nowhere else.
fn annotate_write_root(val: &mut Value, root: &std::path::Path) {
    if crate::util::path_security::list_git_worktrees(root).is_empty() {
        return;
    }
    // No-echo writes answer with a bare `"ok"`. Promote that one shape to an
    // object so the annotation has somewhere to live. Anything that is neither
    // a string nor an object (an array, a number) is left alone rather than
    // reshaped — losing a tool's answer to add a note would be a bad trade.
    if let Some(status) = val.as_str() {
        *val = serde_json::json!({ "status": status });
    }
    if let Some(obj) = val.as_object_mut() {
        obj.insert(
            "wrote_to".to_string(),
            Value::String(root.display().to_string()),
        );
    }
}

/// Name the path a write targeted, so a `path~` predicate has something to match.
///
/// Sibling to [`annotate_write_root`] and deliberately separate from it. That one
/// answers *which checkout did this reach*, and is gated on the call being
/// **unpinned**, because a pinned call already named its target. This one answers
/// *which file did this write*, which is a fact about the call whether or not a
/// workspace was pinned — so it is gated on `is_write` alone.
///
/// Without it, `OP-4` (`**Serves:** edit_file(path~/.claude)`) could never match:
/// `names_path_containing` scans the response, writes answer `"ok"` under the
/// no-echo convention, and the one field promotion added was `wrote_to` — the
/// project ROOT, which for a write to `~/.claude/…` names the wrong thing entirely.
/// `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`.
///
/// **This is a narrow, deliberate exception to no-echo, not its repeal.** The
/// convention exists so a write does not echo the *content* it wrote — the reason
/// `call_content` refuses to clone its own input is that `edit_file`/`create_file`
/// carry whole file bodies. A path is bounded, is already half-present as
/// `wrote_to`, and tells the caller where the write landed. See
/// `docs/adrs/2026-08-31-write-responses-name-the-path-they-wrote.md`.
///
/// Keyed by absoluteness rather than always `abs_path`, because
/// `names_path_containing` scans both and a relative path filed under `abs_path`
/// would be a lie the matcher happens not to notice. Never overwrites a path the
/// tool set itself.
fn annotate_write_path(val: &mut Value, path: &str) {
    // Same promotion as `annotate_write_root`: a bare string has nowhere to hold
    // an annotation, so it becomes `{"status": <the string>}` rather than being
    // discarded.
    if let Some(status) = val.as_str() {
        *val = serde_json::json!({ "status": status });
    }
    let key = if std::path::Path::new(path).is_absolute() {
        "abs_path"
    } else {
        "rel_path"
    };
    if let Some(obj) = val.as_object_mut() {
        obj.entry(key)
            .or_insert_with(|| Value::String(path.to_string()));
    }
}

/// Nest the parameter-alias advisory under `corrections.param_aliases` in `obj`,
/// preserving whatever already lives at `corrections`.
///
/// ONE function, called from BOTH of `call_content`'s object-shaped render paths —
/// the small-output success path and the error path — so the advisory's address is
/// a property of the mechanism rather than of which branch you landed on. It was
/// inline in the success path until the error path needed the same three arms; a
/// second copy of them is exactly the drift this file's own comments warn about.
///
/// **Why nested under a dedicated key rather than merged key-by-key.** `c` is shaped
/// `{params, hint}` (Ruling 9 of
/// `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`), deliberately
/// mirroring `find.rs`'s `{filter, hint}` — one field name, one concept. That shared
/// shape is why a flat per-key insert is wrong: `hint` collides with every in-tree
/// writer of `corrections` (`find.rs`, and any future one following the convention),
/// so a flat merge would silently overwrite the tool's own teaching text with ours.
///
/// **And UNCONDITIONALLY nested, including when nothing else wrote `corrections`.**
/// An earlier round kept that arm flat to protect pinned assertions, which gave the
/// advisory two addresses depending on a fact the caller cannot see. Per this file's
/// model for [`Guidance`], "the field name itself carries the register — agents scan
/// JSON responses and react to the key, not the prose": an agent that learned
/// `corrections.param_aliases` must not get a false negative from a flat
/// `corrections` produced by a tool with nothing of its own to say.
///
/// `param_aliases` is NOT collision-proof by construction — a key naming a mechanism
/// is the key another reporter of that mechanism reaches for. Verified zero today,
/// not assumed zero: ten tools implement `param_aliases()` (`create_file`,
/// `edit_file`, `grep`, `read_file`, `edit_code`, `references`, `symbol_at`,
/// `call_graph`, `doc`, `symbols`) and none writes its own `corrections`;
/// `corrections` is written only by `find.rs` and `update.rs`, both under `doc`.
/// Re-verify that pairing before trusting it stays empty.
pub(crate) fn merge_param_corrections(obj: &mut serde_json::Map<String, Value>, c: &Value) {
    match obj.get_mut("corrections") {
        Some(existing) if existing.is_object() => {
            if let Some(existing_obj) = existing.as_object_mut() {
                existing_obj.insert("param_aliases".to_string(), c.clone());
            }
        }
        Some(existing) => {
            // Already holds a non-object shape (e.g. `update.rs`'s bare array from a
            // top-level-param lift). Leaving it alone reads as "don't clobber the
            // tool's value" but actually drops the framework's own advisory entirely
            // — nothing else re-attaches it. Promote `corrections` into an object
            // that keeps the tool's original value under `tool` and adds ours under
            // `param_aliases`, so both reach the caller.
            let tool_value = existing.take();
            *existing = serde_json::json!({
                "tool": tool_value,
                "param_aliases": c.clone(),
            });
        }
        None => {
            obj.insert(
                "corrections".to_string(),
                serde_json::json!({ "param_aliases": c.clone() }),
            );
        }
    }
}

/// Attach the parameter-alias advisory to an error on its way out of `call_content`.
///
/// **The defect this closes.** `call_content` repairs aliases at its first statement,
/// then renders the advisory at three sites — all of them below the `?` on
/// `self.call(input, ctx).await`. Any `Err` early-returned past every one of them, so
/// a caller whose parameter name had just been silently rewritten learned nothing
/// about it on exactly the calls it pays most attention to. Wire-confirmed 2026-09-11
/// against binary `7e08645f` as a two-cell experiment on one tool, one alias, varying
/// only Ok/Err: `symbols(query="OutputGuard")` returned the advisory;
/// `symbols(query="Tool|Doc", symbol="x")` returned the regex refusal with no mention
/// of `query` — and that refusal's own remedy ("fix the pattern, resend") routes the
/// caller straight back through the alias it was not told about.
/// `docs/issues/archive/2026-09-10-a-repaired-alias-is-never-announced-on-the-error-path.md`.
///
/// **Two carriers here is a fact about THIS function, never a scope for the dispatch
/// it feeds — and publishing it as one is what produced `85856feb201949bc`.** The
/// sentence that used to open this paragraph said "both error types", which is true
/// and was read as coverage; `route_tool_error` had *three* outcomes, and the middle
/// one consulted neither carrier. Derive the count at the surface you actually mean.
/// Counted from the code 2026-09-11, not transcribed:
///
/// - **This function: 2 carriers**, one per arm of `e.downcast::<RecoverableError>()`
///   — the `extra` map on `Ok`, an [`AdvisedError`] wrapper on `Err`. A third error
///   type with its own splice point would move this number and only this one.
/// - **`route_tool_error`: 3 outcomes** — `RecoverableError`, the LSP-transient
///   `-32800`/`-32801` arm, and the fatal `else`. A fourth arm moves this number and
///   no other, which is the whole reason it is no longer the advisory's bound.
/// - **`route_tool_error`: 2 response SHAPES**, and this is the count that does bound
///   the advisory's addresses. An object-shaped body (arms 1 and 2) takes it at
///   `corrections.param_aliases`, the same address the success path uses; plain text
///   (arm 3) cannot hold an object at all, so it takes the `⚠ {hint}` prefix.
///   `ErrorRender` (`src/server.rs`) is that pair made explicit and attached once
///   after arm selection, so an arm added later cannot be added without it.
/// - **`call_content`: 4 render paths** — the buffered envelope, the
///   `OutputForm::Text` compact render, the pretty-JSON value, and this error path,
///   which is the fourth and was added by `295a928e` after the spec, the governing
///   ADR and three separate task reviews had all enumerated three.
///
/// The two carriers themselves. A
/// [`RecoverableError`] (`isError: false`, "bad input, self-correct and retry")
/// already had a splice point: `route_tool_error` puts its `extra` map into the
/// response body at the top level, so the advisory lands at
/// `corrections.param_aliases` — the same address the success path uses.
///
/// A plain `anyhow` error (`isError: true`, "stop and surface to the user") had no
/// splice point — `route_tool_error` deliberately sends only the outermost message
/// on that branch and logs the context chain server-side, because an error oracle
/// over the HTTP transport can leak filesystem layout to an authenticated-but-
/// untrusted client. Widening the message there directly would either overwrite the
/// original error text (`anyhow::Context` prepends) or drop the `.source()` chain
/// the log depends on (`format!` flattens it to a `String`). [`AdvisedError`] is the
/// wrapper this needed: it carries the advisory as a separate field while its
/// `Display`/`Debug`/`source()` all delegate to the inner error unchanged, so
/// `route_tool_error` still shows the tool's own message and still logs the full
/// chain. What it gains depends on which SHAPE the dispatch selected, not on which
/// arm: on the fatal text, a `⚠ {hint}\n\n` prefix, matching the same prefix
/// convention `call_content`'s own compact-text success renderer already uses for
/// this same `hint` string; on an object-shaped body — the LSP-transient arm — the
/// `corrections.param_aliases` key instead, because that shape can hold an object and
/// a caller should parse one address.
/// `docs/issues/archive/2026-09-11-the-alias-advisory-still-does-not-reach-the-plain-anyhow-error-path.md`.
///
/// **The alternative carrier considered and rejected: joining `RecoverableError`'s
/// existing [`Guidance`] (`hint` / `warning` / `must_follow`).** A `RecoverableError`
/// carries at most one `Guidance`, so joining would have meant either overwriting the
/// tool's own recovery text — the bug file's whole point is that both facts must
/// arrive together — or concatenating two registers into one field.
///
/// **Also out of scope, and not a hole:** `call_content`'s ambiguous-write refusal
/// returns before `param_corrections` is ever built. It needs no advisory, because its
/// message already names both alias keys and both discarded values.
pub(crate) fn attach_param_corrections_to_error(
    e: anyhow::Error,
    c: Option<&Value>,
) -> anyhow::Error {
    let Some(c) = c else { return e };
    match e.downcast::<RecoverableError>() {
        Ok(mut rec) => {
            merge_param_corrections(&mut rec.extra, c);
            rec.into()
        }
        // Not a `RecoverableError` — wrap it so the advisory still reaches
        // `route_tool_error`, without touching Display/Debug/source(). See the
        // doc comment above.
        Err(e) => AdvisedError {
            inner: e,
            corrections: c.clone(),
        }
        .into(),
    }
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

/// Carries a plain `anyhow` error's alias-correction advisory across
/// `route_tool_error`'s success/error boundary. Internal plumbing between
/// `attach_param_corrections_to_error` and `route_tool_error` only — never
/// constructed by a tool's own `call()`, unlike [`RecoverableError`].
///
/// Exists to avoid the two failure modes `attach_param_corrections_to_error`'s
/// doc comment identifies for the obvious alternatives: it does not touch the
/// inner error's `Display` (so `route_tool_error` still shows the tool's own
/// message, not a wrapper's), and it does not flatten the source chain into a
/// `String` (so server-side logging via `tracing::error!("{:#}", …)` still
/// walks it).
pub(crate) struct AdvisedError {
    pub(crate) inner: anyhow::Error,
    pub(crate) corrections: Value,
}

impl std::fmt::Display for AdvisedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "{:#}", self.inner)
        } else {
            write!(f, "{}", self.inner)
        }
    }
}

impl std::fmt::Debug for AdvisedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.inner, f)
    }
}

impl std::error::Error for AdvisedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

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
    /// False when `security.shell_command_mode = "disabled"`, which hides
    /// `run_command` from the advertised surface entirely.
    ///
    /// Derived as `mode != "disabled"`, deliberately NOT
    /// `mode == "warn" || mode == "unrestricted"`: a typo'd mode must keep the
    /// tool LISTED so the call reaches `run_command_inner`'s
    /// `unknown shell_command_mode: '<x>'` error. Whitelisting the two good
    /// values instead would make a misconfiguration present as a silently
    /// missing tool — the one symptom that gives the caller nothing to fix.
    ///
    /// Unlike the other four, this one is a policy read rather than a
    /// capability probe, and it is only ever an OPTIMISATION: it trims the
    /// description + schema an agent would otherwise pay for and reach for.
    /// Enforcement stays in `run_command_inner`, which is load-bearing rather
    /// than redundant — see `Availability::RequiresShell`.
    pub shell_enabled: bool,
}

/// Conditional-exposure constraint for a `Tool`.
#[derive(Debug, Clone, Copy)]
pub enum Availability {
    Always,
    RequiresLsp,
    RequiresEmbeddings,
    RequiresGitRemote,
    RequiresLibraries,
    /// Shell is not `"disabled"` in the SESSION-DEFAULT project's security
    /// config.
    ///
    /// **This gate cannot replace the refusal in `run_command_inner`, and the
    /// reason is structural rather than defensive.** `current_capabilities()`
    /// probes the session-default project (`Agent::with_project`), whereas
    /// `run_command` reads `security_config_for(ctx.workspace_override)` — the
    /// PINNED config. A `workspace`-pinned call into a shell-disabled project
    /// is therefore invisible to `list_tools` filtering by construction, since
    /// the pin lives in the request and the tool list predates it. Deleting the
    /// `inner.rs` check because "the tool is hidden now" would reopen exactly
    /// that path. (An MCP client is also free to call a tool it was never
    /// advertised.)
    ///
    /// The converse asymmetry is a real limitation, not an oversight: an MCP
    /// tool list is per-session, not per-request, so a session whose DEFAULT
    /// project disables shell loses `run_command` from its list even while
    /// pinning calls at a project that allows it. The refusal would permit such
    /// a call; the list is simply computed before the pin exists. Anyone who
    /// needs both in one session wants the default project to be the permissive
    /// one.
    RequiresShell,
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
            Availability::RequiresShell => c.shell_enabled,
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
/// `doc(get)` result advertised `$.tags[*]` (4 strings) for a buffer whose
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
    // cap-class: RESULT_CAP tool_output.largest_array_depth — probed
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

/// The `<tool>.<action>` projection every tool's selector key is built from.
///
/// **This is `Tool::selector_key`'s default body as of 2026-09-01 (`30b6fc41`), so it is no
/// longer an opt-in helper — it is the mechanism.** All 21 registered tools reach routing
/// through it, and nothing overrides `selector_key`.
///
/// It was extracted as a shared helper when three tools opted in separately, so they could
/// not drift apart in a way that makes one rule route while its sibling silently does not —
/// `OP-4` names `edit_file` and `create_file` in a single rule, and a half-routable rule is
/// harder to notice than an unroutable one, because it fires for some writes and not others.
/// Promoting it to the default RETIRED that risk rather than mitigating it: with one
/// implementation there is nothing left to diverge.
///
/// An action-less call projects the **bare tool name**, never `None`. `Shape::matches` reads
/// `None` as "cannot match", so returning it for a tool-only shape would make such
/// declarations permanently unmatchable — a silent-absence failure rather than the
/// fail-safe-toward-delivery direction this feature requires.
///
/// *(This doc used to close by saying `LibrarianAdapter::selector_key` "should adopt this
/// once free". It did, by deletion — its body was byte-identical to this one. Do not go
/// looking for that override; `30b6fc41` removed all four. Noted because the sentence
/// survived that commit's sweep of the comments it falsified: the four corrected ones LIVED
/// IN the deleted code, and this one POINTED AT it. Found by codescout-b7, not by me — see
/// `reconnaissance-patterns:R-165`.)*
pub(crate) fn action_selector_key(name: &str, input: &Value) -> Option<String> {
    match input.get("action").and_then(Value::as_str) {
        Some(action) => Some(format!("{name}.{action}")),
        None => Some(name.to_string()),
    }
}

/// Named MCP tool-annotation shapes.
///
/// The byte-cost policy lives here rather than being re-decided in each `impl Tool`,
/// for the same reason [`Tool::pinnable`] keeps its name list in the trait default: the
/// rule is easy to state once and easy to violate twenty times.
///
/// **The rule: emit only what differs from the MCP defaults**, which are
/// `readOnly=false`, `destructive=true`, `idempotent=false`, `openWorld=true`. Every
/// field emitted is paid on every request of every session and counted by
/// `server::tests::tool_surface_under_budget`, whose constant is lower-only.
///
/// Two consequences that are easy to get backwards:
/// - `openWorld` defaults to **true**, so leaving it unset is an active claim that the
///   tool reaches outside the project — a local-only reader must set it `false`.
/// - `destructive` and `idempotent` are spec-meaningless when `readOnly` is true, so
///   pairing them with it is pure dead weight.
///
/// A tool whose real behaviour already matches the defaults — a destructive, non-
/// idempotent, open-world writer such as `doc`, `librarian`, `memory` or `run_command` —
/// returns `None` and costs zero bytes.
pub mod annot {
    use rmcp::model::ToolAnnotations;

    /// A pure reader confined to the project: local files, the AST/LSP index.
    ///
    /// The LSP-backed readers are included here as a deliberate judgement: they talk to a
    /// local stdio subprocess, though rust-analyzer's own `cargo metadata` can reach
    /// crates.io on a cold registry. That reach is transitive and incidental, not the
    /// tool's own domain of interaction.
    pub fn read_only_closed() -> Option<ToolAnnotations> {
        Some(ToolAnnotations::default().read_only(true).open_world(false))
    }

    /// A pure reader that may reach an external service, so `openWorld` is left at its
    /// `true` default. Applies to the embedding-backed readers, whose backend resolves at
    /// runtime — `local:`/`local-dir:` are in-process ONNX, but `openai:`/`ollama:`/a bare
    /// `url` are HTTP to an arbitrary host.
    pub fn read_only_open() -> Option<ToolAnnotations> {
        Some(ToolAnnotations::default().read_only(true))
    }

    /// A writer confined to the project whose effects may be destructive; `destructive`
    /// and `idempotent` stay at their conservative defaults.
    pub fn writer_closed() -> Option<ToolAnnotations> {
        Some(ToolAnnotations::default().open_world(false))
    }

    /// A writer confined to the project whose updates are additive and whose repeat call
    /// has no further effect.
    pub fn additive_closed() -> Option<ToolAnnotations> {
        Some(
            ToolAnnotations::default()
                .destructive(false)
                .idempotent(true)
                .open_world(false),
        )
    }

    /// As [`additive_closed`], but the tool may reach an external service.
    pub fn additive_open() -> Option<ToolAnnotations> {
        Some(
            ToolAnnotations::default()
                .destructive(false)
                .idempotent(true),
        )
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
    /// Cap on `description()` length in **characters**, enforced by
    /// `server::tests::every_tool_description_is_under_its_cap`. 300 by default.
    /// A multi-action dispatcher whose description must name every action
    /// (`doc`, `librarian`) overrides to 1_800. Measured 2026-09-02: `librarian`'s
    /// description is 1,621 chars / 1,623 bytes, leaving 179 chars of headroom under
    /// this char cap — the unit this trait counts. This replaces `is_librarian_tool`,
    /// a name-prefix list in `server.rs` tests that a rename silently orphaned.
    fn description_cap(&self) -> usize {
        300
    }

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
    /// ignore. The librarian family (`doc`, `librarian`) IS
    /// pinnable: its adapter resolves `ctx.workspace_override` before deriving
    /// each call's `current_project` (see `LibrarianAdapter::call`), so a
    /// `workspace=` pin scopes catalog reads and writes to the named workspace.
    /// See docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md.
    fn pinnable(&self) -> bool {
        !matches!(
            self.name(),
            "workspace"
                | "activate_project"
                | "onboarding"
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

    /// Static MCP tool annotations advertised in `list_tools`.
    ///
    /// Defaults to `None` — **no `annotations` key on the wire at all** — which is both
    /// the cheapest and the most conservative shape. The MCP defaults are
    /// `readOnly=false`, `destructive=true`, `idempotent=false`, `openWorld=true`, so an
    /// un-annotated tool is already treated as a destructive, open-world writer. Returning
    /// an empty `ToolAnnotations` instead would serialise `"annotations":{}` and cost ~17
    /// bytes per tool for nothing.
    ///
    /// Override only to say something *different* from those defaults:
    /// - a pure reader sets `read_only(true)`, plus `open_world(false)` if it stays local
    ///   — note `openWorld` defaults to **true**, so leaving it unset is not a safe
    ///   omission, it is an active claim that the tool reaches the outside world;
    /// - an additive-only writer sets `destructive(false)`;
    /// - `destructive` and `idempotent` are spec-meaningless when `read_only` is true, so
    ///   never pair them with it — they would be dead bytes.
    ///
    /// Every field emitted is paid on every request of every session;
    /// `server::tests::tool_surface_under_budget` counts them and
    /// `TOOL_SURFACE_CHAR_BUDGET` is lower-only.
    ///
    /// **This is the PER-TOOL axis; [`Tool::is_write`] is PER-CALL, and neither derives
    /// from the other.** `is_write` gates the cross-process write lock and nothing else:
    /// `run_command` has no override and reports `false` even for `rm -rf src`, while
    /// `workspace` reports `false` yet `action="activate"` writes
    /// `.codescout/libraries.json` (`src/library/auto_register.rs:64-65`). Classify from
    /// what the tool actually does, never by lifting `is_write`.
    ///
    /// These are **hints, not security guarantees** — a client shapes confirmation and
    /// auto-approval UX from them; nothing enforces them. Agreement with `is_write` in the
    /// one direction that matters (a writer must not claim `readOnlyHint: true`) is pinned
    /// by `server::tests::annotations_agree_with_is_write`.
    fn annotations(&self) -> Option<ToolAnnotations> {
        None
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
    async fn call_content(&self, mut input: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
        // Parameter-alias normalization is the FIRST statement in this function,
        // ahead of everything below that reads `input`: `selector_key`,
        // `is_write` and `write_path` all inspect it pre-`call()`, and
        // `write_path` in particular reads the literal key "path" — so a
        // `create_file(file_path=…)` call would lose its write-path annotation
        // entirely if normalization ran later. Mutates `input` in place: never
        // clone `input` itself, which for `create_file`/`edit_file` carries a
        // whole file body.
        // docs/adrs/2026-07-10-repair-and-continue-input-handling.md § Amendment 2026-09-10.
        let aliases = self.param_aliases();
        // Snapshot the raw value under each alias key BEFORE normalization can
        // remove it. This is needed only for the ambiguous-write escalation
        // below, which must name the discarded value, not just its key — and
        // by the time `normalize_params` returns, a superseded alias's value is
        // gone (it was `obj.remove`d and never reinserted anywhere). Clones a
        // handful of short strings, never `input` itself.
        let alias_values: std::collections::HashMap<&'static str, String> = aliases
            .iter()
            .filter_map(|(received, _)| {
                input
                    .get(*received)
                    .and_then(Value::as_str)
                    .map(|v| (*received, v.to_string()))
            })
            .collect();
        let corrections = crate::tools::param_alias::normalize_params(&mut input, aliases);
        let param_notice = crate::tools::param_alias::correction_notice(self.name(), &corrections);
        // Ruling 6 (ADR amendment above): two aliases racing for one canonical
        // key, with the canonical never supplied directly, is genuine
        // ambiguity — "more than one plausible reading" — and the ADR's
        // write-asymmetry clause ("auto-accepting an explicit write target is
        // safe; auto-guessing one must still hard-error") requires a hard
        // error here rather than silently writing the first alias's value and
        // discarding the second. Reads keep repair-and-note; only
        // `superseded_by` (never plain `conflicted`, where the caller supplied
        // the canonical directly — an explicit target) escalates, and only for
        // writes. `normalize_params` itself stays infallible — this is the
        // boundary's decision, not the rewriter's.
        if self.is_write(&input) {
            if let Some(bad) = corrections.iter().find(|c| c.superseded_by.is_some()) {
                let winner_key = bad.superseded_by.clone().unwrap_or_default();
                let winner_val = alias_values
                    .get(winner_key.as_str())
                    .cloned()
                    .unwrap_or_default();
                let loser_val = alias_values
                    .get(bad.received.as_str())
                    .cloned()
                    .unwrap_or_default();
                let canonical = bad.canonical;
                let loser_key = bad.received.clone();
                return Err(RecoverableError::with_hint(
                    format!(
                        "{tool}: ambiguous write target — '{winner_key}' = \"{winner_val}\" and \
                         '{loser_key}' = \"{loser_val}\" were both supplied for '{canonical}', and \
                         '{canonical}' itself was never sent. Refusing to guess which one you meant.",
                        tool = self.name(),
                    ),
                    format!(
                        "Send '{canonical}' directly, or send only one of '{winner_key}' / '{loser_key}'."
                    ),
                )
                .into());
            }
        }
        // Ruling 9: the advisory is an OBJECT keyed by what was corrected, plus
        // its own `hint` — matching the shape `find.rs`/`update.rs` already use
        // for `corrections`, not a bare string (a third shape for one concept).
        // Built ONCE, after normalization, before `self.call`; cloned into each
        // render site below.
        let param_corrections: Option<Value> = (!corrections.is_empty()).then(|| {
            serde_json::json!({
                "params": corrections.iter().map(|c| serde_json::json!({
                    "received": c.received,
                    "canonical": c.canonical,
                    "conflicted": c.conflicted,
                    "superseded_by": c.superseded_by,
                })).collect::<Vec<_>>(),
                "hint": param_notice.clone().unwrap_or_default(),
            })
        });

        // Captured BEFORE `self.call` consumes `input` — never clone `input`
        // itself (some tools, e.g. `create_file`/`edit_file`, carry whole
        // file bodies in it).
        let selector = self.selector_key(&input);
        // Captured BEFORE `self.call` consumes `input`, for the same reason as
        // `selector` above. Only UNPINNED writes are ambiguous: a pinned call
        // already named its target, in the call itself.
        let annotate_root = ctx.workspace_override.is_none()
            && self.is_write(&input)
            && !WRITE_ROOT_ANNOTATION_EXEMPT.contains(&self.name());
        // Captured BEFORE `self.call` consumes `input`, for the same reason as
        // the two above — and note this clones ONE string, never `input`, which
        // for `create_file`/`edit_file` carries a whole file body.
        //
        // Not gated on `workspace_override`, unlike `annotate_root`: that gate
        // exists because a pinned call already named the checkout it meant,
        // which says nothing about which file it wrote. The path is a fact
        // about the call either way.
        let write_path: Option<String> =
            if self.is_write(&input) && !WRITE_ROOT_ANNOTATION_EXEMPT.contains(&self.name()) {
                input
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            };
        // NOT `?`. Every render site for `param_corrections` below sits under this
        // call, so a bare `?` returned past all of them and the advisory was lost on
        // exactly the calls a caller reads hardest — the ones that failed. See
        // `attach_param_corrections_to_error` for the wire-confirmed reproduction, the
        // shape chosen, and why the fatal-error branch is deliberately untouched.
        let mut val = match self.call(input, ctx).await {
            Ok(v) => v,
            Err(e) => {
                return Err(attach_param_corrections_to_error(
                    e,
                    param_corrections.as_ref(),
                ))
            }
        };

        // Field-aware project-root stripping. Runs HERE, on the typed Value,
        // and therefore BEFORE `exceeds_inline_limit`, the `@tool_*` buffer
        // payload, and `format_compact` — the ordering is load-bearing: a
        // summary built from an unstripped Value leaks absolute paths through
        // the serialized envelope, which is how 85% of the pre-fix leaks
        // escaped. The root resolves from the same `ctx` the tool body used,
        // so a `workspace=` pin cannot be mismatched here the way it could
        // when `post_process` had to be handed the pin separately
        // (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md).
        let project_root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        let root_prefix = project_root
            .as_ref()
            .map(|p| format!("{}/", crate::util::fs::to_forward_slash(p)))
            .unwrap_or_default();
        let workspace_notice = worktree_read_notice(ctx, project_root.as_deref()).await;
        crate::tools::core::path_strip::strip_paths_in_value(&mut val, &root_prefix);
        // AFTER stripping, deliberately: the annotation's whole job is to name
        // the checkout absolutely, and the stripper rewrites project-rooted
        // paths into relative ones.
        if annotate_root {
            if let Some(root) = project_root.as_deref() {
                annotate_write_root(&mut val, root);
            }
        }
        // After stripping for the same reason as the root annotation: this names
        // the target the caller asked for, and the stripper rewrites
        // project-rooted paths into relative ones.
        if let Some(path) = write_path.as_deref() {
            annotate_write_path(&mut val, path);
        }
        let val = val;
        let form = self.output_form();
        let json = serde_json::to_string(&val).unwrap_or_else(|_| val.to_string());

        fn inject_notice(val: &mut Value, notice: &str) {
            if let Some(obj) = val.as_object_mut() {
                // `run_command`'s answer IS the primary content the caller reads —
                // a sibling `_workspace_notice` field next to a plausible `stdout`
                // reads as metadata, and the plausible answer wins attention (see
                // docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md).
                // Prepend into `stdout` too, when present, so the warning sits in
                // the channel that is actually read. No other tool's response
                // shape carries a top-level `stdout` string.
                if let Some(stdout) = obj.get("stdout").and_then(|v| v.as_str()) {
                    let prefixed = format!("⚠ {notice}\n\n{stdout}");
                    obj.insert("stdout".to_string(), Value::String(prefixed));
                }
                obj.insert(
                    "_workspace_notice".to_string(),
                    Value::String(notice.to_string()),
                );
            }
        }

        // The post-phase fan-out. Ordering, corpus exclusivity and the
        // anonymous-tier idle re-arm all live in the coordinator now; what
        // stays here is assembling the primary block, which the coordinator
        // must not know about because `inject_hint` mutates it in place.
        //
        // One lock for the whole fan-out, where this used to take two — the
        // guide path and the rule path each locked `guide_hints_emitted`
        // separately. Fewer acquisitions, identical bytes.
        let (guide_hint, injected) = {
            let mut emitted = ctx.guide_hints_emitted.lock();
            let post = crate::engines::coordinator::PostCtx {
                selector: selector.as_deref(),
                value: &val,
                content_topic: self.relevant_guide_topic(&val),
                // Either the default-path buffering will kick in (large JSON),
                // or the tool itself pre-buffered (e.g. `run_command` storing a
                // `@cmd_*` ref and returning a small envelope with
                // `output_id`). Both signal the agent should learn the
                // progressive-disclosure pattern.
                overflowing: exceeds_inline_limit(&json)
                    || val
                        .as_object()
                        .and_then(|o| o.get("output_id"))
                        .and_then(|v| v.as_str())
                        .is_some(),
            };
            let e = crate::engines::coordinator::run_post(&post, &mut emitted);
            (e.hint, e.blocks)
        };

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
            if let Some((topic, shape)) = &guide_hint {
                inject_hint(&mut buffered, topic, *shape);
            }
            if let Some(notice) = &workspace_notice {
                inject_notice(&mut buffered, notice);
            }
            if let Some(c) = &param_corrections {
                // Same unconditional nesting as the small-output path below
                // (see the comment there): the advisory lives at
                // `corrections.param_aliases` in every case, never bare at
                // `corrections`. This assignment is still a wholesale
                // overwrite rather than a merge with any `corrections` the
                // tool itself wrote into the buffered envelope — that gap is
                // the pre-existing, separately-filed buffered-envelope
                // finding, out of scope for this change.
                buffered["corrections"] = serde_json::json!({ "param_aliases": c });
            }
            Content::text(
                serde_json::to_string_pretty(&buffered)
                    .unwrap_or_else(|_| format!("{{\"output_id\":\"{ref_id}\"}}")),
            )
        } else {
            // Small output path.
            let mut val = val;
            if let Some((topic, shape)) = &guide_hint {
                inject_hint(&mut val, topic, *shape);
            }
            if let Some(notice) = &workspace_notice {
                inject_notice(&mut val, notice);
            }
            if let Some(c) = &param_corrections {
                if let Some(obj) = val.as_object_mut() {
                    // Merge rather than overwrite, via the shared
                    // `merge_param_corrections` — the SAME function the error path
                    // calls, so the advisory's address is a property of the
                    // mechanism and not of which branch you landed on. Its doc
                    // comment holds the three arms' rationale: why the advisory is
                    // nested under a dedicated key rather than merged key-by-key,
                    // and why that nesting is unconditional. The `None` arm is the
                    // live one on every ordinary alias-repair call, e.g.
                    // `read_file(file_path=…)`; the other two have no in-tree
                    // production caller today.
                    //
                    // The collider worth naming HERE, because it is the one a grep
                    // mis-answers: the ADR deliberately keeps each tool's own alias
                    // fallback as "a redundant second layer" (`src/fs/mod.rs`'s
                    // `get_path_param`/`require_path_param`,
                    // `src/tools/core/params.rs`'s `require_str_param_or_hint`),
                    // which share the alias SET rather than this trait method: they
                    // read `PATH_PARAM_ALIASES` (`src/fs/mod.rs`) directly, and
                    // `Tool::param_aliases()`'s only caller is `call_content` — so
                    // do NOT re-verify by grepping that method's callers, which
                    // would find only this file and wrongly suggest the second
                    // layer is unreachable. What makes it a collider is the shared
                    // WORD: a tool surfacing its own fallback repair would name it
                    // `param_aliases` because that is what the concept is called
                    // here, and the insert would silently destroy it.
                    merge_param_corrections(obj, c);
                }
            }
            if form == OutputForm::Text {
                if let Some(text) = self.format_compact(&val) {
                    // The notice must be re-attached HERE, not left to `inject_notice`.
                    // That function writes `_workspace_notice` into the `Value`, and
                    // `format_compact` builds its text by selecting the fields the tool
                    // cares about — a field the framework added after the tool returned is
                    // one no renderer knows to read. So the notice was injected and then
                    // discarded, on every tool with a compact form: `symbols`, `grep`,
                    // `tree`, `read_file`, `references`, `semantic_search`, … 18 of them,
                    // which is precisely the read surface it exists to caveat. It survived
                    // only on the pretty-JSON branch below.
                    // docs/issues/2026-09-02-the-worktree-notice-is-injected-then-discarded-by-every-compact-renderer.md
                    //
                    // PREFIXED, matching `inject_notice`'s own choice for `run_command`
                    // ("so the warning sits in the channel that is actually read"): the
                    // notice changes how the content below should be read, so it has to
                    // arrive before it, not after a result the reader has already believed.
                    //
                    // `param_notice` (the hint STRING) is prefixed here too, for the same
                    // reason: `format_compact` renders only the fields the tool selected,
                    // so `param_corrections` (the OBJECT) has nowhere to land on this path
                    // — a text renderer cannot carry a JSON object, which is why both
                    // `param_notice` and `param_corrections` were built above.
                    Content::text({
                        let mut prefix = String::new();
                        if let Some(notice) = &workspace_notice {
                            prefix.push_str(&format!("⚠ {notice}\n\n"));
                        }
                        if let Some(n) = &param_notice {
                            prefix.push_str(&format!("⚠ {n}\n\n"));
                        }
                        format!("{prefix}{text}")
                    })
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

        // V2 hard-injection: append the guide content blocks computed above —
        // either the single whole-topic block (non-declaring topic) or the
        // per-section slices / preamble fallback (declaring topic).
        let mut blocks = vec![primary];
        blocks.extend(injected);
        Ok(blocks)
    }

    /// Non-canonical parameter names this tool accepts, as `(received, canonical)`.
    ///
    /// Advertise ONLY the canonical name in `input_schema`. `call_content` rewrites
    /// these before anything reads the input and announces the correction on the
    /// response — so a tool declaring an alias here must NOT also declare it as a
    /// property. Task 7 of the parameter-alias-collapse plan owes an
    /// `every_declared_alias_is_absent_from_the_schema` guard in `src/server.rs`
    /// to enforce that; as of this task (Task 3) it does not exist yet — see
    /// `docs/issues/2026-09-02-a-doc-comment-announcing-unbuilt-work-outlives-the-work.md`
    /// for why this note names the gap in the future tense rather than the present.
    ///
    /// Defaults to empty: a tool opts in.
    fn param_aliases(&self) -> crate::tools::param_alias::AliasMap {
        &[]
    }

    /// A cheap projection of this call's shape, taken BEFORE `call()` consumes
    /// `input`. **Every tool opts in by default**: the default computes
    /// `action_selector_key(self.name(), input)`, so a tool is reachable by
    /// `triggered` operator rules and by section-grain guide matching without
    /// writing any code for it.
    ///
    /// Overriding to `None` opts a tool OUT and owes a stated reason. `None` makes
    /// `Shape::matches` report "cannot match", which disables operator-rule routing
    /// AND section-grain matching for every call to that tool — a silent-absence
    /// failure, not the fail-safe-toward-delivery direction this feature requires.
    /// Nothing overrides it today, and
    /// `every_registered_tool_supplies_a_selector_key` (`src/server.rs`) fails if
    /// anything starts to.
    ///
    /// **Inverted 2026-09-01, and the previous default is why.** It returned `None`
    /// — documented as "the tool opts out at zero cost" — which left routing
    /// structurally unreachable for 17 of the 21 registered tools while the suite
    /// stayed green, because the only caller exercising the path was a test stub
    /// named after a real tool. Opt-out-by-default put the cost on the invisible
    /// side: a tool that never opted in produced no error, no empty result and no
    /// log line, just a rule that never fired. See
    /// docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md.
    ///
    /// Deliberately still not a clone of `input`: `create_file` and `edit_file`
    /// inputs carry whole file bodies, and a clone would be paid on 100% of calls.
    /// `action_selector_key` reads one field and allocates one short string, which
    /// is the cost this default is affordable at.
    fn selector_key(&self, input: &Value) -> Option<String> {
        action_selector_key(self.name(), input)
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
    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
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

    /// The shape that matters most in practice. A `doc(get)` envelope keeps
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
