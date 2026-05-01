# `edit_code` Tool Consolidation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace four symbol-mutation tools (`rename_symbol`, `remove_symbol`, `replace_symbol`, `insert_code`) with a single `edit_code` tool that dispatches on an `action` enum field.

**Architecture:** New `src/tools/symbol/edit_code.rs` contains `EditCode` with four private `do_*` async methods carrying the exact logic from the old files. `call()` parses `action`, validates action-specific required params, then dispatches. All other files are updated to reference `edit_code` instead of the old names.

**Tech Stack:** Rust, serde_json, anyhow, MCP tool trait pattern, LSP client.

---

## File Map

| Change | File |
|--------|------|
| **Create** | `src/tools/symbol/edit_code.rs` |
| **Delete** | `src/tools/symbol/rename_symbol.rs` |
| **Delete** | `src/tools/symbol/remove_symbol.rs` |
| **Delete** | `src/tools/symbol/replace_symbol.rs` |
| **Delete** | `src/tools/symbol/insert_code.rs` |
| **Modify** | `src/tools/symbol/mod.rs` |
| **Modify** | `src/server.rs` |
| **Modify** | `src/util/path_security.rs` |
| **Modify** | `src/tools/edit_file.rs` |
| **Modify** | `src/prompts/server_instructions.md` |
| **Modify** | `src/mcp_resources/tool_usage.rs` |
| **Modify** | `src/symbol/edit.rs` (comment only) |
| **Modify** | `src/tools/onboarding.rs` (`ONBOARDING_VERSION` bump) |

---

## Task 1: Create `src/tools/symbol/edit_code.rs`

**Files:**
- Create: `src/tools/symbol/edit_code.rs`

- [ ] **Step 1: Write the new file**

```rust
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
                    return Err(
                        RecoverableError::new("action 'rename' requires 'new_name'").into()
                    );
                };
                self.do_rename(ctx, name_path, rel_path, new_name).await
            }
            "remove" => self.do_remove(ctx, name_path, rel_path).await,
            "replace" => {
                let Some(body) = input["body"].as_str() else {
                    return Err(
                        RecoverableError::new("action 'replace' requires 'body'").into()
                    );
                };
                self.do_replace(ctx, name_path, rel_path, body).await
            }
            "insert" => {
                let Some(body) = input["body"].as_str() else {
                    return Err(
                        RecoverableError::new("action 'insert' requires 'body'").into()
                    );
                };
                let position = input["position"].as_str().unwrap_or("after");
                self.do_insert(ctx, name_path, rel_path, body, position).await
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
        let mut lsp_files: std::collections::HashSet<PathBuf> =
            std::collections::HashSet::new();

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

        let (sym, symbols) =
            fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

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

        let (sym, symbols) =
            fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

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

        let (sym, symbols) =
            fetch_validated_symbol(&client, &full_path, &lang, name_path).await?;

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
```

- [ ] **Step 2: Verify file was created**

Run: `mcp__codescout__symbols(path="src/tools/symbol/edit_code.rs")`
Expected: `EditCode` struct + `impl Tool for EditCode` + 4 `do_*` methods visible.

---

## Task 2: Update Tests in `server.rs` to Reference `edit_code` (TDD Gate)

These tests will fail until Task 3 wires in `EditCode`. That's the point.

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Update `server_registers_all_tools` — remove old names, add `edit_code`**

In the `expected_tools` array (around line 1323), change:
```rust
// Remove these four:
"replace_symbol",
"insert_code",
"rename_symbol",
"remove_symbol",
// Add this one:
"edit_code",
```

The array should become (in whatever order the file has them):
```rust
let expected_tools = [
    "read_file",
    "tree",
    "grep",
    "create_file",
    "edit_file",
    "edit_markdown",
    "read_markdown",
    "run_command",
    "onboarding",
    "symbols",
    "references",
    "call_graph",
    "edit_code",
    "symbol_at",
    "memory",
    "semantic_search",
    "index",
    "workspace",
    "library",
];
```

- [ ] **Step 2: Update `server_tool_count_is_l3_target` from 22 to 19**

Find the assertion (around line 1375):
```rust
assert_eq!(
    core_count,
    22,
    "L3 target is 22 core tools; ...
```
Change `22` to `19` in both the `assert_eq!` call and the error string.

- [ ] **Step 3: Update `is_write_call` tests**

