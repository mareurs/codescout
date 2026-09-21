pub mod db;

use crate::agent::Agent;
use anyhow::Result;
use rmcp::model::Content;
use serde_json::Value;
use std::time::Instant;

pub struct UsageRecorder {
    agent: Agent,
    debug: bool,
    /// This MCP server process's own id.
    session_id: String,
    /// The Claude Code session id, resolved by the server and passed in.
    ///
    /// This used to be read here from `.codescout/cc_session_id` on every write.
    /// That file is per-PROJECT, so two concurrent Claude Code sessions both
    /// recorded under whichever id was written last, and every per-session
    /// figure silently merged them. The server already resolves this correctly
    /// (`CLAUDE_CODE_SESSION_ID` first, which is per-process); taking it from
    /// there gives the value one resolution site instead of two that drifted.
    /// docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md
    ///
    /// **This field holds two different kinds of value and the column named after it
    /// cannot say which.** The server passes `serving_session`, which is the companion's
    /// composed `<session>/<agent>` principal token when one was stamped and a bare
    /// session id otherwise — measured 2026-09-20, 1,873 of 68,988 rows in this
    /// checkout's db carry the composed form. That is why `agent_id` below is its own
    /// field rather than something a consumer recovers by splitting this one: doing so
    /// needs a `/` sniff nothing documents, and a consumer grouping by this column
    /// files a subagent apart from its own parent's session without noticing.
    cc_session_id: String,
    /// The `agent_id` half of the principal, split from the companion's stamp by the
    /// server and passed in already-resolved — one resolution site, for the reason the
    /// field above records.
    ///
    /// `None` covers a parent (whose `PreToolUse` payload carries no `agent_id` at all)
    /// and every client without the companion installed, and the two are not
    /// distinguishable here. The ADR accepts that plugin dependency as a product
    /// decision, so this degrades to the previous behaviour rather than erroring.
    /// docs/adrs/2026-09-14-a-subagent-is-a-principal.md
    agent_id: Option<String>,
}

impl UsageRecorder {
    pub fn new(
        agent: Agent,
        debug: bool,
        session_id: String,
        cc_session_id: String,
        agent_id: Option<String>,
    ) -> Self {
        Self {
            agent,
            debug,
            session_id,
            cc_session_id,
            agent_id,
        }
    }

    /// Record a tool call's telemetry against the project named by
    /// `workspace_override`, falling back to the session default when `None`.
    /// The pin MUST match the one the tool body itself resolved, or a pinned
    /// call's stats land in the wrong project's `usage.db` (see
    /// `docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`,
    /// finding 4).
    pub async fn record_content<F, Fut>(
        &self,
        tool_name: &str,
        input: &Value,
        workspace_override: Option<&std::path::Path>,
        f: F,
    ) -> Result<Vec<Content>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Content>>>,
    {
        // Captured HERE, beside the monotonic clock, because this is the only point
        // that observes the call's start. `write_content` stamps `called_at` after
        // `f().await` AND after its own `open_db`, so the start is not recoverable
        // downstream as `called_at - latency_ms` — see the `started_at` migration.
        let started_at = db::now_timestamp();
        let start = Instant::now();
        let result = f().await;
        let latency_ms = start.elapsed().as_millis() as i64;
        // Best-effort — never let recording fail the tool call
        let _ = self
            .write_content(
                tool_name,
                &started_at,
                latency_ms,
                input,
                workspace_override,
                &result,
            )
            .await;
        result
    }

    async fn write_content(
        &self,
        tool_name: &str,
        started_at: &str,
        latency_ms: i64,
        input: &Value,
        workspace_override: Option<&std::path::Path>,
        result: &Result<Vec<Content>>,
    ) -> Result<()> {
        let (project_root, head_sha) = self
            .agent
            .with_project_at(workspace_override, |p| {
                Ok((p.root.clone(), p.head_sha.clone()))
            })
            .await?;
        // A worktree is deleted at the end of its life, taking its OWN
        // `.codescout/usage.db` with it — durable telemetry cannot live there.
        // Write it into the main checkout's db instead, tagged with the
        // worktree's own root in the `project_root` column below (unchanged),
        // so it stays distinguishable and queryable after the worktree is gone.
        // docs/issues/archive/2026-08-20-worktree-removal-deletes-its-usage-telemetry.md
        let db_root = crate::util::path_security::worktree_main_root(&project_root)
            .unwrap_or_else(|| project_root.clone());
        let conn = db::open_db(&db_root)?;
        let (outcome, overflowed, error_msg) = classify_content_result(result);

        // Friction fields (Phase 1 of the legibility probe).
        let is_friction = overflowed || outcome != "success";
        let friction_target = if is_friction {
            extract_friction_target(input)
        } else {
            None
        };
        let overflow_tokens = if overflowed {
            extract_overflow_tokens(result)
        } else {
            None
        };
        let err_family = error_msg
            .as_deref()
            .and_then(|m| db::normalize_err_family(tool_name, m));
        let project_root_str = project_root.to_string_lossy().to_string();

        // Buffer linkage. Both are extracted unconditionally, NOT behind `self.debug`
        // like `input_json`/`output_json` below — that is the whole reason they are
        // columns rather than something a query derives. With debug off, neither the
        // emitted handle nor the referenced ones survive anywhere else, and the join
        // loses both of its sides.
        let emitted_output_id = if overflowed {
            extract_emitted_output_id(result)
        } else {
            None
        };
        let read_output_ids = extract_read_output_ids(input);

        let input_json = if self.debug {
            serde_json::to_string(input).ok()
        } else {
            None
        };

        let output_json = if self.debug {
            match result {
                Ok(blocks) => serde_json::to_string(blocks).ok(),
                Err(e) => Some(serde_json::json!({"error": e.to_string()}).to_string()),
            }
        } else {
            None
        };

        // Resolved once by the server, not re-derived here — see the field doc.
        let cc_session_id = Some(self.cc_session_id.as_str()).filter(|s| !s.is_empty());

        db::write_record(
            &conn,
            tool_name,
            latency_ms,
            outcome,
            overflowed,
            error_msg.as_deref(),
            // The sha AND its dirty bit, as one value. Passing the bare sha env var here
            // is what BL-24 was: a lone sha is not an identity, and a `&str` routes
            // through `From<&str>`, which assumes the tree was clean. Pinned by
            // `db::tests::the_recorder_never_assumes_a_clean_build`, which scans this
            // file — so do not name that env var here, even in a comment.
            db::BuildProvenance::current(),
            head_sha.as_deref(),
            &self.session_id,
            input_json.as_deref(),
            output_json.as_deref(),
            cc_session_id,
            friction_target.as_deref(),
            overflow_tokens,
            err_family,
            Some(project_root_str.as_str()),
            Some(started_at),
            // Resolved once by the server from the companion's principal stamp, exactly
            // as `cc_session_id` above and for the reason its doc names — a second
            // resolution site is the defect that field already paid for.
            self.agent_id.as_deref(),
            db::BufferLinkage {
                emitted: emitted_output_id.as_deref(),
                reads: read_output_ids.as_deref(),
            },
        )?;
        Ok(())
    }
}

