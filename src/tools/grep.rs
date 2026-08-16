//! `grep` tool and related format helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::format_overflow;
use super::{optional_u64_param, OutputForm, RecoverableError, Tool, ToolContext};
use crate::util::fs::to_forward_slash;

// ── grep ───────────────────────────────────────────────────────

pub struct Grep;

#[async_trait::async_trait]
impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Regex search across files. Flags: ignore_case, whole_word, glob (\"*.rs\"), include_hidden. mode=\"files\" for per-file counts. Source hits carry their enclosing symbol. context_lines for surrounding code."
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
                "file_path": { "type": "string", "description": "Alias for path" },
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
        let raw_path = strip_buffer_ref_quotes(
            input["path"]
                .as_str()
                .or_else(|| {
                    crate::fs::PATH_PARAM_ALIASES
                        .iter()
                        .find_map(|a| input.get(*a).and_then(|v| v.as_str()))
                })
                .unwrap_or("."),
        );

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
        // Output size is bounded independently of `max`, which counts matches, not
        // bytes. See MAX_MATCH_BYTES for why the two diverge catastrophically on
        // generated single-line files.
        let mut emitted_bytes = 0usize;
        let mut byte_capped = false;
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
        let mut audit = WalkAudit::default();
        if files_mode {
            use std::collections::BTreeMap;
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            let mut total = 0usize;
            let mut skipped_binary = 0usize;
            for entry in walker {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        audit.errors += 1;
                        tracing::warn!(error = %e, "grep: walk entry unreadable (files mode)");
                        continue;
                    }
                };
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(entry.path()) else {
                    audit.errors += 1;
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
                    *counts.entry(to_forward_slash(entry.path())).or_default() += n;
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
            if total == 0 {
                if let Some(w) = audit.completeness_warning(&search_path, include_hidden) {
                    r["completeness_warning"] = json!(w);
                }
            }
            return Ok(r);
        }
        'outer: for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    audit.errors += 1;
                    tracing::warn!(error = %e, "grep: walk entry unreadable");
                    continue;
                }
            };
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let Ok(bytes) = std::fs::read(entry.path()) else {
                audit.errors += 1;
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
                        let content = clamp_match(line);
                        emitted_bytes += content.len();
                        matches.push(json!({
                            "file": to_forward_slash(entry.path()),
                            "line": i + 1,
                            "content": content
                        }));
                        if matches.len() >= max || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
                            hit_cap = true;
                            byte_capped |= emitted_bytes >= MAX_TOTAL_MATCH_BYTES;
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
                                let content = clamp_match(&content);
                                emitted_bytes += content.len();
                                matches.push(json!({
                                    "file": to_forward_slash(entry.path()),
                                    "match_lines": match_lines,
                                    "start_line": blk_start + 1,
                                    "content": content,
                                }));
                                current = Some((ctx_start, vec![i], ctx_end));
                            }
                        }
                    }

                    if total_match_count >= max || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
                        hit_cap = true;
                        byte_capped |= emitted_bytes >= MAX_TOTAL_MATCH_BYTES;
                        break;
                    }
                }

                // Emit the last in-flight block
                if let Some((blk_start, blk_matches, blk_end)) = current {
                    let content = file_lines[blk_start..=blk_end].join("\n");
                    let match_lines: Vec<u64> =
                        blk_matches.iter().map(|&m| (m + 1) as u64).collect();
                    let content = clamp_match(&content);
                    emitted_bytes += content.len();
                    matches.push(json!({
                        "file": to_forward_slash(entry.path()),
                        "match_lines": match_lines,
                        "start_line": blk_start + 1,
                        "content": content,
                    }));
                }

                if total_match_count >= max || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
                    hit_cap = true;
                    byte_capped |= emitted_bytes >= MAX_TOTAL_MATCH_BYTES;
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
        if context_lines == 0 && !hit_cap {
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
            // Named for what it is: a cap on the NUMBER of matches. It was called
            // `budget`, which reads as a size bound and is how a `limit: 40` search
            // came to emit 4.4M tokens.
            let max_matches = max;
            let (visible, total, files) = cap_grouped(matches, max_matches);
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
                let mut overflow = json!({
                    "shown": visible.len(),
                    "total": total,
                    "hint": hint,
                });
                if byte_capped {
                    // Say WHICH cap fired: "40 of 900 matches" and "stopped at 60KB"
                    // call for different next moves.
                    overflow["reason"] = json!("byte budget");
                    overflow["truncated_bytes"] = json!(true);
                }
                r["overflow"] = overflow;
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
        if total_match_count == 0 {
            if let Some(w) = audit.completeness_warning(&search_path, include_hidden) {
                result["completeness_warning"] = json!(w);
            }
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
    // Bound BEFORE the zero-match early return. A completeness warning exists precisely for the
    // zero case, so appending it further down would leave it invisible exactly where it is the
    // whole point — references.rs paid for that lesson under its BUG 2026-05-21.
    let warning = val.get("completeness_warning").and_then(|v| v.as_str());

    if total == 0 {
        let mut out = "0 matches".to_string();
        if let Some(w) = warning {
            out.push_str("\n\nwarning: ");
            out.push_str(w);
        }
        return out;
    }

    let mut out = String::new();

    if val.get("mode").and_then(|m| m.as_str()) == Some("literal_fallback") {
        out.push_str("[literal fallback] ");
    }

    // Dispatch: file_groups[] → simple mode (new shape).
    //           matches[]    → context mode (legacy shape with start_line items).
    //           files[]      → files mode (per-file ranked counts).
    if let Some(groups) = val["file_groups"].as_array() {
        let files = val["files"].as_u64().unwrap_or(0) as usize;
        format_search_simple_mode(&mut out, groups, total, files);
    } else if let Some(flat) = val["matches"].as_array() {
        let match_word = if total == 1 { "match" } else { "matches" };
        out.push_str(&format!("{total} {match_word}\n"));
        format_search_context_mode(&mut out, flat);
    } else if let Some(files_arr) = val["files"].as_array() {
        let files_count = val["files_count"]
            .as_u64()
            .unwrap_or(files_arr.len() as u64);
        out.push_str(&format!("{total} matches in {files_count} files\n"));
        for f in files_arr {
            let file = f["file"].as_str().unwrap_or("");
            let count = f["count"].as_u64().unwrap_or(0);
            out.push_str(&format!("  {count:>5}  {file}\n"));
        }
        if let Some(sb) = val["skipped_binary"].as_u64() {
            if sb > 0 {
                out.push_str(&format!("  ({sb} binary file(s) skipped)\n"));
            }
        }
    }

    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }
    // Only reachable if a future change attaches the warning alongside results; rendering it in
    // both branches is what keeps that change from silently losing it.
    if let Some(w) = warning {
        out.push_str("\nwarning: ");
        out.push_str(w);
        out.push('\n');
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
        match item["symbol"].as_str() {
            Some(sym) => format!("  {line:>5}: {content}  [{sym}]"),
            None => format!("  {line:>5}: {content}"),
        }
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

/// Byte ceiling for a single emitted match (one line, or one merged context block).
///
/// `limit` bounds the NUMBER of matches, which is only a proxy for output size —
/// and the proxy collapses on generated single-line files. Measured 2026-08-16: a
/// real call with `limit: 40` over a `*.json` glob buffered 4,427,639 tokens,
/// because a minified JSON file is one line and forty of them is megabytes.
/// `grep` is fourth by overflow *count* and first by overflow *tokens* by 5.7x.
/// See docs/issues/archive/2026-08-16-grep-limit-bounds-lines-not-bytes.md
const MAX_MATCH_BYTES: usize = 2_000;

/// Ceiling on the summed size of all emitted matches. Backstop for the case the
/// per-match clamp cannot reach: many matches, each individually reasonable.
const MAX_TOTAL_MATCH_BYTES: usize = 60_000;

/// Clamp one emitted match to `MAX_MATCH_BYTES`, marking the cut.
///
/// The marker is not decoration. A silently truncated result reads as complete —
/// the same defect class as the buffered-summary bug — so a caller who needs the
/// rest has to be able to see that there is a rest.
fn clamp_match(s: &str) -> String {
    if s.len() <= MAX_MATCH_BYTES {
        return s.to_string();
    }
    // Never split a UTF-8 code point; walk back to the nearest boundary.
    let mut cut = MAX_MATCH_BYTES;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}… [truncated: {} of {} bytes shown]",
        &s[..cut],
        cut,
        s.len()
    )
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
    // The buffer path carries the same line-count-is-not-a-size-bound defect as
    // the filesystem path above and needs the same clamp — a buffered command's
    // output can be one enormous line just as easily as a minified file can.
    let mut emitted_bytes = 0usize;

    if context_lines == 0 {
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                total_match_count += 1;
                let content = clamp_match(line);
                emitted_bytes += content.len();
                matches.push(json!({
                    "file": raw_path,
                    "line": i + 1,
                    "content": content,
                }));
                if matches.len() >= max || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
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
                        let content = clamp_match(&content);
                        emitted_bytes += content.len();
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

            if total_match_count >= max || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
                hit_cap = true;
                break;
            }
        }

        if let Some((blk_start, blk_matches, blk_end)) = current {
            let content = file_lines[blk_start..=blk_end].join("\n");
            let match_lines: Vec<u64> = blk_matches.iter().map(|&m| (m + 1) as u64).collect();
            // No accumulation here: this is the final block emitted and nothing
            // reads the running total afterwards. The per-match clamp still applies,
            // which is the part that bounds the payload.
            let content = clamp_match(&content);
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
    if total_match_count == 0 && crate::util::path_security::is_identifier_pattern(pattern) {
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
/// What the walk declined to look at, so a zero-match result can say whether it can be
/// trusted. Two things are invisible to a caller otherwise: `ignore::Walk` yields
/// `Result<DirEntry, _>`, so a walk truncated by a permission error or descriptor exhaustion
/// reads as a complete one; and the crate reports nothing about entries its own `hidden`
/// filter pruned, so `0 matches` means the same thing whether the pattern is absent or its
/// only copies live under a dot-prefixed path. Sibling of `WalkAudit` in
/// `src/tools/symbol/symbols.rs`, added for the same class of false negative.
#[derive(Default)]
struct WalkAudit {
    /// Walk entries and file reads that failed. Counted rather than dropped.
    errors: usize,
}

impl WalkAudit {
    /// Dot-prefixed entries directly under `root`, which `hidden(true)` pruned.
    ///
    /// Only the search root is inspected — one `read_dir`, no recursion — so the warning
    /// wording claims nothing about deeper hidden directories. Naming the entries matters more
    /// than counting them: a reader can judge from `.github/` whether the skip is relevant to
    /// their query, which a bare count never tells them.
    ///
    /// `.git` and `.codescout` are excluded deliberately — see the comment on `uninformative`
    /// below. Both exist by construction in every project codescout touches, so naming them
    /// would make the warning fire on every zero-match everywhere, training readers to skip it
    /// in the cases that do matter.
    fn hidden_at_root(root: &std::path::Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(root) else {
            return Vec::new(); // not a directory, or unreadable — nothing to claim
        };
        // Machine-managed metadata directories, present by construction in every project
        // codescout touches. Their presence carries no information, so naming them would make
        // the warning fire on essentially every zero-match — the failure mode that stops
        // warnings from being read. Content inside them is reachable via include_hidden=true
        // (or, for memories, the `memory` tool) when that is genuinely what was wanted.
        let uninformative = [".git", ".codescout"];
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.starts_with('.') || uninformative.contains(&name.as_str()) {
                    return None;
                }
                let dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                Some(if dir { format!("{name}/") } else { name })
            })
            .collect();
        // Directories before files, alphabetical within each group. A pruned directory hides an
        // unbounded subtree; a pruned dotfile hides exactly one file — so directories carry far
        // more information per character of a truncated list. Pure alphabetical ordering put
        // `.github/` 12th of 16 in this repo, behind five `.env*` files, which cut the one entry
        // the warning existed to surface.
        names.sort_by(|a, b| {
            let (a_dir, b_dir) = (a.ends_with('/'), b.ends_with('/'));
            b_dir.cmp(&a_dir).then_with(|| a.cmp(b))
        });
        names
    }

    /// The warning for a zero-match result, or `None` when the zero can be trusted.
    ///
    /// `None` is load-bearing: a clean walk over a tree with no hidden entries must return a
    /// bare zero, or the warning becomes noise attached to every empty result and stops being
    /// read at all.
    fn completeness_warning(&self, root: &std::path::Path, include_hidden: bool) -> Option<String> {
        let hidden = if include_hidden {
            Vec::new()
        } else {
            Self::hidden_at_root(root)
        };
        if self.errors == 0 && hidden.is_empty() {
            return None;
        }

        let mut msg = String::from("this zero describes what was searched, not the pattern.");
        if self.errors > 0 {
            msg.push_str(&format!(
                " The walk could not read {} entr{} — re-run, and if it persists check for \
                 unreadable directories or file-descriptor exhaustion from many concurrent \
                 searches.",
                self.errors,
                if self.errors == 1 { "y" } else { "ies" }
            ));
        }
        if !hidden.is_empty() {
            let shown: Vec<&str> = hidden.iter().take(8).map(String::as_str).collect();
            let more = hidden.len() - shown.len();
            msg.push_str(&format!(
                " Hidden paths were not searched, including {}{} at the search root. Pass \
                 include_hidden=true to search them — a glob cannot re-admit them, because \
                 overrides are applied inside a walk that has already pruned the parent \
                 directory. `.git` and `.codescout` are excluded from this list.",
                shown.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ));
        }
        Some(msg)
    }
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
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
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
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
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

    /// `limit` counts matching LINES, which is only a proxy for output size — and
    /// the proxy collapses on generated single-line files. Measured 2026-08-16: one
    /// real call with `limit: 40` over a `*.json` glob buffered 4,427,639 tokens,
    /// and `grep` accounts for 68% of all buffered tokens in the corpus on a mere
    /// 3.0% overflow rate. The rate is low; the blast radius per incident is not.
    ///
    /// Mutation caught: removing the byte budget restores unbounded output under a
    /// limit the caller set correctly.
    #[tokio::test]
    async fn grep_bounds_output_bytes_not_only_matching_lines() {
        use serde_json::json;
        let dir = tempdir().unwrap();
        // Minified-JSON shape: the whole file is one enormous line.
        let huge = format!("{{\"needle\":\"{}\"}}", "x".repeat(200_000));
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("min{i}.json")), &huge).unwrap();
        }

        let ctx = test_ctx().await;
        let result = Grep
            .call(
                json!({ "pattern": "needle", "path": dir.path().to_str().unwrap(), "limit": 40 }),
                &ctx,
            )
            .await
            .unwrap();

        let payload = serde_json::to_string(&result).unwrap();
        assert!(
            payload.len() < 64_000,
            "grep output must be bounded by BYTES, not only by matching-line count; \
             limit=40 over five 200KB single-line files produced {} bytes",
            payload.len()
        );
    }

    /// A silently cut result reads as complete — the same defect class as the
    /// buffered-summary bug. If a line is clamped, the payload must say so.
    #[tokio::test]
    async fn grep_marks_a_clamped_line_instead_of_silently_cutting() {
        use serde_json::json;
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("min.json"),
            format!("{{\"needle\":\"{}\"}}", "x".repeat(50_000)),
        )
        .unwrap();

        let ctx = test_ctx().await;
        let result = Grep
            .call(
                json!({ "pattern": "needle", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let payload = serde_json::to_string(&result).unwrap();
        assert!(
            payload.contains("truncated"),
            "a clamped line must SAY it was clamped, got: {}",
            &payload[..payload.len().min(400)]
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
    async fn grep_accepts_file_path_alias_for_scope() {
        // file_path must actually scope the search — not silently fall back to
        // "." (project root). Decoy at root + hit in sub/ discriminates the two.
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("hit.rs"), "NEEDLE\n").unwrap();
        std::fs::write(dir.path().join("decoy.rs"), "NEEDLE\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;

        let r = Grep
            .call(
                json!({ "pattern": "NEEDLE", "file_path": sub.to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        let s = r.to_string();
        assert!(s.contains("hit.rs"), "should find the in-scope match: {s}");
        assert!(
            !s.contains("decoy.rs"),
            "file_path must scope to sub/, not fall back to project root: {s}"
        );
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

    #[tokio::test]
    async fn mode_files_renders_in_compact_output() {
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
        let text = format_grep(&r);
        assert!(
            !text.is_empty() && text != "0 matches",
            "files-mode output must not be empty, got: {text:?}"
        );
        assert!(
            text.contains("matches in") && text.contains("files"),
            "expected a 'N matches in M files' header, got: {text}"
        );
        assert!(
            text.contains("many.rs") && text.contains("one.rs"),
            "expected both files listed with counts, got: {text}"
        );
    }

    #[tokio::test]
    async fn compact_output_includes_enclosing_symbol() {
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
        let text = format_grep(&r);
        assert!(
            text.contains("[alpha]"),
            "expected enclosing symbol annotation in compact output, got: {text}"
        );
    }

    #[tokio::test]
    async fn buffer_suggestion_absent_when_matches_exist() {
        let ctx = test_ctx().await;
        let buf_id = ctx
            .output_buffer
            .store_tool("probe", "{\"my_symbol\": \"value here\"}".to_string());
        let r = Grep
            .call(json!({ "pattern": "my_symbol", "path": buf_id }), &ctx)
            .await
            .unwrap();
        assert!(
            r.get("suggestion").is_none(),
            "no suggestion expected when matches exist, got: {r:?}"
        );
    }

    #[tokio::test]
    async fn glob_and_ignore_case_compose() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.rs"), "TaRgEt\n").unwrap();
        std::fs::write(dir.path().join("skip.txt"), "TaRgEt\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({
                    "pattern": "target",
                    "path": dir.path().to_str().unwrap(),
                    "glob": "*.rs",
                    "ignore_case": true,
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 1, "result: {r:?}");
    }

    /// 2026-07-18: existing glob tests only cover wildcard patterns (`"*.rs"`).
    /// Regression coverage for a literal, wildcard-free `glob` value matched
    /// against a multi-segment relative path — the originally reported bug
    /// (a false negative here) was not reproducible on retest, but this gap
    /// in coverage was real and worth closing.
    #[tokio::test]
    async fn glob_matches_literal_multi_segment_path() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub").join("dir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("file.rs"), "TARGET\n").unwrap();
        std::fs::write(dir.path().join("file.rs"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;

        let r = Grep
            .call(
                json!({
                    "pattern": "TARGET",
                    "path": dir.path().to_str().unwrap(),
                    "glob": "sub/dir/file.rs",
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            r["total"].as_u64().unwrap(),
            1,
            "literal multi-segment glob must match the nested file only: {r:?}"
        );
        let s = r.to_string();
        assert!(
            s.contains("sub") && s.contains("dir") && s.contains("file.rs"),
            "result must reference the nested file: {s}"
        );
    }

    /// Same as above but with the array form of `glob`, and asserting the
    /// root-level decoy file (outside the literal path) is excluded.
    #[tokio::test]
    async fn glob_array_matches_literal_multi_segment_path_only() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub").join("dir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("file.rs"), "TARGET\n").unwrap();
        std::fs::write(dir.path().join("other.rs"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;

        let r = Grep
            .call(
                json!({
                    "pattern": "TARGET",
                    "path": dir.path().to_str().unwrap(),
                    "glob": ["sub/dir/file.rs"],
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            r["total"].as_u64().unwrap(),
            1,
            "array-form literal glob must match only the nested file: {r:?}"
        );
        let s = r.to_string();
        assert!(
            !s.contains("other.rs"),
            "file outside the literal glob must never be included: {s}"
        );
    }
    #[tokio::test]
    async fn zero_match_over_a_tree_with_a_hidden_dir_says_it_was_not_searched() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        std::fs::write(
            dir.path().join(".github/workflows/ci.yml"),
            "run: cargo test -- --skip TARGET_NAME\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("visible.rs"), "fn main() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let res = Grep
            .call(
                json!({ "pattern": "TARGET_NAME", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(res["total"].as_u64().unwrap(), 0);
        let w = res["completeness_warning"].as_str().expect(
            "a zero over a tree with a pruned hidden dir must say the dir was not searched",
        );
        assert!(w.contains(".github/"), "must name the pruned entry: {w}");
        assert!(w.contains("include_hidden"), "must name the remedy: {w}");
    }

    #[tokio::test]
    async fn zero_match_over_a_clean_tree_returns_a_bare_zero() {
        // Searches a subdirectory so the project root's `.codescout/` (created by rooted_ctx)
        // is out of scope. This pins the None branch: without it the warning would ride along
        // on every empty result and stop being read in the cases that matter.
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.rs"), "fn main() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let res = Grep
            .call(
                json!({ "pattern": "ABSENT_PATTERN", "path": sub.to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(res["total"].as_u64().unwrap(), 0);
        assert!(
            res.get("completeness_warning").is_none(),
            "a walk with nothing pruned must leave the zero bare: {res}"
        );
    }

    #[tokio::test]
    async fn metadata_dirs_alone_do_not_trigger_the_warning() {
        // `.git` and `.codescout` exist by construction in every project codescout touches, so
        // if they counted, every zero-match everywhere would carry a warning.
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir_all(sub.join(".git")).unwrap();
        std::fs::create_dir_all(sub.join(".codescout")).unwrap();
        std::fs::write(sub.join("a.rs"), "fn main() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let res = Grep
            .call(
                json!({ "pattern": "ABSENT_PATTERN", "path": sub.to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(res["total"].as_u64().unwrap(), 0);
        assert!(
            res.get("completeness_warning").is_none(),
            "machine-managed metadata dirs must not trigger the warning: {res}"
        );
    }

    #[tokio::test]
    async fn include_hidden_suppresses_the_warning_even_on_a_zero() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        std::fs::write(dir.path().join(".github/ci.yml"), "nothing here\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let res = Grep
            .call(
                json!({
                    "pattern": "ABSENT_PATTERN",
                    "path": dir.path().to_str().unwrap(),
                    "include_hidden": true
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(res["total"].as_u64().unwrap(), 0);
        assert!(
            res.get("completeness_warning").is_none(),
            "nothing was pruned, so this zero is trustworthy: {res}"
        );
    }

    #[tokio::test]
    async fn files_mode_zero_match_carries_the_completeness_warning() {
        // mode="files" returns from its own branch, so it needs the warning wired separately —
        // the shape of bug that hides in an early return.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        std::fs::write(dir.path().join(".github/ci.yml"), "TARGET_NAME\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let res = Grep
            .call(
                json!({
                    "pattern": "TARGET_NAME",
                    "path": dir.path().to_str().unwrap(),
                    "mode": "files"
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(res["total"].as_u64().unwrap(), 0);
        assert!(
            res["completeness_warning"]
                .as_str()
                .unwrap_or_default()
                .contains(".github/"),
            "files mode returns early and needs its own warning: {res}"
        );
    }

    #[test]
    fn format_grep_surfaces_the_completeness_warning_on_zero_matches() {
        let out = format_grep(&json!({
            "total": 0,
            "completeness_warning": "Hidden paths were not searched, including .github/ at the search root."
        }));
        assert!(out.starts_with("0 matches"), "{out}");
        assert!(
            out.contains(".github/"),
            "the warning must survive the zero-match early return: {out}"
        );
    }

    #[test]
    fn format_grep_leaves_a_trustworthy_zero_bare() {
        let out = format_grep(&json!({ "total": 0 }));
        assert_eq!(out, "0 matches");
    }
    #[tokio::test]
    async fn a_pruned_directory_survives_truncation_of_the_hidden_list() {
        // Regression for a live-tree failure the earlier tests could not see: each of them had
        // exactly one hidden entry, so the truncation path never ran. The real repo has 16, and
        // pure alphabetical ordering put `.github/` 12th — behind five `.env*` files — cutting
        // the one entry the warning existed to surface. Directories sort first now, because a
        // pruned directory hides an unbounded subtree and a pruned dotfile hides one file.
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.rs"), "fn main() {}\n").unwrap();
        // Nine dotfiles that all sort before the directory alphabetically.
        for i in 0..9 {
            std::fs::write(sub.join(format!(".aaa{i}")), "x\n").unwrap();
        }
        std::fs::create_dir_all(sub.join(".zeta")).unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let res = Grep
            .call(
                json!({ "pattern": "ABSENT_PATTERN", "path": sub.to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(res["total"].as_u64().unwrap(), 0);
        let w = res["completeness_warning"].as_str().unwrap();
        assert!(
            w.contains(".zeta/"),
            "the pruned directory must survive truncation: {w}"
        );
        assert!(
            w.contains("and 2 more"),
            "10 entries with a cap of 8 must report the remainder: {w}"
        );
    }
}