Find the block around line 2135:
```rust
assert!(server.is_write_call("replace_symbol", &json!({})));
assert!(server.is_write_call("insert_code", &json!({})));
assert!(server.is_write_call("remove_symbol", &json!({})));
assert!(server.is_write_call("rename_symbol", &json!({})));
```
Replace with:
```rust
assert!(server.is_write_call("edit_code", &json!({"action": "replace"})));
assert!(server.is_write_call("edit_code", &json!({"action": "insert"})));
assert!(server.is_write_call("edit_code", &json!({"action": "remove"})));
assert!(server.is_write_call("edit_code", &json!({"action": "rename"})));
```

- [ ] **Step 4: Update LSP-hiding test — remove `rename_symbol` from LSP tool list**

Find around line 1999:
```rust
for lsp_tool in &["symbol_at", "references", "rename_symbol"] {
    assert!(
        !visible.contains(lsp_tool),
        "LSP tool '{}' should be hidden when has_lsp=false",
        lsp_tool
    );
}
```
Change to:
```rust
for lsp_tool in &["symbol_at", "references"] {
    assert!(
        !visible.contains(lsp_tool),
        "LSP tool '{}' should be hidden when has_lsp=false",
        lsp_tool
    );
}
```

- [ ] **Step 5: Update `list_tools_shows_lsp_tools_when_has_lsp` — same removal**

Find around line 2036:
```rust
for lsp_tool in &["symbol_at", "references", "rename_symbol"] {
```
Change to:
```rust
for lsp_tool in &["symbol_at", "references"] {
```

- [ ] **Step 6: Update prompt-surface allowlist**

Find around line 1335:
```rust
"replace_symbol",
"insert_code",
"rename_symbol",
"remove_symbol",
```
Replace those four lines with:
```rust
"edit_code",
```

- [ ] **Step 7: Verify tests fail as expected**

Run: `mcp__codescout__run_command("cargo test server_registers_all_tools 2>&1 | tail -20")`
Expected: FAIL — `edit_code` not found in server. This confirms the TDD gate is live.

---

## Task 3: Wire `EditCode` into `mod.rs` + `server.rs`, Delete Old Files

**Files:**
- Modify: `src/tools/symbol/mod.rs`
- Modify: `src/server.rs`
- Delete: `src/tools/symbol/rename_symbol.rs`
- Delete: `src/tools/symbol/remove_symbol.rs`
- Delete: `src/tools/symbol/replace_symbol.rs`
- Delete: `src/tools/symbol/insert_code.rs`

- [ ] **Step 1: Update `src/tools/symbol/mod.rs`**

Remove these four module declarations and re-exports (around lines 7–25):
```rust
mod insert_code;
mod remove_symbol;
mod rename_symbol;
mod replace_symbol;
```
```rust
pub use insert_code::InsertCode;
pub use remove_symbol::RemoveSymbol;
pub use rename_symbol::RenameSymbol;
pub use replace_symbol::ReplaceSymbol;
```

Add in their place:
```rust
mod edit_code;
pub use edit_code::EditCode;
```

- [ ] **Step 2: Update `src/server.rs` — import and registration**

Around line 34, change the import:
```rust
// Before:
CallGraph, InsertCode, References, RemoveSymbol, RenameSymbol, ReplaceSymbol, SymbolAt,
// After:
CallGraph, EditCode, References, SymbolAt,
```

Around lines 113–116, replace 4 registrations with 1:
```rust
// Before:
Arc::new(ReplaceSymbol),
Arc::new(RemoveSymbol),
Arc::new(InsertCode),
Arc::new(RenameSymbol),
// After:
Arc::new(EditCode),
```

- [ ] **Step 3: Delete the four old source files**

```bash
rm src/tools/symbol/rename_symbol.rs \
   src/tools/symbol/remove_symbol.rs \
   src/tools/symbol/replace_symbol.rs \
   src/tools/symbol/insert_code.rs
```

- [ ] **Step 4: Verify compilation**

Run: `mcp__codescout__run_command("cargo build 2>&1 | head -40")`
Expected: Compiles without error. Any remaining errors are import/naming issues to fix before continuing.

- [ ] **Step 5: Run the TDD tests**

Run: `mcp__codescout__run_command("cargo test server_registers_all_tools server_tool_count 2>&1 | tail -30")`
Expected: Both tests PASS.

---

## Task 4: Fix `src/util/path_security.rs`

The write-tool allowlist and its tests reference the four old tool names.

**Files:**
- Modify: `src/util/path_security.rs`

- [ ] **Step 1: Update the write-tool match arm (around line 388)**

