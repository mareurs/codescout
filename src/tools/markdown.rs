//! Markdown-specific tools: `read_markdown` and `edit_markdown`.
//!
//! `ReadMarkdown` provides heading-based navigation for `.md` files (heading map,
//! single-section, multi-section, and line-range reads).
//!
//! `EditMarkdown` provides heading-addressed section editing with support for
//! `action="edit"` (scoped string replacement) and batch mode.

use anyhow::Result;
use serde_json::{json, Value};

use super::{optional_u64_param, parse_bool_param, RecoverableError, Tool, ToolContext};
use crate::util::text::extract_lines;

// ── read_markdown ────────────────────────────────────────────────────────────

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
        let path = super::require_str_param(&input, "path")?;

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

        // Extract params
        let heading = input["heading"].as_str();
        let headings_param = super::optional_array_param(&input, "headings");
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
                crate::tools::file_summary::extract_markdown_section(&text, heading_query)?;
            let cov =
                super::read_file::markdown_coverage(&text, &resolved, ctx, heading, None, None);

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
            let content = extract_lines(&text, start as usize, end as usize);
            let md_cov = super::read_file::markdown_coverage(
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

        let md_cov = super::read_file::markdown_coverage(&text, &resolved, ctx, None, None, None);

        // ── Tier 3: large — heading map + must_follow, no body ────────────
        if oversized {
            let all_headings = crate::tools::file_summary::parse_all_headings(&text);
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
            let all_headings = crate::tools::file_summary::parse_all_headings(&text);
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
        let all_headings = crate::tools::file_summary::parse_all_headings(&text);
        let heading_count = all_headings.len();
        let mut result = json!({
            "format": "markdown",
            "content": text,
            "total_lines": total_lines,
            "heading_count": heading_count,
        });
        if let Some(c) = md_cov {
            result["coverage"] = c;
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(super::read_file::format_read_file(result))
    }
}

// ── edit_markdown ────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Helper functions (moved from section_edit.rs)
// ---------------------------------------------------------------------------

/// Pure string transformation: apply `action` to the section identified by `heading_query`.
///
/// Returns the full modified file content (always ends with a single newline).
pub fn perform_section_edit(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
) -> Result<String> {
    use crate::tools::file_summary::{heading_level, resolve_section_range};

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Split into lines using split('\n') so the trailing newline is preserved as
    // a final empty-string element: "a\nb\n".split('\n') == ["a", "b", ""].
    let lines: Vec<&str> = content.split('\n').collect();

    // Convert 1-based line numbers from the range to 0-based indices into `lines`.
    let heading_idx = (range.heading_line - 1) as usize;

    // Compute the exclusive-end index for the section.
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    match action {
        "replace" => {
            let new = new_content
                .ok_or_else(|| anyhow::anyhow!("content is required for the replace action"))?;

            // Smart detection: does the new content start with a Markdown heading?
            let replace_heading = new
                .lines()
                .next()
                .map(|l| heading_level(l.trim_end()).is_some())
                .unwrap_or(false);

            let result = if replace_heading {
                // Replace heading + body entirely.
                let before = join_lines(&lines[..heading_idx]);
                let after = join_lines_tail(&lines[end_idx..]);
                format!("{}{}{}", before, ensure_trailing_newline(new), after)
            } else {
                // Preserve the existing heading, replace body only.
                let heading_line_str = lines[heading_idx];
                let before = join_lines(&lines[..heading_idx]);
                let after = join_lines_tail(&lines[end_idx..]);
                let separator = if new.starts_with('\n') { "\n" } else { "\n\n" };
                format!(
                    "{}{}{}{}{}",
                    before,
                    heading_line_str,
                    separator,
                    ensure_trailing_newline(new),
                    after
                )
            };
            Ok(normalize_trailing_newline(&result))
        }

        "insert_before" => {
            let new = new_content.ok_or_else(|| {
                anyhow::anyhow!("content is required for the insert_before action")
            })?;
            let before = join_lines(&lines[..heading_idx]);
            let rest = join_lines_tail(&lines[heading_idx..]);
            let result = format!("{}{}{}", before, ensure_trailing_newline(new), rest);
            Ok(normalize_trailing_newline(&result))
        }

        "insert_after" => {
            let new = new_content.ok_or_else(|| {
                anyhow::anyhow!("content is required for the insert_after action")
            })?;
            let before = join_lines(&lines[..end_idx]);
            let after = join_lines_tail(&lines[end_idx..]);
            let result = format!("{}{}{}", before, new, after);
            Ok(normalize_trailing_newline(&result))
        }

        "remove" => {
            let mut remove_end = end_idx;
            if remove_end < lines.len() && lines[remove_end].trim().is_empty() {
                remove_end += 1;
            }
            let before = join_lines(&lines[..heading_idx]);
            let after = join_lines_tail(&lines[remove_end..]);
            let result = format!("{}{}", before, after);
            Ok(normalize_trailing_newline(&result))
        }

        other => Err(anyhow::anyhow!(
            "invalid action {:?}; expected replace, insert_before, insert_after, or remove",
            other
        )),
    }
}

/// Compute the exclusive-end index (into `split('\n')` lines) for a section
/// that starts at `start_idx` (0-based) and has heading level `level`.
/// Skips headings inside fenced code blocks (``` ... ```).
fn compute_section_end(lines: &[&str], start_idx: usize, level: usize) -> usize {
    let mut in_code_block = false;
    for (i, &line) in lines[start_idx..].iter().enumerate() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if let Some(lvl) = crate::tools::file_summary::heading_level(line) {
            if lvl <= level {
                return start_idx + i;
            }
        }
    }
    lines.len()
}

