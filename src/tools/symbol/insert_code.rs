//! `insert_code` — insert code immediately before or after a named symbol.

use serde_json::{json, Value};

use crate::tools::{guard_worktree_write, require_str_param, Tool, ToolContext};

use super::display::format_insert_code;
use super::{
    editing_end_line, editing_start_line, fetch_validated_symbol, find_parent_symbol,
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path, write_lines,
};

pub struct InsertCode;

#[async_trait::async_trait]
impl Tool for InsertCode {
    fn name(&self) -> &str {
        "insert_code"
    }
    fn description(&self) -> &str {
        "Insert code immediately before or after a named symbol."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["symbol", "path", "code"],
            "properties": {
                "symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "code": { "type": "string", "description": "Code to insert (may contain newlines)" },
                "position": {
                    "type": "string",
                    "enum": ["before", "after"],
                    "description": "Insert before or after the symbol (default: after)"
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        guard_worktree_write(ctx).await?;
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let code = require_str_param(&input, "code")?;
        let position = input["position"].as_str().unwrap_or("after");

        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // BUG-041: fetch + validate with auto-retry on stale LSP positions.
        let (sym, symbols) = fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let code_lines: Vec<&str> = code.lines().collect();
        let insert_at0 = match position {
            "before" => editing_start_line(&sym, &lines),
            _ => (editing_end_line(&sym) as usize + 1).min(lines.len()),
        };

        // BUG-029/036 guard: clamp insertion point to the parent container's body
        // when `sym` is nested. `position="before"` must not land above the parent
        // header (eats the header); `position="after"` must not land past the
        // parent's closer (moves the inserted code outside the parent block).
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
        ctx.agent.mark_file_dirty(full_path).await;
        Ok(json!({ "status": "ok", "inserted_at_line": insert_at + 1, "position": position }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_insert_code(result))
    }
}
