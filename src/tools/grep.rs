//! `grep` tool and related format helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::format_overflow;
use super::{optional_u64_param, OutputForm, RecoverableError, Tool, ToolContext};

// ── grep ───────────────────────────────────────────────────────

pub struct Grep;

#[async_trait::async_trait]
impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Regex search across files. Returns matching lines with location. Pass context_lines for surrounding code."
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern" },
                "path": { "type": "string", "description": "File or directory (default: project root)" },
                "limit": { "type": "integer", "default": 50, "description": "Max matching lines" },
                "context_lines": { "type": "integer", "default": 0, "description": "Context lines before/after each match (max 20). Adjacent matches merge." },
                "ignore_case": { "type": "boolean", "default": false, "description": "Case-insensitive match" },
                "whole_word": { "type": "boolean", "default": false, "description": "Match whole words only (\\b boundaries)" },
                "glob": { "type": ["string", "array"], "description": "Restrict to files matching glob(s), e.g. \"*.rs\" or [\"src/**\", \"*.md\"]" },
                "include_hidden": { "type": "boolean", "default": false, "description": "Also search hidden files/dirs (dotfiles, .github/)" },
                "mode": { "type": "string", "enum": ["lines", "files"], "default": "lines", "description": "\"files\": ranked files + per-file counts, no line content (tames broad searches)" }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let pattern = super::require_str_param_or(&input, "pattern", &["query", "regex"])?;
        let raw_path = strip_buffer_ref_quotes(input["path"].as_str().unwrap_or("."));

        // Buffer ref (@tool_*, @cmd_*, @file_*): search the cached content
        // instead of treating the ref as a filesystem path.
        if raw_path.starts_with('@') {
            let mut input = input.clone();
            input["path"] = serde_json::json!(raw_path);
            return grep_in_buffer(&input, ctx).await;
        }

        let project_root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        let security = ctx
            .agent
            .security_config_for(ctx.workspace_override.as_deref())
            .await;
        let search_path = crate::util::path_security::validate_read_path(
            raw_path,
            project_root.as_deref(),
            &security,
        )?;
        let max = optional_u64_param(&input, "limit").unwrap_or(50) as usize;
        let context_lines = optional_u64_param(&input, "context_lines")
            .unwrap_or(0)
            .min(20) as usize;
        let ignore_case = input
            .get("ignore_case")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let whole_word = input
            .get("whole_word")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let include_hidden = input
            .get("include_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let files_mode = input.get("mode").and_then(|v| v.as_str()) == Some("files");
        let globs = parse_globs(&input);
        let (re, is_literal_fallback) = build_grep_regex(pattern, ignore_case, whole_word)?;
        let mut matches: Vec<Value> = vec![];
        let mut total_match_count = 0usize;
        let mut hit_cap = false;
        let mut skipped_binary = 0usize;

        let mut wb = ignore::WalkBuilder::new(&search_path);
        wb.hidden(!include_hidden).git_ignore(true);
        if !globs.is_empty() {
            let mut ob = ignore::overrides::OverrideBuilder::new(&search_path);
            for g in &globs {
                ob.add(g).map_err(|e| {
                    RecoverableError::with_hint(
                        format!("invalid glob '{g}': {e}"),
                        "globs use gitignore syntax, e.g. \"*.rs\" or \"**/*.md\"",
                    )
                })?;
            }
            wb.overrides(ob.build().map_err(|e| {
                RecoverableError::with_hint(
                    format!("invalid glob set: {e}"),
                    "check the glob patterns",
                )
            })?);
        }
        let walker = wb.build();
        if files_mode {
            use std::collections::BTreeMap;
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut total = 0usize;
            let mut skipped_binary = 0usize;
            for entry in walker.flatten() {
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(entry.path()) else {
                    continue;
                };
                if bytes.iter().take(8192).any(|&b| b == 0) {
                    skipped_binary += 1;
                    continue;
                }
                let text = String::from_utf8_lossy(&bytes);
                let n = text.lines().filter(|l| re.is_match(l)).count();
                if n > 0 {
                    total += n;
                    *counts
                        .entry(entry.path().display().to_string())
                        .or_default() += n;
                }
            }
            let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
            ranked.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            let files: Vec<Value> = ranked
                .iter()
                .map(|(f, c)| json!({ "file": f, "count": c }))
                .collect();
            let mut r = json!({ "files": files, "total": total, "files_count": ranked.len() });
            if skipped_binary > 0 {
                r["skipped_binary"] = json!(skipped_binary);
            }
            return Ok(r);
        }
        'outer: for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let Ok(bytes) = std::fs::read(entry.path()) else {
                continue;
            };
            if bytes.iter().take(8192).any(|&b| b == 0) {
                skipped_binary += 1; // looks binary (NUL byte) — skip
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);

            if context_lines == 0 {
                // Original behaviour: one entry per matching line
                for (i, line) in text.lines().enumerate() {
                    if re.is_match(line) {
                        total_match_count += 1;
                        matches.push(json!({
                            "file": entry.path().display().to_string(),
                            "line": i + 1,
                            "content": line
                        }));
                        if matches.len() >= max {
                            hit_cap = true;
                            break 'outer;
                        }
                    }
                }
            } else {
                // Context mode: merge overlapping windows into blocks
                let file_lines: Vec<&str> = text.lines().collect();
                let n = file_lines.len();
                // (block_start_idx, match_indices, block_end_idx) — all 0-indexed
                let mut current: Option<(usize, Vec<usize>, usize)> = None;

                for (i, line) in file_lines.iter().enumerate() {
                    if !re.is_match(line) {
                        continue;
                    }
                    total_match_count += 1;
                    let ctx_start = i.saturating_sub(context_lines);
                    let ctx_end = (i + context_lines).min(n.saturating_sub(1));

                    match current.take() {
                        None => {
                            current = Some((ctx_start, vec![i], ctx_end));
                        }
                        Some((blk_start, mut blk_matches, blk_end)) => {
                            if ctx_start <= blk_end + 1 {
                                // Overlapping or adjacent: extend block, append match
                                blk_matches.push(i);
                                current = Some((blk_start, blk_matches, ctx_end.max(blk_end)));
                            } else {
                                // Non-overlapping: emit finished block, start new one
                                let content = file_lines[blk_start..=blk_end].join("\n");
                                let match_lines: Vec<u64> =
                                    blk_matches.iter().map(|&m| (m + 1) as u64).collect();
                                matches.push(json!({
                                    "file": entry.path().display().to_string(),
                                    "match_lines": match_lines,
                                    "start_line": blk_start + 1,
                                    "content": content,
                                }));
                                current = Some((ctx_start, vec![i], ctx_end));
                            }
                        }
                    }

                    if total_match_count >= max {
                        hit_cap = true;
                        break;
                    }
                }

                // Emit the last in-flight block
                if let Some((blk_start, blk_matches, blk_end)) = current {
                    let content = file_lines[blk_start..=blk_end].join("\n");
                    let match_lines: Vec<u64> =
                        blk_matches.iter().map(|&m| (m + 1) as u64).collect();
                    matches.push(json!({
                        "file": entry.path().display().to_string(),
                        "match_lines": match_lines,
                        "start_line": blk_start + 1,
                        "content": content,
                    }));
                }

                if total_match_count >= max {
                    hit_cap = true;
                    break 'outer;
                }
            }
        }

        // In context mode, matches contains merged blocks — fewer than total_match_count
        // (which counts individual matching lines). Report blocks so `total` == `matches.len()`.
        let shown_count = if context_lines > 0 {
            matches.len()
        } else {
            total_match_count
        };

        // Index-aware hits: attach enclosing symbol when the result set is
        // small (no overflow) and the file is a known source language.
        if context_lines == 0 && !hit_cap && total_match_count <= max {
            use std::collections::HashMap;
            use std::path::PathBuf;
            let mut cache: HashMap<PathBuf, Vec<crate::lsp::symbols::SymbolInfo>> = HashMap::new();
            for m in matches.iter_mut() {
                let (Some(file), Some(line)) = (
                    m.get("file").and_then(|v| v.as_str()).map(PathBuf::from),
                    m.get("line").and_then(|v| v.as_u64()),
                ) else {
                    continue;
                };
                let Some(lang) = crate::ast::detect_language(&file) else {
                    continue;
                };
                let syms = cache.entry(file.clone()).or_insert_with(|| {
                    std::fs::read_to_string(&file)
                        .ok()
                        .and_then(|src| {
                            crate::ast::parser::extract_symbols_from_source(&src, Some(lang), &file)
                                .ok()
                        })
                        .unwrap_or_default()
                });
                // grep lines are 1-indexed; SymbolInfo lines are 0-indexed.
                if let Some(sym) = enclosing_symbol(syms, (line as u32).saturating_sub(1)) {
                    m["symbol"] = json!(sym);
                }
            }
        }

        // Build grouped output (simple mode) or keep flat (context mode).
        let mut result = if context_lines == 0 {
            use crate::tools::file_group::{cap_grouped, group_by_file, groups_to_json};
            let budget = max;
            let (visible, total, files) = cap_grouped(matches, budget);
            let truncated = hit_cap || total > visible.len();
            let groups = group_by_file(&visible);
            let file_groups = groups_to_json(&groups);
            let mut r = json!({
                "file_groups": file_groups,
                "total": total,
                "files": files,
            });
            if truncated {
                // Pattern 1 (PROGRESSIVE_DISCOVERABILITY.md): concrete + copy-paste-ready.
                // `groups` is already sorted by count desc by group_by_file.
                let top: Vec<String> = groups
                    .iter()
                    .take(3)
                    .map(|g| format!("path=\"{}\" ({} matches)", g.file, g.items.len()))
                    .collect();
                let hint = if top.is_empty() {
                    format!(
                        "Showing {} of {} matches across {} files. \
                         Narrow with a more specific pattern or add path=<file>. \
                         Or mode=\"files\" for a per-file count summary.",
                        visible.len(),
                        total,
                        files
                    )
                } else {
                    format!(
                        "Showing {} of {} matches across {} files. \
                         Narrow with one of: {} — or use a more specific pattern. \
                         Or mode=\"files\" for a per-file count summary.",
                        visible.len(),
                        total,
                        files,
                        top.join(", ")
                    )
                };
                r["overflow"] = json!({
                    "shown": visible.len(),
                    "total": total,
                    "hint": hint,
                });
            }
            r
        } else {
            // Context mode: keep flat matches[], preserve legacy shape for format_grep
            let mut r =
                json!({ "matches": matches, "total": shown_count, "context_lines": context_lines });
            if hit_cap {
                // Derive top files from the flat matches we collected.
                use std::collections::BTreeMap;
                let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                for m in &matches {
                    if let Some(f) = m.get("file").and_then(|v| v.as_str()) {
                        *counts.entry(f.to_string()).or_default() += 1;
                    }
                }
                let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
                ranked.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                let top: Vec<String> = ranked
                    .iter()
                    .take(3)
                    .map(|(f, n)| format!("path=\"{f}\" ({n} matches)"))
                    .collect();
                let hint = if top.is_empty() {
                    format!(
                        "Showing first {shown_count} matches (cap hit). \
                         Narrow with a more specific pattern or path=<file>."
                    )
                } else {
                    format!(
                        "Showing first {shown_count} matches (cap hit). \
                         Narrow with one of: {} — or use a more specific pattern.",
                        top.join(", ")
                    )
                };
                r["overflow"] = json!({
                    "shown": shown_count,
                    "hint": hint,
                });
            }
            r
        };

        if is_literal_fallback {
            result["mode"] = json!("literal_fallback");
            result["reason"] = json!("pattern was not valid regex — searched as literal text");
        }
        if total_match_count == 0 && crate::util::path_security::is_identifier_pattern(pattern) {
            let name = pattern.split('|').next().unwrap_or(pattern);
            result["suggestion"] = json!(format!(
                "Pattern looks like a symbol name. Consider: \
                 symbols(name='{name}') for declarations, \
                 references(symbol='{name}') for direct callers, \
                 call_graph(symbol='{name}', direction='callers') for transitive blast radius."
            ));
        }
        if skipped_binary > 0 {
            result["skipped_binary"] = json!(skipped_binary);
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_grep(result))
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
    }
}

