//! `replace_symbol` — replace the entire body of a named symbol with new source code.

use serde_json::{json, Value};

use crate::tools::{
    guard_worktree_write, require_str_param, require_str_param_or, RecoverableError, Tool,
    ToolContext,
};

use super::display::format_replace_symbol;
use super::path_helpers::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path,
};
use crate::symbol::edit::{
    clamp_range_to_parent, collect_all_name_paths, editing_end_line, editing_start_line,
    find_ast_name_path, find_parent_symbol, write_lines,
};
use crate::symbol::query::{count_symbols_by_name_path, fetch_validated_symbol};

pub struct ReplaceSymbol;

#[async_trait::async_trait]
impl Tool for ReplaceSymbol {
    fn name(&self) -> &str {
        "replace_symbol"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Replace the entire body of a named symbol with new source code. \
         new_body should include the full declaration: attributes, doc comments, \
         signature, and body — matching what symbols(include_body=true) returns."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["symbol", "path", "new_body"],
            "properties": {
                "symbol": { "type": "string" },
                "path": { "type": "string" },
                "new_body": { "type": "string" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        guard_worktree_write(ctx).await?;
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let new_body =
            require_str_param_or(&input, "new_body", &["new_code", "new_source", "body"])?;

        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // BUG-041: fetch + validate with auto-retry on stale LSP positions.
        let (sym, symbols) = fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start0 = editing_start_line(&sym, &lines);
        let end0 = (editing_end_line(&sym) as usize + 1).min(lines.len());

        // BUG-030/034/037/044 guard: clamp both start and end to the parent
        // container's body range when `sym` is nested. Stale LSP data can report
        // a child's `range_start_line` as the parent's attribute line (eating the
        // parent header) or its `range.end` as overshooting into a sibling
        // (dropping the sibling body).
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

        // Pre-write AST snapshot: count how many symbols with this exact name_path
        // exist now. Used after the write to detect if the symbol was silently dropped.
        // Walks the full AST tree (not just top level) so nested methods in Java,
        // Kotlin, Python, TypeScript class bodies are also protected.
        let pre_ast = crate::ast::extract_symbols(&full_path).ok();
        let pre_count = pre_ast
            .as_ref()
            .map(|syms| count_symbols_by_name_path(syms, &sym.name_path))
            .unwrap_or(0);
        // BUG-044: also snapshot the *set* of name_paths, so we can detect
        // sibling symbols that vanish after the write (e.g. `impl Type/method_a`
        // is replaced but `impl Type/method_b` gets eaten by an overshooting range).
        let pre_set = pre_ast.as_ref().map(|s| collect_all_name_paths(s));
        // Target symbol's equivalent in the AST namespace — used to subtract
        // the intentionally-replaced symbol from the "dropped" diff.
        let target_ast_name_path = pre_ast
            .as_ref()
            .and_then(|s| find_ast_name_path(s, &sym.name, sym.start_line));

        let mut new_lines = Vec::new();
        new_lines.extend_from_slice(&lines[..start]);
        new_lines.extend(new_body.lines());
        new_lines.extend_from_slice(&lines[end..]);

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;

        // Post-write integrity check: if AST found the symbol before the write (pre_count > 0)
        // but cannot find it after, the replacement dropped the declaration.
        // This catches the common mistake of passing body-only code to replace_symbol.
        // We use AST (tree-sitter, synchronous) — no LSP round-trip needed.
        let post_ast = crate::ast::extract_symbols(&full_path).ok();
        if pre_count > 0 {
            let post_count = post_ast
                .as_ref()
                .map(|syms| count_symbols_by_name_path(syms, &sym.name_path))
                .unwrap_or(pre_count); // if AST fails post-write, trust the write

            if post_count == 0 {
                // Roll back before notifying LSP so the server never sees the broken state.
                write_lines(&full_path, &lines, content.ends_with('\n'))?;
                ctx.lsp.notify_file_changed(&full_path).await;
                ctx.agent.mark_file_dirty(full_path).await;
                return Err(RecoverableError::with_hint(
                    format!(
                        "replace_symbol('{name_path}') dropped the symbol definition — \
                         new_body must be the complete declaration (attributes, doc comments, \
                         signature, and body), not just body statements. File restored."
                    ),
                    "Use symbols(symbol=..., include_body=true) to see the expected format.",
                )
                .into());
            }
        }

        // BUG-044 guard: compare pre/post AST `name_path` sets. Any symbol that
        // existed pre-write but not post-write, other than the intentionally-edited
        // target, was eaten by the write — almost always an overshooting LSP
        // `range.end` into a sibling. Roll back to avoid silent corruption.
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
                ctx.agent.mark_file_dirty(full_path).await;
                return Err(RecoverableError::with_hint(
                    format!(
                        "replace_symbol('{name_path}') would have dropped sibling symbols: {}. \
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
        ctx.agent.mark_file_dirty(full_path).await;
        Ok(json!({ "status": "ok", "replaced_lines": format!("{}-{}", start + 1, end) }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_replace_symbol(result))
    }
}
