use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tools::RecoverableError;

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Callers,
    Callees,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeSource {
    Lsp,
    Ts,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub caller_sym: String,
    pub callee_sym: String,
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    pub source: EdgeSource,
}

// ── Main entry point ─────────────────────────────────────────────────────────

/// Resolve one hop of the call graph for the symbol at `(sym_path, sym_line, sym_col)`.
///
/// ## Strategy
///
/// 1. **LSP path** — if `prepare_call_hierarchy` returns `Some(item)`:
///    - `Direction::Callers`: calls `incoming_calls` and maps each result to an `Edge`.
///    - `Direction::Callees`: calls `outgoing_calls` and maps each result to an `Edge`.
///
/// 2. **Tree-sitter fallback** — if `prepare_call_hierarchy` returns `None`:
///    - `Direction::Callers`: uses `references()` + tree-sitter to filter real call
///      sites and walk up the AST to find the enclosing function name.
///    - `Direction::Callees`: returns a [`RecoverableError`] — finding callees without
///      LSP call-hierarchy support is not viable (references() finds refs *to* a symbol,
///      not calls *from* it).
pub async fn resolve_one_hop(
    client: &dyn crate::lsp::ops::LspClientOps,
    sym_name: &str,
    sym_path: &std::path::Path,
    sym_line: u32,
    sym_col: u32,
    language_id: &str,
    direction: Direction,
) -> anyhow::Result<Vec<Edge>> {
    let item = client
        .prepare_call_hierarchy(sym_path, sym_line, sym_col, language_id)
        .await?;

    match item {
        Some(hier_item) => {
            resolve_via_lsp(client, sym_name, language_id, direction, &hier_item).await
        }
        None => {
            resolve_via_ts(
                client,
                sym_name,
                sym_path,
                sym_line,
                sym_col,
                language_id,
                direction,
            )
            .await
        }
    }
}

// ── LSP path ─────────────────────────────────────────────────────────────────

async fn resolve_via_lsp(
    client: &dyn crate::lsp::ops::LspClientOps,
    sym_name: &str,
    language_id: &str,
    direction: Direction,
    item: &lsp_types::CallHierarchyItem,
) -> anyhow::Result<Vec<Edge>> {
    match direction {
        Direction::Callers => {
            let calls = client.incoming_calls(item, language_id).await?;
            let mut edges = Vec::new();
            for c in &calls {
                let file = lsp_uri_to_path(&c.from.uri)
                    .unwrap_or_else(|| PathBuf::from(c.from.uri.path().as_str()));
                for range in &c.from_ranges {
                    edges.push(Edge {
                        caller_sym: c.from.name.clone(),
                        callee_sym: sym_name.to_owned(),
                        file: file.clone(),
                        line: range.start.line,
                        col: range.start.character,
                        source: EdgeSource::Lsp,
                    });
                }
                // If from_ranges is empty, emit one edge at the symbol's own position.
                if c.from_ranges.is_empty() {
                    edges.push(Edge {
                        caller_sym: c.from.name.clone(),
                        callee_sym: sym_name.to_owned(),
                        file: file.clone(),
                        line: c.from.range.start.line,
                        col: c.from.range.start.character,
                        source: EdgeSource::Lsp,
                    });
                }
            }
            Ok(edges)
        }
        Direction::Callees => {
            let calls = client.outgoing_calls(item, language_id).await?;
            let mut edges = Vec::new();
            for c in &calls {
                let file = lsp_uri_to_path(&c.to.uri)
                    .unwrap_or_else(|| PathBuf::from(c.to.uri.path().as_str()));
                for range in &c.from_ranges {
                    edges.push(Edge {
                        caller_sym: sym_name.to_owned(),
                        callee_sym: c.to.name.clone(),
                        file: file.clone(),
                        line: range.start.line,
                        col: range.start.character,
                        source: EdgeSource::Lsp,
                    });
                }
                // If from_ranges is empty, emit one edge at the callee's own position.
                if c.from_ranges.is_empty() {
                    edges.push(Edge {
                        caller_sym: sym_name.to_owned(),
                        callee_sym: c.to.name.clone(),
                        file: file.clone(),
                        line: c.to.range.start.line,
                        col: c.to.range.start.character,
                        source: EdgeSource::Lsp,
                    });
                }
            }
            Ok(edges)
        }
    }
}

// ── Tree-sitter fallback ─────────────────────────────────────────────────────

