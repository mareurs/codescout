//! `symbol_at` — unified LSP query at a position. Returns `def` and/or `hover`
//! depending on requested `fields`. Replaces the former `goto_definition` and
//! `hover` tools.

use serde_json::{json, Value};

use crate::ast;
use crate::tools::{require_u64_param, RecoverableError, Tool, ToolContext};

use super::display::{format_goto_definition, format_hover};
use crate::fs::{
    get_lsp_client, require_path_param, resolve_read_path, retry_on_mux_disconnect,
    tag_external_path, uri_to_path, LspTimer,
};

const HOVER_SKIP_TOKENS: &[&str] = &[
    "pub", "async", "unsafe", "extern", "default", "override", "fn", "struct", "enum", "trait",
    "impl", "type", "const", "static", "mod", "use", "dyn", "for", "where", "mut", "ref", "let",
];

/// Find the byte column of the first non-keyword identifier on `line`.
/// Skips Rust visibility and declaration keywords (`pub`, `fn`, `struct`, …)
/// so that hover on a declaration line lands on the symbol name, not `pub`.
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

/// Validated `(path, line_1, line_0, col_param, identifier_opt)` parsed from
/// the tool input.
type PositionInputs = (String, u32, u32, Option<u64>, Option<String>);

/// Read `path`, `line`, `col`, `identifier` from input and validate the line bound.
fn read_position_inputs(input: &Value) -> anyhow::Result<PositionInputs> {
    let rel_path = require_path_param(input)?.to_string();
    let line_1 = require_u64_param(input, "line")? as u32;
    if line_1 == 0 {
        return Err(RecoverableError::with_hint(
            "'line' must be >= 1 (1-indexed)",
            "Line numbers are 1-indexed. Use line: 1 for the first line.",
        )
        .into());
    }
    let line_0 = line_1 - 1;
    let col_param = input["col"].as_u64();
    let identifier = input["identifier"].as_str().map(str::to_string);
    Ok((rel_path, line_1, line_0, col_param, identifier))
}

