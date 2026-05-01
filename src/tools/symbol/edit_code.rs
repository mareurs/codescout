//! `edit_code` — unified symbol mutation: rename (LSP), remove, replace, insert.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::tools::{guard_worktree_write, require_str_param, RecoverableError, Tool, ToolContext};

use super::display::{
    format_insert_code, format_remove_symbol, format_rename_symbol, format_replace_symbol,
};
use super::path_helpers::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path, uri_to_path,
};
use crate::symbol::edit::{
    apply_text_edits, clamp_range_to_parent, collect_all_name_paths, editing_end_line,
    editing_start_line, find_ast_name_path, find_parent_symbol, text_sweep, write_lines,
};
use crate::symbol::query::{
    count_symbols_by_name_path, fetch_validated_symbol, find_unique_symbol_by_name_path,
};

pub struct EditCode;

#[async_trait::async_trait]
impl Tool for EditCode {
    fn name(&self) -> &str {
        "edit_code"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Mutate a symbol in the codebase. action='replace': overwrite the symbol body. \
         action='insert': inject code adjacent to a symbol. action='remove': delete the symbol. \
         action='rename': rename across the entire codebase via LSP (also sweeps textual \
         occurrences in comments/strings)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["symbol", "path", "action"],
            "properties": {
                "symbol":   { "type": "string" },
                "path":     { "type": "string" },
                "action":   { "type": "string", "enum": ["rename", "remove", "replace", "insert"] },
                "new_name": { "type": "string", "description": "rename only" },
                "body":     { "type": "string", "description": "replace: new body; insert: code to inject" },
                "position": {
                    "type": "string",
                    "enum": ["before", "after"],
                    "description": "insert only, default 'after'"
                }
            }
        })
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        if result.get("files_changed").is_some() {
            Some(format_rename_symbol(result))
        } else if result.get("removed_lines").is_some() {
            Some(format_remove_symbol(result))
        } else if result.get("replaced_lines").is_some() {
            Some(format_replace_symbol(result))
        } else if result.get("inserted_at_line").is_some() {
            Some(format_insert_code(result))
        } else {
            None
        }
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        guard_worktree_write(ctx).await?;
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let action = require_str_param(&input, "action")?;

        match action {
            "rename" => {
                let Some(new_name) = input["new_name"].as_str() else {
                    return Err(RecoverableError::new("action 'rename' requires 'new_name'").into());
                };
                self.do_rename(ctx, name_path, rel_path, new_name).await
            }
            "remove" => self.do_remove(ctx, name_path, rel_path).await,
            "replace" => {
                let Some(body) = input["body"].as_str() else {
                    return Err(RecoverableError::new("action 'replace' requires 'body'").into());
                };
                self.do_replace(ctx, name_path, rel_path, body).await
            }
            "insert" => {
                let Some(body) = input["body"].as_str() else {
                    return Err(RecoverableError::new("action 'insert' requires 'body'").into());
                };
                let position = input["position"].as_str().unwrap_or("after");
                self.do_insert(ctx, name_path, rel_path, body, position)
                    .await
            }
            _ => Err(RecoverableError::new(format!("unknown action '{action}'")).into()),
        }
    }
}

