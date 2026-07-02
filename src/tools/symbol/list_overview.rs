//! Path-only-no-name overview path (formerly `list_symbols`).
//!
//! Implements the file/directory/glob symbol-tree overview that `symbols` falls
//! back to when no name search (`name`/`query`/`symbol`/`name_path`) is provided.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::ast;
use crate::tools::output::{OutputGuard, OverflowInfo};
use crate::tools::{optional_u64_param, parse_bool_param, RecoverableError, ToolContext};

use crate::fs::{
    format_library_path, get_path_param, is_glob, resolve_glob_for, resolve_library_roots,
    resolve_read_path_for, retry_on_mux_disconnect, LspTimer,
};
use crate::symbol::query::{filter_variable_symbols, symbol_to_json};

/// Directory/glob scans can produce huge output (each file has many symbols).
/// Cap exploring-mode file count lower than the global OutputGuard default (200).
pub(super) const LIST_SYMBOLS_MAX_FILES: usize = 50;
/// Hard cap on top-level symbols (fallback when flat count is within budget).
pub(super) const LIST_SYMBOLS_SINGLE_FILE_CAP: usize = 100;
/// Cap on *total* symbol entries including depth-1 children.
/// A single `impl` block with 10 methods counts as 11 flat entries, so the
/// flat budget prevents depth-1 output from ballooning even on rich files.
pub(super) const LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP: usize = 150;

/// File count below which directory mode returns full symbols (recursive walk).
pub(super) const LIST_SYMBOLS_RECURSE_SMALL: usize = 30;
/// File count below which directory mode returns AST class names per subdir.
pub(super) const LIST_SYMBOLS_RECURSE_MEDIUM: usize = 80;
/// Max immediate subdirectories shown in directory_map mode.
pub(super) const LIST_SYMBOLS_MAX_SUBDIRS: usize = 15;

/// Count top-level symbols plus their direct children (depth-1 children).
pub(super) fn flat_symbol_count(symbols: &[Value]) -> usize {
    symbols
        .iter()
        .map(|s| 1 + s["children"].as_array().map(|c| c.len()).unwrap_or(0))
        .sum()
}

/// Collapse single-child pass-through directories to find the first meaningful
/// branch point. A pass-through dir has zero direct source files and exactly one
/// immediate subdirectory. Stops when multiple children, direct files present,
/// or max depth (10) reached.
pub(super) fn find_split_point(dir: &Path) -> PathBuf {
    fn is_code_file(path: &Path) -> bool {
        matches!(
            ast::detect_language(path),
            Some(lang) if lang != "markdown"
        )
    }

    fn inner(dir: &Path, depth: usize) -> PathBuf {
        if depth > 10 {
            return dir.to_path_buf();
        }
        let direct_files = ignore::WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(true)
            .git_ignore(true)
            .build()
            .flatten()
            .filter(|e| {
                e.file_type().map(|t| t.is_file()).unwrap_or(false) && is_code_file(e.path())
            })
            .count();

        if direct_files > 0 {
            return dir.to_path_buf();
        }

        let subdirs: Vec<PathBuf> = ignore::WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(true)
            .git_ignore(true)
            .build()
            .flatten()
            .filter(|e| e.depth() == 1 && e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect();

        if subdirs.len() == 1 {
            inner(&subdirs[0], depth + 1)
        } else {
            dir.to_path_buf()
        }
    }
    inner(dir, 0)
}

/// Count source files in `dir` recursively, grouped by immediate subdirectory
/// of the meaningful split point (see `find_split_point`).
/// Returns `(total, Vec<(display_path, count)>)` sorted descending by count.
/// Files directly in the split point contribute to total but not to subdirs.
pub(super) fn count_files_by_subdir(
    project_root: &Path,
    dir: &Path,
) -> (usize, Vec<(String, usize)>) {
    let split = find_split_point(dir);

    let walker = ignore::WalkBuilder::new(&split)
        .max_depth(None)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut total = 0usize;
    let mut subdir_counts: std::collections::HashMap<PathBuf, usize> =
        std::collections::HashMap::new();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        match ast::detect_language(entry.path()) {
            Some(lang) if lang != "markdown" => {}
            _ => continue,
        }
        total += 1;
        let abs = entry.path().to_path_buf();
        if let Ok(rel) = abs.strip_prefix(&split) {
            let components: Vec<_> = rel.components().collect();
            if components.len() > 1 {
                let first = split.join(components[0].as_os_str());
                *subdir_counts.entry(first).or_insert(0) += 1;
            }
        }
    }

    let mut subdirs: Vec<(String, usize)> = subdir_counts
        .into_iter()
        .map(|(abs_path, count)| {
            let display = abs_path
                .strip_prefix(project_root)
                .unwrap_or(&abs_path)
                .display()
                .to_string();
            (display, count)
        })
        .collect();
    subdirs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    (total, subdirs)
}

