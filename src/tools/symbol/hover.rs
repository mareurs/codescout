//! `hover` — type info and documentation for a symbol at a given position.

use serde_json::{json, Value};

use crate::ast;
use crate::tools::{require_u64_param, RecoverableError, Tool, ToolContext};

use super::display::format_hover;
use super::path_helpers::{
    get_lsp_client, require_path_param, resolve_read_path, retry_on_mux_disconnect,
    tag_external_path, LspTimer,
};

const HOVER_SKIP_TOKENS: &[&str] = &[
    "pub", "async", "unsafe", "extern", "default", "override", "fn", "struct", "enum", "trait",
    "impl", "type", "const", "static", "mod", "use", "dyn", "for", "where", "mut", "ref", "let",
];

/// Find the byte column of the first non-keyword identifier on `line`.
/// Skips Rust visibility and declaration keywords (`pub`, `fn`, `struct`, …)
/// so that `hover` on a declaration line lands on the symbol name, not `pub`.
fn find_first_symbol_col(line: &str) -> u32 {
    let bytes = line.as_bytes();
    let mut pos = 0usize;
    loop {
        // Skip non-identifier characters (whitespace, parens, angle brackets, etc.)
        while pos < bytes.len() && !bytes[pos].is_ascii_alphabetic() && bytes[pos] != b'_' {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let start = pos;
        while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
            pos += 1;
        }
        let token = &line[start..pos];
        if !HOVER_SKIP_TOKENS.contains(&token) {
            return start as u32;
        }
    }
    // Fallback: first non-whitespace character
    line.chars().take_while(|c| c.is_whitespace()).count() as u32
}

pub struct Hover;

#[async_trait::async_trait]
impl Tool for Hover {
    fn name(&self) -> &str {
        "hover"
    }
    fn description(&self) -> &str {
        "Get type info and documentation for a symbol via LSP — type signature, \
         inferred types, and doc comments. Pass `col` (1-indexed) when known; \
         fall back to `identifier` to locate by name on the line. \
         Complements find_symbol and goto_definition."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "line"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "line": { "type": "integer", "description": "1-indexed line number" },
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
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // Resolution: col > identifier > first-symbol heuristic.
        // `col` is LSP-native; `identifier` is a substring-match fallback;
        // first-symbol skips leading modifiers on definition lines.
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
            find_first_symbol_col(source_line)
        };

        let timer = LspTimer::start();
        let hover_text = retry_on_mux_disconnect(ctx, &full_path, client, lang, |c, l| {
            let p = full_path.clone();
            async move { c.hover(&p, line_0, col, &l).await }
        })
        .await?;
        let root = ctx.agent.require_project_root().await?;
        timer.record(ctx, raw_lang, &root).await;

        let source_tag = tag_external_path(&full_path, &root, &ctx.agent).await;
        let location = format!(
            "{}:{}",
            full_path.file_name().unwrap_or_default().to_string_lossy(),
            line_1
        );

        match hover_text {
            Some(text) => {
                let mut result = json!({
                    "content": text,
                    "location": location,
                });
                if source_tag != "project" {
                    result["source"] = json!(source_tag);
                }
                if let Some(lib_name) = source_tag.strip_prefix("lib:") {
                    if ctx.agent.should_nudge(lib_name).await {
                        result["library_hint"] = json!({
                            "name": lib_name,
                            "status": "not_indexed",
                            "hint": format!("Library '{}' discovered but not indexed. Run index_project(scope='lib:{}') to enable semantic search.", lib_name, lib_name)
                        });
                    }
                    ctx.agent.maybe_auto_index_library(lib_name).await;
                }
                Ok(result)
            }
            None => {
                // Empty hover is a successful empty result, not a failure.
                let mut result = json!({
                    "content": null,
                    "location": location,
                    "hint": "no hover info at this position — LSP has no type/doc info. \
                             Re-verify line/col via list_symbols, or use find_symbol for name-based lookup.",
                });
                if source_tag != "project" {
                    result["source"] = json!(source_tag);
                }
                Ok(result)
            }
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_hover(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}