/// List the sub-heading texts that a `replace` on `heading_query` would wipe.
///
/// BUG-043: when a section has nested sub-headings (deeper heading levels than
/// the target), `replace` silently consumes them. For plan/spec files whose
/// `##` sections contain dozens of `###` tasks, this causes catastrophic data
/// loss. Callers check this before `replace` and refuse unless the user opts
/// in via `include_subsections: true`.
///
/// Returns the headings with their `#` prefix intact so the error message can
/// echo them verbatim. Empty vec means the section has no children and `replace`
/// is safe.
pub fn find_consumed_subsections(content: &str, heading_query: &str) -> Result<Vec<String>> {
    use crate::tools::file_summary::{heading_level, resolve_section_range};

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = (range.heading_line - 1) as usize;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    let mut in_code_block = false;
    let mut out = Vec::new();
    for &line in &lines[heading_idx + 1..end_idx] {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if heading_level(line).is_some() {
            out.push(line.trim_end().to_string());
        }
    }
    Ok(out)
}

/// Format the BUG-043 guard error. The message itself names `include_subsections`
/// so the opt-in is visible to any caller that only inspects the error text.
fn subsection_guard_error(
    batch_idx: Option<usize>,
    heading: &str,
    victims: &[String],
) -> RecoverableError {
    let prefix = match batch_idx {
        Some(i) => format!("edits[{i}]: "),
        None => String::new(),
    };
    RecoverableError::with_hint(
        format!(
            "{prefix}replace on '{heading}' would wipe {n} nested heading(s): {list}. \
             Pass include_subsections: true to opt into consuming children.",
            n = victims.len(),
            list = victims.join(", "),
        ),
        "Prefer action=\"edit\" with old_string/new_string to target text \
         inside the section without touching its subsections.",
    )
}

/// Join a non-tail slice of lines back into a string.
/// Always appends a '\n' after the last element to act as a separator.
fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    format!("{}\n", lines.join("\n"))
}

/// Join a tail slice (including any trailing "" from split('\n')).
fn join_lines_tail(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    lines.join("\n")
}

/// Ensure `s` ends with exactly one newline.
fn ensure_trailing_newline(s: &str) -> String {
    if s.ends_with('\n') {
        s.to_owned()
    } else {
        format!("{}\n", s)
    }
}

/// Normalise the final result to end with exactly one newline.
fn normalize_trailing_newline(s: &str) -> String {
    let trimmed = s.trim_end_matches('\n');
    format!("{}\n", trimmed)
}

