//! `edit_code` — unified symbol mutation: rename (LSP), remove, replace, insert.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::tools::{guard_worktree_write, require_str_param, RecoverableError, Tool, ToolContext};
use crate::util::text::reindent_to;

use super::display::{
    format_insert_code, format_remove_symbol, format_rename_symbol, format_replace_symbol,
};
use crate::fs::{
    get_lsp_client, guard_not_markdown, require_path_param, resolve_write_path_for, uri_to_path,
};
use crate::symbol::edit::{
    anchor_indent, apply_text_edits, clamp_range_to_parent, collect_all_name_paths,
    editing_end_line, editing_end_line_strict, editing_start_line, find_ast_name_path,
    find_parent_symbol, skip_lead_region, text_sweep, write_lines, CorruptionVerdict, LeadClass,
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
        "Mutate a symbol. action='replace' overwrites body. PRESERVES outer \
         #[...] attributes (drop with attributes:[] / set with attributes:[...] — \
         see schema). action='insert' injects code adjacent. action='remove' \
         deletes. action='rename' renames across codebase via LSP."
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
                "attributes": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "replace only: explicit outer-attribute list. When supplied, replaces ALL existing outer #[...] attributes (and any doc comments captured in the symbol's lead region) with exactly this list. Use [] to drop all attributes. Omit to keep the default preserve-when-body-has-no-leading-attribute heuristic. Each entry should be a complete attribute string, indented to match the symbol's column (e.g. \"    #[tokio::test]\")."
                },
                "position": {
                    "type": "string",
                    "enum": ["before", "after"],
                    "description": "insert only, default 'after'"
                },
                "at_line": {
                    "type": "integer",
                    "description": "Tie-breaker for two symbols whose name_path is byte-identical — two inherent impl-blocks for one type, or two #[cfg]-gated definitions of the same item. Reach for a more specific name_path FIRST; this is only for the case where no more specific name exists. Pass any 1-based line inside the one you mean, as symbols() prints it (declaration, body, or end all work). The ambiguity error lists each candidate's span for exactly this purpose. Ignored when the name already resolves to one symbol."
                }
            }
        })
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        let mut base = if result.get("files_changed").is_some() {
            Some(format_rename_symbol(result))
        } else if result.get("removed_lines").is_some() {
            Some(format_remove_symbol(result))
        } else if result.get("replaced_lines").is_some() {
            Some(format_replace_symbol(result))
        } else if result.get("inserted_at_line").is_some() {
            Some(format_insert_code(result))
        } else {
            None
        };
        // A repaired (truncated) LSP range must reach the caller in the COMPACT
        // form too — a warning only present in the raw JSON is a silent fix.
        if let (Some(s), Some(w)) = (base.as_mut(), result["warning"].as_str()) {
            s.push('\n');
            s.push_str(w);
        }
        if let (Some(s), Some(h)) = (base.as_mut(), result["hint"].as_str()) {
            s.push('\n');
            s.push_str(h);
        }
        base
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        guard_worktree_write(ctx).await?;
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let action = require_str_param(&input, "action")?;
        // Optional tie-breaker for two symbols whose name_paths are byte-identical
        // (two inherent impl blocks for one type, two #[cfg]-gated definitions).
        // No more specific name exists for those, so a line is the only key left.
        let at_line = input["at_line"].as_u64().map(|n| n as u32);

        match action {
            "rename" => {
                let Some(new_name) = input["new_name"].as_str() else {
                    return Err(RecoverableError::new("action 'rename' requires 'new_name'").into());
                };
                let mut result = self.do_rename(ctx, name_path, rel_path, new_name).await?;
                result["hint"] = json!(format!(
                    "verify callers: references(symbol=\"{}\", path=\"{}\")",
                    name_path, rel_path
                ));
                Ok(result)
            }
            "remove" => self.do_remove(ctx, name_path, rel_path, at_line).await,
            "replace" => {
                let Some(body) = input["body"].as_str() else {
                    return Err(RecoverableError::new("action 'replace' requires 'body'").into());
                };
                // Optional `attributes: Vec<String>` — when present (even if
                // empty) it overrides the body-leads-with-decorator heuristic
                // and replaces all outer attributes with the supplied list.
                // Empty array drops all attributes. Omitted (None) keeps the
                // default preserve-when-safe behavior. Closes U-19 + U-21
                // (codescout-usage-frictions tracker).
                let attributes: Option<Vec<String>> = match input.get("attributes") {
                    Some(Value::Array(arr)) => {
                        let mut out = Vec::with_capacity(arr.len());
                        for (i, v) in arr.iter().enumerate() {
                            let Some(s) = v.as_str() else {
                                return Err(RecoverableError::new(format!(
                                    "action 'replace': attributes[{i}] must be a string"
                                ))
                                .into());
                            };
                            out.push(s.to_string());
                        }
                        Some(out)
                    }
                    Some(Value::Null) | None => None,
                    Some(_) => {
                        return Err(RecoverableError::new(
                            "action 'replace': attributes must be an array of strings (or omitted)",
                        )
                        .into());
                    }
                };
                let mut result = self
                    .do_replace(
                        ctx,
                        name_path,
                        rel_path,
                        body,
                        attributes.as_deref(),
                        at_line,
                    )
                    .await?;
                result["hint"] = json!(format!(
                    "verify callers: references(symbol=\"{}\", path=\"{}\")",
                    name_path, rel_path
                ));
                Ok(result)
            }
            "insert" => {
                let Some(body) = input["body"].as_str() else {
                    return Err(RecoverableError::new("action 'insert' requires 'body'").into());
                };
                let position = input["position"].as_str().unwrap_or("after");
                self.do_insert(ctx, name_path, rel_path, body, position, at_line)
                    .await
            }
            _ => Err(RecoverableError::new(format!("unknown action '{action}'")).into()),
        }
    }
}