/// Classify a tool call's outcome for `usage.db`.
///
/// **The `Err` arm has to downcast, because this runs BEFORE the router that
/// would otherwise tell it.** `route_tool_error` (`src/server.rs`) is what turns
/// a [`RecoverableError`] into an `isError: false` response, and it runs after
/// [`UsageRecorder::record_content`] has already written the row. Classifying
/// every `Err` as `"error"` therefore made `recoverable_error` unreachable in
/// production: the column is documented and queried as a three-value taxonomy
/// and held two, so a guard firing correctly and a hard failure were the same
/// value, and two queries filtering on the third returned nothing without
/// saying so.
///
/// The `Ok` arm below can still emit `"recoverable_error"`, but only for a tool
/// returning `Ok(content)` whose JSON body carries a top-level `error` key. No
/// live tool does that — codescout's recoverable errors travel as `Err`, per
/// `get_guide("error-handling")` — which is why that arm's existence read as
/// coverage while the value never appeared in 57k rows.
///
/// [`RecoverableError`]: crate::tools::RecoverableError
///
/// docs/issues/archive/2026-09-02-recoverable-error-outcome-is-unreachable-in-production.md
fn classify_content_result(result: &Result<Vec<Content>>) -> (&'static str, bool, Option<String>) {
    match result {
        Err(e) => {
            let outcome = if e.downcast_ref::<crate::tools::RecoverableError>().is_some() {
                "recoverable_error"
            } else {
                "error"
            };
            (outcome, false, Some(e.to_string()))
        }
        Ok(blocks) => {
            // Parse the text of the first content block as JSON and inspect it for the
            // `error` / `output_id` sentinel keys.
            //
            // **This depends on an invariant of the RENDERER, not of this function.**
            // `Tool::call_content`'s buffered arm emits its `{output_id, summary, hint,
            // buffered_bytes}` envelope as JSON with no `output_form` branch, so
            // `output_id` is findable here for every tool. A buffered arm that ever
            // rendered compactly would make `overflowed` silently `false` for every
            // `OutputForm::Text` tool — `grep`, `symbols`, `references`, `tree` — and
            // nothing in this module would fail, because every other test here builds
            // its blocks by hand. Pinned by
            // `content_tests::the_renderer_and_the_classifier_agree_about_overflow`.
            let text = blocks
                .first()
                .and_then(|c| c.as_text())
                .map(|t| t.text.as_str())
                .unwrap_or("");
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                if let Some(msg) = v.get("error").and_then(Value::as_str) {
                    return ("recoverable_error", false, Some(msg.to_string()));
                }
                if v.get("output_id").is_some() {
                    return ("success", true, None);
                }
            }
            ("success", false, None)
        }
    }
}

/// Token estimate of a buffered (overflowed) result: `buffered_bytes / 4`.
fn extract_overflow_tokens(result: &Result<Vec<Content>>) -> Option<i64> {
    let blocks = result.as_ref().ok()?;
    let text = blocks
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())?;
    let v: Value = serde_json::from_str(text).ok()?;
    let bytes = v.get("buffered_bytes").and_then(Value::as_i64)?;
    Some(bytes / 4)
}

