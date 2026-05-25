//! EditMarkdown tool — heading-addressed section editing.

use anyhow::Result;
use serde_json::{json, Value};

use super::super::{parse_bool_param, RecoverableError, Tool, ToolContext};
use super::frontmatter;

// ── edit_markdown ────────────────────────────────────────────────────────────

// ---------------------------------------------------------------------------
// Helper functions (moved from section_edit.rs)
// ---------------------------------------------------------------------------

/// Pure string transformation: apply `action` to the section identified by `heading_query`.
///
/// Test-only thin wrapper that delegates to `perform_section_edit_ext` with
/// `at=None`, preserving the historical 4-arg signature for the test suite.
/// Production code (`EditMarkdown::call`) calls `perform_section_edit_ext`
/// directly so the `at` parameter threads through.
///
/// Returns the full modified file content (always ends with a single newline).
#[cfg(test)]
pub fn perform_section_edit(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
) -> Result<String> {
    perform_section_edit_ext(content, heading_query, action, new_content, None)
}

/// Extended form: `at` controls where `insert_after` lands. Pass
/// `Some("end-of-section")` (or `None`) for the historical behavior
/// of inserting at the end of the heading's section, or
/// `Some("after-heading-line")` to insert content immediately after
/// the heading line itself. Ignored by other actions.
pub fn perform_section_edit_ext(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
    at: Option<&str>,
) -> Result<String> {
    use crate::tools::file_summary::{heading_level, resolve_section_range};

    let range =
        resolve_section_range(content, heading_query).map_err(|e| anyhow::anyhow!("{}", e))?;

    let lines: Vec<&str> = content.split('\n').collect();
    let heading_idx = range.heading_line - 1;
    let end_idx = compute_section_end(&lines, heading_idx + 1, range.level);

    match action {
        "replace" => {
            let new = new_content
                .ok_or_else(|| anyhow::anyhow!("content is required for the replace action"))?;

            let replace_heading = new
                .lines()
                .next()
                .map(|l| heading_level(l.trim_end()).is_some())
                .unwrap_or(false);

            // F-3: a trailing horizontal-rule separator (`---`, `***`, `___`)
            // immediately before the next sibling heading is structurally a
            // between-sections separator, not the current section's content.
            // Wholesale-body replace silently destroys it. Shrink the replace
            // range to exclude that trailing HR so it survives the edit.
            // Only applies when the HR is preceded by at least one line of
            // real body content (a section whose entire body is just an HR
            // legitimately has that HR replaced).
            let body_start = heading_idx + 1;
            let replace_end_idx = {
                let mut walk = end_idx;
                while walk > body_start && lines[walk - 1].trim().is_empty() {
                    walk -= 1;
                }
                if walk <= body_start {
                    end_idx
                } else {
                    let hr_idx = walk - 1;
                    let line = lines[hr_idx];
                    let is_hr = !line.starts_with("    ") && {
                        let trimmed = line.trim();
                        match trimmed.chars().next() {
                            Some(marker @ ('-' | '*' | '_')) => {
                                let mut count = 0usize;
                                let mut ok = true;
                                for c in trimmed.chars() {
                                    if c == marker {
                                        count += 1;
                                    } else if c != ' ' {
                                        ok = false;
                                        break;
                                    }
                                }
                                ok && count >= 3
                            }
                            _ => false,
                        }
                    };
                    if !is_hr {
                        end_idx
                    } else {
                        let mut before_hr = hr_idx;
                        while before_hr > body_start && lines[before_hr - 1].trim().is_empty() {
                            before_hr -= 1;
                        }
                        if before_hr <= body_start {
                            end_idx
                        } else {
                            hr_idx
                        }
                    }
                }
            };

            let result = if replace_heading {
                let before = join_lines(&lines[..heading_idx]);
                let after = join_lines_tail(&lines[replace_end_idx..]);
                format!("{}{}{}", before, ensure_trailing_newline(new), after)
            } else {
                let heading_line_str = lines[heading_idx];
                let before = join_lines(&lines[..heading_idx]);
                let after = join_lines_tail(&lines[replace_end_idx..]);
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
            let insert_idx = match at.unwrap_or("end-of-section") {
                "end-of-section" => end_idx,
                "after-heading-line" => heading_idx + 1,
                other => {
                    return Err(anyhow::anyhow!(
                        "invalid at={:?}; expected 'end-of-section' (default) or 'after-heading-line'",
                        other
                    ));
                }
            };
            let before = join_lines(&lines[..insert_idx]);
            let after = join_lines_tail(&lines[insert_idx..]);
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
    // Mirror parse_all_headings: if ``` fences in the slice are unbalanced,
    // treat them as plain text so an unclosed fence in the section's body
    // doesn't swallow the next sibling heading.
    let fence_count = lines[start_idx..]
        .iter()
        .filter(|l| l.starts_with("```"))
        .count();
    let fences_balanced = fence_count % 2 == 0;

    let mut in_code_block = false;
    for (i, &line) in lines[start_idx..].iter().enumerate() {
        if fences_balanced && line.starts_with("```") {
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
    let heading_idx = range.heading_line - 1;
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
    let heading_idx = range.heading_line - 1;
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

/// Apply a JSON-shaped frontmatter mutation request to a markdown source.
///
/// `param` is the value of the tool's `frontmatter` field — expected to be an
/// object with optional `set` (object: key → JSON value) and `delete` (array of
/// strings) sub-fields. At least one of the two must be non-empty.
///
/// When the file has no existing frontmatter block, `set:` operations
/// synthesize a new block at the head of the file; `delete:`-only operations
/// are an idempotent no-op (nothing to delete from a non-existent block).
///
/// Returns the rewritten file content with the frontmatter block updated and
/// the body preserved verbatim.
pub(super) fn apply_frontmatter_mutation(content: &str, param: &Value) -> Result<String> {
    let obj = param.as_object().ok_or_else(|| {
        RecoverableError::with_hint(
            "frontmatter param must be an object",
            "Pass `frontmatter: {set: {key: value}, delete: [keys]}`.",
        )
    })?;

    let set: serde_json::Map<String, Value> = obj
        .get("set")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let delete: Vec<String> = obj
        .get("delete")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if set.is_empty() && delete.is_empty() {
        return Err(RecoverableError::with_hint(
            "frontmatter param requires at least one of `set` or `delete`",
            "Pass `frontmatter: {set: {key: value}}` or `frontmatter: {delete: [keys]}`.",
        )
        .into());
    }

    match frontmatter::extract_frontmatter(content)? {
        Some(fm) => {
            let new_block = frontmatter::apply_ops(&fm.lines, &set, &delete)?;
            Ok(frontmatter::splice_back(content, &new_block, &fm))
        }
        None => {
            if set.is_empty() {
                return Ok(content.to_string());
            }
            let new_block = frontmatter::apply_ops(&[], &set, &delete)?;
            let mut out = String::from("---\n");
            for line in &new_block {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("---\n");
            if !content.is_empty() && !content.starts_with('\n') {
                out.push('\n');
            }
            out.push_str(content);
            Ok(out)
        }
    }
}

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
         remove, edit. Supports batch mode via edits array. Optional `frontmatter: {set, delete}` \
         mutates the YAML frontmatter block atomically alongside any body edits."
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
             | 3c | `edit_markdown(path, edits=[...])` | Batch: multiple edits across sections, atomic |\n\
             | 3d | `edit_markdown(path, frontmatter={set: {status: \"fixed\"}})` | Mutate the YAML frontmatter block (status flips, closed dates, etc.) without sed. Combinable with any body edit above — one atomic write covers both. |\n\n\
             ### Action semantics — pick the right verb\n\n\
             | Action | Effect on target section | Use when |\n\
             |---|---|---|\n\
             | `replace` | **OVERWRITES the entire body** (from line after the heading until next sibling heading). Heading preserved; subsections refused unless `include_subsections=true`. | The whole section body should be rewritten from scratch (e.g. refreshing a stale memory table). |\n\
             | `insert_before` / `insert_after` | Adds a new sibling section before/after the target. Target body **preserved**. `at=\"end-of-section\"` (default) or `\"after-heading-line\"` for `insert_after`. | Adding adjacent sections without touching the target's body. |\n\
             | `remove` | Deletes target section (heading + body). | Removing a section entirely. |\n\
             | `edit` | Surgical text replacement within the section via `old_string` / `new_string`. Surrounding body preserved. | Fixing a typo, updating a single line, scoped substring change. |\n\n\
             **Common footgun:** reaching for `action=\"replace\"` when you meant `action=\"insert_after\"`. `replace` destroys the existing body; `insert_after` adds adjacent without loss. Verify-after-edit with `read_markdown(path, heading=\"...\")` on any non-trivial mutation."
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
                    "description": "Operation to perform. 'replace' OVERWRITES the entire body of the named section (heading preserved; body from the line after the heading until the next sibling heading is destroyed) — choose 'insert_after' to add an adjacent section, or 'edit' with old_string/new_string for in-section surgical replacement. 'insert_before' / 'insert_after' add a new sibling section before/after the target (target body preserved). 'remove' deletes the target section (heading + body). 'edit' performs scoped text replacement within the target section. Required unless using edits[] batch mode."
                },
                "content": { "type": "string", "description": "New content for replace/insert actions (body only — heading preserved on replace). For 'replace', this REPLACES the entire existing section body — read the section first if unsure." },
                "at": {
                    "type": "string",
                    "enum": ["end-of-section", "after-heading-line"],
                    "description": "For action='insert_after': where to insert. 'end-of-section' (default) places content at the end of the heading's section, after any nested sub-sections — useful for adding new H3/H4 to an existing section. 'after-heading-line' places content immediately after the heading line itself — useful when a top-level H1 wraps the whole doc and 'end-of-section' would mean EOF. Ignored by other actions."
                },
                "old_string": { "type": "string", "description": "For action='edit': exact text to find within section" },
                "new_string": { "type": "string", "description": "For action='edit': replacement text" },
                "replace_all": { "type": "boolean", "default": false, "description": "For action='edit': replace all occurrences" },
                "include_subsections": { "type": "boolean", "default": false, "description": "For action='replace': opt in to consuming nested sub-headings (deeper levels). Default refuses to wipe children — see BUG-043." },
                "force": { "type": "boolean", "default": false, "description": "Bypass the body-shrink guard. Required when the resulting file would be more than 50% shorter than the original. Use only when the shrinkage is intentional." },
                "frontmatter": {
                    "type": "object",
                    "description": "Mutate the YAML frontmatter block at the start of the file. Flat keys only (one-key-per-line; scalar / string / inline-array values). Combinable atomically with `edits` or `heading`+`action` in the same call. Example: `{set: {status: \"fixed\", closed: \"2026-05-17\"}, delete: [\"legacy_field\"]}`. At least one of `set` / `delete` must be non-empty.",
                    "properties": {
                        "set": {
                            "type": "object",
                            "additionalProperties": true,
                            "description": "Key → value pairs to set. Existing keys are updated in place (order preserved); new keys are appended at the end of the block. Values may be strings, numbers, booleans, null, or arrays of those."
                        },
                        "delete": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Keys to remove from the block. Missing keys are silently ignored (idempotent)."
                        }
                    }
                },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["heading", "action"],
                        "properties": {
                            "heading": { "type": "string" },
                            "action": { "type": "string", "enum": ["replace", "insert_before", "insert_after", "remove", "edit"], "description": "Per-edit operation. 'replace' OVERWRITES the entire body of the named section (see top-level `action` for full semantics) — prefer 'insert_after' for adjacent sections, 'edit' with old_string/new_string for in-section surgical mods." },
                            "content": { "type": "string", "description": "Per-edit content (body only — heading preserved on replace). For 'replace', this REPLACES the entire existing section body." },
                            "at": { "type": "string", "enum": ["end-of-section", "after-heading-line"] },
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

        // Reject librarian-managed artifacts — use artifact(action="update") instead.
        crate::util::librarian_guard::guard_not_librarian_managed(path, &file_content)?;

        // Working buffer — frontmatter mutation (if requested) lands here first,
        // then body edits run on the result. One atomic_write at the end keeps
        // mixed frontmatter+body edits transactional.
        let mut new_content = file_content.clone();

        // ── Frontmatter mutation (optional) ──────────────────────────
        let frontmatter_changed = if input["frontmatter"].is_object() {
            new_content = apply_frontmatter_mutation(&new_content, &input["frontmatter"])?;
            true
        } else {
            false
        };

        // ── Body edit mode detection ─────────────────────────────────
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

        let has_body_edit = has_edits || has_heading || has_action;
        if !frontmatter_changed && !has_body_edit {
            return Err(RecoverableError::with_hint(
                "no operation specified",
                "Pass `frontmatter: {set:{...}, delete:[...]}`, `edits=[...]`, or `heading`+`action`.",
            )
            .into());
        }

        if has_edits {
            // ── Batch mode ───────────────────────────────────────────
            let edits = input["edits"].as_array().unwrap();
            for (i, edit) in edits.iter().enumerate() {
                let heading = edit["heading"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("edits[{}]: missing required 'heading' field", i)
                })?;
                let action = edit["action"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("edits[{}]: missing required 'action' field", i)
                })?;

                new_content = if action == "edit" {
                    let old_string = edit["old_string"].as_str().ok_or_else(|| {
                        anyhow::anyhow!("edits[{}]: old_string is required for action='edit'", i)
                    })?;
                    let new_string = edit["new_string"].as_str().unwrap_or("");
                    let replace_all_val = edit["replace_all"].as_bool().unwrap_or(false);
                    perform_scoped_edit(
                        &new_content,
                        heading,
                        old_string,
                        new_string,
                        replace_all_val,
                    )
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
                        if let Ok(victims) = find_consumed_subsections(&new_content, heading) {
                            if !victims.is_empty() {
                                return Err(
                                    subsection_guard_error(Some(i), heading, &victims).into()
                                );
                            }
                        }
                    }
                    perform_section_edit_ext(
                        &new_content,
                        heading,
                        action,
                        edit_content,
                        edit["at"].as_str(),
                    )
                    .map_err(|e| {
                        RecoverableError::with_hint(
                            format!("edits[{}]: {}", i, e),
                            "Check heading name and action.",
                        )
                    })?
                };
            }
        } else if has_body_edit {
            // ── Single edit mode ─────────────────────────────────────
            let heading = crate::tools::require_str_param(&input, "heading")?;
            let action = crate::tools::require_str_param(&input, "action")?;

            new_content = if action == "edit" {
                let old_string = crate::tools::require_str_param(&input, "old_string")?;
                let new_string = input["new_string"].as_str().unwrap_or("");
                let replace_all_val = parse_bool_param(&input["replace_all"]);
                perform_scoped_edit(
                    &new_content,
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
                    if let Ok(victims) = find_consumed_subsections(&new_content, heading) {
                        if !victims.is_empty() {
                            return Err(subsection_guard_error(None, heading, &victims).into());
                        }
                    }
                }
                perform_section_edit_ext(
                    &new_content,
                    heading,
                    action,
                    content,
                    input["at"].as_str(),
                )
                .map_err(|e| {
                    RecoverableError::with_hint(e.to_string(), "Check heading name and action.")
                })?
            };
        }

        // ── Body-shrink guard ──────────────────────────────────────
        // Refuse a write that would reduce the file by >50% unless the
        // caller passed `force: true`. Files smaller than 200 bytes are
        // exempt — the threshold is meaningless for near-empty shells.
        // The parallel guard on artifact(update) lives in
        // src/librarian/tools/update.rs (load-bearing site for the
        // augmented-tracker body-overwrite footgun).
        const SHRINK_GUARD_MIN_BYTES: usize = 200;
        let force = input["force"].as_bool().unwrap_or(false);
        if !force
            && file_content.len() >= SHRINK_GUARD_MIN_BYTES
            && new_content.len() * 2 < file_content.len()
        {
            let pct = 100 - (new_content.len() * 100 / file_content.len().max(1));
            return Err(RecoverableError::with_hint(
                format!(
                    "body-shrink guard: write to {} would reduce {} → {} bytes ({}% reduction)",
                    resolved.display(),
                    file_content.len(),
                    new_content.len(),
                    pct
                ),
                "Use action='edit' with old_string/new_string for surgical mutation, or pass force=true if the shrinkage is intentional.",
            )
            .into());
        }

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
