//! `read_file` tool and read helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::format_overflow;
use super::{optional_u64_param, OutputForm, RecoverableError, Tool, ToolContext};
use crate::util::text::extract_lines;

pub struct ReadFile;

#[async_trait::async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a file. Large files return a summary + @file_* handle. \
         Format-aware: json_path (JSON), toml_key (TOML/YAML). Use read_markdown for .md files. \
         Source files: a start_line+end_line range overlapping a named symbol is redirected \
         to symbols(include_body=true); pass force=true to bypass."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "File path relative to project root" },
                "file_path": { "type": "string", "description": "Alias for path" },
                "start_line": { "type": "integer", "description": "First line (1-indexed). Pair with end_line." },
                "end_line": { "type": "integer", "description": "Last line (1-indexed, inclusive). Pair with start_line." },
                "json_path": { "type": "string", "description": "JSON subtree by path (e.g. \"$.dependencies\")." },
                "toml_key": { "type": "string", "description": "TOML table or YAML section by key (e.g. \"dependencies\")." },
                "force": { "type": "boolean", "description": "Skip source-symbol hint and read the raw line range." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let raw_path = input["path"]
            .as_str()
            .or_else(|| input["file_path"].as_str())
            .ok_or_else(|| {
                RecoverableError::with_hint(
                    "missing required parameter 'path'",
                    "Provide the file path as: path=\"relative/path/to/file\"",
                )
            })?;
        let path = strip_buffer_ref_quotes(raw_path);

        // Buffer refs bypass the filesystem entirely.
        if path.starts_with("@file_") || path.starts_with("@cmd_") || path.starts_with("@tool_") {
            return read_from_buffer(path, &input, ctx);
        }

        let project_root = ctx.agent.project_root().await;
        let security = ctx.agent.security_config().await;
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
/// Never re-buffers — returns inline or, for oversized content, paginates
/// via `shown_lines` / `next`.
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
            let content_total = content.lines().count();
            let file_id = ctx
                .output_buffer
                .store_file(format!("{}[{}-{}]", path, s, e), content.clone());
            let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_budget(
                &content,
                1,
                usize::MAX,
                crate::tools::INLINE_BYTE_BUDGET,
            );
            let orig_start = s as usize;
            let orig_end = orig_start + lines_shown.saturating_sub(1);
            let mut result = json!({
                "content": chunk,
                "file_id": file_id,
                "total_lines": content_total,
                "shown_lines": [orig_start, orig_end],
                "complete": complete,
            });
            if !complete {
                let buf_next_start = lines_shown + 1;
                let buf_next_end = (buf_next_start + lines_shown - 1).min(content_total);
                result["next"] = json!(format!(
                    "read_file(\"{file_id}\", start_line={buf_next_start}, end_line={buf_next_end})"
                ));
            }
            return Ok(result);
        }
        return Ok(json!({ "content": content, "total_lines": total_lines }));
    }

    // Full buffer: paginate if over the inline limit. Never re-buffer.
    if crate::tools::exceeds_inline_limit(&text) {
        let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_budget(
            &text,
            1,
            usize::MAX,
            crate::tools::INLINE_BYTE_BUDGET,
        );
        let mut result = json!({
            "content": chunk,
            "total_lines": total_lines,
            "shown_lines": [1, lines_shown],
            "complete": complete,
        });
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

/// Validate navigation parameter combinations for real-file reads.
fn validate_read_nav_params(
    input: &Value,
    start_line: Option<u64>,
    end_line: Option<u64>,
) -> Result<()> {
    if start_line.is_some() != end_line.is_some() {
        return Err(RecoverableError::with_hint(
            "both start_line and end_line are required",
            "Provide both start_line and end_line for a line range, e.g. start_line=1, end_line=50",
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

/// Resolve the library source tag for a file (`"project"` or `"lib:<name>"`).
async fn compute_source_tag(resolved: &std::path::Path, ctx: &ToolContext) -> String {
    let inner = ctx.agent.inner.read().await;
    if let Some(project) = inner.active_project() {
        if let Some(lib) = project.library_registry.is_library_path(resolved) {
            return format!("lib:{}", lib.name);
        }
    }
    "project".to_string()
}

/// Read file contents with user-friendly error messages.
fn read_file_text(path: &str, resolved: &std::path::PathBuf) -> Result<String> {
    std::fs::read_to_string(resolved).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RecoverableError::with_hint(
            format!("file not found: '{}'", path),
            "Check the path with tree, or use tree with `glob` to locate the file",
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
    let file_type = crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy());
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

    if !force
        && crate::tools::file_summary::detect_file_type(path)
            == crate::tools::file_summary::FileSummaryType::Source
    {
        let matches = find_symbols_for_range(text, resolved, start, end);
        if !matches.is_empty() {
            let names: Vec<_> = matches.iter().take(3).map(|s| format!("'{s}'")).collect();
            let mut label = names.join(", ");
            if matches.len() > 3 {
                label.push_str(&format!(" and {} more", matches.len() - 3));
            }
            let first = &matches[0];
            return Err(RecoverableError::with_hint(
                format!("source range overlaps named symbol(s): {label}"),
                format!(
                    "Use symbols(name='{first}', include_body=true) to read the body directly. \
                     Pass force=true to read the raw line range anyway."
                ),
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
        let content_total = content.lines().count();
        let file_id = ctx
            .output_buffer
            .store_file(resolved.to_string_lossy().to_string(), content.clone());
        let (chunk, lines_shown, complete) = crate::util::text::extract_lines_to_budget(
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
            "total_lines": content_total,
            "shown_lines": [orig_start, orig_end],
            "complete": complete,
        });
        if !complete {
            let buf_next_start = lines_shown + 1;
            let buf_next_end = (buf_next_start + lines_shown - 1).min(content_total);
            result["next"] = json!(format!(
                "read_file(\"{file_id}\", start_line={buf_next_start}, end_line={buf_next_end})"
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
                     Or use offset/limit to read a line range.",
                    total_lines
                )
            } else {
                format!(
                    "File has {} lines. Use offset/limit to read specific ranges.",
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
    // Summary modes have a "type" key
    if let Some(file_type) = val["type"].as_str() {
        return format_read_file_summary(val, file_type);
    }

    // Auto-chunked response: shown_lines present means partial read with content.
    // Line numbers are intentionally NOT prefixed — the caller supplied the range,
    // so per-line numbers are redundant noise (and were slice-relative/wrong here
    // before). See docs/issues/2026-05-21-read-file-slice-relative-line-numbers.md.
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
        let mut out = "0 lines".to_string();
        if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
            out.push('\n');
            out.push_str(&format_overflow(overflow));
        }
        return out;
    }

    // Raw content, no per-line number prefixes (caller-supplied ranges make them
    // redundant; full-file reads can re-derive line numbers trivially).
    let line_word = if total_lines == 1 { "line" } else { "lines" };
    let mut out = format!("{total_lines} {line_word}\n\n");
    out.push_str(content);

    // Overflow
    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }

    out
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

    // Buffer reference
    if let Some(file_id) = val["file_id"].as_str() {
        out.push_str(&format!("\n\n  Buffer: {file_id}"));
    }
    if let Some(hint) = val["hint"].as_str() {
        out.push_str(&format!("\n  {hint}"));
    }

    out
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

/// Return the `name_path` of every symbol whose body overlaps (inclusive) the
/// read range: symbol contains range, range contains symbol, or they share a boundary.
///
/// `start` and `end` are 1-indexed (as received from tool input).
/// `SymbolInfo.start_line` / `end_line` are 0-indexed.
/// Returns an empty Vec on parse error (fail open).
fn find_symbols_for_range(
    text: &str,
    resolved: &std::path::Path,
    start: u64,
    end: u64,
) -> Vec<String> {
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
        .map(|sym| sym.name_path.clone())
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
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
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
    async fn read_file_call_content_returns_line_numbered_text_not_json() {
        // Regression: small read_file results used to serialize as pretty JSON via
        // the default Tool::call_content path because ReadFile did not declare
        // OutputForm::Text. Now both axes reach format_read_file, so sub-threshold
        // reads come through as raw text. Line-number prefixes were removed
        // (docs/issues/2026-05-21-read-file-slice-relative-line-numbers.md), so the
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
}
