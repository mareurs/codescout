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
            // Finding callees requires knowing which calls appear *inside* the symbol
            // body.  `references()` finds refs *to* the symbol, not *from* it.
            // Without LSP callHierarchy we have no reliable way to enumerate callees.
            Err(RecoverableError::with_hint(
                "call_graph direction=callees requires LSP callHierarchy support (not available for this language/file)",
                "Activate a language server for this file, or use direction=callers which has a tree-sitter fallback.",
            )
            .into())
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

/// Convert an LSP `file://` URI to a [`PathBuf`].
///
/// Delegates to `url::Url` for correct handling of Windows drive letters,
/// UNC paths, and percent-encoding.  Falls back to the raw path string if
/// the URI cannot be parsed.
fn lsp_uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    url::Url::parse(uri.as_str())
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .or_else(|| {
            let s = uri.path().as_str();
            if s.is_empty() {
                None
            } else {
                Some(PathBuf::from(s))
            }
        })
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
        // Mock returns None from prepare_call_hierarchy (map is empty)
        let mock = MockLspClient::new();

        let result = resolve_one_hop(
            &mock,
            "foo",
            std::path::Path::new("/a.rs"),
            0,
            0,
            "rust",
            Direction::Callees,
        )
        .await;

        let err = result.expect_err("expected an error for callees without LSP");
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err}"
        );
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
