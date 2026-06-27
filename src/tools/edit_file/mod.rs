//! `edit_file` tool and language-aware edit helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::{parse_bool_param, Tool, ToolContext};
use crate::tools::edit_repair::{
    decode_literal_escapes, decode_literal_escapes_incl_quotes, finalize_edit_content,
    RepairResult, REPAIR_NOTE,
};
use crate::util::text::{leading_ws, reindent_block};

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
/// Comment lines (// /* * #) are skipped so a keyword inside a comment
/// does not falsely trip the structural-rewrite guard.
fn find_def_keyword(s: &str, lang: &str) -> Option<&'static str> {
    let keywords = def_keywords_for_lang(lang);
    s.lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("//")
                && !t.starts_with("/*")
                && !t.starts_with('*')
                && !t.starts_with('#')
        })
        .find_map(|line| keywords.iter().find(|kw| line.contains(**kw)).copied())
}

/// Lines present in `from` but not (byte-identical) in `to`. Restricts the
/// structural-keyword check to the lines an edit actually adds or removes — a
/// keyword on an unchanged context line is an anchor, not a rewrite.
fn lines_only_in<'a>(from: &'a str, to: &str) -> Vec<&'a str> {
    let to_lines: std::collections::HashSet<&str> = to.lines().collect();
    from.lines().filter(|l| !to_lines.contains(l)).collect()
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
/// Languages where leading whitespace is semantically significant. For these the
/// whitespace-normalized fallback is disabled: trim-matching erases the very thing
/// that carries meaning, and a re-indent that moves a line into a different block
/// can still parse cleanly — so the AST gate cannot catch it. Steer the caller back
/// to exact matching instead. Classified by extension because `detect_language`
/// does not recognize YAML at all (it would otherwise slip through ungated).
fn indentation_significant(path: &str) -> bool {
    matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str()),
        Some("py" | "pyi" | "hs" | "yaml" | "yml")
    )
}

/// Shared "not found — here is the nearest content" message used by both no-match
/// paths (the indentation-significant guard and the zero-window arm).
fn not_found_msg(content: &str, old_string: &str, path: &str) -> String {
    match nearest_window_hint(content, old_string) {
        Some((s, e, text)) => {
            format!("old_string not found in {path}. Nearest content at lines {s}-{e}:\n{text}")
        }
        None => format!("old_string not found in {path}"),
    }
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

    // Diff-aware: scan only the lines the edit adds/removes, not the whole string.
    // A definition keyword on a line that is byte-identical in old and new is
    // unchanged context (an anchor), not a structural rewrite — ignore it. A newly
    // introduced symbol line (BUG-050) is by construction absent from old_string,
    // so route 2 still fires.
    let old_changed = lines_only_in(old_string, new_string).join("\n");
    let new_changed = lines_only_in(new_string, old_string).join("\n");
    let old_kw = old_string
        .contains('\n')
        .then(|| find_def_keyword(&old_changed, lang))
        .flatten();
    let new_kw = new_string
        .contains('\n')
        .then(|| find_def_keyword(&new_changed, lang))
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
        format!(
            "{hint} — or, to change only a modifier or keyword on the \
             declaration line (e.g. `class X` -> `data class X`), make a \
             single-line edit_file replacement of just that token; \
             single-line literal edits are allowed."
        ),
    ))
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
             On a whitespace-only mismatch, retries a unique re-indented match in \
             brace-style languages (exact-only for Python/YAML); 0 or 2+ matches error. \
             Writing outside the project root returns an @ack_* handle instead of failing; \
             re-invoke with path=\"@ack_...\" to write it (approves the directory for the \
             session) without re-sending content."
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
        let input = super::maybe_replay_ack(ctx, input, "edit_file").await?;
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
            let resolved =
                match super::resolve_write_or_capture(ctx, "edit_file", &input, path).await? {
                    super::WriteOutcome::Write(p) => p,
                    super::WriteOutcome::Pending(env) => return Ok(env),
                };
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
            let resolved =
                match super::resolve_write_or_capture(ctx, "edit_file", &input, path).await? {
                    super::WriteOutcome::Write(p) => p,
                    super::WriteOutcome::Pending(env) => return Ok(env),
                };
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

        perform_edit(path, old_string, new_string, replace_all, &input, ctx).await
    }
}