// ── format helpers ──────────────────────────────────────────────────────

pub(super) fn format_grep(val: &Value) -> String {
    let total = val["total"].as_u64().unwrap_or(0) as usize;

    if total == 0 {
        return "0 matches".to_string();
    }

    let mut out = String::new();

    if val.get("mode").and_then(|m| m.as_str()) == Some("literal_fallback") {
        out.push_str("[literal fallback] ");
    }

    // Dispatch: file_groups[] → simple mode (new shape).
    //           matches[]    → context mode (legacy shape with start_line items).
    if let Some(groups) = val["file_groups"].as_array() {
        let files = val["files"].as_u64().unwrap_or(0) as usize;
        format_search_simple_mode(&mut out, groups, total, files);
    } else if let Some(flat) = val["matches"].as_array() {
        let match_word = if total == 1 { "match" } else { "matches" };
        out.push_str(&format!("{total} {match_word}\n"));
        format_search_context_mode(&mut out, flat);
    }

    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }
    out
}

fn format_search_simple_mode(out: &mut String, file_groups: &[Value], total: usize, files: usize) {
    use crate::tools::file_group::{groups_from_json, render_grouped};

    let groups = groups_from_json(file_groups);
    let noun = if total == 1 { "match" } else { "matches" };

    let render_item = |item: &Value| -> String {
        let line = item["line"].as_u64().unwrap_or(0);
        let content = item["content"].as_str().unwrap_or("").trim();
        format!("  {line:>5}: {content}")
    };

    out.push_str(&render_grouped(&groups, total, files, noun, render_item));
}

