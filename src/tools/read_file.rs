//! `read_file` tool and read helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::{insert_below_header, overflow_head};
use super::{
    normalize_line_nav_aliases, optional_u64_param, OutputForm, RecoverableError, Tool, ToolContext,
};
use crate::util::text::extract_lines;

pub struct ReadFile;

#[async_trait::async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::read_only_closed()
    }

    fn description(&self) -> &str {
        "Read a file. Large output → @file_* buffer. Markdown: heading map by default; \
     heading=/headings= for a section, force=true for raw lines. Format-aware: \
     json_path (JSON), toml_key (TOML/YAML). Source: a line range overlapping a \
     symbol redirects to symbols(include_body=true); force=true bypasses."
    }

    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            // NO top-level anyOf/oneOf/allOf — the Anthropic Messages API rejects them
            // and Claude Code drops the whole tool client-side. The path alternation is
            // enforced in call() below, not here. See
            // no_tool_schema_declares_a_top_level_combinator (src/server.rs).
            "properties": {
                "path": { "type": "string", "description": "File path relative to project root" },
                "start_line": { "type": "integer", "description": "First line (1-indexed). Pair with end_line." },
                "end_line": { "type": "integer", "description": "Last line (1-indexed, inclusive). Pair with start_line." },
                "offset": { "type": "integer", "description": "Native-Read-style alias: 1-indexed start line (= start_line). Ignored when start_line/end_line are set." },
                "limit": { "type": "integer", "description": "Native-Read-style alias: line count from offset (end_line = offset + limit - 1). offset defaults to line 1 if omitted." },
                "heading": { "type": "string", "description": "Markdown only: return one section by heading (e.g. \"## Auth\")." },
                "headings": { "type": "array", "items": { "type": "string" }, "description": "Markdown only: return several sections. Mutually exclusive with heading." },
                "json_path": { "type": "string", "description": "JSON subtree by path (e.g. \"$.dependencies\")." },
                "toml_key": { "type": "string", "description": "TOML table or YAML section by key (e.g. \"dependencies\")." },
                "force": { "type": "boolean", "description": "Skip source-symbol hint and read the raw line range. Line ranges only — an oversized whole-file read is summarised either way." }
            }
        })
    }

    fn param_aliases(&self) -> crate::tools::param_alias::AliasMap {
        // `output_id`/`file_id` join the path family: `path` already accepts
        // `@tool_*`/`@cmd_*`/`@file_*` handles via `strip_buffer_ref_quotes`, so these
        // were renames of `path`, never a separate capability.
        //
        // LOAD-BEARING, and the detail is that this is NOT
        // `crate::fs::PATH_PARAM_ALIAS_MAP`: the last two pairs are exactly the two
        // that constant does not carry. Replacing this array with the shared map to
        // "tidy for consistency" breaks two things and NEITHER REDS.
        //   `read_file(output_id=…, heading=…)` starts refusing with "missing required
        //   parameter 'path'" — the `call()` fallback below resolves the alias into a
        //   LOCAL and never writes `input["path"]`, so `markdown::read` re-resolves
        //   from `path` + `PATH_PARAM_ALIASES` only and finds nothing.
        //   `read_file(output_id="@tool_x")` silently loses its `corrections` advisory
        //   while still succeeding.
        // `output_id` is the highest-traffic alias in the corpus, so "nobody sends it"
        // is measurably false. `path_aliases_and_alias_map_agree` (`src/fs/mod.rs`) pins
        // the other two copies of this set against each other and cannot see this one.
        &[
            ("file_path", "path"),
            ("relative_path", "path"),
            ("file", "path"),
            ("output_id", "path"),
            ("file_id", "path"),
        ]
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        // Native-`Read` compatibility: callers habitually pass offset/limit (a 1-indexed
        // start line + a line count, the built-in Read signature). Normalize to
        // start_line/end_line up front — before the buffer fork — so both the buffer and
        // real-file paths honor them through the same line-range logic instead of
        // silently returning the file head.
        let mut input = input;
        normalize_line_nav_aliases(&mut input);

        // Second layer, and redundant only for THREE of the five pairs.
        // `Tool::call_content` normalizes everything in `param_aliases()` onto `path`
        // before `call()` ever runs, so a caller reaching this fallback arrived via a
        // direct `call()` in a test that bypasses that boundary — true of `file_path`,
        // `relative_path` and `file`, which `PATH_PARAM_ALIASES` also carries. It is
        // FALSE for `output_id`/`file_id`: this chain is their only other resolver, so
        // if `param_aliases()` above is ever narrowed to the shared map, the failure is
        // silent here rather than loud. Kept because those direct-`call()` tests exist.
        let raw_path = input["path"]
            .as_str()
            .or_else(|| {
                crate::fs::PATH_PARAM_ALIASES
                    .iter()
                    .find_map(|a| input.get(*a).and_then(|v| v.as_str()))
            })
            // Buffer reads: agents habitually pass the returned handle back under
            // the key the tool emitted it as (output_id) rather than as path.
            .or_else(|| input["output_id"].as_str())
            .or_else(|| input["file_id"].as_str())
            .ok_or_else(|| {
                RecoverableError::with_hint(
                    "missing required parameter 'path'",
                    "read_file(path=\"src/x.rs\") — or read a buffer: read_file(path=\"@tool_abc\").",
                )
            })?;
        let path = strip_buffer_ref_quotes(raw_path);

        // Markdown: heading-addressed reads live in `markdown::read`, which `read_markdown`
        // used to wrap. Route there when the caller asked for headings, or the target is a
        // markdown file or a buffer that came from one — unless `force=true`, which keeps
        // its meaning of "raw line range, skip the smart path", or a format selector is
        // present, which falls through to the typed-format error below rather than being
        // silently ignored. `offset`/`limit` were already normalised above, so the markdown
        // path sees `start_line`/`end_line`.
        let wants_headings = input.get("heading").is_some() || input.get("headings").is_some();
        let wants_format = input.get("json_path").is_some() || input.get("toml_key").is_some();
        let force = input["force"].as_bool().unwrap_or(false);
        if wants_headings
            || (!force && !wants_format && crate::tools::markdown::is_markdown_target(path, ctx))
        {
            return crate::tools::markdown::read(input, ctx).await;
        }

        // Buffer refs bypass the filesystem entirely.
        if path.starts_with("@file_") || path.starts_with("@cmd_") || path.starts_with("@tool_") {
            // `read_from_buffer` attaches `buffer_truncated` itself, to every return shape,
            // before it measures the response: attached here, after sizing, it pushed a
            // response at the limit over it.
            return read_from_buffer(path, &input, ctx);
        }

        let project_root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        let security = ctx
            .agent
            .security_config_for(ctx.workspace_override.as_deref())
            .await;
        let resolved = crate::util::path_security::validate_read_path(
            path,
            project_root.as_deref(),
            &security,
        )?;

        let start_line = optional_u64_param(&input, "start_line");
        let end_line = optional_u64_param(&input, "end_line");
        validate_read_nav_params(&input, start_line, end_line)?;
        // start_line alone defaults end_line to a 50-line window (read_with_line_range
        // clamps past-EOF cases, so saturating_add is safe).
        let end_line = match (start_line, end_line) {
            (Some(s), None) => Some(s.saturating_add(49)),
            (_, e) => e,
        };

        let source_tag = compute_source_tag(&resolved, ctx).await;

        if resolved.is_dir() {
            return Err(RecoverableError::with_hint(
                format!("'{}' is a directory, not a file", path),
                "Use tree to browse directory contents, or provide a specific file path",
            )
            .into());
        }

        let text = read_file_text(path, &resolved)?;

        // Guard at the shared read, not at the markdown route: `force=true` and
        // `json_path`/`toml_key` both fall through to this raw path *specifically to
        // skip* the markdown dispatch above, and `markdown::read`'s own guard call
        // (read_markdown.rs) never runs for them. A guard that only fires on one of
        // two paths to the same bytes is the shape that produced
        // docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md
        // for writes; this is that defect's read twin; `force=true` on a
        // librarian-managed ledger returned its frontmatter with no warning.
        crate::util::librarian_guard::guard_not_librarian_managed(
            path,
            &text,
            Some(&resolved),
            crate::util::librarian_guard::Access::Read,
        )?;

        if let Some(jp) = input["json_path"].as_str() {
            return read_json_path_nav(&text, &resolved, jp, ctx);
        }
        if let Some(tk) = input["toml_key"].as_str() {
            return read_toml_yaml_key(&text, &resolved, tk, ctx);
        }

        let force = input["force"].as_bool().unwrap_or(false);

        if let (Some(start), Some(end)) = (start_line, end_line) {
            return read_with_line_range(
                path,
                &text,
                &resolved,
                start,
                end,
                &source_tag,
                ctx,
                force,
            );
        }
        read_full_file(path, &text, &resolved, &input, &source_tag, ctx)
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        if result.get("format").and_then(|f| f.as_str()) == Some("markdown") {
            return crate::tools::markdown::format_read(result);
        }
        Some(format_read_file(result))
    }
    fn json_path_hint(&self, val: &Value) -> String {
        // Buffered read results carry the payload under `content` (line ranges,
        // toml_key/json_path extractions, heading reads). Point agents there. A payload
        // WITHOUT a `content` string falls to the shared default, which names its largest
        // array. A whole-file outline no longer takes this branch: `read_full_file` bounds
        // it to the inline limit (`fit_envelope`), so it is never buffered a second time.
        // What reaches it today is the `headings` list a missed-heading read answers with
        // (`{ok: false, error, headings, hint}`), pinned by
        // `an_overflowing_missed_heading_list_hints_a_route_that_returns_data`.
        // The constant `$.field` it replaced exists in no read_file payload.
        if val["content"].is_string() {
            "$.content".to_string()
        } else {
            crate::tools::default_json_path_hint(val)
        }
    }
}

/// Strip surrounding quotes from buffer ref paths.
///
/// LLMs often wrap @ref paths in extra quoting — double quotes (`"@tool_abc"`),
/// single quotes (`'@tool_abc'`), or markdown-style backticks (`` `@tool_abc` ``).
/// Stripping any matched pair here lets the ref resolve correctly.
fn strip_buffer_ref_quotes(path: &str) -> &str {
    for q in ['"', '\'', '`'] {
        if let Some(inner) = path.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            if inner.starts_with("@file_")
                || inner.starts_with("@cmd_")
                || inner.starts_with("@tool_")
                || inner.starts_with("@ack_")
            {
                return inner;
            }
        }
    }
    path
}

/// Read from an output buffer ref (`@file_*`, `@cmd_*`, `@tool_*`).
///
/// Handles json_path navigation for `@tool_*` refs and line-range slicing.
/// Never re-wraps its own result in a `@tool_*` envelope: oversized content is
/// paginated via `shown_lines` / `next`, both stated in the ref's own line
/// numbers, with the slice parked under a `@file_*` handle so it stays
/// greppable.
fn read_from_buffer(path: &str, input: &Value, ctx: &ToolContext) -> Result<Value> {
    let raw = ctx.output_buffer.get_stream(path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("buffer reference not found: '{}'", path),
            "Buffer refs expire when the session resets. Re-run the command to get a fresh ref.",
        )
    })?;

    // A handle whose buffer holds only a prefix says so at EVERY read (the same contract as
    // run_command's `buffer_truncated`). It is part of every response below, so it is attached
    // BEFORE each one is measured: added afterwards, it pushed a response sized to the limit
    // over it, and `call_content` buffered that under a second handle.
    let notice = ctx.output_buffer.truncation_notice(path);
    let noted = |mut v: Value| -> Value {
        if let Some(n) = &notice {
            v["buffer_truncated"] = json!([n]);
        }
        v
    };

    // Navigation params this buffer ref cannot honor must fail loudly, not be
    // silently ignored (which masks caller misuse). `toml_key` is never valid
    // on a buffer (buffers are not TOML files); `json_path` is only meaningful
    // for @tool_* JSON refs — @cmd_*/@file_* buffers are raw text.
    if input["toml_key"].as_str().is_some() {
        return Err(RecoverableError::with_hint(
            format!("toml_key is not supported on buffer refs (got '{path}')"),
            "Buffer refs are not TOML files. Slice with start_line/end_line, or grep the ref, e.g. run_command(\"grep pattern @ref\").",
        )
        .into());
    }
    if input["json_path"].as_str().is_some() && !path.starts_with("@tool_") {
        return Err(RecoverableError::with_hint(
            format!("json_path is only supported on @tool_* refs, not '{path}'"),
            "@cmd_*/@file_* buffers are raw text. Slice with start_line/end_line, or grep the ref.",
        )
        .into());
    }

    // json_path navigation is only meaningful for @tool_* (always JSON), and it RE-PARSES
    // the text it is handed — so it reads the pretty-printed form UN-expanded, and must run
    // before `line_addressable_text` below. See that function's doc comment.
    if path.starts_with("@tool_") {
        if let Some(jp) = input["json_path"].as_str() {
            let text: String = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| serde_json::to_string_pretty(&v).ok())
                .unwrap_or_else(|| raw.clone());
            let (content, type_name, count) =
                crate::tools::file_summary::extract_json_path(&text, jp)?;
            // Decided on the response it would return, `count` included. The value's raw bytes
            // understate that response by its escaping and its other keys, and a response over
            // the limit is buffered again by `call_content` under a second handle. The raw
            // pre-check only skips building a candidate that cannot fit: escaping never
            // shrinks a string, so raw bytes over the limit mean the response is too.
            if !crate::tools::exceeds_inline_limit(&content) {
                let mut inline = json!({
                    "content": &content,
                    "path": jp,
                    "value_type": type_name,
                    "format": "json",
                });
                if let Some(c) = count {
                    inline["count"] = json!(c);
                }
                let inline = noted(inline);
                if !crate::tools::exceeds_inline_limit(&inline.to_string()) {
                    return Ok(inline);
                }
            }
            let line_count = content.lines().count().max(1);
            let file_id = ctx
                .output_buffer
                .store_file(format!("{path}:{jp}"), content);
            let mut result = json!({
                "file_id": file_id,
                "path": jp,
                "value_type": type_name,
                "format": "json",
                "total_lines": line_count,
                "hint": format!(
                    "Extracted value at {jp} ({line_count} lines). \
                     read_file(\"{file_id}\", start_line=N, end_line=M) to browse, \
                     or run_command(\"grep pattern {file_id}\") to search."
                ),
            });
            if let Some(c) = count {
                result["count"] = json!(c);
            }
            return Ok(noted(result));
        }
    }

    // ONE derivation, shared with `grep`. Never inline this — see its doc comment.
    let text = crate::tools::output_buffer::line_addressable_text(path, raw);

    let total_lines = text.lines().count();
    let start = optional_u64_param(input, "start_line");
    let end = optional_u64_param(input, "end_line");
    // start_line alone defaults end_line to a 50-line window — same as the real-file path.
    let end = match (start, end) {
        (Some(s), None) => Some(s.saturating_add(49)),
        (_, e) => e,
    };

    if let (Some(s), Some(e)) = (start, end) {
        if s == 0 || e < s {
            return Err(RecoverableError::with_hint(
                format!(
                    "invalid line range: start_line={} end_line={} \
                     (start_line must be >= 1 and end_line >= start_line)",
                    s, e
                ),
                "Lines are 1-indexed. Example: start_line=1, end_line=50",
            )
            .into());
        }
        let content = extract_lines(&text, s as usize, e as usize);
        // The inline arm is chosen on the response it returns, not on the slice's raw bytes:
        // one line of 2,000 `\x01` is 2,000 raw bytes and 12,039 serialized, and was returned
        // here for `call_content` to buffer under `@tool_*`. The raw pre-check only skips a
        // candidate that cannot fit (escaping never shrinks a string).
        if !crate::tools::exceeds_inline_limit(&content) {
            let inline = noted(json!({ "content": &content, "total_lines": total_lines }));
            if !crate::tools::exceeds_inline_limit(&inline.to_string()) {
                return Ok(inline);
            }
        }
        {
            // The slice is still stored under its own handle: that keeps it
            // greppable, and it keeps THIS response small enough that
            // `call_content()` will not re-wrap it in a `@tool_*` envelope
            // (BUG-026, archived 2026-03-15).
            //
            // Navigation, though, continues against the ORIGINAL ref. `shown_lines`
            // and `total_lines` are that buffer's line numbers, so a `next` phrased
            // in the slice's own 1-based frame is off by `s - 1` and sends the
            // caller back over lines it has already seen — on a fresh handle each
            // time, which is what made these chains look like they never converged.
            let file_id = ctx
                .output_buffer
                .store_file(format!("{}[{}-{}]", path, s, e), content.clone());
            let orig_start = s as usize;
            // The page sized with every other key it can carry counted. `shown_lines` ends
            // at most at `e` and `next` resumes at most at `e + 1`, so these are the widest
            // values those keys can take.
            let widest = noted(json!({
                "content": "",
                "file_id": file_id,
                "total_lines": total_lines,
                "shown_lines": [orig_start, e],
                "complete": false,
                "line_truncated": true,
                "hint": over_budget_line_hint(path),
                "next": format!(
                    "read_file(\"{path}\", start_line={}, end_line={e})",
                    e.saturating_add(1)
                ),
            }));
            let (chunk, lines_shown, complete, line_truncated) =
                buffer_page(&content, buffer_page_room(&widest));
            let orig_end = orig_start + lines_shown.saturating_sub(1);
            let mut result = json!({
                "content": chunk,
                "file_id": file_id,
                "total_lines": total_lines,
                "shown_lines": [orig_start, orig_end],
                "complete": complete,
            });
            if line_truncated {
                // Deliberately does NOT set `next`: the only range that would
                // advance past this line is the same one that produced it, so a
                // `next` here rebuilds the retry loop the valve exists to break.
                // The hint routes to an addressing mode that can reach the value.
                result["line_truncated"] = json!(true);
                result["hint"] = json!(over_budget_line_hint(path));
            }
            if !complete {
                // `complete == false` means the budget stopped us short of `e`, and
                // the safety valve in `extract_lines_with_cost` always yields at
                // least one line — so this strictly advances and terminates.
                result["next"] = json!(format!(
                    "read_file(\"{path}\", start_line={}, end_line={e})",
                    orig_end + 1
                ));
            }
            return Ok(noted(result));
        }
    }

    // Full buffer: paginate if the RESPONSE is over the inline limit. Never re-buffer.
    if !crate::tools::exceeds_inline_limit(&text) {
        let inline = noted(json!({ "content": &text, "total_lines": total_lines }));
        if !crate::tools::exceeds_inline_limit(&inline.to_string()) {
            return Ok(inline);
        }
    }
    {
        // A page that stops short has `lines_shown < total_lines`, so `next` resumes at
        // most at `total_lines`: these are the widest values every key can take.
        let widest = noted(json!({
            "content": "",
            "total_lines": total_lines,
            "shown_lines": [1, total_lines],
            "complete": false,
            "line_truncated": true,
            "hint": over_budget_line_hint(path),
            "next": format!(
                "read_file(\"{path}\", start_line={total_lines}, end_line={total_lines})"
            ),
        }));
        let (chunk, lines_shown, complete, line_truncated) =
            buffer_page(&text, buffer_page_room(&widest));
        let mut result = json!({
            "content": chunk,
            "total_lines": total_lines,
            "shown_lines": [1, lines_shown],
            "complete": complete,
        });
        if line_truncated {
            result["line_truncated"] = json!(true);
            result["hint"] = json!(over_budget_line_hint(path));
        }
        if !complete {
            let next_start = lines_shown + 1;
            let next_end = (next_start + lines_shown - 1).min(total_lines);
            result["next"] = json!(format!(
                "read_file(\"{path}\", start_line={next_start}, end_line={next_end})"
            ));
        }
        Ok(noted(result))
    }
}

/// Cut a chunk down when a SINGLE line is wider than the room its page has for it.
///
/// The safety valve in `extract_lines_with_cost` deliberately emits at least one
/// line even when that line exceeds the budget — without it, a caller re-requests
/// the same range forever and never advances. The cost is that
/// [`read_from_buffer`] then returns a chunk larger than the threshold it is
/// measured against, `call_content` re-wraps the response in a `@tool_*`
/// envelope, and the caller gets an envelope instead of content: exactly what
/// that function's doc comment promises never happens.
///
/// The valve exists to guarantee *progress*, not completeness — so keep the
/// progress and drop the excess bytes. Returns `(chunk, true)` when it cut.
///
/// `room` is in SERIALIZED bytes, the unit the response is judged in, and so is the
/// cut. This used to test the line's raw bytes and keep half the budget raw: a line of
/// 2,000 `\x01` (12,000 B escaped) was never cut, and a cut line of them kept 4,500 raw
/// bytes, 27,000 escaped. The kept prefix is never empty for a non-empty line: at least
/// its first character survives, even in a room too small for it.
///
/// Measured 2026-08-29: a `run_command` envelope pretty-prints to four lines, of
/// which the third is the entire stdout as one JSON-escaped string 9998 bytes
/// wide. Line-slicing could never address it. See
/// `docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`.
fn clamp_over_budget_line(chunk: String, room: usize) -> (String, bool) {
    use crate::util::text::{clip_head_escaped, json_escaped_len};
    if json_escaped_len(&chunk) <= room {
        return (chunk, false);
    }
    let kept = clip_head_escaped(
        &chunk,
        room.saturating_sub(json_escaped_len(OVER_BUDGET_MARKER)),
    );
    let keep = match kept.len() {
        0 => chunk.chars().next().map_or(0, char::len_utf8),
        n => n,
    };
    let mut out = chunk;
    out.truncate(keep);
    out.push_str(OVER_BUDGET_MARKER);
    (out, true)
}

/// Appended to a line [`clamp_over_budget_line`] cut.
// cap-class: NOT_A_CAP — the text a cut line ends with; the cut's bound is the page's room.
const OVER_BUDGET_MARKER: &str = "\n…[truncated: this line is wider than the inline budget]";

