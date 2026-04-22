//! `find_symbol` — find symbols by name pattern across the project.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::ast;
use crate::lsp::SymbolInfo;
use crate::tools::output::{OutputGuard, OutputMode};
use crate::tools::{
    is_regex_like, optional_bool_param, optional_u64_param, RecoverableError, Tool, ToolContext,
};

use super::display::format_find_symbol;
use super::path_helpers::{
    format_library_path, get_path_param, is_glob, resolve_glob, resolve_library_roots, LspTimer,
};
use crate::symbol::query::{
    collect_matching, matches_kind_filter, resolve_range_via_document_symbols, symbol_name_matches,
    symbol_to_json, validate_symbol_range,
};

pub struct FindSymbol;

const FIND_SYMBOL_MAX_RESULTS: usize = 50;
const BY_FILE_CAP: usize = 15;

/// Build a per-file distribution from a list of symbol JSON objects.
/// Returns (entries sorted by count desc, number of files omitted by cap).
pub(super) fn build_by_file(matches: &[Value]) -> (Vec<(String, usize)>, usize) {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for m in matches {
        if let Some(file) = m["file"].as_str() {
            *counts.entry(file.to_string()).or_default() += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let overflow = sorted.len().saturating_sub(BY_FILE_CAP);
    sorted.truncate(BY_FILE_CAP);
    (sorted, overflow)
}

/// Build the actionable overflow hint for find_symbol. Uses the top file from by_file
/// as the concrete example path so the hint is copy-paste ready.
pub(super) fn make_find_symbol_hint(shown: usize, by_file: &[(String, usize)]) -> String {
    let top_file = by_file
        .first()
        .map(|(f, _)| f.as_str())
        .unwrap_or("path/to/file.rs");
    format!(
        "Showing {shown} of total. To narrow down:\n\
         \u{2022} paginate:       add offset={shown}, limit=50\n\
         \u{2022} filter by file: add path=\"{top_file}\"\n\
         \u{2022} filter by kind: add kind=\"function\" (also: class, struct, interface, type, enum, module, constant)"
    )
}

#[async_trait::async_trait]
impl Tool for FindSymbol {
    fn name(&self) -> &str {
        "find_symbol"
    }
    fn description(&self) -> &str {
        "Find symbols by name pattern across the project. Returns matching symbols with location."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## When to use\n\
             \n\
             - Know the name → use `find_symbol` (substring match on symbol names).\n\
             - Know the concept → use `semantic_search` first, then drill into symbols.\n\
             - Need all symbols in a file → use `list_symbols` instead.\n\
             \n\
             ## Key parameters\n\
             \n\
             - `query`: substring match (e.g. `\"handle\"` finds `handle_request`, `handle_error`).\n\
             - `symbol`: exact name-path (e.g. `\"MyStruct/my_method\"`) — skips substring search, ignores `kind`.\n\
             - `kind`: filter to `function`, `struct`, `interface`, `enum`, `module`, `constant`, `type`, `class`.\n\
             - `include_body=true`: returns full source of each match.\n\
             - `path`: restrict to a file or glob (e.g. `\"src/tools/**/*.rs\"`).\n\
             \n\
             ## Output and pagination\n\
             \n\
             Exploring mode returns up to 50 results with a `by_file` distribution map.\n\
             Use `detail_level=\"full\"` + `offset`/`limit` to page through large result sets.\n\
             \n\
             ## Gotchas\n\
             \n\
             - Regex patterns are rejected — use plain substrings. Use `grep` for text search.\n\
             - `kind` is ignored when `symbol` (name-path) is provided.\n\
             - LSP must be running for body extraction; tree-sitter fallback gives signatures only.",
        )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Symbol name or substring to search for" },
                "symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method'). Alternative to query." },
                "path": { "type": "string", "description": "File or glob to restrict search (e.g. 'src/**/*.rs')" },
                "kind": {
                    "type": "string",
                    "description": "Filter by kind (interface = Rust traits).",
                    "enum": ["function", "class", "struct", "interface", "type", "enum", "module", "constant"]
                },
                "include_body": { "type": "boolean", "default": false },
                "depth": { "type": "integer", "default": 0, "description": "Children depth to include" },
                "detail_level": { "type": "string", "description": "'full' for bodies (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "limit": { "type": "integer", "description": "Max results (default 50)" },
                "scope": { "type": "string", "description": "'project' (default), 'libraries', 'all', or 'lib:<name>'", "default": "project" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let pattern = input["query"]
            .as_str()
            .or_else(|| input["symbol"].as_str())
            .or_else(|| input["name"].as_str()) // common LLM alias
            .ok_or_else(|| {
                // List the keys the LLM actually sent so it can self-correct.
                let got_keys: Vec<&str> = input
                    .as_object()
                    .map(|o| o.keys().map(|k| k.as_str()).collect())
                    .unwrap_or_default();
                RecoverableError::with_hint(
                    format!(
                        "missing 'query' or 'symbol' parameter (received keys: {})",
                        if got_keys.is_empty() {
                            "(none)".to_string()
                        } else {
                            got_keys.join(", ")
                        }
                    ),
                    "Provide 'query' (substring search) or 'symbol' (exact identifier, e.g. 'MyStruct/my_method')",
                )
            })?;
        let mut guard = OutputGuard::from_input(&input);
        // find_symbol uses a tighter exploring cap than the default 200.
        if matches!(guard.mode, OutputMode::Exploring) {
            guard.max_results = FIND_SYMBOL_MAX_RESULTS;
        }

        // kind filter only applies to pattern-based searches, not exact name_path lookups.
        let is_name_path = input["symbol"].is_string();

        // Reject regex-like patterns early — find_symbol does substring matching,
        // not regex. Point the LLM to grep instead.
        if !is_name_path && is_regex_like(pattern) {
            let trigger = if pattern.contains('|') {
                "'|'"
            } else if pattern.contains(".*") || pattern.contains(".+") {
                "'.*'"
            } else if pattern.starts_with('^') || pattern.ends_with('$') {
                "'^'/'$'"
            } else {
                "regex syntax"
            };
            return Err(RecoverableError::with_hint(
                format!(
                    "pattern looks like a regex (found {trigger}) — \
                     find_symbol searches symbol names, not text"
                ),
                "Use grep(pattern=\"...\") for regex text search, \
                 or make separate find_symbol calls for each symbol name",
            )
            .into());
        }

        let kind_filter: Option<&str> = if is_name_path {
            None
        } else {
            input["kind"].as_str()
        };

        let include_body = optional_bool_param(&input, "include_body")
            .unwrap_or_else(|| guard.should_include_body());
        let depth = optional_u64_param(&input, "depth").unwrap_or(0) as usize;
        let scope = crate::library::scope::Scope::parse(input["scope"].as_str());

        let root = ctx.agent.require_project_root().await?;
        let pattern_lower = pattern.to_lowercase();
        // Build the name predicate once: exact matching for name_path lookups,
        // case-insensitive substring matching for pattern searches.
        // Box<dyn Fn>: two different closure types must be held under one variable across a conditional; generics cannot express this at runtime.
        let name_ok: Box<dyn Fn(&SymbolInfo) -> bool + Send> = if is_name_path {
            let p = pattern.to_owned();
            Box::new(move |sym: &SymbolInfo| symbol_name_matches(sym, &p))
        } else {
            let p = pattern_lower.clone();
            Box::new(move |sym: &SymbolInfo| {
                sym.name.to_lowercase().contains(&p) || sym.name_path.to_lowercase().contains(&p)
            })
        };
        let mut matches = vec![];

        if let Some(rel) = get_path_param(&input, false)? {
            // Restricted search: per-file textDocument/documentSymbol
            let files: Vec<PathBuf> = if is_glob(rel) {
                resolve_glob(ctx, rel).await?
            } else {
                let full = root.join(rel);
                if full.is_dir() {
                    // Walk directory to find source files (same pattern as ListSymbols)
                    let walker = ignore::WalkBuilder::new(&full)
                        .hidden(true)
                        .git_ignore(true)
                        .build();
                    walker
                        .flatten()
                        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                        .map(|e| e.path().to_path_buf())
                        .collect()
                } else {
                    vec![full]
                }
            };

            for file_path in &files {
                let Some(lang) = ast::detect_language(file_path) else {
                    continue;
                };
                let language_id = crate::lsp::servers::lsp_language_id(lang);
                let mux_override = ctx.agent.lsp_mux_override(lang).await;
                let Ok(client) = ctx.lsp.get_or_start(lang, &root, mux_override).await else {
                    continue;
                };
                let timer = LspTimer::start();
                let Ok(symbols) = client.document_symbols(file_path, language_id).await else {
                    continue;
                };
                timer.record(ctx, lang, &root).await;
                let source = if include_body {
                    std::fs::read_to_string(file_path).ok()
                } else {
                    None
                };
                collect_matching(
                    &symbols,
                    name_ok.as_ref(),
                    include_body,
                    source.as_deref(),
                    depth,
                    true,
                    &mut matches,
                    kind_filter,
                );
            }
        } else {
            if scope.includes_project() {
                // Fast path: workspace/symbol — one LSP request per language instead of
                // one textDocument/documentSymbol request per file.
                let mut languages = std::collections::HashSet::new();
                let walker = ignore::WalkBuilder::new(&root)
                    .hidden(true)
                    .git_ignore(true)
                    .build();
                for entry in walker.flatten() {
                    if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        if let Some(lang) = ast::detect_language(entry.path()) {
                            languages.insert(lang);
                        }
                    }
                }

                // Concurrently start/query all LSP servers so different languages
                // (e.g. Kotlin JVM startup) don't block each other.
                let languages: Vec<&str> = languages.into_iter().collect();
                let mut join_set = tokio::task::JoinSet::new();
                for lang in languages {
                    let lsp = ctx.lsp.clone();
                    let root = root.clone();
                    let pattern = pattern_lower.clone();
                    let mux_override = ctx.agent.lsp_mux_override(lang).await;
                    join_set.spawn(async move {
                        let client = lsp.get_or_start(lang, &root, mux_override).await?;
                        client.workspace_symbols(&pattern).await
                    });
                }
                while let Some(task_result) = join_set.join_next().await {
                    let Ok(Ok(symbols)) = task_result else {
                        continue;
                    };
                    for sym in symbols {
                        // LSP servers may use fuzzy/prefix matching — enforce substring.
                        let name_ok = sym.name.to_lowercase().contains(&pattern_lower)
                            || sym.name_path.to_lowercase().contains(&pattern_lower);
                        let kind_ok =
                            kind_filter.map_or(true, |f| matches_kind_filter(&sym.kind, f));
                        if name_ok && kind_ok {
                            // When include_body is requested, validate the range. If
                            // workspace/symbol returned a degenerate range, fall back to
                            // document_symbols for the file to get the correct range.
                            let sym = if include_body {
                                match validate_symbol_range(&sym) {
                                    Ok(()) => sym,
                                    Err(validation_err) => {
                                        match resolve_range_via_document_symbols(&sym, ctx).await {
                                            Some(resolved) => resolved,
                                            None => {
                                                // document_symbols fallback failed too — propagate
                                                // the original validation error captured above.
                                                return Err(validation_err);
                                            }
                                        }
                                    }
                                }
                            } else {
                                sym
                            };
                            let source = if include_body {
                                std::fs::read_to_string(&sym.file).ok()
                            } else {
                                None
                            };
                            matches.push(symbol_to_json(
                                &sym,
                                include_body,
                                source.as_deref(),
                                depth,
                                true,
                            ));
                        }
                    }
                }

                // Tree-sitter fallback: if workspace/symbol returned nothing (LSP
                // not running, still indexing, or doesn't support workspace/symbol),
                // walk source files and extract symbols with tree-sitter.
                if matches.is_empty() {
                    let walker = ignore::WalkBuilder::new(&root)
                        .hidden(true)
                        .git_ignore(true)
                        .build();
                    for entry in walker.flatten() {
                        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                            continue;
                        }
                        let path = entry.path();
                        if ast::detect_language(path).is_none() {
                            continue;
                        }
                        if let Ok(symbols) = crate::ast::extract_symbols(path) {
                            let source = if include_body {
                                std::fs::read_to_string(path).ok()
                            } else {
                                None
                            };
                            collect_matching(
                                &symbols,
                                name_ok.as_ref(),
                                include_body,
                                source.as_deref(),
                                depth,
                                true,
                                &mut matches,
                                kind_filter,
                            );
                        }
                        // Early cap to avoid scanning entire huge projects
                        if matches.len() > guard.max_results {
                            break;
                        }
                    }
                }
            }

            // Search library directories when scope includes them
            let lib_roots = resolve_library_roots(&scope, &ctx.agent).await?;
            for (lib_name, lib_root) in &lib_roots {
                if !lib_root.exists() {
                    continue;
                }
                // Library directories are external — don't apply the project's
                // .gitignore (e.g. .venv/ would hide pip-installed packages).
                let walker = ignore::WalkBuilder::new(lib_root)
                    .hidden(true)
                    .git_ignore(false)
                    .build();
                for entry in walker.flatten() {
                    if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        continue;
                    }
                    let path = entry.path();
                    let Some(lang) = ast::detect_language(path) else {
                        continue;
                    };

                    // Tree-sitter first for library files: it's fast and avoids blocking
                    // on slow LSP startup (e.g. JVM-based Kotlin LSP). Only fall back to
                    // LSP document_symbols if tree-sitter returns nothing.
                    let mut symbols = crate::ast::extract_symbols(path).unwrap_or_default();
                    if symbols.is_empty() {
                        // INVARIANT: Always use project root as workspace_root, not the
                        // library root. LspManager caches one client per language; passing
                        // a different root kills and restarts the server.
                        let mux_override = ctx.agent.lsp_mux_override(lang).await;
                        if let Ok(client) = ctx.lsp.get_or_start(lang, &root, mux_override).await {
                            let language_id = crate::lsp::servers::lsp_language_id(lang);
                            symbols = client
                                .document_symbols(path, language_id)
                                .await
                                .unwrap_or_default();
                        }
                    }

                    let source = if include_body {
                        std::fs::read_to_string(path).ok()
                    } else {
                        None
                    };

                    // Collect matching symbols, rewriting file paths to lib: prefix
                    for sym in &symbols {
                        if name_ok(sym)
                            && kind_filter.map_or(true, |f| matches_kind_filter(&sym.kind, f))
                        {
                            let mut json_val =
                                symbol_to_json(sym, include_body, source.as_deref(), depth, true);
                            if let Some(obj) = json_val.as_object_mut() {
                                obj.insert(
                                    "file".to_string(),
                                    json!(format_library_path(lib_name, lib_root, path)),
                                );
                            }
                            matches.push(json_val);
                        }
                    }

                    if matches.len() > guard.max_results * 2 {
                        break;
                    }
                }
            }
        }

        // Build by_file distribution from the full result set BEFORE truncation.
        let (by_file_entries, by_file_overflow_count) = build_by_file(&matches);
        let hint = if matches.len() > guard.max_results {
            make_find_symbol_hint(guard.max_results, &by_file_entries)
        } else {
            String::from("Restrict with a file path or glob pattern")
        };
        let (mut matches, mut overflow) = guard.cap_items(matches, &hint);
        // Patch by_file into the overflow object (RF6 resolution: mutate after cap_items).
        if let Some(ref mut ov) = overflow {
            if !by_file_entries.is_empty() {
                ov.by_file = Some(by_file_entries);
                ov.by_file_overflow = by_file_overflow_count;
                // Rewrite hint with the real `shown` value now we know it.
                ov.hint = make_find_symbol_hint(ov.shown, ov.by_file.as_deref().unwrap_or(&[]));
            }
        }

        // When include_body is on and there are many results, strip bodies
        // beyond a threshold to avoid blowing the context window.
        const BODY_CAP: usize = 5;
        if include_body && matches.len() > BODY_CAP {
            for item in &mut matches[BODY_CAP..] {
                if let Some(obj) = item.as_object_mut() {
                    obj.remove("body");
                    obj.insert(
                        "body_omitted".to_string(),
                        json!("use find_symbol with symbol for full body"),
                    );
                }
            }
        }

        let total = overflow.as_ref().map_or(matches.len(), |o| o.total);
        let mut result = json!({ "symbols": matches, "total": total });
        if let Some(ov) = overflow {
            result["overflow"] = OutputGuard::overflow_json(&ov);
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_find_symbol(result))
    }

    fn json_path_hint(&self, val: &Value) -> String {
        let has_body = val["symbols"]
            .as_array()
            .and_then(|a| a.first())
            .map(|s| s["body"].is_string())
            .unwrap_or(false);
        if has_body {
            "$.symbols[0].body".to_string()
        } else {
            "$.symbols".to_string()
        }
    }
}