fn format_search_context_mode(out: &mut String, matches: &[Value]) {
    use std::collections::HashMap;

    // Precompute per-file match totals so the header can show `file (N)`,
    // matching the simple-mode format for at-a-glance density.
    let mut per_file_total: HashMap<&str, u64> = HashMap::new();
    for m in matches {
        let file = m["file"].as_str().unwrap_or("?");
        let n = m["match_lines"]
            .as_array()
            .map(|a| a.len() as u64)
            .unwrap_or(0);
        *per_file_total.entry(file).or_insert(0) += n;
    }

    let mut current_file: Option<&str> = None;

    for m in matches {
        let file = m["file"].as_str().unwrap_or("?");
        let start_line = m["start_line"].as_u64().unwrap_or(1);
        let content = m["content"].as_str().unwrap_or("");
        let match_lines: std::collections::HashSet<u64> = m["match_lines"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
            .unwrap_or_default();

        let same_file = current_file == Some(file);
        if !same_file {
            let count = per_file_total.get(file).copied().unwrap_or(0);
            out.push_str("\n  ");
            out.push_str(file);
            out.push_str(&format!(" ({count})\n"));
            current_file = Some(file);
        } else {
            // Separator between non-overlapping blocks in the same file —
            // ripgrep uses `--` for the same purpose.
            out.push_str("  --\n");
        }

        for (i, line) in content.lines().enumerate() {
            let line_num = start_line + i as u64;
            // Ripgrep convention: `N:` for match line, `N-` for context.
            let sep = if match_lines.contains(&line_num) {
                ':'
            } else {
                '-'
            };
            out.push_str(&format!("  {line_num:>5}{sep} {line}\n"));
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }
}

/// Build a search regex. Resolves the body (raw regex, or escaped literal when
/// the pattern isn't valid regex and didn't intend to be), then applies
/// whole-word wrapping and case-insensitivity. Returns (regex, is_literal_fallback).
fn build_grep_regex(
    pattern: &str,
    ignore_case: bool,
    whole_word: bool,
) -> Result<(regex::Regex, bool)> {
    let compile = |p: &str| {
        regex::RegexBuilder::new(p)
            .case_insensitive(ignore_case)
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 20)
            .build()
    };
    let (body, is_literal) = match compile(pattern) {
        Ok(_) => (pattern.to_string(), false),
        Err(e) => {
            if super::is_regex_like(pattern) {
                return Err(RecoverableError::with_hint(
                    format!("invalid regex: {e}"),
                    "patterns are full regex syntax — escape metacharacters like \\( \\. \\[ for literals",
                )
                .into());
            }
            (regex::escape(pattern), true)
        }
    };
    let effective = if whole_word {
        format!(r"\b(?:{body})\b")
    } else {
        body
    };
    let re = compile(&effective).map_err(|e| {
        RecoverableError::with_hint(
            format!("invalid pattern after processing: {e}"),
            "with whole_word=true the term is wrapped in \\b(?:…)\\b word boundaries",
        )
    })?;
    Ok((re, is_literal))
}

/// Innermost symbol whose (full) line range contains `line0` (0-indexed).
/// Recurses into children; returns the fully-qualified `name_path`.
fn enclosing_symbol(symbols: &[crate::lsp::symbols::SymbolInfo], line0: u32) -> Option<String> {
    for s in symbols {
        let start = s.range_start_line.unwrap_or(s.start_line);
        if line0 >= start && line0 <= s.end_line {
            return enclosing_symbol(&s.children, line0).or_else(|| Some(s.name_path.clone()));
        }
    }
    None
}

/// Collect `glob` param values (single string or array of strings).
fn parse_globs(input: &Value) -> Vec<String> {
    match input.get("glob") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Grep against a buffer ref (`@tool_*`, `@cmd_*`, `@file_*`).
///
/// `@tool_*` content is JSON; it is pretty-printed before search so
/// identifier-shaped strings sit on dedicated lines and become matchable.
async fn grep_in_buffer(input: &Value, ctx: &ToolContext) -> Result<Value> {
    let pattern = super::require_str_param_or(input, "pattern", &["query", "regex"])?;
    let raw_path = input["path"].as_str().unwrap_or_default();
    let max = optional_u64_param(input, "limit").unwrap_or(50) as usize;
    let context_lines = optional_u64_param(input, "context_lines")
        .unwrap_or(0)
        .min(20) as usize;
    let ignore_case = input
        .get("ignore_case")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let whole_word = input
        .get("whole_word")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let raw = ctx
        .output_buffer
        .get(raw_path)
        .ok_or_else(|| {
            RecoverableError::with_hint(
                format!("buffer reference not found: '{raw_path}'"),
                "Buffer refs expire when the session resets. Re-run the command to get a fresh ref.",
            )
        })?
        .stdout;

    let text = if raw_path.starts_with("@tool_") {
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            // Materialize escaped newlines inside string values (e.g. an
            // artifact `body`) so multi-line fields become grep-able lines
            // rather than one collapsed line. to_string_pretty splits JSON
            // *structure* but leaves `\n` escaped inside string values.
            // Search-only text, so the rare literal `\n`-in-data (serialized
            // `\\n` → backslash+newline) is a cosmetically acceptable trade.
            // Bug 2026-07-01-grep-buffer-multiline-string-value-collapses.
            .map(|pretty| pretty.replace("\\n", "\n"))
            .unwrap_or(raw)
    } else {
        raw
    };

    let (re, is_literal_fallback) = build_grep_regex(pattern, ignore_case, whole_word)?;

    let mut matches: Vec<Value> = vec![];
    let mut total_match_count = 0usize;
    let mut hit_cap = false;

    if context_lines == 0 {
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                total_match_count += 1;
                matches.push(json!({
                    "file": raw_path,
                    "line": i + 1,
                    "content": line,
                }));
                if matches.len() >= max {
                    hit_cap = true;
                    break;
                }
            }
        }
    } else {
        let file_lines: Vec<&str> = text.lines().collect();
        let n = file_lines.len();
        let mut current: Option<(usize, Vec<usize>, usize)> = None;

        for (i, line) in file_lines.iter().enumerate() {
            if !re.is_match(line) {
                continue;
            }
            total_match_count += 1;
            let ctx_start = i.saturating_sub(context_lines);
            let ctx_end = (i + context_lines).min(n.saturating_sub(1));

            match current.take() {
                None => current = Some((ctx_start, vec![i], ctx_end)),
                Some((blk_start, mut blk_matches, blk_end)) => {
                    if ctx_start <= blk_end + 1 {
                        blk_matches.push(i);
                        current = Some((blk_start, blk_matches, ctx_end.max(blk_end)));
                    } else {
                        let content = file_lines[blk_start..=blk_end].join("\n");
                        let match_lines: Vec<u64> =
                            blk_matches.iter().map(|&m| (m + 1) as u64).collect();
                        matches.push(json!({
                            "file": raw_path,
                            "match_lines": match_lines,
                            "start_line": blk_start + 1,
                            "content": content,
                        }));
                        current = Some((ctx_start, vec![i], ctx_end));
                    }
                }
            }

            if total_match_count >= max {
                hit_cap = true;
                break;
            }
        }

        if let Some((blk_start, blk_matches, blk_end)) = current {
            let content = file_lines[blk_start..=blk_end].join("\n");
            let match_lines: Vec<u64> = blk_matches.iter().map(|&m| (m + 1) as u64).collect();
            matches.push(json!({
                "file": raw_path,
                "match_lines": match_lines,
                "start_line": blk_start + 1,
                "content": content,
            }));
        }
    }

    let shown_count = if context_lines > 0 {
        matches.len()
    } else {
        total_match_count
    };

    let mut result = if context_lines == 0 {
        use crate::tools::file_group::{cap_grouped, group_by_file, groups_to_json};
        let (visible, total, files) = cap_grouped(matches, max);
        let truncated = hit_cap || total > visible.len();
        let groups = group_by_file(&visible);
        let file_groups = groups_to_json(&groups);
        let mut r = json!({
            "file_groups": file_groups,
            "total": total,
            "files": files,
        });
        if truncated {
            r["overflow"] = json!({
                "shown": visible.len(),
                "total": total,
                "hint": "Many matches. Narrow the pattern.",
            });
        }
        r
    } else {
        let mut r = json!({
            "matches": matches,
            "total": shown_count,
            "context_lines": context_lines,
        });
        if hit_cap {
            r["overflow"] = json!({
                "shown": shown_count,
                "hint": format!(
                    "Showing first {shown_count} matches (cap hit). Narrow the pattern."
                ),
            });
        }
        r
    };

    if is_literal_fallback {
        result["mode"] = json!("literal_fallback");
        result["reason"] = json!("pattern was not valid regex — searched as literal text");
    }
    if crate::util::path_security::is_identifier_pattern(pattern) {
        let name = pattern.split('|').next().unwrap_or(pattern);
        result["suggestion"] = json!(format!(
            "Pattern looks like a symbol name. Consider: \
             symbols(name='{name}') for declarations, \
             references(symbol='{name}') for direct callers."
        ));
    }
    Ok(result)
}