async fn resolve_via_ts(
    client: &dyn crate::lsp::ops::LspClientOps,
    sym_name: &str,
    sym_path: &std::path::Path,
    sym_line: u32,
    sym_col: u32,
    language_id: &str,
    direction: Direction,
) -> anyhow::Result<Vec<Edge>> {
    // `language_id` is the LSP languageId (`typescriptreact`, ...), right for the LSP calls
    // below. Every tree-sitter helper keys on the AST vocabulary (`tsx`, ...): handed the
    // languageId, `call_kinds_for` is empty and `get_ts_language` finds no grammar, so a
    // React file's fallback silently produced nothing (BUG 2026-10-17
    // call-graph-phase-b-compares-the-lsp-language-id-to-the-ast-language).
    let ast_lang = crate::lsp::servers::ast_language_for_lsp_id(language_id);
    match direction {
        Direction::Callees => {
            // LIMIT-001 fix: walk the AST descendants of the enclosing
            // function and collect every direct call expression. Parsed with the grammar of
            // the symbol's OWN file (the seed's id only decides when the path says nothing):
            // Phase B places a `.tsx` seed's callee in a `.ts` file and the reverse, and the
            // wrong grammar silently yields a partial edge set.
            let node_lang = crate::ast::detect_language(sym_path).unwrap_or(ast_lang);
            resolve_callees_via_ts(sym_name, sym_path, sym_line, sym_col, node_lang)
        }
        Direction::Callers => {
            let refs = client
                .references(sym_path, sym_line, sym_col, language_id)
                .await?;

            let mut edges = Vec::new();

            for loc in &refs {
                let ref_path = lsp_uri_to_path(&loc.uri)
                    .unwrap_or_else(|| PathBuf::from(loc.uri.path().as_str()));

                let src = match std::fs::read_to_string(&ref_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // The reference's own file decides its grammar (a `.ts` file referencing a
                // `.tsx` symbol parses as TypeScript); the seed's language is the fallback.
                let file_lang = crate::ast::detect_language(&ref_path).unwrap_or(ast_lang);

                let Some(tree) = parse_ts_tree(&src, file_lang) else {
                    continue;
                };

                let byte = position_to_byte(&src, loc.range.start.line, loc.range.start.character);

                if !super::ts_classifier::position_is_call(&tree, byte, file_lang) {
                    continue;
                }

                let caller = enclosing_function_name(&tree, &src, byte, file_lang)
                    .unwrap_or_else(|| "<anonymous>".to_owned());

                // Skip self-edges: when the LSP can't model a symbol (e.g.
                // #[test] fns), `references` may return locations inside the
                // symbol's own body, which the AST walk-up resolves back to
                // the symbol itself. Real self-recursion goes through
                // `incoming_calls` (LSP path), not this fallback.
                if caller == sym_name {
                    continue;
                }

                edges.push(Edge {
                    caller_sym: caller,
                    callee_sym: sym_name.to_owned(),
                    file: ref_path,
                    line: loc.range.start.line,
                    col: loc.range.start.character,
                    source: EdgeSource::Ts,
                });
            }

            Ok(edges)
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Convert an LSP URI to a filesystem path.
///
/// Delegates to [`crate::util::file_address::FileAddress::from_lsp_uri`].
fn lsp_uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    crate::util::file_address::FileAddress::from_lsp_uri(uri)
        .map(crate::util::file_address::FileAddress::into_path)
}

/// Convert a `(line, col)` pair (0-indexed, UTF-16 columns) to a byte offset
/// within `src`.
///
/// We treat `col` as a UTF-8 byte offset within the line — a slight
/// simplification that is correct for ASCII identifiers (which covers all
/// realistic symbol names) and sufficient for call-site detection.
fn position_to_byte(src: &str, line: u32, col: u32) -> usize {
    let mut current_line = 0u32;
    let mut offset = 0usize;
    for ch in src.chars() {
        if current_line == line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
        }
        offset += ch.len_utf8();
    }
    // Advance by col bytes within the line (clamped to line length).
    let remaining = &src[offset..];
    let col_bytes = (col as usize).min(remaining.len());
    offset + col_bytes
}

/// Parse `src` with the tree-sitter grammar for `language_id`.
///
/// Returns `None` if the language is not supported or the parse fails.
fn parse_ts_tree(src: &str, language_id: &str) -> Option<tree_sitter::Tree> {
    let lang = crate::ast::get_ts_language(language_id)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).ok()?;
    parser.parse(src, None)
}

/// Walk up the tree-sitter AST from `byte_offset` to find the innermost
/// enclosing function/method node, and return its declared name.
///
/// `src` must be the same source text that was used to produce `tree`.
///
/// Recognised node kinds per language:
/// - Rust: `function_item`
/// - Python: `function_definition`
/// - TypeScript/JavaScript/TSX/JSX: `function_declaration`, `method_definition`, `arrow_function`
/// - Kotlin: `function_declaration`
/// - Java: `method_declaration`
///
/// Returns `None` if no enclosing function can be found (e.g. top-level code).
fn enclosing_function_name(
    tree: &tree_sitter::Tree,
    src: &str,
    byte_offset: usize,
    language_id: &str,
) -> Option<String> {
    let fn_kinds: &[&str] = match language_id {
        "rust" => &["function_item"],
        "python" => &["function_definition"],
        "typescript" | "javascript" | "tsx" | "jsx" => &[
            "function_declaration",
            "method_definition",
            "arrow_function",
        ],
        "kotlin" => &["function_declaration"],
        "java" => &["method_declaration"],
        _ => return None,
    };

    let src_bytes = src.as_bytes();
    let root = tree.root_node();
    let mut node = root.descendant_for_byte_range(byte_offset, byte_offset)?;

    loop {
        if fn_kinds.contains(&node.kind()) {
            // Walk immediate children to find the name identifier node.
            for i in 0..node.child_count() {
                let child = node.child(i as u32)?;
                if matches!(
                    child.kind(),
                    "identifier" | "simple_identifier" | "property_identifier"
                ) {
                    return child.utf8_text(src_bytes).ok().map(str::to_owned);
                }
            }
            // Function node found but no name child (e.g. anonymous arrow function).
            return None;
        }
        node = node.parent()?;
    }
}

/// Tree-sitter call-kind node names per language.
///
/// Mirrors the set used by `position_is_call`, but exposed as a slice so the
/// callees fallback can pre-filter descendant nodes during the AST walk.
/// Returns an empty slice for languages we don't classify.
fn call_kinds_for(language_id: &str) -> &'static [&'static str] {
    match language_id {
        "rust" => &[
            "call_expression",
            "method_call_expression",
            "macro_invocation",
        ],
        "python" => &["call"],
        "typescript" | "javascript" | "tsx" | "jsx" => &["call_expression", "new_expression"],
        "kotlin" => &["call_expression"],
        "java" => &["method_invocation", "object_creation_expression"],
        _ => &[],
    }
}

