//! `edit_file` tool and language-aware edit helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::{parse_bool_param, Tool, ToolContext};

/// Returns definition keywords for a specific language.
/// Only includes keywords that actually introduce definitions in that language.
fn def_keywords_for_lang(lang: &str) -> &'static [&'static str] {
    match lang {
        "rust" => &["fn ", "async fn ", "struct ", "impl ", "trait ", "enum "],
        "python" => &["def ", "async def ", "class "],
        "go" => &["func ", "struct ", "interface "],
        "typescript" | "tsx" | "javascript" | "jsx" => &[
            "function ",
            "async function ",
            "class ",
            "interface ",
            "enum ",
        ],
        "java" => &["class ", "interface ", "enum "],
        "kotlin" => &["fun ", "class ", "interface ", "enum "],
        "c" | "cpp" => &["struct ", "class ", "enum "],
        "csharp" => &["class ", "struct ", "interface ", "enum "],
        "ruby" => &["def ", "class "],
        _ => &[],
    }
}

/// Returns the matched definition keyword for error reporting, if any.
fn find_def_keyword(s: &str, lang: &str) -> Option<&'static str> {
    def_keywords_for_lang(lang)
        .iter()
        .find(|kw| s.contains(**kw))
        .copied()
}

#[derive(Debug, Clone, PartialEq)]
struct NormWindow {
    start_line: usize,
    end_line: usize,
    start_byte: usize,
    end_byte: usize,
}

fn split_old_lines(old_string: &str) -> Vec<&str> {
    let mut v: Vec<&str> = old_string.split('\n').collect();
    if v.len() > 1 && v.last() == Some(&"") {
        v.pop();
    }
    v
}

fn find_normalized_windows(content: &str, old_string: &str) -> Vec<NormWindow> {
    let old_lines = split_old_lines(old_string);
    let k = old_lines.len();
    if k == 0 {
        return Vec::new();
    }
    let mut spans: Vec<(&str, usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for raw in content.split_inclusive('\n') {
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        spans.push((text, offset, offset + text.len()));
        offset += raw.len();
    }
    let mut out = Vec::new();
    if spans.len() < k {
        return out;
    }
    for i in 0..=(spans.len() - k) {
        if (0..k).all(|j| spans[i + j].0.trim() == old_lines[j].trim()) {
            out.push(NormWindow {
                start_line: i + 1,
                end_line: i + k,
                start_byte: spans[i].1,
                end_byte: spans[i + k - 1].2,
            });
        }
    }
    out
}

fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn reindent_block(new_string: &str, agent_base: &str, file_base: &str) -> String {
    let mut out = String::with_capacity(new_string.len());
    for (idx, line) in new_string.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix(agent_base) {
            out.push_str(file_base);
            out.push_str(rest);
        } else {
            out.push_str(file_base);
            out.push_str(line.trim_start());
        }
    }
    out
}

/// Best-effort nearest window for an error hint when no unique normalized match
/// exists. Returns (start_line, end_line, actual_text) of the content window with
/// the highest count of normalized-matching lines against `old_string`.
fn nearest_window_hint(content: &str, old_string: &str) -> Option<(usize, usize, String)> {
    let old_lines = split_old_lines(old_string);
    let k = old_lines.len();
    if k == 0 {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < k {
        return None;
    }
    let mut best: Option<(usize, usize)> = None; // (score, start_index)
    for i in 0..=(lines.len() - k) {
        let score = (0..k)
            .filter(|&j| lines[i + j].trim() == old_lines[j].trim())
            .count();
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, i));
        }
    }
    best.filter(|&(score, _)| score > 0)
        .map(|(_, i)| (i + 1, i + k, lines[i..i + k].join("\n")))
}
async fn commit_edit(
    ctx: &ToolContext,
    resolved: &std::path::Path,
    new_content: &str,
) -> anyhow::Result<()> {
    crate::util::fs::atomic_write(resolved, new_content)?;
    ctx.agent
        .reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), resolved)
        .await;
    ctx.lsp.notify_file_changed(resolved).await;
    ctx.agent
        .invalidate_call_edges_for(ctx.workspace_override.as_deref(), resolved)
        .await;
    ctx.agent
        .mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved.to_path_buf())
        .await;
    Ok(())
}

/// Returns the language if the file has LSP support, None otherwise.
fn detect_lsp_language(path: &str) -> Option<&'static str> {
    let p = std::path::Path::new(path);
    let lang = crate::ast::detect_language(p)?;
    if crate::lsp::servers::has_lsp_config(lang) {
        Some(lang)
    } else {
        None
    }
}

