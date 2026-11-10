//! Placement of a next hop; a display name alone is never a concrete queue key.
use std::path::Path;

use crate::lsp::{ops::LspClientOps, SymbolInfo};
use crate::tools::symbol::call_edges::resolver::{Definition, Direction, Edge, EdgeSource};
use crate::tools::symbol::scan_budget::{is_compiled_twin, scan_walk, ScanBudget, ScanCoverage};

pub(super) enum Placement {
    Found(Definition),
    Missing,
    Ambiguous,
    Incomplete(ScanCoverage),
    Unresolved(String),
}

pub(super) fn matching<'a>(symbols: &'a [SymbolInfo], name: &str) -> Vec<&'a SymbolInfo> {
    let mut found = Vec::new();
    for symbol in symbols {
        if symbol.name == name || symbol.name_path == name {
            found.push(symbol);
        }
        found.extend(matching(&symbol.children, name));
    }
    found
}

pub(super) fn symbols_in(file: &Path) -> anyhow::Result<Vec<SymbolInfo>> {
    let lang = crate::ast::detect_language(file)
        .ok_or_else(|| anyhow::anyhow!("no definition grammar for {}", file.display()))?;
    let source = std::fs::read_to_string(file)?;
    crate::ast::parser::extract_symbols_from_source(&source, Some(lang), file)
}

fn position(symbol: &SymbolInfo) -> Definition {
    Definition {
        file: symbol.file.clone(),
        line: symbol.start_line,
        col: symbol.start_col,
    }
}

/// Normalize AST/LSP selection positions to the same declaration start when source permits.
pub(super) fn canonical(name: &str, original: Definition) -> Definition {
    let Ok(symbols) = symbols_in(&original.file) else {
        return original;
    };
    matching(&symbols, name)
        .into_iter()
        .filter(|s| {
            s.start_line <= original.line
                && s.end_line >= original.line
                && (s.start_line != original.line || s.start_col <= original.col)
        })
        .max_by_key(|s| (s.start_line, s.start_col))
        .map(position)
        .unwrap_or(original)
}

fn unique(symbols: &[SymbolInfo], name: &str) -> Placement {
    let mut positions = matching(symbols, name)
        .into_iter()
        .map(position)
        .collect::<Vec<_>>();
    positions.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
    positions.dedup();
    match positions.as_slice() {
        [] => Placement::Missing,
        [one] => Placement::Found(one.clone()),
        _ => Placement::Ambiguous,
    }
}

pub(super) fn exact_in_file(file: &Path, name_path: &str) -> anyhow::Result<Option<Definition>> {
    let symbols = symbols_in(file)?;
    let mut found = matching(&symbols, name_path)
        .into_iter()
        .filter(|s| s.name_path == name_path)
        .map(position)
        .collect::<Vec<_>>();
    found.sort_by_key(|a| (a.line, a.col));
    found.dedup();
    match found.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.clone())),
        _ => anyhow::bail!("ambiguous declaration {name_path} in {}", file.display()),
    }
}

pub(super) fn workspace(root: &Path, lang: &str, name: &str, cap: usize) -> Placement {
    let ast_lang = crate::lsp::servers::ast_language_for_lsp_id(lang);
    if !crate::ast::parser::supports_symbol_extraction(ast_lang) {
        return Placement::Incomplete(ScanCoverage::no_parser(ast_lang));
    }
    let family = crate::ast::language_family(ast_lang);
    let in_family = |p: &Path| {
        crate::ast::detect_language(p).is_some_and(|l| crate::ast::language_family(l) == family)
    };
    let mut budget = ScanBudget::new(cap);
    let mut candidate: Option<Definition> = None;
    for item in scan_walk(root) {
        let Some(entry) = budget.entry(item) else {
            continue;
        };
        let path = entry.path();
        if entry.file_type().is_some_and(|t| t.is_symlink()) {
            budget.note_symlink(path, root, in_family);
            continue;
        }
        if !entry.file_type().is_some_and(|t| t.is_file())
            || !in_family(path)
            || is_compiled_twin(path)
        {
            continue;
        }
        if !budget.admit() {
            break;
        }
        let Ok(symbols) = symbols_in(path) else {
            budget.record_unexamined();
            continue;
        };
        match unique(&symbols, name) {
            Placement::Found(found) => {
                if candidate
                    .as_ref()
                    .is_some_and(|previous| previous != &found)
                {
                    return Placement::Ambiguous;
                }
                candidate = Some(found);
            }
            Placement::Ambiguous => return Placement::Ambiguous,
            _ => {}
        }
    }
    let coverage = budget.coverage_requiring_a_candidate();
    if !coverage.is_complete() {
        Placement::Incomplete(coverage)
    } else {
        candidate
            .map(Placement::Found)
            .unwrap_or(Placement::Missing)
    }
}

struct CallSite {
    expression: Option<String>,
    line: u32,
    col: u32,
    modules: Vec<String>,
    lexical_binding: bool,
}

fn rust_path(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "type_identifier" | "self" | "super" | "crate" => {
            node.utf8_text(source).ok().map(str::to_owned)
        }
        "generic_function" => rust_path(node.child_by_field_name("function")?, source),
        "generic_type" => rust_path(node.child_by_field_name("type")?, source),
        "scoped_identifier" | "scoped_type_identifier" => Some(format!(
            "{}::{}",
            rust_path(node.child_by_field_name("path")?, source)?,
            rust_path(node.child_by_field_name("name")?, source)?
        )),
        _ => None,
    }
}

