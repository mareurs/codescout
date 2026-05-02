//! EditMarkdown tool — heading-addressed section editing.

use anyhow::Result;
use serde_json::{json, Value};

use super::super::{parse_bool_param, RecoverableError, Tool, ToolContext};

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
pub(crate) fn perform_scoped_edit(
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
        crate::tools::guard_worktree_write(ctx).await?;
        let path = crate::tools::require_str_param(&input, "path")?;

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
        let session_roots = ctx.agent.session_write_roots_snapshot().await;
        let resolved = crate::util::path_security::validate_write_path(
            path,
            &root,
            &security,
            &session_roots,
        )?;

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
            let heading = crate::tools::require_str_param(&input, "heading")?;
            let action = crate::tools::require_str_param(&input, "action")?;

            if action == "edit" {
                let old_string = crate::tools::require_str_param(&input, "old_string")?;
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
        ctx.agent.invalidate_call_edges(&resolved).await;
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