/// Extract top-level class-like symbol names from source files directly in `dir`
/// (depth 1, no recursion). Uses tree-sitter AST only — no LSP.
/// Kinds included: Class, Struct, Interface, Enum, Object.
/// Returns sorted, deduplicated names.
pub(super) fn ast_class_names_for_dir(dir: &Path) -> Vec<String> {
    use crate::lsp::symbols::SymbolKind;

    let walker = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if ast::detect_language(entry.path()).is_none() {
            continue;
        }
        if let Ok(symbols) = ast::extract_symbols(entry.path()) {
            for sym in &symbols {
                match sym.kind {
                    SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Interface
                    | SymbolKind::Enum
                    | SymbolKind::Object => {
                        names.insert(sym.name.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    result
}

/// Path-only-no-name overview entry point (formerly `ListSymbols::call`).
///
/// Invoked by `Symbols::call` when the input has no `query`/`symbol`/`name`/`name_path`.
/// Response shape matches the legacy `list_symbols` output.
pub(super) async fn list_overview(input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
    let rel_path = get_path_param(&input, false)?.unwrap_or(".");
    let depth = optional_u64_param(&input, "depth").unwrap_or(1) as usize;
    let guard = OutputGuard::from_input(&input);
    let include_docs = parse_bool_param(&input["include_docs"]);
    let scope = crate::library::scope::Scope::parse(input["scope"].as_str());

    // Helper: collect docstrings for a file path as a JSON array
    let collect_docstrings = |path: &std::path::Path| -> Vec<Value> {
        crate::ast::extract_docstrings(path)
            .unwrap_or_default()
            .iter()
            .map(|d| {
                json!({
                    "symbol_name": d.symbol_name,
                    "content": d.content,
                    "start_line": d.start_line + 1,
                    "end_line": d.end_line + 1,
                })
            })
            .collect()
    };

    // If the path contains glob metacharacters, expand and aggregate
    if is_glob(rel_path) {
        let files =
            resolve_glob_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;
        let mut guard = guard;
        guard.max_files = guard.max_files.min(LIST_SYMBOLS_MAX_FILES);
        let (files, file_overflow) =
            guard.cap_files(files, "Narrow with a more specific glob or file path");
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let include_body = guard.should_include_body();
        let mut result = vec![];
        for file_path in &files {
            let Some(lang) = ast::detect_language(file_path) else {
                continue;
            };
            let language_id = crate::lsp::servers::lsp_language_id(lang);
            let mux_override = ctx.agent.lsp_mux_override(lang).await;
            let budget_client = crate::lsp::client_within_budget(
                ctx.lsp.clone(),
                lang,
                &root,
                mux_override,
                crate::lsp::LSP_FIRST_CALL_BUDGET,
            )
            .await;
            if let Some(client) = budget_client {
                let timer = LspTimer::start();
                if let Ok(symbols) = client.document_symbols(file_path, language_id).await {
                    timer.record(&*ctx.lsp, lang, &root).await;
                    let rel = file_path.strip_prefix(&root).unwrap_or(file_path);
                    let source = if include_body {
                        std::fs::read_to_string(file_path).ok()
                    } else {
                        None
                    };
                    let json_symbols: Vec<Value> = symbols
                        .iter()
                        .map(|s| symbol_to_json(s, include_body, source.as_deref(), depth, false))
                        .collect();
                    let json_symbols = if lang == "bash" {
                        filter_variable_symbols(json_symbols)
                    } else {
                        json_symbols
                    };
                    let mut entry = json!({
                        "file": rel.display().to_string(),
                        "symbols": json_symbols,
                    });
                    if include_docs {
                        entry["docstrings"] = json!(collect_docstrings(file_path));
                    }
                    result.push(entry);
                }
            } else if let Ok(symbols) = crate::ast::extract_symbols(file_path) {
                // LSP still warming: serve tree-sitter so the overview is not
                // blocked or silently missing files; mark the entry.
                let rel = file_path.strip_prefix(&root).unwrap_or(file_path);
                let source = if include_body {
                    std::fs::read_to_string(file_path).ok()
                } else {
                    None
                };
                let json_symbols: Vec<Value> = symbols
                    .iter()
                    .map(|s| symbol_to_json(s, include_body, source.as_deref(), depth, false))
                    .collect();
                let json_symbols = if lang == "bash" {
                    filter_variable_symbols(json_symbols)
                } else {
                    json_symbols
                };
                let mut entry = json!({
                    "file": rel.display().to_string(),
                    "symbols": json_symbols,
                    "lsp": "warming",
                });
                if include_docs {
                    entry["docstrings"] = json!(collect_docstrings(file_path));
                }
                result.push(entry);
            }
        }
        let mut result_json = json!({ "pattern": rel_path, "files": result });
        if let Some(ov) = file_overflow {
            result_json["overflow"] = OutputGuard::overflow_json(&ov);
        }
        return Ok(result_json);
    }

    let full_path =
        resolve_read_path_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;

    if full_path.is_file() {
        let raw_lang = ast::detect_language(&full_path)
            .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let mux_override = ctx.agent.lsp_mux_override(raw_lang).await;
        let lang = crate::lsp::servers::lsp_language_id(raw_lang).to_string();
        let mut lsp_warming = false;
        let symbols = match crate::lsp::client_within_budget(
            ctx.lsp.clone(),
            raw_lang,
            &root,
            mux_override,
            crate::lsp::LSP_FIRST_CALL_BUDGET,
        )
        .await
        {
            Some(client) => {
                let timer = LspTimer::start();
                // I-4: single-retry on transient LSP-mux disconnect (covers Kotlin LSP
                // eviction churn). Closure is idempotent — document_symbols is a pure
                // read of the LSP-side index.
                let symbols = retry_on_mux_disconnect(
                    &ctx.agent,
                    &*ctx.lsp,
                    &full_path,
                    ctx.workspace_override.as_deref(),
                    client,
                    lang.clone(),
                    |c, l| {
                        let p = full_path.clone();
                        async move { c.document_symbols(&p, &l).await }
                    },
                )
                .await?;
                timer.record(&*ctx.lsp, raw_lang, &root).await;
                symbols
            }
            None => {
                // LSP cold / not configured: serve tree-sitter now; the detached
                // warm-up (if a server exists) makes the next call LSP-grade.
                lsp_warming = true;
                ast::extract_symbols(&full_path)?
            }
        };
        // BUG-054 mitigation: rust-analyzer (and similar LSPs) return Ok(vec![])
        // during cold-start indexing instead of -32800 RequestCancelled,
        // bypassing the cold-start retry budget in `LspClient`. When LSP returns
        // no symbols for a non-empty source file we have a tree-sitter parser
        // for, fall over to AST extraction so the agent gets a usable result
        // instead of a silent empty array.
        let symbols = if symbols.is_empty() && ast::get_ts_language(raw_lang).is_some() {
            let has_source = std::fs::read_to_string(&full_path)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if has_source {
                ast::extract_symbols(&full_path).unwrap_or(symbols)
            } else {
                symbols
            }
        } else {
            symbols
        };
        let include_body = guard.should_include_body();
        let source = if include_body {
            std::fs::read_to_string(&full_path).ok()
        } else {
            None
        };
        let json_symbols: Vec<Value> = symbols
            .iter()
            .map(|s| symbol_to_json(s, include_body, source.as_deref(), depth, false))
            .collect();
        let mut json_symbols = if raw_lang == "bash" {
            filter_variable_symbols(json_symbols)
        } else {
            json_symbols
        };
        // Attach each symbol's own docstring inline (in addition to the flat
        // file-level `docstrings` array below) so a single-file overview surfaces
        // docs per-symbol.
        if include_docs {
            attach_docs_from_array(&mut json_symbols, &collect_docstrings(&full_path));
        }

        // Cap single-file results to prevent large files blowing the context window.
        let total = json_symbols.len();
        let flat_total = flat_symbol_count(&json_symbols);
        let (json_symbols, overflow) = if flat_total > LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP {
            let mut budget = LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP;
            let mut capped: Vec<Value> = Vec::new();
            for sym in json_symbols {
                let cost = 1 + sym["children"].as_array().map(|c| c.len()).unwrap_or(0);
                if cost <= budget {
                    budget -= cost;
                    capped.push(sym);
                } else {
                    break;
                }
            }
            let shown = capped.len();
            let hint = format!(
                "File has {total} top-level symbols ({flat_total} total including children). \
                 Use depth=0 for a top-level-only overview, or \
                 symbols(symbol='...', include_body=true) for a specific symbol."
            );
            let ov = OverflowInfo {
                shown,
                total,
                hint,
                next_offset: None,
                by_file: None,
                by_file_overflow: 0,
            };
            (capped, Some(ov))
        } else {
            let mut file_guard = guard;
            file_guard.max_results = LIST_SYMBOLS_SINGLE_FILE_CAP;
            let hint = format!(
                "File has {total} symbols. Use depth=0 for top-level overview, \
                 or symbols(symbol='ClassName/methodName', include_body=true) for a specific symbol."
            );
            file_guard.cap_items(json_symbols, &hint)
        };
        if let Some(ov) = overflow {
            let total = ov.total;
            let mut result = json!({ "file": rel_path, "symbols": json_symbols, "total": total });
            result["overflow"] = OutputGuard::overflow_json(&ov);
            if include_docs {
                result["docstrings"] = json!(collect_docstrings(&full_path));
            }
            if lsp_warming {
                result["lsp"] = json!("warming");
                result["hint"] = json!(
                    "Language server is starting; symbols served from tree-sitter. \
                     Re-run shortly for LSP-grade detail."
                );
            }
            return Ok(result);
        }
        let mut result = json!({ "file": rel_path, "symbols": json_symbols });
        if include_docs {
            result["docstrings"] = json!(collect_docstrings(&full_path));
        }
        if lsp_warming {
            result["lsp"] = json!("warming");
            result["hint"] = json!(
                "Language server is starting; symbols served from tree-sitter. \
                 Re-run shortly for LSP-grade detail."
            );
        }
        Ok(result)
    } else if full_path.is_dir() {
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let force_symbols = input["force_mode"].as_str() == Some("symbols");
        let (total_files, subdir_counts) = count_files_by_subdir(&root, &full_path);

        let use_symbol_mode = force_symbols
            || total_files == 0
            || total_files <= LIST_SYMBOLS_RECURSE_SMALL
            || subdir_counts.is_empty();

        if use_symbol_mode {
            let mut dir_files: Vec<(String, PathBuf)> = vec![];

            if scope.includes_project() {
                let walker = ignore::WalkBuilder::new(&full_path)
                    .max_depth(None)
                    .hidden(true)
                    .git_ignore(true)
                    .build();
                for entry in walker.flatten() {
                    if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        continue;
                    }
                    if ast::detect_language(entry.path()).is_none() {
                        continue;
                    }
                    let abs = entry.path().to_path_buf();
                    let display = abs
                        .strip_prefix(&root)
                        .unwrap_or(&abs)
                        .display()
                        .to_string();
                    dir_files.push((display, abs));
                }
            }

            let lib_roots = resolve_library_roots(&scope, &ctx.agent).await?;
            for (lib_name, lib_root) in &lib_roots {
                let walker = ignore::WalkBuilder::new(lib_root)
                    .max_depth(None)
                    .hidden(true)
                    .git_ignore(false)
                    .build();
                for entry in walker.flatten() {
                    if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        continue;
                    }
                    if ast::detect_language(entry.path()).is_none() {
                        continue;
                    }
                    let abs = entry.path().to_path_buf();
                    let display = format_library_path(lib_name, lib_root, &abs);
                    dir_files.push((display, abs));
                }
            }

            let mut guard = guard;
            guard.max_files = guard.max_files.min(LIST_SYMBOLS_MAX_FILES);
            let (dir_files, file_overflow) =
                guard.cap_files(dir_files, "Narrow with a more specific glob or file path");
            let include_body = guard.should_include_body();

            let mut result = vec![];
            for (display_path, abs_path) in &dir_files {
                let Some(lang) = ast::detect_language(abs_path) else {
                    continue;
                };
                let language_id = crate::lsp::servers::lsp_language_id(lang);

                let mux_override = ctx.agent.lsp_mux_override(lang).await;
                let mut symbols =
                    if let Ok(client) = ctx.lsp.get_or_start(lang, &root, mux_override).await {
                        let timer = LspTimer::start();
                        let syms = client
                            .document_symbols(abs_path, language_id)
                            .await
                            .unwrap_or_default();
                        if !syms.is_empty() {
                            timer.record(&*ctx.lsp, lang, &root).await;
                        }
                        syms
                    } else {
                        vec![]
                    };

                if symbols.is_empty() {
                    symbols = crate::ast::extract_symbols(abs_path).unwrap_or_default();
                }

                if symbols.is_empty() {
                    continue;
                }

                let source = if include_body {
                    std::fs::read_to_string(abs_path).ok()
                } else {
                    None
                };
                let json_symbols: Vec<Value> = symbols
                    .iter()
                    .map(|s| {
                        symbol_to_json(
                            s,
                            include_body,
                            source.as_deref(),
                            depth.saturating_sub(1),
                            false,
                        )
                    })
                    .collect();
                let mut entry = json!({
                    "file": display_path,
                    "symbols": json_symbols,
                });
                if include_docs {
                    entry["docstrings"] = json!(collect_docstrings(abs_path));
                }
                result.push(entry);
            }
            let mut result_json = json!({ "directory": rel_path, "files": result });
            if let Some(ov) = file_overflow {
                result_json["overflow"] = OutputGuard::overflow_json(&ov);
            }
            return Ok(result_json);
        }

        // class_overview mode: 31–80 files, has subdirs
        if total_files <= LIST_SYMBOLS_RECURSE_MEDIUM {
            let subdirs_json: Vec<Value> = subdir_counts
                .iter()
                .map(|(path, count)| {
                    let subdir_abs = root.join(path);
                    let classes = ast_class_names_for_dir(&subdir_abs);
                    json!({
                        "path": path,
                        "file_count": count,
                        "classes": classes,
                    })
                })
                .collect();
            let hint = format!(
                "Found {total_files} files across {} directories — showing top-level classes (AST). \
                 Drill down with symbols('<subdir>') for full symbols, or \
                 symbols('{rel_path}/**/*') to scan the full tree.",
                subdir_counts.len()
            );
            return Ok(json!({
                "directory": rel_path,
                "mode": "class_overview",
                "subdirectories": subdirs_json,
                "total_files": total_files,
                "hint": hint,
            }));
        }

        // directory_map mode: > 80 files
        let shown_subdirs: Vec<Value> = subdir_counts
            .iter()
            .take(LIST_SYMBOLS_MAX_SUBDIRS)
            .map(|(path, count)| json!({ "path": path, "file_count": count }))
            .collect();

        let overflow = if subdir_counts.len() > LIST_SYMBOLS_MAX_SUBDIRS {
            Some(json!({
                "shown": LIST_SYMBOLS_MAX_SUBDIRS,
                "total": subdir_counts.len(),
                "hint": format!(
                    "Showing {} of {} directories (largest first).",
                    LIST_SYMBOLS_MAX_SUBDIRS,
                    subdir_counts.len()
                ),
            }))
        } else {
            None
        };

        let hint = format!(
            "Found {total_files} files across {} directories — too large for symbol overview. \
             Drill down with symbols('<subdir>') or use \
             symbols('{rel_path}/**/*') to scan the full tree with file cap.",
            subdir_counts.len()
        );

        let mut result = json!({
            "directory": rel_path,
            "mode": "directory_map",
            "subdirectories": shown_subdirs,
            "total_files": total_files,
            "hint": hint,
        });
        if let Some(ov) = overflow {
            result["overflow"] = ov;
        }
        Ok(result)
    } else {
        Err(RecoverableError::with_hint(
            format!(
                "path is neither file nor directory: {}",
                full_path.display()
            ),
            "Verify the path exists with tree.",
        )
        .into())
    }
}

/// Attach each symbol's own docstring (matched by `symbol_name`) as a `docs`
/// field, recursing into children. Surfaces docs per-symbol in overview rather
/// than only as the flat file-level `docstrings` array.
pub(crate) fn attach_docs_from_array(symbols: &mut [Value], docstrings: &[Value]) {
    for sym in symbols.iter_mut() {
        let Some(obj) = sym.as_object_mut() else {
            continue;
        };
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()).map(str::to_string) {
            if let Some(content) = docstrings
                .iter()
                .find(|d| d["symbol_name"].as_str() == Some(name.as_str()))
                .and_then(|d| d["content"].as_str())
            {
                obj.insert("docs".to_string(), json!(content));
            }
        }
        if let Some(children) = obj.get_mut("children").and_then(|c| c.as_array_mut()) {
            attach_docs_from_array(children, docstrings);
        }
    }
}
