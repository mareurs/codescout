//! `references` — find all usages of a symbol.

use serde_json::{json, Value};

use crate::ast;
use crate::tools::output::OutputGuard;
use crate::tools::{require_str_param, OutputForm, Tool, ToolContext};

use crate::fs::{
    classify_reference_path, get_lsp_client, path_in_excluded_dir, require_path_param,
    resolve_library_roots, resolve_read_path, retry_on_mux_disconnect, uri_to_path, LspTimer,
};
use crate::symbol::query::find_unique_symbol_by_name_path;

pub struct References;

/// Completeness cross-check for `references` results.
///
/// Call sites (from `callHierarchy/incomingCalls`) are a strict subset of all
/// references, so if call-hierarchy finds MORE call sites than
/// `textDocument/references` returned references, references is provably
/// incomplete — a known rust-analyzer quirk for some symbol shapes (e.g.
/// `pub(super)` free fns referenced from a sibling `#[cfg(test)]` module).
/// See `docs/issues/2026-05-21-references-undercounts-vs-call-graph.md`.
pub(crate) fn references_completeness_hint(refs_total: usize, call_sites: usize) -> Option<String> {
    if call_sites > refs_total {
        Some(format!(
            "references found {refs_total}, but call-hierarchy found {call_sites} call sites — \
             rust-analyzer's textDocument/references is incomplete for this symbol. \
             Use call_graph(symbol, direction=\"callers\") for the authoritative caller set."
        ))
    } else {
        None
    }
}
/// True if `needle` occurs in `haystack` bounded by non-identifier characters
/// on both sides, so `foo` matches `foo(` but not `foobar` or `barfoo`.
pub(crate) fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let nlen = needle.len();
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(needle) {
        let i = from + rel;
        let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
        let after = i + nlen;
        let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        from = i + nlen;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// LSP-independent corroboration for a zero-external-callers `references` result
/// (BUG 2026-06-09 references-false-zero-stale-graph). After an incremental
/// reindex the LSP reference graph can lag the on-disk text, so
/// `textDocument/references` returns only the definition for a symbol that has
/// callers — the signal an agent uses to delete supposedly-dead code. The
/// LSP/chunk indices share that staleness; raw source text does not. Walks the
/// project for same-language source files (mirroring call_graph Phase B bounds)
/// and returns up to a few files OTHER than `def_file` where `ident` appears as
/// a whole word. A non-empty result proves the empty LSP set is incomplete.
pub(crate) fn corroborate_zero_references(
    root: &std::path::Path,
    def_file: &std::path::Path,
    ident: &str,
    lang: &str,
) -> Vec<std::path::PathBuf> {
    const MAX_FILES_SCAN: usize = 5_000;
    const MAX_HITS: usize = 5;
    let mut hits: Vec<std::path::PathBuf> = Vec::new();
    let mut scanned = 0usize;
    for entry in ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build()
        .flatten()
    {
        if hits.len() >= MAX_HITS || scanned >= MAX_FILES_SCAN {
            break;
        }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if path == def_file {
            continue;
        }
        if crate::ast::detect_language(path) != Some(lang) {
            continue;
        }
        scanned += 1;
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        if contains_word(&src, ident) {
            hits.push(path.to_path_buf());
        }
    }
    hits
}

#[async_trait::async_trait]
impl Tool for References {
    fn name(&self) -> &str {
        "references"
    }
    fn description(&self) -> &str {
        "Find all usages of a symbol. Requires symbol and file."
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["symbol", "path"],
            "properties": {
                "symbol": { "type": "string", "description": "Symbol identifier (e.g. 'MyStruct/my_method')" },
                "path": { "type": "string", "description": "File containing the symbol" },
                "detail_level": { "type": "string", "description": "'full' for bodies (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "limit": { "type": "integer", "description": "Max results (default 50)" },
                "scope": { "type": "string", "description": "'project' (default), 'libraries', 'all', or 'lib:<name>'", "default": "project" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let name_path = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let scope = crate::library::scope::Scope::parse(input["scope"].as_str());

        let full_path = resolve_read_path(&ctx.agent, rel_path).await?;
        let raw_lang = ast::detect_language(&full_path)
            .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;

        // Find the symbol's position by walking document symbols, then resolve
        // references. I-4: wrap the whole symbol-then-references flow in a
        // single mux-disconnect retry — both calls are idempotent reads.
        // `client` and `lang` are cloned so the post-retry call-hierarchy
        // cross-check (below) still has them.
        let timer = LspTimer::start();
        let name_path_owned = name_path.to_string();
        let (sym, refs) = retry_on_mux_disconnect(
            &ctx.agent,
            &*ctx.lsp,
            &full_path,
            client.clone(),
            lang.clone(),
            |c, l| {
                let p = full_path.clone();
                let np = name_path_owned.clone();
                async move {
                    let symbols = c.document_symbols(&p, &l).await?;
                    let sym = find_unique_symbol_by_name_path(&symbols, &np)?.clone();
                    let refs = c.references(&p, sym.start_line, sym.start_col, &l).await?;
                    anyhow::Ok((sym, refs))
                }
            },
        )
        .await?;
        timer.record(&*ctx.lsp, raw_lang, &root).await;

        // Resolve all library roots for classification (Scope::All to get every lib).
        let lib_roots =
            resolve_library_roots(&crate::library::scope::Scope::All, &ctx.agent).await?;

        // Filter out references inside build-artifact directories. LSP servers often
        // index generated files (target/, node_modules/, dist/, …) and including them
        // creates noise without actionable information.
        let total_raw = refs.len();
        let refs: Vec<_> = refs
            .into_iter()
            .filter(|loc| {
                uri_to_path(loc.uri.as_str())
                    .map(|p| !path_in_excluded_dir(&p))
                    .unwrap_or(true)
            })
            .collect();
        let excluded = total_raw - refs.len();

        // Scope-filter references
        let refs: Vec<_> = refs
            .into_iter()
            .filter(|loc| {
                let Some(path) = uri_to_path(loc.uri.as_str()) else {
                    return true; // keep references we can't resolve
                };
                let (classification, _) = classify_reference_path(&path, &root, &lib_roots);
                match &scope {
                    crate::library::scope::Scope::Project => classification == "project",
                    crate::library::scope::Scope::Libraries => classification.starts_with("lib:"),
                    crate::library::scope::Scope::All => true,
                    crate::library::scope::Scope::Library(name) => {
                        classification == format!("lib:{}", name)
                    }
                }
            })
            .collect();

        let locations: Vec<Value> = refs
            .iter()
            .map(|loc| {
                let file = uri_to_path(loc.uri.as_str())
                    .map(|p| {
                        let (_, display) = classify_reference_path(&p, &root, &lib_roots);
                        display
                    })
                    .unwrap_or_else(|| loc.uri.as_str().to_string());

                // Read context lines around the reference
                let context = uri_to_path(loc.uri.as_str())
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .map(|src| {
                        let lines: Vec<&str> = src.lines().collect();
                        let line = loc.range.start.line as usize;
                        lines.get(line).unwrap_or(&"").to_string()
                    })
                    .unwrap_or_default();

                json!({
                    "file": file,
                    "line": loc.range.start.line + 1,
                    "column": loc.range.start.character,
                    "context": context,
                })
            })
            .collect();

        let guard = OutputGuard::from_input(&input);
        let budget = guard.max_results;

        use crate::tools::file_group::{cap_grouped, group_by_file, groups_to_json};
        let (visible, total, files) = cap_grouped(locations, budget);
        let truncated = total > visible.len();
        let groups = group_by_file(&visible);
        let file_groups = groups_to_json(&groups);

        let mut result = json!({
            "file_groups": file_groups,
            "total": total,
            "files": files,
        });
        if excluded > 0 {
            result["excluded_from_build_dirs"] = json!(excluded);
        }
        if truncated {
            let overflow = json!({
                "shown": visible.len(),
                "total": total,
                "hint": "This symbol has many references. Use detail_level='full' with offset/limit to paginate",
            });
            result["overflow"] = overflow;
        }

        // Completeness cross-check (BUG 2026-05-21): rust-analyzer's
        // textDocument/references can silently undercount for some symbol shapes.
        // callHierarchy/incomingCalls does not share the gap, and call sites are a
        // strict subset of references — so more call sites than references found is
        // proof of an incomplete reference set. Cheap: one prepare_call_hierarchy
        // (None for non-callable symbols → skipped) plus one incoming_calls.
        if let Ok(Some(item)) = client
            .prepare_call_hierarchy(&full_path, sym.start_line, sym.start_col, &lang)
            .await
        {
            if let Ok(incoming) = client.incoming_calls(&item, &lang).await {
                let call_sites: usize = incoming.iter().map(|c| c.from_ranges.len().max(1)).sum();
                if let Some(hint) = references_completeness_hint(total, call_sites) {
                    result["completeness_warning"] = json!(hint);
                }
            }
        }

        // Zero-external-callers corroboration (BUG 2026-06-09 references-false-zero-stale-graph):
        // no callers OUTSIDE the definition file is the high-consequence failure mode — after an
        // incremental reindex the LSP reference graph can lag the on-disk text and return only the
        // definition, and an agent reads a 0-callers result as dead code and deletes it. The
        // call-hierarchy cross-check above is also LSP-backed (shares the staleness), so corroborate
        // with an LSP-independent text scan.
        let external_refs = refs
            .iter()
            .filter(|loc| {
                uri_to_path(loc.uri.as_str())
                    .map(|p| p != full_path)
                    .unwrap_or(false)
            })
            .count();
        if external_refs == 0 && result.get("completeness_warning").is_none() {
            let ident = name_path.rsplit('/').next().unwrap_or(name_path);
            let others = corroborate_zero_references(&root, &full_path, ident, raw_lang);
            if !others.is_empty() {
                let sample = others
                    .iter()
                    .take(3)
                    .map(|p| p.strip_prefix(&root).unwrap_or(p).display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                result["completeness_warning"] = json!(format!("LSP returned 0 references outside the definition file, but `{ident}` appears as a whole word in {n}+ other source file(s) (e.g. {sample}) — the reference index may still be warming after a reindex. Re-run, or corroborate with grep / call_graph(direction='callers') before treating this symbol as unused.", n = others.len()));
            }
        }

        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        use crate::tools::file_group::{groups_from_json, render_grouped};

        // BUG 2026-05-21: surface the completeness warning (if the call-hierarchy
        // cross-check found more call sites than references did) in both the
        // zero-refs and the normal branch.
        let warning = result.get("completeness_warning").and_then(|v| v.as_str());
        let append_warning = |mut out: String| -> String {
            if let Some(w) = warning {
                out.push_str("\n\nwarning: ");
                out.push_str(w);
            }
            out
        };

        let file_groups_arr = result["file_groups"].as_array()?;
        if file_groups_arr.is_empty() {
            return Some(append_warning("0 references".to_string()));
        }

        let groups = groups_from_json(file_groups_arr);
        let total = result["total"].as_u64().unwrap_or(0) as usize;
        let files = result["files"].as_u64().unwrap_or(groups.len() as u64) as usize;
        let noun = if total == 1 {
            "reference"
        } else {
            "references"
        };

        let render_item = |item: &Value| -> String {
            let line = item["line"].as_u64().unwrap_or(0);
            let context = item["context"].as_str().unwrap_or("").trim();
            format!("  {line:>5}  {context}")
        };

        Some(append_warning(render_grouped(
            &groups,
            total,
            files,
            noun,
            render_item,
        )))
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}
