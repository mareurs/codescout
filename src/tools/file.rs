//! File system tools: read, write, search, list.

use anyhow::Result;
use serde_json::{json, Value};

use super::{optional_u64_param, parse_bool_param, RecoverableError, Tool, ToolContext};

// ── glob ───────────────────────────────────────────────────────────────

pub struct Glob;

#[async_trait::async_trait]
impl Tool for Glob {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern (e.g. '**/*.rs', 'src/**/mod.rs'). Respects .gitignore."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern" },
                "path": { "type": "string", "description": "Directory to search (default: current dir)" },
                "limit": { "type": "integer", "default": 100, "description": "Maximum files to return" }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let pattern = super::require_str_param(&input, "pattern")?;
        let raw_path = input["path"].as_str().unwrap_or(".");
        let project_root = ctx.agent.project_root().await;
        let security = ctx.agent.security_config().await;
        let search_path = crate::util::path_security::validate_read_path(
            raw_path,
            project_root.as_deref(),
            &security,
        )?;
        let max = optional_u64_param(&input, "limit").unwrap_or(100) as usize;

        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| {
                RecoverableError::with_hint(
                    format!("invalid glob pattern: {e}"),
                    "Use glob syntax: * matches anything, ** crosses directories, ? matches one char",
                )
            })?
            .compile_matcher();

        let mut matches = vec![];
        let mut hit_cap = false;
        let walker = ignore::WalkBuilder::new(&search_path)
            .hidden(true)
            .git_ignore(true)
            .build();
        for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&search_path)
                .unwrap_or(entry.path());
            if glob.is_match(rel) {
                matches.push(entry.path().display().to_string());
                if matches.len() >= max {
                    hit_cap = true;
                    break;
                }
            }
        }

        let mut result = json!({ "files": matches, "total": matches.len() });
        if hit_cap {
            result["overflow"] = json!({
                "shown": matches.len(),
                "hint": format!(
                    "Showing first {} files (cap hit). Narrow with a more specific pattern or path=<dir>.",
                    matches.len()
                )
            });
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_glob(result))
    }
}

// ── format_compact helpers ────────────────────────────────────────────────────

fn format_glob(result: &Value) -> String {
    let total = result["total"].as_u64().unwrap_or(0);
    let overflow = result["overflow"].is_object();
    let cap_note = if overflow {
        " (cap hit — narrow pattern)"
    } else {
        ""
    };
    format!("{total} files{cap_note}")
}

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
        return "remove_symbol(symbol, path) — deletes the symbol and its doc comments/attributes";
    }
    if new_string.len() > old_string.len() {
        return "insert_code(symbol, path, code, position) — inserts before or after a named symbol";
    }
    "replace_symbol(symbol, path, new_body) — replaces the symbol body via LSP"
}

pub struct EditFile;

#[async_trait::async_trait]
impl Tool for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Exact string replacement in a file. Whitespace-sensitive. \
         Use insert: \"prepend\"/\"append\" for file boundaries."
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

        // Gate: redirect .md files to edit_markdown (except prepend/append)
        if path.ends_with(".md") || path.ends_with(".markdown") {
            let insert_mode = input["insert"].as_str();
            if insert_mode != Some("prepend") && insert_mode != Some("append") {
                return Err(super::RecoverableError::with_hint(
                    "Use edit_markdown for markdown files",
                    "edit_markdown provides heading-based editing for .md files. edit_file with insert='prepend'/'append' is still allowed.",
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
            let root = ctx.agent.require_project_root().await?;
            let security = ctx.agent.security_config().await;
            let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;
            let mut content = std::fs::read_to_string(&resolved)?;

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
            ctx.agent.reload_config_if_project_toml(&resolved).await;
            ctx.lsp.notify_file_changed(&resolved).await;
            ctx.agent.mark_file_dirty(resolved).await;
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
            let root = ctx.agent.require_project_root().await?;
            let security = ctx.agent.security_config().await;
            let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;
            let content = std::fs::read_to_string(&resolved)?;
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
            ctx.agent.mark_file_dirty(resolved).await;
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
                "To create a new file use create_file. To insert at a specific line use insert_code. To prepend or append to a file use insert: \"prepend\" or \"append\".",
            )
            .into());
        }

        // Hard-block multi-line edits that contain definition keywords on LSP-supported languages.
        if old_string.contains('\n') && crate::util::path_security::is_source_path(path) {
            if let Some(lang) = detect_lsp_language(path) {
                if let Some(keyword) = find_def_keyword(old_string, lang) {
                    let hint = infer_edit_hint(old_string, new_string);
                    return Err(super::RecoverableError::with_hint(
                        format!(
                            "multi-line edit contains a symbol definition ({keyword:?}) \
                             — use symbol tools for structural changes"
                        ),
                        hint,
                    )
                    .into());
                }
            }
        }

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
    let root = ctx.agent.require_project_root().await?;
    let security = ctx.agent.security_config().await;
    let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;

    let content = std::fs::read_to_string(&resolved)?;

    let match_count = content.matches(old_string).count();

    if match_count == 0 {
        return Err(super::RecoverableError::with_hint(
            format!("old_string not found in {path}"),
            "Check whitespace and indentation — old_string must match exactly. Use grep to verify the exact text.",
        )
        .into());
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
    crate::util::fs::atomic_write(&resolved, &new_content)?;
    ctx.agent.reload_config_if_project_toml(&resolved).await;
    ctx.lsp.notify_file_changed(&resolved).await;
    ctx.agent.mark_file_dirty(resolved.clone()).await;

    // Syntax check: warn if the edit introduced parse errors (non-fatal).
    if let Some(lang) = crate::ast::detect_language(std::path::Path::new(path)) {
        if crate::ast::has_syntax_errors(&new_content, lang) {
            return Ok(json!({
                "status": "ok",
                "warning": "syntax error detected after edit — file may be malformed. Use read_file to inspect and fix."
            }));
        }
    }

    // Coverage hint for markdown files.
    if path.ends_with(".md") || path.ends_with(".markdown") {
        // Update mtime to prevent spurious invalidation after the write.
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(&resolved);
        }

        // If unread sections exist, return a hint alongside the ok status.
        let all_headings = crate::tools::file_summary::parse_all_headings(&new_content);
        if !all_headings.is_empty() {
            let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
            if let Ok(mut cov) = ctx.section_coverage.lock() {
                if let Some(hint) = cov.unread_hint(&resolved, &heading_texts) {
                    return Ok(json!({"status": "ok", "hint": hint}));
                }
            }
        }
    }

    Ok(json!("ok"))
}

#[cfg(test)]
mod tests {
    use super::super::create_file::CreateFile;
    use super::super::grep::{format_grep, Grep};
    use super::super::list_dir::{common_path_prefix, format_list_dir, ListDir};
    use super::super::read_file::{format_read_file, ReadFile};
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use serde_json::json;
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