/// Suggests the right symbol tool when `edit_file` blocks a structural source edit.
/// Called only after the gate confirms a definition keyword is present.
fn infer_edit_hint(old_string: &str, new_string: &str) -> &'static str {
    if new_string.is_empty() {
        return "edit_code(symbol, path, action='remove') — deletes the symbol and its doc comments/attributes";
    }
    if new_string.len() > old_string.len() {
        return "edit_code(symbol, path, action='insert', body=..., position=...) — inserts before or after a named symbol";
    }
    "edit_code(symbol, path, action='replace', body=...) — replaces the symbol body via LSP"
}

/// Returns Err if the edit looks structural for an LSP-supported source file.
///
/// Two patterns are blocked:
///   1. Multi-line `old_string` containing a definition keyword — rewriting an
///      existing symbol via raw text replacement.
///   2. Multi-line `new_string` containing a definition keyword — introducing a
///      *new* symbol whose placement depends entirely on the `old_string`
///      anchor. BUG-050: a single-line `old_string` here lets a new `fn`
///      silently splice into an unrelated function body.
///
/// Both routes should go through `edit_code` instead.
fn guard_structural_rewrite(
    path: &str,
    old_string: &str,
    new_string: &str,
) -> Result<(), super::RecoverableError> {
    if !crate::util::path_security::is_source_path(path) {
        return Ok(());
    }
    let Some(lang) = detect_lsp_language(path) else {
        return Ok(());
    };

    let old_kw = old_string
        .contains('\n')
        .then(|| find_def_keyword(old_string, lang))
        .flatten();
    let new_kw = new_string
        .contains('\n')
        .then(|| find_def_keyword(new_string, lang))
        .flatten();

    let Some(keyword) = old_kw.or(new_kw) else {
        return Ok(());
    };

    let hint = infer_edit_hint(old_string, new_string);
    Err(super::RecoverableError::with_hint(
        format!(
            "edit contains a symbol definition ({keyword:?}) — \
             use symbol tools for structural changes"
        ),
        hint,
    ))
}

/// Returns true if any edit in `input` would be rejected by `guard_structural_rewrite`.
/// Used by `debug_enforce_symbol_tools` to distinguish structural edits (route to
/// `edit_code`) from literal string substitutions (allow through).
fn is_structural_edit(input: &Value, path: &str) -> bool {
    if let Some(edits) = input["edits"].as_array() {
        return edits.iter().any(|e| {
            let old = e["old_string"].as_str().unwrap_or("");
            let new = e["new_string"].as_str().unwrap_or("");
            guard_structural_rewrite(path, old, new).is_err()
        });
    }
    if let Some(old) = input["old_string"].as_str() {
        let new = input["new_string"].as_str().unwrap_or("");
        return guard_structural_rewrite(path, old, new).is_err();
    }
    // prepend/append: no old_string, never structural
    false
}

pub struct EditFile;