async fn perform_edit(
    path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    input: &Value,
    ctx: &ToolContext,
) -> Result<Value> {
    let resolved =
        match crate::tools::resolve_write_or_capture(ctx, "edit_file", input, path).await? {
            crate::tools::WriteOutcome::Write(p) => p,
            crate::tools::WriteOutcome::Pending(env) => return Ok(env),
        };

    let content = std::fs::read_to_string(&resolved)?;

    let match_count = content.matches(old_string).count();

    if match_count == 0 {
        // Frictionless recovery: an old_string delivered with literal escape
        // sequences (newline/tab as backslash-n / backslash-t) will not match
        // the file's real control characters. If decoding makes it match
        // uniquely, apply the decoded pair instead of failing.
        if let Some(decoded_old) = decode_literal_escapes(old_string) {
            let dcount = content.matches(decoded_old.as_str()).count();
            if dcount == 1 || (replace_all && dcount >= 1) {
                let decoded_new =
                    decode_literal_escapes(new_string).unwrap_or_else(|| new_string.to_string());
                let candidate = content.replace(decoded_old.as_str(), &decoded_new);
                let new_content = finalize_edit_content(
                    std::path::Path::new(path),
                    &content,
                    candidate,
                    &decoded_new,
                    |d| content.replace(decoded_old.as_str(), d),
                )
                .into_content();
                commit_edit(ctx, &resolved, &new_content).await?;
                return Ok(json!({
                    "status": "ok",
                    "applied_via": "escape-decoded match",
                    "note": "old_string matched after decoding literal newline/tab escapes; verify the result"
                }));
            }
        }
        // Second-tier recovery: over-escaped quotes (backslash-quote). A common MCP-client
        // failure (5/13 edit_file stale-matches, 2026-06-09) where the client over-escapes
        // interior quotes that the file holds plain. Runs only after the conservative decode
        // above produced no unique match. Same unique-match gate keeps it safe; quote decoding
        // is whitespace-neutral, so it is sound to run before the indentation-significant bail.
        // Decodes both old and new (an over-escaping client over-escapes both); the
        // "verify the result" note flags the rare asymmetric case.
        if let Some(decoded_old) = decode_literal_escapes_incl_quotes(old_string) {
            let dcount = content.matches(decoded_old.as_str()).count();
            if dcount == 1 || (replace_all && dcount >= 1) {
                let decoded_new = decode_literal_escapes_incl_quotes(new_string)
                    .unwrap_or_else(|| new_string.to_string());
                let candidate = content.replace(decoded_old.as_str(), &decoded_new);
                let new_content = finalize_edit_content(
                    std::path::Path::new(path),
                    &content,
                    candidate,
                    &decoded_new,
                    |d| content.replace(decoded_old.as_str(), d),
                )
                .into_content();
                commit_edit(ctx, &resolved, &new_content).await?;
                return Ok(json!({
                    "status": "ok",
                    "applied_via": "escape-decoded match (quotes)",
                    "note": "old_string matched after decoding escaped quotes; verify the result"
                }));
            }
        }
        // Indentation-significant languages: a whitespace-normalized match could be
        // re-indented into a different block while still parsing, so the AST gate would
        // wave it through. Disable the fallback here and surface the nearest content so
        // the caller can retry with an exact match.
        if indentation_significant(path) {
            return Err(super::RecoverableError::with_hint(
                not_found_msg(&content, old_string, path),
                "Whitespace-normalized matching is disabled for indentation-significant \
                 languages (indentation is semantic). Copy the exact bytes shown (or from \
                 read_file) and retry.",
            )
            .into());
        }
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
                let msg = not_found_msg(&content, old_string, path);
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

    let candidate = content.replace(old_string, new_string);
    let (new_content, repair_note) = match finalize_edit_content(
        std::path::Path::new(path),
        &content,
        candidate,
        new_string,
        |decoded| content.replace(old_string, decoded),
    ) {
        RepairResult::Repaired(c) => (c, Some(REPAIR_NOTE)),
        RepairResult::Clean(c) | RepairResult::Introduced(c) => (c, None),
    };
    commit_edit(ctx, &resolved, &new_content).await?;
    if let Some(note) = repair_note {
        return Ok(json!({ "status": "ok", "note": note }));
    }

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
