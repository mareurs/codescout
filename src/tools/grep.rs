//! `grep` tool and related format helpers.

use anyhow::Result;
use serde_json::{json, Value};

use super::format::format_overflow;
use super::{optional_u64_param, RecoverableError, Tool, ToolContext};

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

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern" },
                "path": { "type": "string", "description": "File or directory (default: project root)" },
                "limit": { "type": "integer", "default": 50, "description": "Max matching lines" },
                "context_lines": { "type": "integer", "default": 0, "description": "Context lines before/after each match (max 20). Adjacent matches merge." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let pattern = super::require_str_param_or(&input, "pattern", &["query", "regex"])?;
        let raw_path = input["path"].as_str().unwrap_or(".");
        let project_root = ctx.agent.project_root().await;
        let security = ctx.agent.security_config().await;
        let search_path = crate::util::path_security::validate_read_path(
            raw_path,
            project_root.as_deref(),
            &security,
        )?;
        let max = optional_u64_param(&input, "limit").unwrap_or(50) as usize;
        let context_lines = optional_u64_param(&input, "context_lines")
            .unwrap_or(0)
            .min(20) as usize;
        let (re, is_literal_fallback) = match regex::RegexBuilder::new(pattern)
            .size_limit(1 << 20)
            .dfa_size_limit(1 << 20)
            .build()
        {
            Ok(re) => (re, false),
            Err(e) => {
                if super::is_regex_like(pattern) {
                    // User intended regex but it's broken — keep the error
                    return Err(RecoverableError::with_hint(
                        format!("invalid regex: {e}"),
                        "patterns are full regex syntax — escape metacharacters like \\( \\. \\[ for literals",
                    )
                    .into());
                }
                // Plain text with metacharacters — search literally
                let escaped = regex::escape(pattern);
                let re = regex::RegexBuilder::new(&escaped)
                    .size_limit(1 << 20)
                    .dfa_size_limit(1 << 20)
                    .build()
                    .map_err(|e2| {
                        RecoverableError::with_hint(
                            format!("invalid pattern even after escaping: {e2}"),
                            format!("original error: {e}"),
                        )
                    })?;
                (re, true)
            }
        };
        let mut matches: Vec<Value> = vec![];
        let mut total_match_count = 0usize;
        let mut hit_cap = false;

        let walker = ignore::WalkBuilder::new(&search_path)
            .hidden(true)
            .git_ignore(true)
            .build();
        'outer: for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };

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
                // (block_start_idx, first_match_idx, block_end_idx) — all 0-indexed
                let mut current: Option<(usize, usize, usize)> = None;

                for (i, line) in file_lines.iter().enumerate() {
                    if !re.is_match(line) {
                        continue;
                    }
                    total_match_count += 1;
                    let ctx_start = i.saturating_sub(context_lines);
                    let ctx_end = (i + context_lines).min(n.saturating_sub(1));

                    match current {
                        None => {
                            current = Some((ctx_start, i, ctx_end));
                        }
                        Some((blk_start, blk_first, blk_end)) => {
                            if ctx_start <= blk_end + 1 {
                                // Overlapping or adjacent: extend the current block
                                current = Some((blk_start, blk_first, ctx_end.max(blk_end)));
                            } else {
                                // Non-overlapping: emit finished block, start new one
                                let content = file_lines[blk_start..=blk_end].join("\n");
                                matches.push(json!({
                                    "file": entry.path().display().to_string(),
                                    "match_line": blk_first + 1,
                                    "start_line": blk_start + 1,
                                    "content": content,
                                }));
                                current = Some((ctx_start, i, ctx_end));
                            }
                        }
                    }

                    if total_match_count >= max {
                        hit_cap = true;
                        break;
                    }
                }

                // Emit the last in-flight block
                if let Some((blk_start, blk_first, blk_end)) = current {
                    let content = file_lines[blk_start..=blk_end].join("\n");
                    matches.push(json!({
                        "file": entry.path().display().to_string(),
                        "match_line": blk_first + 1,
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
                r["overflow"] = json!({
                    "shown": visible.len(),
                    "total": total,
                    "hint": "Many matches. Narrow the pattern or use a more specific path.",
                });
            }
            r
        } else {
            // Context mode: keep flat matches[], preserve legacy shape for format_grep
            let mut r =
                json!({ "matches": matches, "total": shown_count, "context_lines": context_lines });
            if hit_cap {
                r["overflow"] = json!({
                    "shown": shown_count,
                    "hint": format!(
                        "Showing first {} matches (cap hit). Narrow with a more specific pattern or path=<file>.",
                        shown_count
                    )
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
                 references(symbol='{name}') for direct callers, \
                 call_graph(symbol='{name}', direction='callers') for transitive blast radius."
            ));
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_grep(result))
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
    use crate::tools::file_group::{group_by_file, render_grouped};

    // Re-attach file to each item for group_by_file, then render grouped.
    let mut flat: Vec<Value> = vec![];
    for group in file_groups {
        let file = group["file"].as_str().unwrap_or("?");
        if let Some(items) = group["items"].as_array() {
            for item in items {
                let mut clone = item.clone();
                if let Some(obj) = clone.as_object_mut() {
                    obj.insert("file".to_string(), Value::String(file.to_string()));
                }
                flat.push(clone);
            }
        }
    }
    let groups = group_by_file(&flat);
    let noun = if total == 1 { "match" } else { "matches" };

    let rendered = render_grouped(&groups, total, files, noun, |item| {
        let line = item["line"].as_u64().unwrap_or(0);
        let content = item["content"].as_str().unwrap_or("").trim();
        format!("  {line:>5}: {content}")
    });
    out.push_str(&rendered);
}

fn format_search_context_mode(out: &mut String, matches: &[Value]) {
    let mut current_file: Option<&str> = None;

    for m in matches {
        let file = m["file"].as_str().unwrap_or("?");
        let start_line = m["start_line"].as_u64().unwrap_or(1);
        let content = m["content"].as_str().unwrap_or("");

        if current_file != Some(file) {
            out.push_str("\n  ");
            out.push_str(file);
            out.push('\n');
            current_file = Some(file);
        }

        for (i, line) in content.lines().enumerate() {
            let line_num = start_line + i as u64;
            out.push_str(&format!("  {:<4} {}\n", line_num, line));
        }
    }

    if out.ends_with('\n') {
        out.pop();
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
        }
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
}
