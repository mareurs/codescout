//! `rename_symbol` — rename a symbol across the entire codebase using LSP.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::tools::{guard_worktree_write, require_str_param, Tool, ToolContext};

use super::display::format_rename_symbol;
use super::path_helpers::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path, uri_to_path,
};
use crate::symbol::edit::{apply_text_edits, text_sweep};
use crate::symbol::query::find_unique_symbol_by_name_path;

pub struct RenameSymbol;

#[async_trait::async_trait]
impl Tool for RenameSymbol {
    fn name(&self) -> &str {
        "rename_symbol"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Rename a symbol across the entire codebase using LSP. After renaming, sweeps for remaining textual occurrences (comments, docs, strings) that LSP missed and reports them."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["symbol", "path", "new_name"],
            "properties": {
                "symbol": { "type": "string" },
                "path": { "type": "string" },
                "new_name": { "type": "string" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        guard_worktree_write(ctx).await?;
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let new_name = require_str_param(&input, "new_name")?;

        let full_path = resolve_write_path(ctx, rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // Find the symbol to get its position
        let symbols = client.document_symbols(&full_path, &lang).await?;
        let sym = find_unique_symbol_by_name_path(&symbols, name_path)?;

        // Request rename from LSP
        let edit = client
            .rename(&full_path, sym.start_line, sym.start_col, new_name, &lang)
            .await?;

        // Apply workspace edit — validate every file from the LSP response
        // as a write target before modifying it.
        //
        // Two-phase execution for rollback safety (phase-4 I1):
        //   1. Plan phase: validate every path, read each file once, compute
        //      the new content in memory. Any error here aborts with zero
        //      writes to disk.
        //   2. Write phase: write each planned file in sequence. If a write
        //      fails at index N, restore files 0..N-1 from their in-memory
        //      pre-images. Restore failures are logged but not fatal — we
        //      surface the original write error so the caller can investigate.
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
            // Skip if we already planned a write for this path (LSP may repeat
            // the same file across `changes` and `document_changes`).
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

        // Write phase. On failure, roll back previously-written files from
        // their in-memory pre-images.
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

        // Notify LSP of all changed files so its symbol state is refreshed.
        // Without this, symbols can still return old names even though the
        // file on disk is correct (stale textDocument cache in the LSP server).
        for path in &lsp_files {
            ctx.lsp.notify_file_changed(path).await;
            ctx.agent.invalidate_call_edges(path).await;
            ctx.agent.mark_file_dirty(path.clone()).await;
        }

        // Phase 1.5: post-edit corruption scan.
        // If the LSP produced a wrong edit range (e.g. rust-analyzer off-by-N column), the
        // new name can end up embedded inside an existing token: "assertmy_new_fn()" instead
        // of "assert!(my_new_fn())". Detect this by checking whether any occurrence of
        // new_name in a changed file is immediately preceded by an alphanumeric character —
        // a separator (`_`, `(`, ` `, `:`, etc.) should always appear at a call/use site.
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

        // Phase 2: text sweep for remaining textual occurrences
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
            // Run the sweep on a blocking thread — it walks the full project
            // tree and reads each file, and we don't want to stall the tokio
            // runtime on a large monorepo.
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

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_rename_symbol(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}