fn call_site(edge: &Edge) -> Option<CallSite> {
    let lang = crate::ast::detect_language(&edge.file)?;
    let source = std::fs::read_to_string(&edge.file).ok()?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&crate::ast::get_ts_language(lang)?)
        .ok()?;
    let tree = parser.parse(&source, None)?;
    let base: usize = source
        .split_inclusive('\n')
        .take(edge.line as usize)
        .map(str::len)
        .sum();
    let byte = (base + edge.col as usize).min(source.len());
    let mut node = tree.root_node().descendant_for_byte_range(byte, byte)?;
    while !matches!(
        node.kind(),
        "call_expression"
            | "method_call_expression"
            | "call"
            | "new_expression"
            | "method_invocation"
            | "object_creation_expression"
            | "macro_invocation"
    ) {
        node = node.parent()?;
    }
    let function = node
        .child_by_field_name("function")
        .or_else(|| node.child_by_field_name("method"))
        .or_else(|| node.child_by_field_name("name"))
        .or_else(|| node.child_by_field_name("constructor"))
        .or_else(|| node.child_by_field_name("type"))
        .or_else(|| node.named_child(0))?;
    let expression = if lang == "rust" {
        rust_path(function, source.as_bytes())
    } else if matches!(
        function.kind(),
        "identifier" | "simple_identifier" | "type_identifier"
    ) {
        function
            .utf8_text(source.as_bytes())
            .ok()
            .map(str::to_owned)
    } else {
        None
    };
    let mut stack = vec![function];
    let mut identifier = None;
    while let Some(n) = stack.pop() {
        if n.kind() == "type_arguments" {
            continue;
        }
        if n.named_child_count() == 0
            && n.utf8_text(source.as_bytes()).ok() == Some(&edge.callee_sym)
        {
            identifier = Some(n);
        }
        let mut cursor = n.walk();
        stack.extend(n.named_children(&mut cursor));
    }
    let identifier = identifier.unwrap_or(function);
    let start = identifier.start_position();
    let line_text = source.split('\n').nth(start.row)?;
    let col = line_text.get(..start.column)?.encode_utf16().count() as u32;
    let mut modules = Vec::new();
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if parent.kind() == "mod_item" {
            modules.push(
                parent
                    .child_by_field_name("name")?
                    .utf8_text(source.as_bytes())
                    .ok()?
                    .to_owned(),
            );
        }
        ancestor = parent.parent();
    }
    modules.reverse();
    let lexical_binding = lang == "rust"
        && expression.as_deref().is_some_and(|expression| {
            super::rust_lexical::uncertain(node, source.as_bytes(), expression)
        });
    Some(CallSite {
        expression,
        line: start.row as u32,
        col,
        modules,
        lexical_binding,
    })
}

pub(super) async fn next(
    client: &dyn LspClientOps,
    root: &Path,
    lang: &str,
    cap: usize,
    edge: &Edge,
    direction: Direction,
) -> Placement {
    let name = match direction {
        Direction::Callers => &edge.caller_sym,
        Direction::Callees => &edge.callee_sym,
    };
    if let Some(target) = &edge.target {
        return Placement::Found(target.clone());
    }
    if direction == Direction::Callers || edge.source == EdgeSource::Lsp {
        // Legacy/incomplete hierarchy metadata: a unique declaration in its actual file is safe.
        if let Ok(symbols) = symbols_in(&edge.file) {
            let candidates = matching(&symbols, name);
            if direction == Direction::Callers {
                if let Some(found) = candidates
                    .iter()
                    .filter(|s| {
                        s.start_line <= edge.line
                            && s.end_line >= edge.line
                            && (s.start_line != edge.line || s.start_col <= edge.col)
                    })
                    .max_by_key(|s| (s.start_line, s.start_col))
                {
                    return Placement::Found(position(found));
                }
            }
            match unique(&symbols, name) {
                Placement::Missing => {}
                placed => return placed,
            }
        }
        if let Ok(symbols) = client.workspace_symbols(name).await {
            match unique(&symbols, name) {
                Placement::Missing => {}
                placed => return placed,
            }
        }
        return workspace(root, lang, name, cap);
    }
    let Some(site) = call_site(edge) else {
        return Placement::Unresolved(format!(
            "could not locate call-site context in {}:{}",
            edge.file.display(),
            edge.line + 1
        ));
    };
    let language = crate::lsp::servers::lsp_language_id(
        crate::ast::detect_language(&edge.file)
            .unwrap_or(crate::lsp::servers::ast_language_for_lsp_id(lang)),
    );
    if let Ok(locations) = client
        .goto_definition(&edge.file, site.line, site.col, language)
        .await
    {
        let mut targets = locations
            .into_iter()
            .filter_map(|loc| {
                let path = url::Url::parse(loc.uri.as_str())
                    .ok()?
                    .to_file_path()
                    .ok()?;
                Some(Definition {
                    file: path,
                    line: loc.range.start.line,
                    col: loc.range.start.character,
                })
            })
            .collect::<Vec<_>>();
        targets.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
        targets.dedup();
        match targets.as_slice() {
            [one] => return Placement::Found(one.clone()),
            [] => {}
            _ => return Placement::Ambiguous,
        }
    }
    if site.lexical_binding {
        return Placement::Unresolved(
            "lexical binding needs semantic target resolution".to_owned(),
        );
    }
    let Some(expression) = site.expression else {
        return Placement::Unresolved(
            "receiver or callable expression needs semantic target resolution".to_owned(),
        );
    };
    if crate::ast::detect_language(&edge.file) == Some("rust") {
        match super::rust_target::resolve(root, &edge.file, &site.modules, &expression) {
            Ok(Some(target)) => return Placement::Found(target),
            Ok(None) => {}
            Err(reason) => return Placement::Unresolved(reason),
        }
    }
    workspace(root, lang, name, cap)
}
