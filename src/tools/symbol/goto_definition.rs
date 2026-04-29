//! `goto_definition` — jump to the definition of a symbol at a given line.

use serde_json::{json, Value};

use crate::ast;
use crate::tools::{require_u64_param, RecoverableError, Tool, ToolContext};

use super::display::format_goto_definition;
use super::path_helpers::{
    get_lsp_client, require_path_param, resolve_read_path, retry_on_mux_disconnect,
    tag_external_path, uri_to_path, LspTimer,
};

pub struct GotoDefinition;

#[async_trait::async_trait]
impl Tool for GotoDefinition {
    fn name(&self) -> &str {
        "goto_definition"
    }
    fn description(&self) -> &str {
        "Jump to the definition of a symbol via LSP — handles method calls, trait impls, \
         and cross-crate navigation. Pass `col` (1-indexed) when known; fall back to \
         `identifier` to locate by name on the line. Auto-discovers library dependencies."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "### Workflow: Dependency Tracing — \"How does data flow from A to B?\"\n\n\
             | Step | Tool | Purpose |\n\
             |------|------|---------|\n\
             | 1 | `find_symbol(entry_point)` | Locate starting function |\n\
             | 2 | `goto_definition` on called functions | Follow the call chain forward |\n\
             | 3 | `hover` on parameters/return values | See resolved types at each stage |\n\
             | 4 | `find_references` at destination | Confirm which callers reach this point |",
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "line"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "line": { "type": "integer", "description": "1-indexed line number to jump from" },
                "col": { "type": "integer", "description": "1-indexed column. Preferred — LSP-native, no identifier-mismatch risk. When known (e.g. from list_symbols), pass directly." },
                "identifier": { "type": "string", "description": "Optional fallback when col not known. The substring is searched on the line; mismatch errors. Prefer col." }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let rel_path = require_path_param(&input)?;
        let line_1 = require_u64_param(&input, "line")? as u32;
        if line_1 == 0 {
            return Err(RecoverableError::with_hint(
                "'line' must be >= 1 (1-indexed)",
                "Line numbers are 1-indexed. Use line: 1 for the first line.",
            )
            .into());
        }
        let line_0 = line_1 - 1;
        let col_param = input["col"].as_u64();
        let identifier = input["identifier"].as_str();

        let full_path = resolve_read_path(ctx, rel_path).await?;
        let raw_lang = ast::detect_language(&full_path)
            .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
        let root = ctx.agent.require_project_root().await?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // Resolution: col > identifier > first-non-whitespace.
        let source = std::fs::read_to_string(&full_path)?;
        let source_line = source.lines().nth(line_0 as usize).ok_or_else(|| {
            RecoverableError::with_hint(
                format!(
                    "line {} is beyond end of file ({})",
                    line_1,
                    full_path.display()
                ),
                "Check the line number — use list_symbols or grep to find correct lines",
            )
        })?;

        let col = if let Some(c1) = col_param {
            if c1 == 0 {
                return Err(RecoverableError::with_hint(
                    "'col' must be >= 1 (1-indexed)",
                    "Column numbers are 1-indexed. Use col: 1 for the first column.",
                )
                .into());
            }
            (c1 - 1) as u32
        } else if let Some(ident) = identifier {
            source_line.find(ident).ok_or_else(|| {
                RecoverableError::with_hint(
                    format!("identifier '{}' not found on line {}", ident, line_1),
                    "Pass `col` directly (1-indexed) for an LSP-native lookup, \
                     or check the identifier spelling.",
                )
            })? as u32
        } else {
            source_line
                .chars()
                .take_while(|c| c.is_whitespace())
                .count() as u32
        };

        let timer = LspTimer::start();
        let definitions = retry_on_mux_disconnect(ctx, &full_path, client, lang, |c, l| {
            let p = full_path.clone();
            async move { c.goto_definition(&p, line_0, col, &l).await }
        })
        .await?;
        timer.record(ctx, raw_lang, &root).await;

        let from = format!(
            "{}:{}",
            full_path.file_name().unwrap_or_default().to_string_lossy(),
            line_1
        );

        if definitions.is_empty() {
            return Ok(json!({
                "definitions": [],
                "from": from,
                "hint": "no definition resolvable at this position — LSP couldn't bind the symbol. \
                         Verify the cursor is on a symbol name (or pass `col`), \
                         or use find_symbol for name-based lookup.",
            }));
        }

        let mut results = Vec::new();
        for loc in &definitions {
            let def_path = uri_to_path(loc.uri.as_str());
            let (file_display, source_tag) = if let Some(ref p) = def_path {
                let tag = tag_external_path(p, &root, &ctx.agent).await;
                let display = p
                    .strip_prefix(&root)
                    .map(|r| r.display().to_string())
                    .unwrap_or_else(|_| p.display().to_string());
                (display, tag)
            } else {
                (loc.uri.as_str().to_string(), "external".to_string())
            };

            let context = def_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|src| {
                    src.lines()
                        .nth(loc.range.start.line as usize)
                        .map(|l| l.to_string())
                })
                .unwrap_or_default();

            let mut def = json!({
                "file": file_display,
                "line": loc.range.start.line + 1,
                "end_line": loc.range.end.line + 1,
                "context": context.trim(),
            });
            if source_tag != "project" {
                def["source"] = json!(source_tag);
            }
            if let Some(lib_name) = source_tag.strip_prefix("lib:") {
                if ctx.agent.should_nudge(lib_name).await {
                    def["library_hint"] = json!({
                        "name": lib_name,
                        "status": "not_indexed",
                        "hint": format!("Library '{}' discovered but not indexed. Run index_project(scope='lib:{}') to enable semantic search.", lib_name, lib_name)
                    });
                }
                ctx.agent.maybe_auto_index_library(lib_name).await;
            }
            results.push(def);
        }

        Ok(json!({
            "definitions": results,
            "from": from,
        }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_goto_definition(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}
