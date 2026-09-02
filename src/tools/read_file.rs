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

    fn description(&self) -> &str {
        "Read a file. Large output → @file_* buffer. Format-aware: json_path \
         (JSON), toml_key (TOML/YAML). Use read_markdown for .md. Source files: \
         a line range overlapping a symbol redirects to symbols(include_body=true); \
         pass force=true to bypass."
    }

    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "anyOf": [
                { "required": ["path"] },
                { "required": ["file_path"] },
                { "required": ["output_id"] }
            ],
            "properties": {
                "path": { "type": "string", "description": "File path relative to project root" },
                "file_path": { "type": "string", "description": "Alias for path" },
                "output_id": { "type": "string", "description": "Alias for path — pass a returned @tool_*/@cmd_*/@file_* buffer handle here to read it back." },
                "start_line": { "type": "integer", "description": "First line (1-indexed). Pair with end_line." },
                "end_line": { "type": "integer", "description": "Last line (1-indexed, inclusive). Pair with start_line." },
                "offset": { "type": "integer", "description": "Native-Read-style alias: 1-indexed start line (= start_line). Ignored when start_line/end_line are set." },
                "limit": { "type": "integer", "description": "Native-Read-style alias: line count from offset (end_line = offset + limit - 1). offset defaults to line 1 if omitted." },
                "json_path": { "type": "string", "description": "JSON subtree by path (e.g. \"$.dependencies\")." },
                "toml_key": { "type": "string", "description": "TOML table or YAML section by key (e.g. \"dependencies\")." },
                "force": { "type": "boolean", "description": "Skip source-symbol hint and read the raw line range. Line ranges only — an oversized whole-file read is summarised either way." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        // Native-`Read` compatibility: callers habitually pass offset/limit (a 1-indexed
        // start line + a line count, the built-in Read signature). Normalize to
        // start_line/end_line up front — before the buffer fork — so both the buffer and
        // real-file paths honor them through the same line-range logic instead of
        // silently returning the file head.
        let mut input = input;
        normalize_line_nav_aliases(&mut input);

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
                    "read_file(path=\"src/x.rs\") — or read a buffer: read_file(path=\"@tool_abc\"). Aliases: file_path, relative_path, file, output_id.",
                )
            })?;
        let path = strip_buffer_ref_quotes(raw_path);

        // Buffer refs bypass the filesystem entirely.
        if path.starts_with("@file_") || path.starts_with("@cmd_") || path.starts_with("@tool_") {
            let mut res = read_from_buffer(path, &input, ctx)?;
            // Same contract as run_command's `buffer_truncated`: a handle whose buffer
            // holds only a prefix says so at EVERY read, not just on the response that
            // minted it. Attached here rather than inside `read_from_buffer` because
            // that function has several return shapes (json_path extraction, a line
            // slice, a re-parked @file_* handle) and the notice belongs on all of them.
            if let Some(notice) = ctx.output_buffer.truncation_notice(path) {
                if let Some(obj) = res.as_object_mut() {
                    obj.insert("buffer_truncated".into(), json!([notice]));
                }
            }
            return Ok(res);
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

        // Gate: redirect .md files to read_markdown
        if resolved.extension().is_some_and(|e| e == "md") {
            return Err(RecoverableError::with_hint(
                "Use read_markdown for markdown files",
                "read_markdown provides heading-based navigation, size-adaptive output, and buffer-ref slicing for .md files.",
            )
            .into());
        }

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

        if let Some(jp) = input["json_path"].as_str() {
            return read_json_path_nav(&text, &resolved, jp);
        }
        if let Some(tk) = input["toml_key"].as_str() {
            return read_toml_yaml_key(&text, &resolved, tk);
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
        Some(format_read_file(result))
    }
    fn json_path_hint(&self, val: &Value) -> String {
        // Buffered read results carry the payload under `content` (line ranges,
        // toml_key/json_path extractions, full reads). Point agents there rather
        // than the generic default `$.field`.
        if val["content"].is_string() {
            "$.content".to_string()
        } else {
            "$.field".to_string()
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
    let raw = ctx
        .output_buffer
        .get(path)
        .ok_or_else(|| {
            RecoverableError::with_hint(
                format!("buffer reference not found: '{}'", path),
                "Buffer refs expire when the session resets. Re-run the command to get a fresh ref.",
            )
        })?
        .stdout;

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

    // @tool_* refs contain compact single-line JSON — pretty-print so
    // start_line/end_line navigation and json_path extraction are useful.
    let text: String = if path.starts_with("@tool_") {
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or(raw)
    } else {
        raw
    };

    // json_path navigation is only meaningful for @tool_* (always JSON).
    if path.starts_with("@tool_") {
        if let Some(jp) = input["json_path"].as_str() {
            let (content, type_name, count) =
                crate::tools::file_summary::extract_json_path(&text, jp)?;
            let mut result = if crate::tools::exceeds_inline_limit(&content) {
                let line_count = content.lines().count().max(1);
                let file_id = ctx
                    .output_buffer
                    .store_file(format!("{path}:{jp}"), content);
                json!({
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
                })
            } else {
                json!({
                    "content": content,
                    "path": jp,
                    "value_type": type_name,
                    "format": "json",
                })
            };
            if let Some(c) = count {
                result["count"] = json!(c);
            }
            return Ok(result);
        }
    }

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
        if crate::tools::exceeds_inline_limit(&content) {
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
            let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_json_budget(
                &content,
                1,
                usize::MAX,
                crate::tools::INLINE_BYTE_BUDGET,
            );
            // The valve above yields one line even when that line alone busts the
            // budget; without this the response exceeds the threshold it is
            // measured against and gets re-wrapped.
            let (chunk, line_truncated) =
                clamp_over_budget_line(chunk, crate::tools::INLINE_BYTE_BUDGET);
            let orig_start = s as usize;
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
            return Ok(result);
        }
        return Ok(json!({ "content": content, "total_lines": total_lines }));
    }

    // Full buffer: paginate if over the inline limit. Never re-buffer.
    if crate::tools::exceeds_inline_limit(&text) {
        let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_json_budget(
            &text,
            1,
            usize::MAX,
            crate::tools::INLINE_BYTE_BUDGET,
        );
        // Same valve, same consequence — see the range branch above.
        let (chunk, line_truncated) =
            clamp_over_budget_line(chunk, crate::tools::INLINE_BYTE_BUDGET);
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
        return Ok(result);
    }
    Ok(json!({ "content": text, "total_lines": total_lines }))
}

/// Cut a chunk down when a SINGLE line is wider than the whole inline budget.
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
/// Measured 2026-08-29: a `run_command` envelope pretty-prints to four lines, of
/// which the third is the entire stdout as one JSON-escaped string 9998 bytes
/// wide. Line-slicing could never address it. See
/// `docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`.
fn clamp_over_budget_line(chunk: String, budget: usize) -> (String, bool) {
    if !crate::tools::exceeds_inline_limit(&chunk) {
        return (chunk, false);
    }
    // Half the budget, not all of it. The kept bytes are re-measured AFTER JSON
    // escaping and alongside the response's other keys, and an escape-heavy line
    // can nearly double in width. Undershooting costs a few hundred bytes of
    // preview; overshooting reinstates the wrap this function exists to prevent.
    let keep = crate::tools::floor_char_boundary(&chunk, budget / 2);
    let mut out = chunk[..keep].to_string();
    out.push_str("\n…[truncated: this line is wider than the inline budget]");
    (out, true)
}

/// The advisory attached whenever [`clamp_over_budget_line`] cuts.
///
/// Kept separate from the cut so the wording is testable without a
/// `ToolContext`, and so both call sites phrase it identically. It names
/// `json_path` because on a `@tool_*` ref an over-budget line is almost always a
/// JSON-escaped payload, and field addressing reaches it in one call where line
/// addressing cannot reach it at all.
fn over_budget_line_hint(path: &str) -> String {
    if path.starts_with("@tool_") {
        format!(
            "A single line here is wider than the inline budget, so it is shown truncated. \
             On a @tool_* ref that is usually a JSON-escaped payload on one line — address the \
             field instead of the line: read_file(\"{path}\", json_path=\"$.stdout\") for a \
             run_command envelope, or json_path=\"$.<field>\" generally. \
             run_command(\"grep PATTERN {path}\") also searches it."
        )
    } else {
        format!(
            "A single line here is wider than the inline budget, so it is shown truncated. \
             Search it instead of slicing it: run_command(\"grep PATTERN {path}\"). \
             For JSON content, read_file(\"{path}\", json_path=\"$.<field>\") addresses fields \
             rather than lines."
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
fn read_json_path_nav(text: &str, resolved: &std::path::Path, jp: &str) -> Result<Value> {
    let file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    if !matches!(file_type, crate::tools::file_summary::FileSummaryType::Json) {
        return Err(RecoverableError::with_hint(
            "json_path parameter is only supported for JSON files",
            "For Markdown files use read_markdown, for TOML/YAML use toml_key",
        )
        .into());
    }
    let (content, type_name, count) = crate::tools::file_summary::extract_json_path(text, jp)?;
    let mut result = json!({
        "content": content,
        "path": jp,
        "value_type": type_name,
        "format": "json",
    });
    if let Some(c) = count {
        result["count"] = json!(c);
    }
    Ok(result)
}

/// Handle `toml_key` navigation for TOML and YAML files.
fn read_toml_yaml_key(text: &str, resolved: &std::path::Path, tk: &str) -> Result<Value> {
    let mut file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
    // Cargo.lock (and most `.lock` files) are TOML, but detect_file_type
    // classifies `.lock` as Config. Coerce to TOML so toml_key works; a
    // non-TOML `.lock` (e.g. yarn.lock) surfaces a clear parse error below.
    if resolved.to_string_lossy().to_lowercase().ends_with(".lock") {
        file_type = crate::tools::file_summary::FileSummaryType::Toml;
    }
    match file_type {
        crate::tools::file_summary::FileSummaryType::Toml => {
            let result = crate::tools::file_summary::extract_toml_key(text, tk)?;
            Ok(json!({
                "content": result.content,
                "line_range": [result.line_range.0, result.line_range.1],
                "breadcrumb": result.breadcrumb,
                "siblings": result.siblings,
                "format": "toml",
            }))
        }
        crate::tools::file_summary::FileSummaryType::Yaml => {
            let result = crate::tools::file_summary::extract_yaml_key(text, tk)?;
            Ok(json!({
                "content": result.content,
                "line_range": [result.line_range.0, result.line_range.1],
                "breadcrumb": result.breadcrumb,
                "siblings": result.siblings,
                "format": "yaml",
            }))
        }
        _ => Err(RecoverableError::with_hint(
            "toml_key parameter is only supported for TOML and YAML files",
            "For Markdown files use read_markdown, for JSON use json_path",
        )
        .into()),
    }
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

    // Proactive buffering: oversized extracted ranges are stored as @file_* refs
    // so callers can navigate by line number (BUG-025 class).
    if crate::tools::exceeds_inline_limit(&content) {
        let file_id = ctx
            .output_buffer
            .store_file_excerpt(resolved.to_string_lossy().to_string(), content.clone());
        let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_json_budget(
            &content,
            1,
            usize::MAX,
            crate::tools::INLINE_BYTE_BUDGET,
        );
        let orig_start = start as usize;
        let orig_end = orig_start + lines_shown.saturating_sub(1);
        let mut result = json!({
            "content": chunk,
            "file_id": file_id,
            "total_lines": file_total_lines,
            "shown_lines": [orig_start, orig_end],
            "complete": complete,
        });
        if !complete {
            // Continue against the file itself, in the same line numbers
            // `shown_lines` just reported — a `next` phrased in the slice buffer's
            // own 1-based frame is off by `start - 1` and re-serves seen lines.
            //
            // A continuation is a SUBrange of a range the overlap gate already
            // allowed, so it cannot newly trip that gate — with one exception: the
            // head-read exemption turns on `start == 1`, which a follow-up no
            // longer satisfies. Carry `force=true` on source files so the call we
            // hand back is one the caller can actually make.
            let force_arg = if crate::tools::file_summary::detect_file_type(path)
                == crate::tools::file_summary::FileSummaryType::Source
            {
                ", force=true"
            } else {
                ""
            };
            result["next"] = json!(format!(
                "read_file(\"{path}\", start_line={}, end_line={end}{force_arg})",
                orig_end + 1
            ));
        }
        if source_tag != "project" {
            result["source"] = json!(source_tag);
        }
        if let Some(c) = md_cov {
            result["coverage"] = c;
        }
        return Ok(result);
    }

    let mut result = json!({ "content": content });
    if source_tag != "project" {
        result["source"] = json!(source_tag);
    }
    if let Some(c) = md_cov {
        result["coverage"] = c;
    }
    Ok(result)
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
    use super::output::{OutputGuard, OutputMode, OverflowInfo};

    if crate::tools::exceeds_inline_limit(text) {
        let file_id = ctx
            .output_buffer
            .store_file(resolved.to_string_lossy().to_string(), text.to_string());
        let mut result =
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
        result["file_id"] = json!(file_id);

        // This summary describes a file it does not contain — an outline, zero content
        // lines. Until now it carried only `line_count`, which the renderer prints as a
        // bare "1505 lines" header: indistinguishable from a complete read, so a caller
        // could reasonably believe it had seen the file.
        //
        // Thirteen lines below, the milder case (exploring mode, file longer than
        // max_results) builds a full OverflowInfo with a tailored hint. The worse case had
        // none. That asymmetry is the bug — not a design philosophy, a local omission.
        //
        // `shown: 0` is literal, not a placeholder: zero lines of content are shown.
        //
        // See `docs/issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md`.
        let summarised_lines = result["line_count"]
            .as_u64()
            .unwrap_or_else(|| text.lines().count() as u64) as usize;
        let is_source = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy())
            == crate::tools::file_summary::FileSummaryType::Source;
        result["complete"] = json!(false);
        result["overflow"] = OutputGuard::overflow_json(&OverflowInfo {
            shown: 0,
            total: summarised_lines,
            hint: outline_hint(
                &file_id,
                is_source,
                input["force"].as_bool().unwrap_or(false),
            ),
            next_offset: None,
            by_file: None,
            by_file_overflow: 0,
        });

        if path.ends_with(".md") || path.ends_with(".markdown") {
            if let Some(c) = markdown_coverage(text, resolved, ctx, None, None, None) {
                result["coverage"] = c;
            }
        }
        return Ok(result);
    }

    let is_md = path.ends_with(".md") || path.ends_with(".markdown");
    let md_cov = if is_md {
        markdown_coverage(text, resolved, ctx, None, None, None)
    } else {
        None
    };

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
                     + symbols(query=..., include_body=true) to read specific functions. \
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
        return Ok(result);
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
    Ok(result)
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
        return insert_below_header("0 lines".to_string(), &overflow_head(val));
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

                    for sym in symbols {
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
                    for h in headings {
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
                    for k in keys {
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
                for s in sections {
                    let key = s["key"].as_str().unwrap_or("?");
                    let line = s["line"].as_u64().unwrap_or(0);
                    let end = s["end_line"].as_u64().unwrap_or(0);
                    out.push_str(&format!("\n    {key}  L{line}-{end}"));
                }
            }
            if let Some(keys) = val["keys"].as_array() {
                out.push_str("\n  Keys:");
                for k in keys {
                    let key = k["key"].as_str().unwrap_or("?");
                    let line = k["line"].as_u64().unwrap_or(0);
                    out.push_str(&format!("\n    {key}  L{line}"));
                }
            }
        }
        "yaml" => {
            if let Some(sections) = val["sections"].as_array() {
                out.push_str("\n  Sections:");
                for s in sections {
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
    /// Real shape, measured 2026-08-29 on a live buffer: a `run_command`
    /// envelope pretty-prints to 4 lines, of which line 3 is the entire stdout
    /// as one JSON-escaped string, 9998 bytes wide. See
    /// `docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`.
    #[tokio::test]
    async fn read_file_buffer_single_oversized_line_still_fits_the_threshold() {
        let stdout = (1..=1200)
            .map(|i| format!("row {i:05}"))
            .collect::<Vec<_>>()
            .join("\n");
        let envelope = json!({ "exit_code": 0, "stdout": stdout }).to_string();
        let ctx = test_ctx().await;
        let buf_id = ctx.output_buffer.store_tool("cmd", envelope);

        // Premise of the fixture, asserted rather than assumed: after
        // pretty-printing, the payload really is ONE line wider than the budget.
        // If a future edit makes this fixture many-short-lines, this fails here
        // instead of silently degrading into a copy of the two tests above.
        let pretty = {
            let raw = ctx.output_buffer.get(&buf_id).unwrap().stdout;
            let v: Value = serde_json::from_str(&raw).unwrap();
            serde_json::to_string_pretty(&v).unwrap()
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