#[async_trait::async_trait]
impl Tool for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Exact string replacement in a file. Whitespace-sensitive. \
         Use insert: \"prepend\"/\"append\" for file boundaries. \
         On whitespace-only mismatch, applies the unique normalized match \
         re-indented (response: applied_via: \"whitespace-normalized match\"); \
         0 or 2+ matches -> error."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "old_string": { "type": "string", "description": "Exact text to find (whitespace-sensitive). Required unless insert or edits is set." },
                "new_string": { "type": "string", "description": "Replacement text (empty string = delete). Required for single-edit and insert modes." },
                "replace_all": { "type": "boolean", "default": false, "description": "Replace all occurrences." },
                "insert": { "type": "string", "enum": ["prepend", "append"], "description": "Insert at file start/end (old_string not required)." },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" },
                            "replace_all": { "type": "boolean" }
                        },
                        "required": ["old_string", "new_string"]
                    },
                    "description": "Batch mode: array of edit operations applied atomically. Top-level new_string not used."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let path = super::require_str_param(&input, "path")?;
        let new_string = input["new_string"].as_str().unwrap_or("");

        // Gate: redirect .md files to edit_markdown (except prepend/append
        // boundary inserts, and replace_all global swaps). The replace_all
        // exception covers the "rename an ID / date / brand across the
        // whole file" case where edit_markdown's heading-scoped editor
        // adds friction without adding safety. Single non-replace_all
        // edits and mixed-batch edits stay routed to edit_markdown.
        if path.ends_with(".md") || path.ends_with(".markdown") {
            let insert_mode = input["insert"].as_str();
            let single_replace_all = input["replace_all"].as_bool().unwrap_or(false);
            let batch_all_replace_all = input["edits"].as_array().and_then(|edits| {
                if edits.is_empty() {
                    None
                } else {
                    Some(
                        edits
                            .iter()
                            .all(|e| e["replace_all"].as_bool().unwrap_or(false)),
                    )
                }
            });
            let allowed = matches!(insert_mode, Some("prepend") | Some("append"))
                || single_replace_all
                || batch_all_replace_all.unwrap_or(false);
            if !allowed {
                return Err(super::RecoverableError::with_hint(
                    "Use edit_markdown for markdown files",
                    "edit_markdown provides heading-based editing for .md files. edit_file is still allowed with insert='prepend'/'append' or replace_all=true (file-wide find/replace).",
                ).into());
            }
        }

        // Gate: debug_enforce_symbol_tools — block structural edits on source files.
        // Literal string substitutions (single-line, no definition keywords) are
        // allowed through; only multi-line edits containing definition keywords are
        // routed to edit_code.
        if crate::util::path_security::is_source_path(path) {
            let security = ctx
                .agent
                .security_config_for(ctx.workspace_override.as_deref())
                .await;
            if security.debug_enforce_symbol_tools && is_structural_edit(&input, path) {
                return Err(super::RecoverableError::with_hint(
                    "edit_file is blocked for structural edits on source code files (debug_enforce_symbol_tools is enabled)",
                    "Use edit_code(symbol, path, action='replace'|'insert'|'remove'|'rename') \
                     for structural changes, or edit_code(action='rename') for renames.",
                )
                .into());
            }
        }

        // Batch mode — edits array takes precedence over single old_string.
        let edits = super::optional_array_param(&input, "edits");
        let has_old_string = input["old_string"].as_str().is_some();

        if edits.is_some() && has_old_string {
            return Err(super::RecoverableError::with_hint(
                "edits and old_string are mutually exclusive",
                "Use edits for batch mode, or old_string/new_string for single edit.",
            )
            .into());
        }

        if let Some(edits_arr) = edits {
            if edits_arr.is_empty() {
                return Err(super::RecoverableError::with_hint(
                    "edits array is empty",
                    "Pass at least one edit object {old_string, new_string} in the edits array, or use the single-edit form with top-level old_string/new_string.",
                )
                .into());
            }
            let root = ctx
                .agent
                .require_project_root_for(ctx.workspace_override.as_deref())
                .await?;
            let security = ctx
                .agent
                .security_config_for(ctx.workspace_override.as_deref())
                .await;
            let session_roots = ctx
                .agent
                .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
                .await;
            let resolved = crate::util::path_security::validate_write_path(
                path,
                &root,
                &security,
                &session_roots,
            )?;
            let mut content = std::fs::read_to_string(&resolved)?;

            // Pre-pass: identify structural edits before applying any. When the
            // batch mixes safe edits with structural ones, the caller benefits
            // from knowing *which* edits would have been safe so they can split
            // the call (safe ones via `edit_file`, structural ones via
            // `edit_code`). Failing on the first structural edit alone leaves
            // the caller to discover the split heuristically.
            let mut structural_failures: Vec<(usize, String)> = Vec::new();
            let mut safe_indices: Vec<usize> = Vec::new();
            for (i, edit) in edits_arr.iter().enumerate() {
                let old_s = edit["old_string"].as_str().unwrap_or("");
                let new_s = edit["new_string"].as_str().unwrap_or("");
                if old_s.is_empty() {
                    // Empty old_string is caught in the application loop below
                    // with its own error message — skip here so the structural
                    // taxonomy stays clean.
                    continue;
                }
                match guard_structural_rewrite(path, old_s, new_s) {
                    Ok(()) => safe_indices.push(i),
                    Err(e) => structural_failures.push((i, e.message)),
                }
            }
            if !structural_failures.is_empty() {
                let (first_idx, first_msg) = &structural_failures[0];
                let structural_list = structural_failures
                    .iter()
                    .map(|(i, _)| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let safe_list = if safe_indices.is_empty() {
                    String::from("none")
                } else {
                    safe_indices
                        .iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                return Err(super::RecoverableError::with_hint(
                    format!("edit[{first_idx}]: {first_msg}"),
                    format!(
                        "Batch aborted — structural edits at index(es) [{structural_list}] \
                         must use edit_code. Edits that would have applied safely at \
                         index(es) [{safe_list}]. To proceed: split the batch — call \
                         edit_file with only [{safe_list}], then use edit_code for the \
                         structural edit(s)."
                    ),
                )
                .into());
            }

            for (i, edit) in edits_arr.iter().enumerate() {
                let old_s = edit["old_string"].as_str().ok_or_else(|| {
                    super::RecoverableError::new(format!("edit[{i}]: old_string is required"))
                })?;
                let new_s = edit["new_string"].as_str().unwrap_or("");
                let replace_all_edit = parse_bool_param(&edit["replace_all"]);

                if old_s.is_empty() {
                    return Err(super::RecoverableError::with_hint(
                        format!("edit[{i}]: old_string must not be empty"),
                        "Each edit must have a non-empty old_string.",
                    )
                    .into());
                }

                let match_count = content.matches(old_s).count();
                if match_count == 0 {
                    return Err(super::RecoverableError::with_hint(
                        format!("edit[{i}]: old_string not found"),
                        "Batch aborted — no changes written.",
                    )
                    .into());
                }
                if match_count > 1 && !replace_all_edit {
                    return Err(super::RecoverableError::with_hint(
                        format!("edit[{i}]: old_string found {match_count} times"),
                        "Add more context or set replace_all: true. Batch aborted.",
                    )
                    .into());
                }
                if replace_all_edit {
                    content = content.replace(old_s, new_s);
                } else {
                    content = content.replacen(old_s, new_s, 1);
                }
            }

            // All edits passed — write once (atomic to prevent corruption on crash).
            crate::util::fs::atomic_write(&resolved, &content)?;
            ctx.agent
                .reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), &resolved)
                .await;
            ctx.lsp.notify_file_changed(&resolved).await;
            ctx.agent
                .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &resolved)
                .await;
            ctx.agent
                .mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved)
                .await;
            return Ok(json!("ok"));
        }

        // Prepend/append mode — no string match needed.
        if let Some(insert) = input["insert"].as_str() {
            if !input["new_string"].is_string() {
                return Err(super::RecoverableError::with_hint(
                    "new_string is required",
                    "Pass new_string as a string. To insert nothing, use new_string: \"\".",
                )
                .into());
            }
            let root = ctx
                .agent
                .require_project_root_for(ctx.workspace_override.as_deref())
                .await?;
            let security = ctx
                .agent
                .security_config_for(ctx.workspace_override.as_deref())
                .await;
            let session_roots = ctx
                .agent
                .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
                .await;
            let resolved = crate::util::path_security::validate_write_path(
                path,
                &root,
                &security,
                &session_roots,
            )?;
            let content = std::fs::read_to_string(&resolved)?;
            // Reject librarian-managed artifacts — use artifact(action="update") instead.
            crate::util::librarian_guard::guard_not_librarian_managed(path, &content)?;
            let new_content = match insert {
                "prepend" => format!("{}{}", new_string, content),
                "append" => format!("{}{}", content, new_string),
                _ => {
                    return Err(super::RecoverableError::with_hint(
                        format!("invalid insert value: {insert:?}"),
                        "insert must be \"prepend\" or \"append\"",
                    )
                    .into())
                }
            };
            crate::util::fs::atomic_write(&resolved, &new_content)?;
            ctx.lsp.notify_file_changed(&resolved).await;
            ctx.agent
                .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &resolved)
                .await;
            ctx.agent
                .mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved)
                .await;
            return Ok(json!("ok"));
        }

        let old_string = super::require_str_param_or(
            &input,
            "old_string",
            &["old_code", "old_content", "old_text"],
        )?;
        let replace_all = parse_bool_param(&input["replace_all"]);

        if old_string.is_empty() {
            return Err(super::RecoverableError::with_hint(
                "old_string must not be empty",
                "To create a new file use create_file. To insert adjacent to a symbol use edit_code(action='insert'). To prepend or append to a file use insert: \"prepend\" or \"append\".",
            )
            .into());
        }

        // Hard-block multi-line edits that contain definition keywords on LSP-supported languages.
        guard_structural_rewrite(path, old_string, new_string)?;

        // Validate new_string is an explicit string — null/missing must error,
        // not silently delete. Empty string "" is valid (explicit deletion).
        if !input["new_string"].is_string() {
            return Err(super::RecoverableError::with_hint(
                "new_string is required",
                "Pass new_string as a string. To delete matched text, use new_string: \"\".",
            )
            .into());
        }

        perform_edit(path, old_string, new_string, replace_all, ctx).await
    }
}