/// What a page has left for `content`, in serialized bytes: the response limit less every
/// other key the page can carry. Used by every paging range arm: [`read_from_buffer`],
/// [`read_with_line_range`] and the markdown range arm (`read_markdown_line_range`).
///
/// `widest` is the page's response with `content` set to `""` and every optional key present
/// at the widest value it can take. The real page carries a subset of those keys with values
/// no wider, so a `content` whose ESCAPED length fits this room keeps the compact response
/// within the limit `call_content` judges it by. A room counted in raw bytes, or with the
/// other keys left out, let that response be buffered again under a second handle.
///
/// No `INLINE_BYTE_BUDGET` margin on top: that 10% existed for estimates in the wrong unit,
/// and this one is exact. `call_content` measures the value this function's caller returns;
/// what it adds afterwards (`_guide_hint`, parameter corrections) is added after the
/// buffering decision.
pub(super) fn buffer_page_room(widest: &Value) -> usize {
    crate::tools::INLINE_MAX_RESPONSE_LEN.saturating_sub(widest.to_string().len())
}

/// One page of `body`, from its first line, whose content fits `room` serialized bytes.
/// Returns `(chunk, lines_shown, complete, line_truncated)`.
pub(super) fn buffer_page(body: &str, room: usize) -> (String, usize, bool, bool) {
    let (chunk, lines_shown, complete) =
        crate::util::text::extract_lines_to_json_budget(body, 1, usize::MAX, room);
    // The valve yields one line even when that line alone busts the room.
    let (chunk, line_truncated) = clamp_over_budget_line(chunk, room);
    (chunk, lines_shown, complete, line_truncated)
}

/// The advisory attached whenever [`clamp_over_budget_line`] cuts.
///
/// Kept separate from the cut so the wording is testable without a
/// `ToolContext`, and so both call sites phrase it identically. It names
/// `json_path` because on a `@tool_*` ref an over-budget line is almost always a
/// JSON-escaped payload, and field addressing reaches it in one call where line
/// addressing cannot reach it at all.
pub(super) fn over_budget_line_hint(path: &str) -> String {
    // `grep -o` prints only the part of the line that matched. Plain `grep` returns the whole
    // line, which is the thing that was too wide; `[^,]*` ends the match at the next comma,
    // which bounds a hit inside JSON or CSV. The `PATTERN` placeholder is the caller's to fill.
    let grep = format!("run_command(\"grep -o 'PATTERN[^,]*' {path}\")");
    if path.starts_with("@tool_") {
        format!(
            "A single line here is wider than the inline budget, so it is shown truncated. \
             On a @tool_* ref that is usually a JSON-escaped payload on one line — address the \
             field instead of the line: read_file(\"{path}\", json_path=\"$.stdout\") for a \
             run_command envelope, or json_path=\"$.<field>\" generally. \
             {grep} prints just the matching part of the line."
        )
    } else {
        // Every caller passes a buffer ref: `read_from_buffer` the ref it was asked to read,
        // and `read_with_line_range` and the markdown range arm (`read_markdown_line_range`)
        // the `@file_*` handle they just stored the slice under. So this is a `@cmd_*` /
        // `@file_*` ref, and `json_path` is refused on those ("only supported on @tool_*
        // refs"). The branch used to advise it anyway.
        format!(
            "A single line here is wider than the inline budget, so it is shown truncated. \
             Fields cannot be addressed on this kind of ref. Print just the part you need: \
             {grep} (replace PATTERN; `[^,]*` ends the match at the next comma)."
        )
    }
}

/// Validate navigation parameter combinations for real-file reads.
///
/// `start_line` alone is allowed — the caller defaults `end_line` to a 50-line
/// window. The validation only rejects mutually-exclusive combinations and the
/// (None, Some) shape (end_line without start_line is meaningless).
fn validate_read_nav_params(
    input: &Value,
    start_line: Option<u64>,
    end_line: Option<u64>,
) -> Result<()> {
    if start_line.is_none() && end_line.is_some() {
        return Err(RecoverableError::with_hint(
            "end_line provided without start_line",
            "Pass start_line (end_line defaults to start_line+49), or pass both for an explicit range.",
        )
        .into());
    }
    let json_path = input["json_path"].as_str();
    let toml_key = input["toml_key"].as_str();
    let nav_count = usize::from(json_path.is_some()) + usize::from(toml_key.is_some());
    if nav_count > 1 {
        return Err(RecoverableError::with_hint(
            "only one navigation parameter allowed at a time",
            "Use json_path OR toml_key, not both",
        )
        .into());
    }
    if nav_count > 0 && (start_line.is_some() || end_line.is_some()) {
        return Err(RecoverableError::with_hint(
            "navigation parameters are mutually exclusive with start_line/end_line",
            "Use either json_path/toml_key OR start_line+end_line",
        )
        .into());
    }
    Ok(())
}

/// Resolve the library source tag for a file (`"project"` or `"lib:<name>"`),
/// honoring the per-request workspace pin so a pinned read tags against the
/// pinned project's library registry, not the default's.
async fn compute_source_tag(resolved: &std::path::Path, ctx: &ToolContext) -> String {
    let tag = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok(p.library_registry
                .is_library_path(resolved)
                .map(|lib| format!("lib:{}", lib.name)))
        })
        .await;
    match tag {
        Ok(Some(t)) => t,
        _ => "project".to_string(),
    }
}

/// Read file contents with user-friendly error messages.
fn read_file_text(path: &str, resolved: &std::path::PathBuf) -> Result<String> {
    std::fs::read_to_string(resolved).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RecoverableError::with_hint(
            format!(
                "file not found: '{}' (searched {})",
                path,
                resolved.display()
            ),
            "Check the path with tree, or use tree with `glob` to locate the file. If \
             the root above is not the project you meant, a subagent sharing this \
             session's process may have changed the active project — call \
             workspace(action='status') to check.",
        )
        .into(),
        std::io::ErrorKind::InvalidData => RecoverableError::with_hint(
            "file contains non-UTF-8 data (binary file?)",
            "read_file only works with text files. Use tree to check file types.",
        )
        .into(),
        _ => anyhow::anyhow!("failed to read {}: {}", resolved.display(), e),
    })
}

/// Handle `json_path` navigation for JSON files.
fn read_json_path_nav(
    text: &str,
    resolved: &std::path::Path,
    jp: &str,
    ctx: &ToolContext,
) -> Result<Value> {
    let file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    if !matches!(file_type, crate::tools::file_summary::FileSummaryType::Json) {
        return Err(RecoverableError::with_hint(
            "json_path parameter is only supported for JSON files",
            "For Markdown files pass heading= or headings=, for TOML/YAML use toml_key",
        )
        .into());
    }
    let (content, type_name, count) = crate::tools::file_summary::extract_json_path(text, jp)?;
    let mut keys = json!({
        "path": jp,
        "value_type": type_name,
        "format": "json",
    });
    if let Some(c) = count {
        keys["count"] = json!(c);
    }
    Ok(inline_or_file_id(
        content,
        keys,
        &[],
        &resolved.to_string_lossy(),
        &format!("Extracted value at {jp}"),
        ctx,
    ))
}

/// The response of a format-navigation read (`json_path`, `toml_key`) of a REAL file: the
/// extracted `content` beside `keys` when that whole response fits the inline limit, else the
/// content stored under one `@file_*` handle that is browsed by line range.
///
/// Decided on the response it would return, as the `@tool_*` `json_path` arm of
/// [`read_from_buffer`] decides: these reads had no gate at all, so a value over the limit went
/// out whole and `call_content` buffered it under `@tool_*`. The raw pre-check only skips a
/// candidate that cannot fit (escaping never shrinks a string). `droppable` keys have no length
/// of their own (`siblings`): the handle arm carries one only when it still fits, and marks it
/// `<key>_omitted` otherwise, so that arm stays small enough for `call_content` to leave alone.
fn inline_or_file_id(
    content: String,
    keys: Value,
    droppable: &[&str],
    source: &str,
    what: &str,
    ctx: &ToolContext,
) -> Value {
    if !crate::tools::exceeds_inline_limit(&content) {
        let mut inline = keys.clone();
        inline["content"] = json!(&content);
        if !crate::tools::exceeds_inline_limit(&inline.to_string()) {
            return inline;
        }
    }
    let line_count = content.lines().count().max(1);
    // An excerpt: a snapshot of the extracted value. `store_file` would treat the source as
    // the WHOLE file, refresh the handle to it on the next mtime change, and, given a name
    // that is not a real path, evict it on the first read.
    let file_id = ctx
        .output_buffer
        .store_file_excerpt(source.to_string(), content);
    let mut result = keys;
    result["file_id"] = json!(file_id);
    result["total_lines"] = json!(line_count);
    result["hint"] = json!(format!(
        "{what} ({line_count} lines). \
         read_file(\"{file_id}\", start_line=N, end_line=M) to browse, \
         or run_command(\"grep pattern {file_id}\") to search."
    ));
    for key in droppable {
        if crate::tools::exceeds_inline_limit(&result.to_string()) {
            if let Some(obj) = result.as_object_mut() {
                if obj.remove(*key).is_some() {
                    obj.insert(format!("{key}_omitted"), json!(true));
                }
            }
        }
    }
    result
}

/// Handle `toml_key` navigation for TOML and YAML files.
fn read_toml_yaml_key(
    text: &str,
    resolved: &std::path::Path,
    tk: &str,
    ctx: &ToolContext,
) -> Result<Value> {
    let mut file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    // Cargo.lock (and most `.lock` files) are TOML, but detect_file_type
    // classifies `.lock` as Config. Coerce to TOML so toml_key works; a
    // non-TOML `.lock` (e.g. yarn.lock) surfaces a clear parse error below.
    if resolved.to_string_lossy().to_lowercase().ends_with(".lock") {
        file_type = crate::tools::file_summary::FileSummaryType::Toml;
    }
    let (result, format) = match file_type {
        crate::tools::file_summary::FileSummaryType::Toml => (
            crate::tools::file_summary::extract_toml_key(text, tk)?,
            "toml",
        ),
        crate::tools::file_summary::FileSummaryType::Yaml => (
            crate::tools::file_summary::extract_yaml_key(text, tk)?,
            "yaml",
        ),
        _ => {
            return Err(RecoverableError::with_hint(
                "toml_key parameter is only supported for TOML and YAML files",
                "For Markdown files pass heading= or headings=, for JSON use json_path",
            )
            .into())
        }
    };
    let keys = json!({
        "line_range": [result.line_range.0, result.line_range.1],
        "breadcrumb": result.breadcrumb,
        "siblings": result.siblings,
        "format": format,
    });
    Ok(inline_or_file_id(
        result.content,
        keys,
        &["siblings"],
        &resolved.to_string_lossy(),
        &format!("Extracted value at {tk}"),
        ctx,
    ))
}

/// Handle an explicit `start_line`+`end_line` range read from a real file.
#[allow(clippy::too_many_arguments)]
fn read_with_line_range(
    path: &str,
    text: &str,
    resolved: &std::path::PathBuf,
    start: u64,
    end: u64,
    source_tag: &str,
    ctx: &ToolContext,
    force: bool,
) -> Result<Value> {
    if start == 0 || end < start {
        return Err(RecoverableError::with_hint(
            format!(
                "invalid line range: start_line={} end_line={} \
                 (start_line must be >= 1 and end_line >= start_line)",
                start, end
            ),
            "Lines are 1-indexed. Example: start_line=1, end_line=50",
        )
        .into());
    }

    // A file-head read is the canonical "show me the imports" operation, and the
    // gate's recommended recovery cannot serve it: `symbols` is a *definition
    // projection* and does not return `use` / `mod` / `package` lines
    // (src/prompts/guides/iron-laws-detail.md). Refusing it routes the caller to a
    // tool that structurally cannot answer, offering `force=true` only second.
    //
    // The window is measured, not chosen. Across 373 refused reads carrying a
    // range: 131 start at line 1, and exactly ONE starts between lines 2 and 5.
    // So `start == 1` is the real shape — it costs one call out of 103 versus a
    // `start <= 5` window, and it avoids misreading a small file, where a read
    // like lines 3-5 is a whole function body rather than a head read.
    // 102 of those 103 also end by line 60; past that, a read that merely begins
    // at line 1 is a whole-file read in disguise and Iron Law 1 still applies.
    // Evidence: docs/issues/archive/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md
    // (archived 2026-08-18. That bug closed on THIS exemption and the extent-ordered hint —
    // its third step, stating the overlap condition in the always-loaded IL1 text, was
    // measured as prompt-hamsa A-25 and refuted. Do not "finish" the bug by re-adding it.)
    // cap-class: NOT_A_CAP — routing predicate deciding whether a read counts as a head read; it selects a code path and removes no content
    const HEAD_END_MAX: u64 = 60;
    let is_head_read = start == 1 && end <= HEAD_END_MAX;

    if !force
        && !is_head_read
        && crate::tools::file_summary::detect_file_type(path)
            == crate::tools::file_summary::FileSummaryType::Source
    {
        let matches = find_symbols_for_range(text, resolved, start, end);
        if !matches.is_empty() {
            let names: Vec<_> = matches
                .iter()
                .take(3)
                .map(|(n, _, _)| format!("'{n}'"))
                .collect();
            let mut label = names.join(", ");
            if matches.len() > 3 {
                label.push_str(&format!(" and {} more", matches.len() - 3));
            }
            let (first, first_start, first_end) = &matches[0];

            // Order the two escapes by what the caller actually asked for. The
            // requested extent is known here, and when the overlapping symbol is far
            // larger than the slice, `symbols(include_body=true)` returns strictly
            // MORE than was requested — recommending it first inverts Iron Law 1,
            // whose purpose is to stop oversized source reads.
            //
            // Two conditions, because a ratio alone misleads at small sizes:
            // returning a 4-line body for a 2-line request is 2x but costs nothing,
            // while returning 102 lines for 5 is the case worth reordering. So the
            // symbol must be BOTH proportionally larger and absolutely larger.
            //
            // The 2x ratio is a judgment call (unlike the head-read window above,
            // which is measured). The 40-line excess is the corpus's own boundary:
            // its "small slice" bucket — 97 of 244 refusals, the largest — is defined
            // as <= 40 lines, so an excess past that is more than a whole typical
            // request's worth of unasked-for content.
            const EXCESS_LINES_THAT_MATTER: u64 = 40;
            let requested = end.saturating_sub(start) + 1;
            let symbol_lines = u64::from(first_end.saturating_sub(*first_start)) + 1;
            let symbols_returns_much_more = symbol_lines >= requested.saturating_mul(2)
                && symbol_lines.saturating_sub(requested) > EXCESS_LINES_THAT_MATTER;

            let hint = if symbols_returns_much_more {
                format!(
                    "Pass force=true to read exactly the {requested} line(s) you asked for \
                     — '{first}' spans {symbol_lines} lines, so \
                     symbols(name='{first}', include_body=true) would return the whole body."
                )
            } else {
                format!(
                    "Use symbols(name='{first}', include_body=true) to read the body directly. \
                     Pass force=true to read the raw line range anyway."
                )
            };

            return Err(RecoverableError::with_hint(
                format!("source range overlaps named symbol(s): {label}"),
                hint,
            )
            .into());
        }
    }

    let content = extract_lines(text, start as usize, end as usize);
    let file_total_lines = text.lines().count();

    if content.is_empty() && (start as usize) > file_total_lines {
        return Err(RecoverableError::with_hint(
            format!(
                "line range {}-{} is past end of file ({} lines)",
                start, end, file_total_lines
            ),
            format!(
                "File has {} lines. Use a range within 1..={}.",
                file_total_lines, file_total_lines
            ),
        )
        .into());
    }

    let is_md = path.ends_with(".md") || path.ends_with(".markdown");
    let md_cov = if is_md {
        markdown_coverage(text, resolved, ctx, None, Some(start), Some(end))
    } else {
        None
    };

    // Every response below carries `source` and `coverage` beside the content, so both arms
    // are decided and sized with them in.
    let extras = |mut v: Value| -> Value {
        if source_tag != "project" {
            v["source"] = json!(source_tag);
        }
        if let Some(c) = &md_cov {
            v["coverage"] = c.clone();
        }
        v
    };

    // Inline when the RESPONSE fits, not when the slice's raw bytes do: a 12 KB line of
    // ASCII, or 9,990 B of escaped text, went inline here and was buffered by `call_content`
    // under `@tool_*`. The raw pre-check only skips a candidate that cannot fit (escaping
    // never shrinks a string).
    if !crate::tools::exceeds_inline_limit(&content) {
        let inline = extras(json!({ "content": &content }));
        if !crate::tools::exceeds_inline_limit(&inline.to_string()) {
            return Ok(inline);
        }
    }

    // Proactive buffering: oversized extracted ranges are stored as @file_* refs
    // so callers can navigate by line number (BUG-025 class).
    let file_id = ctx
        .output_buffer
        .store_file_excerpt(resolved.to_string_lossy().to_string(), content.clone());
    // Continue against the file itself, in the same line numbers `shown_lines` reports — a
    // `next` phrased in the slice buffer's own 1-based frame is off by `start - 1` and
    // re-serves seen lines.
    //
    // A continuation is a SUBrange of a range the overlap gate already allowed, so it cannot
    // newly trip that gate — with one exception: the head-read exemption turns on
    // `start == 1`, which a follow-up no longer satisfies. Carry `force=true` on source files
    // so the call we hand back is one the caller can actually make.
    //
    // And carry it whenever the caller passed it: on a markdown target `force=true` is what
    // routed the read HERE instead of to `markdown::read`, so a `next` without it resumes in
    // a different arm, with a different response shape, from the one serving this read.
    let force_arg = if force
        || crate::tools::file_summary::detect_file_type(path)
            == crate::tools::file_summary::FileSummaryType::Source
    {
        ", force=true"
    } else {
        ""
    };
    let orig_start = start as usize;
    // The page sized with every other key counted at its widest: `shown_lines` ends at most
    // at `end` and `next` resumes at most at `end + 1`. The over-wide-line hint names the
    // slice's own handle, where `grep -o` reaches the line.
    let widest = extras(json!({
        "content": "",
        "file_id": file_id,
        "total_lines": file_total_lines,
        "shown_lines": [orig_start, end],
        "complete": false,
        "line_truncated": true,
        "hint": over_budget_line_hint(&file_id),
        "next": format!(
            "read_file(\"{path}\", start_line={}, end_line={end}{force_arg})",
            end.saturating_add(1)
        ),
    }));
    let (chunk, lines_shown, complete, line_truncated) =
        buffer_page(&content, buffer_page_room(&widest));
    let orig_end = orig_start + lines_shown.saturating_sub(1);
    let mut result = json!({
        "content": chunk,
        "file_id": file_id,
        "total_lines": file_total_lines,
        "shown_lines": [orig_start, orig_end],
        "complete": complete,
    });
    if line_truncated {
        // No `next` for the line itself: the range that would re-read it is the one that
        // produced it. `next` below resumes AFTER it.
        result["line_truncated"] = json!(true);
        result["hint"] = json!(over_budget_line_hint(&file_id));
    }
    if !complete {
        result["next"] = json!(format!(
            "read_file(\"{path}\", start_line={}, end_line={end}{force_arg})",
            orig_end + 1
        ));
    }
    Ok(extras(result))
}

/// The overflow hint for a whole-file read that was summarised instead of returned.
///
/// `forced` is not a formatting flag. It is the answer to a question the caller asked
/// and the tool silently discarded: `force=true` bypasses the symbol-overlap refusal on
/// a LINE RANGE (`read_with_line_range`), and has never bypassed the size budget —
/// `read_full_file` accepted the parameter and dropped it without a word.
///
/// Kept as a drop rather than made to work, deliberately. Progressive disclosure is the
/// project's design principle (`docs/PROGRESSIVE_DISCOVERABILITY.md`), the input schema
/// already scopes `force` to "the raw line range", and Iron Law 1 says the same. So the
/// defect is the SILENCE, not the budget — the same shape, and the same fix, as
/// `docs/issues/archive/2026-08-07-grep-zero-match-silent-about-hidden-skip.md`: make the
/// result self-describing rather than change what the tool does.
///
/// The note is conditional on purpose. One that fired on every oversized read would be
/// boilerplate rather than a signal, and `outline_hint_stays_silent_about_force_when_not_forced`
/// pins that half.
///
/// Pure, so the wording is testable without a ToolContext or a >10 KB fixture on disk.
fn outline_hint(file_id: &str, is_source: bool, forced: bool) -> String {
    let mut hint = if is_source {
        format!(
            "Outline only — no file content included. For source, prefer \
             symbols(path) then symbols(name='...', include_body=true). To read \
             lines: read_file(path=\"{file_id}\", start_line=N, end_line=M)."
        )
    } else {
        format!(
            "Outline only — no file content included. Read ranges from the buffer: \
             read_file(path=\"{file_id}\", start_line=N, end_line=M)."
        )
    };
    if forced {
        hint.push_str(
            " force=true had no effect on this read: it bypasses the symbol-overlap \
             refusal on a line range, not the size budget, so the file was still \
             summarised. Pass start_line/end_line together with force=true to read a \
             range inline.",
        );
    }
    hint
}

