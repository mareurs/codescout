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

/// True when `needle` occurs in `line` starting at a word boundary.
///
/// Every keyword in [`def_keywords_for_lang`] carries a trailing space, which
/// supplies the *right*-hand boundary. Nothing supplied the left one, so a plain
/// `contains` matched a keyword buried inside an identifier: `let via_trait = …`
/// matched `trait `, `let my_fn = …` matched `fn `, `self.inner_impl (x)` matched
/// `impl `. Each was reported as a structural rewrite and refused, and the only
/// way through was to rename the variable.
///
/// The guard's error asymmetry is deliberate: a false positive costs a caller one
/// rejected edit, a false negative risks the LSP range corruption this guard
/// exists to prevent (BUG-027). So this narrows the match only where it is
/// unambiguous — a preceding `[A-Za-z0-9_]` means the keyword is part of a longer
/// identifier and cannot be introducing a definition.
///
/// Two known residuals, both left over-blocking on purpose:
/// - A keyword inside a **string literal** (`assert!(s.contains("fn "))`) still
///   matches: the preceding `"` is not a word character. Narrowing that needs
///   literal-awareness (`crate::util::text::scan_line` has it) and is a wider
///   change than this guard warrants.
/// - A preceding **non-ASCII** character reads as a non-word boundary, so a
///   unicode identifier ending in a keyword still matches. Rust permits those;
///   they are vanishingly rare next to the cost of getting the common case wrong.
fn contains_def_keyword_at_word_start(line: &str, needle: &str) -> bool {
    let bytes = line.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = line[from..].find(needle) {
        let at = from + rel;
        let preceded_by_word_char = at
            .checked_sub(1)
            .map(|i| bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric())
            .unwrap_or(false);
        if !preceded_by_word_char {
            return true;
        }
        // Needles are ASCII, so `at + 1` is a char boundary.
        from = at + 1;
    }
    false
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
        .find_map(|line| {
            keywords
                .iter()
                .find(|kw| contains_def_keyword_at_word_start(line, kw))
                .copied()
        })
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

/// Byte-exact except for a lone trailing `\r` per line — unlike
/// [`find_normalized_windows`], this never `.trim()`s leading whitespace, so it cannot
/// erase or shift indentation. Safe to run even for [`indentation_significant`]
/// languages (Python/YAML/Haskell), where the trim-based fallback is deliberately
/// disabled.
///
/// Exists for the common real-world case: a Windows-checked-out file has `\r\n` line
/// endings (`core.autocrlf=true` materializes CRLF in the working tree while git's
/// index stores LF — `git ls-files --eol` shows `i/lf w/crlf`), but a multi-line
/// `old_string` arrives with bare `\n` newlines (the normal shape for an MCP payload).
/// The initial exact byte match then fails on every line boundary in the file,
/// regardless of language — confirmed 2026-07-08 against Mercury BOM's `.py` files,
/// where `indentation_significant` additionally blocked the trim-based fallback,
/// leaving no recovery path at all.
fn find_crlf_tolerant_windows(content: &str, old_string: &str) -> Vec<NormWindow> {
    let old_lines: Vec<&str> = old_string
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let k = old_lines.len();
    if k == 0 {
        return Vec::new();
    }
    let mut spans: Vec<(&str, usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for raw in content.split_inclusive('\n') {
        let no_lf = raw.strip_suffix('\n').unwrap_or(raw);
        let text = no_lf.strip_suffix('\r').unwrap_or(no_lf);
        spans.push((text, offset, offset + text.len()));
        offset += raw.len();
    }
    let mut out = Vec::new();
    if spans.len() < k {
        return out;
    }
    for i in 0..=(spans.len() - k) {
        if (0..k).all(|j| spans[i + j].0 == old_lines[j]) {
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
///
/// A pure deletion (`new_string == ""`) is deliberately NOT exempt. Deleting a
/// symbol is a structural change like any other, and `edit_code(action='remove')`
/// is the LSP-aware way to do it — `infer_edit_hint` exists partly to route this
/// exact case there. Exempting deletions was tried on 2026-08-15 and reverted:
/// see `docs/issues/archive/2026-08-11-edit-code-cannot-remove-nonempty-module.md`
/// § Gap 3.
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

    /// `destructive` stays `true`: `replace_all` and a section `replace` are lossy, and
    /// re-applying an `old_string` edit is not a no-op.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::writer_closed()
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Edit a file. Text: exact old_string/new_string (whitespace-sensitive; re-indent retry \
             in brace languages), insert prepend/append, replace_all, or edits[] applied \
             atomically. Markdown: heading-addressed section edits, edits[] of heading items, \
             frontmatter {set, delete} — one atomic write."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(crate::tools::markdown::LONG_DOCS)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "anyOf": [
                { "required": ["path"] },
                { "required": ["file_path"] },
                { "required": ["relative_path"] },
                { "required": ["file"] }
            ],
            "properties": {
                "path": { "type": "string", "description": "File path" },
                // FIXTURE NOTE: the literal "Alias for " prefix here is load-bearing —
                // src/server.rs's required_names_no_key_that_has_a_declared_alias
                // (EXPECTED_ALIAS_COUNTS_BY_TOOL["edit_file"] == 3) parses it.
                "file_path": { "type": "string", "description": "Alias for path" },
                "relative_path": { "type": "string", "description": "Alias for path" },
                "file": { "type": "string", "description": "Alias for path" },
                "old_string": { "type": "string", "description": "Exact text to find (whitespace-sensitive). Required unless insert or edits is set." },
                "new_string": { "type": "string", "description": "Replacement text (empty string = delete). Required for single-edit and insert modes." },
                "replace_all": { "type": "boolean", "default": false, "description": "Replace all occurrences." },
                "insert": { "type": "string", "enum": ["prepend", "append"], "description": "Insert at file start/end (old_string not required)." },
                "heading": { "type": "string", "description": "Markdown only: target section heading (fuzzy matched). Required unless using edits[] batch mode." },
                "occurrence": { "type": "integer", "minimum": 1, "description": "Markdown only: 1-indexed selector when `heading` matches several sections." },
                "action": {
                    "type": "string",
                    "enum": ["replace", "insert_before", "insert_after", "remove", "edit"],
                    "description": "Markdown only: operation to perform on the heading-addressed section. 'replace' OVERWRITES the entire body (heading preserved) — choose 'insert_after' to add an adjacent section, or 'edit' with old_string/new_string for in-section surgical replacement. 'insert_before'/'insert_after' add a sibling section (target body preserved). 'remove' deletes the target section. 'edit' performs scoped text replacement within the target section."
                },
                "content": { "type": "string", "description": "Markdown only: new content for replace/insert actions (body only — heading preserved on replace)." },
                "at": {
                    "type": "string",
                    "enum": ["end-of-section", "after-heading-line"],
                    "description": "Markdown only, for action='insert_after': where to insert. 'end-of-section' (default) or 'after-heading-line'."
                },
                "include_subsections": { "type": "boolean", "default": false, "description": "Markdown only, for action='replace': opt in to consuming nested sub-headings." },
                "force": { "type": "boolean", "default": false, "description": "Markdown only: bypass the body-shrink guard. Required when the write would cut the file by >50% in bytes or lines." },
                "frontmatter": {
                    "type": "object",
                    "description": "Markdown only: mutate the YAML frontmatter block at the start of the file. Flat keys only. Combinable atomically with `edits` or `heading`+`action` in the same call. Example: `{set: {status: \"fixed\"}, delete: [\"legacy_field\"]}`.",
                    "properties": {
                        "set": {
                            "type": "object",
                            "additionalProperties": true,
                            "description": "Key → value pairs to set. Existing keys are updated in place; new keys are appended."
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
                        "properties": {
                            "old_string": { "type": "string", "description": "Text grammar: exact text to find." },
                            "new_string": { "type": "string", "description": "Text grammar: replacement text." },
                            "replace_all": { "type": "boolean" },
                            "heading": { "type": "string", "description": "Markdown grammar: target section heading." },
                            "occurrence": { "type": "integer", "minimum": 1 },
                            "action": { "type": "string", "enum": ["replace", "insert_before", "insert_after", "remove", "edit"] },
                            "content": { "type": "string" },
                            "at": { "type": "string", "enum": ["end-of-section", "after-heading-line"] },
                            "include_subsections": { "type": "boolean" }
                        },
                        "description": "Either the text grammar (old_string+new_string) or the markdown grammar (heading+action) — not mixed within one edits[] call."
                    },
                    "description": "Batch mode: array of edit operations applied atomically. Top-level new_string not used. Every item must use the same grammar (text or markdown)."
                }

            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let input = super::maybe_replay_ack(ctx, input, "edit_file").await?;
        let path = super::require_str_param_or_hint(
            &input,
            "path",
            crate::fs::PATH_PARAM_ALIASES,
            "edit_file(path=\"src/x.rs\", old_string=\"...\", new_string=\"...\"). path is required on every call — there is no implicit current file.",
        )?;
        let new_string = input["new_string"].as_str().unwrap_or("");

        // Gate: route heading-addressed / frontmatter markdown grammar to
        // markdown::edit. Plain old_string/new_string, insert, and replace_all
        // all stay on this path regardless of extension — only `heading`,
        // `action`, `frontmatter`, or a heading-addressed `edits[]` item
        // trips the redirect, and only on a .md/.markdown path.
        let is_md = path.ends_with(".md") || path.ends_with(".markdown");
        let edits_arr = input["edits"].as_array();
        let heading_items = edits_arr
            .map(|e| e.iter().filter(|x| x.get("heading").is_some()).count())
            .unwrap_or(0);
        let plain_items = edits_arr
            .map(|e| e.len().saturating_sub(heading_items))
            .unwrap_or(0);
        let heading_grammar = input["heading"].is_string()
            || input["action"].is_string()
            || input["frontmatter"].is_object()
            || heading_items > 0;
        if heading_grammar && !is_md {
            return Err(super::RecoverableError::with_hint(
                "heading, action, frontmatter and heading-addressed edits[] apply to markdown files only",
                "For non-markdown files use old_string/new_string, insert, or replace_all.",
            )
            .into());
        }
        if heading_items > 0 && plain_items > 0 {
            return Err(super::RecoverableError::with_hint(
                "edits[] mixes heading-addressed items with old_string/new_string items",
                "Send one grammar per call: every item with `heading`+`action`, or every item with `old_string`+`new_string`.",
            )
            .into());
        }
        if heading_grammar {
            return crate::tools::markdown::edit(input, ctx).await;
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
            let mut content = read_edit_target(&resolved, path)?;

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
            // The librarian guard now lives inside read_edit_target, so every write
            // path gets it — including this one.
            let content = read_edit_target(&resolved, path)?;
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

/// Read the file an edit targets, naming it in any failure, and refuse it if it
/// is a librarian-managed artifact.
///
/// **The guard lives here because this is the one thing all three write paths do.**
/// `edit_file` can reach a file three ways — batch `edits[]`, `insert`
/// prepend/append, and a single `old_string`/`new_string` — and
/// `guard_not_librarian_managed` used to be called from exactly one of them, the
/// `insert` branch. The markdown gate meanwhile admits all three and its refusal
/// hint names all three, so the sequence `edit_markdown` refuses (managed) ->
/// `edit_file` refuses (markdown) -> hint says `replace_all=true` -> unguarded
/// write was the path the error messages composed into. Guarding at the shared
/// read makes it structurally unbypassable, including by a fourth write path
/// somebody adds later.
/// docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md
///
/// A bare `std::fs::read_to_string(&resolved)?` propagates
/// `No such file or directory (os error 2)` with no path and no indication of which
/// stage failed. That is fine in isolation and actively misleading after an
/// out-of-scope ack replay, where the obvious reading is "the `@ack_*` handle did not
/// resolve" — the handle is the thing you just passed, and the error names nothing
/// else.
///
/// That misreading is on record: `docs/issues/archive/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md`
/// concluded the ack mechanism was broken and filed it as a bug. Measured 2026-08-14,
/// the mechanism works in both call shapes; the ENOENT was the *target file*. The
/// report cost two extra round-trips and a bug file, and the only thing that made the
/// wrong conclusion attractive was an error message that mentioned no path.
///
/// `display_path` is the caller's own path string, used for both messages so they
/// name what was passed rather than the resolved absolute form.
fn read_edit_target(resolved: &std::path::Path, display_path: &str) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(resolved).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            crate::tools::RecoverableError::with_hint(
                format!("no file to edit at {}", resolved.display()),
                "Check the path, or use create_file if it should be created. If you got \
                 here by passing an `@ack_*` handle: the handle resolved correctly and \
                 this is the target file, not the handle.",
            )
            .into()
        } else {
            anyhow::Error::new(e).context(format!("reading {} to edit it", resolved.display()))
        }
    })?;
    // `FrontmatterWrite` is the conservative value, not a claim. `edit_file` replaces
    // raw text anywhere in the file, so it cannot bound its own extent — an
    // `old_string` may well sit inside the frontmatter block. Passing `BodyWrite` here
    // would be asserting a negative the caller has not established.
    // docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md
    crate::util::librarian_guard::guard_not_librarian_managed(
        display_path,
        &content,
        Some(resolved),
        crate::util::librarian_guard::Access::FrontmatterWrite,
    )?;
    Ok(content)
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

    let content = read_edit_target(&resolved, path)?;

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
        // CRLF-tolerant match: exact except for a lone trailing `\r` per line. Runs for
        // every language (unlike the trim-based fallback below, it never touches
        // indentation, so it's safe even where that one is disabled) — see
        // `find_crlf_tolerant_windows` for why this exists.
        let crlf_windows = find_crlf_tolerant_windows(&content, old_string);
        if crlf_windows.len() == 1 {
            let w = &crlf_windows[0];
            let matched = &content[w.start_byte..w.end_byte];
            let replacement_src = new_string.strip_suffix('\n').unwrap_or(new_string);
            // Adapt the replacement's line endings to match this region's convention so
            // the edit doesn't leave a mixed CRLF/LF block behind.
            let adapted = if matched.contains("\r\n") {
                replacement_src.replace("\r\n", "\n").replace('\n', "\r\n")
            } else {
                replacement_src.replace("\r\n", "\n")
            };
            let mut new_content = String::with_capacity(content.len());
            new_content.push_str(&content[..w.start_byte]);
            new_content.push_str(&adapted);
            new_content.push_str(&content[w.end_byte..]);

            if let Some(lang) = crate::ast::detect_language(std::path::Path::new(path)) {
                let before = crate::ast::has_syntax_errors(&content, lang);
                let after = crate::ast::has_syntax_errors(&new_content, lang);
                if after && !before {
                    return Err(super::RecoverableError::with_hint(
                        format!(
                            "CRLF-tolerant match at lines {}-{} would introduce syntax errors — not written",
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
                "applied_via": "crlf-tolerant match",
                "lines": format!("{}-{}", w.start_line, w.end_line),
                "note": "old_string matched after tolerating \\r\\n vs \\n line-ending differences; verify the result"
            }));
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