/// Walk up the AST from `byte_offset` to find the innermost enclosing
/// function/method node. Returns the node itself (rather than just its name
/// like `enclosing_function_name`) so callers can recurse into its body.
fn enclosing_function_node<'tree>(
    tree: &'tree tree_sitter::Tree,
    byte_offset: usize,
    language_id: &str,
) -> Option<tree_sitter::Node<'tree>> {
    let is_ts = matches!(language_id, "typescript" | "javascript" | "tsx" | "jsx");
    let fn_kinds: &[&str] = match language_id {
        "rust" => &["function_item"],
        "python" => &["function_definition"],
        "typescript" | "javascript" | "tsx" | "jsx" => &[
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function_expression",
        ],
        "kotlin" => &["function_declaration"],
        "java" => &["method_declaration"],
        _ => return None,
    };

    let root = tree.root_node();
    let mut node = root.descendant_for_byte_range(byte_offset, byte_offset)?;
    loop {
        if fn_kinds.contains(&node.kind()) {
            return Some(node);
        }
        // TS/JS: `const f = () => {}` / `const f = function () {}` resolve to the
        // binding *name*, whose ancestors do NOT include the function body (it is
        // the declarator's sibling `value`, not an ancestor). Descend into the
        // value so the callee scan sees the body.
        // (2026-06-04-call-graph-ts-arrow-const-callees)
        if is_ts && node.kind() == "variable_declarator" {
            if let Some(value) = node.child_by_field_name("value") {
                if matches!(value.kind(), "arrow_function" | "function_expression") {
                    return Some(value);
                }
            }
        }
        node = node.parent()?;
    }
}