```rust
// Before:
"create_file" | "edit_file" | "replace_symbol" | "insert_code" | "rename_symbol"
| "remove_symbol" | "library" | "edit_markdown" => {
// After:
"create_file" | "edit_file" | "edit_code" | "library" | "edit_markdown" => {
```

- [ ] **Step 2: Update the test that checks `replace_symbol` (around line 985)**

```rust
// Before:
assert!(check_tool_access("replace_symbol", &config).is_ok());
// After:
assert!(check_tool_access("edit_code", &config).is_ok());
```

- [ ] **Step 3: Update the test list of write tools (around lines 997–1000)**

```rust
// Before:
"remove_symbol",
"replace_symbol",
"insert_code",
"rename_symbol",
// After:
"edit_code",
```

- [ ] **Step 4: Run path_security tests**

Run: `mcp__codescout__run_command("cargo test path_security 2>&1 | tail -20")`
Expected: All path_security tests PASS.

---

## Task 5: Fix `src/tools/edit_file.rs` Hints and Tests

`edit_file` redirects callers to the right symbol tool when it detects a structural edit. Update the hint strings and tests.

**Files:**
- Modify: `src/tools/edit_file.rs`

- [ ] **Step 1: Update hint strings (around lines 54, 57, 59)**

```rust
// Before (line ~54):
return "remove_symbol(symbol, path) — deletes the symbol and its doc comments/attributes";
// After:
return "edit_code(symbol, path, action='remove') — deletes the symbol and its doc comments/attributes";

// Before (line ~57):
return "insert_code(symbol, path, code, position) — inserts before or after a named symbol";
// After:
return "edit_code(symbol, path, action='insert', body=..., position=...) — inserts before or after a named symbol";

// Before (line ~59):
"replace_symbol(symbol, path, new_body) — replaces the symbol body via LSP"
// After:
"edit_code(symbol, path, action='replace', body=...) — replaces the symbol body via LSP"
```

- [ ] **Step 2: Update test assertions (around lines 2925, 2929–2941)**

```rust
// Line ~2925:
assert!(hint.contains("remove_symbol"), "got: {hint}");
// After:
assert!(hint.contains("edit_code"), "got: {hint}");

// Line ~2931:
assert!(hint.contains("replace_symbol"), "got: {hint}");
// After:
assert!(hint.contains("edit_code"), "got: {hint}");

// Line ~2941:
assert!(hint.contains("replace_symbol"), "got: {hint}");
// After:
assert!(hint.contains("edit_code"), "got: {hint}");
```

- [ ] **Step 3: Run edit_file tests**

Run: `mcp__codescout__run_command("cargo test edit_file 2>&1 | tail -20")`
Expected: All edit_file tests PASS.

---

## Task 6: Update `src/prompts/server_instructions.md`

Eight occurrences reference old tool names. Update each.

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Update Iron Law #1 (lines 8–9)**

```markdown
<!-- Before -->
source files. For structural code changes, use `replace_symbol`, `insert_code`,
`remove_symbol` — never the host's native Edit tool.
<!-- After -->
source files. For structural code changes, use `edit_code` — never the host's native Edit tool.
```

- [ ] **Step 2: Update Iron Law #2 (lines 19–20)**

```markdown
<!-- Before -->
2. **NO `edit_file` FOR STRUCTURAL CODE CHANGES.** Use `replace_symbol`, `insert_code`,
   `remove_symbol`, or `rename_symbol`. `edit_file` is for imports, literals, comments, config.
<!-- After -->
2. **NO `edit_file` FOR STRUCTURAL CODE CHANGES.** Use `edit_code`. `edit_file` is for imports, literals, comments, config.
```

- [ ] **Step 3: Update editing workflow line (line 66)**

```markdown
<!-- Before -->
- **Editing code:** `replace_symbol`, `insert_code`, `remove_symbol` for structural
<!-- After -->
- **Editing code:** `edit_code` for structural
```

- [ ] **Step 4: Update rename warning (line 82)**

```markdown
<!-- Before -->
- **MUST FOLLOW:** `rename_symbol` may corrupt string literals containing the
<!-- After -->
- **MUST FOLLOW:** `edit_code` rename may corrupt string literals containing the
```

- [ ] **Step 5: Update tool table (line 194)**

```markdown
<!-- Before -->
| 2 | `rename_symbol(symbol, path, new_name)` | LSP-powered rename across files |
<!-- After -->
| 2 | `edit_code(symbol, path, action='rename', new_name=...)` | LSP-powered rename across files |
```

