//! `remove_symbol` — delete a symbol (function, struct, impl block, test, etc.) by name.

use serde_json::{json, Value};

use crate::tools::{guard_worktree_write, require_str_param, RecoverableError, Tool, ToolContext};

use super::display::format_remove_symbol;
use super::path_helpers::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path,
};
use crate::symbol::edit::{
    clamp_range_to_parent, editing_end_line, editing_start_line, find_parent_symbol, write_lines,
};
use crate::symbol::query::fetch_validated_symbol;

pub struct RemoveSymbol;

#[async_trait::async_trait]
impl Tool for RemoveSymbol {
    fn name(&self) -> &str {
        "remove_symbol"
    }

    fn description(&self) -> &str {
        "Delete a symbol (function, struct, impl block, test, etc.) by name. Removes the lines covered by the LSP symbol range."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["symbol", "path"],
            "properties": {
                "symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method', 'tests/old_test')" },
                "path": { "type": "string", "description": "File path" }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        guard_worktree_write(ctx).await?;
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;

        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // BUG-041: fetch + validate with auto-retry on stale LSP positions.
        let (sym, symbols) = fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start0 = editing_start_line(&sym, &lines);
        let end0 = (editing_end_line(&sym) as usize + 1).min(lines.len());

        // BUG-030/034/037/044 guard: symmetric parent clamp on both start and end.
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
                "The LSP may have stale data. Try list_symbols(path) to refresh.",
            )
            .into());
        }

        let mut new_lines: Vec<&str> = Vec::new();
        new_lines.extend_from_slice(&lines[..start]);
        new_lines.extend_from_slice(&lines[end..]);

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;
        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent.mark_file_dirty(full_path).await;
        let line_count = end - start;
        let removed_range = format!("{}-{}", start + 1, end);
        Ok(json!({
            "status": "ok",
            "removed_lines": removed_range,
            "line_count": line_count,
        }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_remove_symbol(result))
    }
}