impl EditCode {
    /// Did the rename reach only the declaration, while other source files still name the
    /// symbol?
    ///
    /// True is the disagreement, not a certainty: the LSP resolving nothing outside the
    /// declaration file means either the symbol genuinely has no external users, or the
    /// language server did not resolve its cross-file references. `text_sweep` cannot tell
    /// those apart — but it can say that other *source* files spell the name, which in the
    /// second case is a tree that no longer builds.
    ///
    /// Two things this deliberately does NOT do, each of which was considered:
    ///
    /// * It does not fire on source matches alone. `TextualMatch::kind` classifies the FILE,
    ///   not the occurrence, so a comment mentioning the name inside a `.rs` file is
    ///   `kind: "source"` (`text_sweep_finds_matches_in_comments_and_docs`). Gating on that
    ///   would refuse a rename because a docstring says the word.
    /// * It does not widen the rename to cover them. Renaming every `kind: "source"` match
    ///   would rewrite comments and any unrelated symbol sharing the name — a silent
    ///   over-reach in place of a silent under-reach.
    ///
    /// The rule is `references`' own `completeness_warning` (`src/tools/symbol/references.rs`)
    /// applied to the write path, which had strictly more information and said nothing.
    /// See `docs/issues/2026-08-08-edit-code-rename-under-reaches-and-reports-ok.md`.
    pub(super) fn rename_under_reached(
        lsp_files: &std::collections::HashSet<PathBuf>,
        declaration: &std::path::Path,
        textual: &[crate::symbol::edit::TextualMatch],
    ) -> bool {
        lsp_files.iter().all(|p| p == declaration) && textual.iter().any(|m| m.kind == "source")
    }

    async fn do_rename(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
        new_name: &str,
    ) -> anyhow::Result<Value> {
        let full_path =
            resolve_write_path_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            ctx.workspace_override.as_deref(),
        )
        .await?;

        let symbols = client.document_symbols(&full_path, &lang).await?;
        let sym = find_unique_symbol_by_name_path(&symbols, name_path)?;

        let edit = client
            .rename(&full_path, sym.start_line, sym.start_col, new_name, &lang)
            .await?;

        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let security = ctx
            .agent
            .security_config_for(ctx.workspace_override.as_deref())
            .await;
        let session_roots = ctx
            .agent
            .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
            .await;
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
                &root,
                &security,
                &session_roots,
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
            ctx.agent
                .invalidate_call_edges_for(ctx.workspace_override.as_deref(), path)
                .await;
            ctx.agent
                .mark_file_dirty_for(ctx.workspace_override.as_deref(), path.clone())
                .await;
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
                    let rel = crate::util::fs::relative_forward_slash(path, &root);
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
        let (textual, sweep_skipped, sweep_skip_reason, textual_files_total) =
            if old_name_str.len() < 4 {
                (
                    vec![],
                    true,
                    Some(format!(
                        "name too short ({} chars, minimum 4)",
                        old_name_str.len()
                    )),
                    0,
                )
            } else {
                let sweep_root = root.clone();
                let sweep_name = old_name_str.to_string();
                let sweep_files = lsp_files.clone();
                let sweep_result = tokio::task::spawn_blocking(move || {
                    text_sweep(&sweep_root, &sweep_name, &sweep_files, 20, 2)
                })
                .await;
                match sweep_result {
                    Ok(Ok((matches, total_files))) => (matches, false, None::<String>, total_files),
                    Ok(Err(e)) => {
                        tracing::warn!("text sweep after rename failed: {e}");
                        (vec![], false, Some(format!("sweep error: {e}")), 0)
                    }
                    Err(join_err) => {
                        tracing::warn!("text sweep task join failed: {join_err}");
                        (
                            vec![],
                            false,
                            Some(format!("sweep task failed: {join_err}")),
                            0,
                        )
                    }
                }
            };