- [ ] **Step 6: Update rule #7 (line 219)**

```markdown
<!-- Before -->
7. **Symbol edits over `edit_file` for code.** `replace_symbol`, `insert_code`, `remove_symbol` for structural changes. `edit_file` for imports, literals, comments.
<!-- After -->
7. **Symbol edits over `edit_file` for code.** `edit_code` for structural changes. `edit_file` for imports, literals, comments.
```

- [ ] **Step 7: Verify no stale tool names remain in prompts**

Run: `mcp__codescout__grep(pattern="rename_symbol|remove_symbol|replace_symbol|insert_code", path="src/prompts")`
Expected: Zero matches.

---

## Task 7: Fix Remaining References

**Files:**
- Modify: `src/mcp_resources/tool_usage.rs`
- Modify: `src/symbol/edit.rs`

- [ ] **Step 1: Update `tool_usage.rs` test data (around lines 300, 306, 319)**

```rust
// Line ~300 — change the stats entry:
make_stats("rename_symbol", 2),
// After:
make_stats("edit_code", 2),

// Line ~306 — change the string literal:
"rename_symbol".into(),
// After:
"edit_code".into(),

// Line ~319 — change the JSON array:
serde_json::json!(["rename_symbol", "symbol_at"])
// After:
serde_json::json!(["edit_code", "symbol_at"])
```

- [ ] **Step 2: Update comment in `src/symbol/edit.rs` (lines 4–5)**

```rust
// Before:
//! Write-path helpers shared by `insert_code`, `remove_symbol`, `replace_symbol`,
//! and `rename_symbol`.
// After:
//! Write-path helpers shared by `edit_code`.
```

- [ ] **Step 3: Run tool_usage tests**

Run: `mcp__codescout__run_command("cargo test tool_usage 2>&1 | tail -20")`
Expected: All tool_usage tests PASS.

---

## Task 8: Bump `ONBOARDING_VERSION`

Server instructions changed — onboarding refresh must trigger for existing projects.

**Files:**
- Modify: `src/tools/onboarding.rs`

- [ ] **Step 1: Bump version from 16 to 17**

In `src/tools/onboarding.rs` around line 19:
```rust
// Before:
pub(crate) const ONBOARDING_VERSION: u32 = 16;
// After:
pub(crate) const ONBOARDING_VERSION: u32 = 17;
```

---

## Task 9: Full Verify + Commit

- [ ] **Step 1: Run full test suite**

Run: `mcp__codescout__run_command("cargo test 2>&1 | tail -30")`
Expected: All tests pass. Zero failures.

- [ ] **Step 2: Run clippy**

Run: `mcp__codescout__run_command("cargo clippy -- -D warnings 2>&1 | tail -30")`
Expected: Clean — zero warnings.

- [ ] **Step 3: Run fmt check**

Run: `mcp__codescout__run_command("cargo fmt --check 2>&1")`
Expected: No output (already formatted), or run `cargo fmt` to fix.

- [ ] **Step 4: Verify prompt surface test passes**

Run: `mcp__codescout__run_command("cargo test prompt_surfaces_reference_only_real_tools 2>&1 | tail -20")`
Expected: PASS — no stale tool names in any prompt surface.

- [ ] **Step 5: Build release binary**

Run: `mcp__codescout__run_command("cargo build --release 2>&1 | tail -10")`
Expected: Compiles successfully.

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol/edit_code.rs \
        src/tools/symbol/mod.rs \
        src/server.rs \
        src/util/path_security.rs \
        src/tools/edit_file.rs \
        src/prompts/server_instructions.md \
        src/mcp_resources/tool_usage.rs \
        src/symbol/edit.rs \
        src/tools/onboarding.rs
git rm src/tools/symbol/rename_symbol.rs \
       src/tools/symbol/remove_symbol.rs \
       src/tools/symbol/replace_symbol.rs \
       src/tools/symbol/insert_code.rs
git commit -m "feat(tools): consolidate rename/remove/replace/insert into edit_code

Four separate symbol-mutation tools collapsed into one edit_code tool
dispatching on action='rename'|'remove'|'replace'|'insert'. Reduces tool
count from 22 to 19. No behavioral changes — all logic moved verbatim."
```

- [ ] **Step 7: Restart MCP server to pick up release binary**

Run `/mcp` in Claude Code to reconnect. Verify `edit_code` appears in tool list and old tool names do not.
