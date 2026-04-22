//! `list_symbols` — symbol tree for a file, directory, or glob.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::ast;
use crate::tools::output::{OutputGuard, OverflowInfo};
use crate::tools::{optional_u64_param, parse_bool_param, RecoverableError, Tool, ToolContext};

use super::display::format_list_symbols;
use super::path_helpers::{
    format_library_path, get_lsp_client, get_path_param, is_glob, resolve_glob,
    resolve_library_roots, resolve_read_path, LspTimer,
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

pub struct ListSymbols;

#[async_trait::async_trait]
impl Tool for ListSymbols {
    fn name(&self) -> &str {
        "list_symbols"
    }
    fn description(&self) -> &str {
        "Symbol tree for a file, directory, or glob. Includes signatures. Pass include_docs=true for docstrings."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## When to use\n\
             \n\
             - Browse a file's structure → `list_symbols(path=\"src/foo.rs\")`.\n\
             - Explore an entire directory → `list_symbols(path=\"src/tools\")`.\n\
             - Need full bodies → `detail_level=\"full\"`; paginate with `offset`/`limit`.\n\
             \n\
             ## Key parameters\n\
             \n\
             - `path`: file, directory, or glob (e.g. `\"src/**/*.rs\"`). Defaults to `.`.\n\
             - `depth`: how many levels of children to include (0=none, 1=direct, default 1).\n\
             - `include_docs=true`: attach tree-sitter docstrings to each symbol.\n\
             - `scope`: `\"project\"` (default), `\"libraries\"`, `\"all\"`, or `\"lib:<name>\"`.\n\
             \n\
             ## Output\n\
             \n\
             Returns a file-by-file symbol tree with name, kind, and line range.\n\
             Single-file mode caps at 100 top-level symbols; use `offset`/`limit` to page.\n\
             \n\
             ## Tip\n\
             \n\
             After `list_symbols`, use `find_symbol(symbol=\"Struct/method\", include_body=true)` \
             to read a specific method body.",
        )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File, directory, or glob (e.g. 'src/**/*.rs')" },
                "depth": { "type": "integer", "default": 1, "description": "Children depth (0=none, 1=direct)" },
                "detail_level": { "type": "string", "description": "'full' for bodies (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset (files)" },
                "limit": { "type": "integer", "description": "Max files per page (default 50)" },
                "scope": { "type": "string", "description": "'project' (default), 'libraries', 'all', or 'lib:<name>'", "default": "project" },
                "include_docs": { "type": "boolean", "default": false, "description": "Include docstrings (tree-sitter)." },
                "force_mode": {
                    "type": "string",
                    "enum": ["auto", "symbols"],
                    "description": "Override mode selection. 'symbols' forces full symbol output regardless of directory size. Default: 'auto'."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
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
            let files = resolve_glob(ctx, rel_path).await?;
            let mut guard = guard;
            guard.max_files = guard.max_files.min(LIST_SYMBOLS_MAX_FILES);
            let (files, file_overflow) =
                guard.cap_files(files, "Narrow with a more specific glob or file path");
            let root = ctx.agent.require_project_root().await?;
            let include_body = guard.should_include_body();
            let mut result = vec![];
            for file_path in &files {
                let Some(lang) = ast::detect_language(file_path) else {
                    continue;
                };
                let language_id = crate::lsp::servers::lsp_language_id(lang);
                let mux_override = ctx.agent.lsp_mux_override(lang).await;
                if let Ok(client) = ctx.lsp.get_or_start(lang, &root, mux_override).await {
                    let timer = LspTimer::start();
                    if let Ok(symbols) = client.document_symbols(file_path, language_id).await {
                        timer.record(ctx, lang, &root).await;
                        let rel = file_path.strip_prefix(&root).unwrap_or(file_path);
                        let source = if include_body {
                            std::fs::read_to_string(file_path).ok()
                        } else {
                            None
                        };
                        let json_symbols: Vec<Value> = symbols
                            .iter()
                            .map(|s| {
                                symbol_to_json(s, include_body, source.as_deref(), depth, false)
                            })
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
                }
            }
            let mut result_json = json!({ "pattern": rel_path, "files": result });
            if let Some(ov) = file_overflow {
                result_json["overflow"] = OutputGuard::overflow_json(&ov);
            }
            return Ok(result_json);
        }

        let full_path = resolve_read_path(ctx, rel_path).await?;

        if full_path.is_file() {
            let raw_lang = ast::detect_language(&full_path)
                .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
            let root = ctx.agent.require_project_root().await?;
            let (client, lang) = get_lsp_client(ctx, &full_path).await?;
            let timer = LspTimer::start();
            let symbols = client.document_symbols(&full_path, &lang).await?;
            timer.record(ctx, raw_lang, &root).await;
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
            let json_symbols = if raw_lang == "bash" {
                filter_variable_symbols(json_symbols)
            } else {
                json_symbols
            };

            // Cap single-file results to prevent large files blowing the context window.
            // Primary check: flat count (top-level + depth-1 children combined).
            // A file with 50 impl blocks each containing 5 methods has 300 flat entries
            // even though it shows "50 symbols" — the flat cap catches that case.
            let total = json_symbols.len();
            let flat_total = flat_symbol_count(&json_symbols);
            let (json_symbols, overflow) = if flat_total > LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP {
                // Greedily include top-level symbols within the flat budget.
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
                     find_symbol(symbol='...', include_body=true) for a specific symbol."
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
                     or find_symbol(symbol='ClassName/methodName', include_body=true) for a specific symbol."
                );
                file_guard.cap_items(json_symbols, &hint)
            };
            if let Some(ov) = overflow {
                let total = ov.total;
                let mut result =
                    json!({ "file": rel_path, "symbols": json_symbols, "total": total });
                result["overflow"] = OutputGuard::overflow_json(&ov);
                if include_docs {
                    result["docstrings"] = json!(collect_docstrings(&full_path));
                }
                return Ok(result);
            }
            let mut result = json!({ "file": rel_path, "symbols": json_symbols });
            if include_docs {
                result["docstrings"] = json!(collect_docstrings(&full_path));
            }
            Ok(result)
        } else if full_path.is_dir() {
            let root = ctx.agent.require_project_root().await?;
            let force_symbols = input["force_mode"].as_str() == Some("symbols");
            let (total_files, subdir_counts) = count_files_by_subdir(&root, &full_path);

            // Flat dir, small tree, or forced → full symbol mode
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
                                timer.record(ctx, lang, &root).await;
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
                     Drill down with list_symbols('<subdir>') for full symbols, or \
                     list_symbols('{rel_path}/**/*') to scan the full tree.",
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
                 Drill down with list_symbols('<subdir>') or use \
                 list_symbols('{rel_path}/**/*') to scan the full tree with file cap.",
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
                "Verify the path exists with list_dir.",
            )
            .into())
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_list_symbols(result))
    }
}