/// Perform a heading-scoped string replacement within a markdown file.
///
/// Finds the section identified by `heading_query`, locates `old_string` within it,
/// and replaces with `new_string`. If `replace_all` is true, replaces all occurrences
/// within the section; otherwise only the first.
///
/// Returns the full modified file content.
fn perform_scoped_edit(
    content: &str,
    heading_query: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<String> {
    use crate::tools::file_summary::resolve_section_range;

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = (range.heading_line - 1) as usize;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    // Extract the section content (heading + body) with trailing newline
    let section_text = format!("{}\n", join_lines_tail(&lines[heading_idx..end_idx]));

    if !section_text.contains(old_string) {
        return Err(anyhow::anyhow!(
            "old_string not found in section '{}'. \
             The text must match exactly (whitespace-sensitive).",
            heading_query
        ));
    }

    let new_section = if replace_all {
        section_text.replace(old_string, new_string)
    } else {
        section_text.replacen(old_string, new_string, 1)
    };

    let before = join_lines(&lines[..heading_idx]);
    let after = join_lines_tail(&lines[end_idx..]);
    let result = format!("{}{}{}", before, new_section, after);
    Ok(normalize_trailing_newline(&result))
}

// ---------------------------------------------------------------------------
// EditMarkdown tool
// ---------------------------------------------------------------------------

pub struct EditMarkdown;

#[async_trait::async_trait]
impl Tool for EditMarkdown {
    fn name(&self) -> &str {
        "edit_markdown"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Edit a Markdown document by heading. Actions: replace, insert_before, insert_after, \
         remove, edit. Supports batch mode via edits array."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "### Workflow: Editing a Markdown Document\n\n\
             | Step | Tool | Purpose |\n\
             |------|------|---------|\n\
             | 1 | `read_markdown(path)` | Get heading map — see all sections |\n\
             | 2 | `read_markdown(path, headings=[...])` | Read target sections (one call, multiple sections) |\n\
             | 3a | `edit_markdown(path, heading, action, content)` | Whole-section: replace (body only — heading preserved), insert, remove |\n\
             | 3b | `edit_markdown(path, heading, action=\"edit\", old_string, new_string)` | Surgical: scoped string replacement within a section |\n\
             | 3c | `edit_markdown(path, edits=[...])` | Batch: multiple edits across sections, atomic |"
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Markdown file path" },
                "heading": { "type": "string", "description": "Target section heading (fuzzy matched). Required unless using edits[] batch mode." },
                "action": {
                    "type": "string",
                    "enum": ["replace", "insert_before", "insert_after", "remove", "edit"],
                    "description": "Operation to perform. Required unless using edits[] batch mode."
                },
                "content": { "type": "string", "description": "New content for replace/insert actions (body only — heading preserved on replace)" },
                "old_string": { "type": "string", "description": "For action='edit': exact text to find within section" },
                "new_string": { "type": "string", "description": "For action='edit': replacement text" },
                "replace_all": { "type": "boolean", "default": false, "description": "For action='edit': replace all occurrences" },
                "include_subsections": { "type": "boolean", "default": false, "description": "For action='replace': opt in to consuming nested sub-headings (deeper levels). Default refuses to wipe children — see BUG-043." },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["heading", "action"],
                        "properties": {
                            "heading": { "type": "string" },
                            "action": { "type": "string", "enum": ["replace", "insert_before", "insert_after", "remove", "edit"] },
                            "content": { "type": "string" },
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" },
                            "replace_all": { "type": "boolean" },
                            "include_subsections": { "type": "boolean" }
                        }
                    },
                    "description": "Batch mode: array of edit operations applied atomically. Mutually exclusive with top-level heading/action."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let path = super::require_str_param(&input, "path")?;

        // Gate: .md files only
        if !path.ends_with(".md") && !path.ends_with(".markdown") {
            return Err(RecoverableError::with_hint(
                "edit_markdown only supports .md files",
                "Use edit_file for non-markdown files.",
            )
            .into());
        }

        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;
        let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;

        let file_content = std::fs::read_to_string(&resolved)?;

        // Determine mode: batch vs single
        let has_edits = input["edits"].is_array();
        let has_heading = input["heading"].is_string();
        let has_action = input["action"].is_string();

        if has_edits && (has_heading || has_action) {
            return Err(RecoverableError::with_hint(
                "edits array is mutually exclusive with top-level heading/action",
                "Use either edits=[] for batch mode, or heading+action for single edit.",
            )
            .into());
        }

        let new_content = if has_edits {
            // ── Batch mode ───────────────────────────────────────────
            let edits = input["edits"].as_array().unwrap();
            let mut content = file_content.clone();

            for (i, edit) in edits.iter().enumerate() {
                let heading = edit["heading"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("edits[{}]: missing required 'heading' field", i)
                })?;
                let action = edit["action"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("edits[{}]: missing required 'action' field", i)
                })?;

                content = if action == "edit" {
                    let old_string = edit["old_string"].as_str().ok_or_else(|| {
                        anyhow::anyhow!("edits[{}]: old_string is required for action='edit'", i)
                    })?;
                    let new_string = edit["new_string"].as_str().unwrap_or("");
                    let replace_all_val = edit["replace_all"].as_bool().unwrap_or(false);
                    perform_scoped_edit(&content, heading, old_string, new_string, replace_all_val)
                        .map_err(|e| {
                            RecoverableError::with_hint(
                                format!("edits[{}]: {}", i, e),
                                "Check heading name and old_string content.",
                            )
                        })?
                } else {
                    let edit_content = edit["content"].as_str();
                    if action == "replace"
                        && !edit["include_subsections"].as_bool().unwrap_or(false)
                    {
                        if let Ok(victims) = find_consumed_subsections(&content, heading) {
                            if !victims.is_empty() {
                                return Err(
                                    subsection_guard_error(Some(i), heading, &victims).into()
                                );
                            }
                        }
                    }
                    perform_section_edit(&content, heading, action, edit_content).map_err(|e| {
                        RecoverableError::with_hint(
                            format!("edits[{}]: {}", i, e),
                            "Check heading name and action.",
                        )
                    })?
                };
            }

            content
        } else {
            // ── Single edit mode ─────────────────────────────────────
            let heading = super::require_str_param(&input, "heading")?;
            let action = super::require_str_param(&input, "action")?;

            if action == "edit" {
                let old_string = super::require_str_param(&input, "old_string")?;
                let new_string = input["new_string"].as_str().unwrap_or("");
                let replace_all_val = parse_bool_param(&input["replace_all"]);
                perform_scoped_edit(
                    &file_content,
                    heading,
                    old_string,
                    new_string,
                    replace_all_val,
                )
                .map_err(|e| {
                    RecoverableError::with_hint(e.to_string(), "Check heading name and old_string.")
                })?
            } else {
                let content = input["content"].as_str();
                if action == "replace" && !input["include_subsections"].as_bool().unwrap_or(false) {
                    if let Ok(victims) = find_consumed_subsections(&file_content, heading) {
                        if !victims.is_empty() {
                            return Err(subsection_guard_error(None, heading, &victims).into());
                        }
                    }
                }
                perform_section_edit(&file_content, heading, action, content).map_err(|e| {
                    RecoverableError::with_hint(e.to_string(), "Check heading name and action.")
                })?
            }
        };

        crate::util::fs::atomic_write(&resolved, &new_content)?;

        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(&resolved);
        }

        ctx.agent.reload_config_if_project_toml(&resolved).await;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent.mark_file_dirty(resolved.clone()).await;

        // Coverage hint: warn about unread sections.
        let all_headings = crate::tools::file_summary::parse_all_headings(&new_content);
        if !all_headings.is_empty() {
            let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
            if let Ok(mut cov) = ctx.section_coverage.lock() {
                if let Some(hint) = cov.unread_hint(&resolved, &heading_texts) {
                    return Ok(json!({"status": "ok", "hint": hint}));
                }
            }
        }

        Ok(json!("ok"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn read_markdown_empty_file_returns_small_tier() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("empty.md");
        std::fs::write(&file, "").unwrap();

        let out = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert_eq!(out["content"].as_str(), Some(""));
        assert_eq!(out["total_lines"].as_u64(), Some(0));
        assert!(out.get("hint").is_none());
        assert!(out.get("file_id").is_none());
    }

    #[tokio::test]
    async fn read_markdown_large_no_headings_must_follow_pivots_to_line_ranges() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("flat.md");
        // ~100KB of plain lines, no headings.
        let content: String = (0..10_000).map(|i| format!("line {}\n", i)).collect();
        std::fs::write(&file, &content).unwrap();

        let out = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(out.get("file_id").is_some(), "still large tier");
        assert_eq!(out["heading_count"].as_u64(), Some(0));
        assert_eq!(out["heading_map"].as_array().map(|a| a.len()), Some(0));
        let mf = out["must_follow"].as_str().unwrap();
        assert!(
            mf.contains("start_line"),
            "must_follow must mention start_line; got: {mf}"
        );
        assert!(
            !mf.contains("heading=\""),
            "must_follow must not suggest heading nav when there are no headings; got: {mf}"
        );
    }

    use super::*;

    // ── perform_section_edit tests (moved from section_edit.rs) ──────────

    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use serde_json::json;
    use tempfile::tempdir;

    async fn test_ctx() -> crate::tools::ToolContext {
        crate::tools::ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        }
    }

    /// Synthesize markdown content with `lines` total lines and `sections` H2 sections.
    fn synth_md(lines: usize, sections: usize) -> String {
        let mut out = String::from("# Title\n\n");
        let per_section = (lines.saturating_sub(2) / sections.max(1)).max(1);
        for i in 0..sections {
            out.push_str(&format!("## Section {}\n\n", i + 1));
            for _ in 0..per_section {
                out.push_str("body line\n");
            }
        }
        out
    }

    #[tokio::test]
    async fn read_markdown_small_returns_full_content_no_hint() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("small.md");
        std::fs::write(&file, synth_md(30, 2)).unwrap();

        let out = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            out.get("content").is_some(),
            "small tier must include content"
        );
        assert!(
            out.get("hint").is_none(),
            "small tier must not include hint"
        );
        assert!(out.get("file_id").is_none(), "small tier must not buffer");
        assert!(
            out.get("heading_map").is_none(),
            "small tier has no heading_map"
        );
        assert!(
            out.get("heading_count").is_some(),
            "small tier must report heading_count"
        );
    }

    #[tokio::test]
    async fn read_markdown_medium_returns_content_with_hint() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("medium.md");
        // 300 lines: > LINE_SOFT_CAP (150) but well under INLINE_BYTE_BUDGET.
        std::fs::write(&file, synth_md(300, 6)).unwrap();

        let out = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(out.get("content").is_some(), "medium tier includes content");
        assert!(out.get("hint").is_some(), "medium tier includes hint");
        assert!(
            out.get("heading_count").is_some(),
            "medium tier reports heading_count"
        );
        assert!(out.get("file_id").is_none(), "medium tier does not buffer");
        let hint = out["hint"].as_str().unwrap();
        assert!(
            hint.contains("heading="),
            "hint must reference heading-nav recipe"
        );
        assert!(
            !hint.contains("heading=\""),
            "hint must not wrap heading in quotes; got: {hint}"
        );
    }

    #[tokio::test]
    async fn read_markdown_large_returns_summary_no_content() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("large.md");
        // Force byte size above INLINE_BYTE_BUDGET. Each "body line\n" = 10 bytes;
        // 10_000 lines ≈ 100KB, comfortably above typical INLINE_BYTE_BUDGET.
        std::fs::write(&file, synth_md(10_000, 20)).unwrap();

        let out = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            out.get("content").is_none(),
            "large tier must NOT include content"
        );
        assert!(
            out.get("file_id").is_some(),
            "large tier buffers with file_id"
        );
        assert!(
            out.get("heading_map").is_some(),
            "large tier includes heading_map"
        );
        assert!(
            out.get("must_follow").is_some(),
            "large tier includes must_follow citing IRON LAW #6"
        );
        assert!(
            out.get("recipe").is_none(),
            "large tier no longer uses the recipe field — must_follow supersedes it"
        );
        assert!(out.get("total_lines").is_some());
        assert!(out.get("total_bytes").is_some());
        assert!(out.get("heading_count").is_some());
        let mf = out["must_follow"].as_str().unwrap();
        assert!(mf.contains("IRON LAW #6"), "must_follow cites IRON LAW #6");
        assert!(mf.contains("heading="), "must_follow mentions heading nav");
        assert!(
            !mf.contains("heading=\""),
            "must_follow must not wrap heading in quotes; got: {mf}"
        );
    }

    #[tokio::test]
    async fn read_markdown_large_includes_must_follow_citing_iron_law_6() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.md");
        std::fs::write(&file, synth_md(10_000, 20)).unwrap();

        let out = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let mf = out["must_follow"]
            .as_str()
            .expect("large-tier response must include must_follow");
        assert!(
            mf.contains("IRON LAW #6"),
            "must_follow must cite IRON LAW #6, got: {mf}"
        );
        assert!(
            mf.contains("@file_"),
            "must_follow must reference the file_id to steer reuse, got: {mf}"
        );
    }

    #[tokio::test]
    async fn heading_on_large_section_returns_ok_false_with_must_follow_and_section_map() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.md");
        // One H1 containing many H2 subsections — H1 match will be oversized.
        let mut body = String::from("# Root\n\n");
        for i in 0..200 {
            body.push_str(&format!("## Sub {i}\n\n"));
            body.push_str(&"word ".repeat(500));
            body.push_str("\n\n");
        }
        std::fs::write(&file, &body).unwrap();

        let err = super::ReadMarkdown
            .call(
                json!({ "path": file.to_str().unwrap(), "heading": "# Root" }),
                &ctx,
            )
            .await
            .unwrap_err();

        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("oversized heading must be RecoverableError (isError:false)");
        assert!(
            rec.message.contains("too large") || rec.message.contains("exceeds"),
            "error message should explain oversize; got: {}",
            rec.message
        );
        match &rec.guidance {
            Some(crate::tools::Guidance::MustFollow(s)) => {
                assert!(
                    s.contains("IRON LAW #6"),
                    "must_follow must cite IRON LAW #6; got: {s}"
                );
            }
            other => panic!("expected MustFollow guidance, got {:?}", other),
        }
        assert!(
            rec.extra.get("file_id").is_some(),
            "extra must include file_id for subsequent buffer-ref reads"
        );
        let sm = rec
            .extra
            .get("section_map")
            .expect("extra must include nested section_map");
        let arr = sm.as_array().expect("section_map is an array");
        assert!(
            !arr.is_empty(),
            "section_map must list nested sub-headings (H2s under H1)"
        );
        assert!(
            rec.extra.get("next_actions").is_some(),
            "extra must include concrete next_actions"
        );
    }

    // ── BUG-043: subsection-consumption detection ──────────────────────────

    /// `find_consumed_subsections` returns empty when the section has no nested
    /// sub-headings — safe to `replace` without losing structure.
    #[test]
    fn find_consumed_subsections_empty_for_leaf_section() {
        let content = "# Title\n## Setup\nsome content\n## Usage\nuse it\n";
        let result = super::find_consumed_subsections(content, "## Setup").unwrap();
        assert!(
            result.is_empty(),
            "leaf section has no subsections to consume: {result:?}"
        );
    }

    /// Core BUG-043 repro: `## File Map` is the only level-2 heading and is
    /// followed only by level-3 task headings. Its section extends to EOF,
    /// so `replace` would wipe every `###` task. `find_consumed_subsections`
    /// must return those `###` headings so the tool can refuse the edit.
    #[test]
    fn find_consumed_subsections_lists_level3_children_under_level2() {
        let content = "\
# Plan
intro

## File Map
map body

### Task A
work
### Task B
more work
### Task C
even more
";
        let result = super::find_consumed_subsections(content, "## File Map").unwrap();
        assert_eq!(
            result,
            vec![
                "### Task A".to_string(),
                "### Task B".to_string(),
                "### Task C".to_string(),
            ],
            "must list every heading that would be wiped by replace"
        );
    }

    /// Sibling `##` heading is NOT a child — doesn't count as consumed.
    #[test]
    fn find_consumed_subsections_stops_at_sibling_heading() {
        let content = "# Title\n## Setup\n### Step 1\ndo it\n## Usage\nuse it\n";
        let result = super::find_consumed_subsections(content, "## Setup").unwrap();
        assert_eq!(
            result,
            vec!["### Step 1".to_string()],
            "only the ### under ## Setup is consumed; ## Usage is a sibling"
        );
    }

    #[test]
    fn replace_body_only() {
        let content = "# Title\n## Setup\nold content\nmore old\n## Usage\nuse it\n";
        let result =
            perform_section_edit(content, "## Setup", "replace", Some("new content\n")).unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\n\nnew content\n## Usage\nuse it\n"
        );
    }

    #[test]
    fn replace_with_heading() {
        let content = "# Title\n## Setup\nold content\n## Usage\nuse it\n";
        let result = perform_section_edit(
            content,
            "## Setup",
            "replace",
            Some("## Installation\nnew steps\n"),
        )
        .unwrap();
        assert_eq!(
            result,
            "# Title\n## Installation\nnew steps\n## Usage\nuse it\n"
        );
    }

    #[test]
    fn replace_empty_section() {
        let content = "# Title\n## Empty\n## Next\nstuff\n";
        let result =
            perform_section_edit(content, "## Empty", "replace", Some("now has content\n"))
                .unwrap();
        assert_eq!(
            result,
            "# Title\n## Empty\n\nnow has content\n## Next\nstuff\n"
        );
    }

    #[test]
    fn insert_before() {
        let content = "# Title\n## Setup\ncontent\n";
        let result = perform_section_edit(
            content,
            "## Setup",
            "insert_before",
            Some("## Prerequisites\ninstall stuff\n"),
        )
        .unwrap();
        assert_eq!(
            result,
            "# Title\n## Prerequisites\ninstall stuff\n## Setup\ncontent\n"
        );
    }

    #[test]
    fn insert_after() {
        let content = "# Title\n## Setup\ncontent\n## Usage\nuse it\n";
        let result = perform_section_edit(
            content,
            "## Setup",
            "insert_after",
            Some("\n## Testing\ntest it\n"),
        )
        .unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\ncontent\n\n## Testing\ntest it\n## Usage\nuse it\n"
        );
    }

    #[test]
    fn remove_section() {
        let content = "# Title\n## Setup\ncontent\n\n## Usage\nuse it\n";
        let result = perform_section_edit(content, "## Setup", "remove", None).unwrap();
        assert_eq!(result, "# Title\n## Usage\nuse it\n");
    }

    #[test]
    fn remove_last_section() {
        let content = "# Title\n## Setup\ncontent\n";
        let result = perform_section_edit(content, "## Setup", "remove", None).unwrap();
        assert_eq!(result, "# Title\n");
    }

    #[test]
    fn nested_section_replace() {
        let content =
            "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling\n";
        let result =
            perform_section_edit(content, "## Parent", "replace", Some("replaced all\n")).unwrap();
        assert_eq!(
            result,
            "# Title\n## Parent\n\nreplaced all\n## Sibling\nsibling\n"
        );
    }

    #[test]
    fn trailing_newline_normalization() {
        let content = "# Title\n## Setup\ncontent";
        let result = perform_section_edit(content, "## Setup", "replace", Some("new")).unwrap();
        assert!(
            result.ends_with('\n'),
            "result should end with newline: {:?}",
            result
        );
    }

    #[test]
    fn replace_body_preserves_blank_line_after_heading() {
        let content = "# Title\n\n## Goals\n\n- item 1\n- item 2\n\n## Next\n\nmore\n";
        let result =
            perform_section_edit(content, "Goals", "replace", Some("- new item\n")).unwrap();
        assert!(
            result.contains("## Goals\n\n- new item\n"),
            "should have blank line between heading and body: {:?}",
            result
        );
    }

    #[test]
    fn replace_body_no_double_blank_when_content_starts_with_newline() {
        let content = "# Title\n\n## Goals\n\n- item 1\n";
        let result =
            perform_section_edit(content, "Goals", "replace", Some("\n- new item\n")).unwrap();
        assert!(
            result.contains("## Goals\n\n- new item\n"),
            "should not produce double blank line: {:?}",
            result
        );
        assert!(
            !result.contains("## Goals\n\n\n"),
            "must not have triple newline: {:?}",
            result
        );
    }

    #[test]
    fn remove_only_section() {
        let content = "## Only\ncontent\n";
        let result = perform_section_edit(content, "## Only", "remove", None).unwrap();
        assert!(result.trim().is_empty() || result == "\n");
    }

    #[test]
    fn consecutive_edits() {
        let content = "# Title\n## A\noriginal a\n## B\noriginal b\n";
        let after_first =
            perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
        assert!(after_first.contains("updated a"));
        let after_second =
            perform_section_edit(&after_first, "## B", "replace", Some("updated b\n")).unwrap();
        assert!(after_second.contains("updated a"));
        assert!(after_second.contains("updated b"));
    }

    #[test]
    fn smart_replace_detection_non_heading() {
        let content = "# Title\n## Setup\nold content\n";
        let result =
            perform_section_edit(content, "## Setup", "replace", Some("#hashtag comment\n"))
                .unwrap();
        assert!(result.contains("## Setup"));
        assert!(result.contains("#hashtag comment"));
    }

    #[test]
    fn heading_inside_code_block_edit() {
        // A heading inside a fenced code block is part of the section body,
        // so replacing the section should consume it.
        let content = "# Title\n## Real\ncontent\n```\n## Fake\n```\n";
        let result =
            perform_section_edit(content, "## Real", "replace", Some("new content\n")).unwrap();
        assert!(result.contains("## Real"));
        assert!(result.contains("new content"));
        // ## Fake is inside a code block — it's part of ## Real's body and gets replaced
        assert!(
            !result.contains("## Fake"),
            "code block content should be replaced as part of the section body"
        );
    }

    /// Regression: a level-1 heading inside a fenced code block must NOT split a
    /// level-2 section boundary. Without code-block tracking in `compute_section_end`,
    /// the `# comment` line would be treated as a section boundary, leaving a stray
    /// tail and corrupting the document.
    #[test]
    fn code_block_heading_different_level_does_not_split_section() {
        let content =
            "# Title\n## Section\ntext\n```bash\n# not a heading\nmore code\n```\n## Next\nstuff\n";
        let result =
            perform_section_edit(content, "## Section", "replace", Some("replaced\n")).unwrap();
        assert!(result.contains("## Section"));
        assert!(result.contains("replaced"));
        assert!(result.contains("## Next"));
        assert!(result.contains("stuff"));
        // The code block content must be consumed as part of ## Section's body
        assert!(
            !result.contains("# not a heading"),
            "code block content should have been replaced along with the section body"
        );
    }

    /// Regression: `insert_after` on a section whose body contains a fenced code
    /// block with a higher-level heading must insert AFTER the code block, not
    /// in the middle of it.
    #[test]
    fn insert_after_section_with_code_block_heading() {
        let content = "## Reading\n```bash\n# shell comment\nls -la\n```\n## Next\ntext\n";
        let result = perform_section_edit(
            content,
            "## Reading",
            "insert_after",
            Some("## Inserted\nnew section\n"),
        )
        .unwrap();
        // The inserted section should appear between ## Reading and ## Next
        let reading_pos = result.find("## Reading").unwrap();
        let inserted_pos = result.find("## Inserted").unwrap();
        let next_pos = result.find("## Next").unwrap();
        assert!(
            reading_pos < inserted_pos && inserted_pos < next_pos,
            "## Inserted should be between ## Reading and ## Next, got positions: reading={reading_pos}, inserted={inserted_pos}, next={next_pos}"
        );
        // The code block should remain intact inside ## Reading
        assert!(result.contains("# shell comment"));
    }

    #[test]
    fn duplicate_heading_edit_error() {
        let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond\n";
        let err = perform_section_edit(content, "## Example", "replace", Some("x")).unwrap_err();
        assert!(
            err.to_string().contains("found") && err.to_string().contains("times"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn heading_not_found() {
        let content = "# Title\n## Setup\ntext";
        let err =
            perform_section_edit(content, "## Nonexistent", "replace", Some("x")).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn missing_content_for_replace() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Setup", "replace", None).unwrap_err();
        assert!(
            err.to_string().contains("content"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn invalid_action() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Setup", "invalid", Some("x")).unwrap_err();
        assert!(
            err.to_string().contains("invalid"),
            "unexpected error: {}",
            err
        );
    }

    // ── perform_scoped_edit tests (action="edit") ────────────────────────

    #[test]
    fn scoped_edit_first_occurrence() {
        let content = "# Title\n## Setup\nfoo bar foo\nmore foo\n## Next\nfoo\n";
        let result = perform_scoped_edit(content, "## Setup", "foo", "baz", false).unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\nbaz bar foo\nmore foo\n## Next\nfoo\n"
        );
    }

    #[test]
    fn scoped_edit_replace_all() {
        let content = "# Title\n## Setup\nfoo bar foo\nmore foo\n## Next\nfoo\n";
        let result = perform_scoped_edit(content, "## Setup", "foo", "baz", true).unwrap();
        assert_eq!(
            result,
            "# Title\n## Setup\nbaz bar baz\nmore baz\n## Next\nfoo\n"
        );
    }

    #[test]
    fn scoped_edit_not_found() {
        let content = "# Title\n## Setup\ncontent\n";
        let err = perform_scoped_edit(content, "## Setup", "nonexistent", "x", false).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn scoped_edit_does_not_affect_other_sections() {
        let content = "# Title\n## A\nhello world\n## B\nhello world\n";
        let result = perform_scoped_edit(content, "## A", "hello", "goodbye", false).unwrap();
        assert!(result.contains("## A\ngoodbye world"));
        assert!(result.contains("## B\nhello world"));
    }

    #[test]
    fn scoped_edit_empty_replacement() {
        let content = "# Title\n## Setup\nremove this word\n";
        let result = perform_scoped_edit(content, "## Setup", " this", "", false).unwrap();
        assert_eq!(result, "# Title\n## Setup\nremove word\n");
    }

    // ── batch mode tests ────────────────────────────────────────────────

    #[test]
    fn batch_replace_two_sections() {
        let content = "# Title\n## A\nold a\n## B\nold b\n";
        let after_a = perform_section_edit(content, "## A", "replace", Some("new a\n")).unwrap();
        let after_b = perform_section_edit(&after_a, "## B", "replace", Some("new b\n")).unwrap();
        assert!(after_b.contains("new a"));
        assert!(after_b.contains("new b"));
    }

    #[test]
    fn batch_mixed_actions() {
        let content = "# Title\n## A\ncontent a\n## B\ncontent b\n## C\ncontent c\n";
        let step1 = perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
        let step2 = perform_section_edit(&step1, "## B", "remove", None).unwrap();
        let step3 = perform_section_edit(
            &step2,
            "## C",
            "insert_after",
            Some("\n## D\nnew section\n"),
        )
        .unwrap();
        assert!(step3.contains("updated a"));
        assert!(!step3.contains("## B"));
        assert!(step3.contains("## D\nnew section"));
    }

    #[test]
    fn batch_edit_action() {
        let content = "# Title\n## A\nhello world\n## B\nhello world\n";
        let result = perform_scoped_edit(content, "## A", "hello", "goodbye", false).unwrap();
        let result = perform_scoped_edit(&result, "## B", "hello", "hi", false).unwrap();
        assert!(result.contains("goodbye world"));
        assert!(result.contains("hi world"));
    }

    // ── fenced code block edge cases ────────────────────────────────────

    /// Multiple code blocks in a single section — all must be part of the section body.
    #[test]
    fn multiple_code_blocks_in_section() {
        let content = concat!(
            "# Title\n",
            "## Setup\n",
            "First block:\n",
            "```bash\n",
            "# install deps\n",
            "apt install foo\n",
            "```\n",
            "Second block:\n",
            "```python\n",
            "# run script\n",
            "import sys\n",
            "```\n",
            "## Next\n",
            "other\n",
        );
        let result =
            perform_section_edit(content, "## Setup", "replace", Some("simplified\n")).unwrap();
        assert!(result.contains("## Setup"));
        assert!(result.contains("simplified"));
        assert!(result.contains("## Next"));
        assert!(!result.contains("# install deps"));
        assert!(!result.contains("# run script"));
    }

    /// Code block with language tag — the ``` fence detection must work with ```bash, ```python, etc.
    #[test]
    fn code_block_with_language_tag() {
        let content = "## Sec\n```rust\n// # Not a heading\nfn main() {}\n```\n## Next\ntext\n";
        let result = perform_section_edit(content, "## Sec", "replace", Some("new\n")).unwrap();
        assert!(result.contains("## Sec"));
        assert!(result.contains("## Next"));
        assert!(!result.contains("fn main"));
    }

    /// Section whose entire body is a code block.
    #[test]
    fn section_body_is_entirely_code_block() {
        let content = "## Code\n```\n# heading-like\nsome code\n```\n## After\ntext\n";
        let result =
            perform_section_edit(content, "## Code", "replace", Some("replaced\n")).unwrap();
        assert_eq!(result, "## Code\n\nreplaced\n## After\ntext\n");
    }

    /// Code block at the very end of the file (last section, code block is last content).
    #[test]
    fn code_block_at_end_of_file() {
        let content = "# Title\n## Last\ntext\n```\n# inside fence\ncode\n```\n";
        let result =
            perform_section_edit(content, "## Last", "replace", Some("new last\n")).unwrap();
        assert!(result.contains("new last"));
        assert!(!result.contains("# inside fence"));
        assert!(result.ends_with('\n'));
    }

    /// Unclosed code fence — everything after ``` to EOF is "inside" the code block.
    /// The section boundary should extend to EOF since no real heading follows.
    #[test]
    fn unclosed_code_fence() {
        let content = "# Title\n## Broken\ntext\n```\n# looks like heading\ncode\n";
        let result =
            perform_section_edit(content, "## Broken", "replace", Some("fixed\n")).unwrap();
        assert!(result.contains("fixed"));
        // The unclosed fence content is part of the section — gets replaced
        assert!(!result.contains("# looks like heading"));
    }

    /// Multiple `#` levels inside a single code block — none should act as boundaries.
    #[test]
    fn multiple_heading_levels_inside_code_block() {
        let content = concat!(
            "## Section\n",
            "```markdown\n",
            "# H1 inside\n",
            "## H2 inside\n",
            "### H3 inside\n",
            "```\n",
            "## Real Next\n",
            "content\n",
        );
        let result =
            perform_section_edit(content, "## Section", "replace", Some("clean\n")).unwrap();
        assert!(result.contains("clean"));
        assert!(result.contains("## Real Next"));
        assert!(!result.contains("# H1 inside"));
        assert!(!result.contains("## H2 inside"));
        assert!(!result.contains("### H3 inside"));
    }

    /// Consecutive code fences with no content between them.
    #[test]
    fn consecutive_code_fences() {
        let content = "## Sec\n```\n# a\n```\n```\n# b\n```\n## Next\ntext\n";
        let result = perform_section_edit(content, "## Sec", "replace", Some("new\n")).unwrap();
        assert!(result.contains("## Next"));
        assert!(!result.contains("# a"));
        assert!(!result.contains("# b"));
    }

    /// Insert_before a section that is preceded by a code block ending.
    #[test]
    fn insert_before_section_after_code_block() {
        let content = "## First\ntext\n```\n# comment\n```\n## Second\nmore\n";
        let result = perform_section_edit(
            content,
            "## Second",
            "insert_before",
            Some("## Middle\ninserted\n"),
        )
        .unwrap();
        let first_pos = result.find("## First").unwrap();
        let middle_pos = result.find("## Middle").unwrap();
        let second_pos = result.find("## Second").unwrap();
        assert!(first_pos < middle_pos && middle_pos < second_pos);
    }

    /// Remove a section whose body contains code blocks.
    #[test]
    fn remove_section_with_code_blocks() {
        let content =
            "# Title\n## Keep\nkept\n## Remove\ntext\n```\n# fake\ncode\n```\n## Also Keep\nstuff\n";
        let result = perform_section_edit(content, "## Remove", "remove", None).unwrap();
        assert!(result.contains("## Keep"));
        assert!(result.contains("kept"));
        assert!(result.contains("## Also Keep"));
        assert!(result.contains("stuff"));
        assert!(!result.contains("## Remove"));
        assert!(!result.contains("# fake"));
    }

    /// Scoped edit (action="edit") within a section that has code blocks —
    /// the old_string/new_string should work on the full section body including code blocks.
    #[test]
    fn scoped_edit_in_section_with_code_block() {
        let content =
            "## Config\nSet `foo=bar` in config.\n```toml\n# main config\nfoo = \"bar\"\n```\n## Next\ntext\n";
        let result = perform_scoped_edit(content, "## Config", "foo", "baz", true).unwrap();
        assert!(result.contains("Set `baz=bar`"));
        assert!(result.contains("baz = \"bar\""));
        // Should not touch ## Next
        assert!(result.contains("## Next\ntext"));
    }

    // ── heading matching edge cases ─────────────────────────────────────

    /// Heading with inline code backticks — the tool should match via stripped formatting.
    #[test]
    fn heading_with_backtick_code() {
        let content = "# Title\n## The `auth` Module\ncontent\n## Other\ntext\n";
        // Query without backticks should match via strip_inline_formatting
        let result =
            perform_section_edit(content, "## The auth Module", "replace", Some("new\n")).unwrap();
        assert!(result.contains("new"));
        assert!(result.contains("## Other"));
    }

    /// Heading with bold formatting — matched via stripping.
    #[test]
    fn heading_with_bold_formatting() {
        let content = "# Title\n## **Important** Notes\ncontent\n";
        let result =
            perform_section_edit(content, "## Important Notes", "replace", Some("updated\n"))
                .unwrap();
        assert!(result.contains("updated"));
    }

    /// Prefix match — partial heading should match.
    #[test]
    fn heading_prefix_match() {
        let content = "# Title\n## Installation and Setup Guide\ncontent\n";
        let result =
            perform_section_edit(content, "## Installation", "replace", Some("simplified\n"))
                .unwrap();
        assert!(result.contains("simplified"));
    }

    // ── boundary conditions ─────────────────────────────────────────────

    /// Section with only whitespace lines as body.
    #[test]
    fn section_with_whitespace_only_body() {
        let content = "# Title\n## Empty-ish\n\n\n\n## Next\ncontent\n";
        let result =
            perform_section_edit(content, "## Empty-ish", "replace", Some("now has stuff\n"))
                .unwrap();
        assert!(result.contains("now has stuff"));
        assert!(result.contains("## Next"));
    }

    /// Replace the top-level `#` heading — its section spans to EOF (or next `#`),
    /// so all child sections (##, ###, etc.) are part of its body and get replaced.
    #[test]
    fn replace_top_level_heading_consumes_children() {
        let content = "# Title\nintro text\n## Child\nchild text\n";
        let result =
            perform_section_edit(content, "# Title", "replace", Some("new intro\n")).unwrap();
        assert!(result.contains("new intro"));
        // ## Child is a subsection of # Title — it gets replaced too
        assert!(
            !result.contains("## Child"),
            "child section should be consumed by parent replace"
        );
    }

    /// Insert after the last section in the document.
    #[test]
    fn insert_after_last_section() {
        let content = "# Title\n## Only\ncontent\n";
        let result = perform_section_edit(
            content,
            "## Only",
            "insert_after",
            Some("\n## Appended\nnew stuff\n"),
        )
        .unwrap();
        assert!(result.contains("## Only\ncontent"));
        assert!(result.contains("## Appended\nnew stuff"));
    }

    /// Deeply nested section (###) inside a ## section — replace ## consumes ### children.
    #[test]
    fn replace_consumes_nested_children() {
        let content =
            "# Title\n## Parent\ntext\n### Child1\nc1\n### Child2\nc2\n## Sibling\nother\n";
        let result =
            perform_section_edit(content, "## Parent", "replace", Some("flat now\n")).unwrap();
        assert!(result.contains("flat now"));
        assert!(result.contains("## Sibling"));
        assert!(!result.contains("### Child1"));
        assert!(!result.contains("### Child2"));
    }

    /// Code block inside a nested ### section — replace of parent ## should consume everything.
    #[test]
    fn code_block_inside_nested_child_consumed_by_parent_replace() {
        let content = concat!(
            "## Parent\n",
            "intro\n",
            "### Child\n",
            "```bash\n",
            "# shell comment\n",
            "echo hello\n",
            "```\n",
            "## Next\n",
            "other\n",
        );
        let result =
            perform_section_edit(content, "## Parent", "replace", Some("replaced\n")).unwrap();
        assert!(result.contains("replaced"));
        assert!(result.contains("## Next"));
        assert!(!result.contains("### Child"));
        assert!(!result.contains("# shell comment"));
    }

    #[tokio::test]
    async fn read_markdown_accepts_file_id_buffer_ref_for_line_range() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("large.md");
        std::fs::write(&file, synth_md(10_000, 20)).unwrap();

        // First call: populate the buffer via the large tier.
        let first = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let file_id = first["file_id"].as_str().unwrap().to_string();

        // Second call: use the buffer ref for a line slice.
        let slice = super::ReadMarkdown
            .call(
                json!({ "path": file_id, "start_line": 1, "end_line": 5 }),
                &ctx,
            )
            .await
            .unwrap();

        let content = slice["content"].as_str().unwrap();
        assert!(content.lines().count() <= 5);
    }

    #[tokio::test]
    async fn buffer_ref_accepts_single_heading_nav() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.md");
        std::fs::write(&file, synth_md(10_000, 20)).unwrap();

        let first = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let fid = first["file_id"].as_str().unwrap().to_string();

        let second = super::ReadMarkdown
            .call(json!({ "path": fid, "heading": "## Section 5" }), &ctx)
            .await
            .unwrap();
        assert!(
            second.get("content").is_some() || second.get("file_id").is_some(),
            "heading nav on @file_* must return content or a nested buffer, got: {second}"
        );
    }

    #[tokio::test]
    async fn buffer_ref_accepts_multi_heading_nav() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.md");
        // 500 sections keeps each section small (~20 lines) so combining two
        // doesn't overflow the inline limit, while the file total (~100KB) still
        // triggers Tier-3 and returns a file_id.
        std::fs::write(&file, synth_md(10_000, 500)).unwrap();

        let first = super::ReadMarkdown
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let fid = first["file_id"].as_str().unwrap().to_string();

        let second = super::ReadMarkdown
            .call(
                json!({
                    "path": fid,
                    "headings": ["## Section 3", "## Section 5"],
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(second["sections_returned"], 2);
    }
}
