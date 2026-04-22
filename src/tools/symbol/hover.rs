//! `hover` — type info and documentation for a symbol at a given position.

use serde_json::{json, Value};

use crate::ast;
use crate::tools::{require_u64_param, RecoverableError, Tool, ToolContext};

use super::display::format_hover;
use super::{get_lsp_client, require_path_param, resolve_read_path, tag_external_path, LspTimer};

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
        "Get type info and documentation for a symbol at a given position. \
         Returns the type signature, inferred types, and doc comments. \
         Complements find_symbol (name lookup) and goto_definition (navigation)."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "line"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "line": { "type": "integer", "description": "1-indexed line number" },
                "identifier": { "type": "string", "description": "Optional identifier on the line to target (disambiguates when multiple symbols on same line)" }
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
        let identifier = input["identifier"].as_str();

        let full_path = resolve_read_path(ctx, rel_path).await?;
        let raw_lang = ast::detect_language(&full_path)
            .ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
        let (client, lang) = get_lsp_client(ctx, &full_path).await?;

        // Determine column: find identifier on the line, or skip modifiers to find first symbol
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

        let col = if let Some(ident) = identifier {
            source_line.find(ident).ok_or_else(|| {
                RecoverableError::with_hint(
                    format!("identifier '{}' not found on line {}", ident, line_1),
                    "Check the identifier spelling, or omit it to use the first symbol on the line",
                )
            })? as u32
        } else {
            find_first_symbol_col(source_line)
        };

        let timer = LspTimer::start();
        let hover_text = client.hover(&full_path, line_0, col, &lang).await?;
        let root = ctx.agent.require_project_root().await?;
        timer.record(ctx, raw_lang, &root).await;

        match hover_text {
            Some(text) => {
                let source_tag = tag_external_path(&full_path, &root, &ctx.agent).await;
                let mut result = json!({
                    "content": text,
                    "location": format!("{}:{}", full_path.file_name().unwrap_or_default().to_string_lossy(), line_1),
                });
                if source_tag != "project" {
                    result["source"] = json!(source_tag);
                }
                // Nudge if library was discovered but not indexed
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
            None => Err(RecoverableError::with_hint(
                format!("no hover info at {}:{}", full_path.display(), line_1),
                "The LSP has no type/doc info at this position. \
             Try specifying an 'identifier' parameter, or use find_symbol for name-based lookup",
            )
            .into()),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_hover(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}