impl EditCode {
    async fn do_rename(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
        new_name: &str,
    ) -> anyhow::Result<Value> {
        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        let symbols = client.document_symbols(&full_path, &lang).await?;
        let sym = find_unique_symbol_by_name_path(&symbols, name_path)?;

        let edit = client
            .rename(&full_path, sym.start_line, sym.start_col, new_name, &lang)
            .await?;

        let rename_root = ctx.agent.require_project_root().await?;
        let rename_security = ctx.agent.security_config().await;
        let mut lsp_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        struct PlannedWrite {
            path: PathBuf,
            pre_image: String,
            new_content: String,
            edit_count: usize,
        }
        let mut plan: Vec<PlannedWrite> = Vec::new();
        let plan_path = |path: PathBuf,
                         plain_edits: Vec<lsp_types::TextEdit>,
                         plan: &mut Vec<PlannedWrite>|
         -> anyhow::Result<()> {
            if plan.iter().any(|p| p.path == path) {
                return Ok(());
            }
            let path_str = path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non-UTF8 path from LSP: {:?}", path))?;
            crate::util::path_security::validate_write_path(
                path_str,
                &rename_root,
                &rename_security,
            )?;
            let pre_image = std::fs::read_to_string(&path)?;
            let new_content = apply_text_edits(&pre_image, &plain_edits);
            let edit_count = plain_edits.len();
            plan.push(PlannedWrite {
                path,
                pre_image,
                new_content,
                edit_count,
            });
            Ok(())
        };

        if let Some(changes) = &edit.changes {
            for (uri, edits) in changes {
                let Some(path) = uri_to_path(uri.as_str()) else {
                    continue;
                };
                plan_path(path, edits.clone(), &mut plan)?;
            }
        }

        if let Some(doc_changes) = &edit.document_changes {
            let operations: Vec<&lsp_types::DocumentChangeOperation> = match doc_changes {
                lsp_types::DocumentChanges::Edits(edits) => {
                    for text_edit in edits {
                        let Some(path) = uri_to_path(text_edit.text_document.uri.as_str()) else {
                            continue;
                        };
                        let plain_edits: Vec<lsp_types::TextEdit> = text_edit
                            .edits
                            .iter()
                            .map(|e| match e {
                                lsp_types::OneOf::Left(te) => te.clone(),
                                lsp_types::OneOf::Right(ate) => ate.text_edit.clone(),
                            })
                            .collect();
                        plan_path(path, plain_edits, &mut plan)?;
                    }
                    vec![]
                }
                lsp_types::DocumentChanges::Operations(ops) => ops.iter().collect(),
            };
            for change in operations {
                if let lsp_types::DocumentChangeOperation::Edit(text_edit) = change {
                    let Some(path) = uri_to_path(text_edit.text_document.uri.as_str()) else {
                        continue;
                    };
                    let plain_edits: Vec<lsp_types::TextEdit> = text_edit
                        .edits
                        .iter()
                        .map(|e| match e {
                            lsp_types::OneOf::Left(te) => te.clone(),
                            lsp_types::OneOf::Right(ate) => ate.text_edit.clone(),
                        })
                        .collect();
                    plan_path(path, plain_edits, &mut plan)?;
                }
            }
        }

        let mut files_changed = 0usize;
        let mut total_edits = 0usize;
        for (i, planned) in plan.iter().enumerate() {
            if let Err(e) = std::fs::write(&planned.path, &planned.new_content) {
                let mut dirty: Vec<String> = Vec::new();
                for prev in plan.iter().take(i) {
                    if let Err(restore_err) = std::fs::write(&prev.path, &prev.pre_image) {
                        tracing::error!(
                            "rename rollback failed for {:?}: {}",
                            prev.path,
                            restore_err,
                        );
                        dirty.push(prev.path.display().to_string());
                    }
                }
                if dirty.is_empty() {
                    anyhow::bail!(
                        "write failed for {:?}: {} (previous {} file(s) restored)",
                        planned.path,
                        e,
                        i,
                    );
                } else {
                    anyhow::bail!(
                        "write failed for {:?}: {}; rollback ALSO failed for: {} \
                         — these files are now in an inconsistent state and need \
                         manual review",
                        planned.path,
                        e,
                        dirty.join(", "),
                    );
                }
            }
            lsp_files.insert(planned.path.clone());
            files_changed += 1;
            total_edits += planned.edit_count;
        }

        for path in &lsp_files {
            ctx.lsp.notify_file_changed(path).await;
            ctx.agent.invalidate_call_edges(path).await;
            ctx.agent.mark_file_dirty(path.clone()).await;
        }

        let mut corruption_hints: Vec<Value> = vec![];
        if new_name.len() >= 4 {
            if let Ok(embedded_re) =
                regex::Regex::new(&format!(r"[a-zA-Z0-9]{}", regex::escape(new_name)))
            {
                for path in &lsp_files {
                    let Ok(content) = std::fs::read_to_string(path) else {
                        continue;
                    };
                    let rel = path
                        .strip_prefix(&rename_root)
                        .unwrap_or(path)
                        .display()
                        .to_string();
                    let mut flagged_lines: Vec<u32> = vec![];
                    let mut previews: Vec<String> = vec![];
                    for (i, line) in content.lines().enumerate() {
                        if embedded_re.is_match(line) {
                            flagged_lines.push((i + 1) as u32);
                            if previews.len() < 3 {
                                previews.push(line.trim().to_string());
                            }
                        }
                    }
                    if !flagged_lines.is_empty() {
                        corruption_hints.push(json!({
                            "file": rel,
                            "lines": flagged_lines,
                            "previews": previews,
                        }));
                    }
                }
            }
        }

        let old_name_str = name_path.rsplit('/').next().unwrap_or(name_path);
        let (textual, sweep_skipped, sweep_skip_reason) = if old_name_str.len() < 4 {
            (
                vec![],
                true,
                Some(format!(
                    "name too short ({} chars, minimum 4)",
                    old_name_str.len()
                )),
            )
        } else {
            let sweep_root = rename_root.clone();
            let sweep_name = old_name_str.to_string();
            let sweep_files = lsp_files.clone();
            let sweep_result = tokio::task::spawn_blocking(move || {
                text_sweep(&sweep_root, &sweep_name, &sweep_files, 20, 2)
            })
            .await;
            match sweep_result {
                Ok(Ok(matches)) => (matches, false, None::<String>),
                Ok(Err(e)) => {
                    tracing::warn!("text sweep after rename failed: {e}");
                    (vec![], false, Some(format!("sweep error: {e}")))
                }
                Err(join_err) => {
                    tracing::warn!("text sweep task join failed: {join_err}");
                    (
                        vec![],
                        false,
                        Some(format!("sweep task failed: {join_err}")),
                    )
                }
            }
        };

        let textual_total: usize = textual.iter().map(|m| m.occurrence_count).sum();
        let textual_shown = textual.len();
        let textual_json: Vec<Value> = textual
            .into_iter()
            .map(|m| {
                json!({
                    "file": m.file,
                    "lines": m.lines,
                    "previews": m.previews,
                    "occurrence_count": m.occurrence_count,
                    "kind": m.kind,
                })
            })
            .collect();

        let mut result = json!({
            "status": "ok",
            "old_name": old_name_str,
            "new_name": new_name,
            "files_changed": files_changed,
            "total_edits": total_edits,
            "textual_matches": textual_json,
            "textual_match_count": textual_total,
            "textual_matches_shown": textual_shown,
            "sweep_skipped": sweep_skipped,
            "verify_hint": "LSP rename may match occurrences inside string literals, comments, or macro arguments. Verify each changed file is still valid (e.g. cargo check / tsc --noEmit).",
        });
        if !corruption_hints.is_empty() {
            result["corruption_warning"] = json!(
                "new_name appears immediately after an alphanumeric character in the files \
                 below — the LSP may have applied an edit at the wrong column. Inspect \
                 these lines and run a build check (e.g. cargo check) before proceeding."
            );
            result["corruption_hints"] = json!(corruption_hints);
        }
        if let Some(reason) = sweep_skip_reason {
            result["sweep_skip_reason"] = json!(reason);
        }
        Ok(result)
    }

