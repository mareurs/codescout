//! `find_references` — find all usages of a symbol.

use serde_json::{json, Value};

use crate::ast;
use crate::tools::output::OutputGuard;
use crate::tools::{require_str_param, Tool, ToolContext};

use super::display::format_find_references;
use super::path_helpers::{
    classify_reference_path, get_lsp_client, path_in_excluded_dir, require_path_param,
    resolve_library_roots, resolve_read_path, uri_to_path, LspTimer,
};
use super::symbol_query::find_unique_symbol_by_name_path;

pub struct FindReferences;

#[async_trait::async_trait]
impl Tool for FindReferences {
    fn name(&self) -> &str {
        "find_references"
    }
    fn description(&self) -> &str {
        "Find all usages of a symbol. Requires symbol and file."
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

        let full_path = resolve_read_path(ctx, rel_path).await?;
        let raw_lang = ast::detect_language(&full_path)
            .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
        let root = ctx.agent.require_project_root().await?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // Find the symbol's position by walking document symbols
        let timer = LspTimer::start();
        let symbols = client.document_symbols(&full_path, &lang).await?;
        timer.record(ctx, raw_lang, &root).await;
        let sym = find_unique_symbol_by_name_path(&symbols, name_path)?;

        // Get references at the symbol's position
        let refs = client
            .references(&full_path, sym.start_line, sym.start_col, &lang)
            .await?;

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
        let total = locations.len();
        let (locations, overflow) = guard.cap_items(locations, "This symbol has many references. Use detail_level='full' with offset/limit to paginate");
        let mut result = json!({ "references": locations, "total": total });
        if excluded > 0 {
            result["excluded_from_build_dirs"] = json!(excluded);
        }
        if let Some(ov) = overflow {
            result["overflow"] = OutputGuard::overflow_json(&ov);
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_find_references(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}