async fn perform_edit(
    path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    ctx: &ToolContext,
) -> Result<Value> {
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    let session_roots = ctx
        .agent
        .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
        .await;
    let resolved =
        crate::util::path_security::validate_write_path(path, &root, &security, &session_roots)?;

    let content = std::fs::read_to_string(&resolved)?;

    let match_count = content.matches(old_string).count();

    if match_count == 0 {
        let windows = find_normalized_windows(&content, old_string);
        match windows.len() {
            1 => {
                let w = &windows[0];
                let matched = &content[w.start_byte..w.end_byte];
                let first_file_line = matched.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                let file_base = leading_ws(first_file_line).to_string();
                let agent_base = split_old_lines(old_string)
                    .into_iter()
                    .find(|l| !l.trim().is_empty())
                    .map(|l| leading_ws(l).to_string())
                    .unwrap_or_default();
                // Strip one trailing newline: the matched span excludes the last line's
                // newline (content[w.end_byte..] supplies it), so the replacement must not
                // re-emit one or we double the newline.
                let replacement_src = new_string.strip_suffix('\n').unwrap_or(new_string);
                let reindented = reindent_block(replacement_src, &agent_base, &file_base);
                let mut new_content = String::with_capacity(content.len());
                new_content.push_str(&content[..w.start_byte]);
                new_content.push_str(&reindented);
                new_content.push_str(&content[w.end_byte..]);

                if let Some(lang) = crate::ast::detect_language(std::path::Path::new(path)) {
                    let before = crate::ast::has_syntax_errors(&content, lang);
                    let after = crate::ast::has_syntax_errors(&new_content, lang);
                    if after && !before {
                        return Err(super::RecoverableError::with_hint(
                            format!(
                                "whitespace-normalized match at lines {}-{} would introduce syntax errors — not written",
                                w.start_line, w.end_line
                            ),
                            "Verify the target with read_file and retry edit_file with the exact text.",
                        )
                        .into());
                    }
                }

                commit_edit(ctx, &resolved, &new_content).await?;
                if path.ends_with(".md") || path.ends_with(".markdown") {
                    if let Ok(mut cov) = ctx.section_coverage.lock() {
                        cov.update_mtime(&resolved);
                    }
                }
                return Ok(json!({
                    "status": "ok",
                    "applied_via": "whitespace-normalized match",
                    "lines": format!("{}-{}", w.start_line, w.end_line),
                    "note": "old_string matched after normalizing indentation/line-endings; verify the result"
                }));
            }
            0 => {
                let msg = match nearest_window_hint(&content, old_string) {
                    Some((s, e, text)) => format!(
                        "old_string not found in {path}. Nearest content at lines {s}-{e}:\n{text}"
                    ),
                    None => format!("old_string not found in {path}"),
                };
                return Err(super::RecoverableError::with_hint(
                    msg,
                    "No exact or whitespace-normalized match. Copy the actual bytes shown (or from read_file) and retry.",
                ).into());
            }
            _ => {
                let ranges = windows
                    .iter()
                    .map(|w| format!("{}-{}", w.start_line, w.end_line))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(super::RecoverableError::with_hint(
                    format!("old_string matches {} regions after whitespace normalization (lines {ranges})", windows.len()),
                    "Ambiguous — add surrounding context so exactly one region matches, or fix whitespace to match one exactly.",
                ).into());
            }
        }
    }

    if match_count > 1 && !replace_all {
        let line_numbers: Vec<usize> = content
            .match_indices(old_string)
            .map(|(byte_offset, _)| content[..byte_offset].lines().count() + 1)
            .collect();
        let lines_str = line_numbers
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(super::RecoverableError::with_hint(
            format!(
                "old_string found {match_count} times (lines {lines_str}). Include more surrounding context or use replace_all: true."
            ),
            "Expand old_string to include unique surrounding context, or set replace_all: true to replace every occurrence.",
        )
        .into());
    }

    let new_content = content.replace(old_string, new_string);
    commit_edit(ctx, &resolved, &new_content).await?;

    // Syntax check: warn if the edit introduced parse errors (non-fatal).
    if let Some(lang) = crate::ast::detect_language(std::path::Path::new(path)) {
        if crate::ast::has_syntax_errors(&new_content, lang) {
            return Ok(json!({
                "status": "ok",
                "warning": "syntax error detected after edit — file may be malformed. Use read_file to inspect and fix."
            }));
        }
    }

    // Update section-coverage mtime on markdown writes so the next read
    // doesn't spuriously invalidate. The unread-section hint field was removed
    // (telemetry showed it never fired across ~1.7k edit_file calls).
    if path.ends_with(".md") || path.ends_with(".markdown") {
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(&resolved);
        }
    }

    Ok(json!("ok"))
}

#[cfg(test)]
#[cfg(test)]
mod tests;