    async fn do_remove(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
    ) -> anyhow::Result<Value> {
        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        let (sym, symbols) = fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start0 = editing_start_line(&sym, &lines);
        let end0 = (editing_end_line(&sym) as usize + 1).min(lines.len());

        let (start, end) = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
            let parent_body_start = parent.start_line as usize + 1;
            let parent_body_end_exclusive = parent.end_line as usize;
            clamp_range_to_parent(start0, end0, parent_body_start, parent_body_end_exclusive)
        } else {
            (start0, end0)
        };

        if start >= lines.len() {
            return Err(RecoverableError::with_hint(
                format!(
                    "symbol range out of bounds: start line {} but file has {} lines",
                    start + 1,
                    lines.len(),
                ),
                "The LSP may have stale data. Try symbols(path) to refresh.",
            )
            .into());
        }

        let mut new_lines: Vec<&str> = Vec::new();
        new_lines.extend_from_slice(&lines[..start]);
        new_lines.extend_from_slice(&lines[end..]);

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;
        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent.invalidate_call_edges(&full_path).await;
        ctx.agent.mark_file_dirty(full_path).await;
        let line_count = end - start;
        let removed_range = format!("{}-{}", start + 1, end);
        Ok(json!({
            "status": "ok",
            "removed_lines": removed_range,
            "line_count": line_count,
        }))
    }

    async fn do_replace(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
        new_body: &str,
    ) -> anyhow::Result<Value> {
        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        let (sym, symbols) = fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start0 = editing_start_line(&sym, &lines);
        let end0 = (editing_end_line(&sym) as usize + 1).min(lines.len());

        let (start, end) = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
            let parent_body_start = parent.start_line as usize + 1;
            let parent_body_end_exclusive = parent.end_line as usize;
            clamp_range_to_parent(start0, end0, parent_body_start, parent_body_end_exclusive)
        } else {
            (start0, end0)
        };

        if start >= lines.len() {
            return Err(RecoverableError::with_hint(
                format!(
                    "symbol range out of bounds: start line {} but file has {} lines",
                    start + 1,
                    lines.len(),
                ),
                "The LSP may have stale data. Try symbols(path) to refresh.",
            )
            .into());
        }

        let pre_ast = crate::ast::extract_symbols(&full_path).ok();
        let pre_count = pre_ast
            .as_ref()
            .map(|syms| count_symbols_by_name_path(syms, &sym.name_path))
            .unwrap_or(0);
        let pre_set = pre_ast.as_ref().map(|s| collect_all_name_paths(s));
        let target_ast_name_path = pre_ast
            .as_ref()
            .and_then(|s| find_ast_name_path(s, &sym.name, sym.start_line));

        let mut new_lines = Vec::new();
        new_lines.extend_from_slice(&lines[..start]);
        new_lines.extend(new_body.lines());
        new_lines.extend_from_slice(&lines[end..]);

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;

        let post_ast = crate::ast::extract_symbols(&full_path).ok();
        if pre_count > 0 {
            let post_count = post_ast
                .as_ref()
                .map(|syms| count_symbols_by_name_path(syms, &sym.name_path))
                .unwrap_or(pre_count);

            if post_count == 0 {
                write_lines(&full_path, &lines, content.ends_with('\n'))?;
                ctx.lsp.notify_file_changed(&full_path).await;
                ctx.agent.invalidate_call_edges(&full_path).await;
                ctx.agent.mark_file_dirty(full_path).await;
                return Err(RecoverableError::with_hint(
                    format!(
                        "edit_code replace('{name_path}') dropped the symbol definition — \
                         body must be the complete declaration (attributes, doc comments, \
                         signature, and body), not just body statements. File restored."
                    ),
                    "Use symbols(symbol=..., include_body=true) to see the expected format.",
                )
                .into());
            }
        }

        if let (Some(pre), Some(post)) = (pre_set.as_ref(), post_ast.as_ref()) {
            let post_set = collect_all_name_paths(post);
            let dropped: Vec<String> = pre
                .difference(&post_set)
                .filter(|np| target_ast_name_path.as_deref() != Some(np.as_str()))
                .cloned()
                .collect();
            if !dropped.is_empty() {
                write_lines(&full_path, &lines, content.ends_with('\n'))?;
                ctx.lsp.notify_file_changed(&full_path).await;
                ctx.agent.invalidate_call_edges(&full_path).await;
                ctx.agent.mark_file_dirty(full_path).await;
                return Err(RecoverableError::with_hint(
                    format!(
                        "edit_code replace('{name_path}') would have dropped sibling symbols: {}. \
                         The edit range overshot into adjacent code (likely a stale LSP range). \
                         File restored.",
                        dropped.join(", ")
                    ),
                    "Try symbols(path) to refresh, then retry; or narrow the edit via \
                     edit_file with unique anchors.",
                )
                .into());
            }
        }

        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent.invalidate_call_edges(&full_path).await;
        ctx.agent.mark_file_dirty(full_path).await;
        Ok(json!({ "status": "ok", "replaced_lines": format!("{}-{}", start + 1, end) }))
    }

    async fn do_insert(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
        code: &str,
        position: &str,
    ) -> anyhow::Result<Value> {
        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        let (sym, symbols) = fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let code_lines: Vec<&str> = code.lines().collect();
        let insert_at0 = match position {
            "before" => editing_start_line(&sym, &lines),
            _ => (editing_end_line(&sym) as usize + 1).min(lines.len()),
        };

        let insert_at = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
            let parent_body_start = parent.start_line as usize + 1;
            let parent_body_end_exclusive = parent.end_line as usize;
            insert_at0
                .max(parent_body_start)
                .min(parent_body_end_exclusive)
        } else {
            insert_at0
        };

        let mut new_lines = Vec::new();
        new_lines.extend_from_slice(&lines[..insert_at]);
        new_lines.extend(code_lines.iter().copied());
        if position == "before" {
            new_lines.push("");
        } else {
            let needs_blank = lines.get(insert_at).is_some_and(|l| !l.trim().is_empty());
            if needs_blank {
                new_lines.push("");
            }
        }
        new_lines.extend_from_slice(&lines[insert_at..]);

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;
        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent.invalidate_call_edges(&full_path).await;
        ctx.agent.mark_file_dirty(full_path).await;
        Ok(json!({ "status": "ok", "inserted_at_line": insert_at + 1, "position": position }))
    }
}