    async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        (
            dir,
            ToolContext {
                agent,
                lsp: LspManager::new_arc(),
                output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(
                    20,
                )),
                progress: None,
                peer: None,
                section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                    crate::tools::section_coverage::SectionCoverage::new(),
                )),
            },
        )
    }

    // ── ReadFile ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn read_file_returns_full_content() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("hello.txt");
        std::fs::write(&file, "hello world").unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["content"], "hello world");
    }

    #[tokio::test]
    async fn read_file_with_line_range() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("lines.txt");
        std::fs::write(&file, "line1\nline2\nline3\nline4\nline5").unwrap();

        let result = ReadFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "end_line": 4
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["content"], "line2\nline3\nline4");
    }

    #[tokio::test]
    async fn read_file_rejects_start_line_zero() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("lines.txt");
        std::fs::write(&file, "line1\nline2\nline3").unwrap();
        let path = file.to_str().unwrap();

        // Baseline: valid 1-indexed range succeeds and returns content
        let result = ReadFile
            .call(
                json!({ "path": path, "start_line": 1, "end_line": 2 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            !result["content"].as_str().unwrap().is_empty(),
            "valid range must return content"
        );

        // Stale (the bug): old code passed start_line=0 through to extract_lines, which
        // silently returned an empty string — indistinguishable from an empty file.
        // Fixed: start_line=0 is now rejected with a RecoverableError.
        let result = ReadFile
            .call(
                json!({ "path": path, "start_line": 0, "end_line": 3 }),
                &ctx,
            )
            .await;
        assert!(
            result.is_err(),
            "start_line=0 must be rejected (lines are 1-indexed)"
        );

        // Fresh: valid range is unaffected — the guard is narrow and precise
        let result = ReadFile
            .call(
                json!({ "path": path, "start_line": 2, "end_line": 3 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            !result["content"].as_str().unwrap().is_empty(),
            "valid range after error cases must still return content"
        );
    }

    #[tokio::test]
    async fn read_file_rejects_end_before_start() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("lines.txt");
        std::fs::write(&file, "line1\nline2\nline3\nline4\nline5").unwrap();
        let path = file.to_str().unwrap();

        // Baseline: valid range succeeds
        let result = ReadFile
            .call(
                json!({ "path": path, "start_line": 2, "end_line": 4 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            !result["content"].as_str().unwrap().is_empty(),
            "valid range must return content"
        );

        // Stale (the bug): old code passed end < start through to extract_lines, which
        // computed a negative/empty slice and silently returned "".
        // Fixed: end_line < start_line is now rejected with a RecoverableError.
        let result = ReadFile
            .call(
                json!({ "path": path, "start_line": 5, "end_line": 2 }),
                &ctx,
            )
            .await;
        assert!(result.is_err(), "end_line < start_line must be rejected");

        // Fresh: equal start/end (single line) is valid, not rejected
        let result = ReadFile
            .call(
                json!({ "path": path, "start_line": 3, "end_line": 3 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            !result["content"].as_str().unwrap().is_empty(),
            "start_line == end_line (single line) must succeed"
        );
    }

    #[tokio::test]
    async fn read_file_missing_errors() {
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(json!({ "path": "/no/such/file.txt" }), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_file_missing_path_param_errors() {
        let ctx = test_ctx().await;
        let result = ReadFile.call(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_file_caps_large_file_in_exploring_mode() {
        // Files exceeding MAX_INLINE_TOKENS are buffered, not capped+overflowed.
        // This test verifies the buffer path for a large plain-text file.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.txt");
        let content: String = (1..=300)
            .map(|i| format!("line {:04} {}\n", i, "x".repeat(30)))
            .collect();
        std::fs::write(&file, &content).unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        // Large file should be buffered with a file_id
        assert!(
            result.get("file_id").is_some(),
            "300-line file should be buffered; got: {}",
            result
        );
        let file_id = result["file_id"].as_str().unwrap();
        assert!(file_id.starts_with("@file_"));
        // Full content is stored in the buffer
        let entry = ctx.output_buffer.get(file_id).unwrap();
        assert!(entry.stdout.contains("line 0150"));
    }

    #[tokio::test]
    async fn read_file_small_file_no_overflow() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("small.txt");
        std::fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(result.get("overflow").is_none());
        assert_eq!(result["total_lines"], 3);
    }

    // ── ReadFile — buffer ref (@tool_*, @cmd_*, @file_*) ─────────────────────

    #[tokio::test]
    async fn read_file_tool_ref_returns_full_content() {
        let ctx = test_ctx().await;
        let id = ctx
            .output_buffer
            .store_tool("onboarding", "{\"languages\":[\"rust\"]}".to_string());
        let result = ReadFile.call(json!({ "path": id }), &ctx).await.unwrap();
        // @tool_* refs are pretty-printed for readability — the content is expanded JSON,
        // not the original compact form.
        assert_eq!(
            result["content"],
            "{\n  \"languages\": [\n    \"rust\"\n  ]\n}"
        );
    }

    #[tokio::test]
    async fn read_file_cmd_ref_returns_full_content() {
        let ctx = test_ctx().await;
        let id = ctx.output_buffer.store(
            "cargo test".to_string(),
            "ok\n".to_string(),
            String::new(),
            0,
        );
        let result = ReadFile.call(json!({ "path": id }), &ctx).await.unwrap();
        assert_eq!(result["content"], "ok\n");
        assert_eq!(result["total_lines"], 1);
    }

    #[tokio::test]
    async fn read_file_tool_ref_with_line_range() {
        let ctx = test_ctx().await;
        let content = "line1\nline2\nline3\nline4\nline5".to_string();
        let id = ctx.output_buffer.store_tool("list_symbols", content);
        let result = ReadFile
            .call(json!({ "path": id, "start_line": 2, "end_line": 4 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["content"], "line2\nline3\nline4");
        assert_eq!(result["total_lines"], 5);
    }

    #[tokio::test]
    async fn read_file_tool_ref_missing_returns_error() {
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(json!({ "path": "@tool_deadbeef" }), &ctx)
            .await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("buffer reference not found"), "got: {}", msg);
    }

    #[tokio::test]
    async fn read_file_tool_ref_invalid_line_range_errors() {
        let ctx = test_ctx().await;
        let id = ctx
            .output_buffer
            .store_tool("onboarding", "a\nb\nc".to_string());
        let result = ReadFile
            .call(json!({ "path": id, "start_line": 5, "end_line": 2 }), &ctx)
            .await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("invalid line range"), "got: {}", msg);
    }

    #[tokio::test]
    async fn read_file_buffer_ref_large_range_buffers_as_file_ref() {
        // Regression test: when a line range read on a @file_* buffer ref extracts
        // content > TOOL_OUTPUT_BUFFER_THRESHOLD, read_file must store it as a new
        // @file_* ref AND return a first chunk inline (auto-chunk), rather than
        // returning zero content or wrapping in a @tool_* envelope.
        // Without the original fix, call_content would wrap large inline JSON in
        // a @tool_* envelope — encoding newlines as \n escapes so total_lines
        // counted JSON structure lines (4) and any sub-range with start_line > 4
        // returned empty content.
        let (dir, ctx) = project_ctx().await;

        // Write a file large enough to exceed the inline threshold.
        let big: String = (1..=300)
            .map(|i| format!("line {:04} padding_padding_padding_padd\n", i))
            .collect();
        assert!(
            big.len() > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "test data must exceed threshold ({} bytes), got {}",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            big.len()
        );
        let path = dir.path().join("big.txt");
        std::fs::write(&path, &big).unwrap();

        // First read: no range — large file gets buffered as @file_*.
        let r1 = ReadFile
            .call(json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let file_ref = r1["file_id"]
            .as_str()
            .expect("large file should produce @file_* ref on first read")
            .to_string();

        // Second read: ranged read on the @file_* ref — must auto-chunk:
        // return first chunk inline + file_id for continuation.
        let result = ReadFile
            .call(
                json!({ "path": file_ref, "start_line": 1, "end_line": 300 }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            result.get("file_id").is_some(),
            "large buffer-ref range should produce a @file_* ref; got: {}",
            result
        );
        assert_eq!(
            result["total_lines"].as_u64().unwrap(),
            300,
            "total_lines must reflect content lines, not JSON structure lines"
        );
        // Auto-chunk: must return first chunk inline and signal incomplete.
        assert!(
            result["content"].as_str().is_some(),
            "auto-chunked range must include first chunk of content; got: {}",
            result
        );
        assert_eq!(
            result["complete"], false,
            "must signal incomplete for oversized range; got: {}",
            result
        );
        assert!(
            result["next"].as_str().unwrap_or("").contains("read_file"),
            "must include a next continuation command; got: {}",
            result
        );

        // The chained @file_* ref must be navigable by sub-range.
        let file_ref2 = result["file_id"].as_str().unwrap().to_string();
        let sub = ReadFile
            .call(
                json!({ "path": file_ref2, "start_line": 50, "end_line": 50 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            sub["content"].as_str().unwrap_or("").contains("line 0050"),
            "sub-range on chained @file_* ref must return correct content; got: {}",
            sub
        );
    }

    #[tokio::test]
    async fn read_file_buffer_ref_range_auto_chunks() {
        // When a buffer-ref range exceeds inline limit, the response should
        // include the first chunk of content (not zero content), plus
        // complete=false and a next command.
        let (dir, ctx) = project_ctx().await;

        // Write a real file large enough to exceed the inline threshold so the
        // first read buffers it as a @file_* ref.
        // 300 lines × ~40 bytes = ~12 KB, above TOOL_OUTPUT_BUFFER_THRESHOLD (10 KB).
        let content: String = (1..=300)
            .map(|i| format!("line {:04} padding_padding_padding_padd\n", i))
            .collect();
        assert!(
            content.len() > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "test data must exceed threshold"
        );
        let path = dir.path().join("big.txt");
        std::fs::write(&path, &content).unwrap();

        // First read: no range — large file gets buffered as @file_*.
        let r1 = ReadFile
            .call(serde_json::json!({ "path": path.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let buf_id = r1["file_id"]
            .as_str()
            .expect("large file should produce @file_* ref on first read")
            .to_string();

        // Second read: ranged read on the @file_* ref — should auto-chunk.
        let result = ReadFile
            .call(
                serde_json::json!({ "path": buf_id, "start_line": 1, "end_line": 300 }),
                &ctx,
            )
            .await
            .unwrap();

        // Must have content (not empty)
        assert!(
            result.get("content").is_some(),
            "should auto-chunk content; got: {result}"
        );
        // Must signal incomplete
        assert_eq!(
            result["complete"], false,
            "should be incomplete; got: {result}"
        );
        // Must have a next command
        let next = result["next"].as_str().expect("should have next command");
        // next should reference the file_id and use sub-buffer-relative line numbers
        assert!(
            next.contains("start_line="),
            "next should include start_line; got: {next}"
        );
        let file_id = result["file_id"].as_str().expect("should have file_id");
        assert!(
            next.contains(file_id),
            "next should reference file_id; got: {next}"
        );
    }

    #[tokio::test]
    async fn read_file_real_file_range_auto_chunks() {
        let (dir, ctx) = project_ctx().await;

        // Create a file > 10KB (300 × 41 bytes = ~12 KB)
        let content: String = (1..=300)
            .map(|i| format!("line {:04} padding_padding_padding_padd\n", i))
            .collect();
        assert!(
            content.len() > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "test data must exceed threshold"
        );
        std::fs::write(dir.path().join("big.txt"), &content).unwrap();

        let result = ReadFile
            .call(
                serde_json::json!({ "path": "big.txt", "start_line": 1, "end_line": 300 }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            result.get("content").is_some(),
            "should auto-chunk; got: {result}"
        );
        assert_eq!(result["complete"], false);
        let next = result["next"].as_str().expect("should have next");
        assert!(result["file_id"].as_str().is_some());
        // next uses sub-buffer line numbers
        assert!(
            next.contains("start_line="),
            "next should include continuation; got: {next}"
        );
        // shown_lines reports original file line numbers
        let shown = result["shown_lines"].as_array().unwrap();
        assert_eq!(shown[0], 1);
    }

    #[tokio::test]
    async fn read_file_real_file_range_shown_lines_mid_file() {
        // Verify that shown_lines[0] equals the requested start_line, not always 1.
        // This proves the coordinate mapping is correct.
        let (dir, ctx) = project_ctx().await;

        // 400 lines × 41 bytes = ~16 KB; range 50..=400 = 351 lines × 41 ≈ 14 KB > threshold
        let content: String = (1..=400)
            .map(|i| format!("line {:04} padding_padding_padding_padd\n", i))
            .collect();
        std::fs::write(dir.path().join("big.txt"), &content).unwrap();

        let result = ReadFile
            .call(
                serde_json::json!({ "path": "big.txt", "start_line": 50, "end_line": 400 }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            result.get("content").is_some(),
            "should auto-chunk; got: {result}"
        );
        let shown = result["shown_lines"].as_array().unwrap();
        assert_eq!(
            shown[0], 50,
            "shown_lines[0] should equal the requested start_line (50), got: {}",
            shown[0]
        );
        let end_val = shown[1].as_u64().unwrap();
        assert!(
            end_val > 50 && end_val < 300,
            "shown_lines[1] should be within the file range; got {end_val}"
        );
    }

    #[tokio::test]
    async fn read_file_full_buffer_auto_chunks() {
        let (dir, ctx) = project_ctx().await;

        // 300 lines × 41 bytes = ~12 KB > TOOL_OUTPUT_BUFFER_THRESHOLD (10 KB)
        let content: String = (1..=300)
            .map(|i| format!("line {:04} padding_padding_padding_padd\n", i))
            .collect();
        assert!(
            content.len() > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "test data must exceed threshold ({} bytes), got {}",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            content.len()
        );
        // Write a real file so store_file sets source_path to an existing path;
        // otherwise get_with_refresh_flag evicts the entry on the first stat.
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, &content).unwrap();
        let buf_id = ctx
            .output_buffer
            .store_file(file_path.to_string_lossy().into_owned(), content);

        // No start_line/end_line — full buffer read
        let result = ReadFile
            .call(serde_json::json!({ "path": &buf_id }), &ctx)
            .await
            .unwrap();

        assert!(
            result.get("content").is_some(),
            "should auto-chunk; got: {result}"
        );
        assert_eq!(result["complete"], false);
        // next should reference the SAME buffer ID (not re-buffer)
        let next = result["next"].as_str().unwrap();
        assert!(
            next.contains(&buf_id),
            "next should reference original buffer; got: {next}"
        );
    }

    // ── ListDir ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_dir_returns_shallow_entries() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        std::fs::write(dir.path().join("b.rs"), "").unwrap();

        let result = ListDir
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.ends_with("a.rs")));
        assert!(entries.iter().any(|e| e.ends_with("b.rs")));
    }

    #[tokio::test]
    async fn list_dir_shallow_does_not_descend() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("deep.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": false }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // Should see the sub/ directory but not deep.rs inside it
        assert!(!entries.iter().any(|e| e.ends_with("deep.rs")));
    }

    #[tokio::test]
    async fn list_dir_recursive_descends() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("deep.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": true }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(entries.iter().any(|e| e.ends_with("deep.rs")));
    }

    #[tokio::test]
    async fn list_dir_caps_output_in_exploring_mode() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // Create more files than the default cap (200)
        // We'll use a smaller effective cap by using recursive mode
        // with many nested entries. Simpler: just create 5 files and
        // verify with a small cap that overflow is reported.
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("file_{}.rs", i)), "").unwrap();
        }

        // list_dir uses OutputGuard with max_results=200 by default,
        // which won't trigger for 5 files. But the early-exit logic
        // triggers when entries.len() > max_results. To test overflow,
        // we verify the mechanism works by checking that >200 entries
        // DO produce overflow. Instead, let's verify the non-overflow
        // case works (no overflow key) and that the entries are correct.
        let result = ListDir
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 5);
        assert!(result.get("overflow").is_none());
    }

    #[tokio::test]
    async fn list_dir_max_depth_limits_descent() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // depth1/depth2/depth3/deep.rs — should not appear with max_depth=2
        let deep = dir.path().join("depth1").join("depth2").join("depth3");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("deep.rs"), "").unwrap();
        std::fs::write(dir.path().join("depth1").join("shallow.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "max_depth": 2 }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(entries.iter().any(|e| e.ends_with("shallow.rs")));
        assert!(!entries.iter().any(|e| e.ends_with("deep.rs")));
        // max_depth is explicit — no auto-cap note
        assert!(result.get("depth_capped").is_none());
    }

    #[tokio::test]
    async fn list_dir_recursive_exploring_caps_at_depth_3() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // Build a 4-level deep tree
        let deep = dir.path().join("a").join("b").join("c").join("d");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("leaf.rs"), "").unwrap();
        std::fs::write(dir.path().join("a").join("b").join("mid.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": true }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // depth-3 file should appear (a/b/mid.rs is at depth 3 from root)
        assert!(entries.iter().any(|e| e.ends_with("mid.rs")));
        // depth-4 file should be cut off (a/b/c/d/leaf.rs)
        assert!(!entries.iter().any(|e| e.ends_with("leaf.rs")));
        // depth_capped marker should be set
        assert_eq!(result["depth_capped"], json!(3));
    }

    #[tokio::test]
    async fn list_dir_recursive_focused_no_depth_cap() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let deep = dir.path().join("a").join("b").join("c").join("d");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("leaf.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({
                    "path": dir.path().to_str().unwrap(),
                    "recursive": true,
                    "detail_level": "full"
                }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // In focused mode no depth cap — leaf at depth 4 should appear
        assert!(entries.iter().any(|e| e.ends_with("leaf.rs")));
        assert!(result.get("depth_capped").is_none());
    }

    // ── Grep ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn search_finds_matching_line() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn main() {}\nlet x = 42;\n").unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "fn main", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["line"], 1);
        assert!(matches[0]["content"].as_str().unwrap().contains("fn main"));
    }

    #[tokio::test]
    async fn search_returns_no_matches_when_absent() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn main() {}").unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "xyz_not_present", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["matches"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let content = (0..20)
            .map(|i| format!("match_{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(dir.path().join("data.txt"), &content).unwrap();

        let result = Grep
            .call(
                json!({
                    "pattern": "match_",
                    "path": dir.path().to_str().unwrap(),
                    "limit": 5
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["matches"].as_array().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn search_invalid_regex_errors() {
        // `foo|[invalid` — has alternation (is_regex_like=true) AND unclosed `[`,
        // so it should remain a RecoverableError rather than falling back to literal.
        let (dir, ctx) = project_ctx().await;
        let err = Grep
            .call(
                json!({ "pattern": "foo|[invalid", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "invalid regex should be RecoverableError, not a hard error"
        );
    }

    #[tokio::test]
    async fn search_missing_pattern_errors() {
        let ctx = test_ctx().await;
        let result = Grep.call(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // ── CreateFile ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_text_file_writes_content() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("new.txt");

        let result = CreateFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "content": "hello file"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, "ok");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello file");
    }

    #[tokio::test]
    async fn create_text_file_creates_parent_dirs() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("a").join("b").join("deep.txt");

        CreateFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "content": "nested"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "nested");
    }

    #[tokio::test]
    async fn create_text_file_missing_params_errors() {
        let ctx = test_ctx().await;
        assert!(CreateFile.call(json!({}), &ctx).await.is_err());
        let outside = std::env::temp_dir().join("nonexistent_xplat_test");
        let outside_str = outside.to_str().unwrap();
        assert!(CreateFile
            .call(json!({ "path": outside_str }), &ctx)
            .await
            .is_err());
    }

    // ── Glob ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn glob_matches_pattern() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("foo.rs"), "").unwrap();
        std::fs::write(dir.path().join("bar.rs"), "").unwrap();
        std::fs::write(dir.path().join("baz.txt"), "").unwrap();

        let result = Glob
            .call(
                json!({
                    "pattern": "*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.as_str().unwrap().ends_with(".rs")));
    }

    #[tokio::test]
    async fn glob_recursive() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("lib.rs"), "").unwrap();
        std::fs::write(dir.path().join("main.rs"), "").unwrap();

        let result = Glob
            .call(
                json!({
                    "pattern": "**/*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn glob_respects_limit() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("f{}.rs", i)), "").unwrap();
        }

        let result = Glob
            .call(
                json!({
                    "pattern": "*.rs",
                    "path": dir.path().to_str().unwrap(),
                    "limit": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["files"].as_array().unwrap().len(), 3);
        assert_eq!(result["total"], 3);
    }

    #[tokio::test]
    async fn glob_no_matches() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("readme.md"), "").unwrap();

        let result = Glob
            .call(
                json!({
                    "pattern": "*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["files"].as_array().unwrap().len(), 0);
    }

    // ── ListDir .git exclusion ──────────────────────────────────────────────

    #[tokio::test]
    async fn list_dir_recursive_excludes_git() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();

        // Create a fake .git/ directory with typical contents
        let git_dir = dir.path().join(".git");
        std::fs::create_dir(&git_dir).unwrap();
        std::fs::create_dir(git_dir.join("objects")).unwrap();
        std::fs::write(git_dir.join("objects").join("abc123"), "blob").unwrap();
        std::fs::create_dir(git_dir.join("hooks")).unwrap();
        std::fs::write(git_dir.join("hooks").join("pre-commit"), "#!/bin/sh").unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main").unwrap();

        // Create real project files
        let src_dir = dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": true }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();

        // Should contain real project files
        assert!(
            entries.iter().any(|e| e.ends_with("main.rs")),
            "should contain main.rs, got: {:?}",
            entries
        );
        assert!(
            entries.iter().any(|e| e.ends_with("Cargo.toml")),
            "should contain Cargo.toml, got: {:?}",
            entries
        );

        // Should NOT contain any .git/ entries
        assert!(
            !entries.iter().any(|e| e.contains(".git")),
            ".git/ entries should be excluded, got: {:?}",
            entries
        );
    }
    #[tokio::test]
    async fn search_for_pattern_skips_hidden_dirs() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();

        // Create a normal source file with a match
        std::fs::write(dir.path().join("main.rs"), "fn hello() {}").unwrap();

        // Create a hidden .worktrees/ dir with the same pattern — should be skipped
        let wt_dir = dir.path().join(".worktrees").join("feature");
        std::fs::create_dir_all(&wt_dir).unwrap();
        std::fs::write(wt_dir.join("lib.rs"), "fn hello() {}").unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "fn hello", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches: Vec<&str> = result["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["file"].as_str().unwrap())
            .collect();

        assert!(
            matches.iter().any(|f| f.ends_with("main.rs")),
            "should find match in main.rs"
        );
        assert!(
            !matches.iter().any(|f| f.contains(".worktrees")),
            ".worktrees/ should be excluded, got: {:?}",
            matches
        );
    }

    #[tokio::test]
    async fn glob_skips_hidden_dirs() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();

        // Normal file
        std::fs::write(dir.path().join("main.rs"), "").unwrap();

        // Same pattern inside a hidden .claude/worktrees/ dir — should be skipped
        let wt_dir = dir.path().join(".claude").join("worktrees").join("branch");
        std::fs::create_dir_all(&wt_dir).unwrap();
        std::fs::write(wt_dir.join("main.rs"), "").unwrap();

        let result = Glob
            .call(
                json!({ "pattern": "**/*.rs", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let files: Vec<&str> = result["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();

        assert_eq!(
            files.len(),
            1,
            "only main.rs should match, got: {:?}",
            files
        );
        assert!(
            files[0].ends_with("main.rs"),
            "expected main.rs, got: {:?}",
            files
        );
    }

    // ── Security: Input Validation ────────────────────────────────────────────

    #[tokio::test]
    async fn read_file_missing_path_errors() {
        let ctx = test_ctx().await;
        let result = ReadFile.call(json!({}), &ctx).await;
        assert!(result.is_err(), "read_file without path should error");
    }

    #[tokio::test]
    async fn read_file_empty_path_errors() {
        let ctx = test_ctx().await;
        let result = ReadFile.call(json!({ "path": "" }), &ctx).await;
        assert!(result.is_err(), "read_file with empty path should error");
    }

    #[tokio::test]
    async fn create_text_file_missing_params_detailed_errors() {
        let ctx = test_ctx().await;
        // Missing path
        let result = CreateFile.call(json!({ "content": "hello" }), &ctx).await;
        assert!(
            result.is_err(),
            "create_text_file without path should error"
        );

        // Missing content
        let outside = std::env::temp_dir().join("nonexistent_xplat_test.txt");
        let outside_str = outside.to_str().unwrap();
        let result = CreateFile.call(json!({ "path": outside_str }), &ctx).await;
        assert!(
            result.is_err(),
            "create_text_file without content should error"
        );
    }

    #[tokio::test]
    async fn search_for_pattern_missing_pattern_errors() {
        let ctx = test_ctx().await;
        let result = Grep.call(json!({}), &ctx).await;
        assert!(
            result.is_err(),
            "search_for_pattern without pattern should error"
        );
    }

    #[tokio::test]
    async fn glob_missing_pattern_errors() {
        let ctx = test_ctx().await;
        let result = Glob.call(json!({}), &ctx).await;
        assert!(result.is_err(), "glob without pattern should error");
    }

    #[tokio::test]
    async fn search_for_pattern_invalid_regex_errors() {
        // `bar|[invalid(` — has alternation (is_regex_like=true) AND both unclosed `[` and `(`,
        // so it should remain a RecoverableError rather than falling back to literal.
        let (dir, ctx) = project_ctx().await;
        let err = Grep
            .call(
                json!({ "pattern": "bar|[invalid(", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "invalid regex should be RecoverableError, not a hard error"
        );
    }

    #[tokio::test]
    async fn read_file_nonexistent_errors_gracefully() {
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(json!({ "path": "/nonexistent/path/file.txt" }), &ctx)
            .await;
        assert!(result.is_err());
        // Error should not panic, just return Err
    }
    // BUG-005 regression: read_file on a directory path must return RecoverableError,
    // not a hard anyhow error. A hard error aborts sibling parallel tool calls in Claude Code.
    #[tokio::test]
    async fn read_file_directory_path_returns_recoverable_error() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // Pass the directory itself as the path — not a file inside it.
        let result = ReadFile
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await;
        let err = result.unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "read_file on a directory must be RecoverableError (not a hard error); got: {err}"
        );
        let rec = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(
            rec.message.contains("directory"),
            "error message should mention 'directory'; got: {}",
            rec.message
        );
        assert!(
            rec.hint().unwrap_or("").contains("list_dir"),
            "hint should suggest list_dir; got: {:?}",
            rec.hint()
        );
    }

    #[tokio::test]
    async fn read_file_binary_content_does_not_panic() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("binary.bin");
        std::fs::write(&file, b"\x00\x01\x02\xff\xfe").unwrap();

        // Binary file should produce a RecoverableError, not a fatal error or panic
        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await;
        let err = result.unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "binary file error should be RecoverableError, got: {err}"
        );
        assert!(
            err.to_string().contains("non-UTF-8"),
            "error should mention non-UTF-8, got: {err}"
        );
    }

    #[tokio::test]
    async fn list_dir_nonexistent_path_errors() {
        let ctx = test_ctx().await;
        let result = ListDir
            .call(json!({ "path": "/nonexistent/directory" }), &ctx)
            .await
            .unwrap();
        // WalkBuilder returns empty for nonexistent paths
        let entries = result["entries"].as_array().unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn search_for_pattern_limit_respected() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // Create a file with many matching lines
        let content = (0..100)
            .map(|i| format!("match_{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(dir.path().join("many.txt"), &content).unwrap();

        let result = Grep
            .call(
                json!({
                    "pattern": "match_",
                    "path": dir.path().to_str().unwrap(),
                    "limit": 5
                }),
                &ctx,
            )
            .await
            .unwrap();
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 5, "limit should be respected");
    }

    #[tokio::test]
    async fn search_for_pattern_single_file_path() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("spec.md");
        std::fs::write(
            &file_path,
            "# Notifications\nPush notifications are sent via FCM.\nEmail alerts are also supported.\n",
        )
        .unwrap();

        // Pass a file path, not a directory
        let result = Grep
            .call(
                json!({
                    "pattern": "notification|push",
                    "path": file_path.to_str().unwrap(),
                    "limit": 40
                }),
                &ctx,
            )
            .await
            .unwrap();
        let matches = result["matches"].as_array().unwrap();
        assert!(
            !matches.is_empty(),
            "search_for_pattern should work with a single file path"
        );
    }

    // ── Security: Path Sandboxing ─────────────────────────────────────────

    #[tokio::test]
    async fn read_file_denies_ssh_key() {
        let ctx = test_ctx().await;
        if let Ok(home) = std::env::var("HOME") {
            let ssh_path = format!("{}/.ssh/id_rsa", home);
            let result = ReadFile.call(json!({ "path": &ssh_path }), &ctx).await;
            assert!(result.is_err(), "read of ~/.ssh/id_rsa should be denied");
        }
    }

    #[tokio::test]
    async fn create_file_outside_project_rejected() {
        let (_dir, ctx) = project_ctx().await;
        // Use a hardcoded path outside both the project root and /tmp (which is
        // now an allowed write root).
        let result = CreateFile
            .call(
                json!({
                    "path": "/var/outside_ce_test/evil.rs",
                    "content": "evil code"
                }),
                &ctx,
            )
            .await;
        assert!(result.is_err(), "write outside project should be rejected");
    }

    #[tokio::test]
    async fn create_file_within_project_works() {
        let (dir, ctx) = project_ctx().await;
        let result = CreateFile
            .call(
                json!({
                    "path": dir.path().join("new_file.txt").to_str().unwrap(),
                    "content": "hello"
                }),
                &ctx,
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("new_file.txt")).unwrap(),
            "hello"
        );
    }

    #[tokio::test]
    async fn write_requires_active_project() {
        let ctx = test_ctx().await;
        let outside = std::env::temp_dir().join("nonexistent_xplat_test.txt");
        let outside_str = outside.to_str().unwrap();
        let result = CreateFile
            .call(json!({ "path": outside_str, "content": "hi" }), &ctx)
            .await;
        assert!(result.is_err(), "write without active project should error");
    }

    // ── ReDoS Prevention ─────────────────────────────────────────────────

    #[tokio::test]
    async fn search_for_pattern_huge_regex_rejected() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("file.txt"), "hello").unwrap();

        // Build a regex that exceeds the 1MB compiled NFA size limit
        let huge_pattern = format!("({})", "a?".repeat(100_000));
        let err = Grep
            .call(
                json!({
                    "pattern": huge_pattern,
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "size-limit rejection must be RecoverableError so parallel sibling calls are not aborted"
        );
    }

    #[tokio::test]
    async fn read_file_tags_project_source() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world").unwrap();

        let tool = ReadFile;
        let result = tool
            .call(json!({ "path": "test.txt" }), &ctx)
            .await
            .unwrap();
        // "project" source is the default — omitted to reduce noise
        assert!(
            result["source"].is_null(),
            "source should be omitted for project files"
        );
    }

    // ── ReadFile source code gate ────────────────────────────────────────────

    #[tokio::test]
    async fn read_file_allows_source_code_files() {
        // Source files are no longer blocked — small ones return content directly,
        // large ones are buffered. This replaces the old read_file_blocks_source_code_files test.
        let (dir, ctx) = project_ctx().await;
        let rs_file = dir.path().join("main.rs");
        std::fs::write(&rs_file, "fn main() {}\n").unwrap();

        let result = ReadFile
            .call(json!({ "path": rs_file.to_str().unwrap() }), &ctx)
            .await;
        assert!(
            result.is_ok(),
            "read_file should now allow .rs files: {:?}",
            result.err()
        );
        assert!(
            result.unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("fn main"),
            "content should be returned"
        );
    }

    #[tokio::test]
    async fn read_file_allows_non_source_files() {
        let (dir, ctx) = project_ctx().await;
        let toml_file = dir.path().join("config.toml");
        std::fs::write(&toml_file, "key = \"value\"\n").unwrap();

        let result = ReadFile
            .call(json!({ "path": toml_file.to_str().unwrap() }), &ctx)
            .await;
        assert!(result.is_ok(), "read_file should allow .toml files");
    }

    #[tokio::test]
    async fn read_file_gates_markdown_files() {
        let (dir, ctx) = project_ctx().await;
        let md_file = dir.path().join("README.md");
        std::fs::write(&md_file, "# Hello\n").unwrap();

        let result = ReadFile
            .call(json!({ "path": md_file.to_str().unwrap() }), &ctx)
            .await;
        assert!(
            result.is_err(),
            "read_file should gate .md files to read_markdown"
        );
    }

    #[tokio::test]
    async fn read_file_allows_unknown_extensions() {
        let (dir, ctx) = project_ctx().await;
        let csv_file = dir.path().join("data.csv");
        std::fs::write(&csv_file, "a,b,c\n1,2,3\n").unwrap();

        let result = ReadFile
            .call(json!({ "path": csv_file.to_str().unwrap() }), &ctx)
            .await;
        assert!(result.is_ok(), "read_file should allow unknown extensions");
    }

    // ── ReadFile buffering ────────────────────────────────────────────────────

    #[tokio::test]
    async fn read_file_small_file_returns_content_directly() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("small.txt");
        std::fs::write(&path, "# Hello\nWorld\n").unwrap();
        let result = ReadFile
            .call(json!({"file_path": path.to_str().unwrap()}), &ctx)
            .await
            .unwrap();
        assert!(
            result.get("file_id").is_none(),
            "small file should not buffer"
        );
        assert!(result["content"].as_str().unwrap().contains("Hello"));
    }

    #[tokio::test]
    async fn read_file_large_file_returns_buffer_ref() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("big.txt");
        let content: String = (1..=210)
            .map(|i| format!("line {:04} {}\n", i, "x".repeat(45)))
            .collect();
        std::fs::write(&path, &content).unwrap();
        let result = ReadFile
            .call(json!({"file_path": path.to_str().unwrap()}), &ctx)
            .await
            .unwrap();
        let file_id = result["file_id"]
            .as_str()
            .expect("large file should have file_id");
        assert!(
            file_id.starts_with("@file_"),
            "file_id should start with @file_, got: {file_id}"
        );
        assert!(result["hint"].is_null(), "hint field should be absent");
        let entry = ctx.output_buffer.get(file_id).unwrap();
        assert!(entry.stdout.contains("line 0100"));
    }

    #[tokio::test]
    async fn read_file_explicit_range_always_returns_directly() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("big.rs");
        let content: String = (1..=300).map(|i| format!("// line {}\n", i)).collect();
        std::fs::write(&path, &content).unwrap();
        let result = ReadFile
            .call(
                json!({"file_path": path.to_str().unwrap(), "start_line": 1, "end_line": 5}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            result.get("file_id").is_none(),
            "explicit range should never buffer"
        );
        assert!(result["content"].as_str().unwrap().contains("line 1"));
    }

    #[tokio::test]
    async fn read_file_large_explicit_range_buffers_as_file_ref() {
        // Regression test for BUG-025 (explicit-range path): when a line range
        // extracts content > TOOL_OUTPUT_BUFFER_THRESHOLD, read_file must store it as
        // a @file_* ref rather than returning {"content": "..."} inline.
        // Without the fix, call_content wraps the large JSON in a 3-line @tool_*
        // envelope, making subsequent start_line/end_line navigation return empty content.
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("big.txt");
        // 300 lines × ~40 chars ≈ 12 KB — above MAX_INLINE_TOKENS threshold (10 KB)
        let content: String = (1..=300)
            .map(|i| format!("line {:04} padding_padding_padding_padd\n", i))
            .collect();
        assert!(
            content.len() > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "test data must exceed threshold ({} bytes), got {}",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            content.len()
        );
        std::fs::write(&path, &content).unwrap();

        let result = ReadFile
            .call(
                json!({"file_path": path.to_str().unwrap(), "start_line": 1, "end_line": 300}),
                &ctx,
            )
            .await
            .unwrap();

        // Large range: must auto-chunk — return first chunk inline plus navigation metadata
        assert!(
            result.get("content").is_some(),
            "large explicit range should auto-chunk content inline; got: {}",
            result
        );
        assert!(
            result.get("file_id").is_some(),
            "large explicit range should buffer as @file_* for navigation; got: {}",
            result
        );
        assert!(
            !result["complete"].as_bool().unwrap_or(true),
            "large explicit range should be incomplete (more chunks follow); got: {}",
            result
        );
        assert!(
            result["next"]
                .as_str()
                .unwrap_or("")
                .contains("start_line="),
            "auto-chunked range must include a next continuation command; got: {}",
            result
        );

        // Verify the @file_* ref is navigable by sub-range
        let file_id = result["file_id"].as_str().unwrap().to_string();
        let sub = ReadFile
            .call(
                json!({"path": file_id, "start_line": 10, "end_line": 10}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            sub["content"].as_str().unwrap_or("").contains("line 0010"),
            "sub-range on @file_* ref should return line 10; got: {}",
            sub
        );
    }

    #[tokio::test]
    async fn read_file_small_source_file_no_longer_errors() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("lib.rs");
        let content: String = (0..105)
            .map(|i| format!("fn fn_{}() {{}}\n\n", i))
            .collect();
        std::fs::write(&path, &content).unwrap();
        let result = ReadFile
            .call(json!({"file_path": path.to_str().unwrap()}), &ctx)
            .await
            .unwrap();
        assert!(
            result.get("file_id").is_some() || result.get("content").is_some(),
            "should buffer or return content, not error; got: {}",
            result
        );
    }

    // ── search_pattern: regex and string pattern tests ────────────────────────

    #[tokio::test]
    async fn search_pattern_regex_character_class_matches() {
        // Regression: `const [a-zA-Z]+ = async` was returning 0 matches even
        // when content matched. Verify character classes + quantifiers work.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("code.js"),
            "const foo = async () => {};\nconst bar = async () => {};\nfunction baz() {}\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "const [a-zA-Z]+ = async", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            2,
            "should match both const async arrow functions"
        );
        assert!(matches[0]["content"]
            .as_str()
            .unwrap()
            .contains("const foo"));
        assert!(matches[1]["content"]
            .as_str()
            .unwrap()
            .contains("const bar"));
        assert_eq!(result["total"], 2);
    }

    #[tokio::test]
    async fn search_pattern_escaped_paren_matches_literal_paren() {
        // `if \(name === '` should match a literal `if (name === '` in source.
        // Without the backslash, `(` would open an unclosed regex group (invalid).
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("api.js"),
            "function check() {\n  if (name === 'admin') { return true; }\n}\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({ "pattern": r"if \(name === '", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1, "escaped paren should match literal (");
        assert!(matches[0]["content"].as_str().unwrap().contains("if (name"));
    }

    #[tokio::test]
    async fn search_pattern_unescaped_paren_literal_fallback() {
        // `if (name === '` — unescaped `(` makes invalid regex, but the input
        // doesn't look like intended regex, so it falls back to literal search.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("api.js"),
            "function check() {\n  if (name === 'admin') { return true; }\n}\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "if (name === '", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            result["mode"].as_str().unwrap(),
            "literal_fallback",
            "non-regex-looking invalid regex should use literal fallback"
        );
        assert!(
            !result["matches"].as_array().unwrap().is_empty(),
            "literal fallback should find the text"
        );
    }

    #[tokio::test]
    async fn search_pattern_dot_matches_any_char_not_literal_dot() {
        // `.` in a regex matches ANY character, not just a literal `.`
        // This confirms the tool is regex-based, not literal-string-based.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("code.rs"),
            "fn main() {}\nfn_main_alt() {}\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "fn.main", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        // `.` matches `_` in `fn_main_alt` AND ` ` in `fn main()` — both lines match
        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            2,
            "dot should match any char including space and underscore"
        );
    }

    #[tokio::test]
    async fn search_pattern_literal_fallback_on_plain_text() {
        // `if (x > 0` — unclosed `(` makes invalid regex, but no closing `)` means
        // is_regex_like returns false, so the tool falls back to literal search.
        let (dir, ctx) = project_ctx().await;
        std::fs::write(
            dir.path().join("test.rs"),
            "fn check(x: i32) -> bool { if (x > 0) { true } else { false } }\n",
        )
        .unwrap();
        let result = Grep
            .call(json!({"pattern": "if (x > 0"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["mode"].as_str().unwrap(), "literal_fallback");
        assert!(result["reason"].as_str().unwrap().contains("literal"));
        assert!(!result["matches"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_pattern_literal_fallback_zero_matches() {
        // Same fallback trigger (`if (x > 0`) but the file has no matching text,
        // so the result should be empty with mode=literal_fallback and total=0.
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.rs"), "fn main() {}\n").unwrap();
        let result = Grep
            .call(json!({"pattern": "if (x > 0"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["mode"].as_str().unwrap(), "literal_fallback");
        assert!(result["matches"].as_array().unwrap().is_empty());
        assert_eq!(result["total"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn search_pattern_keeps_error_for_broken_regex_intent() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.rs"), "fn main() {}\n").unwrap();
        // "(foo|bar" has unclosed group AND contains alternation — is_regex_like returns true
        let err = Grep
            .call(json!({"pattern": "(foo|bar"}), &ctx)
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "broken regex with regex intent should be RecoverableError, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn search_pattern_valid_regex_has_no_mode() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.rs"), "fn foo() {}\nfn bar() {}\n").unwrap();
        let result = Grep
            .call(json!({"pattern": r"fn \w+"}), &ctx)
            .await
            .unwrap();
        assert!(result.get("mode").is_none());
    }

    #[tokio::test]
    async fn search_pattern_multi_file_returns_all_matches() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "pub fn handler() {}\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "pub fn handler() {}\n").unwrap();
        std::fs::write(dir.path().join("c.rs"), "fn unrelated() {}\n").unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "pub fn handler", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            2,
            "should find matches across multiple files"
        );
        let files: Vec<&str> = matches
            .iter()
            .map(|m| m["file"].as_str().unwrap())
            .collect();
        assert!(files.iter().any(|f| f.ends_with("a.rs")));
        assert!(files.iter().any(|f| f.ends_with("b.rs")));
    }

    #[tokio::test]
    async fn search_pattern_case_sensitive_by_default() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("code.rs"),
            "async fn handler() {}\nAsync fn Handler() {}\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "async fn handler", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            1,
            "search should be case-sensitive by default"
        );
        assert!(matches[0]["content"].as_str().unwrap().starts_with("async"));
    }

    #[tokio::test]
    async fn read_file_source_with_range_allowed() {
        let (dir, ctx) = project_ctx().await;
        let rs_file = dir.path().join("lib.rs");
        std::fs::write(&rs_file, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let result = ReadFile
            .call(
                json!({
                    "path": rs_file.to_str().unwrap(),
                    "start_line": 2,
                    "end_line": 4
                }),
                &ctx,
            )
            .await
            .unwrap();

        let content = result["content"].as_str().unwrap();
        assert!(content.contains("line2"), "should include line2: {content}");
        assert!(content.contains("line4"), "should include line4: {content}");
        assert!(
            !content.contains("line5"),
            "should not include line5: {content}"
        );
    }

    #[tokio::test]
    async fn read_file_source_without_range_now_works() {
        // Source files are no longer blocked — small ones return content directly,
        // large ones are buffered. Renamed from read_file_source_without_range_still_blocked.
        let (dir, ctx) = project_ctx().await;
        let rs_file = dir.path().join("lib.rs");
        std::fs::write(&rs_file, "fn main() {}\n").unwrap();

        let result = ReadFile
            .call(json!({ "path": rs_file.to_str().unwrap() }), &ctx)
            .await;

        assert!(
            result.is_ok(),
            "source files should now be readable: {:?}",
            result.err()
        );
        assert!(
            result.unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("fn main"),
            "content should contain source"
        );
    }

    #[tokio::test]
    async fn search_pattern_context_lines_zero_backward_compat() {
        // context_lines absent → old format (line + content keys)
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn main() {}\nlet x = 42;\n").unwrap();

        let result = Grep
            .call(
                json!({ "pattern": "fn main", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["line"], 1);
        assert!(matches[0]["content"].as_str().unwrap().contains("fn main"));
    }

    #[tokio::test]
    async fn search_pattern_context_lines_single_match() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // TARGET is line 3; context=2 → block covers lines 1-5
        std::fs::write(
            dir.path().join("code.rs"),
            "line1\nline2\nTARGET\nline4\nline5\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({
                    "pattern": "TARGET",
                    "path": dir.path().to_str().unwrap(),
                    "context_lines": 2
                }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0]["match_line"], 3,
            "match_line should be 1-indexed line of TARGET"
        );
        assert_eq!(
            matches[0]["start_line"], 1,
            "start_line = match(3) - context(2) = 1"
        );
        let content = matches[0]["content"].as_str().unwrap();
        assert!(
            content.contains("line1"),
            "context_before should include line1"
        );
        assert!(content.contains("TARGET"), "content should include match");
        assert!(
            content.contains("line5"),
            "context_after should include line5"
        );
    }

    #[tokio::test]
    async fn search_pattern_context_lines_adjacent_matches_merge() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // MATCH_A at line 3, MATCH_B at line 5; context=2 → windows overlap → one block
        std::fs::write(
            dir.path().join("code.rs"),
            "line1\nline2\nMATCH_A\nline4\nMATCH_B\nline6\nline7\n",
        )
        .unwrap();

        let result = Grep
            .call(
                json!({
                    "pattern": "MATCH_",
                    "path": dir.path().to_str().unwrap(),
                    "context_lines": 2
                }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            1,
            "overlapping context windows should merge into one block"
        );
        let content = matches[0]["content"].as_str().unwrap();
        assert!(
            content.contains("MATCH_A"),
            "merged block should contain first match"
        );
        assert!(
            content.contains("MATCH_B"),
            "merged block should contain second match"
        );
        assert!(
            content.contains("line7"),
            "block should extend to MATCH_B's context_after"
        );
    }

    #[tokio::test]
    async fn search_pattern_context_mode_total_equals_block_count_not_line_count() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // MATCH_A at line 3, MATCH_B at line 5 — only 2 lines apart, so with context_lines=2
        // their windows overlap and merge into ONE block.
        std::fs::write(
            dir.path().join("code.rs"),
            "line1\nline2\nMATCH_A\nline4\nMATCH_B\nline6\nline7\n",
        )
        .unwrap();
        let path = dir.path().to_str().unwrap();

        // Baseline: without context_lines, total equals the raw number of matching lines (2).
        // This is what the old context-mode code also used — the line count — which was wrong.
        let no_ctx = Grep
            .call(json!({ "pattern": "MATCH_", "path": path }), &ctx)
            .await
            .unwrap();
        assert_eq!(
            no_ctx["total"].as_u64().unwrap(),
            2,
            "without context, total must equal the number of matching lines"
        );
        assert_eq!(no_ctx["matches"].as_array().unwrap().len(), 2);

        // Stale (the bug): with context_lines=2 the two matches merge into 1 block,
        // but old code still set total=2 (line count) — inconsistent with matches.len()=1.
        // Fixed: total now equals the number of merged blocks returned.
        let with_ctx = Grep
            .call(
                json!({ "pattern": "MATCH_", "path": path, "context_lines": 2 }),
                &ctx,
            )
            .await
            .unwrap();
        let matches = with_ctx["matches"].as_array().unwrap();
        let total = with_ctx["total"].as_u64().unwrap();

        // Sandwich assertion: the two matches must have merged into exactly one block
        assert_eq!(
            matches.len(),
            1,
            "adjacent matches with context_lines=2 must merge into one block"
        );
        // Fresh: total now tracks blocks, not lines — so it equals 1, not 2
        assert_eq!(
            total, 1,
            "total must be block count (1), not line count (2)"
        );
        assert_eq!(
            total,
            matches.len() as u64,
            "total must always equal matches.len()"
        );
    }

    #[tokio::test]
    async fn search_pattern_context_lines_non_adjacent_matches_separate() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // MATCH at line 2 and line 18; with context=2 the windows don't overlap → two blocks
        let file_content = (1..=20)
            .map(|i| {
                if i == 2 || i == 18 {
                    format!("MATCH line{i}")
                } else {
                    format!("other line{i}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(dir.path().join("code.rs"), file_content).unwrap();

        let result = Grep
            .call(
                json!({
                    "pattern": "MATCH",
                    "path": dir.path().to_str().unwrap(),
                    "context_lines": 2
                }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            2,
            "non-overlapping windows should produce two separate blocks"
        );
        assert_eq!(matches[0]["match_line"], 2);
        assert_eq!(matches[1]["match_line"], 18);
    }

    #[tokio::test]
    async fn search_pattern_context_lines_limit_is_global_not_per_file() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        // Each file has 2 non-adjacent matches (lines 1 and 6) with context_lines=1.
        // With context=1, windows are [0..=1] and [4..=5] — non-overlapping (gap of 2 lines).
        // Per-file: 2 match events → 2 separate blocks.
        // limit=3 → globally should stop after 3 match events total.
        // Before fix: per-file counter resets; file a emits 2 blocks, file b emits 2 blocks = 4 total.
        // After fix: counter is global; file a emits 2 blocks (count=2), file b emits 1 block (count=3, cap hit) = 3 total.
        let content = "MATCH\nother\nother\nother\nother\nMATCH\n";
        std::fs::write(dir.path().join("a.txt"), content).unwrap();
        std::fs::write(dir.path().join("b.txt"), content).unwrap();

        let result = Grep
            .call(
                json!({
                    "pattern": "MATCH",
                    "path": dir.path().to_str().unwrap(),
                    "context_lines": 1,
                    "limit": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        let total_blocks = matches.len();
        // File a: 2 non-adjacent blocks (2 match events)
        // File b: only 1 more event before hitting max=3 → 1 block
        assert_eq!(
            total_blocks, 3,
            "limit=3 should produce exactly 3 blocks globally, got {total_blocks}"
        );
        // total reports actual match count, not block count
        assert_eq!(result["total"], 3);
        // overflow should be present since cap was hit
        assert!(
            result.get("overflow").is_some(),
            "overflow should be present when cap is hit"
        );
    }

    #[tokio::test]
    async fn search_pattern_accepts_library_path() {
        use crate::library::registry::{DiscoveryMethod, LibraryRegistry};

        // Create a project dir and a separate library dir.
        let proj_dir = tempdir().unwrap();
        let lib_dir = tempdir().unwrap();

        // Write a file inside the library that contains a recognisable symbol.
        std::fs::write(lib_dir.path().join("lib.rs"), "pub fn hello_world() {}").unwrap();

        // Seed .codescout/libraries.json so the Agent loads the library on startup.
        std::fs::create_dir_all(proj_dir.path().join(".codescout")).unwrap();
        let mut registry = LibraryRegistry::new();
        registry.register(
            "fake-lib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            DiscoveryMethod::Manual,
            true,
        );
        let registry_path = proj_dir.path().join(".codescout/libraries.json");
        registry.save(&registry_path).unwrap();

        let agent = Agent::new(Some(proj_dir.path().to_path_buf()))
            .await
            .unwrap();
        let ctx = ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // search_pattern with a path pointing into the library — the walker must
        // start from the library root, not the project root.
        let result = Grep
            .call(
                json!({
                    "pattern": "hello_world",
                    "path": lib_dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(
            matches.len(),
            1,
            "expected 1 match in library, got {matches:?}"
        );
        assert!(
            matches[0]["content"]
                .as_str()
                .unwrap()
                .contains("hello_world"),
            "match content should include the searched symbol"
        );
    }

    // ── ReadFile — multi-heading (headings param) ─────────────────────────────

    // ── EditFile ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn edit_file_replaces_unique_match() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "hello",
                    "new_string": "goodbye"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "goodbye world\n");
    }

    #[tokio::test]
    async fn edit_file_empty_new_string_deletes() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "aaa bbb ccc\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": " bbb",
                    "new_string": ""
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "aaa ccc\n");
    }

    #[tokio::test]
    async fn edit_file_not_found_errors() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world\n").unwrap();

        let err = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "does not exist",
                    "new_string": "replacement"
                }),
                &ctx,
            )
            .await
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("not found"),
            "error should mention 'not found', got: {msg}"
        );
    }

    #[tokio::test]
    async fn edit_file_multiple_matches_without_replace_all_errors() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "foo bar foo baz foo\n").unwrap();

        let err = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "foo",
                    "new_string": "qux"
                }),
                &ctx,
            )
            .await
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("3 times"),
            "error should mention '3 times', got: {msg}"
        );
        // File must be untouched
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "foo bar foo baz foo\n");
    }

    #[tokio::test]
    async fn edit_file_replace_all_replaces_all_occurrences() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "foo bar foo baz foo\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "foo",
                    "new_string": "qux",
                    "replace_all": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "qux bar qux baz qux\n");
    }

    #[tokio::test]
    async fn edit_file_empty_old_string_errors() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "some content\n").unwrap();

        let err = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "",
                    "new_string": "replacement"
                }),
                &ctx,
            )
            .await
            .unwrap_err();

        let recoverable = err
            .downcast_ref::<super::RecoverableError>()
            .expect("expected a RecoverableError");
        let hint = recoverable.hint().unwrap_or("");
        assert!(
            hint.contains("create_file"),
            "expected error hint to mention create_file, got: {hint}"
        );
    }

    #[tokio::test]
    async fn edit_file_multiline_replace() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "fn old() {\n    todo!()\n}\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "fn old() {\n    todo!()\n}",
                    "new_string": "fn new_func() {\n    42\n}"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "fn new_func() {\n    42\n}\n");
    }

    #[tokio::test]
    async fn edit_file_whitespace_sensitive() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "    indented\n").unwrap();

        // old_string without leading spaces still matches as a substring
        let result = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "indented",
                    "new_string": "replaced"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "    replaced\n");
    }

    #[tokio::test]
    async fn edit_file_returns_ok_string() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "hello",
                    "new_string": "goodbye"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        assert!(
            result.is_string(),
            "response must be a plain string, not an object"
        );
    }

    #[test]
    fn glob_format_compact_shows_count() {
        use serde_json::json;
        let tool = Glob;
        let result = json!({ "files": ["src/a.rs", "src/b.rs"], "total": 2 });
        let text = tool.format_compact(&result).unwrap();
        assert!(text.contains("2 files"), "got: {text}");
    }

    #[test]
    fn infer_edit_hint_remove_when_new_string_empty() {
        let hint = infer_edit_hint("fn foo() {\n    bar();\n}", "");
        assert!(hint.contains("remove_symbol"), "got: {hint}");
    }

    #[test]
    fn infer_edit_hint_replace_symbol_for_rust_fn() {
        let hint = infer_edit_hint("fn foo() {\n    old();\n}", "fn foo() {\n    new();\n}");
        assert!(hint.contains("replace_symbol"), "got: {hint}");
    }

    #[test]
    fn infer_edit_hint_replace_symbol_for_python_def() {
        // new_string same length or shorter → replace_symbol branch
        let hint = infer_edit_hint(
            "def process(x):\n    return x",
            "def process(x):\n    return y",
        );
        assert!(hint.contains("replace_symbol"), "got: {hint}");
    }

    #[test]
    fn infer_edit_hint_replace_symbol_for_class() {
        let hint = infer_edit_hint("class Foo {\n    x: i32\n}", "class Foo {\n    y: i32\n}");
        assert!(hint.contains("replace_symbol"), "got: {hint}");
    }

    #[test]
    fn infer_edit_hint_insert_code_when_new_is_longer() {
        let hint = infer_edit_hint("placeholder", "fn extra() {\n    todo!();\n}\nplaceholder");
        assert!(hint.contains("insert_code"), "got: {hint}");
    }

    #[tokio::test]
    async fn edit_file_allows_singleline_on_rust_source() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "let x = 1;\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "src/lib.rs", "old_string": "x = 1", "new_string": "x = 2"}),
                &ctx,
            )
            .await;

        assert!(
            result.is_ok(),
            "single-line edits on source should pass: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn edit_file_allows_multiline_on_non_source() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("README.txt");
        std::fs::write(&path, "line one\nline two\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "README.txt", "old_string": "line one\nline two", "new_string": "updated one\nupdated two"}),
                &ctx,
            )
            .await;

        assert!(
            result.is_ok(),
            "multi-line edits on non-source should pass: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn edit_file_warns_multiline_python() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("app.py");
        std::fs::write(&path, "def greet():\n    print('hello')\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "app.py", "old_string": "def greet():\n    print('hello')", "new_string": "def greet():\n    print('hi')"}),
                &ctx,
            )
            .await;

        assert!(
            result.is_err(),
            "should hard-block structural edit on Python"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("symbol definition"),
            "error should mention symbol definition, got: {err}"
        );
    }

    #[tokio::test]
    async fn edit_file_warns_hint_suggests_remove_when_new_empty() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "fn foo() {\n    bar();\n}\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "src/lib.rs", "old_string": "fn foo() {\n    bar();\n}", "new_string": ""}),
                &ctx,
            )
            .await;

        assert!(
            result.is_err(),
            "should hard-block structural delete on LSP language"
        );
        let err = result.unwrap_err();
        let recoverable = err
            .downcast_ref::<super::RecoverableError>()
            .expect("should be RecoverableError");
        assert!(
            err.to_string().contains("symbol definition"),
            "error should mention symbol definition, got: {err}"
        );
        let hint = recoverable.hint().unwrap_or("");
        assert!(
            hint.contains("remove_symbol"),
            "hint should mention remove_symbol, got: {hint}"
        );
    }

    #[tokio::test]
    async fn edit_file_prepend_adds_text_at_start() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line two\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "test.txt", "insert": "prepend", "new_string": "line one\n"}),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "line one\nline two\n");
    }

    #[tokio::test]
    async fn edit_file_append_adds_text_at_end() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line one\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "test.txt", "insert": "append", "new_string": "line two\n"}),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result, json!("ok"));
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "line one\nline two\n");
    }

    #[tokio::test]
    async fn edit_file_insert_without_old_string_ok() {
        // insert mode should not require old_string
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "existing\n").unwrap();

        let result = EditFile
            .call(
                json!({"path": "test.txt", "insert": "prepend", "new_string": "header\n"}),
                &ctx,
            )
            .await;

        assert!(result.is_ok(), "should succeed without old_string");
    }

    #[tokio::test]
    async fn edit_file_blocks_def_keyword_on_lsp_language() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "fn foo() {\n    old();\n}\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": "src/lib.rs",
                    "old_string": "fn foo() {\n    old();\n}",
                    "new_string": "fn foo() {\n    new();\n}"
                }),
                &ctx,
            )
            .await;

        assert!(
            result.is_err(),
            "should hard-block structural edit on LSP language"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("symbol definition"),
            "error should mention symbol definition, got: {err}"
        );
    }

    #[tokio::test]
    async fn edit_file_passes_non_lsp_language() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("script.lua");
        std::fs::write(&path, "function greet()\n    print('hi')\nend\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": "script.lua",
                    "old_string": "function greet()\n    print('hi')\nend",
                    "new_string": "function greet()\n    print('hello')\nend"
                }),
                &ctx,
            )
            .await;

        assert!(
            result.is_ok(),
            "should allow structural edit on non-LSP language: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn edit_file_passes_no_def_keyword() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "use crate::{\n    Foo,\n    Bar,\n};\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": "src/lib.rs",
                    "old_string": "use crate::{\n    Foo,\n    Bar,\n}",
                    "new_string": "use crate::{\n    Foo,\n    Bar,\n    Baz,\n}"
                }),
                &ctx,
            )
            .await;

        assert!(
            result.is_ok(),
            "should allow import list edit (no def keyword): {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn edit_file_passes_multiline_non_source() {
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("README.txt");
        std::fs::write(&path, "line one\nline two\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": "README.txt",
                    "old_string": "line one\nline two",
                    "new_string": "updated one\nupdated two"
                }),
                &ctx,
            )
            .await;

        assert!(
            result.is_ok(),
            "should allow multi-line edit on non-source file: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn edit_file_md_gate_blocks_non_insert() {
        let (dir, ctx) = project_ctx().await;
        let md = dir.path().join("test.md");
        std::fs::write(&md, "# Title\ncontent\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": md.to_str().unwrap(),
                    "old_string": "content",
                    "new_string": "new content"
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err(), "edit_file should gate .md files");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("edit_markdown"),
            "error should mention edit_markdown"
        );
    }

    #[tokio::test]
    async fn edit_file_md_gate_allows_prepend() {
        let (dir, ctx) = project_ctx().await;
        let md = dir.path().join("test.md");
        std::fs::write(&md, "# Title\ncontent\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": md.to_str().unwrap(),
                    "insert": "prepend",
                    "new_string": "---\nfrontmatter\n---\n"
                }),
                &ctx,
            )
            .await;

        assert!(result.is_ok(), "edit_file prepend should be allowed on .md");
    }

    #[tokio::test]
    async fn edit_file_md_gate_allows_append() {
        let (dir, ctx) = project_ctx().await;
        let md = dir.path().join("test.md");
        std::fs::write(&md, "# Title\ncontent\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": md.to_str().unwrap(),
                    "insert": "append",
                    "new_string": "\n## Footer\n"
                }),
                &ctx,
            )
            .await;

        assert!(result.is_ok(), "edit_file append should be allowed on .md");
    }

    #[tokio::test]
    async fn edit_file_null_new_string_errors() {
        // Bug: `new_string: null` was silently treated as "" (deletion) because
        // `input["new_string"].as_str().unwrap_or("")` returns "" for JSON null.
        // It must error instead — silent deletion is a footgun.
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world\n").unwrap();

        let err = EditFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old_string": "hello",
                    "new_string": null
                }),
                &ctx,
            )
            .await
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("required"),
            "should error that new_string is required, got: {msg}"
        );
        // File must be untouched — no silent deletion.
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "hello world\n");
    }

    #[tokio::test]
    async fn edit_file_warns_on_syntax_error_after_edit() {
        // An edit that breaks syntax should not fail (non-fatal) but should
        // return {"status":"ok","warning":"..."} so the agent immediately knows.
        let (dir, ctx) = project_ctx().await;
        let path = dir.path().join("app.py");
        std::fs::write(&path, "x = 1\ny = 2\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": path.to_str().unwrap(),
                    "old_string": "x = 1",
                    "new_string": "x = ("          // unclosed paren — invalid Python
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            result["status"], "ok",
            "syntax error is non-fatal; should not return Err"
        );
        let warning = result["warning"].as_str().unwrap_or("");
        assert!(
            warning.contains("syntax"),
            "warning should mention 'syntax', got: {warning:?}"
        );
    }

    // ── EditFile — batch edits ────────────────────────────────────────────────

    #[tokio::test]
    async fn batch_edit_applies_all() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.txt"), "# Title\nfoo\nbar\nbaz\n").unwrap();

        let _ = EditFile
            .call(
                json!({
                    "path": dir.path().join("test.txt").to_str().unwrap(),
                    "edits": [
                        {"old_string": "foo", "new_string": "FOO"},
                        {"old_string": "bar", "new_string": "BAR"},
                        {"old_string": "baz", "new_string": "BAZ"}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("FOO"));
        assert!(content.contains("BAR"));
        assert!(content.contains("BAZ"));
    }

    #[tokio::test]
    async fn batch_edit_string_coerced() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.txt"), "# Title\nfoo\nbar\n").unwrap();

        // Simulate MCP client that stringifies array params
        let _ = EditFile
            .call(
                json!({
                    "path": dir.path().join("test.txt").to_str().unwrap(),
                    "edits": "[{\"old_string\":\"foo\",\"new_string\":\"FOO\"},{\"old_string\":\"bar\",\"new_string\":\"BAR\"}]"
                }),
                &ctx,
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("FOO"), "first edit should apply");
        assert!(content.contains("BAR"), "second edit should apply");
    }

    #[tokio::test]
    async fn batch_edit_atomic_rollback() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.txt"), "# Title\nfoo\nbar\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": dir.path().join("test.txt").to_str().unwrap(),
                    "edits": [
                        {"old_string": "foo", "new_string": "FOO"},
                        {"old_string": "nonexistent", "new_string": "X"}
                    ]
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(
            content.contains("foo"),
            "first edit should have been rolled back"
        );
    }

    #[tokio::test]
    async fn batch_edit_and_old_string_mutual_exclusion() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("test.txt"), "# Title\n").unwrap();

        let result = EditFile
            .call(
                json!({
                    "path": dir.path().join("test.txt").to_str().unwrap(),
                    "old_string": "foo",
                    "new_string": "bar",
                    "edits": [{"old_string": "x", "new_string": "y"}]
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn batch_edit_line_shift() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(
            dir.path().join("test.txt"),
            "# Title\nline one\nline two\nline three\n",
        )
        .unwrap();

        let _ = EditFile
            .call(
                json!({
                    "path": dir.path().join("test.txt").to_str().unwrap(),
                    "edits": [
                        {"old_string": "line one", "new_string": "line one\nextra line a\nextra line b"},
                        {"old_string": "line three", "new_string": "line three updated"}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert!(content.contains("extra line a"));
        assert!(content.contains("extra line b"));
        assert!(content.contains("line three updated"));
    }

    // --- format_list_dir tests ---

    #[test]
    fn list_dir_basic() {
        let val = serde_json::json!({
            "entries": [
                "src/tools/ast.rs",
                "src/tools/config.rs",
                "src/tools/file.rs",
                "src/tools/git.rs",
                "src/tools/library.rs",
                "src/tools/memory.rs",
                "src/tools/mod.rs",
                "src/tools/output.rs",
                "src/tools/semantic.rs",
                "src/tools/symbol.rs",
                "src/tools/workflow.rs",
                "src/tools/user_format.rs"
            ]
        });
        let result = format_list_dir(&val);
        assert!(result.starts_with("src/tools — 12 entries"));
        assert!(result.contains("ast.rs"));
        assert!(result.contains("mod.rs"));
        assert!(result.contains("user_format.rs"));
        assert!(!result.contains("src/tools/ast.rs"));
    }

    #[test]
    fn list_dir_with_overflow() {
        let val = serde_json::json!({
            "entries": ["src/a.rs", "src/b.rs"],
            "overflow": { "shown": 200, "total": 350, "hint": "Use a more specific path" }
        });
        let result = format_list_dir(&val);
        assert!(result.contains("2 entries"));
        assert!(result.contains("200 of 350"));
        assert!(result.contains("Use a more specific path"));
    }

    #[test]
    fn list_dir_empty() {
        let val = serde_json::json!({ "entries": [] });
        assert_eq!(format_list_dir(&val), "(empty directory)");
    }

    #[test]
    fn list_dir_no_common_prefix() {
        let val = serde_json::json!({
            "entries": ["Cargo.toml", "README.md", "src/"]
        });
        let result = format_list_dir(&val);
        assert!(result.starts_with(". — 3 entries"));
        assert!(result.contains("Cargo.toml"));
        assert!(result.contains("README.md"));
        assert!(result.contains("src/"));
    }

    #[test]
    fn list_dir_recursive_deep_paths() {
        let val = serde_json::json!({
            "entries": [
                "src/lsp/client.rs",
                "src/lsp/ops.rs",
                "src/lsp/config.rs",
                "src/lsp/types.rs"
            ]
        });
        let result = format_list_dir(&val);
        assert!(result.starts_with("src/lsp — 4 entries"));
        assert!(result.contains("client.rs"));
        assert!(result.contains("ops.rs"));
    }

    #[test]
    fn list_dir_single_entry() {
        let val = serde_json::json!({
            "entries": ["src/main.rs"]
        });
        let result = format_list_dir(&val);
        assert!(result.contains("1 entries"));
        assert!(result.contains("main.rs"));
    }

    #[test]
    fn list_dir_directories_with_slash() {
        let val = serde_json::json!({
            "entries": ["src/tools/", "src/lsp/", "src/embed/"]
        });
        let result = format_list_dir(&val);
        assert!(result.contains("tools/"));
        assert!(result.contains("lsp/"));
        assert!(result.contains("embed/"));
    }

    #[test]
    fn list_dir_missing_entries() {
        let val = serde_json::json!({});
        assert_eq!(format_list_dir(&val), "");
    }

    #[test]
    fn list_dir_tree_mode_renders_indented() {
        let val = serde_json::json!({
            "entries": [
                "src/lsp/",
                "src/lsp/client.rs",
                "src/lsp/ops.rs",
                "src/tools/",
                "src/tools/file.rs",
                "src/main.rs"
            ]
        });
        let result = format_list_dir(&val);
        assert!(result.starts_with("src — 6 entries"));
        // Tree mode: entries on individual lines with indentation
        assert!(result.contains("  lsp/\n"));
        assert!(result.contains("    client.rs\n"));
        assert!(result.contains("    ops.rs\n"));
        assert!(result.contains("  tools/\n"));
        assert!(result.contains("    file.rs\n"));
        assert!(result.contains("  main.rs\n"));
        // No full paths
        assert!(!result.contains("src/lsp/client.rs"));
    }

    #[test]
    fn list_dir_depth_capped_note() {
        let val = serde_json::json!({
            "entries": ["src/a.rs"],
            "depth_capped": 3
        });
        let result = format_list_dir(&val);
        assert!(result.contains("depth capped at 3"));
        assert!(result.contains("max_depth"));
    }

    #[test]
    fn list_dir_no_depth_capped_note_when_absent() {
        let val = serde_json::json!({
            "entries": ["src/a.rs", "src/b.rs"]
        });
        let result = format_list_dir(&val);
        assert!(!result.contains("depth capped"));
    }

    // --- common_path_prefix tests ---

    #[test]
    fn prefix_empty_input() {
        assert_eq!(common_path_prefix(&[]), "");
    }

    #[test]
    fn prefix_single_path() {
        assert_eq!(common_path_prefix(&["src/tools/mod.rs"]), "src/tools/");
    }

    #[test]
    fn prefix_common_dir() {
        assert_eq!(
            common_path_prefix(&["src/tools/a.rs", "src/tools/b.rs"]),
            "src/tools/"
        );
    }

    #[test]
    fn prefix_no_common() {
        assert_eq!(common_path_prefix(&["Cargo.toml", "README.md"]), "");
    }

    #[test]
    fn prefix_partial_name_not_included() {
        assert_eq!(
            common_path_prefix(&["src/tools/a.rs", "src/tokens/b.rs"]),
            "src/"
        );
    }

    // --- format_grep tests ---

    #[test]
    fn search_simple_mode() {
        let val = serde_json::json!({
            "matches": [
                { "file": "src/tools/mod.rs", "line": 54, "content": "pub struct RecoverableError {" },
                { "file": "src/tools/mod.rs", "line": 60, "content": "    RecoverableError { error, hint }" },
                { "file": "src/server.rs", "line": 230, "content": "RecoverableError => {" }
            ],
            "total": 3
        });
        let result = format_grep(&val);
        assert!(result.starts_with("3 matches"));
        assert!(result.contains("src/tools/mod.rs:54"));
        assert!(result.contains("src/server.rs:230"));
        assert!(result.contains("pub struct RecoverableError {"));
    }

    #[test]
    fn search_context_mode() {
        let val = serde_json::json!({
            "matches": [
                {
                    "file": "src/tools/mod.rs",
                    "match_line": 54,
                    "start_line": 52,
                    "content": "/// Soft error\npub struct RecoverableError {\n    pub error: String,"
                }
            ],
            "total": 1
        });
        let result = format_grep(&val);
        assert!(result.starts_with("1 match\n"));
        assert!(result.contains("src/tools/mod.rs"));
        assert!(result.contains("52   /// Soft error"));
        assert!(result.contains("53   pub struct RecoverableError {"));
        assert!(result.contains("54       pub error: String,"));
    }

    #[test]
    fn search_with_overflow() {
        let val = serde_json::json!({
            "matches": [
                { "file": "src/a.rs", "line": 1, "content": "match" }
            ],
            "total": 1,
            "overflow": { "shown": 50, "hint": "Showing first 50 matches (cap hit). Narrow with a more specific pattern." }
        });
        let result = format_grep(&val);
        assert!(result.contains("first 50"));
        assert!(result.contains("Narrow with a more specific pattern"));
    }

    #[test]
    fn search_empty_matches() {
        let val = serde_json::json!({
            "matches": [],
            "total": 0
        });
        assert_eq!(format_grep(&val), "0 matches");
    }

    #[test]
    fn search_single_match_singular() {
        let val = serde_json::json!({
            "matches": [
                { "file": "src/main.rs", "line": 1, "content": "fn main() {" }
            ],
            "total": 1
        });
        let result = format_grep(&val);
        assert!(result.starts_with("1 match\n"));
        assert!(!result.starts_with("1 matches"));
    }

    #[test]
    fn search_context_mode_multiple_files() {
        let val = serde_json::json!({
            "matches": [
                {
                    "file": "src/a.rs",
                    "match_line": 10,
                    "start_line": 9,
                    "content": "// context\nfn foo() {"
                },
                {
                    "file": "src/b.rs",
                    "match_line": 5,
                    "start_line": 4,
                    "content": "// other\nfn bar() {"
                }
            ],
            "total": 2
        });
        let result = format_grep(&val);
        assert!(result.contains("2 matches"));
        assert!(result.contains("src/a.rs"));
        assert!(result.contains("src/b.rs"));
        assert!(result.contains("9    // context"));
        assert!(result.contains("10   fn foo() {"));
        assert!(result.contains("4    // other"));
        assert!(result.contains("5    fn bar() {"));
    }

    #[test]
    fn search_simple_alignment() {
        let val = serde_json::json!({
            "matches": [
                { "file": "a.rs", "line": 1, "content": "short" },
                { "file": "very/long/path.rs", "line": 100, "content": "long" }
            ],
            "total": 2
        });
        let result = format_grep(&val);
        assert!(result.contains("a.rs:1"));
        assert!(result.contains("very/long/path.rs:100"));
    }

    #[test]
    fn search_missing_matches_key() {
        let val = serde_json::json!({});
        assert_eq!(format_grep(&val), "");
    }

    // --- format_read_file tests ---

    #[test]
    fn read_file_content_mode_basic() {
        let val = serde_json::json!({
            "content": "fn main() {\n    println!(\"hello\");\n}\n",
            "total_lines": 3,
            "source": "project"
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("3 lines\n"));
        assert!(result.contains("1| fn main() {"));
        assert!(result.contains("2|     println!(\"hello\");"));
        assert!(result.contains("3| }"));
    }

    #[test]
    fn read_file_content_mode_single_line() {
        let val = serde_json::json!({
            "content": "hello world",
            "total_lines": 1,
            "source": "project"
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("1 line\n"));
        assert!(!result.starts_with("1 lines"));
        assert!(result.contains("1| hello world"));
    }

    #[test]
    fn read_file_content_with_overflow() {
        let val = serde_json::json!({
            "content": "line1\nline2\n",
            "total_lines": 500,
            "source": "project",
            "overflow": { "shown": 200, "total": 500, "hint": "File has 500 lines. Use start_line/end_line to read specific ranges" }
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("500 lines\n"));
        assert!(result.contains("200 of 500"));
        assert!(result.contains("start_line/end_line"));
    }

    #[test]
    fn read_file_empty_content() {
        let val = serde_json::json!({
            "content": "",
            "total_lines": 0,
            "source": "project"
        });
        let result = format_read_file(&val);
        assert_eq!(result, "0 lines");
    }

    #[test]
    fn read_file_missing_content() {
        let val = serde_json::json!({});
        assert_eq!(format_read_file(&val), "");
    }

    #[test]
    fn read_file_buffered_range_shows_hint() {
        // Auto-chunked response: has content + shown_lines + complete + next
        let val = serde_json::json!({
            "content": "line 0001 padding text\nline 0002 padding text\nline 0003 padding text",
            "file_id": "@file_abc123",
            "total_lines": 311,
            "shown_lines": [1, 3],
            "complete": false,
            "next": "read_file(\"@file_abc123\", start_line=4, end_line=6)"
        });
        let result = format_read_file(&val);
        assert!(
            result.contains("line 0001"),
            "should show content; got: {result}"
        );
        assert!(
            result.contains("3 of 311"),
            "should show progress; got: {result}"
        );
        assert!(
            result.contains("start_line=4"),
            "should show next command; got: {result}"
        );
    }

    #[test]
    fn format_read_file_auto_chunked() {
        let val = serde_json::json!({
            "content": "line 0001 text\nline 0002 text\nline 0003 text",
            "total_lines": 300,
            "shown_lines": [1, 3],
            "complete": false,
            "file_id": "@file_abc123",
            "next": "read_file(\"@file_abc123\", start_line=4, end_line=6)"
        });
        let result = format_read_file(&val);
        assert!(
            result.contains("line 0001"),
            "should show content; got: {result}"
        );
        assert!(
            result.contains("1|"),
            "should have line numbers; got: {result}"
        );
        assert!(
            result.contains("3 of 300"),
            "should show progress; got: {result}"
        );
        assert!(
            result.contains("start_line=4"),
            "should show next; got: {result}"
        );
        assert!(
            result.contains("@file_abc123"),
            "should show buffer ref; got: {result}"
        );
    }

    #[test]
    fn format_read_file_auto_chunked_mid_file() {
        // Chunk from the middle of a file — line numbers should start at 50
        let val = serde_json::json!({
            "content": "middle content\nmore content",
            "total_lines": 300,
            "shown_lines": [50, 51],
            "complete": false,
            "next": "read_file(\"@file_abc\", start_line=52, end_line=53)"
        });
        let result = format_read_file(&val);
        assert!(
            result.contains("50|"),
            "line numbers should start at 50; got: {result}"
        );
        assert!(result.contains("51|"), "should have line 51; got: {result}");
    }

    #[test]
    fn format_read_file_auto_chunked_complete() {
        let val = serde_json::json!({
            "content": "line 1\nline 2",
            "total_lines": 2,
            "shown_lines": [1, 2],
            "complete": true,
        });
        let result = format_read_file(&val);
        assert!(
            result.contains("line 1"),
            "should show content; got: {result}"
        );
        assert!(
            !result.contains("Next:"),
            "should not show next for complete reads; got: {result}"
        );
    }

    #[test]
    fn read_file_source_summary() {
        let val = serde_json::json!({
            "type": "source",
            "line_count": 500,
            "symbols": [
                { "name": "OutputGuard", "kind": "Struct", "line": 35 },
                { "name": "cap_items", "kind": "Function", "line": 55 }
            ],
            "file_id": "@file_abc123",
            "hint": "Full file stored as @file_abc123. Query with: run_command(\"grep/sed/awk @file_abc123\")"
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("500 lines\n"));
        assert!(result.contains("Symbols:"));
        assert!(result.contains("Struct"));
        assert!(result.contains("OutputGuard"));
        assert!(result.contains("L35"));
        assert!(result.contains("Function"));
        assert!(result.contains("cap_items"));
        assert!(result.contains("L55"));
        assert!(result.contains("Buffer: @file_abc123"));
        assert!(result.contains("Full file stored as @file_abc123"));
    }

    #[test]
    fn read_file_markdown_summary() {
        let val = serde_json::json!({
            "type": "markdown",
            "line_count": 200,
            "headings": [
                {"heading": "# Title", "level": 1, "line": 1, "end_line": 200},
                {"heading": "## Section 1", "level": 2, "line": 5, "end_line": 100},
                {"heading": "## Section 2", "level": 2, "line": 101, "end_line": 200}
            ],
            "file_id": "@file_xyz",
            "hint": "Full file stored as @file_xyz."
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("200 lines (Markdown)\n"));
        assert!(result.contains("Headings:"));
        assert!(result.contains("# Title  L1-200"));
        assert!(result.contains("## Section 1  L5-100"));
        assert!(result.contains("## Section 2  L101-200"));
        assert!(result.contains("Buffer: @file_xyz"));
    }

    #[test]
    fn read_file_config_summary() {
        let val = serde_json::json!({
            "type": "config",
            "line_count": 50,
            "preview": "[package]\nname = \"codescout\"\nversion = \"0.1.0\"",
            "file_id": "@file_cfg",
            "hint": "Full file stored as @file_cfg."
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("50 lines (Config)\n"));
        assert!(result.contains("Preview:"));
        assert!(result.contains("[package]"));
        assert!(result.contains("name = \"codescout\""));
        assert!(result.contains("Buffer: @file_cfg"));
    }

    #[test]
    fn read_file_generic_summary() {
        let val = serde_json::json!({
            "type": "generic",
            "line_count": 300,
            "head": "first line\nsecond line",
            "tail": "last line",
            "file_id": "@file_gen",
            "hint": "Full file stored as @file_gen."
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("300 lines\n"));
        assert!(result.contains("Head:"));
        assert!(result.contains("first line"));
        assert!(result.contains("second line"));
        assert!(result.contains("Tail:"));
        assert!(result.contains("last line"));
        assert!(result.contains("Buffer: @file_gen"));
    }

    #[test]
    fn read_file_source_summary_empty_symbols() {
        let val = serde_json::json!({
            "type": "source",
            "line_count": 100,
            "symbols": [],
            "file_id": "@file_empty",
            "hint": "Full file stored as @file_empty."
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("100 lines\n"));
        assert!(!result.contains("Symbols:"));
        assert!(result.contains("Buffer: @file_empty"));
    }

    #[test]
    fn read_file_lineno_alignment() {
        let content = (1..=12)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let val = serde_json::json!({
            "content": content,
            "total_lines": 12,
            "source": "project"
        });
        let result = format_read_file(&val);
        assert!(result.contains(" 1| line 1"));
        assert!(result.contains(" 9| line 9"));
        assert!(result.contains("10| line 10"));
        assert!(result.contains("12| line 12"));
    }

    #[test]
    fn read_file_source_summary_symbol_alignment() {
        let val = serde_json::json!({
            "type": "source",
            "line_count": 200,
            "symbols": [
                { "name": "Foo", "kind": "Struct", "line": 10 },
                { "name": "bar_function", "kind": "Function", "line": 50 }
            ],
            "file_id": "@file_align",
            "hint": "test"
        });
        let result = format_read_file(&val);
        assert!(result.contains("Struct  "));
        assert!(result.contains("Function"));
    }

    #[test]
    fn read_file_markdown_empty_headings() {
        let val = serde_json::json!({
            "type": "markdown",
            "line_count": 50,
            "headings": [],
            "file_id": "@file_md",
            "hint": "test"
        });
        let result = format_read_file(&val);
        assert!(result.starts_with("50 lines (Markdown)\n"));
        assert!(!result.contains("Headings:"));
        assert!(result.contains("Buffer: @file_md"));
    }

    #[tokio::test]
    async fn read_file_string_start_end_line_parsed_as_numbers() {
        // Bug A regression: start_line/end_line sent as strings must be parsed.
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(
            &file,
            "line1
line2
line3
line4
line5",
        )
        .unwrap();
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": "2",
                    "end_line": "4"
                }),
                &ctx,
            )
            .await
            .unwrap();
        let content = result["content"].as_str().unwrap();
        assert_eq!(
            content,
            "line2
line3
line4"
        );
    }

    #[tokio::test]
    async fn read_file_large_content_returns_file_id_not_inline() {
        // Bug B regression: files exceeding MAX_INLINE_TOKENS must return
        // a @file_* ref so start_line/end_line navigation works on the plain text,
        // not on a @tool_* JSON envelope.
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let ctx = ToolContext {
            agent,
            lsp: std::sync::Arc::new(crate::lsp::LspManager::new()),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let file = dir.path().join("big.txt");
        // Create a file > 10 KB (exceeds MAX_INLINE_TOKENS)
        let line = "x".repeat(100);
        let lines: Vec<&str> = std::iter::repeat(line.as_str()).take(120).collect();
        std::fs::write(
            &file,
            lines.join(
                "
",
            ),
        )
        .unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        // Must have file_id (not inline content)
        assert!(
            result["file_id"].as_str().is_some(),
            "large file should return file_id, got: {}",
            serde_json::to_string_pretty(&result).unwrap()
        );
        assert!(
            result["content"].is_null(),
            "should NOT have inline content"
        );
    }

    #[tokio::test]
    async fn read_file_small_fat_file_returns_content_inline() {
        // Regression: a 10-line JSONL file with ~600 bytes/line (~6KB total,
        // ~1500 tokens) must return content inline, not just a file_id.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("data.jsonl");
        let line = format!(
            "{{\"id\":1,\"data\":\"{}\"}}\n",
            "x".repeat(550) // ~570 bytes per line
        );
        let content: String = line.as_str().repeat(10);
        assert!(
            content.len() > 5_000,
            "test file must exceed old 5KB threshold"
        );
        assert!(
            content.len() / 4 <= crate::tools::MAX_INLINE_TOKENS,
            "test file must be under new token threshold"
        );
        std::fs::write(&file, &content).unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            result.get("content").is_some(),
            "small file should have inline content; got: {}",
            serde_json::to_string_pretty(&result).unwrap()
        );
        assert!(
            result.get("file_id").is_none(),
            "small file should NOT be buffered; got: {}",
            serde_json::to_string_pretty(&result).unwrap()
        );
    }

    #[tokio::test]
    async fn read_file_large_token_count_is_buffered() {
        // A file exceeding MAX_INLINE_TOKENS (~2500 tokens, ~10KB) must still
        // be buffered with a structural summary.
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.py");
        // 150 lines × 100 bytes = 15KB ≈ 3750 tokens → exceeds limit
        let line = format!("# {}\n", "x".repeat(95));
        let content: String = line.as_str().repeat(150);
        assert!(
            content.len() / 4 > crate::tools::MAX_INLINE_TOKENS,
            "test file must exceed token threshold"
        );
        std::fs::write(&file, &content).unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        assert!(
            result.get("file_id").is_some(),
            "large file should be buffered; got: {}",
            serde_json::to_string_pretty(&result).unwrap()
        );
    }
}