/// Extract the callee identifier from a call-expression node.
///
/// Returns the rightmost / final segment of the call target:
/// - Rust: `b()` → `b`; `obj.m()` → `m`; `d::e()` → `e`; `m!(...)` → `m`.
/// - Python: `b()` → `b`; `o.c()` → `c`.
/// - TS/JS/TSX/JSX: `b()` → `b`; `o.c()` → `c`; `new D(...)` → `D`.
/// - Kotlin: `b()` → `b`; `o.c()` → `c`.
/// - Java: `b()` → `b`; `o.c()` → `c`; `new D(...)` → `D`.
fn callee_identifier(node: tree_sitter::Node<'_>, src: &str, language_id: &str) -> Option<String> {
    let src_bytes = src.as_bytes();
    let text = |n: tree_sitter::Node<'_>| n.utf8_text(src_bytes).ok().map(str::to_owned);

    // Walk a "scoped"/"path"/"member"/"navigation" expression down to its
    // rightmost identifier-like child.
    fn rightmost_ident<'a>(mut n: tree_sitter::Node<'a>, src_bytes: &[u8]) -> Option<String> {
        loop {
            match n.kind() {
                "identifier"
                | "simple_identifier"
                | "property_identifier"
                | "type_identifier"
                | "field_identifier"
                | "shorthand_property_identifier" => {
                    return n.utf8_text(src_bytes).ok().map(str::to_owned);
                }
                _ => {}
            }
            // Drop to the last non-trivial named child and keep descending.
            let count = n.named_child_count() as u32;
            if count == 0 {
                return n.utf8_text(src_bytes).ok().map(str::to_owned);
            }
            let mut next: Option<tree_sitter::Node<'a>> = None;
            for i in (0..count).rev() {
                if let Some(c) = n.named_child(i) {
                    next = Some(c);
                    break;
                }
            }
            match next {
                Some(c) if c.id() != n.id() => n = c,
                _ => return n.utf8_text(src_bytes).ok().map(str::to_owned),
            }
        }
    }

    match (language_id, node.kind()) {
        // ── Rust ──────────────────────────────────────────────────────────
        ("rust", "call_expression") => {
            let f = node.child_by_field_name("function")?;
            match f.kind() {
                "identifier" => text(f),
                _ => rightmost_ident(f, src_bytes),
            }
        }
        ("rust", "method_call_expression") => {
            let name = node.child_by_field_name("method")?;
            text(name)
        }
        ("rust", "macro_invocation") => {
            let name = node.child_by_field_name("macro")?;
            // `macro` can be `identifier` or `scoped_identifier`.
            match name.kind() {
                "identifier" => text(name),
                _ => rightmost_ident(name, src_bytes),
            }
        }
        // ── Python ────────────────────────────────────────────────────────
        ("python", "call") => {
            let f = node.child_by_field_name("function")?;
            match f.kind() {
                "identifier" => text(f),
                "attribute" => {
                    let attr = f.child_by_field_name("attribute")?;
                    text(attr)
                }
                _ => rightmost_ident(f, src_bytes),
            }
        }
        // ── TS / JS / TSX / JSX ──────────────────────────────────────────
        ("typescript" | "javascript" | "tsx" | "jsx", "call_expression") => {
            let f = node.child_by_field_name("function")?;
            match f.kind() {
                "identifier" => text(f),
                "member_expression" => {
                    let p = f.child_by_field_name("property")?;
                    text(p)
                }
                _ => rightmost_ident(f, src_bytes),
            }
        }
        ("typescript" | "javascript" | "tsx" | "jsx", "new_expression") => {
            let c = node.child_by_field_name("constructor")?;
            match c.kind() {
                "identifier" => text(c),
                _ => rightmost_ident(c, src_bytes),
            }
        }
        // ── Kotlin ────────────────────────────────────────────────────────
        ("kotlin", "call_expression") => {
            // Kotlin grammar: the callee is the first named child; for
            // `obj.foo()` the call_expression is wrapped in a
            // navigation_expression where the rightmost simple_identifier is
            // the method name. For bare `foo()` the first child is the
            // simple_identifier directly.
            let first = node.named_child(0)?;
            rightmost_ident(first, src_bytes)
        }
        // ── Java ──────────────────────────────────────────────────────────
        ("java", "method_invocation") => {
            let name = node.child_by_field_name("name")?;
            text(name)
        }
        ("java", "object_creation_expression") => {
            let typ = node.child_by_field_name("type")?;
            match typ.kind() {
                "type_identifier" => text(typ),
                _ => rightmost_ident(typ, src_bytes),
            }
        }
        _ => None,
    }
}

