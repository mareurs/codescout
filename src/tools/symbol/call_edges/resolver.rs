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
    match direction {
        Direction::Callees => {
            // LIMIT-001 fix: walk the AST descendants of the enclosing
            // function and collect every direct call expression.
            resolve_callees_via_ts(sym_name, sym_path, sym_line, sym_col, language_id)
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

                let Some(tree) = parse_ts_tree(&src, language_id) else {
                    continue;
                };

                let byte = position_to_byte(&src, loc.range.start.line, loc.range.start.character);

                if !super::ts_classifier::position_is_call(&tree, byte, language_id) {
                    continue;
                }

                let caller = enclosing_function_name(&tree, &src, byte, language_id)
                    .unwrap_or_else(|| "<anonymous>".to_owned());

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
        match node.parent() {
            Some(p) => node = p,
            None => return None,
        }
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

    let root = tree.root_node();
    let mut node = root.descendant_for_byte_range(byte_offset, byte_offset)?;
    loop {
        if fn_kinds.contains(&node.kind()) {
            return Some(node);
        }
        match node.parent() {
            Some(p) => node = p,
            None => return None,
        }
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

        let fixture_uri = format!("file://{}", fixture.to_string_lossy());

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