/// Run `goto_definition` and return the JSON object the legacy tool produced
/// at the top level: `{ "definitions": [...], "from": "...", maybe "hint": ... }`.
pub(crate) async fn fetch_definition(ctx: &ToolContext, input: &Value) -> anyhow::Result<Value> {
    let (rel_path, line_1, line_0, col_param, identifier) = read_position_inputs(input)?;

    let full_path = resolve_read_path(&ctx.agent, &rel_path).await?;
    let raw_lang =
        ast::detect_language(&full_path).ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
    let root = ctx.agent.require_project_root().await?;
    let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;

    // Resolution: col > identifier > first-non-whitespace.
    let source = std::fs::read_to_string(&full_path)?;
    let source_line = source.lines().nth(line_0 as usize).ok_or_else(|| {
        RecoverableError::with_hint(
            format!(
                "line {} is beyond end of file ({})",
                line_1,
                full_path.display()
            ),
            "Check the line number — use symbols(path) or grep to find correct lines",
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
    } else if let Some(ref ident) = identifier {
        source_line.find(ident.as_str()).ok_or_else(|| {
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
    let definitions =
        retry_on_mux_disconnect(&ctx.agent, &*ctx.lsp, &full_path, client, lang, |c, l| {
            let p = full_path.clone();
            async move { c.goto_definition(&p, line_0, col, &l).await }
        })
        .await?;
    timer.record(&*ctx.lsp, raw_lang, &root).await;

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
                     or use symbols(name=...) for name-based lookup.",
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
                    "hint": format!("Library '{}' discovered but not indexed. Run index(action='build', scope='lib:{}') to enable semantic search.", lib_name, lib_name)
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

/// Run `hover` and return the JSON object the legacy tool produced at the top level:
/// `{ "content": <string|null>, "location": "...", maybe "hint": ..., maybe "source": ... }`.
pub(crate) async fn fetch_hover(ctx: &ToolContext, input: &Value) -> anyhow::Result<Value> {
    let (rel_path, line_1, line_0, col_param, identifier) = read_position_inputs(input)?;

    let full_path = resolve_read_path(&ctx.agent, &rel_path).await?;
    let raw_lang =
        ast::detect_language(&full_path).ok_or_else(|| anyhow::anyhow!("unsupported language"))?;
    let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &full_path).await?;

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
            "Check the line number — use symbols(path) or grep to find correct lines",
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
    } else if let Some(ref ident) = identifier {
        source_line.find(ident.as_str()).ok_or_else(|| {
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
    let hover_text =
        retry_on_mux_disconnect(&ctx.agent, &*ctx.lsp, &full_path, client, lang, |c, l| {
            let p = full_path.clone();
            async move { c.hover(&p, line_0, col, &l).await }
        })
        .await?;
    let root = ctx.agent.require_project_root().await?;
    timer.record(&*ctx.lsp, raw_lang, &root).await;

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
                        "hint": format!("Library '{}' discovered but not indexed. Run index(action='build', scope='lib:{}') to enable semantic search.", lib_name, lib_name)
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
                         Re-verify line/col via symbols(path), or use symbols(name=...) for name-based lookup.",
            });
            if source_tag != "project" {
                result["source"] = json!(source_tag);
            }
            Ok(result)
        }
    }
}

pub struct SymbolAt;

#[async_trait::async_trait]
impl Tool for SymbolAt {
    fn name(&self) -> &str {
        "symbol_at"
    }
    fn description(&self) -> &str {
        "Inspect a symbol at a position via LSP — returns definition location(s) and/or \
         hover (type signature + docs). Pass `fields` to choose; defaults to both. \
         Pass `col` (1-indexed) when known; fall back to `identifier` to locate by name on the line."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "### Workflow: Dependency Tracing — \"How does data flow from A to B?\"\n\n\
             | Step | Tool | Purpose |\n\
             |------|------|---------|\n\
             | 1 | `symbols(name=entry_point)` | Locate starting function |\n\
             | 2 | `symbol_at` with `fields: [\"def\"]` on called functions | Follow the call chain forward |\n\
             | 3 | `symbol_at` with `fields: [\"hover\"]` on parameters/return values | See resolved types at each stage |\n\
             | 4 | `references` at destination | Confirm which callers reach this point |",
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "line"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "line": { "type": "integer", "description": "1-indexed line number" },
                "col": { "type": "integer", "description": "1-indexed column. Preferred — LSP-native, no identifier-mismatch risk. When known (e.g. from symbols), pass directly." },
                "identifier": { "type": "string", "description": "Optional fallback when col not known. The substring is searched on the line; mismatch errors. Prefer col." },
                "fields": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["def", "hover"] },
                    "description": "Which LSP queries to run. Defaults to both [\"def\", \"hover\"]."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        // Parse `fields`. Default = both. Unknown values rejected.
        let (want_def, want_hover) = match input.get("fields") {
            None => (true, true),
            Some(Value::Array(arr)) => {
                let mut def = false;
                let mut hov = false;
                for v in arr {
                    match v.as_str() {
                        Some("def") => def = true,
                        Some("hover") => hov = true,
                        Some(other) => {
                            return Err(RecoverableError::with_hint(
                                format!("unknown field '{}' in `fields`", other),
                                "Allowed values: \"def\", \"hover\". Omit `fields` to request both.",
                            )
                            .into());
                        }
                        None => {
                            return Err(RecoverableError::with_hint(
                                "`fields` entries must be strings",
                                "Use e.g. fields: [\"def\", \"hover\"].",
                            )
                            .into());
                        }
                    }
                }
                if !def && !hov {
                    return Err(RecoverableError::with_hint(
                        "`fields` must request at least one of \"def\", \"hover\"",
                        "Omit `fields` to request both, or pass e.g. [\"hover\"].",
                    )
                    .into());
                }
                (def, hov)
            }
            Some(_) => {
                return Err(RecoverableError::with_hint(
                    "`fields` must be an array of strings",
                    "Use e.g. fields: [\"def\", \"hover\"], or omit to request both.",
                )
                .into());
            }
        };

        let mut out = serde_json::Map::new();
        if want_def {
            let def = fetch_definition(ctx, &input).await?;
            out.insert("def".to_string(), def);
        }
        if want_hover {
            let hov = fetch_hover(ctx, &input).await?;
            out.insert("hover".to_string(), hov);
        }
        Ok(Value::Object(out))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        let mut sections: Vec<String> = Vec::new();
        if let Some(def) = result.get("def") {
            let s = format_goto_definition(def);
            if !s.is_empty() {
                sections.push(format!("def:\n{}", indent_block(&s)));
            } else {
                sections.push("def: (empty)".to_string());
            }
        }
        if let Some(hov) = result.get("hover") {
            let s = format_hover(hov);
            if !s.is_empty() {
                sections.push(format!("hover:\n{}", indent_block(&s)));
            } else {
                sections.push("hover: (empty)".to_string());
            }
        }
        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n"))
        }
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}

fn indent_block(s: &str) -> String {
    s.lines()
        .map(|l| format!("  {}", l))
        .collect::<Vec<_>>()
        .join("\n")
}