/// The symbol/path a call addressed, for friction attribution. Priority order:
/// the most specific address first (name_path/symbol), then name, then path/query/pattern.
///
/// `file_path` and `rel_path` are included because they are documented **aliases** of
/// `path` on the tools that accept them, not separate concepts — so they sit immediately
/// after it, and the canonical spelling still wins if a caller sends both. Omitting them
/// cost 57 error rows their target on this project alone (measured 2026-08-20; all 51
/// `file_path` rows carried no `path` at all), each one a file
/// `legibility::recorder_lane` could not join to a candidate.
///
/// Deliberately absent: `command`. It is the largest target-less population by volume
/// (438 rows) and is still not a target — see
/// `extract_friction_target_ignores_shell_commands` for the reasoning, which is a
/// decision rather than an oversight.
fn extract_friction_target(input: &Value) -> Option<String> {
    const KEYS: [&str; 8] = [
        "name_path",
        "symbol",
        "name",
        "query",
        "path",
        "file_path",
        "rel_path",
        "pattern",
    ];
    for k in KEYS {
        if let Some(s) = input.get(k).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// The `@ref` handle an overflowed call handed out, from its own response envelope.
///
/// Free to extract: `extract_overflow_tokens` above already deserializes exactly this
/// JSON object to read `buffered_bytes`, and discarded the sibling `output_id` — its own
/// fixture is `{"output_id":"@tool_x","buffered_bytes":10000}`.
///
/// This is the half that is **not** recoverable at query time. `output_json` is
/// debug-gated (see `write_content`), so on a recorder running with debug off there is no
/// other record of which handle a call emitted, and the join has nothing to anchor on.
fn extract_emitted_output_id(result: &Result<Vec<Content>>) -> Option<String> {
    let blocks = result.as_ref().ok()?;
    let text = blocks
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())?;
    let v: Value = serde_json::from_str(text).ok()?;
    let id = v.get("output_id").and_then(Value::as_str)?;
    (!id.is_empty()).then(|| id.to_string())
}

/// Buffer handles a call **named in its arguments**, deduplicated, as a JSON array.
///
/// **This records what a call MENTIONED, never what the buffer resolver RESOLVED**, and
/// the gap is not a rounding error. Measured 2026-09-21 over this project's own
/// `usage.db`: 2,068 rows contain an `@` with no valid handle at all, and a handle can be
/// quoted in prose that is never served — a subagent brief naming `@tool_x` verbatim
/// records a reference the server never resolved. Closing that would need plumbing from
/// `OutputBuffer` to this recorder, which is deliberately not built.
///
/// So a join over this column answers *"was this handle named later?"* and **not** *"was
/// this result retrieved?"*. Promoting the first into the second is precisely the defect
/// these columns exist to make measurable rather than to repeat —
/// `docs/issues/2026-09-20-predicate-probe-overstates-retrieval-and-redundancy.md`.
///
/// A JSON array rather than a scalar because multi-handle calls are real: over 5,355
/// handle-bearing calls the distribution of distinct handles per call was
/// `{1: 5285, 2: 55, 3: 11, 4: 2, 5: 1, 9: 1}` (2026-09-21), so keeping only the first
/// would drop 1.31% of calls — and would drop them by biasing retrieval *downward*, the
/// same direction as the defect being fixed. Query with `json_each`.
fn extract_read_output_ids(input: &Value) -> Option<String> {
    let blob = serde_json::to_string(input).ok()?;
    let handles = scan_buffer_handles(&blob);
    if handles.is_empty() {
        return None;
    }
    serde_json::to_string(&handles).ok()
}

/// Every distinct buffer handle in `hay`, in first-seen order.
///
/// Hand-rolled rather than a `regex`: this runs on the recording path of **every** tool
/// call, and the scan is a prefix test plus an alphanumeric run.
///
/// The `.err` suffix normalizes away for free, and that is load-bearing rather than
/// incidental — per `get_guide("progressive-disclosure")`, `.err` selects the *stderr
/// stream of the same entry*, so `@cmd_abc.err` and `@cmd_abc` MUST join as one handle.
/// Consuming only `[A-Za-z0-9]` after the prefix stops at the `.`, which yields exactly
/// that. `scan_normalizes_the_err_suffix` pins it, because a future change to the
/// accepted charset would silently split the two apart.
fn scan_buffer_handles(hay: &str) -> Vec<String> {
    const REF_PREFIXES: [&str; 5] = ["@cmd_", "@tool_", "@file_", "@ack_", "@bg_"];
    let bytes = hay.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        // `bytes[i]` is ASCII `@`, so `i` is a char boundary and this slice is safe.
        let rest = &hay[i..];
        let Some(prefix) = REF_PREFIXES.iter().find(|p| rest.starts_with(**p)) else {
            i += 1;
            continue;
        };
        let body_start = i + prefix.len();
        let mut end = body_start;
        while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
            end += 1;
        }
        if end > body_start {
            let handle = &hay[i..end];
            if !out.iter().any(|h| h == handle) {
                out.push(handle.to_string());
            }
        }
        i = end.max(i + 1);
    }
    out
}

#[cfg(test)]
mod content_tests {
    use super::*;
    use rmcp::model::Content;

    async fn test_ctx() -> crate::tools::ToolContext {
        crate::tools::ToolContext {
            agent: crate::agent::Agent::new(None).await.unwrap(),
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
            workspace_override: None,
        }
    }

