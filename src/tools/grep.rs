//! `grep` tool and related format helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::{insert_below_header, overflow_head};
use super::{optional_u64_param, OutputForm, RecoverableError, Tool, ToolContext};
use crate::util::fs::to_forward_slash;

// ── grep ───────────────────────────────────────────────────────

pub struct Grep;

#[async_trait::async_trait]
impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::read_only_closed()
    }

    fn description(&self) -> &str {
        "Regex search across files. Flags: ignore_case, whole_word, glob (\"*.rs\"), include_hidden. mode=\"files\" for per-file counts. Source hits carry their enclosing symbol. context_lines for surrounding code."
    }

    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern" },
                "path": { "type": "string", "description": "File or directory (default: project root)" },
                // FIXTURE NOTE: the literal "Alias for " prefix here is load-bearing —
                // src/server.rs's required_names_no_key_that_has_a_declared_alias
                // (EXPECTED_ALIAS_COUNTS_BY_TOOL["grep"] == 1) parses it. `grep`'s
                // `path` is optional (required=["pattern"] alone), so this alias is
                // correctly NOT flagged as an offender — see the companion gate's
                // scope note for why it's excluded there too.
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
        // Simple mode oversamples so `cap_grouped` has something to choose from; context
        // mode returns blocks flat and must stop at what was asked for. BL-31.
        let collect_limit = if context_lines == 0 {
            max.saturating_mul(COLLECTION_OVERSAMPLE)
        } else {
            max
        };
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
        // An absolute glob outside the search root can never match a candidate this walk
        // yields, so without this the call returns a confident `0 matches` about a file it
        // never opened. See unsatisfiable_absolute_glob.
        if let Some(g) = unsatisfiable_absolute_glob(&globs, &search_path) {
            return Err(RecoverableError::with_hint(
                format!(
                    "glob '{}' is an absolute path outside the search root '{}', so it cannot \
                     match anything — glob patterns are filtered against a walk rooted there, \
                     and every candidate starts with that root",
                    g,
                    search_path.display()
                ),
                "To search that file, pass it as path=<absolute path> — `path` resolves the \
                 target directly instead of filtering a walk, and works across repos. For \
                 sustained work in another repo, workspace(action=\"activate\", path=…) first.",
            )
            .into());
        }
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
                // Counted here, before the read: this is the point the override has
                // admitted the file, which is what a starved glob is the absence of.
                audit.accepted += 1;
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
                if let Some(w) = audit.completeness_warning(&search_path, include_hidden, &globs) {
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
            // Counted here, before the read: this is the point the override has
            // admitted the file, which is what a starved glob is the absence of.
            audit.accepted += 1;
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
                        if matches.len() >= collect_limit || emitted_bytes >= MAX_TOTAL_MATCH_BYTES
                        {
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
            //
            // This budget is `max`, while collection ran to `collect_limit` — the two
            // are deliberately DIFFERENT now. While they were equal, `cap_grouped`
            // early-returned on `budget >= total` every single time and its
            // file-diversity round-robin was unreachable from grep. BL-31.
            let max_matches = max;
            // Rank the narrowing candidates on the PRE-cap tally, and keep owned
            // copies so the borrow ends before `cap_grouped` takes `matches`.
            //
            // These used to be read off `group_by_file(&visible)` — the post-cap set —
            // which counts what SURVIVED the diversity round-robin rather than what
            // matched. The cap flattens nearly every file to the same small number, so
            // the sort tied almost everything and fell back to path order: one real
            // search offered three files "(3 matches)" each while holding 11, 5 and 18,
            // and never named the file holding 20. Every figure in the hint was wrong
            // and so was the selection.
            // docs/issues/archive/2026-08-17-grep-narrowing-hint-ranks-by-capped-display-count.md
            let precap_top: Vec<(String, usize)> = group_by_file(&matches)
                .iter()
                .take(3)
                .map(|g| (g.file.to_string(), g.items.len()))
                .collect();
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
                // A `+` when collection itself stopped early: the pre-cap tally is
                // then a floor for that file too, and a bare number would re-import
                // the same false precision one level down.
                let top: Vec<String> = precap_top
                    .iter()
                    .map(|(file, n)| {
                        let plus = if hit_cap { "+" } else { "" };
                        format!("path=\"{file}\" ({n}{plus} matches)")
                    })
                    .collect();
                let narrow = if top.is_empty() {
                    "narrow with a more specific pattern or add path=<file>".to_string()
                } else {
                    format!("narrow with one of: {}", top.join(", "))
                };
                // `max` is BOTH the walk's break threshold (`:81`) and cap_grouped's
                // budget (`:339`), so a collection cap always lands on
                // `visible.len() == total`. Printing "N of N" there emits the exact
                // string a COMPLETE result prints: the reader cannot tell a capped
                // sample from an exhaustive one, and a capped sample that happens to be
                // homogeneous reads as a finding. Never print a denominator the walk
                // never counted.
                // BL-2 / docs/issues/archive/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md
                let hint = if hit_cap {
                    let (stopped_at, more) = if byte_capped {
                        (
                            "the output byte budget".to_string(),
                            "raising limit will not help — shorten the matched lines or",
                        )
                    } else {
                        // Name the real threshold. Collection walks to `collect_limit`,
                        // not to `limit`, so "stopped at limit=4" would be false after
                        // BL-31 — though "raise limit" stays the right advice, since
                        // `collect_limit` scales with it.
                        (
                            format!("the candidate cap ({collect_limit}) for limit={max}"),
                            "raise limit, or",
                        )
                    };
                    format!(
                        "Collection stopped at {stopped_at}, so the true total is unknown — \
                         {} matches across {} files is a floor, not a count. \
                         To see more, {more} {narrow}. \
                         Or mode=\"files\" for a per-file count summary.",
                        visible.len(),
                        files
                    )
                } else {
                    // Reachable since BL-31 decoupled the two thresholds: collection
                    // ran to `collect_limit` without stopping early, so every match was
                    // counted and `total` is exact. Printing the denominator here is
                    // honest — which is precisely the condition BL-2 required, not a
                    // relaxation of it.
                    format!(
                        "Showing {} of {} matches across {} files. \
                         To trim, {narrow}. \
                         Or mode=\"files\" for a per-file count summary.",
                        visible.len(),
                        total,
                        files
                    )
                };
                let mut overflow = json!({
                    "shown": visible.len(),
                    "total": total,
                    "hint": hint,
                });
                if hit_cap {
                    // `total` is what the walk collected before stopping, not what
                    // exists. Machine readers need that distinction as much as prose
                    // readers do — `shown == total` alone cannot carry it.
                    overflow["total_is_lower_bound"] = json!(true);
                }
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
            if let Some(w) = audit.completeness_warning(&search_path, include_hidden, &globs) {
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
        // Set only when the walk stopped early, so `total` is a lower bound.
        let total_is_floor = val
            .get("overflow")
            .and_then(|o| o.get("total_is_lower_bound"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        format_search_simple_mode(&mut out, groups, total, files, total_is_floor);
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

    // Overflow and the completeness warning go immediately BELOW the count header, not at
    // the tail. The compact summary is cut from the tail (`truncate_compact`), so a note
    // appended after a long row list is dropped on exactly the results big enough to need
    // it. The header keeps first place because it carries the `capped` marker a reader
    // anchors on — see `format::insert_below_header`. Same lesson as the
    // bound-before-the-early-return care taken for `warning` above, other end of the
    // function.
    let mut head_extra = overflow_head(val);
    if let Some(w) = warning {
        head_extra.push_str(&format!("warning: {w}\n"));
    }
    insert_below_header(out, &head_extra)
}

fn format_search_simple_mode(
    out: &mut String,
    file_groups: &[Value],
    total: usize,
    files: usize,
    total_is_floor: bool,
) {
    use crate::tools::file_group::{groups_from_json, render_grouped};

    let groups = groups_from_json(file_groups);
    // The header is the line a reader anchors on, and it is read before the overflow
    // hint two lines below. When collection stopped early `total` is a floor, so a
    // bare "N matches in M files" states as fact something the walk never
    // established — the qualifier has to travel with the number.
    let noun = match (total, total_is_floor) {
        (_, true) => "matches (capped)",
        (1, false) => "match",
        (_, false) => "matches",
    };

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

/// How many candidate matches simple-mode collection gathers per unit of `limit`
/// before [`cap_grouped`] trims back down to `limit`.
///
/// Collection used to stop at exactly `limit`, which made `cap_grouped`'s
/// file-diversity round-robin **unreachable**: it early-returns when
/// `budget >= total`, and `total` could never exceed the budget. So the capped result
/// was simply the first `limit` matches in filesystem walk order, and the overflow
/// hint's "narrow with one of" list named whichever files the walker happened to reach
/// first rather than the ones with the most matches — measured live, every suggested
/// file held exactly one match, which cannot reduce an already-capped result. BL-31.
///
/// Oversampling changes how many candidates the trimmer chooses *from*; it does not
/// change how many are returned. `MAX_TOTAL_MATCH_BYTES` is deliberately unchanged and
/// remains the authoritative payload bound — a `limit: 40` search once emitted 4.4M
/// tokens, and that was fixed by bounding **bytes**, not by bounding the count. On a
/// heavy corpus the byte budget still stops the walk first.
///
/// Applies to simple mode only. Context mode returns its merged blocks flat, without
/// `cap_grouped`, so oversampling there would return more blocks than were asked for.
const COLLECTION_OVERSAMPLE: usize = 4;

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

/// The absolute glob that cannot match anything this walk will yield, if there is one.
///
/// `ignore::overrides::OverrideBuilder` matches its patterns against candidates produced
/// by a walk rooted at `search_path`, so an absolute pattern outside that root is
/// **unsatisfiable by construction** — every candidate carries the root as a prefix. The
/// walk then completes normally and reports `0 matches`, which reads as "the pattern is
/// absent" when it means "the target was never visited". A false negative that looks like
/// a finding is worse than an error, because nothing prompts the caller to look again.
///
/// Measured 2026-08-18: a sibling repo's file searched with `glob=<abs path>` returned 0,
/// while the same file and the same pattern searched with `path=<abs path>` returned its
/// 8 matches — `path` resolves the target directly and so escapes the root. The zero also
/// carried the hidden-paths completeness warning, whose suggested remedy
/// (`include_hidden=true`) could not have helped here; naming an unchecked cause ends the
/// search for the real one.
///
/// Negations (`!…`) and relative patterns are not absolute paths and are left alone. An
/// absolute glob *inside* the root is fine — `/…/codescout/src/**/*.rs` shares the prefix,
/// so the walk can yield candidates that match it.
///
/// `docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md`.
fn unsatisfiable_absolute_glob(globs: &[String], search_path: &std::path::Path) -> Option<String> {
    globs
        .iter()
        .find(|g| {
            let p = std::path::Path::new(g.as_str());
            p.is_absolute() && !p.starts_with(search_path)
        })
        .cloned()
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
    // Same decoupling as the filesystem path: simple mode oversamples so `cap_grouped`
    // has candidates to choose from, context mode returns blocks flat. BL-31 — this
    // function carries the identical collect-at-`max`, cap-at-`max` shape and the bug
    // report named only its sibling.
    let collect_limit = if context_lines == 0 {
        max.saturating_mul(COLLECTION_OVERSAMPLE)
    } else {
        max
    };
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
    // Mirrors the filesystem path: which cap fired decides whether raising `limit`
    // is even the right advice.
    let mut byte_capped = false;

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
                if matches.len() >= collect_limit || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
                    hit_cap = true;
                    byte_capped |= emitted_bytes >= MAX_TOTAL_MATCH_BYTES;
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
                byte_capped |= emitted_bytes >= MAX_TOTAL_MATCH_BYTES;
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
            // Same rule as the filesystem path: after a collection cap, `total` is
            // what we managed to collect, not what exists. Publishing it beside an
            // equal `shown` with nothing marking it a floor reads as complete.
            // BL-2 / docs/issues/archive/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md
            let mut overflow = json!({
                "shown": visible.len(),
                "total": total,
            });
            if hit_cap {
                let stopped_at = if byte_capped {
                    "the output byte budget".to_string()
                } else {
                    format!("limit={max}")
                };
                overflow["hint"] = json!(format!(
                    "Collection stopped at {stopped_at}, so the true total is unknown — \
                     {} matches is a floor, not a count. Raise limit or narrow the pattern.",
                    visible.len()
                ));
                overflow["total_is_lower_bound"] = json!(true);
                if byte_capped {
                    overflow["reason"] = json!("byte budget");
                    overflow["truncated_bytes"] = json!(true);
                }
            } else {
                overflow["hint"] = json!("Many matches. Narrow the pattern.");
            }
            r["overflow"] = overflow;
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
    /// Files the walk yielded and the override admitted. Zero while a glob is set means
    /// no file was ever opened, so the zero describes the file filter and not the
    /// pattern. Mirrors `accepted` on the `WalkAudit` in `src/tools/symbol/symbols.rs`,
    /// which carries it for the same class of false negative.
    accepted: usize,
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

    /// Whether `.gitignore` rules are actually in force for a walk rooted at `root`.
    ///
    /// `WalkBuilder::require_git` defaults to true, so the `ignore` crate applies gitignore
    /// rules only inside a git worktree. Gating on the same condition keeps the clause off
    /// searches of unversioned directories, where it would be true only vacuously.
    /// `a_gitignore_outside_a_git_repo_is_not_applied` pins that default rather than trusting
    /// it, since the whole gate rests on it.
    ///
    /// Deliberately does NOT look for `.gitignore` files or count excluded paths. Naming a
    /// population asserts that population is non-empty, and that is the assertion which fails
    /// green — so the clause claims a mechanism, and a mechanism needs only this.
    fn git_ignore_in_effect(root: &std::path::Path) -> bool {
        root.ancestors().any(|a| a.join(".git").exists())
    }

    /// The warning for a zero-match result, or `None` when the zero can be trusted.
    ///
    /// `None` is load-bearing: a clean walk over a tree with no hidden entries must return a
    /// bare zero, or the warning becomes noise attached to every empty result and stops being
    /// read at all.
    ///
    /// `globs` is taken because a zero also arises when the override admitted no file at all
    /// (see `accepted`). That cause is named first and on its own terms: the note on
    /// `unsatisfiable_absolute_glob` records a zero that carried the hidden-paths warning
    /// whose remedy could not have helped, and naming an unchecked cause ends the search for
    /// the real one. The clause claims only what the counter proves — that nothing under this
    /// root passed the filter — and offers the anchoring mismatch as the thing to check,
    /// since an empty tree produces the same count.
    ///
    /// The gitignore clause is the one condition keyed on an argument rather than a counter.
    /// `include_hidden` lifts the dotfile filter and nothing else, so acting on the hidden
    /// clause suppresses it while leaving a second, independent exclusion standing — which is
    /// how a widened search came to be strictly less informative than the narrow one it
    /// replaced. The clause names the mechanism and never a path: a root-level list of
    /// gitignored entries would be symmetric with `hidden_at_root` in shape only, because a
    /// nested `.gitignore` prunes a subtree whose own root is not ignored. On this repo that
    /// list is six entries, none of them the one that caused the reported zero — an unchecked
    /// cause, which the paragraph above exists to forbid. See `R-121`.
    fn completeness_warning(
        &self,
        root: &std::path::Path,
        include_hidden: bool,
        globs: &[String],
    ) -> Option<String> {
        let hidden = if include_hidden {
            Vec::new()
        } else {
            Self::hidden_at_root(root)
        };
        // Only a glob can starve the walk this way. With no glob set, zero accepted files
        // means an empty tree, which the error and hidden clauses already account for.
        let starved = !globs.is_empty() && self.accepted == 0;
        // Fires exactly when the caller has lifted the filter they were told about. That is
        // what keeps a bare `None` meaningful on the default path: an ordinary zero is
        // unchanged, and only a deliberately widened search that still found nothing is told
        // a second filter is still standing.
        let unlifted_gitignore = include_hidden && Self::git_ignore_in_effect(root);
        if self.errors == 0 && hidden.is_empty() && !starved && !unlifted_gitignore {
            return None;
        }

        let mut msg = String::from("this zero describes what was searched, not the pattern.");
        if starved {
            msg.push_str(&format!(
                " No file under '{}' passed the glob filter ({}), so none was opened — this \
                     zero is about the file filter, not the pattern. Globs are matched against a \
                     walk rooted there, so they resolve relative to THAT root and not the project \
                     root: when `path` narrows the search, a project-root-relative glob such as \
                     `src/foo.rs` cannot match. Drop the leading segments `path` already supplies, \
                     or omit `path` and let the glob carry the whole route.",
                root.display(),
                globs.join(", ")
            ));
        }
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
        if unlifted_gitignore {
            msg.push_str(
                " include_hidden=true lifts the dotfile filter only. Gitignore rules are a \
                     second and independent exclusion that no grep argument lifts, and they apply \
                     at every depth — a nested .gitignore prunes its own subtree, so a match can \
                     sit under a path that is not itself ignored. Reach one with a shell `grep`, \
                     or `git grep --no-index`.",
            );
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

    /// The predicate, in isolation. Each case is a decision the walk's semantics force:
    /// absolute-outside cannot match, absolute-inside can, relative is the normal case,
    /// and a negation is not an absolute path whatever it negates.
    #[test]
    fn unsatisfiable_absolute_glob_flags_only_absolute_paths_outside_the_root() {
        // "Absolute" is platform-defined, and this test is entirely about that predicate.
        // `/home/u/proj` carries no drive letter, so on Windows it is RELATIVE: every case
        // degraded to `None`, which failed the two `Some` assertions and — worse — made the
        // three `None` assertions pass VACUOUSLY, asserting nothing at all. Drive-prefix the
        // literals so both halves are live on both platforms.
        //
        // Forward slashes are kept on Windows deliberately: Rust accepts `/` as a separator
        // there, `Path::starts_with` compares by component either way, and a backslash inside
        // a GLOB is an escape, not a separator. Same treatment `containing_root_tests` in
        // `src/librarian/tools/mod.rs` gives the same problem.
        #[cfg(windows)]
        const P: &str = "C:";
        #[cfg(not(windows))]
        const P: &str = "";

        let root = format!("{P}/home/u/proj");
        let root = std::path::Path::new(&root);
        let outside = format!("{P}/home/u/other/x.rs");
        let inside = format!("{P}/home/u/proj/src/**/*.rs");
        let elsewhere = format!("{P}/elsewhere/y.rs");

        assert_eq!(
            unsatisfiable_absolute_glob(std::slice::from_ref(&outside), root).as_deref(),
            Some(outside.as_str())
        );
        assert_eq!(
            unsatisfiable_absolute_glob(&[inside], root),
            None,
            "an absolute glob INSIDE the root shares the prefix every candidate carries"
        );
        assert_eq!(
            unsatisfiable_absolute_glob(&["*.rs".to_string(), "src/**".to_string()], root),
            None,
            "relative globs are the normal case and are matched against the root"
        );
        assert_eq!(
            unsatisfiable_absolute_glob(&[format!("!{outside}")], root),
            None,
            "a negation is not an absolute path"
        );
        assert_eq!(
            unsatisfiable_absolute_glob(&["*.rs".to_string(), elsewhere.clone()], root).as_deref(),
            Some(elsewhere.as_str()),
            "the offending glob is named even when it is not first"
        );
    }

    /// End-to-end, with the bug's own control attached.
    ///
    /// `glob` is filtered against a walk rooted at the search path, so an absolute glob
    /// outside that root matched nothing and the call answered a confident `0 matches`
    /// about a file it never opened — a false negative that reads as a finding.
    /// `docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md`
    #[tokio::test]
    async fn grep_rejects_an_absolute_glob_outside_the_search_root() {
        use serde_json::json;
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("a.rs"), "fn foo() {}\n").unwrap();
        let outside = tempdir().unwrap();
        let target = outside.path().join("lib.mjs");
        std::fs::write(&target, "export function readInput() {}\n").unwrap();

        let ctx = test_ctx().await;

        let err = Grep
            .call(
                json!({
                    "pattern": "export function readInput",
                    "path": root.path().to_str().unwrap(),
                    "glob": target.to_str().unwrap(),
                }),
                &ctx,
            )
            .await
            .expect_err("an unsatisfiable absolute glob must error, not report a zero");
        let msg = err.to_string();
        assert!(
            msg.contains("cannot match"),
            "must say why it is empty: {msg}"
        );
        assert!(
            msg.contains("path="),
            "must name the remedy that actually works: {msg}"
        );

        // The control, taken from the bug's own Reproduction step 2: the same file and the
        // same pattern via `path` still match. Without it this test would pass just as well
        // if the fix had broken cross-repo reads altogether.
        let ok = Grep
            .call(
                json!({
                    "pattern": "export function readInput",
                    "path": target.to_str().unwrap(),
                }),
                &ctx,
            )
            .await
            .expect("path= resolves the target directly and must still work");
        assert!(
            ok["total"].as_u64().unwrap_or(0) >= 1,
            "control must still match: {ok}"
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

    /// The narrowing hint must report and rank on the **pre-cap** per-file tally.
    ///
    /// `grep_overflow_hint_names_top_files` above has a skewed fixture but only
    /// asserts the hint *mentions* the hot file — never the number — so it stayed
    /// green while every count in the hint was wrong. The counts came off
    /// `group_by_file(&visible)`, i.e. what survived the diversity round-robin,
    /// which flattens files to a handful each; the sort then tied nearly
    /// everything and fell back to `group_by_file`'s path-ascending tiebreak.
    ///
    /// `aaa_decoy.rs` is the discriminator: alphabetically first, but with far
    /// fewer real matches. Ranking on capped counts puts it first; ranking on the
    /// true tally puts `hot.rs` first. A fixture without such a decoy cannot tell
    /// the two implementations apart.
    /// docs/issues/archive/2026-08-17-grep-narrowing-hint-ranks-by-capped-display-count.md
    #[tokio::test]
    async fn grep_overflow_hint_counts_and_ranks_before_the_cap() {
        use serde_json::json;
        let dir = tempdir().unwrap();
        let many: String = (0..40).map(|i| format!("fn target_{i}() {{}}\n")).collect();
        std::fs::write(dir.path().join("hot.rs"), many).unwrap();
        // Sorts first by path, holds far fewer matches.
        std::fs::write(
            dir.path().join("aaa_decoy.rs"),
            "fn target_x() {}\nfn target_y() {}\nfn target_z() {}\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("mid.rs"), "fn target_m() {}\n").unwrap();

        let ctx = test_ctx().await;
        // limit=15 caps the DISPLAY (15 of 44) without capping collection — the
        // candidate cap is a multiple of `limit`, so a smaller limit truncates the
        // walk too and every tally becomes a floor (`40` would read `16+`). This
        // isolates the display cap, which is the thing under test.
        let result = Grep
            .call(
                json!({ "pattern": "target", "path": dir.path().to_str().unwrap(), "limit": 15 }),
                &ctx,
            )
            .await
            .unwrap();

        let hint = result["overflow"]["hint"]
            .as_str()
            .expect("limit=15 against 44 matches must overflow with a hint");

        // Exactly 40, with no `+`: collection was complete, so the tally is a
        // count rather than a floor. Ranking on the post-cap set would report a
        // single-digit number here.
        assert!(
            hint.contains("hot.rs\" (40 matches)"),
            "hint must report hot.rs's TRUE match count (40), not the post-cap \
                 display count; got: {hint}"
        );

        // And the ranking must be by that tally, not by the capped tie. Paths in
        // the hint are absolute, so compare on the tail.
        let first_candidate = hint
            .split("path=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("hint must offer at least one path=");
        assert!(
            first_candidate.ends_with("hot.rs"),
            "the highest-match file must be offered FIRST; ranking on capped \
                 counts ties every file and falls back to path order, which would \
                 put aaa_decoy.rs here. got first candidate: {first_candidate}"
        );
    }

    #[tokio::test]
    /// BL-2 / `docs/issues/archive/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md`.
    ///
    /// When the walk stops early the total is a floor, and the hint must never print a
    /// denominator it did not count — "Showing N of N matches" is byte-identical to what
    /// a genuinely complete result prints, so a reader cannot tell a capped sample from
    /// an exhaustive one.
    ///
    /// The defect IS that identity, so a test inspecting only the capped string would
    /// have passed against the buggy output. Both rows are load-bearing: the `complete`
    /// row pins what "nothing was hidden" looks like, and was green before the fix.
    ///
    /// **Fixture widened for BL-31.** It used to be 3 files x 3 matches with `limit=4`,
    /// which capped because collection stopped at `limit`. Collection now runs to
    /// `limit * COLLECTION_OVERSAMPLE`, so that corpus is counted in full and "Showing 4
    /// of 9" became *honest* — the denominator is real. The corpus is now large enough
    /// to exhaust the candidate cap, which is what this test is actually about. Widening
    /// it rather than relaxing the assertion keeps BL-2's invariant exactly as strict.
    async fn grep_capped_collection_never_renders_as_a_complete_result() {
        use serde_json::json;
        let dir = tempdir().unwrap();
        // 8 files x 3 matches = 24. limit=4 gathers 4*4=16 candidates and stops there,
        // so the true total is genuinely unknown.
        for name in [
            "a.rs", "b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs", "h.rs",
        ] {
            std::fs::write(
                dir.path().join(name),
                "fn target_1() {}\nfn target_2() {}\nfn target_3() {}\n",
            )
            .unwrap();
        }

        let ctx = test_ctx().await;
        let tool = Grep;
        let path = dir.path().to_str().unwrap();

        let capped = tool
            .call(
                json!({ "pattern": "target", "path": path, "limit": 4 }),
                &ctx,
            )
            .await
            .unwrap();
        let complete = tool
            .call(
                json!({ "pattern": "target", "path": path, "limit": 50 }),
                &ctx,
            )
            .await
            .unwrap();

        // --- control row: a complete result hides nothing and says so by omission.
        assert!(
            complete.get("overflow").is_none(),
            "limit=50 over 24 matches must not overflow, got: {complete}"
        );
        assert_eq!(complete["total"].as_u64(), Some(24));

        // --- the row under test.
        let overflow = capped
            .get("overflow")
            .expect("limit=4 over 24 matches must overflow");
        assert_eq!(
            overflow
                .get("total_is_lower_bound")
                .and_then(|v| v.as_bool()),
            Some(true),
            "a collection-capped result must mark its total as a floor, got: {overflow}"
        );

        let hint = overflow["hint"]
            .as_str()
            .expect("overflow.hint is a string");
        assert!(
            !hint.contains("of 4 matches"),
            "the hint must not print a denominator it never counted — that is the \
             exact string a complete result prints. Got: {hint}"
        );
        assert!(
            hint.contains("true total is unknown"),
            "the hint must say the total is unknown, got: {hint}"
        );

        // --- the cross-row assertion: the two renderings must not be confusable,
        // on the header line a reader anchors on before reaching the hint.
        let capped_text = tool.format_compact(&capped).unwrap();
        let complete_text = tool.format_compact(&complete).unwrap();
        assert!(
            !complete_text.contains("capped"),
            "a complete result must carry no incompleteness marker, got:\n{complete_text}"
        );
        let header = capped_text.lines().next().unwrap_or_default();
        assert!(
            header.contains("capped"),
            "the capped result's FIRST line must not read as a plain count — that is \
             the line a reader anchors on. Got: {header}"
        );
    }

    /// BL-31 / `docs/issues/archive/2026-08-16-grep-file-diversity-round-robin-never-runs.md`.
    ///
    /// `cap_grouped` exists to preserve file diversity when a result is trimmed, but
    /// grep bound `max` as BOTH the collection break threshold and the cap budget — so
    /// `cap_grouped` was always handed a vector no larger than its budget, took its
    /// `budget >= total` early return, and its round-robin never ran. The capped result
    /// was the first `limit` matches in walk order.
    ///
    /// This asserts at the **caller** level, which is the whole point: `cap_grouped`'s
    /// own unit tests exercise the round-robin directly with `budget < total` and were
    /// green throughout. Nothing tested that a caller ever reached it.
    #[tokio::test]
    async fn grep_capped_result_spans_files_by_diversity_not_walk_order() {
        use serde_json::json;
        let dir = tempdir().unwrap();
        // 3 files x 3 matches = 9, well inside the candidate cap (4*4=16), so every
        // match is counted and the trim is a genuine choice rather than a truncation.
        for name in ["a.rs", "b.rs", "c.rs"] {
            std::fs::write(
                dir.path().join(name),
                "fn target_1() {}\nfn target_2() {}\nfn target_3() {}\n",
            )
            .unwrap();
        }

        let ctx = test_ctx().await;
        let res = Grep
            .call(
                json!({ "pattern": "target", "path": dir.path().to_str().unwrap(), "limit": 4 }),
                &ctx,
            )
            .await
            .unwrap();

        let groups = res["file_groups"]
            .as_array()
            .expect("simple mode returns file_groups");
        assert_eq!(
            groups.len(),
            3,
            "a 4-match budget over 3 equally-hot files must span ALL THREE (2/1/1), not \
             spend 3 of 4 on whichever file the walker reached first. Got: {res}"
        );

        let shown: usize = groups
            .iter()
            .map(|g| g["items"].as_array().map_or(0, |m| m.len()))
            .sum();
        assert_eq!(shown, 4, "the budget itself is unchanged, got: {res}");
        let per_file: Vec<usize> = groups
            .iter()
            .map(|g| g["items"].as_array().map_or(0, |m| m.len()))
            .collect();
        assert_eq!(
            per_file,
            vec![2, 1, 1],
            "round-robin gives every file one before any file gets a second, got: {res}"
        );

        // Every match was counted, so the denominator is real — this is the branch BL-2
        // had to keep "correct but unreachable" while the two thresholds were the same
        // number. Fixing diversity brought it back to life.
        assert_eq!(res["total"].as_u64(), Some(9));
        let hint = res["overflow"]["hint"]
            .as_str()
            .expect("a trimmed result overflows");
        assert!(
            hint.contains("Showing 4 of 9"),
            "when collection completed, printing the true denominator is honest — BL-2 \
             forbade printing one that was never counted, not printing one at all. \
             Got: {hint}"
        );
        assert!(
            res["overflow"].get("total_is_lower_bound").is_none(),
            "nothing was cut off, so the total is exact and must not be flagged a floor"
        );
    }

    /// The buffer twin. `grep_in_buffer` carries its own copy of the
    /// collect-then-`cap_grouped` sequence (`:846`), so a fix applied only to the
    /// filesystem path leaves `@cmd_*` / `@tool_*` searches still publishing a
    /// `total` equal to `shown` with nothing marking it as a floor.
    #[tokio::test]
    async fn grep_buffer_capped_collection_marks_the_total_as_a_floor() {
        use serde_json::json;
        let ctx = test_ctx().await;
        let body: String = (0..20).map(|i| format!("target_{i}\n")).collect();
        let raw = json!({ "id": "abc", "body": body }).to_string();
        let buf_id = ctx.output_buffer.store_tool("artifact", raw);

        let tool = Grep;
        let result = tool
            .call(
                json!({ "pattern": "target_", "path": buf_id, "limit": 5 }),
                &ctx,
            )
            .await
            .unwrap();

        let overflow = result
            .get("overflow")
            .expect("limit=5 over 20 buffer matches must overflow");
        assert_eq!(
            overflow
                .get("total_is_lower_bound")
                .and_then(|v| v.as_bool()),
            Some(true),
            "the buffer path must mark a collection-capped total as a floor, got: {overflow}"
        );
        let hint = overflow["hint"]
            .as_str()
            .expect("overflow.hint is a string");
        assert!(
            hint.contains("true total is unknown"),
            "the buffer hint must say the total is unknown, got: {hint}"
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
    async fn a_gitignore_outside_a_git_repo_is_not_applied() {
        // Pins `WalkBuilder::require_git`'s default, which is the whole premise of the gate on
        // the gitignore clause: if gitignore rules applied outside a worktree too, gating the
        // clause on finding a `.git` would silence it exactly where it was needed. A premise a
        // fix rests on earns a test that fails when the dependency changes it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "secret.txt\n").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            r["total"].as_u64().unwrap(),
            1,
            "no .git here, so the .gitignore must not be honoured: {r}"
        );
    }

    #[tokio::test]
    async fn widening_past_hidden_names_the_gitignore_filter_it_cannot_lift() {
        // The reported defect. The hidden clause names `.scratch/` and prescribes
        // include_hidden=true; passing it does reach the directory, but a NESTED .gitignore
        // prunes the contents, and the resulting zero used to be bare — so acting on the
        // warning made the caller strictly less informed.
        //
        // `.scratch/` is deliberately NOT itself ignored, only its subtree is. That is what
        // makes a root-level list of gitignored entries useless here, and why the clause names
        // the mechanism instead of a path. See R-121.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        let scratch = dir.path().join(".scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join(".gitignore"), "*\n").unwrap();
        std::fs::write(scratch.join("ledger.md"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;

        let narrow = Grep
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(narrow["total"].as_u64().unwrap(), 0, "hidden by default");
        let narrow_w = narrow["completeness_warning"].as_str().unwrap();
        assert!(
            narrow_w.contains(".scratch/"),
            "the hidden clause should still name the directory: {narrow_w}"
        );

        let widened = Grep
            .call(
                json!({ "pattern": "TARGET", "path": dir.path().to_str().unwrap(), "include_hidden": true }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            widened["total"].as_u64().unwrap(),
            0,
            "the nested .gitignore still prunes it"
        );
        let widened_w = widened["completeness_warning"]
            .as_str()
            .expect("a widened search that still found nothing must not go bare");
        assert!(
            widened_w.contains("Gitignore rules"),
            "must name the filter include_hidden did not lift: {widened_w}"
        );
    }

    #[tokio::test]
    async fn a_trustworthy_zero_stays_bare_inside_a_git_repo() {
        // The default path stays byte-identical to before the gitignore clause existed: a
        // clean walk with nothing hidden returns a bare zero even inside a worktree. `None`
        // is what stops the warning becoming noise attached to every empty result, and a
        // condition that fired on every zero would have spent it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn present() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        // Bootstrapping the agent inside a worktree registers `.codescout/private-memories/`
        // via `MemoryStore::ensure_gitignored`, which writes a `.gitignore` at the root. That
        // is a hidden file, so the hidden clause fires on it — correctly, and for a reason
        // that has nothing to do with what this test is pinning. Clear it to get the clean
        // tree the assertion is about. Tolerant of absence: if the bootstrap stops writing
        // it, this test should keep testing its own subject rather than start failing.
        let _ = std::fs::remove_file(dir.path().join(".gitignore"));
        let r = Grep
            .call(
                json!({ "pattern": "absent_xyz", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 0);
        assert!(
            r.get("completeness_warning").is_none(),
            "nothing hidden and nothing widened — this zero must stay bare: {r}"
        );
    }

    #[tokio::test]
    async fn widening_outside_a_git_repo_stays_bare() {
        // The other half of the gate. Outside a worktree the `ignore` crate applies no
        // gitignore rules at all, so claiming a filter stood in the way would be true only
        // vacuously — the shape that trains readers to skip warnings.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn present() {}\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;
        let r = Grep
            .call(
                json!({ "pattern": "absent_xyz", "path": dir.path().to_str().unwrap(), "include_hidden": true }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 0);
        assert!(
            r.get("completeness_warning").is_none(),
            "no git worktree, so no gitignore filter to warn about: {r}"
        );
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

    /// A glob that admits no file at all returns a bare zero, which reads as "the
    /// pattern is absent" when it means "no file was ever opened".
    ///
    /// This is the RELATIVE form of the hole `unsatisfiable_absolute_glob` guards:
    /// overrides are matched against a walk rooted at the resolved `search_path`,
    /// which `path` sets — so a glob written relative to the PROJECT root (the form
    /// this tool's own `glob` doc example uses, `["src/**", "*.md"]`) is
    /// unsatisfiable the moment `path` narrows the root, and nothing says so.
    /// Confirmed live 2026-08-27 against this repo: `glob="src/tools/grep.rs"`
    /// returns 1 match bare and 0 with `path="src"`, same pattern, same file.
    ///
    /// See `docs/issues/archive/2026-07-18-grep-glob-literal-path-false-negative-unconfirmed.md`.
    #[tokio::test]
    async fn glob_that_admits_no_file_names_the_glob_instead_of_a_bare_zero() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub").join("dir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("file.rs"), "TARGET\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;

        // Control: the file is findable, so a zero below is about the filter.
        let ok = Grep
            .call(
                json!({
                    "pattern": "TARGET",
                    "path": sub.to_str().unwrap(),
                    "glob": "file.rs",
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            ok["total"].as_u64().unwrap(),
            1,
            "control: a root-relative glob under this path matches: {ok:?}"
        );

        // Same file, same pattern — glob written relative to the project root
        // while `path` roots the walk at `sub/dir`. Admits nothing.
        let r = Grep
            .call(
                json!({
                    "pattern": "TARGET",
                    "path": sub.to_str().unwrap(),
                    "glob": "sub/dir/file.rs",
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            r["total"].as_u64().unwrap(),
            0,
            "precondition: the glob admits nothing: {r:?}"
        );
        let w = r["completeness_warning"].as_str().unwrap_or_default();
        assert!(
            w.contains("passed the glob filter"),
            "a zero from a glob that opened no file must name the glob as the cause — \
                 otherwise it reads as a finding: {r:?}"
        );
    }

    /// `None` is load-bearing (see `completeness_warning`): a glob that DOES admit
    /// files and simply finds nothing must not be blamed for the zero, or the clause
    /// attaches to every empty result and stops being read.
    ///
    /// Asserted on the glob clause specifically rather than on the absence of any
    /// warning: `rooted_ctx` writes a `.gitignore` at the root, so the pre-existing
    /// hidden-paths clause fires here and is correct to. That clause also contains the
    /// word "glob" ("a glob cannot re-admit them"), which is why this test and its
    /// sibling both match the full phrase rather than the bare word.
    #[tokio::test]
    async fn a_glob_that_admits_files_but_finds_nothing_stays_a_bare_zero() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file.rs"), "SOMETHING ELSE\n").unwrap();
        let ctx = rooted_ctx(dir.path()).await;

        let r = Grep
            .call(
                json!({
                    "pattern": "TARGET",
                    "path": dir.path().to_str().unwrap(),
                    "glob": "*.rs",
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 0);
        assert!(
            !r["completeness_warning"]
                .as_str()
                .unwrap_or_default()
                .contains("passed the glob filter"),
            "the glob admitted a file and the pattern was genuinely absent — this zero \
                 must not blame the glob for it: {r:?}"
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
        // Narrowed 2026-08-27, and the name is kept because an archived bug file cites it.
        // `include_hidden=true` no longer suppresses the warning unconditionally: inside a
        // git worktree it now draws the gitignore clause, since that filter is independent of
        // the dotfile one and the flag never lifted it. What this fixture pins is the other
        // side of that gate — no `.git` here, so the `ignore` crate applies no gitignore rules
        // (`require_git` defaults to true), nothing was pruned, and the zero is trustworthy.
        // The worktree case is `widening_past_hidden_names_the_gitignore_filter_it_cannot_lift`.
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