        let textual_total: usize = textual.iter().map(|m| m.occurrence_count).sum();
        let textual_shown = textual.len();

        // Did the rename actually reach the callers?
        //
        // The LSP resolving nothing outside the declaration file means one of two things:
        // the symbol genuinely has no external users, or the language server does not
        // resolve cross-file references for it — measured and reproducible on Kotlin for a
        // top-level `object`, with the tree compiling green in the same session. The sweep
        // cannot tell those apart. It CAN tell that other *source* files name the symbol,
        // and in the second case those files no longer compile.
        //
        // This is the rule `references` already applies on the read path (its
        // `completeness_warning`, `src/tools/symbol/references.rs`). The write path had
        // strictly more information in hand — the sweep, already classified — and returned
        // a bare `status: "ok"`. Porting the rule, not inventing one.
        //
        // Deliberately NOT gated on "any source match": `kind` classifies the FILE, not the
        // occurrence, so a comment mentioning the name in a .rs file is `kind: "source"`
        // (see `text_sweep_finds_matches_in_comments_and_docs`). Gating on that would fire
        // on a docstring. The narrow signal is the *disagreement*: LSP reached nothing
        // outside the declaration while source files say otherwise.
        //
        // Nor is the rename widened to cover them. Same reason: renaming every
        // `kind: "source"` match would rewrite comments and any unrelated symbol that
        // happens to share the name — trading a silent under-reach for a silent over-reach.
        // The writes below already landed; what was wrong was calling that a success.
        //
        // See `docs/issues/2026-08-08-edit-code-rename-under-reaches-and-reports-ok.md`.
        let under_reached = Self::rename_under_reached(&lsp_files, &full_path, &textual);
        let uncovered_sources: Vec<String> = textual
            .iter()
            .filter(|m| m.kind == "source")
            .map(|m| m.file.clone())
            .collect();

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
            "status": if under_reached { "incomplete" } else { "ok" },
            "old_name": old_name_str,
            "new_name": new_name,
            "files_changed": files_changed,
            "total_edits": total_edits,
            "textual_matches": textual_json,
            "textual_match_count": textual_total,
            "textual_matches_shown": textual_shown,
            "textual_files_total": textual_files_total,
            "textual_files_truncated": textual_files_total > textual_shown,
            "sweep_skipped": sweep_skipped,
            // The old hint pointed only at OVER-reach — string literals and comments inside
            // the files that changed. The failure that actually breaks a build is the
            // opposite one, in the files that did NOT change, and a caller following the
            // old text verifies the single edited file and concludes the rename is sound.
            // Name the under-reach first when it is present.
            "verify_hint": if under_reached {
                "INCOMPLETE: the rename edited only the declaration file, but source files listed \
                 in `completeness_warning` still reference the old name — they will not compile. \
                 Rename those occurrences too, then build. (The usual caution about over-reach \
                 into string literals and comments still applies to the file that did change.)"
            } else {
                "LSP rename may match occurrences inside string literals, comments, or macro arguments. Verify each changed file is still valid (e.g. cargo check / tsc --noEmit)."
            },
        });
        if under_reached {
            let shown = uncovered_sources
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let more = uncovered_sources.len().saturating_sub(4);
            result["completeness_warning"] = json!(format!(
                "the LSP resolved no reference outside the declaration file, so only that file \
                 was renamed — but `{old_name_str}` appears as a whole word in {n} other source \
                 file(s): {shown}{extra}. Either the symbol is genuinely unused and those are \
                 comments, or the language server under-reported and the tree no longer builds. \
                 Check with grep / call_graph(direction='callers') before treating this rename \
                 as done.",
                n = uncovered_sources.len(),
                extra = if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                },
            ));
            result["uncovered_source_files"] = json!(uncovered_sources);
        }
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
        at_line: Option<u32>,
    ) -> anyhow::Result<Value> {
        let full_path =
            resolve_write_path_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            ctx.workspace_override.as_deref(),
        )
        .await?;

        let (sym, symbols, range_repair) =
            fetch_validated_symbol(&client, &full_path, &lang, name_path, at_line).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start0 = editing_start_line(&sym, &lines);
        let end0 = (editing_end_line(&sym) as usize + 1).min(lines.len());

        let (start, end) = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
            let parent_body_start = parent.start_line as usize + 1;
            // `+ 1`: parent.end_line is the last line the parent node SPANS
            // (inclusive). For dedent-delimited languages (Python, ...) that is the
            // last child's last body line, so the first line NOT in the parent body
            // is `end_line + 1`. A bare `end_line` clamped a replace/remove of the
            // LAST child back by one, dropping its trailing statement — a sibling of
            // the do_insert fix (docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md).
            // Brace languages are unaffected: a child's end is always strictly below
            // the parent closer, so the clamp never binds.
            let parent_body_end_exclusive = parent.end_line as usize + 1;
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

        // `remove` had NO post-edit verification at all — it wrote and returned `ok`.
        // The reported corruption was therefore not a check that missed; it was a check
        // that did not exist on this path, while `replace` next door had one. Reported
        // once, when an AST-repaired range overshot the start by two lines and took the
        // preceding function's `)` and `}` with it, leaving the file invalid under
        // `status: "ok"`. See
        // docs/issues/2026-08-07-edit-code-remove-ast-repair-over-deletes.md.
        let pre_ast = crate::ast::extract_symbols(&full_path).ok();
        let pre_set = pre_ast.as_ref().map(|s| collect_all_name_paths(s));
        let target_ast_name_path = pre_ast
            .as_ref()
            .and_then(|s| find_ast_name_path(s, &sym.name, sym.start_line));

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;

        let post_ast = crate::ast::extract_symbols(&full_path);
        let extract_err = post_ast.as_ref().err().map(|e| e.to_string());
        let post_ast = post_ast.ok();

        let syntax_regressed = std::fs::read_to_string(&full_path)
            .ok()
            .zip(crate::ast::detect_language(&full_path))
            .map(|(post_src, l)| crate::symbol::edit::syntax_regressed(&content, &post_src, l))
            .unwrap_or(false);

        // `pre_count = 0` deliberately: dropping the target IS the operation here, so the
        // TargetDropped arm must never fire. What means something on a removal is that a
        // SIBLING vanished, or that the file stopped parsing.
        // A removal legitimately takes the target's own descendants with it, so they
        // are not part of the question the sibling check asks ("did the range
        // overshoot into ADJACENT code?"). Without this split, removing a non-empty
        // module reported its own children as dropped siblings — a wrong diagnosis of
        // a correct operation — and rolled the removal back, so a non-empty module
        // could not be removed at all.
        // docs/issues/2026-08-11-edit-code-cannot-remove-nonempty-module.md gap 2
        let (sibling_set, removed_descendants) =
            match (pre_set.as_ref(), target_ast_name_path.as_deref()) {
                (Some(set), Some(target)) => {
                    let (outside, descendants) =
                        crate::symbol::edit::split_target_subtree(set, target);
                    (Some(outside), descendants)
                }
                _ => (pre_set.clone(), Vec::new()),
            };

        let verdict = crate::symbol::edit::corruption_verdict(
            0,
            sibling_set.as_ref(),
            target_ast_name_path.as_deref(),
            &sym.name_path,
            post_ast.as_deref(),
            syntax_regressed,
        );

        let rollback_reason = match &verdict {
            CorruptionVerdict::SyntaxBroken => Some((
                format!(
                    "edit_code remove('{name_path}') left the file syntactically invalid — \
                     it parsed before this edit and does not parse after it. No sibling \
                     symbol was dropped, so the range most likely overshot into adjacent \
                     code and took a delimiter with it. File restored."
                ),
                "Re-read with symbols(path) to refresh the range, then retry; if it still \
                 looks wrong, narrow the edit via edit_file with unique anchors. Serialize \
                 write calls — parallel edit_code writes in one block can leave the LSP \
                 with a stale view (BUG-021).",
            )),
            CorruptionVerdict::SiblingsDropped(dropped) => Some((
                format!(
                    "edit_code remove('{name_path}') would have dropped sibling symbols: \
                     {}. The range overshot into adjacent code (likely a stale LSP range). \
                     File restored.",
                    dropped.join(", ")
                ),
                "Try symbols(path) to refresh, then retry; or narrow the edit via \
                 edit_file with unique anchors.",
            )),
            CorruptionVerdict::TargetDropped
            | CorruptionVerdict::Clean
            | CorruptionVerdict::Unverified => None,
        };

        if let Some((msg, hint)) = rollback_reason {
            write_lines(&full_path, &lines, content.ends_with('\n'))?;
            ctx.lsp.notify_file_changed(&full_path).await;
            ctx.agent
                .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &full_path)
                .await;
            ctx.agent
                .mark_file_dirty_for(ctx.workspace_override.as_deref(), full_path)
                .await;
            return Err(RecoverableError::with_hint(msg, hint).into());
        }

        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent
            .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &full_path)
            .await;
        ctx.agent
            .mark_file_dirty_for(ctx.workspace_override.as_deref(), full_path)
            .await;
        let line_count = end - start;
        let removed_range = format!("{}-{}", start + 1, end);
        let mut response = json!({
            "status": "ok",
            "removed_lines": removed_range,
            "line_count": line_count,
        });
        if let Some(r) = range_repair {
            response["warning"] = json!(r.warning(&sym.name));
        }
        // Naming what went with the target. These are expected — removing a module
        // removes its contents — but silence would leave the caller to discover the
        // scope of their own removal by re-reading the file.
        if !removed_descendants.is_empty() {
            response["removed_descendants"] = json!(removed_descendants);
        }
        if verdict == CorruptionVerdict::Unverified {
            response["corruption_check"] = json!("skipped");
            response["corruption_check_error"] = json!(
                extract_err.unwrap_or_else(|| "post-edit symbol extraction failed".to_string())
            );
            response["hint"] = json!(
                "The removal was written but NOT verified against dropped siblings. \
                 Re-read the file with symbols(path) to confirm it is intact."
            );
        }
        Ok(response)
    }

    async fn do_replace(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
        new_body: &str,
        attributes: Option<&[String]>,
        at_line: Option<u32>,
    ) -> anyhow::Result<Value> {
        let full_path =
            resolve_write_path_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            ctx.workspace_override.as_deref(),
        )
        .await?;

        let (sym, symbols, range_repair) =
            fetch_validated_symbol(&client, &full_path, &lang, name_path, at_line).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start0 = editing_start_line(&sym, &lines);
        let end0 = (editing_end_line(&sym) as usize + 1).min(lines.len());

        let (start, end) = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
            let parent_body_start = parent.start_line as usize + 1;
            // `+ 1`: parent.end_line is the last line the parent node SPANS
            // (inclusive). For dedent-delimited languages (Python, ...) that is the
            // last child's last body line, so the first line NOT in the parent body
            // is `end_line + 1`. A bare `end_line` clamped a replace/remove of the
            // LAST child back by one, dropping its trailing statement — a sibling of
            // the do_insert fix (docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md).
            // Brace languages are unaffected: a child's end is always strictly below
            // the parent closer, so the clamp never binds.
            let parent_body_end_exclusive = parent.end_line as usize + 1;
            clamp_range_to_parent(start0, end0, parent_body_start, parent_body_end_exclusive)
        } else {
            (start0, end0)
        };

        // U-19 + U-21 fix: when `attributes` is explicitly supplied (Some, even
        // empty), force `start = start0` so the full decorator+symbol range
        // gets replaced, and prepend the supplied attributes verbatim to the
        // new body. Skips the body_leads_with_decorator heuristic — the
        // caller's intent is explicit. `attributes: []` drops all outer
        // attributes; non-empty replaces them with exactly that list.
        let (start, effective_body): (usize, String) = if let Some(attrs) = attributes {
            let attrs_block = attrs.join("\n");
            let combined = if attrs_block.is_empty() {
                new_body.to_string()
            } else {
                format!("{attrs_block}\n{new_body}")
            };
            (start, combined)
        } else {
            // R-08 / BUG-031 reconciliation: editing_start_line walks back past
            // preceding doc comments / attributes so that a new_body containing
            // doc-comments+signature replaces them cleanly (no BUG-031 duplication).
            // But when the LLM passes a new_body that intentionally omits decorators
            // (e.g. only changing the body), that walk-back drops the existing doc
            // comment. Detect this: if new_body does NOT lead with a decorator,
            // narrow `start` forward past any leading decorator/attribute lines in
            // the captured range so they are preserved.
            //
            // Asking that as ONE all-or-nothing question silently dropped doc
            // comments: a body leading with `#[test]` (or even a plain `//`) was
            // treated as owning the entire lead region, so the file's `///` lines
            // fell inside the replaced range and were never re-emitted. The lead
            // region holds two independent classes -- documentation and attributes
            // -- so each is preserved unless the body supplies its own of that
            // class. See docs/issues/ for the doc-comment-drop report.
            let body_lead = new_body
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim_start());
            let body_supplies_docs = body_lead.is_some_and(|t| {
                t.starts_with("///")
                    || t.starts_with("//!")
                    || t.starts_with("/**")
                    || t.starts_with("/*")
            });
            let body_leads_with_decorator = body_lead.is_some_and(|t| {
                t.starts_with("///")
                    || t.starts_with("//!")
                    || t.starts_with("//")
                    || t.starts_with("#[")
                    || t.starts_with("/**")
                    || t.starts_with("/*")
                    || t.starts_with('@')
            });

            let start_narrowed = {
                let mut s = start;
                // Phase 1 -- documentation. Skipped past whenever the body brings
                // none of its own, including when the body leads with an attribute.
                // This is the phase the old single-question form never reached.
                if !body_supplies_docs {
                    s = skip_lead_region(&lines, s, end, LeadClass::Docs);
                }
                // Phase 2 -- attributes as well, but only when the body supplies no
                // lead region at all. A body opening with `#[...]` is stating its
                // own attributes, so the file's are its to replace. (Preserving
                // attributes past a body that DOES lead with a decorator is a
                // separate, accepted limitation -- the explicit `attributes` param
                // is the channel for it, and
                // archive/2026-05-18-edit-code-replace-misses-outer-attrs.md is
                // wontfix.)
                if !body_leads_with_decorator {
                    s = skip_lead_region(&lines, s, end, LeadClass::All);
                }
                s
            };
            (start_narrowed, new_body.to_string())
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

        // Re-base the replacement onto the symbol's own indentation column. The
        // caller often supplies a body dedented to column 0 (e.g. copied from a
        // `symbols` dump); splicing that verbatim into a nested symbol breaks
        // indentation — a hard IndentationError in Python, mis-aligned code in
        // brace languages like Kotlin. `reindent_to` is a no-op when the body is
        // already based at the target, so correctly-indented input is untouched.
        // See do_insert for why the column is sampled at the validated `start_line`
        // rather than at the editing range's start.
        let target_base = anchor_indent(&lines, sym.start_line as usize);
        let effective_body = reindent_to(&effective_body, &target_base);

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
        new_lines.extend(effective_body.lines());
        new_lines.extend_from_slice(&lines[end..]);

        write_lines(&full_path, &new_lines, content.ends_with('\n'))?;

        // Keep the re-extraction ERROR rather than discarding it with `.ok()` — when
        // the post-edit AST is unavailable the corruption checks cannot run, and the
        // agent needs to know that (omnibus 49ee6a03, F10).
        let post_ast = crate::ast::extract_symbols(&full_path);
        let extract_err = post_ast.as_ref().err().map(|e| e.to_string());
        let post_ast = post_ast.ok();

        // Read back what was actually written rather than re-joining `new_lines`: the
        // question is whether the FILE parses, and `write_lines` owns the trailing-newline
        // decision. Nothing to verify if the read fails — the name-set checks below still
        // run and `Unverified` still covers a failed re-extract.
        let syntax_regressed = std::fs::read_to_string(&full_path)
            .ok()
            .zip(crate::ast::detect_language(&full_path))
            .map(|(post_src, lang)| {
                crate::symbol::edit::syntax_regressed(&content, &post_src, lang)
            })
            .unwrap_or(false);

        let verdict = crate::symbol::edit::corruption_verdict(
            pre_count,
            pre_set.as_ref(),
            target_ast_name_path.as_deref(),
            &sym.name_path,
            post_ast.as_deref(),
            syntax_regressed,
        );

        // Both corrupting verdicts roll the file back identically; only the message differs.
        let rollback_reason = match &verdict {
            CorruptionVerdict::TargetDropped => Some((
                format!(
                    "edit_code replace('{name_path}') dropped the symbol definition — \
                     body must be the complete declaration (attributes, doc comments, \
                     signature, and body), not just body statements. File restored."
                ),
                "Use symbols(symbol=..., include_body=true) to see the expected format.",
            )),
            CorruptionVerdict::SiblingsDropped(dropped) => Some((
                format!(
                    "edit_code replace('{name_path}') would have dropped sibling symbols: {}. \
                     The edit range overshot into adjacent code (likely a stale LSP range). \
                     File restored.",
                    dropped.join(", ")
                ),
                "Try symbols(path) to refresh, then retry; or narrow the edit via \
                 edit_file with unique anchors.",
            )),
            // The verdict the name-set checks structurally cannot reach: an edit that
            // drops a closing delimiter loses no symbol name, so it used to land here as
            // `Clean` with `status: "ok"` and no rollback, leaving the file invalid.
            // Reported once, on a removal whose AST-repaired range overshot the start by
            // two lines and took the previous function's `)` and `}` with it.
            // See docs/issues/2026-08-07-edit-code-remove-ast-repair-over-deletes.md.
            CorruptionVerdict::SyntaxBroken => Some((
                format!(
                    "edit_code('{name_path}') left the file syntactically invalid — it \
                     parsed before this edit and does not parse after it. No symbol was \
                     dropped, so the edit most likely overshot into adjacent code and took \
                     a delimiter with it. File restored."
                ),
                "Re-read with symbols(path) to refresh the ranges, then retry; if the \
                 range still looks wrong, narrow the edit via edit_file with unique \
                 anchors. Serialize write calls — parallel edit_code writes in one block \
                 can leave the LSP with a stale view (BUG-021).",
            )),
            CorruptionVerdict::Clean | CorruptionVerdict::Unverified => None,
        };

        if let Some((msg, hint)) = rollback_reason {
            write_lines(&full_path, &lines, content.ends_with('\n'))?;
            ctx.lsp.notify_file_changed(&full_path).await;
            ctx.agent
                .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &full_path)
                .await;
            ctx.agent
                .mark_file_dirty_for(ctx.workspace_override.as_deref(), full_path)
                .await;
            return Err(RecoverableError::with_hint(msg, hint).into());
        }

        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent
            .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &full_path)
            .await;
        ctx.agent
            .mark_file_dirty_for(ctx.workspace_override.as_deref(), full_path)
            .await;
        let mut response =
            json!({ "status": "ok", "replaced_lines": format!("{}-{}", start + 1, end) });
        if let Some(r) = range_repair {
            response["warning"] = json!(r.warning(&sym.name));
        }
        // The write succeeded but the dropped-symbol / dropped-sibling checks could
        // not run. Say so — the previous code reported a bare "ok" here, which read
        // as "verified clean" when nothing had actually been verified.
        if verdict == CorruptionVerdict::Unverified {
            response["corruption_check"] = json!("skipped");
            response["corruption_check_error"] = json!(
                extract_err.unwrap_or_else(|| "post-edit symbol extraction failed".to_string())
            );
            response["hint"] = json!(
                "The edit was written but NOT verified against dropped symbols. \
                 Re-read the file with symbols(path) to confirm it is intact."
            );
        }
        Ok(response)
    }

    async fn do_insert(
        &self,
        ctx: &ToolContext,
        name_path: &str,
        rel_path: &str,
        code: &str,
        position: &str,
        at_line: Option<u32>,
    ) -> anyhow::Result<Value> {
        let full_path =
            resolve_write_path_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;
        guard_not_markdown(&full_path)?;
        let (client, lang) = get_lsp_client(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            ctx.workspace_override.as_deref(),
        )
        .await?;

        let (sym, symbols, range_repair) =
            fetch_validated_symbol(&client, &full_path, &lang, name_path, at_line).await?;

        let content = std::fs::read_to_string(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();
        // Re-base the inserted code onto the indentation of the symbol we are
        // inserting next to, so a sibling method/function lands at the right
        // column instead of wherever the caller happened to dedent it. No-op
        // when the code is already correctly based (e.g. top-level inserts).
        //
        // Sampled at `sym.start_line`, not at `editing_start_line`. Both name the
        // same symbol, but only `start_line` is validated on every fetch (see
        // `validate_symbol_position`); `editing_start_line` keys off
        // `range_start_line`, which nothing checks, and its documented
        // discard-the-walk-back path can leave it on a ` * ` block-comment
        // continuation whose leading whitespace is one column deeper than the
        // declaration it belongs to. The editing *range* still starts where
        // `editing_start_line` says — that is a different question from what column
        // this symbol sits at.
        let target_base = anchor_indent(&lines, sym.start_line as usize);
        let reindented = reindent_to(code, &target_base);
        let code_lines: Vec<&str> = reindented.lines().collect();
        let insert_at0 = match position {
            "before" => editing_start_line(&sym, &lines),
            _ => match editing_end_line_strict(&sym) {
                Some(end) => (end as usize + 1).min(lines.len()),
                None => {
                    // BUG-051 residual closure (2026-05-17): refuse universally
                    // when AST cannot pinpoint the symbol's end. The earlier
                    // parented-fallback path used the lenient `editing_end_line`
                    // (which falls back to LSP's `end_line`), but the parent
                    // body-end clamp below only catches over-extension — under-
                    // extension still landed inserts mid-body. Refusing in both
                    // top-level and parented cases prevents silent file
                    // corruption; the BUG-029 happy path is unaffected because
                    // it relies on AST returning Some, which strict still does.
                    //
                    // Diagnose the cause instead of guessing it. The old message
                    // asserted "AST parse failed" and hinted "the file likely has
                    // syntax errors" for EVERY cause — wrong whenever the file
                    // parses fine, which is the common case: a symbol the
                    // tree-sitter extractor does not surface at all (a method of an
                    // `impl` nested in a function body is the measured example, see
                    // `ast_does_not_expose_methods_of_an_impl_nested_in_a_function`)
                    // while the LSP resolves it happily. That hint sent a session
                    // hunting for syntax errors that did not exist.
                    let (msg, hint) = if crate::ast::has_syntax_errors(&content, &lang) {
                        (
                            format!(
                                "cannot determine end of '{}' for insert-after — the file has \
                                 syntax errors, so its AST is incomplete",
                                sym.name
                            ),
                            "Fix the syntax errors first, then retry — confirming a symbol's \
                             end line needs a clean parse.",
                        )
                    } else {
                        (
                            format!(
                                "cannot determine end of '{}' for insert-after — the file parses \
                                 cleanly, but the AST does not pinpoint this symbol's end",
                                sym.name
                            ),
                            "The AST extractor does not surface every symbol the LSP does; a \
                             method inside an impl-block nested in a function body is the known \
                             case. Insert-after refuses rather than trust the LSP end, which can \
                             be short and would splice the new code mid-body (BUG-051). Use \
                             edit_code(action='replace') on the ENCLOSING symbol to add the \
                             sibling, or edit_file with unique anchors.",
                        )
                    };
                    return Err(RecoverableError::with_hint(msg, hint).into());
                }
            },
        };

        let insert_at = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
            let parent_body_start = parent.start_line as usize + 1;
            // Tree-sitter's `end_line` is the last line the parent node SPANS
            // (inclusive): for brace languages that's the closer `}` line, for
            // dedent-delimited languages (Python, ...) it's the last child's last
            // body line. The first line NOT in the parent body is therefore
            // `end_line + 1`. Using a bare `end_line` here under-extended the
            // exclusive bound by one and, when inserting after the LAST child of a
            // Python-style class, clamped insert_at0 (strict-AST child_end + 1)
            // back into the child body — splicing the new sibling before the
            // child's trailing statement and orphaning it (2026-06-05 regression).
            // Brace languages are unaffected: a child's strict-AST end is always
            // strictly below the parent closer, so insert_at0 never reaches this
            // bound.
            //
            // `parent` comes from the raw LSP `symbols` list — never AST-repaired.
            // Trusting `parent.end_line` as-is re-introduces the exact truncation
            // this whole repair mechanism exists to fix, one level up: when the
            // LSP under-reports the PARENT's own end (e.g. its last child's tail
            // is a multi-line macro call, confusing the same LSP boundary logic
            // that motivated the child-level repair), the clamp silently drags a
            // correctly AST-resolved child insert position backward into the
            // child's own body (2026-07-19 regression: insert landed inside a
            // sibling's multi-line `assert_eq!` argument list). `editing_end_line`
            // re-resolves the parent's end from the AST the same way the child's
            // end already is, falling back to the raw LSP value only when AST
            // can't pin the parent down — never worse than the prior behavior.
            let parent_end = editing_end_line(parent);
            let parent_body_end_exclusive = parent_end as usize + 1;
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

        // Safety net + frictionless repair, shared with edit_file through
        // edit_repair::finalize_edit_content. do_insert has no LSP round-trip to
        // self-heal a malformed insert (unlike do_replace, which re-parses and
        // restores): an insert that introduces an unrepairable parse error is
        // rejected without writing, while an insert whose body arrived with
        // literal escape sequences (a backslash-n instead of a real newline) is
        // auto-decoded and written.
        let mut candidate = new_lines.join("\n");
        if content.ends_with('\n') && !candidate.is_empty() {
            candidate.push('\n');
        }
        let reassemble = |decoded_code: &str| -> String {
            let reindented = reindent_to(decoded_code, &target_base);
            let decoded_lines: Vec<&str> = reindented.lines().collect();
            let mut rl: Vec<&str> = Vec::new();
            rl.extend_from_slice(&lines[..insert_at]);
            rl.extend(decoded_lines.iter().copied());
            if position == "before" || lines.get(insert_at).is_some_and(|l| !l.trim().is_empty()) {
                rl.push("");
            }
            rl.extend_from_slice(&lines[insert_at..]);
            let mut out = rl.join("\n");
            if content.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            out
        };
        let (final_content, repaired) = match crate::tools::edit_repair::finalize_edit_content(
            &full_path, &content, candidate, code, reassemble,
        ) {
            crate::tools::edit_repair::RepairResult::Repaired(c) => (c, true),
            crate::tools::edit_repair::RepairResult::Clean(c) => (c, false),
            crate::tools::edit_repair::RepairResult::Introduced(_) => {
                return Err(RecoverableError::with_hint(
                    format!(
                        "inserting near '{}' would introduce syntax errors — not written",
                        sym.name
                    ),
                    "Check the inserted code's braces/indentation; verify the target with \
                     symbols(path) and retry.",
                )
                .into());
            }
        };

        crate::util::fs::atomic_write(&full_path, &final_content)?;
        ctx.lsp.notify_file_changed(&full_path).await;
        ctx.agent
            .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &full_path)
            .await;
        ctx.agent
            .mark_file_dirty_for(ctx.workspace_override.as_deref(), full_path)
            .await;
        let mut response =
            json!({ "status": "ok", "inserted_at_line": insert_at + 1, "position": position });
        if repaired {
            response["note"] = json!(crate::tools::edit_repair::REPAIR_NOTE);
        }
        if let Some(r) = range_repair {
            response["warning"] = json!(r.warning(&sym.name));
        }
        Ok(response)
    }
}
