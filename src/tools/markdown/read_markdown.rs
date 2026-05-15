//! ReadMarkdown tool — heading-based navigation for .md files.

use anyhow::Result;
use serde_json::{json, Value};

use super::super::{optional_u64_param, RecoverableError, Tool, ToolContext};
use crate::util::text::extract_lines;

pub struct ReadMarkdown;

#[async_trait::async_trait]
impl Tool for ReadMarkdown {
    fn name(&self) -> &str {
        "read_markdown"
    }

    fn description(&self) -> &str {
        "Read a Markdown file with heading-based navigation. Returns heading map by default, \
         or targeted sections via heading/headings params."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Markdown file path relative to project root" },
                "heading": { "type": "string", "description": "Markdown section by heading (e.g. \"## Auth\")." },
                "headings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of headings to read (returns multiple sections). Mutually exclusive with heading."
                },
                "start_line": { "type": "integer", "description": "First line (1-indexed). Pair with end_line." },
                "end_line": { "type": "integer", "description": "Last line (1-indexed, inclusive). Pair with start_line." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let path = crate::tools::require_str_param(&input, "path")?;

        // Resolve path → (resolved PathBuf, text String).
        // Buffer-ref branch loads from the output buffer and falls through to
        // the shared heading-nav / line-range logic below. Disk-read branch
        // validates, stats, and reads the file.
        let (resolved, text) = if path.starts_with("@file_") {
            let buf = ctx
                .output_buffer
                .get(path)
                .ok_or_else(|| {
                    RecoverableError::with_hint(
                        format!("buffer reference not found: '{}'", path),
                        "Buffer refs expire when the session resets. Re-run read_markdown on the file to get a fresh ref.",
                    )
                })?;
            let resolved = buf
                .source_path
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from(path));
            (resolved, buf.stdout.clone())
        } else {
            // Gate: .md files only
            if !path.ends_with(".md") && !path.ends_with(".markdown") {
                return Err(RecoverableError::with_hint(
                    "read_markdown only supports .md files",
                    "Use read_file for non-markdown files.",
                )
                .into());
            }

            let project_root = ctx.agent.project_root().await;
            let security = ctx.agent.security_config().await;
            let resolved = crate::util::path_security::validate_read_path(
                path,
                project_root.as_deref(),
                &security,
            )?;

            if resolved.is_dir() {
                return Err(RecoverableError::with_hint(
                    format!("'{}' is a directory, not a file", path),
                    "Use tree to browse directory contents, or provide a specific file path",
                )
                .into());
            }

            let text = std::fs::read_to_string(&resolved).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => RecoverableError::with_hint(
                    format!("file not found: '{}'", path),
                    "Check the path with tree, or use tree with `glob` to locate the file",
                )
                .into(),
                _ => anyhow::anyhow!("failed to read {}: {}", resolved.display(), e),
            })?;
            (resolved, text)
        };

        // Reject librarian-managed artifacts — use artifact(action="get") instead.
        crate::util::librarian_guard::guard_not_librarian_managed(path, &text)?;

        // Extract params
        let heading = input["heading"].as_str();
        let headings_param = crate::tools::optional_array_param(&input, "headings");
        let start_line = optional_u64_param(&input, "start_line");
        let end_line = optional_u64_param(&input, "end_line");

        // Mutual exclusivity checks
        if heading.is_some() && headings_param.is_some() {
            return Err(RecoverableError::with_hint(
                "heading and headings are mutually exclusive",
                "Use heading for a single section, or headings for multiple sections.",
            )
            .into());
        }

        let has_nav = heading.is_some() || headings_param.is_some();
        let has_range = start_line.is_some() || end_line.is_some();

        if has_nav && has_range {
            return Err(RecoverableError::with_hint(
                "navigation parameters are mutually exclusive with start_line/end_line",
                "Use heading/headings OR start_line+end_line, not both",
            )
            .into());
        }

        if start_line.is_some() != end_line.is_some() {
            return Err(RecoverableError::with_hint(
                "both start_line and end_line are required",
                "Provide both start_line and end_line for a line range, e.g. start_line=1, end_line=50",
            )
            .into());
        }

        // ── Multi-heading navigation ─────────────────────────────────────
        if let Some(headings_arr) = headings_param {
            let heading_queries: Vec<String> = headings_arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            let mut sections = Vec::new();
            let mut seen_headings = Vec::new();

            for query in &heading_queries {
                let section = crate::tools::file_summary::extract_markdown_section(&text, query)?;
                seen_headings.push(
                    section
                        .breadcrumb
                        .last()
                        .cloned()
                        .unwrap_or_else(|| query.clone()),
                );
                sections.push(section.content);
            }

            let content = sections.join("\n\n");

            // Oversized multi-heading join — fall back to must_follow.
            if crate::tools::exceeds_inline_limit(&content) {
                let file_id = ctx
                    .output_buffer
                    .store_file(resolved.to_string_lossy().to_string(), content.clone());
                let lines = content.lines().count();
                let must_follow = format!(
                    "IRON LAW #6: The combined section content is too large to return inline. \
                     Use {:?} for subsequent reads — NOT the original path. \
                     Request one heading at a time, or slice with start_line/end_line.",
                    file_id
                );
                let next_actions: Vec<String> = seen_headings
                    .iter()
                    .take(3)
                    .map(|h| format!("read_markdown({:?}, heading={})", file_id, h))
                    .collect();
                let err = crate::tools::RecoverableError::with_must_follow(
                    format!(
                        "combined headings span {} lines — exceeds inline threshold",
                        lines
                    ),
                    must_follow,
                )
                .with_extra("file_id", serde_json::json!(file_id))
                .with_extra("requested_headings", serde_json::json!(seen_headings))
                .with_extra("next_actions", serde_json::json!(next_actions));
                return Err(err.into());
            }

            // Record coverage
            if !seen_headings.is_empty() {
                if let Ok(mut cov) = ctx.section_coverage.lock() {
                    cov.mark_seen(&resolved, &seen_headings);
                }
            }

            let mut result = json!({
                "content": content,
                "sections_returned": heading_queries.len(),
            });

            // Coverage hint
            let all_headings = crate::tools::file_summary::parse_all_headings(&text);
            if !all_headings.is_empty() {
                let all_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
                if let Ok(mut cov) = ctx.section_coverage.lock() {
                    if let Some(status) = cov.status(&resolved, &all_texts) {
                        if !status.unread.is_empty() {
                            result["coverage"] = json!({
                                "read": status.read_count,
                                "total": status.total_count,
                                "unread": status.unread,
                            });
                        }
                    }
                }
            }

            return Ok(result);
        }

        // ── Single-heading navigation ────────────────────────────────────
        if let Some(heading_query) = heading {
            let section_result =
                match crate::tools::file_summary::extract_markdown_section(&text, heading_query) {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = e.message.clone();
                        if msg.contains("not found") {
                            let headings_json: Vec<serde_json::Value> =
                                crate::tools::file_summary::parse_all_headings(&text)
                                    .iter()
                                    .map(|h| serde_json::json!({"h": h.text, "l": h.line}))
                                    .collect();
                            return Err(RecoverableError::with_hint(
                                format!("heading {:?} not found", heading_query),
                                "pick a heading from `headings` array or use start_line/end_line",
                            )
                            .with_extra("headings", serde_json::json!(headings_json))
                            .into());
                        }
                        return Err(e.into());
                    }
                };
            let cov = crate::tools::read_file::markdown_coverage(
                &text, &resolved, ctx, heading, None, None,
            );

            // Oversized match — return ok:false with must_follow + nested section_map
            // + next_actions. The agent must pick a sub-heading or a line range, not
            // retry against the original path.
            if crate::tools::exceeds_inline_limit(&section_result.content) {
                let file_id = ctx.output_buffer.store_file(
                    resolved.to_string_lossy().to_string(),
                    section_result.content.clone(),
                );
                let section_lines = section_result.content.lines().count();

                let (start_ln, end_ln) = section_result.line_range;
                let all_headings = crate::tools::file_summary::parse_all_headings(&text);
                let nested: Vec<serde_json::Value> = all_headings
                    .iter()
                    .filter(|h| h.line > start_ln && h.line <= end_ln)
                    .map(|h| json!({"level": h.level, "text": h.text, "line": h.line}))
                    .collect();

                let heading_label = section_result
                    .breadcrumb
                    .last()
                    .cloned()
                    .unwrap_or_else(|| heading_query.to_string());

                let must_follow = format!(
                    "IRON LAW #6: Use {:?} for subsequent reads — NOT the original path. \
                     Pick a sub-heading from section_map OR use read_markdown({:?}, start_line=N, end_line=M).",
                    file_id, file_id
                );

                let next_actions: Vec<String> = {
                    let mut actions = Vec::new();
                    if let Some(first) = nested.first() {
                        if let Some(h) = first.get("text").and_then(|v| v.as_str()) {
                            actions.push(format!("read_markdown({:?}, heading={})", file_id, h));
                        }
                    }
                    actions.push(format!(
                        "read_markdown({:?}, start_line={}, end_line={})",
                        file_id,
                        start_ln,
                        start_ln + 100.min(section_lines)
                    ));
                    actions
                };

                let err = crate::tools::RecoverableError::with_must_follow(
                    format!(
                        "heading {:?} spans {} lines — exceeds inline threshold",
                        heading_label, section_lines
                    ),
                    must_follow,
                )
                .with_extra("file_id", serde_json::json!(file_id))
                .with_extra("section_map", serde_json::json!(nested))
                .with_extra("next_actions", serde_json::json!(next_actions))
                .with_extra("breadcrumb", serde_json::json!(section_result.breadcrumb))
                .with_extra("line_range", serde_json::json!([start_ln, end_ln]));
                return Err(err.into());
            }

            let mut val = json!({
                "content": section_result.content,
                "line_range": [section_result.line_range.0, section_result.line_range.1],
                "breadcrumb": section_result.breadcrumb,
                "siblings": section_result.siblings,
                "format": "markdown",
            });
            if let Some(c) = cov {
                val["coverage"] = c;
            }
            return Ok(val);
        }

        // ── Line-range read ──────────────────────────────────────────────
        if let (Some(start), Some(end)) = (start_line, end_line) {
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
            let content_total = text.lines().count();
            if (start as usize) > content_total {
                return Err(RecoverableError::with_hint(
                    format!(
                        "start_line {} exceeds file length {}",
                        start, content_total
                    ),
                    format!(
                        "valid range is 1..={}; use read_markdown(path, start_line=N, end_line=M) within bounds",
                        content_total
                    ),
                )
                .with_extra("lines", serde_json::json!(content_total))
                .into());
            }
            let content = extract_lines(&text, start as usize, end as usize);
            let md_cov = crate::tools::read_file::markdown_coverage(
                &text, &resolved, ctx, None, start_line, end_line,
            );

            // Buffer large extracts
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
                        "read_markdown(\"{file_id}\", start_line={buf_next_start}, \
                         end_line={buf_next_end})"
                    ));
                }
                if let Some(c) = md_cov {
                    result["coverage"] = c;
                }
                return Ok(result);
            }

            let mut result = json!({ "content": content });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            return Ok(result);
        }

        // ── Default branch: adaptive tiers ────────────────────────────────────
        let total_bytes = text.len();
        let total_lines = text.lines().count();
        let oversized = crate::tools::exceeds_inline_limit(&text);
        let all_headings = crate::tools::file_summary::parse_all_headings(&text);
        let oversized_by_headings = all_headings.len() > crate::tools::HEADINGS_HARD_CAP;

        let md_cov =
            crate::tools::read_file::markdown_coverage(&text, &resolved, ctx, None, None, None);

        // ── Tier 3: large — heading map + must_follow, no body ────────────
        if oversized || oversized_by_headings {
            let heading_count = all_headings.len();
            let heading_map: Vec<Value> = all_headings
                .iter()
                .map(|h| {
                    json!({
                        "level": h.level,
                        "text": h.text,
                        "line": h.line,
                    })
                })
                .collect();

            let file_id = ctx
                .output_buffer
                .store_file(resolved.to_string_lossy().to_string(), text.clone());

            let must_follow = if heading_count == 0 {
                format!(
                    "IRON LAW #6: For subsequent reads, use {:?} (NOT the original path). \
                     Slice with read_markdown({:?}, start_line=N, end_line=M).",
                    file_id, file_id
                )
            } else {
                format!(
                    "IRON LAW #6: For subsequent reads, use {:?} (NOT the original path). \
                     Pick a heading: read_markdown({:?}, heading=## Section). \
                     Or slice: read_markdown({:?}, start_line=N, end_line=M).",
                    file_id, file_id, file_id
                )
            };

            let mut result = json!({
                "format": "markdown",
                "total_lines": total_lines,
                "total_bytes": total_bytes,
                "heading_count": heading_count,
                "heading_map": heading_map,
                "file_id": file_id,
                "must_follow": must_follow,
            });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            return Ok(result);
        }

        // ── Tier 2: medium — full content + soft hint ─────────────────────
        if total_lines > crate::tools::LINE_SOFT_CAP {
            let heading_count = all_headings.len();
            let hint = if heading_count == 0 {
                format!(
                    "{} lines, no headings. For focused reads: read_markdown(path, start_line=N, end_line=M).",
                    total_lines
                )
            } else {
                format!(
                    "{} lines, {} sections. For focused reads: read_markdown(path, heading=## Section).",
                    total_lines, heading_count
                )
            };

            let mut result = json!({
                "format": "markdown",
                "content": text,
                "total_lines": total_lines,
                "heading_count": heading_count,
                "hint": hint,
            });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            return Ok(result);
        }

        // ── Tier 1: small — full content only ─────────────────────────────
        let mut result = json!({
            "content": text,
            "lines": total_lines,
        });
        if let Some(c) = md_cov {
            result["coverage"] = c;
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(crate::tools::read_file::format_read_file(result))
    }
}