/// Handle a full-file read (no range, no navigation param).
///
/// Large files are summarised and buffered. Small files are returned inline,
/// capped at `max_results` lines in exploring mode.
fn read_full_file(
    path: &str,
    text: &str,
    resolved: &std::path::PathBuf,
    input: &Value,
    source_tag: &str,
    ctx: &ToolContext,
) -> Result<Value> {
    use super::output::{OutputGuard, OverflowInfo};

    // `markdown_coverage` MARKS headings as seen, so it runs once, here, for whichever arm
    // returns.
    let md_cov = if path.ends_with(".md") || path.ends_with(".markdown") {
        markdown_coverage(text, resolved, ctx, None, None, None)
    } else {
        None
    };

    // Inline when the RESPONSE fits. This decided on the file's raw bytes, so a one-line file
    // of 10,000 ASCII bytes (10,030 B as a response) went inline and `call_content` buffered
    // it under `@tool_*`. The raw pre-check only skips a candidate that cannot fit (escaping
    // never shrinks a string).
    if !crate::tools::exceeds_inline_limit(text) {
        let inline = full_file_inline(path, text, resolved, input, source_tag, md_cov.clone());
        if !crate::tools::exceeds_inline_limit(&inline.to_string()) {
            return Ok(inline);
        }
    }
    {
        let file_id = ctx
            .output_buffer
            .store_file(resolved.to_string_lossy().to_string(), text.to_string());
        let summary =
            match crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy()) {
                crate::tools::file_summary::FileSummaryType::Source => {
                    crate::tools::file_summary::summarize_source(&resolved.to_string_lossy(), text)
                }
                crate::tools::file_summary::FileSummaryType::Markdown => {
                    crate::tools::file_summary::summarize_markdown(text)
                }
                crate::tools::file_summary::FileSummaryType::Json => {
                    crate::tools::file_summary::summarize_json(text)
                }
                crate::tools::file_summary::FileSummaryType::Yaml => {
                    crate::tools::file_summary::summarize_yaml(text)
                }
                crate::tools::file_summary::FileSummaryType::Toml => {
                    crate::tools::file_summary::summarize_toml(text)
                }
                crate::tools::file_summary::FileSummaryType::Config => {
                    crate::tools::file_summary::summarize_config(text)
                }
                crate::tools::file_summary::FileSummaryType::Generic => {
                    crate::tools::file_summary::summarize_generic_file(text)
                }
            };
        // Computed once above and handed to `finish`, which `fit_envelope` may call several
        // times.
        let coverage = md_cov;
        let is_source = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy())
            == crate::tools::file_summary::FileSummaryType::Source;
        let force = input["force"].as_bool().unwrap_or(false);

        // The whole response around a summary. This is the construction `read_full_file` always
        // had; it is a closure so `fit_envelope` can MEASURE the real envelope, and cut only if
        // that is over the inline limit, instead of cutting to a guess about what the keys cost.
        //
        // This summary describes a file it does not contain: an outline, zero content lines.
        // It carries `complete: false` and an `overflow` with `shown: 0` (literal, not a
        // placeholder) so a caller cannot take it for a complete read. See
        // `docs/issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md`.
        let finish = |mut result: Value, cut_notes: &[String]| -> Value {
            let summarised_lines = result["line_count"]
                .as_u64()
                .unwrap_or_else(|| text.lines().count() as u64)
                as usize;
            result["file_id"] = json!(file_id);
            result["complete"] = json!(false);
            result["overflow"] = OutputGuard::overflow_json(&OverflowInfo {
                shown: 0,
                total: summarised_lines,
                hint: {
                    let mut hint = outline_hint(&file_id, is_source, force);
                    // Where each cut array's middle can be read, as a ready-to-run call.
                    for note in cut_notes {
                        hint.push(' ');
                        hint.push_str(note);
                    }
                    hint
                },
                next_offset: None,
                by_file: None,
                by_file_overflow: 0,
            });
            if let Some(c) = &coverage {
                result["coverage"] = c.clone();
            }
            result
        };
        Ok(crate::tools::file_summary::fit_envelope(
            summary, &file_id, finish,
        ))
    }
}

/// The inline response of a whole-file read: the first page in exploring mode, else the whole
/// text. [`read_full_file`] returns it only when its serialized form fits the inline limit.
fn full_file_inline(
    path: &str,
    text: &str,
    resolved: &std::path::Path,
    input: &Value,
    source_tag: &str,
    md_cov: Option<Value>,
) -> Value {
    use super::output::{OutputGuard, OutputMode, OverflowInfo};

    let guard = OutputGuard::from_input(input);
    let total_lines = text.lines().count();
    let max_lines = guard.max_results;

    if guard.mode == OutputMode::Exploring && total_lines > max_lines {
        let content = extract_lines(text, 1, max_lines);
        let overflow = OverflowInfo {
            shown: max_lines,
            total: total_lines,
            hint: if crate::tools::file_summary::detect_file_type(path)
                == crate::tools::file_summary::FileSummaryType::Source
            {
                format!(
                    "File has {} lines. For source code, prefer symbols(path) \
                     + symbols(name=..., include_body=true) to read specific functions. \
                     Or use start_line/end_line to read a specific line range.",
                    total_lines
                )
            } else {
                format!(
                    "File has {} lines. Use start_line=N, end_line=M to read a specific range.",
                    total_lines
                )
            },
            next_offset: None,
            by_file: None,
            by_file_overflow: 0,
        };
        let mut result = json!({ "content": content, "total_lines": total_lines });
        if source_tag != "project" {
            result["source"] = json!(source_tag);
        }
        result["overflow"] = OutputGuard::overflow_json(&overflow);
        if let Some(c) = md_cov {
            result["coverage"] = c;
        }
        return result;
    }

    let mut result = json!({ "content": text, "total_lines": total_lines });
    if source_tag != "project" {
        result["source"] = json!(source_tag);
    }
    if crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy())
        == crate::tools::file_summary::FileSummaryType::Source
    {
        result["hint"] = json!(
            "Source file — prefer symbols(path) for overview, \
             symbols(name='...', include_body=true) for specific functions."
        );
    }
    if let Some(c) = md_cov {
        result["coverage"] = c;
    }
    result
}

/// Record which markdown headings were covered by a read operation and return
/// an optional `coverage` JSON value to merge into the response when unread
/// sections remain.
///
/// `heading_query` – the heading param if a single-section read was requested.
/// `start_line` / `end_line` – line-range bounds (1-indexed, inclusive) if a
///   range read was requested; both `None` means the whole file was read.
pub(super) fn markdown_coverage(
    text: &str,
    resolved: &std::path::PathBuf,
    ctx: &ToolContext,
    heading_query: Option<&str>,
    start_line: Option<u64>,
    end_line: Option<u64>,
) -> Option<serde_json::Value> {
    let all_headings = crate::tools::file_summary::parse_all_headings(text);
    if all_headings.is_empty() {
        return None;
    }
    let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();

    // Determine which headings were "seen" based on the read mode.
    let seen: Vec<String> = if let Some(query) = heading_query {
        // Single heading read — only that section.
        match crate::tools::file_summary::resolve_section_range(text, query) {
            Ok(range) => vec![range.heading_text],
            Err(_) => vec![],
        }
    } else if start_line.is_some() || end_line.is_some() {
        // Line-range read — mark headings whose heading line falls within range.
        let s = start_line.unwrap_or(1) as usize;
        let e = end_line.unwrap_or(usize::MAX as u64) as usize;
        all_headings
            .iter()
            .filter(|h| h.line >= s && h.line <= e)
            .map(|h| h.text.clone())
            .collect()
    } else {
        // Full file read — all headings seen.
        heading_texts.clone()
    };

    if !seen.is_empty() {
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.mark_seen(resolved, &seen);
        }
    }

    // Return a coverage hint only when unread sections remain.
    if let Ok(mut cov) = ctx.section_coverage.lock() {
        if let Some(status) = cov.status(resolved, &heading_texts) {
            if !status.unread.is_empty() {
                return Some(serde_json::json!({
                    "read": status.read_count,
                    "total": status.total_count,
                    "unread": status.unread,
                }));
            }
        }
    }
    None
}

pub(super) fn format_read_file(val: &Value) -> String {
    // Applied at the WRAPPER, not inside the body, because the body has five return
    // paths (summary mode, the `shown_lines` slice, the legacy no-content buffered
    // mode, empty content, and whole content) and the notice belongs on all of them.
    // The live probe that caught this hit the `shown_lines` path; a fix threaded
    // through only that one would have been just as invisible on the other four.
    //
    // Head-placed via `insert_below_header` for the reason `overflow_head` documents:
    // the content this notice describes is a PREFIX, so appending the notice after it
    // lets the content push it out of the kept window — reproducing the exact defect
    // the notice reports.
    insert_below_header(
        format_read_file_body(val),
        &crate::tools::format::truncation_head(val),
    )
}

fn format_read_file_body(val: &Value) -> String {
    // Summary modes have a "type" key
    if let Some(file_type) = val["type"].as_str() {
        return format_read_file_summary(val, file_type);
    }

    // Auto-chunked response: shown_lines present means partial read with content.
    // Line numbers are intentionally NOT prefixed — the caller supplied the range,
    // so per-line numbers are redundant noise (and were slice-relative/wrong here
    // before). See docs/issues/archive/2026-05-21-read-file-slice-relative-line-numbers.md.
    if val.get("shown_lines").and_then(|v| v.as_array()).is_some() {
        let total = val["total_lines"].as_u64().unwrap_or(0);
        let complete = val["complete"].as_bool().unwrap_or(true);
        let content = val["content"].as_str().unwrap_or("");
        let lines_shown = content.lines().count();

        let mut out = format!("{total} lines\n\n");
        out.push_str(content);

        if let Some(file_id) = val["file_id"].as_str() {
            out.push_str(&format!("\n\n  Buffer: {file_id}"));
        }
        // A clamped line carries a hint that names the route off it. This renderer used to drop
        // it, so the caller saw `…[truncated: this line is wider than the inline budget]` and
        // nothing to do about it; the hint existed only in JSON no one reads in the text form.
        if val["line_truncated"].as_bool() == Some(true) {
            if let Some(hint) = val["hint"].as_str() {
                out.push_str(&format!("\n\n  {hint}"));
            }
        }
        if !complete {
            out.push_str(&format!("\n  [{lines_shown} of {total} lines shown]"));
            if let Some(next) = val["next"].as_str() {
                out.push_str(&format!("\n  Next: {next}"));
            }
        }
        return out;
    }

    // Old no-content buffered mode (kept for backward compat)
    if val.get("content").is_none() {
        if let Some(file_id) = val["file_id"].as_str() {
            let total = val["total_lines"].as_u64().unwrap_or(0);
            let mut out = format!("{total} lines\n\n  Buffer: {file_id}");
            if let Some(hint) = val["hint"].as_str() {
                out.push_str(&format!("\n  {hint}"));
            }
            return out;
        }
    }

    // Content mode
    let content = match val["content"].as_str() {
        Some(c) => c,
        None => return String::new(),
    };

    let total_lines = val["total_lines"]
        .as_u64()
        .unwrap_or_else(|| content.lines().count() as u64);

    if content.is_empty() {
        // A zero NAMES ITS SCOPE when the scope is non-empty. `0 lines` alone is worse than
        // uninformative here: the non-empty branch below spends the same two words on the
        // TOTAL, so one phrase would denote two quantities and an empty slice of a 157-line
        // buffer reads as "the content is not there" — the one conclusion that is false.
        // The total was already in the payload and already in the local above.
        // `docs/adrs/2026-08-27-negative-results-name-their-scope.md`: name the scope when
        // the zero is suspicious, stay silent when it is trustworthy — hence the branch
        // rather than an unconditional suffix.
        let header = if total_lines > 0 {
            format!("0 lines (target has {total_lines}; requested range is past the end)")
        } else {
            "0 lines".to_string()
        };
        return insert_below_header(header, &overflow_head(val));
    }

    // Raw content, no per-line number prefixes (caller-supplied ranges make them
    // redundant; full-file reads can re-derive line numbers trivially).
    let line_word = if total_lines == 1 { "line" } else { "lines" };
    let mut out = format!("{total_lines} {line_word}\n\n");
    out.push_str(content);

    // Below the header, not after the content. This is the sharpest instance of the tail
    // cut on any surface: `content` is a whole file, so an overflow note appended here is
    // dropped essentially always — the reader is told "1505 lines" and shown a prefix,
    // with the sentence saying it is a prefix cut away. See `format::overflow_head`.
    insert_below_header(out, &overflow_head(val))
}

/// The line to print where `bound_summary` cut the middle out of `val[key]`, if `index` is
/// where the gap falls: `    … 1384 symbols omitted (L59-L1442) …`. `None` everywhere else, and
/// for a summary that was not cut.
pub(crate) fn omitted_gap(val: &Value, key: &str, index: usize) -> Option<String> {
    let gap = val.get(format!("{key}_omitted"))?;
    if gap["after"].as_u64()? as usize != index {
        return None;
    }
    let count = gap["count"].as_u64()?;
    let span = match (gap["from_line"].as_u64(), gap["to_line"].as_u64()) {
        (Some(f), Some(t)) => format!(" (L{f}-L{t})"),
        _ => String::new(),
    };
    Some(format!("\n    … {count} {key} omitted{span} …"))
}

fn format_read_file_summary(val: &Value, file_type: &str) -> String {
    let line_count = val["line_count"].as_u64().unwrap_or(0);

    let type_label = match file_type {
        "markdown" => " (Markdown)",
        "json" => " (JSON)",
        "yaml" => " (YAML)",
        "toml" => " (TOML)",
        "config" => " (Config)",
        _ => "",
    };
    let mut out = format!("{line_count} lines{type_label}\n");

    match file_type {
        "source" => {
            if let Some(symbols) = val["symbols"].as_array() {
                if !symbols.is_empty() {
                    out.push_str("\n  Symbols:");

                    // Compute alignment widths
                    let max_kind = symbols
                        .iter()
                        .map(|s| s["kind"].as_str().unwrap_or("").len())
                        .max()
                        .unwrap_or(0);
                    let max_name = symbols
                        .iter()
                        .map(|s| s["name"].as_str().unwrap_or("").len())
                        .max()
                        .unwrap_or(0);

                    for (i, sym) in symbols.iter().enumerate() {
                        if let Some(gap) = omitted_gap(val, "symbols", i) {
                            out.push_str(&gap);
                        }
                        let kind = sym["kind"].as_str().unwrap_or("?");
                        let name = sym["name"].as_str().unwrap_or("?");
                        let line = sym["line"].as_u64().unwrap_or(0);
                        let kind_pad = " ".repeat(max_kind - kind.len());
                        let name_pad = " ".repeat(max_name.saturating_sub(name.len()));
                        out.push_str(&format!(
                            "\n    {kind}{kind_pad}  {name}{name_pad}  L{line}"
                        ));
                    }
                }
            }
        }
        "markdown" => {
            if let Some(headings) = val["headings"].as_array() {
                if !headings.is_empty() {
                    out.push_str("\n  Headings:");
                    for (i, h) in headings.iter().enumerate() {
                        if let Some(gap) = omitted_gap(val, "headings", i) {
                            out.push_str(&gap);
                        }
                        let heading = h["heading"].as_str().unwrap_or("?");
                        let line = h["line"].as_u64().unwrap_or(0);
                        let end_line = h["end_line"].as_u64().unwrap_or(0);
                        let level = h["level"].as_u64().unwrap_or(1) as usize;
                        let indent = "  ".repeat(level.saturating_sub(1));
                        out.push_str(&format!("\n    {indent}{heading}  L{line}-{end_line}"));
                    }
                }
            }
        }
        "json" => {
            if let Some(schema) = val.get("schema") {
                let root_type = schema["root_type"].as_str().unwrap_or("?");
                out.push_str(&format!("\n  Root: {root_type}"));
                if let Some(keys) = schema["keys"].as_array() {
                    for (i, k) in keys.iter().enumerate() {
                        if let Some(gap) = omitted_gap(schema, "keys", i) {
                            out.push_str(&gap);
                        }
                        let path = k["path"].as_str().unwrap_or("?");
                        let typ = k["type"].as_str().unwrap_or("?");
                        let mut desc = format!("\n    {path}: {typ}");
                        if let Some(count) = k["count"].as_u64() {
                            desc.push_str(&format!(" ({count} items)"));
                        }
                        out.push_str(&desc);
                    }
                }
                if let Some(count) = schema["count"].as_u64() {
                    out.push_str(&format!("\n    Count: {count}"));
                    if let Some(elem) = schema["element_type"].as_str() {
                        out.push_str(&format!(" (element type: {elem})"));
                    }
                }
            }
        }
        "toml" => {
            if let Some(sections) = val["sections"].as_array() {
                out.push_str("\n  Sections:");
                for (i, s) in sections.iter().enumerate() {
                    if let Some(gap) = omitted_gap(val, "sections", i) {
                        out.push_str(&gap);
                    }
                    let key = s["key"].as_str().unwrap_or("?");
                    let line = s["line"].as_u64().unwrap_or(0);
                    let end = s["end_line"].as_u64().unwrap_or(0);
                    out.push_str(&format!("\n    {key}  L{line}-{end}"));
                }
            }
            if let Some(keys) = val["keys"].as_array() {
                out.push_str("\n  Keys:");
                for (i, k) in keys.iter().enumerate() {
                    if let Some(gap) = omitted_gap(val, "keys", i) {
                        out.push_str(&gap);
                    }
                    let key = k["key"].as_str().unwrap_or("?");
                    let line = k["line"].as_u64().unwrap_or(0);
                    out.push_str(&format!("\n    {key}  L{line}"));
                }
            }
        }
        "yaml" => {
            if let Some(sections) = val["sections"].as_array() {
                out.push_str("\n  Sections:");
                for (i, s) in sections.iter().enumerate() {
                    if let Some(gap) = omitted_gap(val, "sections", i) {
                        out.push_str(&gap);
                    }
                    let key = s["key"].as_str().unwrap_or("?");
                    let line = s["line"].as_u64().unwrap_or(0);
                    let end = s["end_line"].as_u64().unwrap_or(0);
                    out.push_str(&format!("\n    {key}  L{line}-{end}"));
                }
            }
        }
        // Residual: .xml, .ini, .env, .lock, .cfg (JSON/YAML/TOML have dedicated branches)
        "config" => {
            if let Some(preview) = val["preview"].as_str() {
                out.push_str("\n  Preview:");
                for line in preview.lines() {
                    out.push_str(&format!("\n    {line}"));
                }
            }
        }
        "generic" => {
            if let Some(head) = val["head"].as_str() {
                out.push_str("\n  Head:");
                for line in head.lines() {
                    out.push_str(&format!("\n    {line}"));
                }
            }
            if let Some(tail) = val["tail"].as_str() {
                out.push_str("\n  Tail:");
                for line in tail.lines() {
                    out.push_str(&format!("\n    {line}"));
                }
            }
        }
        _ => {}
    }

    // Incompleteness note, buffer handle and hint go BELOW THE HEADER, not after the
    // outline. All three were tail-placed, and the outline they trailed is unbounded — a
    // 300-symbol file pushes them past the compaction cut, so the one response that most
    // needs to say "this is a summary, here is the handle to get the rest" lost both the
    // statement and the handle. This bug's own fix note called that sequencing out.
    // See `format::overflow_head`.
    let mut head_extra = overflow_head(val);
    if let Some(file_id) = val["file_id"].as_str() {
        head_extra.push_str(&format!("  Buffer: {file_id}\n"));
    }
    if let Some(hint) = val["hint"].as_str() {
        head_extra.push_str(&format!("  {hint}\n"));
    }

    insert_below_header(out, &head_extra)
}

/// Recursively flatten a symbol tree into a single Vec of references.
fn flatten_symbols<'a>(
    syms: &'a [crate::lsp::SymbolInfo],
    out: &mut Vec<&'a crate::lsp::SymbolInfo>,
) {
    for sym in syms {
        out.push(sym);
        flatten_symbols(&sym.children, out);
    }
}