/// Strip surrounding quotes/backticks from @ref paths the same way read_file
/// does. Lets buffer-ref greps survive LLM quoting habits.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use crate::tools::ToolContext;
    use tempfile::tempdir;

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
            workspace_override: None,
        }
    }
    async fn rooted_ctx(root: &std::path::Path) -> ToolContext {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        ToolContext {
            agent: Agent::new(Some(root.to_path_buf())).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        }
    }

    #[tokio::test]
    async fn suggestion_only_when_zero_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn my_symbol() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let tool = Grep;

        let hit = tool
            .call(
                json!({ "pattern": "my_symbol", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            hit.get("suggestion").is_none(),
            "no suggestion when there are matches"
        );

        let miss = tool
            .call(
                json!({ "pattern": "no_such_symbol_xyz", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            miss.get("suggestion").is_some(),
            "suggestion expected on zero matches for an identifier"
        );
    }

    #[tokio::test]
    async fn searches_non_utf8_and_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        // latin-1 é (0xE9) around an ASCII target
        std::fs::write(
            dir.path().join("latin.txt"),
            [
                b'c', b'a', b'f', 0xE9, b' ', b'T', b'A', b'R', b'G', b'E', b'T', b'\n',
            ],
        )
        .unwrap();
        // binary file with a NUL byte
        std::fs::write(
            dir.path().join("blob.bin"),
            [b'T', b'A', b'R', b'G', b'E', b'T', 0x00, 0x01],
        )
        .unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let tool = Grep;

        let r = tool
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            r["total"].as_u64().unwrap(),
            1,
            "latin-1 file matched, binary file skipped"
        );
        assert_eq!(r["skipped_binary"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn grep_returns_grouped_shape_simple_mode() {
        use serde_json::json;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn foo() {}\nfn foo_bar() {}\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn foo_baz() {}\n").unwrap();

        let ctx = test_ctx().await;
        let tool = Grep;
        let result = tool
            .call(
                json!({ "pattern": "foo", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let groups = result["file_groups"].as_array().unwrap();
        assert!(!groups.is_empty(), "file_groups must be non-empty");
        for group in groups {
            assert!(group.get("file").is_some(), "group must have file");
            let items = group["items"].as_array().unwrap();
            for item in items {
                assert!(
                    item.get("file").is_none(),
                    "per-item file should be stripped, got: {item}"
                );
                assert!(item.get("line").is_some(), "item must have line");
                assert!(item.get("content").is_some(), "item must have content");
            }
        }
        assert!(
            result["total"].as_u64().unwrap() >= 3,
            "total must be >= 3, got {}",
            result["total"]
        );
        assert!(
            result["files"].as_u64().unwrap() >= 2,
            "files must be >= 2, got {}",
            result["files"]
        );
    }

    #[tokio::test]
    async fn grep_call_content_returns_ripgrep_style_text_not_json() {
        // Regression: small grep results used to serialize as pretty JSON via the
        // default Tool::call_content path. Now Grep declares OutputForm::Text, so
        // even sub-threshold results come through as the compact ripgrep-style
        // form ("file\n  N: content"), saving ~60% tokens on bulk locator output.
        use serde_json::json;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn foo() {}\nfn foo_bar() {}\n").unwrap();

        let ctx = test_ctx().await;
        let tool = Grep;
        let content = tool
            .call_content(
                json!({ "pattern": "foo", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(content.len(), 1, "expected exactly 1 content block");
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            !text.trim_start().starts_with('{'),
            "small grep output must NOT be JSON, got: {text}"
        );
        assert!(
            text.contains("a.rs"),
            "text must reference matched file, got: {text}"
        );
        assert!(
            text.contains(": fn foo"),
            "text must use ripgrep-style `N: content` lines, got: {text}"
        );
    }

    #[tokio::test]
    async fn grep_buffer_ref_matches_content_in_tool_buffer() {
        // Probe bug 2026-05-09-grep-buffer-false-negatives.
        // Seed an @tool_* buffer with a known identifier, then assert grep finds it.
        use serde_json::json;
        let ctx = test_ctx().await;
        let raw = r#"{"symbols":[{"name":"foo_bar_baz","kind":"fn"}]}"#;
        let buf_id = ctx.output_buffer.store_tool("symbols", raw.to_string());

        let tool = Grep;
        let result = tool
            .call(json!({ "pattern": "foo_bar_baz", "path": buf_id }), &ctx)
            .await
            .unwrap();

        let total = result.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            total > 0,
            "grep should find 'foo_bar_baz' in @tool_* buffer content, got total={total}: {result}"
        );
    }

    #[tokio::test]
    async fn grep_buffer_ref_matches_multiline_string_value() {
        // Bug 2026-07-01-grep-buffer-multiline-string-value-collapses.
        // A multi-line JSON string value (e.g. an artifact `body`) must be
        // grep-able line-by-line, not collapsed into one physical line by
        // to_string_pretty (which leaves embedded `\n` escaped).
        use serde_json::json;
        let ctx = test_ctx().await;
        let body: String = (1..=10)
            .map(|n| format!("## F-{n} — entry {n}\n"))
            .collect();
        let raw = json!({ "id": "abc", "title": "session-log", "body": body }).to_string();
        let buf_id = ctx.output_buffer.store_tool("artifact", raw);

        let tool = Grep;
        let result = tool
            .call(json!({ "pattern": "## F-", "path": buf_id }), &ctx)
            .await
            .unwrap();

        let total = result.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            total >= 10,
            "grep should match each heading line in a multi-line string value, got total={total}: {result}"
        );
    }

    #[tokio::test]
    async fn grep_overflow_hint_names_top_files() {
        // I-5: when grep overflows, the hint must be concrete and copy-paste-ready —
        // it should cite the top file paths by match count so the LLM can narrow.
        use serde_json::json;
        let dir = tempdir().unwrap();
        // Create three files; one dominates by match count.
        let many: String = (0..40).map(|i| format!("fn target_{i}() {{}}\n")).collect();
        std::fs::write(dir.path().join("hot.rs"), many).unwrap();
        std::fs::write(
            dir.path().join("warm.rs"),
            "fn target_a() {}\nfn target_b() {}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("cold.rs"), "fn target_c() {}\n").unwrap();

        let ctx = test_ctx().await;
        let tool = Grep;
        // limit=5 forces overflow against the 43 total matches.
        let result = tool
            .call(
                json!({ "pattern": "target", "path": dir.path().to_str().unwrap(), "limit": 5 }),
                &ctx,
            )
            .await
            .unwrap();

        let overflow = result
            .get("overflow")
            .expect("limit=5 against 43 matches must overflow");
        let hint = overflow["hint"]
            .as_str()
            .expect("overflow.hint must be a string");
        assert!(
            hint.contains("path="),
            "hint must include a concrete `path=\"...\"` suggestion, got: {hint}"
        );
        assert!(
            hint.contains("hot.rs"),
            "hint must cite the highest-match file, got: {hint}"
        );
        assert!(
            hint.contains("matches"),
            "hint must include match counts so the model can pick, got: {hint}"
        );
    }

    #[test]
    fn build_grep_regex_ignore_case_matches_mixed_case() {
        let (re, _) = build_grep_regex("foo", true, false).unwrap();
        assert!(re.is_match("FOO"));
        assert!(re.is_match("foo"));
        let (cs, _) = build_grep_regex("foo", false, false).unwrap();
        assert!(!cs.is_match("FOO"), "default must stay case-sensitive");
    }

    #[test]
    fn build_grep_regex_whole_word_excludes_substring() {
        let (re, _) = build_grep_regex("cat", false, true).unwrap();
        assert!(re.is_match("a cat sat"));
        assert!(
            !re.is_match("category"),
            "whole_word must not match substrings"
        );
    }

    #[tokio::test]
    async fn ignore_case_flag_from_input() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "Hello WORLD\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "world", "path": dir.path().to_str().unwrap(), "ignore_case": true }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn glob_filters_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.rs"), "TARGET\n").unwrap();
        std::fs::write(dir.path().join("skip.txt"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap(), "glob": "*.rs" }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 1, "only the .rs file matches");
    }

    #[tokio::test]
    async fn include_hidden_searches_dotfiles() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let off = Grep
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            off["total"].as_u64().unwrap(),
            0,
            "hidden skipped by default"
        );
        let on = Grep
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap(), "include_hidden": true }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(on["total"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn mode_files_returns_ranked_counts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("many.rs"), "X\nX\nX\n").unwrap();
        std::fs::write(dir.path().join("one.rs"), "X\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "X", "path": dir.path().to_str().unwrap(), "mode": "files" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            r.get("file_groups").is_none(),
            "files mode has no per-line groups"
        );
        let files = r["files"].as_array().unwrap();
        assert_eq!(
            files[0]["count"].as_u64().unwrap(),
            3,
            "ranked by count desc"
        );
        assert_eq!(r["total"].as_u64().unwrap(), 4);
        assert_eq!(r["files_count"].as_u64().unwrap(), 2);
    }

    #[test]
    fn enclosing_symbol_returns_innermost_name_path() {
        use crate::lsp::symbols::{SymbolInfo, SymbolKind};
        fn sym(name_path: &str, start: u32, end: u32, children: Vec<SymbolInfo>) -> SymbolInfo {
            SymbolInfo {
                name: name_path.rsplit('/').next().unwrap().to_string(),
                name_path: name_path.to_string(),
                kind: SymbolKind::Function,
                file: std::path::PathBuf::from("x.rs"),
                start_line: start,
                end_line: end,
                range_start_line: None,
                start_col: 0,
                children,
                detail: None,
            }
        }
        // impl Foo (10..30) { fn bar (15..25) { ... } }
        let syms = vec![sym(
            "impl Foo",
            10,
            30,
            vec![sym("impl Foo/bar", 15, 25, vec![])],
        )];
        assert_eq!(
            enclosing_symbol(&syms, 20),
            Some("impl Foo/bar".to_string()),
            "innermost wins"
        );
        assert_eq!(
            enclosing_symbol(&syms, 12),
            Some("impl Foo".to_string()),
            "outer when not in child"
        );
        assert_eq!(enclosing_symbol(&syms, 99), None, "outside all symbols");
    }

    #[tokio::test]
    async fn grep_attaches_enclosing_symbol_when_small() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("code.rs"),
            "fn alpha() {\n    let needle = 1;\n}\n",
        )
        .unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "needle", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        let item = &r["file_groups"][0]["items"][0];
        assert_eq!(item["symbol"].as_str().unwrap(), "alpha");
    }

    #[tokio::test]
    async fn grep_no_symbol_for_markdown() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("doc.md"), "# Title\nneedle here\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "needle", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(r["file_groups"][0]["items"][0].get("symbol").is_none());
    }
}