    /// The renderer and the classifier agree about overflow — driven through the
    /// PRODUCTION render path rather than a hand-built envelope.
    ///
    /// **Every other test in this module builds its `Vec<Content>` by hand**, so each
    /// asserts about this module's own idea of what an overflow envelope looks like.
    /// `classify_detects_overflow_by_output_id_not_legacy_key` is the closest, and it
    /// would stay green if `call_content` stopped emitting `output_id` in the first
    /// block — `classify_content_result` would then report `overflowed = false` for
    /// every buffered call, `is_friction` would go quiet, and `usage.db` would fill
    /// with rows claiming an inline result that was actually buffered. That is
    /// `CLAUDE.md` § *Testing Discipline*: a second level asserting about its own
    /// re-implementation reads as coverage until you break the thing that ships.
    ///
    /// **The invariant pinned here:** the buffered arm of `Tool::call_content` emits
    /// JSON carrying `output_id` as the FIRST content block, for every `OutputForm`.
    /// That arm (`src/tools/core/types.rs`) has **no `output_form` branch** — on the
    /// buffered path `format_compact` fills the envelope's `summary` field and is not
    /// the wire form. Classification is correct today only because of that, and
    /// nothing else stated it. Prose nearby generalises the other way
    /// (`OutputForm`'s own doc comment, and `cap_probe.rs`'s "Grep declares
    /// `OutputForm::Text`, so its primary content block is never JSON") — true of the
    /// SMALL path, not of this one.
    ///
    /// Sibling coverage, deliberately not duplicated: `core::tests::
    /// a_compact_rendered_read_still_carries_the_worktree_notice` drives the same
    /// `Text` + `format_compact` fixture shape through the **small** path. This is its
    /// buffered twin.
    #[tokio::test]
    async fn the_renderer_and_the_classifier_agree_about_overflow() {
        struct BulkTool;

        #[async_trait::async_trait]
        impl crate::tools::Tool for BulkTool {
            fn name(&self) -> &str {
                "bulk"
            }
            fn description(&self) -> &str {
                "test"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            async fn call(
                &self,
                _input: serde_json::Value,
                _ctx: &crate::tools::ToolContext,
            ) -> anyhow::Result<serde_json::Value> {
                // Load-bearing: this must exceed TOOL_OUTPUT_BUFFER_THRESHOLD
                // (10_000 bytes) so the BUFFERED arm runs. Shrink it and the test
                // silently starts exercising the small-output path, where there is
                // no `output_id` at all and the assertions below stop discriminating.
                Ok(serde_json::json!({ "rows": vec!["x".repeat(200); 100] }))
            }
            // Load-bearing pair: `Text` + a non-JSON `format_compact` is the
            // combination `grep`/`symbols`/`references`/`tree` ship, and the one that
            // would render non-JSON here if the buffered arm ever grew an
            // `output_form` branch. With the default `Json` form this test would pass
            // for the wrong reason and catch nothing.
            fn output_form(&self) -> crate::tools::OutputForm {
                crate::tools::OutputForm::Text
            }
            fn format_compact(&self, _result: &serde_json::Value) -> Option<String> {
                Some("compact text, deliberately not JSON".to_string())
            }
        }

        // Brings the trait's provided `call_content` into scope; the impl above is
        // fully qualified, which is not enough to call through it.
        use crate::tools::Tool as _;

        let ctx = test_ctx().await;
        let rendered = BulkTool.call_content(serde_json::json!({}), &ctx).await;

        let blocks = rendered.as_ref().expect("fixture must not error");
        let first = blocks
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");

        // Fixture check first: if the payload stopped overflowing, everything below
        // would pass vacuously against the small path.
        assert!(
            first.contains("output_id"),
            "fixture check: the payload must have overflowed and the envelope must be \
             the FIRST block, or the classifier assertion below proves nothing; got: {first}"
        );
        assert!(
            serde_json::from_str::<serde_json::Value>(first).is_ok(),
            "the buffered arm must emit JSON even for an OutputForm::Text tool — this is \
             the invariant `classify_content_result` silently depends on; got: {first}"
        );

        let (outcome, overflowed, msg) = classify_content_result(&rendered);
        assert_eq!(outcome, "success");
        assert!(
            overflowed,
            "a buffered result must classify as overflowed, or usage.db records it as an \
             inline one; first block was: {first}"
        );
        assert!(msg.is_none());
    }

    #[test]
    fn classify_content_error_result() {
        let r: anyhow::Result<Vec<Content>> = Err(anyhow::anyhow!("boom"));
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "error");
        assert!(!overflowed);
        assert_eq!(msg.as_deref(), Some("boom"));
    }