/// Return the `name_path` and 0-indexed line span of every symbol whose body
/// overlaps (inclusive) the read range: symbol contains range, range contains
/// symbol, or they share a boundary.
///
/// `start` and `end` are 1-indexed (as received from tool input).
/// `SymbolInfo.start_line` / `end_line` are 0-indexed and are returned as such —
/// the caller uses them only to compare *extents* (to decide which escape the
/// refusal hint should lead with), never to render a line number.
/// Returns an empty Vec on parse error (fail open).
fn find_symbols_for_range(
    text: &str,
    resolved: &std::path::Path,
    start: u64,
    end: u64,
) -> Vec<(String, u32, u32)> {
    let syms = match crate::ast::extract_symbols_from_text(text, resolved) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let mut flat = Vec::new();
    flatten_symbols(&syms, &mut flat);

    let s0 = (start.saturating_sub(1)) as u32;
    let e0 = (end.saturating_sub(1)) as u32;

    flat.into_iter()
        .filter(|sym| {
            // symbol body contains read range
            (sym.start_line <= s0 && e0 <= sym.end_line)
            // read range contains symbol body
            || (s0 <= sym.start_line && sym.end_line <= e0)
        })
        .map(|sym| (sym.name_path.clone(), sym.start_line, sym.end_line))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use crate::tools::ToolContext;
    use serde_json::json;

    async fn test_ctx() -> ToolContext {
        ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: LspManager::new_arc(),
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
    /// Phase 3 regression (regime 3): a read tool pinned to workspace A via
    /// `ToolContext.workspace_override` must read A's files even when the
    /// session default is workspace B. Today `read_file` resolves the default
    /// project, so it reads B — this test is RED until Phase 3 wires the
    /// override into path resolution. The single mutation it catches is
    /// "ignore workspace_override," which IS the regime-3 last-writer-wins bug.
    /// Contract: pinned(A) ⇒ reads A, regardless of default B.
    #[tokio::test]
    async fn read_file_honors_workspace_override_pin() {
        use tempfile::tempdir;

        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        std::fs::write(dir_a.path().join("marker.txt"), "ALPHA-CONTENT").unwrap();
        std::fs::write(dir_b.path().join("marker.txt"), "BETA-CONTENT").unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Default (unpinned) project is B.
        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();
        let mut ctx = test_ctx().await;
        ctx.agent = agent;
        // Pin THIS request to workspace A.
        ctx.workspace_override = Some(root_a);

        let result = ReadFile
            .call(json!({ "path": "marker.txt" }), &ctx)
            .await
            .unwrap();
        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");

        assert!(
            body.contains("ALPHA-CONTENT"),
            "pinned read should resolve workspace A (ALPHA), got: {result}"
        );
        assert!(
            !body.contains("BETA-CONTENT"),
            "pinned read must NOT leak the default workspace B (BETA), got: {result}"
        );
    }

    /// The SILENT half of the workspace-clobber bug. `Agent::activate` clears the
    /// registry and reassigns `default_workspace_root` for everything sharing the
    /// session's process, so a subagent activating a foreign project leaves the
    /// parent pointed there. The parent's next `read_file` on a file tracked in ITS
    /// project returns "file not found" — true of the tree actually searched, and
    /// indistinguishable from a genuine absence by a caller who never learned the
    /// root moved.
    ///
    /// Measured 2026-08-26 (occurrence 4): `read_file` / `grep` / `symbols` all
    /// returned confident negatives for a 131 KB tracked source file, and
    /// `workspace(action="status")` was the only call that surfaced the real root.
    /// The read-only form of the same clobber already got a diagnosis hint in
    /// `check_tool_access`; this form had none, because nothing named the tree.
    ///
    /// So the message must name the ROOT it searched, not only the relative path it
    /// was handed. `resolved` is already in scope at the failure site and was simply
    /// never used in the text.
    ///
    /// docs/issues/archive/2026-08-26-workspace-read-only-flips-mid-session.md
    #[tokio::test]
    async fn file_not_found_names_the_root_it_searched() {
        use tempfile::tempdir;

        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        // The file exists ONLY in A. B is the root the session is (wrongly) on.
        std::fs::write(dir_a.path().join("marker.txt"), "ALPHA-CONTENT").unwrap();
        let root_b = std::fs::canonicalize(dir_b.path()).unwrap();

        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();
        let mut ctx = test_ctx().await;
        ctx.agent = agent;

        let err = ReadFile
            .call(json!({ "path": "marker.txt" }), &ctx)
            .await
            .expect_err("marker.txt does not exist under B");
        let msg = format!("{err:#}");

        assert!(
            msg.contains("file not found:"),
            "usage classification keys on this exact prefix \
                 (src/usage/db.rs normalize_err_family): {msg}"
        );
        assert!(
            msg.contains(&root_b.display().to_string()),
            "the error must name the ROOT it searched, so a caller can tell \
                 'this file is absent' from 'you are pointed at the wrong tree': {msg}"
        );
    }

    /// Phase 3 regression (regime 3, concurrent form): N tasks share ONE Agent,
    /// each pins a distinct workspace and reads its marker file concurrently on
    /// a multi-thread runtime. Each must read ITS OWN workspace with zero
    /// cross-bleed — proving per-request resolution survives interleaved/parallel
    /// activation, which is exactly the original last-writer-wins-on-the-global-
    /// slot bug. A shared-state regression flips this red.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn read_file_concurrent_pins_no_cross_workspace_bleed() {
        use tempfile::tempdir;

        const N: usize = 5;
        let mut dirs = Vec::new();
        let mut roots = Vec::new();
        for i in 0..N {
            let d = tempdir().unwrap();
            std::fs::create_dir_all(d.path().join(".codescout")).unwrap();
            std::fs::write(d.path().join("marker.txt"), format!("WS-{i}")).unwrap();
            roots.push(std::fs::canonicalize(d.path()).unwrap());
            dirs.push(d); // keep tempdirs alive for the duration
        }

        // Default (unpinned) project is workspace 0; tasks pin 0..N concurrently.
        let agent = Agent::new(Some(dirs[0].path().to_path_buf()))
            .await
            .unwrap();

        let mut handles = Vec::new();
        for (i, root_i) in roots.iter().cloned().enumerate() {
            let agent = agent.clone();
            handles.push(tokio::spawn(async move {
                let mut ctx = test_ctx().await;
                ctx.agent = agent;
                ctx.workspace_override = Some(root_i);
                let result = ReadFile
                    .call(json!({ "path": "marker.txt" }), &ctx)
                    .await
                    .unwrap();
                let body = result
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                (i, body)
            }));
        }

        for h in handles {
            let (i, body) = h.await.unwrap();
            assert!(
                body.contains(&format!("WS-{i}")),
                "task {i} pinned to its own workspace must read WS-{i}, got: {body:?}"
            );
            for j in 0..N {
                if j != i {
                    assert!(
                        !body.contains(&format!("WS-{j}")),
                        "task {i} leaked workspace {j}'s content (regime-3 bleed): {body:?}"
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn read_file_buffer_midpoint_returns_content() {
        // Probe bug 2026-05-09-read-file-buffer-midpoint-empty.
        // Seed a buffer with 200 plain lines; read midpoint range.
        let lines: Vec<String> = (1..=200).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", content);

        let tool = ReadFile;
        let result = tool
            .call(
                json!({ "path": buf_id, "start_line": 150, "end_line": 160 }),
                &ctx,
            )
            .await
            .unwrap();

        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("line 150") && body.contains("line 160"),
            "buffer midpoint read should include lines 150-160, got: {body:?} from {result}"
        );
    }
    /// A `@tool_*` payload whose JSON carries a multi-line string VALUE — the only shape
    /// where `grep`'s expansion and `read_file`'s raw numbering can diverge. Flatten it and
    /// every test below passes against the UNFIXED code.
    fn multiline_tool_payload() -> String {
        let filler: Vec<String> = (1..=40).map(|i| format!("    let v{i} = {i};")).collect();
        json!({
            "symbols": [{
                "name": "demo",
                "body": format!(
                    "fn demo() {{\n{}\n    MARKER_DEEP_INSIDE\n}}",
                    filler.join("\n")
                ),
            }]
        })
        .to_string()
    }

    /// First `"line"` value anywhere in a grep result, so the assertion does not depend on
    /// whether grep grouped into `file_groups` (context_lines == 0) or returned a flat
    /// `matches` array.
    fn first_line_number(v: &serde_json::Value) -> Option<u64> {
        match v {
            serde_json::Value::Object(m) => m
                .get("line")
                .and_then(|x| x.as_u64())
                .or_else(|| m.values().find_map(first_line_number)),
            serde_json::Value::Array(a) => a.iter().find_map(first_line_number),
            _ => None,
        }
    }

    /// `grep` and `read_file` must address ONE `@tool_*` handle in ONE coordinate space.
    ///
    /// `grep_in_buffer` materializes escaped newlines before matching — the fix for
    /// `docs/issues/archive/2026-07-01-grep-buffer-multiline-string-value-collapses.md`,
    /// and correct for matching — so it numbers the EXPANDED text. `read_from_buffer`
    /// numbered the raw pretty-printed JSON. One handle, two line spaces, and no field said
    /// so: a citation BELOW the raw line count resolved to a different line and SUCCEEDED,
    /// one above it returned `0 lines`. The silent half is the expensive one.
    ///
    /// LOAD-BEARING: the `\n` is inside a JSON string value (see `multiline_tool_payload`).
    /// The flat-buffer control below is INERT for this defect and pinned as such — the two
    /// move together or the pair stops discriminating.
    ///
    /// BUG docs/issues/archive/2026-09-15-grep-and-read-file-number-one-buffer-handle-differently.md
    #[tokio::test]
    async fn grep_and_read_file_number_one_tool_handle_identically() {
        let ctx = test_ctx().await;
        let buf_id = ctx
            .output_buffer
            .store_tool("symbols", multiline_tool_payload());

        let hit = crate::tools::grep::Grep
            .call(
                json!({ "pattern": "MARKER_DEEP_INSIDE", "path": buf_id.clone() }),
                &ctx,
            )
            .await
            .unwrap();
        let cited = first_line_number(&hit)
            .unwrap_or_else(|| panic!("grep reported no line for the marker: {hit}"));

        let back = ReadFile
            .call(
                json!({ "path": buf_id.clone(), "start_line": cited, "end_line": cited }),
                &ctx,
            )
            .await
            .unwrap();
        let body = back.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("MARKER_DEEP_INSIDE"),
            "grep cited line {cited} of {buf_id}; read_file must resolve the SAME line there, \
             got {body:?} from {back}"
        );
    }

    /// CONTROL, and **inert for the coordinate split by construction** — do not credit it
    /// with covering that defect. A `@tool_*` payload with no multi-line string value numbers
    /// identically before and after the fix, so this is green either way. Its job is the
    /// other direction: if it ever reds, the fix broke ordinary flat-buffer addressing.
    #[tokio::test]
    async fn a_flat_tool_buffer_numbers_identically_and_witnesses_nothing() {
        let ctx = test_ctx().await;
        let flat = json!({ "rows": (1..=30).map(|i| json!({ "n": i })).collect::<Vec<_>>() });
        let buf_id = ctx.output_buffer.store_tool("rows", flat.to_string());

        let hit = crate::tools::grep::Grep
            .call(
                json!({ "pattern": "\"n\": 17", "path": buf_id.clone() }),
                &ctx,
            )
            .await
            .unwrap();
        let cited = first_line_number(&hit)
            .unwrap_or_else(|| panic!("grep reported no line on the flat buffer: {hit}"));

        let back = ReadFile
            .call(
                json!({ "path": buf_id.clone(), "start_line": cited, "end_line": cited }),
                &ctx,
            )
            .await
            .unwrap();
        let body = back.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("17"),
            "a flat buffer must round-trip a grep citation, got {body:?} from {back}"
        );
    }
    /// `json_path` must read the UN-EXPANDED text, because it RE-PARSES what it is handed.
    ///
    /// `line_addressable_text` materializes escaped newlines, which puts a bare newline
    /// inside a JSON string literal and makes the text invalid JSON. Sharing one `text`
    /// between the two consumers — the obvious tidy-up, since they now sit a few lines
    /// apart and look like duplicates — silently breaks every `json_path` read on a buffer
    /// holding a multi-line value.
    ///
    /// LOAD-BEARING: `multiline_tool_payload`'s `body` contains `\n`.
    /// `read_file_buffer_json_path_array_element_returns_value` above uses a single-line
    /// body and stays GREEN under that exact mutation — it is not a witness for this and
    /// must not be credited as one.
    #[tokio::test]
    async fn json_path_still_resolves_when_a_sibling_value_is_multi_line() {
        let ctx = test_ctx().await;
        let buf_id = ctx
            .output_buffer
            .store_tool("symbols", multiline_tool_payload());

        let result = ReadFile
            .call(
                json!({ "path": buf_id, "json_path": "$.symbols[0].name" }),
                &ctx,
            )
            .await
            .expect("json_path must not fail on a buffer whose sibling value is multi-line");

        let rendered = format!("{result}");
        assert!(
            rendered.contains("demo"),
            "json_path must resolve the name beside a multi-line body, got {rendered}"
        );
    }
    /// `@cmd_*` / `@file_*` buffers are RAW TEXT and must be served byte-for-byte, even when
    /// their bytes happen to parse as JSON.
    ///
    /// `line_addressable_text`'s handle-kind guard is the only thing preventing otherwise: a
    /// `curl` or `gh api --json` capture is valid JSON, and without the guard it would be
    /// pretty-printed AND newline-expanded — so `read_file` would serve a reformatted
    /// document instead of what the command actually wrote, and the `sed -n 'N,Mp' @cmd_x`
    /// callers that `read_file_err_handle_reads_stderr_not_stdout` protects would address
    /// different lines.
    ///
    /// LOAD-BEARING: the content must be VALID JSON *and* carry an escaped `\n`. Plain text
    /// fails `from_str` and falls through to the same answer, so a non-JSON fixture leaves
    /// the guard untested — which is exactly how it came to survive its first mutation.
    #[tokio::test]
    async fn a_cmd_buffer_that_happens_to_be_json_is_served_raw_not_reformatted() {
        let ctx = test_ctx().await;
        let raw = r#"{"a":"one\ntwo","b":1}"#;
        let buf_id = ctx
            .output_buffer
            .store("curl".to_string(), raw.to_string(), String::new(), 0);

        let result = ReadFile
            .call(json!({ "path": buf_id }), &ctx)
            .await
            .unwrap();
        let content = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(
            content, raw,
            "a @cmd_* buffer must be served byte-for-byte, never pretty-printed or expanded"
        );
    }

    /// An out-of-range buffer read must NAME THE SCOPE it examined rather than answer a bare
    /// `0 lines`. On a 157-line buffer that zero is not merely uninformative, it is false in
    /// the reader's units — the non-empty branch spends the same two words on the buffer's
    /// TOTAL, so one phrase denotes two quantities.
    ///
    /// `docs/adrs/2026-08-27-negative-results-name-their-scope.md`: name the scope when the
    /// zero is suspicious. The total was already in the payload AND already in a local on the
    /// line above the early return.
    #[test]
    fn an_out_of_range_read_names_the_total_instead_of_a_bare_zero() {
        let rendered = format_read_file(&json!({ "content": "", "total_lines": 157 }));
        assert!(
            rendered.contains("157"),
            "an empty slice of a 157-line target must name the 157; got {rendered:?}"
        );
    }

    /// The complement, so the fix above cannot be satisfied by unconditionally printing a
    /// total: a genuinely empty target has no scope worth naming and must stay quiet. Without
    /// this, "always add the number" passes the sibling and makes every empty file noisy.
    #[test]
    fn a_genuinely_empty_target_still_reads_as_a_plain_zero() {
        let rendered = format_read_file(&json!({ "content": "", "total_lines": 0 }));
        assert_eq!(
            rendered.trim(),
            "0 lines",
            "an empty target names no scope; got {rendered:?}"
        );
    }

    /// A `.err` handle must be READ from stderr, with a stdout control both ways.
    ///
    /// The shipped defect served 4001 lines of stdout for a `.err` handle with no error, so
    /// "the stderr token is present" alone would not discriminate — a fix that concatenated
    /// both streams satisfies it and still misleads. Each stream carries a token the other
    /// does not, and the absence is asserted as well as the presence.
    ///
    /// BUG docs/issues/archive/2026-09-14-read-file-and-grep-accept-a-err-handle-and-silently-answer-from-stdout.md
    #[tokio::test]
    async fn read_file_err_handle_reads_stderr_not_stdout() {
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store(
            "failing".to_string(),
            "STDOUT_ONLY_TOKEN\n".to_string(),
            "STDERR_ONLY_TOKEN\n".to_string(),
            1,
        );

        let via_err = ReadFile
            .call(json!({ "path": format!("{buf_id}.err") }), &ctx)
            .await
            .unwrap();
        let err_text = format!("{via_err:?}");
        assert!(
            err_text.contains("STDERR_ONLY_TOKEN"),
            "a .err handle must read stderr; got {err_text}"
        );
        assert!(
            !err_text.contains("STDOUT_ONLY_TOKEN"),
            "stdout must not leak through a .err handle; got {err_text}"
        );

        // The bare handle keeps its existing contract — stdout, and stdout only. Buffer line
        // numbering is stdout-relative for a bare handle and `sed -n 'N,Mp' @cmd_x` callers
        // depend on it, so this fix must not move it.
        let bare = ReadFile
            .call(json!({ "path": buf_id }), &ctx)
            .await
            .unwrap();
        let bare_text = format!("{bare:?}");
        assert!(bare_text.contains("STDOUT_ONLY_TOKEN"));
        assert!(!bare_text.contains("STDERR_ONLY_TOKEN"));
    }

    #[tokio::test]
    async fn read_file_buffer_json_path_array_element_returns_value() {
        // Probe bug 2026-05-09-read-file-json-path-array-elements.
        let content = r#"{"symbols":[{"name":"alpha","body":"fn alpha() {}"},{"name":"beta","body":"fn beta() {}"}],"context":"ok"}"#;
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("symbols", content.to_string());

        let tool = ReadFile;
        let result = tool
            .call(
                json!({ "path": buf_id, "json_path": "$.symbols[0].body" }),
                &ctx,
            )
            .await
            .unwrap();

        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("fn alpha"),
            "json_path $.symbols[0].body should return the body string, got: {result}"
        );
    }
    #[tokio::test]
    async fn read_file_toml_key_on_buffer_ref_errors_not_silently_ignored() {
        // Regression: toml_key was silently dropped for every buffer ref —
        // the caller got the whole buffer back instead of an error, masking
        // a misuse. It must fail loudly.
        let ctx = test_ctx().await;
        let buf_id = ctx
            .output_buffer
            .store_tool("cmd", "hello = 1\n".to_string());

        let err = ReadFile
            .call(json!({ "path": buf_id, "toml_key": "hello" }), &ctx)
            .await
            .expect_err("toml_key on a buffer ref must error, not be silently ignored");
        let msg = err.to_string();
        assert!(
            msg.contains("toml_key"),
            "error must name the offending param; got: {msg}"
        );
    }
    #[tokio::test]
    async fn read_file_toml_key_works_on_lock_file() {
        // Cargo.lock is TOML; toml_key must work on it even though `.lock`
        // is classified as Config by detect_file_type.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cargo.lock");
        std::fs::write(
            &path,
            "version = 3\n\n[[package]]\nname = \"foo\"\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "toml_key": "version" }),
                &ctx,
            )
            .await
            .expect("toml_key must work on a Cargo.lock (TOML) file");
        assert_eq!(
            value["format"], "toml",
            "expected toml format, got: {value}"
        );
        assert!(
            value.get("content").is_some(),
            "expected content for the key, got: {value}"
        );
    }
    #[tokio::test]
    async fn read_file_json_path_hint_points_at_content() {
        // ReadFile's buffered results carry `content` (not `field`); the
        // json_path hint must point at $.content, not the generic default.
        let with_content = json!({ "content": "hello", "total_lines": 1 });
        assert_eq!(ReadFile.json_path_hint(&with_content), "$.content");
        // Falls back to the generic default when there is no content field.
        let without = json!({ "file_id": "@file_x", "total_lines": 9 });
        assert_eq!(ReadFile.json_path_hint(&without), "$.field");
    }
    /// ReadFile's hint, tested by FOLLOWING it for each buffered payload shape. A `content` string
    /// hints itself (and wins even beside an array); an array of records with no `content`
    /// hints the array. That shape is synthetic here, a pin on the fallback branch alone:
    /// whole-file outlines are bounded inline now, and the live payload that takes this branch
    /// is the missed-heading list, followed end to end by
    /// `an_overflowing_missed_heading_list_hints_a_route_that_returns_data`. A payload with
    /// neither gets the shared default's placeholder, pinned in `core/types.rs`, which is not a
    /// route and has no row here. This replaces a test that compared the hint to a string.
    #[tokio::test]
    async fn read_file_hints_lead_to_the_data_for_each_payload_shape() {
        let ctx = test_ctx().await;
        let outline = json!({
            "file_id": "@file_x",
            "total_lines": 6206,
            "symbols": [{ "name": "sym_alpha" }, { "name": "sym_beta" }, { "name": "sym_gamma" }],
        });
        let content_and_array = json!({
            "content": "the file text, line one",
            "symbols": [{ "name": "not_this" }],
        });

        let jp = ReadFile.json_path_hint(&outline);
        let got = crate::tools::hint_probe::follow_path_on(&outline, &jp, &ctx)
            .await
            .unwrap_or_else(|e| panic!("outline route {jp:?} failed: {e}"));
        assert_eq!(got["value_type"], "array", "{jp:?}: {got}");
        let rendered = got.to_string();
        assert!(
            rendered.contains("sym_alpha") && rendered.contains("sym_gamma"),
            "{jp:?} must return the symbols, first to last: {rendered}"
        );

        let jp = ReadFile.json_path_hint(&content_and_array);
        let got = crate::tools::hint_probe::follow_path_on(&content_and_array, &jp, &ctx)
            .await
            .unwrap_or_else(|e| panic!("content route {jp:?} failed: {e}"));
        assert!(
            got.to_string().contains("the file text, line one")
                && !got.to_string().contains("not_this"),
            "a content string must outrank an array beside it: {jp:?} gave {got}"
        );
    }

    /// REACH and REMEDY through the real `call_content`: a heading that is not in a markdown file
    /// answers `{ok:false, error, headings:[{h,l}..], hint}`, and a file with enough headings made
    /// that list the whole payload. It used to spill under `@tool_*` (the one `read_file` payload
    /// that did so WITHOUT a `content` string, which this test once pinned through
    /// `ReadFile::json_path_hint`). The list is now bounded at the source by `fit_envelope`: its
    /// MIDDLE is cut by the excess, the response stays inline with no handle, and the hint names
    /// the omitted lines as a `read_file` range of the file the caller asked about. That route is
    /// followed here and must come back as data holding a heading from the cut middle.
    #[tokio::test]
    async fn an_overflowing_missed_heading_list_hints_a_route_that_returns_data() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let doc: String = (0..1500)
            .map(|i| format!("## Section number {i:04} with a long title\n\nbody {i}\n\n"))
            .collect();
        std::fs::write(dir.path().join("notes.md"), doc).unwrap();
        let mut ctx = test_ctx().await;
        ctx.agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let input = json!({ "path": "notes.md", "heading": "## no such heading" });
        let content = ReadFile.call_content(input.clone(), &ctx).await.unwrap();
        let text = content[0]
            .as_text()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        assert!(!text.contains("@tool_"), "the list spilled: {text:.300}");
        let result = ReadFile.call(input, &ctx).await.unwrap();
        assert!(
            !crate::tools::exceeds_inline_limit(&result.to_string()),
            "{} B is over the inline limit",
            result.to_string().len()
        );
        for name in ["Section number 0000", "Section number 1499"] {
            assert!(text.contains(name), "both ends of the list survive: {name}");
        }
        let hint = result["hint"].as_str().unwrap();
        assert!(hint.contains("entries omitted"), "{hint}");
        let route =
            regex::Regex::new(r#"read_file\(path="notes\.md", start_line=(\d+), end_line=(\d+)\)"#)
                .unwrap()
                .captures(hint)
                .unwrap_or_else(|| panic!("the hint names no range of the file: {hint}"));
        let followed = ReadFile
            .call(
                json!({
                    "path": "notes.md",
                    "start_line": route[1].parse::<u64>().unwrap(),
                    "end_line": route[2].parse::<u64>().unwrap(),
                }),
                &ctx,
            )
            .await
            .unwrap_or_else(|e| panic!("following the hinted route failed: {e}"));
        // The first omitted heading is entry `after` of the list (entry i is `Section number i`).
        let first_cut = result["headings_omitted"]["after"].as_u64().unwrap();
        let first_cut = format!("Section number {first_cut:04}");
        assert!(
            followed["content"].as_str().unwrap().contains(&first_cut),
            "the route must return the cut middle, from {first_cut:?}: {followed:.300}"
        );
    }

    /// D4b, by FOLLOWING every route the hint offers on a real buffer of each ref kind.
    /// `over_budget_line_hint` is only called from `read_from_buffer`, so its path is always a
    /// buffer ref. Its non-`@tool_` branch used to advise `json_path` on `@cmd_*`/`@file_*`
    /// refs, which `read_file` refuses; following that route fails with
    /// `json_path is only supported on @tool_* refs`. So this runs each `json_path` route through
    /// `read_file` and each `run_command` route through `run_command`, and every one must work:
    /// a `json_path` offered on a refused kind, or a grep that cannot find the needle, is a red.
    /// A literal `$.<field>` is a template and is filled with `stdout`, the key of a
    /// `run_command` envelope; a literal `$.field` is not filled and would fail when followed.
    #[tokio::test]
    async fn the_over_budget_hint_never_names_a_route_the_ref_refuses() {
        use crate::tools::hint_probe::{commands_in, json_paths_in};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let mut ctx = test_ctx().await;
        ctx.agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let wide = format!(
            "{}needle-hit,tail, {}",
            "filler, ".repeat(1_500),
            "filler, ".repeat(1_500)
        );
        let cmd = ctx
            .output_buffer
            .store("wide".into(), wide.clone(), String::new(), 0);
        // A `@file_*` handle in production is a slice or extraction of another buffer, stored
        // under a derived name (`<ref>[1-1]`), not under the path of a file that is not there.
        let file = ctx
            .output_buffer
            .store_file(format!("{cmd}[1-1]"), wide.clone());
        let tool = ctx.output_buffer.store_tool(
            "run_command",
            json!({ "exit_code": 0, "stdout": wide }).to_string(),
        );

        for (kind, handle) in [("@cmd_", cmd), ("@file_", file), ("@tool_", tool)] {
            assert!(
                handle.starts_with(kind),
                "fixture: {handle} is not a {kind} ref"
            );
            let hint = over_budget_line_hint(&handle);

            for jp in json_paths_in(&hint, "stdout") {
                crate::tools::read_file::ReadFile
                    .call(json!({ "path": handle, "json_path": jp }), &ctx)
                    .await
                    .unwrap_or_else(|e| {
                        panic!(
                            "the hint for {kind} offers json_path {jp:?}, which fails: {e}\n{hint}"
                        )
                    });
            }
            let commands = commands_in(&hint, "needle");
            assert!(
                !commands.is_empty(),
                "the hint for {kind} offers no run_command route: {hint}"
            );
            for command in commands {
                let out = crate::tools::run_command::RunCommand
                    .call(json!({ "command": command }), &ctx)
                    .await
                    .unwrap_or_else(|e| panic!("the {kind} route {command:?} failed: {e}"));
                assert!(
                    out.to_string().contains("needle-hit"),
                    "the {kind} route {command:?} must return the match, got: {out:.300}"
                );
            }
        }
    }

    /// REACH and REMEDY for D4b, through the real `call_content`. The hint was attached to the
    /// result's JSON but `format_read_file_body` never rendered it, so the agent saw only
    /// `…[truncated: this line is wider than the inline budget]` and no route off it. This reads
    /// a wide single line from a `@cmd_*` buffer, takes the `run_command(...)` route out of
    /// what the AGENT sees, and runs it: it must return the part of the line asked for.
    #[tokio::test]
    async fn a_clamped_wide_line_shows_the_agent_a_route_that_returns_the_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let mut ctx = test_ctx().await;
        ctx.agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        // One ~40 KB line with the needle in the MIDDLE, commas on both sides, and a distinct
        // sentinel after it. At the END of the line a greedy `.*` returns the same short match
        // as `[^,]*`, so the route's bound was untested; here `.*` returns the needle AND
        // everything after it, sentinel included, and the output busts the inline budget.
        let line = format!(
            "{}needle-token-4242,AFTER-SENTINEL,{}",
            "filler-record, ".repeat(1_300),
            "filler-record, ".repeat(1_300)
        );
        let id = ctx
            .output_buffer
            .store("wide".into(), line, String::new(), 0);

        let content = ReadFile
            .call_content(json!({ "path": id }), &ctx)
            .await
            .unwrap();
        let text = crate::tools::hint_probe::primary_text(&content);
        assert!(
            text.contains("wider than the inline budget"),
            "precondition: the read must clamp the line: {text:.200}"
        );

        // The route, as the agent reads it: `run_command("<cmd>")` with a PATTERN placeholder.
        let start = text
            .find("run_command(\"")
            .unwrap_or_else(|| panic!("the clamped read shows the agent no route: {text:.400}"))
            + "run_command(\"".len();
        let end = text[start..].find("\")").expect("an unterminated route") + start;
        let command = text[start..end].replace("PATTERN", "needle-token");
        let out = crate::tools::run_command::RunCommand
            .call(json!({ "command": command }), &ctx)
            .await
            .unwrap_or_else(|e| panic!("the shown route {command:?} failed: {e}"));
        let rendered = out.to_string();
        assert!(
            rendered.contains("needle-token-4242"),
            "the shown route {command:?} must return the match, got: {rendered:.300}"
        );
        assert!(
            !rendered.contains("AFTER-SENTINEL"),
            "the route must return only the match, not the rest of the wide line: {rendered:.300}"
        );
    }

    #[tokio::test]
    async fn read_file_json_path_on_non_tool_buffer_ref_errors() {
        // json_path is only meaningful for @tool_* JSON refs; on @cmd_/@file_
        // refs it was silently ignored. It must error instead.
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store(
            "echo hi".to_string(),
            "{\"a\": 1}".to_string(),
            String::new(),
            0,
        );

        let err = ReadFile
            .call(json!({ "path": buf_id, "json_path": "$.a" }), &ctx)
            .await
            .expect_err("json_path on a @cmd_ ref must error, not be silently ignored");
        let msg = err.to_string();
        assert!(
            msg.contains("json_path"),
            "error must name the offending param; got: {msg}"
        );
    }

    #[tokio::test]
    async fn read_file_call_content_returns_line_numbered_text_not_json() {
        // Regression: small read_file results used to serialize as pretty JSON via
        // the default Tool::call_content path because ReadFile did not declare
        // OutputForm::Text. Now both axes reach format_read_file, so sub-threshold
        // reads come through as raw text. Line-number prefixes were removed
        // (docs/issues/archive/2026-05-21-read-file-slice-relative-line-numbers.md), so the
        // content is shown verbatim with no `N| ` prefixes.
        let content = "alpha\nbeta\ngamma".to_string();
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", content);

        let blocks = ReadFile
            .call_content(
                json!({ "path": buf_id, "start_line": 1, "end_line": 3 }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(blocks.len(), 1, "expected exactly 1 content block");
        let text = blocks[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            text.contains("alpha\nbeta\ngamma"),
            "expected raw text content, got: {text}"
        );
        assert!(
            !text.contains("1| ") && !text.contains("3| "),
            "line-number prefixes must be dropped, got: {text}"
        );
        assert!(
            !text.trim_start().starts_with('{'),
            "read_file output must be text, not JSON, got: {text}"
        );
    }

    /// `read_file(path, force=true)` on an oversized whole file returns an outline and
    /// zero content lines. That is correct — `force` scopes to a line range in both the
    /// input schema and Iron Law 1, and letting it defeat the size budget would defeat
    /// progressive disclosure. What was wrong is that the parameter was accepted and
    /// dropped in silence, so the caller had no way to learn that the thing they asked
    /// for is not a thing this path does.
    ///
    /// Measured 2026-08-17 at `021c130d` before the fix: `read_file("src/librarian/
    /// classify.rs", force=true)` on a 10,559-byte file returned `showing 0 of 378`
    /// with a hint that never mentioned `force`.
    /// `docs/issues/archive/2026-08-15-read-file-force-ignored-on-full-reads.md`.
    #[test]
    fn outline_hint_says_force_did_not_apply_when_forced() {
        let hint = super::outline_hint("@file_abc", true, true);
        assert!(
            hint.contains("force=true"),
            "a discarded force=true must be named in the hint; got: {hint}"
        );
        assert!(
            hint.contains("start_line"),
            "naming the drop is only half — the hint must say what DOES work; got: {hint}"
        );
    }

    /// The complement, and it is the half that keeps the fix from becoming noise: a
    /// caller who never passed `force` must not be told anything about it. A note that
    /// fires unconditionally is not a signal, it is boilerplate the reader learns to skip.
    #[test]
    fn outline_hint_stays_silent_about_force_when_not_forced() {
        for is_source in [true, false] {
            let hint = super::outline_hint("@file_abc", is_source, false);
            assert!(
                !hint.contains("force"),
                "unforced read mentions force (is_source={is_source}); got: {hint}"
            );
        }
    }

    /// The runtime note only reaches a caller who already spent the call. The schema is
    /// the surface they read BEFORE spending it, so it has to carry the same scope —
    /// "read the raw line range" describes what `force` does and leaves what it does not
    /// do to inference, which is how it came to be read as a general escape hatch.
    #[test]
    fn force_schema_says_what_a_whole_file_read_does() {
        use crate::tools::core::Tool;
        let schema = ReadFile.input_schema();
        let desc = schema["properties"]["force"]["description"]
            .as_str()
            .expect("force is a declared property with a description");
        assert!(
            desc.contains("whole-file"),
            "the force description must state the whole-file behaviour, not only the \
             line-range one; got: {desc}"
        );
    }

    #[tokio::test]
    async fn read_file_buffer_start_line_alone_defaults_50_line_window() {
        // I-6: start_line alone should default end_line to start+49 (50-line window),
        // not be silently ignored (buffer) or rejected (real file).
        let lines: Vec<String> = (1..=200).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", content);

        let tool = ReadFile;
        let result = tool
            .call(json!({ "path": buf_id, "start_line": 100 }), &ctx)
            .await
            .unwrap();

        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("line 100") && body.contains("line 149"),
            "start_line alone should yield a 50-line window 100..=149, got: {body:?}"
        );
        assert!(
            !body.contains("line 150"),
            "window should stop at start+49 (line 149), got: {body:?}"
        );
        assert!(
            !body.contains("line 99"),
            "window should start at start_line (line 100), got: {body:?}"
        );
    }

    /// Bug 2026-08-25-run-command-nested-buffer-recursion: an oversized
    /// mid-range slice reported `shown_lines` in the ORIGINAL buffer's frame
    /// but emitted `next` in the freshly-minted `@file_*` slice's own 1-based
    /// frame. The two differ by `start - 1`, so following `next` re-read
    /// already-seen lines and minted yet another handle every time — the
    /// "chain that never converges" in the report.
    ///
    /// The contract is pinned by `format_read_file_auto_chunked_mid_file`:
    /// `next.start_line == shown_lines[1] + 1`, both in one frame.
    #[tokio::test]
    async fn read_file_buffer_oversized_slice_next_continues_from_shown_lines() {
        let lines: Vec<String> = (1..=40)
            .map(|i| format!("line {i:04} {}", "x".repeat(900)))
            .collect();
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", lines.join("\n"));

        let result = ReadFile
            .call(
                json!({ "path": &buf_id, "start_line": 13, "end_line": 24 }),
                &ctx,
            )
            .await
            .unwrap();

        let shown = result["shown_lines"]
            .as_array()
            .unwrap_or_else(|| panic!("oversized slice must paginate, got: {result}"));
        assert_eq!(
            shown[0].as_u64().unwrap(),
            13,
            "shown_lines must start at the requested line, got: {result}"
        );
        let shown_end = shown[1].as_u64().unwrap();
        let next = result["next"]
            .as_str()
            .unwrap_or_else(|| panic!("an incomplete read must offer next, got: {result}"));
        assert!(
            next.contains(&format!("start_line={}", shown_end + 1)),
            "next must resume at shown_lines[1] + 1 = {}, got: {next}",
            shown_end + 1
        );
        assert!(
            next.contains(&buf_id),
            "next must address the original buffer {buf_id}, not a fresh handle: {next}"
        );
        assert_eq!(
            result["total_lines"].as_u64().unwrap(),
            40,
            "total_lines must be the buffer's total, so shown_lines reads against it: {result}"
        );
    }

    /// The same defect on the real-file path (`read_with_line_range`), which
    /// carries a byte-identical copy of the oversized-slice block.
    #[tokio::test]
    async fn read_file_oversized_range_next_continues_from_shown_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        let lines: Vec<String> = (1..=40)
            .map(|i| format!("line {i:04} {}", "x".repeat(900)))
            .collect();
        std::fs::write(&path, lines.join("\n")).unwrap();
        let ctx = test_ctx().await;

        let result = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "start_line": 13, "end_line": 24 }),
                &ctx,
            )
            .await
            .unwrap();

        let shown = result["shown_lines"]
            .as_array()
            .unwrap_or_else(|| panic!("oversized range must paginate, got: {result}"));
        assert_eq!(
            shown[0].as_u64().unwrap(),
            13,
            "shown_lines must start at the requested line, got: {result}"
        );
        let shown_end = shown[1].as_u64().unwrap();
        let next = result["next"]
            .as_str()
            .unwrap_or_else(|| panic!("an incomplete read must offer next, got: {result}"));
        assert!(
            next.contains(&format!("start_line={}", shown_end + 1)),
            "next must resume at shown_lines[1] + 1 = {}, got: {next}",
            shown_end + 1
        );
        assert_eq!(
            result["total_lines"].as_u64().unwrap(),
            40,
            "total_lines must be the file's total, so shown_lines reads against it: {result}"
        );
    }
    // ---- the whole-file summary must fit the inline budget and carry ONE handle ----
    //
    // `read_full_file` summarises a file over the inline limit and attaches the buffer handle
    // (`file_id`). The summaries were bounded by COUNT (all symbols; 20+10 lines; 30 lines;
    // 30 sections), and a count has no size, so a 1,500-function file or a file of a few very
    // wide lines made the JSON exceed the inline limit. `call_content` then buffered it a
    // second time under `@tool_*`, leaving the caller two handles for one read and a
    // `json_path="$.field"` hint that reaches nothing. Measured 2026-10-05: `read_file` on a
    // 6,206-line source file returned `@tool_0bce7715` (20,636 B) beside `@file_0bce76f9`.

    /// Every distinct buffer handle named in `text`, so "one handle" is counted, not assumed.
    fn handles_in(text: &str) -> std::collections::BTreeSet<String> {
        text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '@' || c == '_'))
            .filter(|t| {
                ["@file_", "@tool_", "@cmd_", "@bg_"]
                    .iter()
                    .any(|p| t.starts_with(p))
            })
            .map(str::to_owned)
            .collect()
    }

    /// The primary block of `call_content` for `path`, as text.
    async fn read_text(path: &std::path::Path) -> String {
        let ctx = test_ctx().await;
        let content = ReadFile
            .call_content(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        content[0]
            .as_text()
            .map(|t| t.text.clone())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn a_source_file_with_many_symbols_is_summarised_inline_with_one_handle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.rs");
        let src: String = (1..=1500).map(|i| format!("fn f{i:04}() {{}}\n")).collect();
        std::fs::write(&path, &src).unwrap();
        assert!(
            src.len() > 20_000,
            "fixture must be far over the inline limit"
        );

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.400}"
        );
        assert!(
            !text.contains("buffered_bytes"),
            "the `@tool_*` envelope's field leaked: {text:.400}"
        );
        let handles = handles_in(&text);
        assert_eq!(handles.len(), 1, "one read, one handle; got {handles:?}");
        assert!(handles.iter().next().unwrap().starts_with("@file_"));
        assert!(text.contains("f0001"), "the FIRST symbol must survive");
        assert!(text.contains("f1500"), "the LAST symbol must survive");
        assert!(
            text.contains("entries omitted"),
            "a cut must say so, and say how much: {text:.600}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} bytes is over the inline limit",
            text.len()
        );
    }

    #[tokio::test]
    async fn a_source_file_summary_within_the_budget_is_not_cut() {
        // Over the inline limit as TEXT (40 functions of ~330 B), but the symbol list is a
        // few KB: nothing to cut, so nothing may be, and no marker may claim otherwise.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fits.rs");
        let src: String = (1..=40)
            .map(|i| format!("fn g{i:02}() {{ let _ = \"{}\"; }}\n", "p".repeat(300)))
            .collect();
        std::fs::write(&path, &src).unwrap();
        assert!(crate::tools::exceeds_inline_limit(&src));
        let ctx = test_ctx().await;

        let result = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert_eq!(
            result["symbols"],
            crate::tools::file_summary::summarize_source(path.to_str().unwrap(), &src)["symbols"],
            "a summary that fits must be returned exactly as the summarizer built it"
        );
        assert!(result.get("symbols_truncated").is_none(), "{result}");
        let hint = result["overflow"]["hint"].as_str().unwrap_or("");
        assert!(!hint.contains("entries omitted"), "{hint}");
    }
    /// CONTROL AT THE EDGE of the contract: a file whose summary USED TO FIT must come back
    /// exactly as it did before any bound existed. The golden is what `fac7abce` (before
    /// `bound_summary`) returned for this fixture, captured by running that commit's code, with
    /// the buffer handle replaced by `@file_X`: a 17,200 B source file whose summary is 8,938 B
    /// and whose whole envelope is 9,224 B, under the 10,003 B inline limit. A bound sized for
    /// the worst case (6,000 B) cut it and added `<key>_truncated`, `total_<key>`,
    /// `<key>_omitted`, a note in the hint and a gap line: a contract change for ordinary files.
    #[tokio::test]
    async fn a_summary_that_fits_is_returned_exactly_as_before_any_bound_existed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fits.rs");
        let src: String = (1..=200)
            .map(|i| format!("fn f{i:03}() {{ let _ = \"{}\"; }}\n", "p".repeat(60)))
            .collect();
        std::fs::write(&path, &src).unwrap();
        let ctx = test_ctx().await;

        let result = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let id = result["file_id"].as_str().unwrap().to_string();
        let got: Value = serde_json::from_str(
            &serde_json::to_string(&result)
                .unwrap()
                .replace(&id, "@file_X"),
        )
        .unwrap();
        let golden: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/golden/read_file_summary_fits.json"
        ))
        .unwrap();
        assert_eq!(got, golden, "a summary that fit changed shape or content");
        // The size this fixture sits at is the point: over the old 6,000 B budget, under the limit.
        assert!(
            (6_100..9_000).contains(&got["symbols"].to_string().len()),
            "the fixture drifted off the edge: {} B",
            got["symbols"].to_string().len()
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&result.to_string()),
            "{} B",
            result.to_string().len()
        );
    }

    #[tokio::test]
    async fn the_cut_symbol_list_shows_its_gap_between_the_two_halves() {
        // The gap line must sit WHERE the entries are missing, with the neighbours' line
        // numbers either side: a gap line at the wrong index tells the reader the wrong
        // thing about which lines the omitted symbols cover.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gap.rs");
        let src: String = (1..=1500).map(|i| format!("fn f{i:04}() {{}}\n")).collect();
        std::fs::write(&path, &src).unwrap();
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let gap = &result["symbols_omitted"];
        let after = gap["after"].as_u64().unwrap() as usize;
        let from = gap["from_line"].as_u64().unwrap();
        let to = gap["to_line"].as_u64().unwrap();
        assert_eq!(
            from as usize,
            after + 1,
            "each `fn` is on its own line: {gap}"
        );
        assert_eq!(to + 1, result["symbols"][after]["line"].as_u64().unwrap());

        let rendered = format_read_file(&result);
        let lines: Vec<&str> = rendered.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.contains("symbols omitted"))
            .unwrap_or_else(|| panic!("no gap line in: {rendered:.600}"));
        assert!(
            lines[at].contains(&format!("(L{from}-L{to})")),
            "{}",
            lines[at]
        );
        assert!(
            lines[at - 1].ends_with(&format!("L{}", from - 1)),
            "the entry before the gap must be the last kept head symbol: {}",
            lines[at - 1]
        );
        assert!(
            lines[at + 1].ends_with(&format!("L{}", to + 1)),
            "the entry after the gap must be the first kept tail symbol: {}",
            lines[at + 1]
        );
    }

    #[tokio::test]
    async fn a_file_of_few_very_wide_lines_is_summarised_inline_with_one_handle() {
        // Twelve lines of 6 KB: far under any line budget, so only a byte bound can refuse it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.txt");
        let body: String = (b'a'..=b'l')
            .map(|c| format!("{}\n", (c as char).to_string().repeat(6_000)))
            .collect();
        std::fs::write(&path, &body).unwrap();

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(
            text.contains("bytes shown"),
            "a cut must say so: {text:.300}"
        );
        assert!(text.contains("aaaa"), "the head of the file must survive");
        assert!(text.contains("llll"), "the tail of the file must survive");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }

    #[tokio::test]
    async fn one_enormous_json_line_is_summarised_inline_with_one_handle() {
        // The invalid-JSON fallback path: a single 80 KB line that does not parse falls back
        // to the generic head/tail summary, which is one line wide.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.json");
        let body = format!("{{\"items\": [{}", "{\"k\": \"v\"}, ".repeat(6_500));
        std::fs::write(&path, &body).unwrap();
        assert!(body.len() > 70_000);

        // The renderer prints no content for type `json` (only a schema), so the bound is
        // asserted on the VALUE it was applied to, where `head` and `tail` both live.
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        for field in ["head", "tail"] {
            let s = value[field]
                .as_str()
                .unwrap_or_else(|| panic!("`{field}` missing from {value:.300}"));
            assert!(
                s.contains("bytes shown"),
                "`{field}` was not cut: {} B",
                s.len()
            );
        }
        assert!(
            !crate::tools::exceeds_inline_limit(&value.to_string()),
            "the summary is {} B and would be buffered a second time",
            value.to_string().len()
        );

        // And through the real entry point: inline, one handle.
        let text = read_text(&path).await;
        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
    }

    #[tokio::test]
    async fn a_wide_config_preview_is_summarised_inline_with_one_handle() {
        // `summarize_config` takes the first 30 LINES; here each is 700 B.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.ini");
        let body: String = (0..40)
            .map(|i| format!("key{i:02}={}\n", "v".repeat(700)))
            .collect();
        std::fs::write(&path, &body).unwrap();

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(text.contains("bytes shown"), "{text:.300}");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }

    #[tokio::test]
    async fn wide_yaml_keys_are_bounded_by_bytes_not_by_the_thirty_entry_count() {
        // 30 top-level keys of 600 B each: the entry COUNT (30) is within its cap, the BYTES
        // (~20 KB of `sections`) are not.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.yaml");
        let body: String = (0..30)
            .map(|i| format!("{}{i:02}: 1\n", "k".repeat(600)))
            .collect();
        std::fs::write(&path, &body).unwrap();

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(text.contains("entries omitted"), "{text:.300}");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }
    #[tokio::test]
    async fn a_json_object_with_wide_keys_is_summarised_inline_with_one_handle() {
        // A VALID object of 30 keys, each 600 B wide: `summarize_json` lists them under
        // `schema.keys`, one level down from where a top-level-only bound would look.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.json");
        let body = format!(
            "{{{}}}",
            (0..30)
                .map(|i| format!("\"{}{i:02}\": {i}", "k".repeat(600)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::fs::write(&path, &body).unwrap();
        assert!(crate::tools::exceeds_inline_limit(&body));

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(text.contains("entries omitted"), "{text:.300}");
        assert!(
            text.contains("keys omitted"),
            "the gap must be shown where it falls: {text:.600}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }
    // ---- one reach test per renderer site that prints a gap ----
    //
    // `format_read_file_summary` has a separate loop per summary type, each calling
    // `omitted_gap` for its own array key. A site with no test is a site whose gap line can
    // vanish while every other assertion (the hint's `entries omitted`) stays green.

    #[tokio::test]
    async fn a_wide_yaml_shows_its_gap_line_between_the_kept_sections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gap.yaml");
        let body: String = (0..30)
            .map(|i| format!("{}{i:02}: 1\n", "k".repeat(600)))
            .collect();
        std::fs::write(&path, &body).unwrap();
        let text = read_text(&path).await;
        assert!(text.contains("sections omitted"), "{text:.600}");
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
    }
    #[tokio::test]
    async fn a_single_enormous_yaml_key_is_bounded_too() {
        // ONE top-level key of 20 KB: the `sections` array has a single entry, so a bound that
        // only looks at arrays of two or more lets the whole thing through to a second handle.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("one.yaml");
        std::fs::write(&path, format!("{}: 1\n", "k".repeat(20_000))).unwrap();

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(text.contains("entries omitted"), "{text:.300}");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }
    // ---- read_markdown: the three responses the first sweep probed and left ----

    /// Quote-heavy body, so escaping inflates it: every `"` serializes to two bytes.
    fn quoted_body(lines: usize, width: usize) -> String {
        (0..lines)
            .map(|_| format!("{}\n", "\"".repeat(width)))
            .collect()
    }

    #[tokio::test]
    async fn a_tier_two_markdown_that_serializes_over_the_limit_gets_a_handle_not_a_tool_buffer() {
        // 30 headings (under HEADINGS_HARD_CAP), 180 lines (over LINE_SOFT_CAP: tier 2), 7.8 KB
        // RAW, under the limit. Serialized with its heading map and with every quote doubled it
        // is far over, and the response used to be buffered under a bare `@tool_*`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tier2.md");
        let body: String = (1..=30)
            .map(|i| format!("## Section {i:02}\n{}\n", quoted_body(4, 60)))
            .collect();
        assert!(
            body.len() < 10_003 && body.lines().count() > 150,
            "{}",
            body.len()
        );
        std::fs::write(&path, &body).unwrap();
        let ctx = test_ctx().await;

        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert!(value["file_id"].is_string(), "no handle: {value:.300}");
        assert!(
            value.get("content").is_none(),
            "the body must not ride along"
        );

        let text = assert_inline_with_one_handle(&path, "tier 2 markdown").await;
        assert!(text.contains("Section 01"), "the heading map must be there");
    }

    #[tokio::test]
    async fn a_tier_one_markdown_that_serializes_over_the_limit_gets_a_handle_too() {
        // 20 headings, 80 lines: tier 1. 6.3 KB raw, doubled quotes push it over once serialized.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tier1.md");
        let body: String = (1..=20)
            .map(|i| format!("## S{i:02}\n{}\n", quoted_body(2, 150)))
            .collect();
        assert!(
            body.len() < 10_003 && body.lines().count() < 150,
            "{}",
            body.len()
        );
        std::fs::write(&path, &body).unwrap();
        assert_inline_with_one_handle(&path, "tier 1 markdown").await;
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert!(value["file_id"].is_string() && value.get("content").is_none());
    }

    #[tokio::test]
    async fn a_markdown_that_serializes_within_the_limit_still_returns_its_body_inline() {
        // The control: plain text, 8 KB, 30 headings: tier 2 as always, body inline, no handle.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.md");
        let body: String = (1..=30)
            .map(|i| {
                format!(
                    "## Section {i:02}\n{}\n\n\n\n\n\n",
                    "plain words here".repeat(10)
                )
            })
            .collect();
        assert!(!crate::tools::exceeds_inline_limit(&body));
        std::fs::write(&path, &body).unwrap();
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert_eq!(value["content"].as_str().unwrap(), body);
        assert!(value.get("file_id").is_none(), "{value:.200}");
    }

    /// `## Big` with `n` `###` sub-headings, each carrying `body` bytes of text.
    fn big_section_file(n: usize, body: usize) -> String {
        let mut s = String::from("## Big\n");
        for i in 1..=n {
            s.push_str(&format!(
                "### Sub {i:03} {}\n{}\n\n",
                "s".repeat(60),
                "b".repeat(body)
            ));
        }
        s
    }

    fn error_body_len(rec: &crate::tools::RecoverableError) -> usize {
        rec.message.len()
            + rec.hint().unwrap_or_default().len()
            + serde_json::to_string(&rec.extra).unwrap().len()
    }

    #[tokio::test]
    async fn the_oversized_section_error_bounds_its_section_map_and_keeps_the_route() {
        // 300 sub-headings: the error's `extra` was 26,629 B, put inline by the server, because
        // an `Err` never reaches `call_content`'s buffering.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bigsec.md");
        std::fs::write(&path, big_section_file(300, 20)).unwrap();
        let ctx = test_ctx().await;

        let err = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "heading": "## Big" }),
                &ctx,
            )
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("an oversized section is a RecoverableError");

        assert!(
            error_body_len(rec) <= crate::tools::INLINE_BYTE_BUDGET,
            "the error body is {} B",
            error_body_len(rec)
        );
        let map = rec.extra["section_map"].as_array().unwrap();
        assert!(map.len() < 300 && map.len() > 20, "{} kept", map.len());
        assert!(map[0]["h"].as_str().unwrap().contains("Sub 001"));
        assert!(map.last().unwrap()["h"]
            .as_str()
            .unwrap()
            .contains("Sub 300"));
        assert_eq!(rec.extra["section_map_truncated"], true);
        assert_eq!(rec.extra["total_section_map"], 300);
        let file_id = rec.extra["file_id"].as_str().unwrap().to_string();
        // The route `next_actions` names must still be in the kept map.
        let first_action = rec.extra["next_actions"][0].as_str().unwrap();
        assert!(first_action.contains("Sub 001") && first_action.contains(&file_id));

        // And the cut note's own route works: follow its line range on the section's handle.
        let hint = rec.hint().unwrap();
        assert!(hint.contains("entries omitted"), "{hint}");
        let after = rec.extra["section_map_omitted"]["after"].as_u64().unwrap();
        let (from, to) = (
            rec.extra["section_map_omitted"]["from_line"]
                .as_u64()
                .unwrap(),
            rec.extra["section_map_omitted"]["to_line"]
                .as_u64()
                .unwrap(),
        );
        assert!(
            hint.contains(&format!("start_line={from}, end_line={to}")),
            "{hint}"
        );
        let followed = ReadFile
            .call(
                json!({ "path": file_id, "start_line": from, "end_line": from + 2 }),
                &ctx,
            )
            .await
            .unwrap();
        let content = followed["content"].as_str().unwrap();
        assert!(
            content.starts_with(&format!("### Sub {:03}", after + 1)),
            "line {from} of the section buffer is not the first omitted sub-heading: {content:.80}"
        );
        assert!(to > from);
    }

    #[tokio::test]
    async fn the_oversized_section_error_clips_the_sections_own_huge_heading() {
        // The section's OWN heading is 12 KB: it is echoed in the message and the breadcrumb.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hugeown.md");
        let mut body = format!("## {}\n", "o".repeat(12_000));
        for i in 1..=40 {
            body.push_str(&format!("### Sub {i:03}\n{}\n\n", "b".repeat(300)));
        }
        std::fs::write(&path, &body).unwrap();
        let ctx = test_ctx().await;

        let err = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "heading": format!("## {}", "o".repeat(40)) }),
                &ctx,
            )
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("an oversized section is a RecoverableError");

        assert!(
            error_body_len(rec) <= crate::tools::INLINE_BYTE_BUDGET,
            "the error body is {} B",
            error_body_len(rec)
        );
        assert!(
            rec.message.len() < 400,
            "the message echoes {} B",
            rec.message.len()
        );
        let crumb = rec.extra["breadcrumb"][0].as_str().unwrap();
        assert!(
            crumb.len() <= 200 && crumb.starts_with("## ooo"),
            "{} B",
            crumb.len()
        );
    }

    #[tokio::test]
    async fn the_oversized_section_error_keeps_a_map_that_fits_whole() {
        // 40 sub-headings of 300 B: the section is over the inline limit (14.8 KB) but its map
        // (~3.6 KB) fits, so nothing in the error changes shape.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("smallmap.md");
        std::fs::write(&path, big_section_file(40, 300)).unwrap();
        let ctx = test_ctx().await;
        let err = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "heading": "## Big" }),
                &ctx,
            )
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap();
        assert_eq!(rec.extra["section_map"].as_array().unwrap().len(), 40);
        assert!(rec.extra.get("section_map_truncated").is_none());
        assert!(!rec.hint().unwrap().contains("omitted"));
    }
    #[tokio::test]
    async fn the_oversized_section_error_clips_a_huge_sub_heading_and_keeps_its_route() {
        // A 12 KB FIRST sub-heading: `next_actions` echoes it and `section_map` lists it, so the
        // error body was 12,700 B with the map dropped to `summary_omitted`, contradicting the
        // claim that the first sub-heading is always kept. The echoed text is clipped to a
        // prefix with its true length beside it, and a prefix still resolves the heading.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hugesub.md");
        let mut body = format!("## Big\n### {}\nfirst body\n\n", "s".repeat(12_000));
        for i in 2..=40 {
            body.push_str(&format!("### Sub {i:03}\n{}\n\n", "b".repeat(300)));
        }
        std::fs::write(&path, &body).unwrap();
        let ctx = test_ctx().await;

        let err = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "heading": "## Big" }),
                &ctx,
            )
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("an oversized section is a RecoverableError");

        assert!(
            error_body_len(rec) <= crate::tools::INLINE_BYTE_BUDGET,
            "the error body is {} B",
            error_body_len(rec)
        );
        assert!(
            rec.extra.get("summary_omitted").is_none(),
            "{:?}",
            rec.extra
        );
        let map = rec.extra["section_map"].as_array().unwrap();
        assert!(map.len() > 30, "the map was gutted: {} entries", map.len());
        let first = &map[0];
        assert!(first["h"].as_str().unwrap().starts_with("### sss"));
        assert!(
            first["h"].as_str().unwrap().len() <= 200,
            "the heading was not clipped"
        );
        assert!(first["h_bytes"].as_u64().unwrap() > 12_000, "{first}");
        assert!(
            map[1].get("h_bytes").is_none(),
            "a short heading must not be marked clipped"
        );

        // The route in `next_actions` still works: a PREFIX of the heading resolves it.
        let action = rec.extra["next_actions"][0].as_str().unwrap();
        assert!(action.len() < 400, "next_actions[0] is {} B", action.len());
        let file_id = rec.extra["file_id"].as_str().unwrap();
        let quoted = action
            .split("heading=")
            .nth(1)
            .unwrap()
            .trim_end_matches(')');
        let heading: String = serde_json::from_str(quoted).unwrap();
        // The prefix resolves the 12 KB sub-heading. That section is ITSELF over the inline
        // limit (its heading alone is 12 KB), so the answer is its own oversized-section error,
        // naming exactly the two lines it spans: the heading and its one body line.
        let followed = ReadFile
            .call(json!({ "path": file_id, "heading": heading }), &ctx)
            .await
            .unwrap_err();
        let rec2 = followed
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("the resolved section is oversized");
        assert!(
            rec2.message.contains("spans 2 lines"),
            "the clipped heading did not resolve its section: {}",
            rec2.message
        );
        assert!(
            error_body_len(rec2) <= crate::tools::INLINE_BYTE_BUDGET,
            "the follow-up error body is {} B",
            error_body_len(rec2)
        );
    }

    fn two_sections(width: usize) -> String {
        format!(
            "## M1\n{}\n\n## M2\n{}\n",
            "lorem ipsum dolor sit amet ".repeat(width / 27),
            "consectetur adipiscing elit ".repeat(width / 28)
        )
    }

    #[tokio::test]
    async fn a_multi_heading_read_drops_the_duplicate_sections_when_it_would_not_fit() {
        // Two sections of ~4.5 KB: `content` is the two joined (9.2 KB, fits), `sections` repeats
        // the same text (serialized 18.9 KB together, one `@tool_*`).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.md");
        std::fs::write(&path, two_sections(4_500)).unwrap();
        let ctx = test_ctx().await;
        let input = json!({ "path": path.to_str().unwrap(), "headings": ["## M1", "## M2"] });

        let value = ReadFile.call(input.clone(), &ctx).await.unwrap();

        let content = value["content"].as_str().unwrap();
        assert!(content.contains("lorem") && content.contains("consectetur"));
        assert!(value.get("sections").is_none(), "the duplicate stayed");
        assert_eq!(value["sections_omitted"], true);
        assert!(value["hint"]
            .as_str()
            .unwrap()
            .contains("`sections` omitted"));
        assert!(!crate::tools::exceeds_inline_limit(&value.to_string()));
        let text = ReadFile
            .call_content(input, &ctx)
            .await
            .unwrap()
            .remove(0)
            .as_text()
            .map(|t| t.text.clone())
            .unwrap();
        assert!(!text.contains("@tool_"), "{text:.200}");
    }

    #[tokio::test]
    async fn a_multi_heading_read_that_fits_keeps_its_per_section_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi-small.md");
        std::fs::write(&path, two_sections(1_000)).unwrap();
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "headings": ["## M1", "## M2"] }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(value["sections"].as_array().unwrap().len(), 2);
        assert!(value.get("sections_omitted").is_none());
    }
    #[tokio::test]
    async fn a_single_enormous_json_key_is_bounded_too() {
        // ONE top-level key of 20 KB in a VALID object: `schema.keys` has a single entry, one
        // level down, so a nested collector that only takes arrays of two or more lets it through.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("one.json");
        std::fs::write(&path, format!("{{\"{}\": 1}}", "k".repeat(20_000))).unwrap();

        let text = assert_inline_with_one_handle(&path, "one huge JSON key").await;
        assert!(text.contains("entries omitted"), "{text:.300}");
    }
    #[tokio::test]
    async fn a_generic_file_with_one_wide_head_keeps_the_head_close_to_the_budget() {
        // 20 lines of 600 B then 10 short ones: `head` (12 KB) is the only wide string, `tail`
        // (~400 B) is part of the fixed cost. The best share for `head` is ~8 KB, over HALF of
        // it (6 KB), so a search capped at half returns a ~5 KB envelope for a file that could carry 9.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("headwide.txt");
        let mut body: String = (0..20)
            .map(|i| format!("{i:02}{}\n", "x".repeat(598)))
            .collect();
        body.extend((0..10).map(|i| format!("short line {i:02} {}\n", "s".repeat(20))));
        std::fs::write(&path, &body).unwrap();
        let ctx = test_ctx().await;

        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            value["head"].as_str().unwrap().contains("bytes shown"),
            "{value:.200}"
        );
        let size = value.to_string().len();
        assert!(
            (8_800..=crate::tools::INLINE_BYTE_BUDGET).contains(&size),
            "{size} B: the head was cut far below the {} B target",
            crate::tools::INLINE_BYTE_BUDGET
        );
    }

    // ---- TOML flat keys: the route the hint names must work ----

    /// `n` flat TOML keys of 600 B each, written in line order `order` (names `kNN`).
    fn wide_flat_toml(order: impl Iterator<Item = usize>) -> String {
        order
            .map(|i| format!("{}{i:02} = 1\n", "k".repeat(600)))
            .collect()
    }

    /// The `(start_line, end_line)` pairs with real numbers in a hint (the generic `N`/`M`
    /// placeholders do not match).
    fn numeric_routes(hint: &str) -> Vec<(u64, u64)> {
        let re = regex::Regex::new(r"start_line=(\d+), end_line=(\d+)").unwrap();
        re.captures_iter(hint)
            .map(|c| (c[1].parse().unwrap(), c[2].parse().unwrap()))
            .collect()
    }

    #[tokio::test]
    async fn a_toml_whose_keys_sort_against_their_line_order_offers_no_false_route() {
        // Written z..a: alphabetical order is the REVERSE of line order. The gap used to come
        // out `from_line: 15, to_line: 4` and the hint offered an impossible range.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reversed.toml");
        std::fs::write(&path, wide_flat_toml((0..20).rev())).unwrap();
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let gap = &value["keys_omitted"];
        assert!(
            gap["count"].as_u64().unwrap() > 0,
            "nothing was cut: {value:.200}"
        );
        assert!(
            gap["from_line"].is_null() && gap["to_line"].is_null(),
            "{gap}"
        );
        let hint = value["overflow"]["hint"].as_str().unwrap();
        assert!(
            numeric_routes(hint).is_empty(),
            "a made-up line range in: {hint}"
        );
        assert!(
            hint.contains("keys:") && hint.contains("entries omitted"),
            "{hint}"
        );

        // Follow the route the hint DOES name: read the file in ranges from its handle.
        let file_id = value["file_id"].as_str().unwrap();
        let followed = ReadFile
            .call(
                json!({ "path": file_id, "start_line": 1, "end_line": 2 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            followed["content"]
                .as_str()
                .unwrap()
                .starts_with(&"k".repeat(600)),
            "{followed:.200}"
        );
    }

    #[tokio::test]
    async fn a_toml_whose_keys_are_in_line_order_offers_a_route_that_returns_the_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ordered.toml");
        std::fs::write(&path, wide_flat_toml(0..20)).unwrap();
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let after = value["keys_omitted"]["after"].as_u64().unwrap() as usize;
        let hint = value["overflow"]["hint"].as_str().unwrap();
        let routes = numeric_routes(hint);
        assert_eq!(routes.len(), 1, "{hint}");
        let (start, end) = routes[0];
        assert!(
            start <= end,
            "an impossible range was offered: {start}-{end}"
        );

        // Follow it: the lines it names are exactly the omitted keys, the first one first.
        let file_id = value["file_id"].as_str().unwrap();
        let followed = ReadFile
            .call(
                json!({ "path": file_id, "start_line": start, "end_line": end }),
                &ctx,
            )
            .await
            .unwrap();
        let content = followed["content"].as_str().unwrap();
        assert!(
            content.starts_with(&format!("{}{after:02}", "k".repeat(600))),
            "line {start} is not the first omitted key (index {after}): {content:.40}"
        );
        assert_eq!(content.lines().count() as u64, end - start + 1);
    }

    #[tokio::test]
    async fn a_multi_heading_read_keeps_its_coverage_and_drops_only_the_duplicate() {
        // A third, unread heading makes `coverage` non-empty. The duplicate `sections` is the
        // first thing to go and it is enough: `coverage` must survive. A drop order of
        // coverage-first, or dropping both once the response is over, would lose it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi-cov.md");
        std::fs::write(&path, format!("{}\n## M3\nshort\n", two_sections(4_500))).unwrap();
        let ctx = test_ctx().await;

        let value = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "headings": ["## M1", "## M2"] }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(value.get("sections").is_none() && value["sections_omitted"] == true);
        assert!(
            value.get("coverage_omitted").is_none(),
            "coverage was dropped too"
        );
        assert_eq!(value["coverage"]["unread"][0], "## M3", "{value:.300}");
    }

    #[tokio::test]
    async fn a_multi_heading_read_whose_content_serializes_over_the_limit_takes_the_error_path() {
        // Two sections of 3,000 quotes: the joined content is ~6 KB RAW, under the limit, but
        // ~12 KB serialized. The oversized test must measure what lands in the response.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi-quotes.md");
        let body = format!(
            "## Q1\n{}\n\n## Q2\n{}\n",
            "\"".repeat(3_000),
            "\"".repeat(3_000)
        );
        std::fs::write(&path, &body).unwrap();
        assert!(body.len() < 10_003);
        let ctx = test_ctx().await;

        let err = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "headings": ["## Q1", "## Q2"] }),
                &ctx,
            )
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("an over-limit combined read is a RecoverableError");
        assert!(
            rec.message.contains("exceeds inline threshold"),
            "{}",
            rec.message
        );
        assert!(rec.extra["file_id"].is_string());
    }
    // ---- SWEEPS: what is measured must be what is returned ----
    //
    // `read()` adds `"format": "markdown"` AFTER the tier builders measured their response, and
    // the multi-heading read added its hint and `sections_omitted` after it measured. Every key
    // added after the measurement widens a window just below the inline limit: a response the
    // builder judged to fit (<= 10,003 B) came back 10,004-10,136 B and was buffered under a
    // `@tool_*` beside its own handle. Point probes at the edges miss that; a sweep that walks
    // the content size across the whole band, one response kind at a time, cannot.

    // cap-class: NOT_A_CAP — the inline limit the sweeps assert AGAINST; it shapes no result
    const LIMIT: usize = 10_003; // `exceeds_inline_limit`: len / 4 > 2,500

    /// What one sweep saw, for the assertions after it.
    #[derive(Default)]
    struct Sweep {
        points: usize,
        worst: usize,
        /// Largest response that was returned UNCUT and inline (it sits on the edge).
        worst_uncut: usize,
        cut: usize,
        uncut: usize,
    }

    #[tokio::test]
    async fn sweep_tier_three_heading_map_across_the_edge_band() {
        // 100 headings (over HEADINGS_HARD_CAP, so always tier 3); the LAST heading widens by
        // 2 B per point, walking the uncut response from ~9.9 KB to ~10.3 KB.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("map.md");
        let ctx = test_ctx().await;
        let mut sw = Sweep::default();
        for x in (1_300..=1_600).step_by(2) {
            let mut body: String = (1..100)
                .map(|i| format!("## H{i:03} {}\nb\n\n", "w".repeat(60)))
                .collect();
            body.push_str(&format!("## H100 {}\nb\n", "w".repeat(60 + x)));
            std::fs::write(&path, &body).unwrap();

            let value = ReadFile
                .call(json!({ "path": path.to_str().unwrap() }), &ctx)
                .await
                .unwrap();
            let size = value.to_string().len();
            assert!(size <= LIMIT, "x={x}: the returned map is {size} B");
            let text = assert_inline_with_one_handle(&path, &format!("tier-3 map x={x}")).await;
            assert!(text.contains("H001"), "x={x}: the first heading was lost");
            sw.points += 1;
            sw.worst = sw.worst.max(size);
            if value.get("headings_truncated").is_some() {
                sw.cut += 1;
            } else {
                sw.uncut += 1;
                sw.worst_uncut = sw.worst_uncut.max(size);
            }
        }
        eprintln!(
            "SWEEP tier3: {} points, worst {} B, worst uncut {} B, cut {}, uncut {}",
            sw.points, sw.worst, sw.worst_uncut, sw.cut, sw.uncut
        );
        assert!(
            sw.cut > 0 && sw.uncut > 0,
            "the sweep never crossed the edge"
        );
        assert!(
            sw.worst_uncut >= 9_985,
            "the sweep stopped short of the band: {}",
            sw.worst_uncut
        );
    }

    #[tokio::test]
    async fn sweep_tier_one_and_two_fall_through_across_the_edge_band() {
        // 30 headings, tier 1/2 (raw body under the limit); the last section widens by 2 B per
        // point so the serialized response (body + heading map + hint + format) crosses the limit.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inline.md");
        let ctx = test_ctx().await;
        let mut sw = Sweep::default();
        for x in (1_000..=1_500).step_by(2) {
            let mut body: String = (1..30)
                .map(|i| format!("## S{i:02}\n{}\n\n", "p".repeat(250)))
                .collect();
            body.push_str(&format!("## S30\n{}\n", "p".repeat(250 + x)));
            assert!(
                body.len() <= LIMIT,
                "x={x}: the raw body must stay under the limit"
            );
            std::fs::write(&path, &body).unwrap();

            let value = ReadFile
                .call(json!({ "path": path.to_str().unwrap() }), &ctx)
                .await
                .unwrap();
            let size = value.to_string().len();
            assert!(size <= LIMIT, "x={x}: the returned response is {size} B");
            let text = read_text(&path).await;
            assert!(
                !text.contains("@tool_"),
                "x={x}: parked under @tool_: {text:.120}"
            );
            assert!(handles_in(&text).len() <= 1, "x={x}: {text:.120}");
            sw.points += 1;
            sw.worst = sw.worst.max(size);
            if value.get("content").is_some() {
                sw.uncut += 1;
                sw.worst_uncut = sw.worst_uncut.max(size);
            } else {
                assert!(
                    value["file_id"].is_string(),
                    "x={x}: fell through without a handle"
                );
                sw.cut += 1;
            }
        }
        eprintln!(
            "SWEEP tier1/2: {} points, worst {} B, worst inline {} B, inline {}, fell through {}",
            sw.points, sw.worst, sw.worst_uncut, sw.uncut, sw.cut
        );
        assert!(
            sw.cut > 0 && sw.uncut > 0,
            "the sweep never crossed the edge"
        );
        assert!(
            sw.worst_uncut >= 9_985,
            "the sweep stopped short of the band: {}",
            sw.worst_uncut
        );
    }

    #[tokio::test]
    async fn sweep_multi_heading_read_across_the_edge_band() {
        // Two sections of width n: `content` is ~2n. It fits alone from ~9.5 KB to 10,003 B, where
        // the response (content + hint + `sections_omitted` + format) is what must fit, or the read
        // must take the error path. At no point may the result be parked under `@tool_*`.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.md");
        let ctx = test_ctx().await;
        let mut sw = Sweep::default();
        for n in 4_800..=5_000 {
            std::fs::write(&path, two_sections_exact(n)).unwrap();
            let input = json!({ "path": path.to_str().unwrap(), "headings": ["## M1", "## M2"] });

            let text = match ReadFile.call_content(input.clone(), &ctx).await {
                Ok(mut content) => content.remove(0).as_text().map(|t| t.text.clone()).unwrap(),
                // The error path (content alone does not fit) surfaces as an `Err`.
                Err(e) => e.to_string(),
            };
            assert!(
                !text.contains("@tool_"),
                "n={n}: parked under @tool_: {text:.120}"
            );
            assert!(handles_in(&text).len() <= 1, "n={n}: {text:.120}");
            sw.points += 1;
            match ReadFile.call(input, &ctx).await {
                Ok(value) => {
                    let size = value.to_string().len();
                    assert!(size <= LIMIT, "n={n}: the returned response is {size} B");
                    sw.worst = sw.worst.max(size);
                    sw.worst_uncut = sw.worst_uncut.max(size);
                    sw.uncut += 1;
                }
                Err(e) => {
                    let rec = e.downcast_ref::<crate::tools::RecoverableError>().unwrap();
                    assert!(rec.extra["file_id"].is_string(), "n={n}");
                    sw.cut += 1;
                }
            }
        }
        eprintln!(
            "SWEEP multi: {} points, worst ok {} B, ok {}, error path {}",
            sw.points, sw.worst, sw.uncut, sw.cut
        );
        assert!(
            sw.cut > 0 && sw.uncut > 0,
            "the sweep never crossed the edge"
        );
        // The decision to take the error path is deliberately CONSERVATIVE by the 24 B of
        // `"coverage_omitted":true,` it always counts (coverage's presence is only known after
        // it is marked), so the largest inline response sits up to 24 B under the limit.
        assert!(
            sw.worst >= 9_975,
            "the sweep stopped short of the band: {}",
            sw.worst
        );
    }
    #[tokio::test]
    async fn sweep_tier_two_fall_through_across_the_edge_band() {
        // The same sweep, for TIER 2: 210 lines (over LINE_SOFT_CAP), 30 headings. The earlier
        // sweep has ~90 lines and only ever builds tier 1, so a tier-2 candidate built without
        // `format` slipped past it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inline2.md");
        let ctx = test_ctx().await;
        let mut sw = Sweep::default();
        for x in (2_300..=2_900).step_by(2) {
            let section = |i: usize, last: usize| {
                format!("## S{i:02}\n{}{}\n\n", "p".repeat(40) + "\n", {
                    let mut lines = "q".repeat(40) + "\n";
                    lines = lines.repeat(3);
                    lines + &"r".repeat(40 + last)
                })
            };
            let mut body: String = (1..30).map(|i| section(i, 0)).collect();
            body.push_str(&section(30, x));
            assert!(body.lines().count() > 150, "{}", body.lines().count());
            assert!(
                body.len() <= LIMIT,
                "x={x}: the raw body must stay under the limit"
            );
            std::fs::write(&path, &body).unwrap();

            let value = ReadFile
                .call(json!({ "path": path.to_str().unwrap() }), &ctx)
                .await
                .unwrap();
            let size = value.to_string().len();
            assert!(size <= LIMIT, "x={x}: the returned response is {size} B");
            let text = read_text(&path).await;
            assert!(
                !text.contains("@tool_"),
                "x={x}: parked under @tool_: {text:.120}"
            );
            assert!(handles_in(&text).len() <= 1, "x={x}: {text:.120}");
            sw.points += 1;
            sw.worst = sw.worst.max(size);
            if value.get("content").is_some() {
                assert!(
                    value["lines"].as_u64().unwrap() > 150,
                    "this must be tier 2"
                );
                sw.uncut += 1;
                sw.worst_uncut = sw.worst_uncut.max(size);
            } else {
                sw.cut += 1;
            }
        }
        eprintln!(
            "SWEEP tier2: {} points, worst {} B, worst inline {} B, inline {}, fell through {}",
            sw.points, sw.worst, sw.worst_uncut, sw.uncut, sw.cut
        );
        assert!(
            sw.cut > 0 && sw.uncut > 0,
            "the sweep never crossed the edge"
        );
        assert!(
            sw.worst_uncut >= 9_985,
            "the sweep stopped short of the band: {}",
            sw.worst_uncut
        );
    }

    #[tokio::test]
    async fn sweep_multi_heading_read_with_coverage_across_the_edge_band() {
        // A third, unread heading keeps `coverage` in the response. The candidate that drops only
        // `sections` then carries `coverage` too, and its fit is decided by what it MEASURES: a
        // candidate built without `format` was accepted up to 21 B too large (the no-coverage
        // sweep cannot see this: the 24 B it always reserves for `coverage_omitted` absorbs it).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi-cov.md");
        let ctx = test_ctx().await;
        let mut sw = Sweep::default();
        for n in 4_700..=4_950 {
            std::fs::write(&path, format!("{}\n## M3\nshort\n", two_sections_exact(n))).unwrap();
            let input = json!({ "path": path.to_str().unwrap(), "headings": ["## M1", "## M2"] });
            match ReadFile.call(input, &ctx).await {
                Ok(value) => {
                    let size = value.to_string().len();
                    assert!(size <= LIMIT, "n={n}: the returned response is {size} B");
                    sw.worst = sw.worst.max(size);
                    sw.uncut += 1;
                    if value.get("coverage").is_some() {
                        sw.worst_uncut = sw.worst_uncut.max(size);
                    }
                }
                Err(_) => sw.cut += 1,
            }
            sw.points += 1;
        }
        eprintln!(
            "SWEEP multi+coverage: {} points, worst ok {} B, worst with coverage kept {} B, ok {}, error path {}",
            sw.points, sw.worst, sw.worst_uncut, sw.uncut, sw.cut
        );
        assert!(
            sw.cut > 0 && sw.uncut > 0,
            "the sweep never crossed the edge"
        );
        assert!(
            sw.worst_uncut >= 9_985,
            "no response that KEPT its coverage reached the edge: {}",
            sw.worst_uncut
        );
    }

    /// `## M1` and `## M2`, each a single line of exactly `n` plain bytes.
    fn two_sections_exact(n: usize) -> String {
        format!("## M1\n{}\n\n## M2\n{}\n", "m".repeat(n), "n".repeat(n))
    }

    // ---- JSON escaping must not break the one-handle guarantee ----
    //
    // The summary is measured against the SERIALIZED inline limit, and escaping inflates raw
    // bytes: 2x for `"` `\` and tab, 6x for `\x01` and the ESC of ANSI colour codes. A cut sized
    // in raw bytes passed its tests on plain text and left these over the limit: two handles.
    // Measured by the reviewer through `call_content`: 11,593 B for quotes, 33,769 B for `\x01`.

    const INFLATING_CHARS: [(&str, char); 5] = [
        ("quote", '"'),
        ("backslash", '\\'),
        ("tab", '\t'),
        ("control", '\u{1}'),
        ("ansi-escape", '\u{1b}'),
    ];

    /// The outcome every case here must reach: inline, no `@tool_*`, exactly one handle.
    async fn assert_inline_with_one_handle(path: &std::path::Path, what: &str) -> String {
        let text = read_text(path).await;
        assert!(
            !text.contains("@tool_"),
            "{what}: a second handle was minted: {text:.200}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{what}: {text:.200}");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{what}: {} B is over the inline limit",
            text.len()
        );
        text
    }

    #[tokio::test]
    async fn wide_lines_of_every_escaping_kind_come_back_inline_with_one_handle() {
        for (name, ch) in INFLATING_CHARS {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("wide.txt");
            let line = ch.to_string().repeat(6_000);
            std::fs::write(&path, format!("{}\n", [line.as_str(); 12].join("\n"))).unwrap();
            assert_inline_with_one_handle(&path, &format!("wide .txt of {name}")).await;
        }
    }

    #[tokio::test]
    async fn config_previews_of_every_escaping_kind_come_back_inline_with_one_handle() {
        for (name, ch) in INFLATING_CHARS {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("wide.ini");
            let body: String = (0..40)
                .map(|i| format!("key{i:02}={}\n", ch.to_string().repeat(700)))
                .collect();
            std::fs::write(&path, body).unwrap();
            assert_inline_with_one_handle(&path, &format!(".ini of {name}")).await;
        }
    }

    #[tokio::test]
    async fn symbolless_source_of_every_escaping_kind_comes_back_inline_with_one_handle() {
        // A `.rs` file with no symbols falls back to the generic head/tail summary: the shape
        // of a generated or data-only source file.
        for (name, ch) in INFLATING_CHARS {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("data.rs");
            let line = format!("// {}", ch.to_string().repeat(6_000));
            std::fs::write(&path, format!("{}\n", [line.as_str(); 12].join("\n"))).unwrap();
            assert_inline_with_one_handle(&path, &format!("symbol-less .rs of {name}")).await;
        }
    }

    #[tokio::test]
    async fn wide_markdown_summary_lines_of_every_escaping_kind_come_back_inline() {
        // `.mdx` takes the Markdown SUMMARY (headings array), whose entries carry the heading
        // text: quote-heavy headings make every entry cost double.
        for (name, ch) in INFLATING_CHARS {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("wide.mdx");
            let body: String = (1..=40)
                .map(|i| format!("# H{i:02} {}\nbody\n", ch.to_string().repeat(300)))
                .collect();
            std::fs::write(&path, body).unwrap();
            assert_inline_with_one_handle(&path, &format!(".mdx of {name}")).await;
        }
    }

    #[tokio::test]
    async fn a_file_whose_symbol_names_are_quote_heavy_comes_back_inline_with_one_handle() {
        // Kotlin allows backtick identifiers containing quotes. Each symbol's NAME doubles when
        // serialized, so an array-entry budget measured in raw bytes would overshoot.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Quoted.kt");
        let src: String = (0..400)
            .map(|i| format!("fun `say \"a\" \"b\" \"c\" \"d\" \"e\" {i:03}`() {{}}\n"))
            .collect();
        std::fs::write(&path, &src).unwrap();
        let ctx = test_ctx().await;
        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let names: Vec<&str> = value["symbols"]
            .as_array()
            .unwrap_or_else(|| panic!("no symbols in {value:.300}"))
            .iter()
            .filter_map(|s| s["name"].as_str())
            .collect();
        assert!(
            names.iter().any(|n| n.contains('"')),
            "the fixture must reach the summary with quotes in its names: {names:.5?}"
        );

        assert_inline_with_one_handle(&path, "quote-heavy symbol names").await;
    }

    #[tokio::test]
    async fn a_wide_toml_shows_its_gap_line_between_the_kept_sections() {
        // 40 `[table]` headers of 600 B: the `sections` list, 30 entries after its own cap.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gap.toml");
        let body: String = (0..40)
            .map(|i| format!("[{}{i:02}]\nv = 1\n", "t".repeat(600)))
            .collect();
        std::fs::write(&path, &body).unwrap();
        let text = read_text(&path).await;
        assert!(text.contains("sections omitted"), "{text:.600}");
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }

    #[tokio::test]
    async fn a_wide_flat_toml_shows_its_gap_line_between_the_kept_keys() {
        // No table headers: the summary lists top-level `keys`, capped at 20 by the summarizer.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("flat.toml");
        let body: String = (0..20)
            .map(|i| format!("{}{i:02} = 1\n", "k".repeat(600)))
            .collect();
        std::fs::write(&path, &body).unwrap();
        let text = read_text(&path).await;
        assert!(text.contains("keys omitted"), "{text:.600}");
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
    }

    #[tokio::test]
    async fn a_wide_mdx_shows_its_gap_line_between_the_kept_headings() {
        // `.mdx` is a Markdown SUMMARY type that `read_file` does not route to the heading-map
        // reader, so it reaches the `markdown` branch of the summary renderer.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gap.mdx");
        let body: String = (1..=35)
            .map(|i| format!("# H{i:02} {}\nbody\n", "w".repeat(400)))
            .collect();
        std::fs::write(&path, &body).unwrap();
        let text = read_text(&path).await;
        assert!(text.contains("headings omitted"), "{text:.600}");
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
    }

    // ---- read_markdown's oversized tier: the heading map is bounded by bytes ----
    //
    // `HEADINGS_HARD_CAP` (40) is a TRIGGER into the oversized tier, not a cap on what the
    // tier returns: it then lists EVERY heading beside `file_id`. Its size is the headings'
    // text, so the overflow point depends on heading width, not on a heading count. A file of
    // 200 ordinary 60-character headings is ~15 KB of map.

    fn markdown_with_headings(n: usize, width: usize) -> String {
        (1..=n)
            .map(|i| format!("## Section {i:03} {}\nbody {i}\n\n", "w".repeat(width)))
            .collect()
    }

    #[tokio::test]
    async fn a_markdown_file_with_many_headings_is_mapped_inline_with_one_handle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("many.md");
        std::fs::write(&path, markdown_with_headings(200, 60)).unwrap();

        let text = read_text(&path).await;

        assert!(
            !text.contains("@tool_"),
            "a second handle was minted: {text:.300}"
        );
        assert_eq!(handles_in(&text).len(), 1, "{text:.300}");
        assert!(
            text.contains("Section 001"),
            "the FIRST heading must survive"
        );
        assert!(
            text.contains("Section 200"),
            "the LAST heading must survive"
        );
        assert!(
            text.contains("entries omitted"),
            "a cut must say so: {text:.300}"
        );
        assert!(
            text.contains("headings omitted"),
            "and show where: {text:.300}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "{} B",
            text.len()
        );
    }

    #[tokio::test]
    async fn a_markdown_heading_map_that_fits_is_returned_whole() {
        // 60 short headings: over HEADINGS_HARD_CAP (so the oversized tier), but a ~1.5 KB map.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.md");
        std::fs::write(&path, markdown_with_headings(60, 4)).unwrap();
        let ctx = test_ctx().await;

        let result = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert_eq!(
            result["headings"].as_array().unwrap().len(),
            60,
            "{result:.300}"
        );
        assert!(result.get("headings_truncated").is_none(), "{result:.300}");
        assert!(!result["hint"].as_str().unwrap().contains("omitted"));
    }
    #[tokio::test]
    async fn the_cut_heading_map_shows_its_gap_between_the_two_halves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gapmap.md");
        std::fs::write(&path, markdown_with_headings(200, 60)).unwrap();
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let gap = &result["headings_omitted"];
        let after = gap["after"].as_u64().unwrap() as usize;
        let from = gap["from_line"].as_u64().unwrap();
        let to = gap["to_line"].as_u64().unwrap();
        let headings = result["headings"].as_array().unwrap();
        // Heading i (1-based) is on line 3i-2, so the gap spans the line AFTER the last kept
        // head heading up to the line before the first kept tail heading.
        assert_eq!(from, headings[after - 1]["l"].as_u64().unwrap() + 3);
        assert_eq!(to + 1, headings[after]["l"].as_u64().unwrap());

        let rendered = ReadFile
            .format_compact(&result)
            .expect("a markdown result renders");
        let lines: Vec<&str> = rendered.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.contains("headings omitted"))
            .unwrap_or_else(|| panic!("no gap line in: {rendered:.600}"));
        assert!(
            lines[at].contains(&format!("(L{from}-L{to})")),
            "{}",
            lines[at]
        );
        assert!(
            lines[at - 1].contains(headings[after - 1]["h"].as_str().unwrap()),
            "the entry before the gap must be the last kept head heading: {}",
            lines[at - 1]
        );
        assert!(
            lines[at + 1].contains(headings[after]["h"].as_str().unwrap()),
            "the entry after the gap must be the first kept tail heading: {}",
            lines[at + 1]
        );
    }

    #[tokio::test]
    async fn a_default_markdown_read_marks_every_heading_seen_so_no_unread_list_is_returned() {
        // PINS A REACHABILITY FACT the tier-3 bound relies on. A default (whole-file) read marks
        // every heading as seen, so `markdown_coverage` has nothing unread to report even after
        // a section was read first: no `coverage.unread` list can sit beside the heading map and
        // double its size. (If this ever fails, the bound still cuts a nested array; this test
        // only says the case just became reachable.)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cov.md");
        std::fs::write(&path, markdown_with_headings(200, 60)).unwrap();
        let ctx = test_ctx().await;
        ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "heading": "## Section 001" }),
                &ctx,
            )
            .await
            .unwrap();

        let value = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            value["headings"].as_array().unwrap().len() > 10,
            "{value:.200}"
        );
        assert!(
            value.get("coverage").is_none(),
            "{:.300}",
            value["coverage"]
        );
    }

    /// Bug 2026-08-25-file-slice-handle-refreshes-to-whole-file: the
    /// `@file_*` handle returned for an oversized RANGE is minted with
    /// `source_path` pointing at the whole file, so the first `get()` after
    /// an mtime bump replaces the excerpt with the file's entire contents —
    /// under a handle whose `shown_lines`/`total_lines` still describe the
    /// range, and which the caller was handed in order to grep the range.
    ///
    /// Measured 2026-08-25 against the live server: a handle minted as 12
    /// lines reported 41 and served the file's line 1.
    #[tokio::test]
    async fn ranged_read_handle_stays_the_range_after_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        let lines: Vec<String> = (1..=40)
            .map(|i| format!("line {i:04} {}", "x".repeat(900)))
            .collect();
        std::fs::write(&path, lines.join("\n")).unwrap();
        let ctx = test_ctx().await;

        let result = ReadFile
            .call(
                json!({ "path": path.to_str().unwrap(), "start_line": 13, "end_line": 24 }),
                &ctx,
            )
            .await
            .unwrap();
        let file_id = result["file_id"]
            .as_str()
            .unwrap_or_else(|| panic!("oversized range should be buffered: {result}"))
            .to_string();

        // Replace the file and push its mtime past the entry's timestamp —
        // the exact trigger `get_with_refresh_flag` watches for.
        std::fs::write(&path, "REPLACED\n").unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
        filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future)).unwrap();

        let entry = ctx
            .output_buffer
            .get(&file_id)
            .expect("the excerpt handle should still resolve");
        assert!(
            !entry.stdout.contains("REPLACED"),
            "an excerpt handle must not absorb content from outside the range \
                 it was minted for; got: {:?}",
            entry.stdout.chars().take(80).collect::<String>()
        );
        assert_eq!(
            entry.stdout.lines().count(),
            12,
            "the handle was minted as lines 13-24 and must stay 12 lines"
        );
    }

    /// Bug 2026-08-25-run-command-nested-buffer-recursion, second mechanism —
    /// the one that produces the "recurses into meta-wrappers" headline.
    ///
    /// `INLINE_BYTE_BUDGET` caps the RAW chunk at 90% of
    /// `TOOL_OUTPUT_BUFFER_THRESHOLD`, but the threshold that decides whether
    /// `call_content` re-wraps a response applies to the SERIALIZED JSON.
    /// Inside a JSON string every `\n` costs two bytes, so the per-line
    /// escaping charge scales with line count and eats the whole 10% of
    /// headroom the constant's doc comment budgets for key names.
    ///
    /// Measured 2026-08-25 against the live server: a 1200-line buffer read
    /// as one range produced `buffered_bytes: 10169` — the caller got a
    /// `@tool_*` envelope instead of their lines, and its hint sent them to
    /// `json_path="$.content"`, which peels to another `@file_*`, which
    /// slices to another `@tool_*`. That is the reported chain.
    #[tokio::test]
    async fn read_file_buffer_range_chunk_fits_the_threshold_it_is_measured_against() {
        let lines: Vec<String> = (1..=1200).map(|i| format!("ln {i:05}")).collect();
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", lines.join("\n"));

        let result = ReadFile
            .call(
                json!({ "path": &buf_id, "start_line": 1, "end_line": 1200 }),
                &ctx,
            )
            .await
            .unwrap();

        let serialized = serde_json::to_string(&result).unwrap().len();
        assert!(
            serialized <= crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "a paginated response must fit the threshold it is measured \
                 against, or call_content re-wraps it and the caller gets an \
                 envelope instead of lines; got {serialized} bytes vs {}",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
        );
        assert!(
            result["content"]
                .as_str()
                .is_some_and(|c| c.starts_with("ln 00001")),
            "the chunk should still start at the requested line: {result}"
        );
    }

    /// Same arithmetic, the whole-buffer branch — it inlines a chunk against
    /// the same budget and is measured against the same threshold.
    #[tokio::test]
    async fn read_file_buffer_full_chunk_fits_the_threshold_it_is_measured_against() {
        let lines: Vec<String> = (1..=1200).map(|i| format!("ln {i:05}")).collect();
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", lines.join("\n"));

        let result = ReadFile
            .call(json!({ "path": &buf_id }), &ctx)
            .await
            .unwrap();

        let serialized = serde_json::to_string(&result).unwrap().len();
        assert!(
            serialized <= crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "a paginated response must fit the threshold it is measured \
                 against; got {serialized} bytes vs {}",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
        );
    }

    /// The third arm of the same contract, and the one the two above cannot
    /// reach: a chunk that is ONE line wider than the whole budget.
    ///
    /// `read_from_buffer`'s doc comment promises it "never re-wraps its own
    /// result in a `@tool_*` envelope". The safety valve in
    /// `extract_lines_with_cost` always yields at least one line — deliberately,
    /// to stop an agent re-requesting the same range forever — so when a single
    /// line exceeds the budget it is emitted whole and the promise breaks.
    ///
    /// Both sibling tests use 1200 SHORT lines. That fixture can never reach the
    /// valve: the budget stops it at a line boundary long before any one line is
    /// oversized. They assert exactly the property under test here and would
    /// both stay green with this defect present — which is why this arm is
    /// written with a fixture whose premise is asserted rather than assumed.
    ///
    /// Real shape: a `run_command` envelope pretty-prints to 4 lines, of which line 3 is
    /// the entire stdout as one JSON-escaped string. Since the two coordinate spaces were
    /// unified behind `line_addressable_text`, a stdout CONTAINING newlines expands into many
    /// short lines and no longer reaches this valve — so the shape that does is stdout with
    /// no newline in it: a captured one-line API response, a base64 blob, or any `@cmd_*`
    /// buffer, which is not expanded at all. See
    /// `docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`.
    #[tokio::test]
    async fn read_file_buffer_single_oversized_line_still_fits_the_threshold() {
        // LOAD-BEARING SEPARATOR — a space, not `\n`. `line_addressable_text` materializes
        // escaped newlines, so a `\n`-joined stdout becomes 1200 short lines and this fixture
        // goes inert. That is not hypothetical: it is what the guard below caught the moment
        // grep's and read_file's coordinate spaces were unified.
        let stdout = (1..=1200)
            .map(|i| format!("row {i:05}"))
            .collect::<Vec<_>>()
            .join(" ");
        let envelope = json!({ "exit_code": 0, "stdout": stdout }).to_string();
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", envelope);

        // Premise of the fixture, asserted rather than assumed: in the coordinate space the
        // production path actually addresses, the payload really is ONE line wider than the
        // budget. If a future edit makes this fixture many-short-lines, this fails here
        // instead of silently degrading into a copy of the two tests above.
        //
        // DERIVED FROM THE PRODUCTION FUNCTION, not re-typed. This block used to call
        // `to_string_pretty` itself, which made it a second implementation of the coordinate
        // space — it kept passing while the shipped derivation moved underneath it, and the
        // fixture guard went on certifying a premise the production path no longer saw.
        let pretty = {
            let raw = ctx.output_buffer.get(&buf_id).unwrap().stdout;
            crate::tools::output_buffer::line_addressable_text(&buf_id, raw)
        };
        let widest = pretty.lines().map(|l| l.len()).max().unwrap();
        assert!(
            widest > crate::tools::INLINE_BYTE_BUDGET,
            "fixture must contain a single line wider than the whole budget, \
             or it cannot reach the safety valve and proves nothing; widest \
             line is {widest} vs budget {}",
            crate::tools::INLINE_BYTE_BUDGET
        );

        let oversized_lineno = pretty
            .lines()
            .position(|l| l.len() > crate::tools::INLINE_BYTE_BUDGET)
            .unwrap()
            + 1;

        let result = ReadFile
            .call(
                json!({
                    "path": &buf_id,
                    "start_line": oversized_lineno,
                    "end_line": oversized_lineno,
                }),
                &ctx,
            )
            .await
            .unwrap();

        let serialized = serde_json::to_string(&result).unwrap().len();
        assert!(
            serialized <= crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "a single over-budget line must still be paginated to fit, or \
             call_content re-wraps the response and the caller gets an envelope \
             instead of content — the exact outcome read_from_buffer's doc \
             comment rules out; got {serialized} bytes vs {}",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
        );

        // Fitting is not enough on its own: a response that fits by silently
        // dropping the line would pass the assertion above and strand the
        // caller. It has to say the line was cut AND name the way through.
        let rendered = serde_json::to_string(&result).unwrap();
        assert!(
            rendered.contains("json_path"),
            "the response must name the addressing mode that does reach the \
             payload, or the caller has a smaller response and no route: {result}"
        );
    }

    #[test]
    fn normalize_line_nav_aliases_maps_offset_and_limit() {
        let mut input = json!({ "path": "x", "offset": 100, "limit": 50 });
        normalize_line_nav_aliases(&mut input);
        assert_eq!(input["start_line"], json!(100));
        assert_eq!(input["end_line"], json!(149));
    }

    #[test]
    fn normalize_line_nav_aliases_limit_only_defaults_offset_to_one() {
        let mut input = json!({ "path": "x", "limit": 30 });
        normalize_line_nav_aliases(&mut input);
        assert_eq!(input["start_line"], json!(1));
        assert_eq!(input["end_line"], json!(30));
    }

    #[test]
    fn normalize_line_nav_aliases_offset_only_leaves_end_line_unset() {
        let mut input = json!({ "path": "x", "offset": 42 });
        normalize_line_nav_aliases(&mut input);
        assert_eq!(input["start_line"], json!(42));
        assert!(input.get("end_line").is_none());
    }

    #[test]
    fn normalize_line_nav_aliases_explicit_start_line_wins() {
        let mut input = json!({ "path": "x", "start_line": 10, "offset": 100, "limit": 5 });
        normalize_line_nav_aliases(&mut input);
        assert_eq!(input["start_line"], json!(10));
        // The aliases must not overwrite an explicit start_line or inject an end_line.
        assert!(input.get("end_line").is_none());
    }

    #[test]
    fn normalize_line_nav_aliases_noop_without_aliases() {
        let mut input = json!({ "path": "x" });
        normalize_line_nav_aliases(&mut input);
        assert!(input.get("start_line").is_none());
        assert!(input.get("end_line").is_none());
    }

    #[tokio::test]
    async fn read_file_buffer_offset_limit_returns_slice_not_head() {
        // Regression: read_file(@buf, offset=N, limit=M) is native-Read line nav and must
        // return lines N..=N+M-1, NOT silently return the buffer head.
        let lines: Vec<String> = (1..=300).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", content);

        let tool = ReadFile;
        let result = tool
            .call(json!({ "path": buf_id, "offset": 100, "limit": 50 }), &ctx)
            .await
            .unwrap();

        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("line 100") && body.contains("line 149"),
            "offset=100 limit=50 should yield lines 100..=149, got: {body:?}"
        );
        assert!(
            !body.contains("line 99"),
            "window should start at offset (line 100), not the head, got: {body:?}"
        );
        assert!(
            !body.contains("line 150"),
            "window should stop at offset+limit-1 (line 149), got: {body:?}"
        );
    }

    #[tokio::test]
    async fn read_file_buffer_offset_string_typed_maps_to_range() {
        // MCP clients pass offset/limit as strings ("128"); optional_u64_param coerces them.
        let lines: Vec<String> = (1..=300).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", content);

        let tool = ReadFile;
        let result = tool
            .call(
                json!({ "path": buf_id, "offset": "200", "limit": "10" }),
                &ctx,
            )
            .await
            .unwrap();

        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("line 200") && body.contains("line 209"),
            "offset=\"200\" limit=\"10\" should yield lines 200..=209, got: {body:?}"
        );
        assert!(
            !body.contains("line 199") && !body.contains("line 210"),
            "string-typed offset/limit must map to the exact window, got: {body:?}"
        );
    }

    /// A source file whose first lines carry imports AND symbol declarations —
    /// the shape that makes the overlap gate fire on a plain "show me the
    /// imports" read. `mod` declarations and the struct all begin inside the
    /// first 20 lines, so `find_symbols_for_range(1, 20)` is non-empty.
    fn head_read_fixture() -> &'static str {
        "\
use std::collections::HashMap;
use std::path::Path;