/// Tree-sitter fallback for `Direction::Callees` — used when LSP
/// `callHierarchy` is unavailable for the language/file.
///
/// Strategy: parse the source, locate the function node enclosing the symbol
/// at `(sym_line, sym_col)`, then walk its descendants collecting every
/// call-expression node. For each call we extract the callee identifier with
/// per-language rules (see [`callee_identifier`]) and emit one [`Edge`].
///
/// Returns `RecoverableError` if the language is not in the supported set
/// (Rust, Python, TS/JS/TSX/JSX, Kotlin, Java), if the source can't be read,
/// or if no enclosing function can be located.
fn resolve_callees_via_ts(
    sym_name: &str,
    sym_path: &std::path::Path,
    sym_line: u32,
    sym_col: u32,
    language_id: &str,
) -> anyhow::Result<Vec<Edge>> {
    let kinds = call_kinds_for(language_id);
    if kinds.is_empty() {
        return Err(RecoverableError::with_hint(
            "call_graph direction=callees requires LSP callHierarchy support (not available for this language/file)",
            "Activate a language server for this file, or use direction=callers which has a tree-sitter fallback.",
        )
        .into());
    }

    let src = std::fs::read_to_string(sym_path).map_err(|e| {
        RecoverableError::with_hint(
            format!("could not read source for callees fallback: {e}"),
            "Verify the file path resolves to a readable file.",
        )
    })?;

    let tree = parse_ts_tree(&src, language_id).ok_or_else(|| {
        RecoverableError::with_hint(
            "tree-sitter parse failed for callees fallback",
            "The grammar may not be registered for this language; activate an LSP if available.",
        )
    })?;

    let byte = position_to_byte(&src, sym_line, sym_col);
    let fn_node = enclosing_function_node(&tree, byte, language_id).ok_or_else(|| {
        RecoverableError::with_hint(
            "could not locate enclosing function for callees fallback",
            "Ensure (sym_line, sym_col) points inside a function/method body.",
        )
    })?;

    let mut edges = Vec::new();
    let mut cursor = fn_node.walk();
    let mut stack: Vec<tree_sitter::Node<'_>> = vec![fn_node];
    while let Some(n) = stack.pop() {
        if kinds.contains(&n.kind()) {
            if let Some(callee) = callee_identifier(n, &src, language_id) {
                let start = n.start_position();
                edges.push(Edge {
                    caller_sym: sym_name.to_owned(),
                    callee_sym: callee,
                    file: sym_path.to_path_buf(),
                    line: start.row as u32,
                    col: start.column as u32,
                    source: EdgeSource::Ts,
                });
            }
        }
        for child in n.children(&mut cursor) {
            stack.push(child);
        }
    }

    Ok(edges)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::mock::MockLspClient;

    // ── Test 1: LSP success — Callers ────────────────────────────────────────

    #[tokio::test]
    async fn resolve_one_hop_uses_lsp_when_available() {
        let mock = MockLspClient::new();

        // Build the CallHierarchyItem for "a" at a.rs:10:5
        let a_uri: lsp_types::Uri = "file:///a.rs".parse().unwrap();
        let a_item = lsp_types::CallHierarchyItem {
            name: "a".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: a_uri.clone(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 5,
                },
                end: lsp_types::Position {
                    line: 15,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 5,
                },
                end: lsp_types::Position {
                    line: 10,
                    character: 6,
                },
            },
            data: None,
        };

        // Seed prepare_call_hierarchy
        mock.prepare_call_hierarchy_results.lock().unwrap().insert(
            (std::path::PathBuf::from("/a.rs"), 10, 5),
            Some(a_item.clone()),
        );

        // Build caller "b" at b.rs:3:0
        let b_uri: lsp_types::Uri = "file:///b.rs".parse().unwrap();
        let b_item = lsp_types::CallHierarchyItem {
            name: "b".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: b_uri,
            range: lsp_types::Range::default(),
            selection_range: lsp_types::Range::default(),
            data: None,
        };
        let incoming = lsp_types::CallHierarchyIncomingCall {
            from: b_item,
            from_ranges: vec![lsp_types::Range {
                start: lsp_types::Position {
                    line: 3,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 3,
                    character: 1,
                },
            }],
        };

        // Seed incoming_calls keyed by "a"
        mock.incoming_calls_results
            .lock()
            .unwrap()
            .insert("a".into(), vec![incoming]);

        let edges = resolve_one_hop(
            &mock,
            "a",
            std::path::Path::new("/a.rs"),
            10,
            5,
            "rust",
            Direction::Callers,
        )
        .await
        .unwrap();

        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert_eq!(e.source, EdgeSource::Lsp);
        assert_eq!(e.caller_sym, "b");
        assert_eq!(e.callee_sym, "a");
        assert_eq!(e.line, 3);
        assert_eq!(e.col, 0);
    }

    // ── Test 2: TS fallback — Callers ────────────────────────────────────────

    #[tokio::test]
    async fn resolve_one_hop_ts_fallback_callers() {
        // Rust source fixture: "bar" calls "foo"
        let src = "fn bar() { foo(1); }\nfn foo(x: i32) {}\n";

        // Write fixture to a temp file
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.rs");
        std::fs::write(&fixture, src).unwrap();

        let fixture_uri = {
            // Build file:// URI in a way that works on both Unix and Windows.
            // Unix: /tmp/foo            → file:///tmp/foo (already starts with /)
            // Windows: C:\Users\foo    → file:///C:/Users/foo
            let path_str = fixture.to_string_lossy().replace('\\', "/");
            if path_str.starts_with('/') {
                format!("file://{}", path_str)
            } else {
                format!("file:///{}", path_str)
            }
        };

        // Build a reference location pointing at the "foo" identifier inside `bar`
        // "fn bar() { foo(1); }" — "foo" starts at col 11 on line 0
        let ref_loc = lsp_types::Location {
            uri: fixture_uri.parse::<lsp_types::Uri>().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 11,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 14,
                },
            },
        };

        // Mock: prepare_call_hierarchy returns None (no LSP support)
        // references returns our fixture location
        let mock = MockLspClient::new();
        // prepare_call_hierarchy_results is empty → returns None for any key
        mock.references_results
            .lock()
            .unwrap()
            .insert(fixture.clone(), vec![ref_loc]);

        let edges = resolve_one_hop(
            &mock,
            "foo",
            &fixture,
            1, // sym_line for "foo" (line 1)
            3, // sym_col
            "rust",
            Direction::Callers,
        )
        .await
        .unwrap();

        // Exactly one call-site edge, coming from "bar"
        assert_eq!(edges.len(), 1, "expected 1 edge, got {:?}", edges);
        let e = &edges[0];
        assert_eq!(e.source, EdgeSource::Ts);
        assert_eq!(e.caller_sym, "bar");
        assert_eq!(e.callee_sym, "foo");
    }

    /// Regression test for docs/issues/archive/2026-05-25-call-graph-tree-sitter-self-edges.md.
    /// Pre-fix: resolve_one_hop emits a spurious self-edge when references
    /// returns a location inside the queried symbol's own body (which happens
    /// for symbols RA cannot fully model, e.g. #[test] fns).
    #[tokio::test]
    async fn resolve_one_hop_ts_fallback_callers_skips_self_edge() {
        let src = "fn bar(x: i32) {}\nfn foo() { bar(1); }\n";

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.rs");
        std::fs::write(&fixture, src).unwrap();

        let fixture_uri = {
            let path_str = fixture.to_string_lossy().replace('\\', "/");
            if path_str.starts_with('/') {
                format!("file://{}", path_str)
            } else {
                format!("file:///{}", path_str)
            }
        };

        // Reference INSIDE foo's body — at the `bar(1)` call site.
        // Simulates RA returning internal call sites (rather than external
        // callers) when asked references on a symbol it doesn't fully model.
        let ref_loc = lsp_types::Location {
            uri: fixture_uri.parse::<lsp_types::Uri>().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 1,
                    character: 11,
                },
                end: lsp_types::Position {
                    line: 1,
                    character: 14,
                },
            },
        };

        let mock = MockLspClient::new();
        mock.references_results
            .lock()
            .unwrap()
            .insert(fixture.clone(), vec![ref_loc]);

        let edges = resolve_one_hop(&mock, "foo", &fixture, 1, 3, "rust", Direction::Callers)
            .await
            .unwrap();

        for e in &edges {
            assert_ne!(
                (e.caller_sym.as_str(), e.callee_sym.as_str()),
                ("foo", "foo"),
                "spurious self-edge: tree-sitter fallback resolved a reference \
                 inside foo's own body as a caller-of-foo edge. \
                 Pre-fix this returned 1 edge foo→foo; post-fix should return 0."
            );
        }
    }

    // ── Test 3: Callees with no LSP → RecoverableError ───────────────────────

    #[tokio::test]
    async fn resolve_one_hop_callees_without_lsp_returns_recoverable_error() {
        // Mock returns None from prepare_call_hierarchy (map is empty).
        // Use an unsupported language ("go") so the TS callees fallback is not
        // available either, exercising the RecoverableError branch.
        let mock = MockLspClient::new();

        let result = resolve_one_hop(
            &mock,
            "foo",
            std::path::Path::new("/a.go"),
            0,
            0,
            "go",
            Direction::Callees,
        )
        .await;

        let err = result.expect_err("expected an error for callees without LSP");
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err}"
        );
    }

    // ── Test 4: TS fallback — Callees (LIMIT-001 fix) ────────────────────────

    #[tokio::test]
    async fn resolve_callees_via_ts_rust_finds_direct_calls() {
        let src = "fn a() {\n    b();\n    c();\n    d::e();\n}\nfn b() {}\nfn c() {}\nmod d { pub fn e() {} }\n";

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.rs");
        std::fs::write(&fixture, src).unwrap();

        // Mock: prepare_call_hierarchy returns None → forces TS fallback.
        let mock = MockLspClient::new();

        let edges = resolve_one_hop(
            &mock,
            "a",
            &fixture,
            0, // sym_line — start of `fn a`
            3, // sym_col — inside the identifier `a`
            "rust",
            Direction::Callees,
        )
        .await
        .unwrap();

        let callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(callees.contains(&"b"), "missing b in {:?}", callees);
        assert!(callees.contains(&"c"), "missing c in {:?}", callees);
        assert!(callees.contains(&"e"), "missing e in {:?}", callees);
        for e in &edges {
            assert_eq!(e.caller_sym, "a", "wrong caller in {:?}", e);
            assert_eq!(e.source, EdgeSource::Ts);
        }
        assert_eq!(edges.len(), 3, "unexpected edges: {:?}", edges);
    }

    #[tokio::test]
    async fn resolve_callees_via_ts_python_finds_direct_calls() {
        let src = "def a():\n    b()\n    obj.c()\n\ndef b():\n    pass\n";

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.py");
        std::fs::write(&fixture, src).unwrap();

        let mock = MockLspClient::new();

        let edges = resolve_one_hop(
            &mock,
            "a",
            &fixture,
            0,
            4, // sym_col inside `a`
            "python",
            Direction::Callees,
        )
        .await
        .unwrap();

        let callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(callees.contains(&"b"), "missing b in {:?}", callees);
        assert!(callees.contains(&"c"), "missing c in {:?}", callees);
    }

    #[tokio::test]
    async fn resolve_callees_via_ts_typescript_finds_direct_calls() {
        let src = "function a() {\n    b();\n    obj.c();\n    new D();\n}\nfunction b() {}\n";

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.ts");
        std::fs::write(&fixture, src).unwrap();

        let mock = MockLspClient::new();

        let edges = resolve_one_hop(
            &mock,
            "a",
            &fixture,
            0,
            9, // sym_col inside `a`
            "typescript",
            Direction::Callees,
        )
        .await
        .unwrap();

        let callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(callees.contains(&"b"), "missing b in {:?}", callees);
        assert!(callees.contains(&"c"), "missing c in {:?}", callees);
        assert!(callees.contains(&"D"), "missing D in {:?}", callees);
    }
    /// `CachedResolver::lang` is the LSP languageId (`get_lsp_client`'s second value), so
    /// for a React file the resolver is handed `"typescriptreact"` / `"javascriptreact"`.
    /// The tree-sitter fallbacks key on the AST vocabulary (`"tsx"` / `"jsx"`); fed the
    /// languageId, `call_kinds_for` is empty and `direction=callees` answers
    /// "requires LSP callHierarchy" for a file the fallback fully supports.
    #[tokio::test]
    async fn resolve_callees_via_ts_accepts_the_lsp_language_id_of_react_files() {
        for (file, lsp_id, src) in [
            (
                "fixture.tsx",
                "typescriptreact",
                "function a() {\n    b();\n}\nfunction b() {}\n",
            ),
            (
                "fixture.jsx",
                "javascriptreact",
                "function a() {\n    b();\n}\nfunction b() {}\n",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let fixture = dir.path().join(file);
            std::fs::write(&fixture, src).unwrap();
            let mock = MockLspClient::new();

            let edges = resolve_one_hop(&mock, "a", &fixture, 0, 9, lsp_id, Direction::Callees)
                .await
                .unwrap_or_else(|e| panic!("{lsp_id}: callees fallback refused: {e}"));

            let callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
            assert_eq!(callees, vec!["b"], "{lsp_id}: {edges:?}");
        }
    }

    /// The callers fallback parses each reference's file with the same vocabulary.
    #[tokio::test]
    async fn resolve_callers_via_ts_accepts_the_lsp_language_id_of_react_files() {
        let src = "function bar() { foo(1); }\nfunction foo(x: number) {}\n";
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.tsx");
        std::fs::write(&fixture, src).unwrap();
        let uri: lsp_types::Uri = url::Url::from_file_path(&fixture)
            .unwrap()
            .as_str()
            .parse()
            .unwrap();
        // `foo(1)` inside `bar`: line 0, column 17.
        let ref_loc = lsp_types::Location {
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 17,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 20,
                },
            },
        };
        let mock = MockLspClient::new();
        mock.references_results
            .lock()
            .unwrap()
            .insert(fixture.clone(), vec![ref_loc]);

        let edges = resolve_one_hop(
            &mock,
            "foo",
            &fixture,
            1,
            9,
            "typescriptreact",
            Direction::Callers,
        )
        .await
        .unwrap();

        assert_eq!(edges.len(), 1, "expected the call site in bar: {edges:?}");
        assert_eq!(edges[0].caller_sym, "bar");
    }
    /// Phase B places a `.tsx` seed's callee in a `.ts` module and a `.ts` seed's callee in a
    /// `.tsx` file, so the node being resolved is not in the seed's language. Parsed with the
    /// seed's grammar, a `.ts` file's type assertion `<string>x` is a JSX tag and the call
    /// after it is lost (reviewer PROBE_A: `Ok([])`), and a `.tsx` file's JSX parsed as
    /// TypeScript loses the calls inside it (PROBE_B: `["format"]`, `track` missing). Both
    /// came back as a quiet, partial edge set.
    #[tokio::test]
    async fn resolve_callees_via_ts_uses_the_grammar_of_the_symbols_own_file() {
        for (file, seed_id, src, expected) in [
            (
                "util.ts",
                "typescriptreact",
                "export function outer() { const y = <string>x; return inner(y); }\nfunction inner(a: string) { return a; }\n",
                vec!["inner"],
            ),
            (
                "view.tsx",
                "typescript",
                "export function view() { return <div onClick={() => track()}>{format(1)}</div>; }\n",
                vec!["format", "track"],
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let fixture = dir.path().join(file);
            std::fs::write(&fixture, src).unwrap();
            let name = if file == "util.ts" { "outer" } else { "view" };
            let mock = MockLspClient::new();

            let edges = resolve_one_hop(&mock, name, &fixture, 0, 16, seed_id, Direction::Callees)
                .await
                .unwrap_or_else(|e| panic!("{file} via {seed_id}: {e}"));

            let mut callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
            callees.sort_unstable();
            assert_eq!(
                callees, expected,
                "{file} resolved with the seed's id {seed_id} lost calls: {edges:?}"
            );
        }
    }

    /// The callers fallback already parsed each reference with its own file's language; this
    /// pins it. The reference sits in a `.ts` file after a type assertion, which the TSX
    /// grammar (the seed's) cannot parse as TypeScript.
    #[tokio::test]
    async fn resolve_callers_via_ts_uses_the_grammar_of_the_references_own_file() {
        let src = "function bar() { const y = <string>x; foo(y); }\nfunction foo(a: string) {}\n";
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("a.ts");
        std::fs::write(&fixture, src).unwrap();
        let uri: lsp_types::Uri = url::Url::from_file_path(&fixture)
            .unwrap()
            .as_str()
            .parse()
            .unwrap();
        // `foo(y)` inside `bar`: line 0, column 38.
        let ref_loc = lsp_types::Location {
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 38,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 41,
                },
            },
        };
        let mock = MockLspClient::new();
        mock.references_results
            .lock()
            .unwrap()
            .insert(fixture.clone(), vec![ref_loc]);

        let edges = resolve_one_hop(
            &mock,
            "foo",
            &fixture,
            1,
            9,
            "typescriptreact",
            Direction::Callers,
        )
        .await
        .unwrap();

        assert_eq!(edges.len(), 1, "expected the call site in bar: {edges:?}");
        assert_eq!(edges[0].caller_sym, "bar");
    }

    #[tokio::test]
    async fn resolve_callees_via_ts_function_valued_consts() {
        // Regression (2026-06-04-call-graph-ts-arrow-const-callees): callees of
        // function-valued const bindings must resolve via the TS fallback. The
        // resolved symbol position is the binding *name*; the function body is the
        // declarator's sibling `value`, not an ancestor, so an ancestor-only walk
        // used to miss it (arrow_function in fn_kinds was unreachable from `a`).
        let src = "const a = () => {\n    b();\n    obj.c();\n};\nconst e = function () {\n    d();\n};\nfunction b() {}\n";
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.ts");
        std::fs::write(&fixture, src).unwrap();
        let mock = MockLspClient::new();

        // `const a = () => { b(); obj.c(); }` — line 0, col 6 = `a`
        let arrow = resolve_one_hop(&mock, "a", &fixture, 0, 6, "typescript", Direction::Callees)
            .await
            .unwrap();
        let ac: Vec<&str> = arrow.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(ac.contains(&"b"), "arrow: missing b in {:?}", ac);
        assert!(ac.contains(&"c"), "arrow: missing c in {:?}", ac);

        // `const e = function () { d(); }` — line 4, col 6 = `e`
        let fe = resolve_one_hop(&mock, "e", &fixture, 4, 6, "typescript", Direction::Callees)
            .await
            .unwrap();
        let ec: Vec<&str> = fe.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(
            ec.contains(&"d"),
            "function_expression: missing d in {:?}",
            ec
        );
    }

    #[tokio::test]
    async fn resolve_callees_via_ts_kotlin_finds_direct_calls() {
        let src = "fun a() {\n    b()\n    obj.c()\n}\nfun b() {}\n";

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("fixture.kt");
        std::fs::write(&fixture, src).unwrap();

        let mock = MockLspClient::new();

        let edges = resolve_one_hop(&mock, "a", &fixture, 0, 4, "kotlin", Direction::Callees)
            .await
            .unwrap();

        let callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(callees.contains(&"b"), "missing b in {:?}", callees);
        assert!(callees.contains(&"c"), "missing c in {:?}", callees);
    }

    #[tokio::test]
    async fn resolve_callees_via_ts_java_finds_direct_calls() {
        let src = "class X {\n    void a() {\n        b();\n        obj.c();\n    }\n    void b() {}\n}\n";

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("Fixture.java");
        std::fs::write(&fixture, src).unwrap();

        let mock = MockLspClient::new();

        let edges = resolve_one_hop(
            &mock,
            "a",
            &fixture,
            1,
            9, // sym_col inside `a`
            "java",
            Direction::Callees,
        )
        .await
        .unwrap();

        let callees: Vec<&str> = edges.iter().map(|e| e.callee_sym.as_str()).collect();
        assert!(callees.contains(&"b"), "missing b in {:?}", callees);
        assert!(callees.contains(&"c"), "missing c in {:?}", callees);
    }

    // ── Helper unit tests ────────────────────────────────────────────────────

    #[test]
    fn position_to_byte_first_line() {
        let src = "hello world\nsecond line\n";
        // col 6 on line 0 → byte 6 ('w')
        assert_eq!(position_to_byte(src, 0, 6), 6);
    }

    #[test]
    fn position_to_byte_second_line() {
        let src = "hello\nworld\n";
        // line 1, col 0 → byte 6 (start of "world")
        assert_eq!(position_to_byte(src, 1, 0), 6);
    }

    #[test]
    fn enclosing_function_name_rust() {
        let src = "fn my_func() { foo(); }\n";
        let tree = parse_ts_tree(src, "rust").unwrap();
        // byte offset of "foo" = 15
        let byte = position_to_byte(src, 0, 15);
        let name = enclosing_function_name(&tree, src, byte, "rust");
        assert_eq!(name.as_deref(), Some("my_func"));
    }

    #[test]
    fn lsp_uri_to_path_round_trips() {
        let uri: lsp_types::Uri = "file:///tmp/hello.rs".parse().unwrap();
        let path = lsp_uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/hello.rs"));
    }
}