    #[test]
    fn classify_content_recoverable_error() {
        let text = serde_json::json!({"error": "path not found"}).to_string();
        let r: anyhow::Result<Vec<Content>> = Ok(vec![Content::text(text)]);
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "recoverable_error");
        assert!(!overflowed);
        assert_eq!(msg.as_deref(), Some("path not found"));
    }

    #[test]
    fn classify_detects_overflow_by_output_id_not_legacy_key() {
        // real overflow envelope marker
        let real = Ok(vec![Content::text(
            r#"{"output_id":"@tool_abc","summary":"...","buffered_bytes":12000}"#.to_string(),
        )]);
        let (_outcome, overflowed, _) = classify_content_result(&real);
        assert!(overflowed, "output_id envelope must set overflowed=true");

        // legacy key must NOT trigger (guards the exact wrong-key regression)
        let legacy = Ok(vec![Content::text(r#"{"overflow":true}"#.to_string())]);
        let (_o2, overflowed_legacy, _) = classify_content_result(&legacy);
        assert!(
            !overflowed_legacy,
            "legacy 'overflow' key must not be treated as overflow"
        );

        // normal result
        let normal = Ok(vec![Content::text(r#"{"result":"ok"}"#.to_string())]);
        let (_o3, overflowed_normal, _) = classify_content_result(&normal);
        assert!(!overflowed_normal);
    }

    #[test]
    fn classify_content_clean_success() {
        let r: anyhow::Result<Vec<Content>> = Ok(vec![Content::text("plain text output")]);
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "success");
        assert!(!overflowed);
        assert!(msg.is_none());
    }

    #[test]
    fn classify_content_empty_blocks() {
        let r: anyhow::Result<Vec<Content>> = Ok(vec![]);
        let (outcome, overflowed, msg) = classify_content_result(&r);
        assert_eq!(outcome, "success");
        assert!(!overflowed);
        assert!(msg.is_none());
    }

    #[test]
    fn extract_overflow_tokens_reads_buffered_bytes_over_four() {
        let env = Ok(vec![Content::text(
            r#"{"output_id":"@tool_x","buffered_bytes":10000}"#.to_string(),
        )]);
        assert_eq!(extract_overflow_tokens(&env), Some(2500));

        let no_bytes = Ok(vec![Content::text(
            r#"{"output_id":"@tool_x"}"#.to_string(),
        )]);
        assert_eq!(extract_overflow_tokens(&no_bytes), None);

        let err: Result<Vec<Content>> = Err(anyhow::anyhow!("boom"));
        assert_eq!(extract_overflow_tokens(&err), None);
    }

    #[test]
    fn extract_friction_target_coalesces_input_keys() {
        use serde_json::json;
        assert_eq!(
            extract_friction_target(&json!({"name_path": "A/b", "path": "src/x.rs"})),
            Some("A/b".to_string()),
            "name_path wins over path"
        );
        assert_eq!(
            extract_friction_target(&json!({"symbol": "Foo/bar"})),
            Some("Foo/bar".to_string())
        );
        assert_eq!(
            extract_friction_target(&json!({"path": "src/lib.rs"})),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(extract_friction_target(&json!({"unrelated": 1})), None);
    }

    /// `file_path` and `rel_path` are documented ALIASES of `path` on the tools that
    /// accept them (`read_file`, `edit_file`, `grep`, `create_file`, `read_markdown`,
    /// and `artifact`'s `rel_path`). Extracting only `path` means a call
    /// that spelled it the other way records no target at all.
    ///
    /// Measured 2026-08-20 on this project's own `usage.db`: **51 error rows carried
    /// `file_path` and NONE of them also carried `path`** — `read_file` 31, `edit_file` 10,
    /// `read_markdown` 4, `edit_markdown` 4, `edit_code` 2 — plus 6 `artifact` rows
    /// carrying `rel_path`. Every one is a file target that `legibility::recorder_lane`
    /// should have been able to join to a `rel_file` candidate and could not.
    #[test]
    fn extract_friction_target_reads_the_documented_path_aliases() {
        use serde_json::json;
        assert_eq!(
            extract_friction_target(&json!({"file_path": "src/x.rs"})),
            Some("src/x.rs".to_string()),
            "file_path is a documented alias of path and must yield a target"
        );
        assert_eq!(
            extract_friction_target(&json!({"rel_path": "docs/trackers/foo.md"})),
            Some("docs/trackers/foo.md".to_string()),
            "rel_path is artifact's spelling of the same concept"
        );
        // An alias must not outrank the more specific keys, or a symbol-addressed call
        // that also names a file would be attributed to the file.
        assert_eq!(
            extract_friction_target(&json!({"name_path": "A/b", "file_path": "src/x.rs"})),
            Some("A/b".to_string()),
            "name_path still wins over an alias"
        );
        // `path` and `file_path` are the same concept, so which one wins cannot matter for
        // correctness — but pin it so the order is a decision rather than an accident.
        assert_eq!(
            extract_friction_target(&json!({"path": "a.rs", "file_path": "b.rs"})),
            Some("a.rs".to_string()),
            "canonical `path` is preferred when a caller sends both spellings"
        );
    }

    /// `command` is deliberately NOT a friction target, and this test is the record of
    /// that decision rather than an oversight.
    ///
    /// `run_command` accounts for 438 of the 596 target-less error rows (2026-08-20), so
    /// adding `command` would close most of the gap by volume. It is still wrong:
    ///
    /// * The field is documented as *the symbol/path a call addressed*. A shell command is
    ///   neither, and the sole consumer — `legibility::score_and_rank` — looks friction up
    ///   by `name_path` then `rel_file`, so a command string is inert there: never matched,
    ///   never surfaced, pure storage.
    /// * A whole command varies by flags, so it groups badly; the executable name groups
    ///   well but discards what was addressed. Neither is *the target*.
    /// * `input_json` is populated on ~99% of rows, so the command is already recoverable
    ///   at query time. Storing a derived form buys nothing and makes one column mean two
    ///   things.
    ///
    /// If a consumer ever needs per-command grouping, give it its own column rather than
    /// widening this one's contract.
    #[test]
    fn extract_friction_target_ignores_shell_commands() {
        use serde_json::json;
        assert_eq!(
            extract_friction_target(&json!({"command": "cargo test --lib"})),
            None,
            "a shell command is not a symbol or a path — see this test's doc comment"
        );
        // ...but a run_command that DOES name a cwd path still yields nothing, because cwd
        // is the directory the command ran in, not the thing it addressed.
        assert_eq!(
            extract_friction_target(&json!({"command": "ls", "cwd": "src/"})),
            None,
            "cwd is where the call ran, not what it addressed"
        );
    }

    #[test]
    fn extract_emitted_output_id_reads_the_envelopes_handle() {
        let env = Ok(vec![Content::text(
            r#"{"output_id":"@tool_x","buffered_bytes":10000}"#.to_string(),
        )]);
        assert_eq!(
            extract_emitted_output_id(&env),
            Some("@tool_x".to_string()),
            "the handle sits beside buffered_bytes in the envelope extract_overflow_tokens \
             already parses"
        );
        let no_id = Ok(vec![Content::text(
            r#"{"buffered_bytes":10000}"#.to_string(),
        )]);
        assert_eq!(extract_emitted_output_id(&no_id), None);
        let err: Result<Vec<Content>> = Err(anyhow::anyhow!("boom"));
        assert_eq!(extract_emitted_output_id(&err), None);
    }

    #[test]
    fn extract_read_output_ids_collects_every_distinct_handle() {
        use serde_json::json;
        assert_eq!(
            extract_read_output_ids(&json!({"path": "@tool_abc123"})),
            Some(r#"["@tool_abc123"]"#.to_string())
        );
        assert_eq!(
            extract_read_output_ids(&json!({"path": "src/lib.rs"})),
            None,
            "a call naming no handle records NULL, not an empty array"
        );
        // Multi-handle is 1.31% of handle-bearing calls (measured 2026-09-21 over 5,355):
        // small, but dropping it would bias retrieval DOWNWARD — the same direction as the
        // defect these columns fix.
        let multi = extract_read_output_ids(
            &json!({"command": "diff @cmd_aaa111 @cmd_bbb222", "cwd": "@file_ccc333"}),
        )
        .unwrap();
        let parsed: Vec<String> = serde_json::from_str(&multi).unwrap();
        assert_eq!(parsed, vec!["@cmd_aaa111", "@cmd_bbb222", "@file_ccc333"]);
    }

    /// `.err` selects the stderr stream of the SAME buffer entry, so it must normalize to
    /// the same handle or a stderr read would never join to the result that produced it.
    ///
    /// This falls out of consuming only `[A-Za-z0-9]` after the prefix — which means a
    /// future widening of that charset (to accept `.`, say) would silently split the two
    /// apart with no other test noticing. That is what this test is for.
    #[test]
    fn scan_normalizes_the_err_suffix() {
        assert_eq!(
            scan_buffer_handles("grep ERROR @cmd_abc123.err"),
            vec!["@cmd_abc123".to_string()]
        );
        assert_eq!(
            scan_buffer_handles("@cmd_abc123 and @cmd_abc123.err"),
            vec!["@cmd_abc123".to_string()],
            "the bare handle and its .err form are one buffer and must dedup to one entry"
        );
    }

    /// The column's stated ceiling, pinned so nobody re-promotes *named* into *retrieved*.
    ///
    /// Extraction reads the call's ARGUMENTS, not the buffer resolver, so a handle quoted
    /// in prose is recorded exactly like one that was served. Subagent briefs in this repo
    /// quote handles verbatim, and 2,068 rows carry an `@` with no valid handle at all
    /// (measured 2026-09-21). The behaviour is correct for what the field claims; this test
    /// exists so the claim cannot quietly widen.
    #[test]
    fn a_handle_merely_quoted_in_prose_is_still_recorded_as_named() {
        use serde_json::json;
        let quoted = extract_read_output_ids(&json!({
            "body": "Tell the fork to read @tool_abc123 — it is NOT reading it here."
        }));
        assert_eq!(
            quoted,
            Some(r#"["@tool_abc123"]"#.to_string()),
            "this column records what a call NAMED; it does not and cannot claim the \
             buffer was resolved"
        );
        // A bare `@` with no valid prefix is not a handle, and must not become one.
        assert_eq!(
            extract_read_output_ids(&json!({"body": "email me@example.com or @nope_abc"})),
            None
        );
    }

    /// Regression for
    /// docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md:
    /// the recorder used to read `.codescout/cc_session_id` itself. That file is
    /// per-PROJECT, so with two Claude Code sessions open on one repo both wrote
    /// rows under whichever id the file held last and per-session figures merged
    /// them silently. The id the server resolved must win over the file.
    #[tokio::test]
    async fn record_content_uses_the_passed_cc_session_id_not_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        // A file holding a DIFFERENT session's id — the concurrent-session case.
        std::fs::write(
            dir.path().join(".codescout").join("cc_session_id"),
            "other-session-from-the-file",
        )
        .unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "mcp-session".to_string(),
            "my-cc-session".to_string(),
            None,
        );

        let _ = recorder
            .record_content(
                "symbols",
                &serde_json::json!({"query": "x"}),
                None,
                || async { Ok(vec![Content::text("ok")]) },
            )
            .await;

        let db = dir.path().join(".codescout").join("usage.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        let got: String = conn
            .query_row(
                "SELECT cc_session_id FROM tool_calls ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            got, "my-cc-session",
            "the server-resolved id must win; reading the shared per-project file \
             is what merged concurrent sessions into one"
        );
    }

    #[tokio::test]
    async fn record_content_stores_input_in_debug_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            true,
            "test-session".to_string(),
            "cc-test".to_string(),
            None,
        );
        let input = json!({"query": "test_symbol", "path": "src/lib.rs"});

        let _ = recorder
            .record_content("symbols", &input, None, || async {
                Ok(vec![Content::text("found it")])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, out, sid, cs): (Option<String>, Option<String>, String, String) = conn
            .query_row(
                "SELECT input_json, output_json, session_id, codescout_sha FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();

        assert!(
            inp.is_some(),
            "input_json should be populated in debug mode"
        );
        assert!(inp.unwrap().contains("test_symbol"));
        assert!(
            out.is_some(),
            "output_json should be populated in debug mode for all outcomes"
        );
        assert!(out.unwrap().contains("found it"));
        assert_eq!(sid, "test-session");
        assert!(!cs.is_empty(), "codescout_sha should be set");
    }

    #[tokio::test]
    async fn record_content_honors_workspace_override_pin() {
        // BUG (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md,
        // finding 4): write_content resolved the usage-db root via the plain,
        // unpinned with_project. call_tool_inner already computed the pin and
        // threaded it into check_tool_access/timeout_secs/the write guard — but
        // not into the recorder, so EVERY pinned call's telemetry silently
        // landed in the session-default project's usage.db.
        use serde_json::json;

        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        let canon_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Default (unpinned) project is B; pin THIS call to A.
        let agent = crate::agent::Agent::new(Some(dir_b.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent,
            false,
            "pin-session".to_string(),
            "cc-pin".to_string(),
            None,
        );
        let input = json!({"query": "x"});

        let _ = recorder
            .record_content("symbols", &input, Some(&canon_a), || async {
                Ok(vec![Content::text("ok")])
            })
            .await;

        let rows_in = |root: &std::path::Path| -> i64 {
            let conn = crate::usage::db::open_db(root).unwrap();
            conn.query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(
            rows_in(&canon_a),
            1,
            "the pinned call's telemetry must land in workspace A's usage.db"
        );
        assert_eq!(
            rows_in(dir_b.path()),
            0,
            "it must NOT land in the session-default workspace B's usage.db"
        );
    }

    #[tokio::test]
    /// Regression for
    /// docs/issues/archive/2026-08-20-worktree-removal-deletes-its-usage-telemetry.md: a call
    /// pinned into a linked worktree used to write its telemetry to `<worktree>/.codescout/
    /// usage.db`, which is deleted along with the worktree at the end of its life. It must
    /// land in the MAIN checkout's durable db instead, still tagged with the worktree's own
    /// root via `project_root` so it stays distinguishable.
    async fn record_content_pinned_into_a_worktree_writes_to_the_main_checkouts_db() {
        use serde_json::json;

        let main = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(main.path().join(".codescout")).unwrap();
        let canon_main = std::fs::canonicalize(main.path()).unwrap();
        let canon_worktree = std::fs::canonicalize(worktree.path()).unwrap();

        // Shape a linked worktree the same way `worktree_main_root`'s own unit test does:
        // a `.git` FILE pointing `gitdir: <main>/.git/worktrees/<name>`.
        std::fs::write(
            worktree.path().join(".git"),
            // `join`, not concatenation — `canon_main` is canonicalized, and a verbatim
            // Windows path does not treat `/` as a separator, so the concatenated form
            // parses as a single component and the worktree goes undetected.
            format!(
                "gitdir: {}\n",
                canon_main
                    .join(".git")
                    .join("worktrees")
                    .join("feat")
                    .display()
            ),
        )
        .unwrap();

        let agent = crate::agent::Agent::new(Some(canon_main.clone()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent,
            false,
            "wt-session".to_string(),
            "cc-wt".to_string(),
            None,
        );
        let input = json!({"query": "x"});

        let _ = recorder
            .record_content("symbols", &input, Some(&canon_worktree), || async {
                Ok(vec![Content::text("ok")])
            })
            .await;

        assert!(
            !worktree.path().join(".codescout").join("usage.db").exists(),
            "the worktree's own usage.db must never be created — it would be deleted \
             with the worktree"
        );

        let conn = crate::usage::db::open_db(&canon_main).unwrap();
        let (count, project_root): (i64, String) = conn
            .query_row(
                "SELECT COUNT(*), MAX(project_root) FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "the call's telemetry must land in the MAIN checkout's usage.db"
        );
        assert_eq!(
            project_root,
            canon_worktree.to_string_lossy(),
            "project_root must still name the worktree, so the row stays distinguishable \
             from the main checkout's own calls"
        );
    }

    #[tokio::test]
    async fn record_content_stores_output_for_errors_in_debug_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            true,
            "test-session".to_string(),
            "cc-test".to_string(),
            None,
        );
        let input = json!({"path": "/bad/path"});

        let _ = recorder
            .record_content("read_file", &input, None, || async {
                Err(anyhow::anyhow!("file not found"))
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, out): (Option<String>, Option<String>) = conn
            .query_row("SELECT input_json, output_json FROM tool_calls", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();

        assert!(inp.is_some(), "input_json should be populated");
        assert!(out.is_some(), "output_json should be populated for errors");
        assert!(out.unwrap().contains("file not found"));
    }

    /// A `RecoverableError` stores `outcome="recoverable_error"`; a hard error does not.
    ///
    /// **Both directions, in one database, because either alone is monotone.**
    /// Asserting only the recoverable row passes against a classifier that
    /// returns `"recoverable_error"` for everything — which is the same
    /// two-value column this bug was about, relabelled. Asserting only the
    /// hard row is what the code did before the fix.
    ///
    /// **It asserts the STORED ROW, not `classify_content_result`'s return.**
    /// That is the whole point of siting it here: the classifier could be
    /// unit-tested green while the value never reached `usage.db`, because
    /// the disposition is decided by `route_tool_error` in `src/server.rs`
    /// *after* the recorder has already written. A unit test of the
    /// classifier cannot see that ordering; this traverses it.
    ///
    /// Measured before the fix: 0 `recoverable_error` rows in 57k calls, in a
    /// column documented and queried as a three-value taxonomy, with two live
    /// queries filtering on the dead value and returning empty without saying
    /// so.
    ///
    /// docs/issues/archive/2026-09-02-recoverable-error-outcome-is-unreachable-in-production.md
    #[tokio::test]
    async fn record_content_distinguishes_a_recoverable_error_from_a_hard_one() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "test-session".to_string(),
            "cc-test".to_string(),
            None,
        );
        let input = json!({});

        // The shape every codescout guard uses: a RecoverableError travelling
        // as `Err`, routed to isError:false further up. Before the fix this
        // landed as "error", indistinguishable from the row below it.
        let _ = recorder
            .record_content("edit_file", &input, None, || async {
                Err(crate::tools::RecoverableError::new("0 matches for old_string").into())
            })
            .await;
        let _ = recorder
            .record_content("read_file", &input, None, || async {
                Err(anyhow::anyhow!("file not found"))
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT tool_name, outcome FROM tool_calls ORDER BY tool_name")
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            rows,
            vec![
                ("edit_file".to_string(), "recoverable_error".to_string()),
                ("read_file".to_string(), "error".to_string()),
            ],
            "the outcome column must separate a guard firing correctly from a hard \
                 failure; got {rows:?}"
        );
    }

    #[tokio::test]
    async fn record_content_no_input_in_normal_mode() {
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "test-session".to_string(),
            "cc-test".to_string(),
            None,
        );
        let input = json!({"query": "test_symbol"});

        let _ = recorder
            .record_content("symbols", &input, None, || async {
                Ok(vec![Content::text("found it")])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, sid, cs): (Option<String>, String, String) = conn
            .query_row(
                "SELECT input_json, session_id, codescout_sha FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        assert!(inp.is_none(), "input_json should be None in normal mode");
        assert_eq!(sid, "test-session", "session_id should always be set");
        assert!(!cs.is_empty(), "codescout_sha should always be set");
    }

    #[tokio::test]
    async fn record_content_populates_friction_fields_on_overflow() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "test-session".to_string(),
            "cc-test".to_string(),
            None,
        );
        let input = json!({"name_path": "LspManager/get_or_start", "path": "src/lsp/manager.rs"});

        let _ = recorder
            .record_content("symbols", &input, None, || async {
                Ok(vec![Content::text(
                    r#"{"output_id":"@tool_x","summary":"...","buffered_bytes":10000}"#.to_string(),
                )])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (overflowed, ft, tok, pr): (i64, Option<String>, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT overflowed, friction_target, overflow_tokens, project_root FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(overflowed, 1, "output_id envelope -> overflowed");
        assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
        assert_eq!(tok, Some(2500), "10000 bytes / 4");
        assert!(pr.is_some(), "project_root always set");
    }

    /// The wiring from extractor to column, end to end — and the reason these are
    /// columns at all.
    ///
    /// `debug` is **false** here, so `input_json` and `output_json` are both NULL. If
    /// the linkage were derived at query time from those blobs instead of recorded,
    /// this row would carry no linkage whatever and the join would have nothing to
    /// stand on. That is the whole argument against the precedent in
    /// `extract_friction_target_ignores_shell_commands` ("already recoverable at query
    /// time, so storing a derived form buys nothing") — true of `command`, false here,
    /// because that reasoning assumed a debug-gated field would be present.
    #[tokio::test]
    async fn record_content_records_buffer_linkage_with_debug_off() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let recorder = UsageRecorder::new(
            agent.clone(),
            false,
            "test-session".to_string(),
            "cc-test".to_string(),
            None,
        );

        // A call that BOTH names an existing handle and overflows into a new one, so a
        // swap of the two fields cannot pass by writing the same value to both.
        let input = json!({"path": "@cmd_prior1"});
        let _ = recorder
            .record_content("read_file", &input, None, || async {
                Ok(vec![Content::text(
                    r#"{"output_id":"@tool_fresh2","buffered_bytes":10000}"#.to_string(),
                )])
            })
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (inp, emitted, reads): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT input_json, emitted_output_id, read_output_ids FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(
            inp.is_none(),
            "debug is off — this is the configuration the columns exist for"
        );
        assert_eq!(
            emitted.as_deref(),
            Some("@tool_fresh2"),
            "the handle this call handed out"
        );
        assert_eq!(
            reads.as_deref(),
            Some(r#"["@cmd_prior1"]"#),
            "the handle this call named — a DIFFERENT value, so a field swap reds here"
        );
    }
}