mod helpers;
mod util;

/// Config for the thing.
pub struct Config {
    pub name: String,
    pub value: u64,
}

impl Config {
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: 0,
        }
    }
}
"
    }

    async fn ctx_with_file(dir: &std::path::Path, name: &str, body: &str) -> ToolContext {
        std::fs::create_dir_all(dir.join(".codescout")).unwrap();
        std::fs::write(dir.join(name), body).unwrap();
        let mut ctx = test_ctx().await;
        ctx.agent = Agent::new(Some(dir.to_path_buf())).await.unwrap();
        ctx
    }

    const MD_FIXTURE: &str = "\
# Title

## A
line a1
line a2
line a3
line a4
line a5
line a6
line a7
line a8
line a9
line a10

## B
line b1
line b2
line b3
line b4
line b5
line b6
line b7
line b8
line b9
line b10
";

    #[tokio::test]
    async fn read_file_on_markdown_returns_the_heading_map_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let out = ReadFile
            .call(json!({"path": "notes.md"}), &ctx)
            .await
            .unwrap();
        assert_eq!(out["format"], "markdown");
        assert!(out["headings"].is_array(), "{out}");
    }

    #[tokio::test]
    async fn read_file_on_markdown_serves_heading_and_headings() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let one = ReadFile
            .call(json!({"path": "notes.md", "heading": "## A"}), &ctx)
            .await
            .unwrap();
        let one_content = one["content"].as_str().unwrap();
        // The whole-file default tier ALSO contains "## A" (it contains everything),
        // so that assertion alone is monotone under widening — a mutation that killed
        // the single-heading dispatch branch and fell through to the full file passed
        // it silently. Assert the heading-scoped read actually scoped: it must exclude
        // the sibling section.
        assert!(one_content.contains("## A"), "{one}");
        assert!(
            !one_content.contains("## B"),
            "heading=\"## A\" leaked the sibling section, so this did not scope: {one}"
        );
        let two = ReadFile
            .call(
                json!({"path": "notes.md", "headings": ["## A", "## B"]}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(two["sections"].as_array().map(|s| s.len()), Some(2));
    }

    #[tokio::test]
    async fn read_file_on_markdown_honours_offset_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let out = ReadFile
            .call(json!({"path": "notes.md", "offset": 5, "limit": 3}), &ctx)
            .await
            .unwrap();
        let content = out["content"].as_str().unwrap();
        assert_eq!(content.lines().count(), 3, "{content}");
        assert!(
            out.get("headings").is_none(),
            "a line range must not return the heading map"
        );
    }

    #[tokio::test]
    async fn read_file_force_on_markdown_is_a_raw_line_range() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let out = ReadFile
            .call(
                json!({"path": "notes.md", "start_line": 1, "end_line": 2, "force": true}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            out.get("format").is_none(),
            "force skips the markdown path: {out}"
        );
    }

    #[tokio::test]
    async fn heading_on_a_non_markdown_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "main.rs", "fn main() {}\n").await;
        let err = ReadFile
            .call(json!({"path": "main.rs", "heading": "## A"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("markdown"), "{err}");
    }

    #[tokio::test]
    async fn read_file_refuses_a_managed_ledger_and_names_doc() {
        let text = "---\nid: '0123456789abcdef'\nentry_prefix: R\n---\n## R-1 — x\n";
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/trackers")).unwrap();
        let ctx = ctx_with_file(dir.path(), "docs/trackers/x.md", text).await;
        let err = ReadFile
            .call(json!({"path": "docs/trackers/x.md"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("doc(action=\"get\""), "{err}");
    }

    // F1: `force=true` used to route around the markdown dispatch above — and with it,
    // the librarian guard that dispatch calls — landing on the raw line-range path with
    // no guard of its own. Regression for
    // docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md's
    // read twin: a managed ledger's frontmatter came back verbatim under `force=true`.
    #[tokio::test]
    async fn read_file_force_true_on_a_managed_ledger_is_still_refused() {
        let text = "---\nid: '0123456789abcdef'\nentry_prefix: R\n---\n## R-1 — x\n";
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/trackers")).unwrap();
        let ctx = ctx_with_file(dir.path(), "docs/trackers/x.md", text).await;
        let err = ReadFile
            .call(
                json!({
                    "path": "docs/trackers/x.md",
                    "start_line": 1,
                    "end_line": 5,
                    "force": true
                }),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("doc(action=\"get\""), "{err}");
    }

    // F2: `is_markdown_target` lowercases before comparing the extension, but
    // `resolve_markdown_source`'s own gate didn't — so `read_file` routed an uppercase
    // `.MD` path INTO the markdown dispatch, which then refused it as "not a markdown
    // file". Regression for that mismatch.
    #[tokio::test]
    async fn read_file_on_uppercase_md_extension_is_read_as_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "NOTES.MD", MD_FIXTURE).await;
        let out = ReadFile
            .call(json!({"path": "NOTES.MD"}), &ctx)
            .await
            .unwrap();
        assert_eq!(out["format"], "markdown", "{out}");
    }

    // M1a: `is_markdown_target`'s `@file_` branch (read_markdown.rs's
    // `is_markdown_target`) is the path Iron Law 6 tells agents to use for a second,
    // heading-scoped read of a large markdown file already buffered by a first read
    // (`read_file("@file_ref", heading="## Section")`). Nothing exercised it.
    // M1a: `is_markdown_target`'s `@file_` branch (read_markdown.rs's
    // `is_markdown_target`) is the path Iron Law 6 tells agents to use for a second,
    // heading-scoped read of a large markdown file already buffered by a first read
    // (`read_file("@file_ref", heading="## Section")`). Nothing exercised it. No
    // `heading`/`headings` param here on purpose — those alone force the markdown
    // dispatch regardless of `is_markdown_target`'s answer, which would make this
    // test pass even with the `@file_` branch dead.
    // M1a: `is_markdown_target`'s `@file_` branch (read_markdown.rs's
    // `is_markdown_target`) is the path Iron Law 6 tells agents to use for a second,
    // heading-scoped read of a large markdown file already buffered by a first read
    // (`read_file("@file_ref", heading="## Section")`). Nothing exercised it. No
    // `heading`/`headings` param here on purpose — those alone force the markdown
    // dispatch regardless of `is_markdown_target`'s answer, which would make this
    // test pass even with the `@file_` branch dead.
    #[tokio::test]
    async fn read_file_on_a_markdown_backed_file_buffer_dispatches_to_markdown_read() {
        let dir = tempfile::tempdir().unwrap();
        // `store_file` stats `source_path` on every `get()` for mtime-based
        // auto-refresh (output_buffer.rs), so the path must exist on disk or the
        // entry is evicted before `is_markdown_target` ever sees it.
        let file_path = dir.path().join("notes.md");
        std::fs::write(&file_path, MD_FIXTURE).unwrap();
        let ctx = test_ctx().await;
        let buf_id = ctx
            .output_buffer
            .store_file(file_path.to_string_lossy().into_owned(), MD_FIXTURE.into());
        let out = ReadFile.call(json!({"path": buf_id}), &ctx).await.unwrap();
        assert_eq!(
            out["format"], "markdown",
            "@file_ ref backed by a .md source_path should dispatch to markdown::read: {out}"
        );
    }

    /// Without this guard the markdown route would swallow `json_path` silently — a new
    /// instance of the very class Task 7 closes for `offset`/`limit`.
    #[tokio::test]
    async fn json_path_on_markdown_is_refused_not_silently_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let err = ReadFile
            .call(json!({"path": "notes.md", "json_path": "$.a"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("only supported for JSON"), "{err}");
    }

    /// M1b: sibling of `json_path_on_markdown_is_refused_not_silently_ignored` for the
    /// `toml_key` half of `wants_format` — same guard, untested until now. Without it
    /// the markdown route would swallow `toml_key` silently on a `.md` file instead of
    /// falling through to the typed-format error.
    #[tokio::test]
    async fn toml_key_on_markdown_is_refused_not_silently_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let err = ReadFile
            .call(json!({"path": "notes.md", "toml_key": "a"}), &ctx)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("only supported for TOML"), "{err}");
    }

    /// Step 1 of the IL1 fix: a file-head read is the canonical "show me the
    /// imports" operation, and the gate's recommended recovery
    /// (`symbols(include_body=true)`) is STRUCTURALLY incapable of serving it —
    /// `symbols` is a definition projection and does not return `use` lines.
    /// Refusing it costs the caller a round trip and offers `force=true` only
    /// second. Measured: 84 of 244 refused reads carried `start_line <= 5`, and
    /// 69 of those ended by line 60.
    ///
    /// The mutation this catches: deleting the head-read exemption restores the
    /// refusal on the single largest recoverable population of this error class.
    #[tokio::test]
    async fn head_read_of_imports_is_allowed_though_symbols_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "cfg.rs", head_read_fixture()).await;

        let result = ReadFile
            .call(
                json!({ "path": "cfg.rs", "start_line": 1, "end_line": 20 }),
                &ctx,
            )
            .await
            .expect("a file-head read must not be refused by the overlap gate");

        let body = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            body.contains("use std::collections::HashMap"),
            "the head read must return the imports it asked for, got: {result}"
        );
    }

    /// The exemption must NOT become a general hole. A read that overlaps a
    /// symbol but does not start at the file head is still refused — this is the
    /// symbol-body population the traced sequences show the gate genuinely helps.
    #[tokio::test]
    async fn non_head_read_overlapping_a_symbol_is_still_refused() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "cfg.rs", head_read_fixture()).await;

        let err = ReadFile
            .call(
                json!({ "path": "cfg.rs", "start_line": 13, "end_line": 20 }),
                &ctx,
            )
            .await
            .expect_err("a mid-file read overlapping a symbol must still be refused");

        assert!(
            err.to_string().contains("overlaps named symbol"),
            "expected the overlap refusal, got: {err}"
        );
    }

    /// The exemption is bounded by extent, not just by start line: a read that
    /// begins at line 1 but runs past the window is a whole-file read wearing a
    /// head read's clothes, and Iron Law 1 exists for exactly that.
    #[tokio::test]
    async fn head_read_past_the_window_is_still_refused() {
        let dir = tempfile::tempdir().unwrap();
        let long = format!("{}\n{}", head_read_fixture(), "// filler\n".repeat(80));
        let ctx = ctx_with_file(dir.path(), "cfg.rs", &long).await;

        let err = ReadFile
            .call(
                json!({ "path": "cfg.rs", "start_line": 1, "end_line": 61 }),
                &ctx,
            )
            .await
            .expect_err("a head read past the window must still be refused");

        assert!(
            err.to_string().contains("overlaps named symbol"),
            "expected the overlap refusal, got: {err}"
        );
    }

    /// A symbol spanning ~102 lines, starting past the head-read window.
    fn large_symbol_fixture() -> String {
        let mut s = String::from("// leading comment\n\npub fn big() {\n");
        for i in 0..100 {
            s.push_str(&format!("    let x{i} = {i};\n"));
        }
        s.push_str("}\n");
        s
    }

    /// A small symbol, placed past the head-read window so the exemption does
    /// not apply and the gate actually fires.
    fn small_symbol_fixture() -> String {
        let mut s = String::new();
        for _ in 0..20 {
            s.push_str("// filler\n");
        }
        s.push_str("pub fn small() {\n    let a = 1;\n    let b = 2;\n}\n");
        s
    }

    /// Step 2 of the IL1 fix. When the caller asks for a small slice of a large
    /// symbol, leading the hint with `symbols(include_body=true)` recommends a
    /// call that returns STRICTLY MORE than was requested — the opposite of Iron
    /// Law 1's intent, which is to stop oversized source reads. The requested
    /// extent is known at refusal time, so the hint can order itself by it.
    ///
    /// Mutation caught: dropping the extent comparison restores a hint that
    /// pushes a 5-line request toward a 102-line response.
    #[tokio::test]
    async fn hint_leads_with_force_for_a_small_slice_of_a_large_symbol() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "big.rs", &large_symbol_fixture()).await;

        let err = ReadFile
            .call(
                json!({ "path": "big.rs", "start_line": 50, "end_line": 54 }),
                &ctx,
            )
            .await
            .expect_err("a mid-symbol read must still be refused");
        let msg = err.to_string();

        let force_at = msg.find("force=true").expect("hint must offer force=true");
        let symbols_at = msg
            .find("symbols(name=")
            .expect("hint must still name symbols");
        assert!(
            force_at < symbols_at,
            "for a 5-line slice of a ~102-line symbol the hint must LEAD with \
                 force=true, since symbols(include_body=true) returns ~20x what was \
                 asked for. Got: {msg}"
        );
    }

    /// The converse, so the reordering is conditional rather than a blanket
    /// preference for `force=true`: when the symbol is not much larger than the
    /// requested range, `symbols(include_body=true)` is the better answer and
    /// must stay first.
    #[tokio::test]
    async fn hint_leads_with_symbols_when_the_symbol_is_not_much_larger() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "small.rs", &small_symbol_fixture()).await;

        let err = ReadFile
            .call(
                json!({ "path": "small.rs", "start_line": 22, "end_line": 23 }),
                &ctx,
            )
            .await
            .expect_err("a mid-symbol read must still be refused");
        let msg = err.to_string();

        let symbols_at = msg.find("symbols(name=").expect("hint must name symbols");
        let force_at = msg
            .find("force=true")
            .expect("hint must still offer force=true");
        assert!(
            symbols_at < force_at,
            "when the symbol is close in size to the request, symbols() must stay \
                 the leading suggestion. Got: {msg}"
        );
    }
}

#[cfg(test)]
#[path = "read_file_buffer_edge_tests.rs"]
mod buffer_edge_tests;
