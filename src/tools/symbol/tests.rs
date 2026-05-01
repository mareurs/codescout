//! Unit tests for helper functions extracted from `mod.rs`.
//!
//! Moved wholesale during Phase 1b.4; imports made explicit in Phase 1b.5.

use super::display::{
    format_find_references, format_goto_definition, format_hover, format_overview_symbols,
    format_search_symbols,
};
use super::list_overview::{
    ast_class_names_for_dir, count_files_by_subdir, find_split_point, flat_symbol_count,
    LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP,
};
use super::path_helpers::{
    classify_reference_path, format_library_path, resolve_library_roots, tag_external_path,
    uri_to_path,
};
use super::symbols::{build_by_file, make_search_symbols_hint};
use super::*;
use crate::agent::Agent;
use crate::lsp::SymbolInfo;
use crate::symbol::edit::{
    apply_text_edits, clamp_range_to_parent, editing_end_line, editing_start_line,
    find_insert_before_line, text_sweep, write_lines,
};
use crate::symbol::query::{
    collect_matching, filter_variable_symbols, find_matching_symbol, find_symbol_by_name_path,
    find_unique_symbol_by_name_path, is_lead_in_line, matches_kind_filter, symbol_name_matches,
    symbol_to_json, validate_symbol_position, validate_symbol_range,
};
use crate::tools::{Tool, ToolContext};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
    crate::lsp::LspManager::new_arc()
}

fn buf() -> Arc<crate::tools::output_buffer::OutputBuffer> {
    Arc::new(crate::tools::output_buffer::OutputBuffer::new(20))
}

/// Substring predicate for `collect_matching` tests: case-insensitive match on name or name_path.
fn substr_pred(pat: &'static str) -> impl Fn(&SymbolInfo) -> bool {
    move |sym: &SymbolInfo| {
        sym.name.to_lowercase().contains(pat) || sym.name_path.to_lowercase().contains(pat)
    }
}

/// Create a test Cargo project and return the context.
async fn rust_project_ctx() -> Option<(tempfile::TempDir, ToolContext)> {
    if !std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return None;
    }

    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    let codescout_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&codescout_dir).unwrap();
    // Opt out of mux so these unit tests use rust-analyzer directly,
    // without needing the codescout-mux binary on PATH.
    std::fs::write(
        codescout_dir.join("project.toml"),
        "[project]\nname = \"test-project\"\n\n[lsp.rust]\nmux = false\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/main.rs"),
        r#"fn main() {
println!("hello");
}

fn add(a: i32, b: i32) -> i32 {
a + b
}

struct Point {
x: f64,
y: f64,
}

impl Point {
fn new(x: f64, y: f64) -> Self {
    Self { x, y }
}
}
"#,
    )
    .unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    Some((
        dir,
        ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: buf(),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        },
    ))
}

#[tokio::test]
async fn get_symbols_overview_returns_symbols() {
    let Some((_dir, ctx)) = rust_project_ctx().await else {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    };

    let result = Symbols
        .call(
            json!({
                "path": "src/main.rs",
                "depth": 1
            }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(!symbols.is_empty(), "should find at least one symbol");

    // Should find main, add, Point
    let names: Vec<&str> = symbols
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"main"),
        "should find main function, got: {:?}",
        names
    );
    assert!(
        names.contains(&"add"),
        "should find add function, got: {:?}",
        names
    );

    ctx.lsp.shutdown_all().await;
}

#[tokio::test]
async fn symbols_project_wide_uses_workspace_symbol() {
    let Some((_dir, ctx)) = rust_project_ctx().await else {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    };

    // Trigger LSP startup and background indexing via a file-restricted call.
    let _ = Symbols
        .call(json!({ "query": "main", "path": "src/main.rs" }), &ctx)
        .await;

    // Retry project-wide search (no relative_path → workspace/symbol fast path)
    // until rust-analyzer finishes background indexing (typically < 3s).
    let mut found = false;
    for _ in 0..10 {
        let result = Symbols
            .call(json!({ "query": "Point" }), &ctx)
            .await
            .unwrap();
        let symbols = result["symbols"].as_array().unwrap();
        if symbols.iter().any(|s| s["name"].as_str() == Some("Point")) {
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    assert!(
        found,
        "should find 'Point' project-wide via workspace/symbol within 5s"
    );

    ctx.lsp.shutdown_all().await;
}

// ── validate_symbol_range tests ──────────────────────────────────────────

/// Degenerate range (start == end) where tree-sitter confirms multi-line →
/// validate_symbol_range must return Err with "suspicious range".
#[test]
fn validate_symbol_range_rejects_degenerate_range() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    // 3-line function (0-indexed lines 0..2)
    std::fs::write(&file, "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();

    let sym = SymbolInfo {
        name: "add".to_string(),
        name_path: "add".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0, // degenerate — only the fn-name line
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(result.is_err(), "degenerate range should be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("suspicious range"),
        "error should mention suspicious range; got: {msg}"
    );
}

/// Non-degenerate range (start != end) → validate_symbol_range accepts it.
#[test]
fn validate_symbol_range_accepts_good_range() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    std::fs::write(&file, "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();

    // When start != end (LSP returned a real range), accept it.
    let sym = SymbolInfo {
        name: "add".to_string(),
        name_path: "add".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 5, // already a real range
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(result.is_ok(), "good range should be accepted");
}

/// Truncated end_line (end inside body, not at closing `}`) must be rejected.
/// This is the BUG-018 pattern: start != end but end < AST end.
#[test]
fn validate_symbol_range_rejects_truncated_end_line() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    // 3-line function (0-indexed lines 0..2)
    std::fs::write(&file, "fn target() {\n    old_body();\n}\n").unwrap();

    let sym = SymbolInfo {
        name: "target".to_string(),
        name_path: "target".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 1, // truncated — inside body, misses closing `}` at line 2
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(
        result.is_err(),
        "truncated end_line should be rejected; got Ok"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("suspicious range"),
        "error should mention suspicious range; got: {msg}"
    );
}

// ── validate_symbol_range: multi-language coverage ────────────────────────

#[test]
fn validate_symbol_range_rejects_degenerate_python() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.py");
    std::fs::write(
        &file,
        "def add(a, b):\n    result = a + b\n    return result\n",
    )
    .unwrap();

    let sym = SymbolInfo {
        name: "add".to_string(),
        name_path: "add".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0, // degenerate
        start_col: 4,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(
        result.is_err(),
        "Python degenerate range should be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("suspicious range"), "got: {msg}");
}

#[test]
fn validate_symbol_range_rejects_degenerate_typescript() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.ts");
    std::fs::write(
        &file,
        "function add(a: number, b: number): number {\n    const result = a + b;\n    return result;\n}\n",
    )
    .unwrap();

    let sym = SymbolInfo {
        name: "add".to_string(),
        name_path: "add".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0, // degenerate
        start_col: 9,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(
        result.is_err(),
        "TypeScript degenerate range should be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("suspicious range"), "got: {msg}");
}

#[test]
fn validate_symbol_range_rejects_degenerate_go() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.go");
    std::fs::write(
        &file,
        "package main\n\nfunc Add(a int, b int) int {\n\tresult := a + b\n\treturn result\n}\n",
    )
    .unwrap();

    let sym = SymbolInfo {
        name: "Add".to_string(),
        name_path: "Add".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 2, // "func Add..." is line 2 (0-indexed)
        end_line: 2,   // degenerate
        start_col: 5,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(result.is_err(), "Go degenerate range should be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("suspicious range"), "got: {msg}");
}

#[test]
fn validate_symbol_range_rejects_degenerate_rust_with_doc() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    // Doc comment on line 0; fn keyword on line 1.
    std::fs::write(
        &file,
        "/// Adds two numbers.\nfn add(a: i32, b: i32) -> i32 {\n    let r = a + b;\n    r\n}\n",
    )
    .unwrap();

    let sym = SymbolInfo {
        name: "add".to_string(),
        name_path: "add".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 1, // fn keyword, not the doc comment
        end_line: 1,   // degenerate
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(
        result.is_err(),
        "Rust+doc comment degenerate range should be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("suspicious range"), "got: {msg}");
}

#[test]
fn validate_symbol_range_picks_correct_function() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    // `add` at lines 0-2, `multiply` at lines 4-6.
    std::fs::write(
        &file,
        "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n",
    )
    .unwrap();

    let sym = SymbolInfo {
        name: "multiply".to_string(),
        name_path: "multiply".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 4,
        end_line: 4, // degenerate
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(
        result.is_err(),
        "degenerate multiply range should be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("multiply"),
        "error should name the symbol; got: {msg}"
    );
    assert!(msg.contains("suspicious range"), "got: {msg}");
}

#[test]
fn validate_symbol_range_accepts_when_ast_unavailable() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    std::fs::write(&file, "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n").unwrap();

    let sym = SymbolInfo {
        name: "nonexistent_fn".to_string(),
        name_path: "nonexistent_fn".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0, // degenerate, but name not in AST
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    // Name not in file — AST can't confirm anything, so we accept the range
    let result = validate_symbol_range(&sym);
    assert!(
        result.is_ok(),
        "unknown name: range should be accepted (no AST confirmation to the contrary)"
    );
}

#[test]
fn validate_symbol_range_recurses_into_children() {
    use crate::lsp::SymbolKind;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    // `distance` is a method inside `impl Point` — it will be a child symbol.
    std::fs::write(
        &file,
        "struct Point { x: f64, y: f64 }\nimpl Point {\n    fn distance(&self) -> f64 {\n        (self.x * self.x + self.y * self.y).sqrt()\n    }\n}\n",
    )
    .unwrap();

    let sym = SymbolInfo {
        name: "distance".to_string(),
        name_path: "Point/distance".to_string(),
        kind: SymbolKind::Method,
        file: file.clone(),
        start_line: 2, // fn distance line (0-indexed)
        end_line: 2,   // degenerate
        start_col: 7,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let result = validate_symbol_range(&sym);
    assert!(
        result.is_err(),
        "method in impl with degenerate range should be rejected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("suspicious range"), "got: {msg}");
}

#[tokio::test]
async fn symbols_by_name() {
    let Some((_dir, ctx)) = rust_project_ctx().await else {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    };

    let result = Symbols
        .call(
            json!({
                "query": "add",
                "path": "src/main.rs"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(!symbols.is_empty(), "should find 'add' symbol");
    assert!(symbols.iter().any(|s| s["name"].as_str() == Some("add")));

    ctx.lsp.shutdown_all().await;
}

#[tokio::test]
async fn get_symbols_overview_accepts_detail_level() {
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    // Should error because no project, but NOT because of unknown param
    let err = Symbols
        .call(json!({ "path": "x", "detail_level": "full" }), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("project"),
        "should fail on project, not param: {}",
        err
    );
}

#[tokio::test]
async fn path_not_found_is_recoverable_error() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let err = Symbols
        .call(json!({ "path": "nonexistent/file.py" }), &ctx)
        .await
        .unwrap_err();

    assert!(
        err.downcast_ref::<crate::tools::RecoverableError>()
            .is_some(),
        "path not found must be RecoverableError, got: {}",
        err
    );
}

#[tokio::test]
async fn path_not_found_hint_mentions_list_dir() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let err = Symbols
        .call(json!({ "path": "missing.rs" }), &ctx)
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("should be RecoverableError");
    assert!(
        rec.hint().unwrap_or("").contains("tree"),
        "hint should mention tree, got: {:?}",
        rec.hint()
    );
}

#[tokio::test]
async fn glob_no_match_is_recoverable_error() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let err = Symbols
        .call(json!({ "path": "src/**/*.nonexistent" }), &ctx)
        .await
        .unwrap_err();

    assert!(
        err.downcast_ref::<crate::tools::RecoverableError>()
            .is_some(),
        "empty glob must be RecoverableError, got: {}",
        err
    );
}

#[tokio::test]
async fn tools_error_without_project() {
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    assert!(Symbols.call(json!({"path": "x"}), &ctx).await.is_err());
    assert!(Symbols.call(json!({"query": "x"}), &ctx).await.is_err());
    assert!(References
        .call(json!({"symbol": "x", "path": "y"}), &ctx)
        .await
        .is_err());
}

#[test]
fn apply_text_edits_simple_replacement() {
    let content = "hello world\nfoo bar\nbaz\n";
    let edits = vec![lsp_types::TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 6,
            },
            end: lsp_types::Position {
                line: 0,
                character: 11,
            },
        },
        new_text: "rust".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert!(result.starts_with("hello rust"), "got: {}", result);
}

#[cfg(unix)]
#[test]
fn uri_to_path_parses_unix_uri() {
    let p = uri_to_path("file:///home/user/code.rs").unwrap();
    assert_eq!(p, PathBuf::from("/home/user/code.rs"));
}

#[tokio::test]
async fn symbols_project_wide_treesitter_fallback() {
    // No rust-analyzer needed — this test verifies the tree-sitter fallback
    // that kicks in when workspace/symbol returns empty.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn unique_benchmark_fn() -> i32 { 42 }\n\npub struct UniqueTestStruct { x: i32 }\n",
    )
    .unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Project-wide search (no relative_path) — LSP will fail/return empty,
    // so tree-sitter fallback should find the symbol.
    let result = Symbols
        .call(json!({ "query": "unique_benchmark_fn" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "should find symbol via tree-sitter fallback: {:?}",
        result
    );
    assert!(symbols
        .iter()
        .any(|s| s["name"].as_str().unwrap() == "unique_benchmark_fn"));

    // Also check struct is findable
    let result2 = Symbols
        .call(json!({ "query": "UniqueTestStruct" }), &ctx)
        .await
        .unwrap();
    let symbols2 = result2["symbols"].as_array().unwrap();
    assert!(
        symbols2
            .iter()
            .any(|s| s["name"].as_str().unwrap() == "UniqueTestStruct"),
        "should find struct via tree-sitter fallback: {:?}",
        result2
    );
}

#[tokio::test]
async fn get_symbols_overview_finds_nested_files() {
    // No LSP needed — verifies recursive walk + tree-sitter fallback.
    // Source files ONLY in subdirectories (not at root).
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn nested_function() -> i32 { 42 }\n",
    )
    .unwrap();
    // Also one at root for comparison
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Project-wide (no path) — should find both root and nested files
    let result = Symbols.call(json!({}), &ctx).await.unwrap();

    let files = result["files"].as_array().unwrap();
    let file_names: Vec<&str> = files.iter().map(|f| f["file"].as_str().unwrap()).collect();
    assert!(
        files.len() >= 2,
        "should find files in subdirectories, got: {:?}",
        file_names
    );
    assert!(
        file_names.iter().any(|f| f.contains("src/lib.rs")),
        "should find nested src/lib.rs, got: {:?}",
        file_names
    );
    assert!(
        file_names.iter().any(|f| f.contains("main.rs")),
        "should find root main.rs, got: {:?}",
        file_names
    );
}

#[tokio::test]
async fn symbols_overview_small_tree_recurses_fully() {
    // When targeting a specific subdirectory with a small file count (≤ RECURSE_SMALL),
    // the new three-mode dispatch recurses fully to give complete symbol output.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/deep/nested")).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("src/top.rs"), "pub fn top_level() {}\n").unwrap();
    std::fs::write(
        dir.path().join("src/deep/nested/hidden.rs"),
        "pub fn deeply_nested() {}\n",
    )
    .unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // Target "src" specifically — small tree (2 files) → full recursive symbol mode
    let result = Symbols.call(json!({ "path": "src" }), &ctx).await.unwrap();

    let files = result["files"].as_array().unwrap();
    let file_names: Vec<&str> = files.iter().map(|f| f["file"].as_str().unwrap()).collect();
    assert!(
        file_names.iter().any(|f| f.contains("top.rs")),
        "should find src/top.rs, got: {:?}",
        file_names
    );
    // Small tree (≤ RECURSE_SMALL) → full recursive walk includes deeply nested files
    assert!(
        file_names.iter().any(|f| f.contains("hidden.rs")),
        "small tree should recurse fully and find nested file, got: {:?}",
        file_names
    );
}

#[test]
fn symbols_in_tree() {
    let symbols = vec![SymbolInfo {
        name: "Foo".into(),
        name_path: "Foo".into(),
        kind: crate::lsp::SymbolKind::Struct,
        file: PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 5,
        start_col: 0,
        children: vec![SymbolInfo {
            name: "bar".into(),
            name_path: "Foo/bar".into(),
            kind: crate::lsp::SymbolKind::Method,
            file: PathBuf::from("test.rs"),
            start_line: 2,
            end_line: 4,
            start_col: 4,
            children: vec![],
            range_start_line: None,
            detail: None,
        }],
        range_start_line: None,
        detail: None,
    }];

    assert!(find_symbol_by_name_path(&symbols, "Foo").is_some());
    assert!(find_symbol_by_name_path(&symbols, "Foo/bar").is_some());
    assert!(find_symbol_by_name_path(&symbols, "nonexistent").is_none());
}

#[test]
fn find_symbol_by_name_path_exact_match() {
    let test_file = std::env::temp_dir().join("test.rs");
    let symbols = vec![SymbolInfo {
        name: "MyStruct".to_string(),
        name_path: "MyStruct".to_string(),
        kind: crate::lsp::SymbolKind::Struct,
        file: test_file.clone(),
        start_line: 0,
        end_line: 10,
        start_col: 0,
        children: vec![SymbolInfo {
            name: "my_method".to_string(),
            name_path: "MyStruct/my_method".to_string(),
            kind: crate::lsp::SymbolKind::Method,
            file: test_file,
            start_line: 2,
            end_line: 5,
            start_col: 4,
            children: vec![],
            range_start_line: None,
            detail: None,
        }],
        range_start_line: None,
        detail: None,
    }];

    // Exact name_path match for nested symbol
    let found = find_symbol_by_name_path(&symbols, "MyStruct/my_method");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "my_method");

    // Exact name_path match for top-level
    let found = find_symbol_by_name_path(&symbols, "MyStruct");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "MyStruct");

    // Bare name match (fallback)
    let found = find_symbol_by_name_path(&symbols, "my_method");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "my_method");

    // Miss
    let found = find_symbol_by_name_path(&symbols, "nonexistent");
    assert!(found.is_none());
}

#[test]
fn symbol_name_matches_generic_types() {
    let make_sym = |name: &str, name_path: &str| SymbolInfo {
        name: name.to_string(),
        name_path: name_path.to_string(),
        kind: crate::lsp::SymbolKind::Struct,
        file: std::env::temp_dir().join("test.ts"),
        start_line: 0,
        end_line: 10,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let sym = make_sym("IRepository<T, ID>", "IRepository<T, ID>");
    // Exact match
    assert!(symbol_name_matches(&sym, "IRepository<T, ID>"));
    // Generic prefix match
    assert!(symbol_name_matches(&sym, "IRepository"));
    // Partial prefix must NOT match (would be "IRepo" → 's' next, not '<'/'('/' ')
    assert!(!symbol_name_matches(&sym, "IRepo"));

    // Parenthesis suffix (callable generic)
    let sym2 = make_sym("createStore()", "createStore()");
    assert!(symbol_name_matches(&sym2, "createStore"));
    assert!(!symbol_name_matches(&sym2, "create"));

    // Space suffix (e.g. "impl Trait for Struct<T>")
    let sym3 = make_sym("impl Tool for MyStruct<T>", "impl Tool for MyStruct<T>");
    assert!(symbol_name_matches(&sym3, "impl Tool for MyStruct<T>"));

    // Exact name still works with no suffix
    let sym4 = make_sym("PlainStruct", "PlainStruct");
    assert!(symbol_name_matches(&sym4, "PlainStruct"));
    assert!(!symbol_name_matches(&sym4, "Plain"));
}

#[test]
fn find_symbol_by_name_path_generic_types() {
    let test_file = std::env::temp_dir().join("test.ts");
    let symbols = vec![
        SymbolInfo {
            name: "IRepository<T, ID>".to_string(),
            name_path: "IRepository<T, ID>".to_string(),
            kind: crate::lsp::SymbolKind::Interface,
            file: test_file.clone(),
            start_line: 0,
            end_line: 20,
            start_col: 0,
            children: vec![SymbolInfo {
                name: "findById".to_string(),
                name_path: "IRepository<T, ID>/findById".to_string(),
                kind: crate::lsp::SymbolKind::Method,
                file: test_file.clone(),
                start_line: 2,
                end_line: 3,
                start_col: 4,
                children: vec![],
                range_start_line: None,
                detail: None,
            }],
            range_start_line: None,
            detail: None,
        },
        SymbolInfo {
            name: "IRepositoryExtended".to_string(),
            name_path: "IRepositoryExtended".to_string(),
            kind: crate::lsp::SymbolKind::Interface,
            file: test_file,
            start_line: 22,
            end_line: 30,
            start_col: 0,
            children: vec![],
            range_start_line: None,
            detail: None,
        },
    ];

    // Bare query matches the generic type, not the similarly-named one
    let found = find_symbol_by_name_path(&symbols, "IRepository");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "IRepository<T, ID>");

    // "IRepositoryExtended" should NOT match query "IRepository" (different suffix char)
    let found_ext = find_symbol_by_name_path(&symbols, "IRepositoryExtended");
    assert!(found_ext.is_some());
    assert_eq!(found_ext.unwrap().name, "IRepositoryExtended");

    // Child method still reachable through generic parent
    let found_method = find_symbol_by_name_path(&symbols, "findById");
    assert!(found_method.is_some());
    assert_eq!(found_method.unwrap().name, "findById");
}

#[test]
fn find_unique_symbol_by_name_path_errors_on_ambiguous_name() {
    let test_file = std::env::temp_dir().join("test.rs");
    let make_method = |parent: &str, name: &str| SymbolInfo {
        name: name.to_string(),
        name_path: format!("{}/{}", parent, name),
        kind: crate::lsp::SymbolKind::Function,
        file: test_file.clone(),
        start_line: 0,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let symbols = vec![
        SymbolInfo {
            name: "ToolA".to_string(),
            name_path: "ToolA".to_string(),
            kind: crate::lsp::SymbolKind::Struct,
            file: test_file.clone(),
            start_line: 0,
            end_line: 20,
            start_col: 0,
            children: vec![make_method("ToolA", "call")],
            range_start_line: None,
            detail: None,
        },
        SymbolInfo {
            name: "ToolB".to_string(),
            name_path: "ToolB".to_string(),
            kind: crate::lsp::SymbolKind::Struct,
            file: test_file.clone(),
            start_line: 25,
            end_line: 45,
            start_col: 0,
            children: vec![make_method("ToolB", "call")],
            range_start_line: None,
            detail: None,
        },
    ];

    // Baseline (the bug): old find_symbol_by_name_path silently returns the first
    // depth-first match for a bare name — caller has no way to know it was ambiguous.
    let old_result = find_symbol_by_name_path(&symbols, "call");
    assert!(
        old_result.is_some(),
        "old function returns Some for ambiguous name — no error, caller is unaware"
    );
    assert_eq!(
        old_result.unwrap().name_path,
        "ToolA/call",
        "old function returns first depth-first match, silently ignoring ToolB/call"
    );

    // Stale → Fixed: find_unique_symbol_by_name_path detects ambiguity and errors,
    // listing all matching name_paths so the caller can supply a more specific query.
    let result = find_unique_symbol_by_name_path(&symbols, "call");
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("ToolA/call"),
        "expected ToolA/call in error, got: {err_str}"
    );
    assert!(
        err_str.contains("ToolB/call"),
        "expected ToolB/call in error, got: {err_str}"
    );

    // Fresh: supplying the full name_path resolves the ambiguity unambiguously
    let result = find_unique_symbol_by_name_path(&symbols, "ToolA/call");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name_path, "ToolA/call");

    // Not found → RecoverableError mentioning the query
    let result = find_unique_symbol_by_name_path(&symbols, "nonexistent");
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("nonexistent"),
        "expected symbol name in error, got: {err_str}"
    );
}

#[test]
fn replace_symbol_with_ambiguous_name_path_returns_error() {
    // When name_path matches 2+ symbols, find_unique_symbol_by_name_path must
    // return a RecoverableError about ambiguity.
    let test_file = std::env::temp_dir().join("ambig_test.rs");
    let make_method = |parent: &str, name: &str| SymbolInfo {
        name: name.to_string(),
        name_path: format!("{}/{}", parent, name),
        kind: crate::lsp::SymbolKind::Function,
        file: test_file.clone(),
        start_line: 0,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let symbols = vec![
        SymbolInfo {
            name: "ToolA".to_string(),
            name_path: "ToolA".to_string(),
            kind: crate::lsp::SymbolKind::Struct,
            file: test_file.clone(),
            start_line: 0,
            end_line: 20,
            start_col: 0,
            children: vec![make_method("ToolA", "call")],
            range_start_line: None,
            detail: None,
        },
        SymbolInfo {
            name: "ToolB".to_string(),
            name_path: "ToolB".to_string(),
            kind: crate::lsp::SymbolKind::Struct,
            file: test_file.clone(),
            start_line: 25,
            end_line: 45,
            start_col: 0,
            children: vec![make_method("ToolB", "call")],
            range_start_line: None,
            detail: None,
        },
    ];

    // Current behavior: find_unique_symbol_by_name_path returns ambiguity error.
    let result = find_unique_symbol_by_name_path(&symbols, "call");
    assert!(result.is_err(), "expected error for ambiguous name_path");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("ambiguous"),
        "expected 'ambiguous' in error, got: {err_str}"
    );
    assert!(
        err_str.contains("ToolA/call"),
        "expected ToolA/call in error, got: {err_str}"
    );
    assert!(
        err_str.contains("ToolB/call"),
        "expected ToolB/call in error, got: {err_str}"
    );
}

#[tokio::test]
async fn find_referencing_symbols_returns_references() {
    if !std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "test-refs"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // Write a file where `add` is defined and called twice
    std::fs::write(
        dir.path().join("src/main.rs"),
        r#"fn add(a: i32, b: i32) -> i32 {
a + b
}

fn main() {
let x = add(1, 2);
let y = add(3, 4);
println!("{} {}", x, y);
}
"#,
    )
    .unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    // rust-analyzer needs time to load the Cargo project and build its index
    // before textDocument/references returns results. Retry with back-off.
    let mut result_value: Option<Value> = None;
    for attempt in 0..10 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
        }

        let result = References
            .call(
                json!({
                    "symbol": "add",
                    "path": "src/main.rs"
                }),
                &ctx,
            )
            .await;

        // If LSP startup fails (e.g. cargo not in PATH), skip gracefully
        let value = match result {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Skipping: LSP error: {}", e);
                return;
            }
        };

        let total = value["total"].as_u64().unwrap_or(0);
        if total >= 3 {
            result_value = Some(value);
            break;
        }
        eprintln!(
            "Attempt {}: got {} references, retrying...",
            attempt + 1,
            total
        );
    }

    let result = match result_value {
        Some(v) => v,
        None => {
            eprintln!("Skipping: rust-analyzer did not index in time");
            return;
        }
    };

    let refs = result["references"].as_array().unwrap();
    let total = result["total"].as_u64().unwrap();

    // Should find at least 3 references: definition + 2 call sites
    assert!(
        total >= 3,
        "Expected >= 3 references (def + 2 calls), got {}. refs: {:?}",
        total,
        refs
    );

    // All references should be in src/main.rs
    for r in refs {
        let file = r["file"].as_str().unwrap();
        assert!(
            file.contains("main.rs"),
            "Reference in unexpected file: {}",
            file
        );
        // context should contain meaningful text
        let ctx_line = r["context"].as_str().unwrap();
        assert!(!ctx_line.is_empty(), "Context line should not be empty");
    }
}

#[tokio::test]
async fn symbols_schema_includes_scope() {
    let tool = Symbols;
    let schema = tool.input_schema();
    assert!(schema["properties"]["scope"].is_object());
}

#[tokio::test]
async fn get_symbols_overview_schema_includes_scope() {
    let tool = Symbols;
    let schema = tool.input_schema();
    assert!(schema["properties"]["scope"].is_object());
}

#[tokio::test]
async fn find_referencing_symbols_schema_includes_scope() {
    let tool = References;
    let schema = tool.input_schema();
    assert!(schema["properties"]["scope"].is_object());
}

#[tokio::test]
async fn tag_external_path_returns_project_for_internal() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let root = agent.require_project_root().await.unwrap();
    let internal = root.join("src/main.rs");
    let tag = tag_external_path(&internal, &root, &agent).await;
    assert_eq!(tag, "project");
}

#[tokio::test]
async fn tag_external_path_discovers_and_registers() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let root = agent.require_project_root().await.unwrap();

    // Create a fake library directory with Cargo.toml
    let lib_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        lib_dir.path().join("Cargo.toml"),
        "[package]\nname = \"fake_lib\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let lib_src = lib_dir.path().join("src");
    std::fs::create_dir_all(&lib_src).unwrap();
    let lib_file = lib_src.join("lib.rs");
    std::fs::write(&lib_file, "pub fn hello() {}").unwrap();

    let tag = tag_external_path(&lib_file, &root, &agent).await;
    assert_eq!(tag, "lib:fake_lib");

    // Verify it was registered
    let registry = agent.library_registry().await.unwrap();
    assert!(registry.lookup("fake_lib").is_some());
}

#[tokio::test]
async fn symbols_directory_relative_path() {
    let Some((_dir, ctx)) = rust_project_ctx().await else {
        return; // skip if rust-analyzer not available
    };

    // "src" is a directory — should walk it and find symbols inside
    let result = Symbols
        .call(json!({ "query": "add", "path": "src" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with directory relative_path should find symbols"
    );
    assert!(symbols.iter().any(|s| s["name"] == "add"));
}

#[test]
fn collect_matching_matches_name_path() {
    let symbols = vec![SymbolInfo {
        name: "MyStruct".into(),
        name_path: "MyStruct".into(),
        kind: crate::lsp::SymbolKind::Struct,
        file: PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 10,
        start_col: 0,
        children: vec![SymbolInfo {
            name: "my_method".into(),
            name_path: "MyStruct/my_method".into(),
            kind: crate::lsp::SymbolKind::Method,
            file: PathBuf::from("test.rs"),
            start_line: 2,
            end_line: 5,
            start_col: 4,
            children: vec![],
            range_start_line: None,
            detail: None,
        }],
        range_start_line: None,
        detail: None,
    }];

    // Pattern with "/" should match via name_path
    let mut results = vec![];
    collect_matching(
        &symbols,
        &substr_pred("mystruct/my_method"),
        false,
        None,
        0,
        true,
        &mut results,
        None,
    );
    assert!(
        !results.is_empty(),
        "pattern with '/' should match against name_path"
    );
    assert_eq!(results[0]["name"], "my_method");

    // Pattern without "/" should still match via name as before
    let mut results2 = vec![];
    collect_matching(
        &symbols,
        &substr_pred("my_method"),
        false,
        None,
        0,
        true,
        &mut results2,
        None,
    );
    assert!(
        !results2.is_empty(),
        "pattern without '/' should still match via name"
    );
}

async fn rich_project_ctx() -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/utils")).unwrap();
    std::fs::create_dir_all(dir.path().join("src/empty")).unwrap();
    let codescout_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&codescout_dir).unwrap();
    // Opt out of mux so these unit tests use rust-analyzer directly,
    // without needing the codescout-mux binary on PATH.
    std::fs::write(
        codescout_dir.join("project.toml"),
        "[project]\nname = \"test-project\"\n\n[lsp.rust]\nmux = false\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/main.rs"),
        "fn main() {}\n\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn helper() -> bool { true }\n\npub struct Calculator;\n\nimpl Calculator {\n    pub fn compute() -> i32 { 42 }\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/utils/math.rs"),
        "pub fn multiply(a: i32, b: i32) -> i32 { a * b }\n",
    )
    .unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    (
        dir,
        ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: buf(),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        },
    )
}

#[tokio::test]
async fn symbols_path_type_file() {
    let (_dir, ctx) = rich_project_ctx().await;

    let result = Symbols
        .call(json!({ "query": "add", "path": "src/main.rs" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with file relative_path should find symbols"
    );
    assert!(symbols.iter().any(|s| s["name"] == "add"));
}

#[tokio::test]
async fn symbols_path_type_directory() {
    let (_dir, ctx) = rich_project_ctx().await;

    let result = Symbols
        .call(json!({ "query": "helper", "path": "src" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with directory relative_path should find symbols: {:?}",
        result
    );
    assert!(symbols.iter().any(|s| s["name"] == "helper"));
}

#[tokio::test]
async fn symbols_path_type_nested_directory() {
    let (_dir, ctx) = rich_project_ctx().await;

    let result = Symbols
        .call(json!({ "query": "multiply", "path": "src/utils" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with nested directory relative_path should find symbols: {:?}",
        result
    );
    assert!(symbols.iter().any(|s| s["name"] == "multiply"));
}

#[tokio::test]
async fn symbols_path_type_glob() {
    let (_dir, ctx) = rich_project_ctx().await;

    let result = Symbols
        .call(json!({ "query": "add", "path": "src/**/*.rs" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with glob relative_path should find symbols: {:?}",
        result
    );
}

#[tokio::test]
async fn symbols_empty_directory_returns_empty() {
    let (_dir, ctx) = rich_project_ctx().await;

    let result = Symbols
        .call(json!({ "query": "anything", "path": "src/empty" }), &ctx)
        .await
        .unwrap();

    let total = result["total"].as_u64().unwrap();
    assert_eq!(total, 0, "empty directory should return 0 results");
}

#[tokio::test]
async fn symbols_name_path_pattern_in_directory() {
    let (_dir, ctx) = rich_project_ctx().await;

    let result = Symbols
        .call(
            json!({ "query": "impl Calculator/compute", "path": "src" }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with name_path pattern in directory should find symbols: {:?}",
        result
    );
    assert!(symbols.iter().any(|s| s["name"] == "compute"));
}

#[tokio::test]
async fn symbols_name_path_pattern_project_wide() {
    let (_dir, ctx) = rich_project_ctx().await;

    // tree-sitter merges impl methods under the type name directly
    // (no "impl" prefix), so name_path is "Calculator/compute"
    let result = Symbols
        .call(json!({ "query": "Calculator/compute" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(
        !symbols.is_empty(),
        "symbols with name_path pattern project-wide should find symbols via tree-sitter: {:?}",
        result
    );
    assert!(symbols.iter().any(|s| s["name"] == "compute"));
}

#[test]
fn collect_matching_slash_pattern_precision() {
    let symbols = vec![SymbolInfo {
        name: "MyStruct".into(),
        name_path: "MyStruct".into(),
        kind: crate::lsp::SymbolKind::Struct,
        file: PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 10,
        start_col: 0,
        children: vec![SymbolInfo {
            name: "my_method".into(),
            name_path: "MyStruct/my_method".into(),
            kind: crate::lsp::SymbolKind::Method,
            file: PathBuf::from("test.rs"),
            start_line: 2,
            end_line: 5,
            start_col: 4,
            children: vec![],
            range_start_line: None,
            detail: None,
        }],
        range_start_line: None,
        detail: None,
    }];

    let mut results = vec![];
    collect_matching(
        &symbols,
        &substr_pred("mystruct/my_method"),
        false,
        None,
        0,
        true,
        &mut results,
        None,
    );
    assert_eq!(
        results.len(),
        1,
        "slash pattern should match exactly 1 result (the method), not the parent struct"
    );
    assert_eq!(results[0]["name"], "my_method");
}

#[test]
fn matches_kind_filter_function_group() {
    use crate::lsp::SymbolKind;
    assert!(matches_kind_filter(&SymbolKind::Function, "function"));
    assert!(matches_kind_filter(&SymbolKind::Method, "function"));
    assert!(matches_kind_filter(&SymbolKind::Constructor, "function"));
    assert!(!matches_kind_filter(&SymbolKind::Variable, "function"));
    assert!(!matches_kind_filter(&SymbolKind::Class, "function"));
}

#[test]
fn filter_variable_symbols_removes_variables_at_all_levels() {
    let input = json!([
        { "name": "PASS", "kind": "Variable", "start_line": 1, "end_line": 1 },
        {
            "name": "call",
            "kind": "Function",
            "start_line": 5,
            "end_line": 10,
            "children": [
                { "name": "tool", "kind": "Variable", "start_line": 6, "end_line": 6 },
                { "name": "params", "kind": "Variable", "start_line": 6, "end_line": 6 }
            ]
        },
        { "name": "assert_contains", "kind": "Function", "start_line": 12, "end_line": 14 }
    ]);
    let result = filter_variable_symbols(input.as_array().unwrap().to_vec());
    assert_eq!(result.len(), 2, "top-level Variable removed");
    assert_eq!(result[0]["name"], "call");
    assert!(
        !result[0].as_object().unwrap().contains_key("children"),
        "empty children stripped"
    );
    assert_eq!(result[1]["name"], "assert_contains");
}

#[test]
fn filter_variable_symbols_preserves_non_variable_children() {
    let input = json!([
        {
            "name": "outer",
            "kind": "Function",
            "start_line": 1,
            "end_line": 10,
            "children": [
                { "name": "inner", "kind": "Function", "start_line": 3, "end_line": 5 },
                { "name": "local_var", "kind": "Variable", "start_line": 6, "end_line": 6 }
            ]
        }
    ]);
    let result = filter_variable_symbols(input.as_array().unwrap().to_vec());
    assert_eq!(result.len(), 1);
    let children = result[0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "inner");
}

#[test]
fn matches_kind_filter_struct_vs_class() {
    use crate::lsp::SymbolKind;
    assert!(matches_kind_filter(&SymbolKind::Class, "class"));
    assert!(!matches_kind_filter(&SymbolKind::Struct, "class"));
    assert!(matches_kind_filter(&SymbolKind::Struct, "struct"));
    assert!(!matches_kind_filter(&SymbolKind::Class, "struct"));
}

#[test]
fn matches_kind_filter_module_group() {
    use crate::lsp::SymbolKind;
    assert!(matches_kind_filter(&SymbolKind::Module, "module"));
    assert!(matches_kind_filter(&SymbolKind::Namespace, "module"));
    assert!(matches_kind_filter(&SymbolKind::Package, "module"));
    assert!(!matches_kind_filter(&SymbolKind::Function, "module"));
}

#[test]
fn collect_matching_with_kind_filter_class_only() {
    use crate::lsp::SymbolKind;
    let symbols = vec![
        SymbolInfo {
            name: "WeeklyGrid".into(),
            name_path: "WeeklyGrid".into(),
            kind: SymbolKind::Class,
            file: PathBuf::from("test.ts"),
            start_line: 0,
            end_line: 10,
            start_col: 0,
            children: vec![],
            range_start_line: None,
            detail: None,
        },
        SymbolInfo {
            name: "weeklyGrid".into(),
            name_path: "weeklyGrid".into(),
            kind: SymbolKind::Variable,
            file: PathBuf::from("test.ts"),
            start_line: 12,
            end_line: 12,
            start_col: 0,
            children: vec![],
            range_start_line: None,
            detail: None,
        },
        SymbolInfo {
            name: "renderWeeklyGrid".into(),
            name_path: "renderWeeklyGrid".into(),
            kind: SymbolKind::Function,
            file: PathBuf::from("test.ts"),
            start_line: 14,
            end_line: 20,
            start_col: 0,
            children: vec![],
            range_start_line: None,
            detail: None,
        },
    ];

    let mut out = vec![];
    collect_matching(
        &symbols,
        &substr_pred("weeklygrid"),
        false,
        None,
        0,
        true,
        &mut out,
        Some("class"),
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["name"], "WeeklyGrid");
}

#[test]
fn collect_matching_kind_filter_none_returns_all_matching() {
    use crate::lsp::SymbolKind;
    let symbols = vec![
        SymbolInfo {
            name: "foo".into(),
            name_path: "foo".into(),
            kind: SymbolKind::Function,
            file: PathBuf::from("test.rs"),
            start_line: 0,
            end_line: 5,
            start_col: 0,
            children: vec![],
            range_start_line: None,
            detail: None,
        },
        SymbolInfo {
            name: "FOO".into(),
            name_path: "FOO".into(),
            kind: SymbolKind::Constant,
            file: PathBuf::from("test.rs"),
            start_line: 7,
            end_line: 7,
            start_col: 0,
            children: vec![],
            range_start_line: None,
            detail: None,
        },
    ];

    let mut out = vec![];
    collect_matching(
        &symbols,
        &substr_pred("foo"),
        false,
        None,
        0,
        true,
        &mut out,
        None,
    );
    assert_eq!(
        out.len(),
        2,
        "no filter → all name-matching symbols returned"
    );
}

#[test]
fn build_by_file_sorts_desc_and_caps_at_15() {
    // 20 distinct files, file_i has (20 - i) matches
    let mut matches: Vec<Value> = vec![];
    for i in 0usize..20 {
        for _ in 0..(20 - i) {
            matches.push(json!({ "file": format!("src/file{i}.rs") }));
        }
    }
    let (by_file, overflow) = build_by_file(&matches);
    assert_eq!(by_file.len(), 15, "cap at 15");
    assert_eq!(overflow, 5, "20 files - 15 = 5 overflow");
    // First entry has highest count
    assert_eq!(by_file[0].0, "src/file0.rs");
    assert_eq!(by_file[0].1, 20);
    // Sorted descending
    for w in by_file.windows(2) {
        assert!(w[0].1 >= w[1].1);
    }
}

#[test]
fn build_by_file_no_overflow_under_cap() {
    let matches: Vec<Value> = (0..3)
        .flat_map(|i| vec![json!({ "file": format!("src/f{i}.rs") }); 5])
        .collect();
    let (by_file, overflow) = build_by_file(&matches);
    assert_eq!(by_file.len(), 3);
    assert_eq!(overflow, 0);
}

#[test]
fn make_search_symbols_hint_contains_top_file_and_kind_and_offset() {
    let by_file = vec![
        ("src/components/WeeklyGrid.tsx".to_string(), 12usize),
        ("src/screens/Home.tsx".to_string(), 3),
    ];
    let hint = make_search_symbols_hint(50, &by_file);
    assert!(
        hint.contains("src/components/WeeklyGrid.tsx"),
        "should show top file path"
    );
    assert!(hint.contains("kind="), "should mention kind filter");
    assert!(
        hint.contains("offset=50"),
        "should show next pagination offset"
    );
}

#[test]
fn kind_filter_skipped_when_using_name_path() {
    // Verify the logic: if name_path is set, kind_filter is None.
    let input = json!({ "symbol": "Foo", "kind": "function" });
    let is_name_path = input["symbol"].is_string();
    let kind_filter: Option<&str> = if is_name_path {
        None
    } else {
        input["kind"].as_str()
    };
    assert!(kind_filter.is_none());
}

// ── symbol_to_json field contract ────────────────────────────────────────

fn make_test_sym(name: &str, detail: Option<&str>) -> crate::lsp::SymbolInfo {
    crate::lsp::SymbolInfo {
        name: name.to_string(),
        name_path: name.to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/foo.rs"),
        start_line: 0,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: detail.map(|s| s.to_string()),
    }
}

#[test]
fn symbol_to_json_omits_file_when_show_file_false() {
    let sym = make_test_sym("foo", None);
    let result = symbol_to_json(&sym, false, None, 0, false);
    assert!(
        result.get("file").is_none(),
        "file must be absent when show_file=false, got: {result}"
    );
    assert_eq!(result["name"], "foo");
}

#[test]
fn symbol_to_json_field_order_name_kind_before_line_numbers() {
    // Regression: without preserve_order, serde_json used BTreeMap and sorted keys
    // alphabetically, putting end_line before kind/name. Line numbers must come last
    // as positional metadata, with identity fields (name, kind) first.
    let sym = make_test_sym("my_fn", Some("fn my_fn() -> u32"));
    let result = symbol_to_json(&sym, false, None, 0, false);

    let keys: Vec<&str> = result
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();

    // name and name_path come before start_line / end_line
    let name_pos = keys.iter().position(|k| *k == "name").unwrap();
    let start_pos = keys.iter().position(|k| *k == "start_line").unwrap();
    let end_pos = keys.iter().position(|k| *k == "end_line").unwrap();
    assert!(
        name_pos < start_pos,
        "name must appear before start_line, got key order: {keys:?}"
    );
    // start_line comes immediately before end_line
    assert_eq!(
        start_pos + 1,
        end_pos,
        "start_line must immediately precede end_line, got key order: {keys:?}"
    );
    // end_line is the final field
    assert_eq!(
        end_pos,
        keys.len() - 1,
        "end_line must be the last field, got key order: {keys:?}"
    );
}

#[test]
fn symbol_to_json_includes_file_when_show_file_true() {
    let sym = make_test_sym("foo", None);
    let result = symbol_to_json(&sym, false, None, 0, true);
    assert_eq!(result["file"], "src/foo.rs");
}

#[test]
fn symbol_to_json_includes_signature_when_detail_present() {
    let sym = make_test_sym("foo", Some("(x: i32) -> bool"));
    let result = symbol_to_json(&sym, false, None, 0, false);
    assert_eq!(result["signature"], "(x: i32) -> bool");
}

#[test]
fn symbol_to_json_omits_signature_when_detail_absent() {
    let sym = make_test_sym("foo", None);
    let result = symbol_to_json(&sym, false, None, 0, false);
    assert!(
        result.get("signature").is_none(),
        "signature must be absent when detail=None"
    );
}

#[test]
fn symbol_to_json_never_includes_source_field() {
    let sym = make_test_sym("foo", None);
    for show_file in [false, true] {
        let result = symbol_to_json(&sym, false, None, 0, show_file);
        assert!(
            result.get("source").is_none(),
            "source field must never appear (show_file={show_file})"
        );
    }
}

#[test]
fn symbols_overview_flat_cap_triggers_on_symbol_with_many_children() {
    // 20 top-level symbols each with 10 children = 220 flat entries > FLAT_CAP(150).
    // Greedy take: each symbol costs 11 flat entries; 150/11 = 13 symbols fit.
    let symbols: Vec<Value> = (0..20)
        .map(|i| {
            let children: Vec<Value> = (0..10)
                .map(|j| json!({ "name": format!("child_{i}_{j}") }))
                .collect();
            json!({ "name": format!("sym{i}"), "children": children })
        })
        .collect();

    let flat = flat_symbol_count(&symbols);
    assert_eq!(flat, 220); // 20 * (1 + 10)

    // Greedy capping within FLAT_CAP=150
    let budget = LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP;
    let mut remaining = budget;
    let mut capped: Vec<Value> = Vec::new();
    for sym in symbols {
        let cost = 1 + sym["children"].as_array().map(|c| c.len()).unwrap_or(0);
        if cost <= remaining {
            remaining -= cost;
            capped.push(sym);
        } else {
            break;
        }
    }
    // Each symbol costs 11; 13 symbols = 143 flat entries ≤ 150; 14th would be 154.
    assert_eq!(capped.len(), 13);
}

#[test]
fn symbols_overview_flat_cap_not_triggered_for_leaf_heavy_symbols() {
    // 50 top-level leaf symbols (no children) = 50 flat entries — under FLAT_CAP.
    let symbols: Vec<Value> = (0..50)
        .map(|i| json!({ "name": format!("fn{i}") }))
        .collect();
    let flat = flat_symbol_count(&symbols);
    assert_eq!(flat, 50);
    assert!(flat <= LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP);
}

#[test]
fn symbols_overview_single_file_cap_unit() {
    // Unit test: simulate the cap logic on a Vec<Value> of 150 symbol entries.
    use crate::tools::output::OutputGuard;
    let symbols: Vec<Value> = (0..150)
        .map(|i| json!({ "name": format!("sym{i}"), "start_line": i + 1 }))
        .collect();

    const SINGLE_FILE_CAP: usize = 100;
    let total = symbols.len();
    let hint = format!(
        "File has {total} symbols. Use depth=1 for top-level overview, \
         or symbols(name_path='ClassName/methodName', include_body=true) for a specific symbol."
    );
    let g = OutputGuard {
        max_results: SINGLE_FILE_CAP,
        ..OutputGuard::default()
    };
    let (kept, overflow) = g.cap_items(symbols, &hint);

    assert_eq!(kept.len(), 100);
    let ov = overflow.expect("overflow must be present");
    assert_eq!(ov.total, 150);
    assert_eq!(ov.shown, 100);
    assert!(ov.hint.contains("symbols"));
    assert!(ov.hint.contains("symbol"));
    assert!(
        ov.by_file.is_none(),
        "single-file overflow must not include by_file"
    );
}

#[test]
fn symbols_overview_single_file_no_overflow_under_cap_unit() {
    use crate::tools::output::OutputGuard;
    let symbols: Vec<Value> = (0..40)
        .map(|i| json!({ "name": format!("sym{i}") }))
        .collect();

    let g = OutputGuard {
        max_results: 100,
        ..OutputGuard::default()
    };
    let (kept, overflow) = g.cap_items(symbols, "hint");

    assert_eq!(kept.len(), 40);
    assert!(
        overflow.is_none(),
        "no overflow for 40 symbols under cap of 100"
    );
}

#[test]
fn text_sweep_finds_matches_in_comments_and_docs() {
    let dir = tempfile::tempdir().unwrap();

    // Source file with a comment mentioning the old name
    std::fs::write(
        dir.path().join("main.rs"),
        "fn bar() {}\n// FooHandler manages connections\n",
    )
    .unwrap();

    // Documentation file
    std::fs::write(
        dir.path().join("README.md"),
        "# Project\nThe FooHandler struct is the entry point.\nSee FooHandler::new() for details.\n",
    )
    .unwrap();

    // Config file
    std::fs::write(
        dir.path().join("config.toml"),
        "[server]\nhandler = \"FooHandler\"\n",
    )
    .unwrap();

    let lsp_files = std::collections::HashSet::new();
    let matches = text_sweep(dir.path(), "FooHandler", &lsp_files, 20, 2).unwrap();

    // Should find matches in all 3 files
    assert_eq!(matches.len(), 3);

    // Documentation first, then config, then source
    assert_eq!(matches[0].kind, "documentation");
    assert_eq!(matches[1].kind, "config");
    assert_eq!(matches[2].kind, "source");

    // README has 2 occurrences, both shown as previews
    assert_eq!(matches[0].occurrence_count, 2);
    assert_eq!(matches[0].previews.len(), 2);

    // Config has 1 occurrence
    assert_eq!(matches[1].occurrence_count, 1);

    // Source has 1 occurrence (comment line)
    assert_eq!(matches[2].occurrence_count, 1);
}

#[test]
fn text_sweep_skips_lsp_modified_files() {
    let dir = tempfile::tempdir().unwrap();

    let modified_file = dir.path().join("already.rs");
    std::fs::write(&modified_file, "// FooHandler was here\n").unwrap();
    std::fs::write(dir.path().join("untouched.md"), "FooHandler docs\n").unwrap();

    let mut lsp_files = std::collections::HashSet::new();
    lsp_files.insert(modified_file);

    let matches = text_sweep(dir.path(), "FooHandler", &lsp_files, 20, 2).unwrap();

    assert_eq!(matches.len(), 1);
    assert!(matches[0].file.contains("untouched.md"));
}

#[test]
fn text_sweep_respects_max_matches_cap() {
    let dir = tempfile::tempdir().unwrap();

    // Create 30 markdown files, each with one match
    for i in 0..30 {
        std::fs::write(
            dir.path().join(format!("doc{i:02}.md")),
            format!("FooHandler reference in doc {i}\n"),
        )
        .unwrap();
    }

    let lsp_files = std::collections::HashSet::new();
    let matches = text_sweep(dir.path(), "FooHandler", &lsp_files, 20, 2).unwrap();

    assert_eq!(matches.len(), 20);
}

#[test]
fn text_sweep_limits_previews_per_file() {
    let dir = tempfile::tempdir().unwrap();

    // File with 10 occurrences
    let content = (0..10)
        .map(|i| format!("line {i}: FooHandler usage"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.path().join("many.rs"), &content).unwrap();

    let lsp_files = std::collections::HashSet::new();
    let matches = text_sweep(dir.path(), "FooHandler", &lsp_files, 20, 2).unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].occurrence_count, 10);
    assert_eq!(matches[0].previews.len(), 2); // capped at 2
    assert_eq!(matches[0].lines.len(), 10); // all line numbers kept
}

#[test]
fn text_sweep_uses_word_boundary() {
    let dir = tempfile::tempdir().unwrap();

    std::fs::write(
        dir.path().join("test.rs"),
        "let foo_handler = 1;\n// FooHandler docs\nlet FooHandlerConfig = 2;\n",
    )
    .unwrap();

    let lsp_files = std::collections::HashSet::new();
    let matches = text_sweep(dir.path(), "FooHandler", &lsp_files, 20, 2).unwrap();

    assert_eq!(matches.len(), 1);
    // \bFooHandler\b does NOT match inside FooHandlerConfig because
    // there's no word boundary between 'r' and 'C' (both are word chars).
    // So only 1 match: the comment line.
    assert_eq!(matches[0].occurrence_count, 1);
    assert!(matches[0].previews[0].contains("// FooHandler docs"));
}

// ── write_lines / splice edge cases ────────────────────────────────────

#[test]
fn write_lines_no_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let lines: Vec<&str> = vec!["line1", "line2", "line3"];
    write_lines(&file, &lines, false).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "line1\nline2\nline3"
    );
}

#[test]
fn write_lines_with_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let lines: Vec<&str> = vec!["line1", "line2", "line3"];
    write_lines(&file, &lines, true).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "line1\nline2\nline3\n"
    );
}

#[test]
fn write_lines_empty_with_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let lines: Vec<&str> = vec![];
    write_lines(&file, &lines, true).unwrap();
    // Empty content should not become "\n"
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
}

/// Simulates the replace_symbol pattern: lines before + multi-line body + lines after.
/// The body should be split into individual lines before inserting.
#[test]
fn splice_multiline_body_no_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.rs");

    let original = "// before\nfn foo() {\n    old();\n}\n// after\n";
    std::fs::write(&file, original).unwrap();

    let content = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Simulate replace_symbol: replace lines 1-3 (0-indexed) with new body
    let new_body = "fn foo() {\n    new();\n}";
    let start = 1usize;
    let end = 4usize; // exclusive

    let mut new_lines = Vec::new();
    new_lines.extend_from_slice(&lines[..start]);
    // FIX: split body into lines, don't push as single element
    new_lines.extend(new_body.lines());
    new_lines.extend_from_slice(&lines[end..]);

    write_lines(&file, &new_lines, content.ends_with('\n')).unwrap();

    let result = std::fs::read_to_string(&file).unwrap();
    assert_eq!(result, "// before\nfn foo() {\n    new();\n}\n// after\n");
}

/// When body has trailing newline, the extra \n must NOT create a blank line.
#[test]
fn splice_multiline_body_with_trailing_newline_no_blank_line() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.rs");

    let original = "// before\nfn foo() {\n    old();\n}\n// after\n";
    std::fs::write(&file, original).unwrap();

    let content = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // LLM passes body WITH trailing newline (common)
    let new_body = "fn foo() {\n    new();\n}\n";
    let start = 1usize;
    let end = 4usize;

    let mut new_lines = Vec::new();
    new_lines.extend_from_slice(&lines[..start]);
    new_lines.extend(new_body.lines()); // .lines() strips the trailing \n — correct!
    new_lines.extend_from_slice(&lines[end..]);

    write_lines(&file, &new_lines, content.ends_with('\n')).unwrap();

    let result = std::fs::read_to_string(&file).unwrap();
    // Must NOT have blank line between "}" and "// after"
    assert_eq!(result, "// before\nfn foo() {\n    new();\n}\n// after\n");
}

/// Demonstrates the BUG: pushing multi-line body as single element creates extra blank line
/// when body has trailing newline. This test documents the broken behavior.
#[test]
fn splice_push_single_element_creates_blank_line_bug() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.rs");

    let original = "// before\nfn foo() {\n    old();\n}\n// after\n";
    std::fs::write(&file, original).unwrap();

    let content = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    let new_body = "fn foo() {\n    new();\n}\n"; // trailing newline
    let start = 1usize;
    let end = 4usize;

    let mut new_lines = Vec::new();
    new_lines.extend_from_slice(&lines[..start]);
    new_lines.push(new_body); // THE BUG: push as single element
    new_lines.extend_from_slice(&lines[end..]);

    write_lines(&file, &new_lines, content.ends_with('\n')).unwrap();

    let result = std::fs::read_to_string(&file).unwrap();
    // BUG: extra blank line between "}" and "// after"
    assert!(
        result.contains("}\n\n// after"),
        "Expected blank line bug, got: {:?}",
        result
    );
}

#[test]
fn apply_text_edits_preserves_trailing_newline() {
    let content = "hello world\nfoo bar\nbaz\n";
    let edits = vec![lsp_types::TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 1,
                character: 0,
            },
            end: lsp_types::Position {
                line: 1,
                character: 7,
            },
        },
        new_text: "replaced".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "hello world\nreplaced\nbaz\n");
}

#[test]
fn apply_text_edits_multiline_replacement() {
    let content = "aaa\nbbb\nccc\n";
    let edits = vec![lsp_types::TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 1,
                character: 0,
            },
            end: lsp_types::Position {
                line: 1,
                character: 3,
            },
        },
        new_text: "xxx\nyyy".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "aaa\nxxx\nyyy\nccc\n");
}

// ── BUG-002: apply_text_edits uses UTF-16 offsets correctly ─────────────

/// LSP character positions are UTF-16 code units.  A line like
/// `// α foo` has `α` (U+03B1) at byte 3, but at UTF-16 offset 3.
/// `foo` starts at byte 6 but UTF-16 offset 5.
/// The old byte-index code would slice at byte 5, landing mid-codepoint
/// and either panicking or producing garbled text.
#[test]
fn apply_text_edits_utf16_offset() {
    // Line 0: "// α: foo"
    //   byte offsets:  0='/', 1='/', 2=' ', 3..5='α'(2 bytes), 5=':', 6=' ', 7='f', 8='o', 9='o'
    //   UTF-16 offsets: 0='/', 1='/', 2=' ', 3='α'(1 unit),    4=':', 5=' ', 6='f', 7='o', 8='o'
    // Replace "foo" (UTF-16 chars 6..9) with "bar"
    let content = "// \u{03B1}: foo\n";
    let edits = vec![lsp_types::TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 6,
            },
            end: lsp_types::Position {
                line: 0,
                character: 9,
            },
        },
        new_text: "bar".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "// \u{03B1}: bar\n");
}

/// Surrogate pair: emoji (U+1F600) is 4 UTF-8 bytes but 2 UTF-16 code units.
/// Text after the emoji has a higher UTF-16 offset than byte offset.
#[test]
fn apply_text_edits_utf16_surrogate_pair() {
    // Line: "😀 foo"
    //   bytes: 0..3=😀(4 bytes), 4=' ', 5='f', 6='o', 7='o'
    //   UTF-16: 0..1=😀(2 units), 2=' ', 3='f', 4='o', 5='o'
    // Replace "foo" (UTF-16 3..6) with "bar"
    let content = "\u{1F600} foo\n";
    let edits = vec![lsp_types::TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 3,
            },
            end: lsp_types::Position {
                line: 0,
                character: 6,
            },
        },
        new_text: "bar".to_string(),
    }];
    let result = apply_text_edits(content, &edits);
    assert_eq!(result, "\u{1F600} bar\n");
}

// ── find_insert_before_line tests ──────────────────────────────────────

#[test]
fn find_insert_before_line_walks_past_doc_comments() {
    // Blank line between code and docs is NOT consumed (stops at blank line)
    let lines = vec![
        "other code",
        "",
        "/// Doc line 1",
        "/// Doc line 2",
        "fn foo() {}",
    ];
    assert_eq!(find_insert_before_line(&lines, 4), 2);
}

#[test]
fn find_insert_before_line_walks_past_attributes() {
    let lines = vec!["other code", "#[test]", "#[ignore]", "fn foo() {}"];
    assert_eq!(find_insert_before_line(&lines, 3), 1);
}

#[test]
fn find_insert_before_line_stops_at_code() {
    let lines = vec!["let x = 1;", "fn foo() {}"];
    assert_eq!(find_insert_before_line(&lines, 1), 1);
}

#[test]
fn find_insert_before_line_walks_past_kdoc_bare_asterisk_line() {
    // A bare `*` continuation line (KDoc/JSDoc blank doc line) must not stop the walk.
    // Reproduces the root cause of BUG-027: kotlin-language-server reports range.start
    // mid-docstring; the heuristic must walk past bare `*` lines to reach `/**`.
    let lines = vec![
        "other code",             // 0
        "",                       // 1 — blank line: stops the walk
        "    /**",                // 2 — doc opener: expected editing start
        "     * Description.",    // 3
        "     *",                 // 4 — bare asterisk (blank doc continuation)
        "     * @param x ...",    // 5
        "     */",                // 6
        "    fun foo(x: Int) {}", // 7 — symbol_start
    ];
    assert_eq!(find_insert_before_line(&lines, 7), 2);
}

#[test]
fn find_insert_before_line_at_start_of_file() {
    let lines = vec!["/// Doc", "fn foo() {}"];
    assert_eq!(find_insert_before_line(&lines, 1), 0);
}

#[test]
fn editing_start_line_uses_range_start_line_when_present() {
    let sym = crate::lsp::SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 8,
        end_line: 12,
        start_col: 0,
        children: vec![],
        range_start_line: Some(5),
        detail: None,
    };
    let lines = vec![
        "other code",
        "",
        "/// doc1",
        "/// doc2",
        "#[test]",
        "#[ignore]", // line 5 — range_start_line
        "// between",
        "// gap",
        "fn foo() {", // line 8 — start_line (selectionRange)
        "    body",
        "}",
    ];
    // Should use range_start_line (5), NOT heuristic or start_line
    assert_eq!(editing_start_line(&sym, &lines), 5);
}

#[test]
fn editing_start_line_falls_back_to_heuristic_when_none() {
    let sym = crate::lsp::SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 3,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let lines = vec![
        "other code",
        "#[test]",
        "#[ignore]",
        "fn foo() {", // line 3
        "    body",
        "}",
    ];
    // No range_start_line → heuristic walks back past #[test] #[ignore]
    assert_eq!(editing_start_line(&sym, &lines), 1);
}

#[test]
fn editing_start_line_walks_back_to_block_comment_opener_when_lsp_range_is_mid_comment() {
    // Reproduces BUG-027: kotlin-language-server sets range.start at the first @param
    // line inside a KDoc block, not at the `/**` opener. If editing_start_line trusts
    // this blindly, replace_symbol leaves `/**\n * preamble\n *\n` behind — an unclosed
    // block comment that cascades into Kotlin "Unresolved reference" compile errors.
    let sym = crate::lsp::SymbolInfo {
        name: "createSolver".to_string(),
        name_path: "Stage1SolverConfigFactory/createSolver".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("Stage1SolverConfigFactory.kt"),
        start_line: 6, // "fun createSolver("
        end_line: 9,
        start_col: 4,
        children: vec![],
        range_start_line: Some(3), // Kotlin LSP lands here: "* @param lessonCount"
        detail: None,
    };
    let lines = vec![
        "    /**",                                 // 0 ← correct editing start
        "     * Create a configured solver.",      // 1
        "     *",                                  // 2 — bare asterisk
        "     * @param lessonCount Number of ...", // 3 ← range_start_line (Kotlin LSP bug)
        "     * @param moveThreadCount Threads",   // 4
        "     */",                                 // 5
        "    fun createSolver(",                   // 6 ← start_line
        "        lessonCount: Int,",               // 7
        "        moveThreadCount: Int = 4,",       // 8
        "    ): Solver<Stage1Solution> { }",       // 9
    ];
    // Must return 0 (the `/**` opener), not 3 (the Kotlin LSP's wrong range.start)
    assert_eq!(editing_start_line(&sym, &lines), 0);
}

#[test]
fn editing_start_line_does_not_walk_back_from_attribute_even_if_lsp_range_set() {
    // Regression: attributes (#[attr]) must NOT trigger the block-comment walk-back.
    // range_start_line = Some(5) pointing to `#[ignore]` must be used as-is.
    let sym = crate::lsp::SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 8,
        end_line: 12,
        start_col: 0,
        children: vec![],
        range_start_line: Some(5), // `#[ignore]` — NOT inside a block comment
        detail: None,
    };
    let lines = vec![
        "other code", // 0
        "",           // 1
        "/// doc1",   // 2
        "/// doc2",   // 3
        "#[test]",    // 4
        "#[ignore]",  // 5 ← range_start_line — correctly at attribute start
        "// between", // 6
        "// gap",     // 7
        "fn foo() {", // 8 ← start_line
        "    body",   // 9
        "}",          // 10
    ];
    // Must return 5 unchanged — not walk back further into the doc comments
    assert_eq!(editing_start_line(&sym, &lines), 5);
}
/// BUG-031 reproduction: rust-analyzer sets range_start_line to the `pub fn`
/// line, skipping `///` doc comments above. editing_start_line must detect
/// this and walk back to include the doc comments — otherwise replace_symbol
/// leaves the old doc comments orphaned and duplicates them.
#[test]
fn editing_start_line_walks_back_past_doc_comments_when_range_misses_them() {
    let sym = crate::lsp::SymbolInfo {
        name: "is_source_path".to_string(),
        name_path: "is_source_path".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 5, // `pub fn is_source_path(...)` — selectionRange
        end_line: 9,
        start_col: 0,
        children: vec![],
        range_start_line: Some(5), // LSP range.start = fn line, missed doc comments
        detail: None,
    };
    let lines = vec![
        "use regex::Regex;",                                          // 0
        "",                                                           // 1
        "/// Returns true if the path refers to a source code file.", // 2
        "/// Used to gate `edit_file` multi-line source edits.",      // 3
        "#[inline]",                                                  // 4
        "pub fn is_source_path(path: &str) -> bool {",                // 5 ← range_start_line
        "    Regex::new(SOURCE_EXTENSIONS)",                          // 6
        "        .map(|re| re.is_match(path))",                       // 7
        "        .unwrap_or(false)",                                  // 8
        "}",                                                          // 9
    ];
    // Must return 2 (first `///` doc comment), not 5 (range_start_line)
    assert_eq!(editing_start_line(&sym, &lines), 2);
}

/// BUG-031 variant: range_start_line correctly includes doc comments (points
/// to first `///` line). editing_start_line should trust it and NOT walk back
/// further past a blank line into unrelated code.
#[test]
fn editing_start_line_trusts_range_when_it_already_covers_docs() {
    let sym = crate::lsp::SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 5,
        end_line: 7,
        start_col: 0,
        children: vec![],
        range_start_line: Some(3), // Points to first `///` — correct!
        detail: None,
    };
    let lines = vec![
        "fn unrelated() {}", // 0
        "// random comment", // 1
        "",                  // 2 — blank line separates
        "/// Doc for foo",   // 3 ← range_start_line (correct)
        "#[test]",           // 4
        "fn foo() {",        // 5
        "    body",          // 6
        "}",                 // 7
    ];
    // Should stay at 3, not walk back past blank line to 1
    assert_eq!(editing_start_line(&sym, &lines), 3);
}

/// BUG-037 regression: rust-analyzer starts `impl Trait for Type` range at
/// the `impl` keyword, excluding the outer `#[async_trait]` attribute.
/// `editing_start_line` must return the `impl` line unchanged — walking back
/// to include the attribute in the editing range would silently drop it, since
/// the LLM's `new_body` starts at `impl` (matching what `symbols` shows).
#[test]
fn editing_start_line_does_not_walk_back_to_outer_attribute_on_impl_block() {
    let sym = crate::lsp::SymbolInfo {
        name: "impl SomeTrait for SomeType".to_string(),
        name_path: "impl SomeTrait for SomeType".to_string(),
        kind: crate::lsp::SymbolKind::Object,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 2,
        end_line: 6,
        start_col: 0,
        children: vec![],
        range_start_line: Some(2), // rust-analyzer starts at `impl`, not `#[async_trait]`
        detail: None,
    };
    let lines = vec![
        "}",                             // 0 ← end of a previous impl block
        "#[async_trait::async_trait]",   // 1 ← attribute NOT in LSP range
        "impl SomeTrait for SomeType {", // 2 ← range_start_line
        "    async fn foo(&self) {",     // 3
        "    }",                         // 4
        "}",                             // 5
    ];
    // Must return 2 (the `impl` line) — not 1 (the attribute).
    // Walking back to 1 would include `#[async_trait]` in the deletion range
    // while the LLM's new_body starts at `impl`, silently dropping the attribute.
    assert_eq!(editing_start_line(&sym, &lines), 2);
}

/// BUG-037 corollary: when doc comments ARE present above the attribute+impl,
/// walk-back is still triggered (BUG-031 behaviour) because the LLM is expected
/// to include docs in new_body.
#[test]
fn editing_start_line_walks_back_when_docs_exist_above_attribute_on_impl() {
    let sym = crate::lsp::SymbolInfo {
        name: "impl SomeTrait for SomeType".to_string(),
        name_path: "impl SomeTrait for SomeType".to_string(),
        kind: crate::lsp::SymbolKind::Object,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 3,
        end_line: 6,
        start_col: 0,
        children: vec![],
        range_start_line: Some(3), // range starts at `impl`
        detail: None,
    };
    let lines = vec![
        "}",                             // 0 ← end of a previous block
        "/// Implements SomeTrait.",     // 1 ← doc comment above the attribute
        "#[async_trait::async_trait]",   // 2
        "impl SomeTrait for SomeType {", // 3 ← range_start_line
        "    async fn foo(&self) {}",    // 4
        "}",                             // 5
    ];
    // Doc comment at line 1 triggers walk-back — returns 1.
    assert_eq!(editing_start_line(&sym, &lines), 1);
}

/// BUG-029 reproduction: editing_end_line uses AST to cap LSP end_line.
/// For async nested functions inside `mod tests`, AST may return a different
/// end_line, causing insert_code "after" to misplace code.
#[test]
fn editing_end_line_nested_fn_returns_closing_brace_line() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.rs");
    // Reproduce the actual BUG-029 scenario: async fn inside mod tests
    let source = "\
use serde_json::json;

pub async fn write_message(writer: &mut Vec<u8>, msg: &str) -> Result<(), std::io::Error> {
writer.extend_from_slice(msg.as_bytes());
Ok(())
}

#[cfg(test)]
mod tests {
use super::*;

#[tokio::test]
async fn write_produces_valid_framing() {
    let msg = json!({\"test\": true});
    let mut buf = Vec::new();
    write_message(&mut buf, &msg.to_string()).await.unwrap();
    assert!(!buf.is_empty());
}

#[tokio::test]
async fn another_test() {
    let x = 42;
    assert_eq!(x, 42);
}
}
";
    std::fs::write(&file, source).unwrap();

    // Simulate what LSP returns for `write_produces_valid_framing`
    let sym = crate::lsp::SymbolInfo {
        name: "write_produces_valid_framing".to_string(),
        name_path: "tests/write_produces_valid_framing".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: file.clone(),
        start_line: 12, // `async fn write_produces_valid_framing` (0-indexed)
        end_line: 17,   // closing `}` of write_produces_valid_framing
        start_col: 4,
        children: vec![],
        range_start_line: Some(11), // `#[tokio::test]`
        detail: None,
    };

    let end = editing_end_line(&sym);
    // Must return 17 (the `}` line), NOT something smaller
    assert_eq!(
        end, 17,
        "editing_end_line should return closing brace line (17), got {end}"
    );

    // Verify the insertion point is correct
    let lines: Vec<&str> = source.lines().collect();
    let insert_at = (end as usize + 1).min(lines.len());
    assert!(
        insert_at <= lines.len(),
        "insert point should be within file bounds"
    );
    // Line after closing brace should be empty or start of next function
    if insert_at < lines.len() {
        let next_line = lines[insert_at].trim();
        assert!(
            next_line.is_empty()
                || next_line.starts_with('#')
                || next_line.starts_with("async")
                || next_line.starts_with("fn"),
            "line after insert should be blank or next function start, got: '{next_line}'"
        );
    }
}
/// BUG-029 scenario: LSP reports end_line inside the function body (at last
/// statement, not closing `}`). AST should correct this upward.
#[test]
fn editing_end_line_corrects_lsp_short_end_line_via_ast() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("test.rs");
    let source = "\
fn foo() {
let x = 1;
let y = 2;
println!(\"{}\", x + y);
}
";
    std::fs::write(&file, source).unwrap();

    // Simulate LSP returning end_line at the last statement (line 3) instead of `}` (line 4)
    let sym = crate::lsp::SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 3, // WRONG — points to last statement, not `}`
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };

    let end = editing_end_line(&sym);
    // AST should find end_line=4 (the `}`) which is > 3, so it won't cap.
    // Current code only caps when ast_end < sym.end_line.
    // This means a short LSP end_line is NOT corrected upward — this IS the bug.
    // We need editing_end_line to also correct UPWARD when AST shows more.
    assert_eq!(
        end, 4,
        "editing_end_line should correct short LSP end to AST end (4), got {end}"
    );
}

// ── clamp_range_to_parent: T1 unit tests ─────────────────────────────
// These are pure-logic tests (no LSP, no filesystem) that pin down the
// symmetric parent clamp added for BUG-030/034/037/044.

#[test]
fn clamp_range_to_parent_caps_end_at_parent_closer() {
    // Child range overshoots into parent's closer (or beyond).
    // parent body occupies lines 1..20 (exclusive end = 20, the `}` line).
    let (s, e) = clamp_range_to_parent(5, 26, 1, 20);
    assert_eq!((s, e), (5, 20), "end must be capped at parent closer line");
}

#[test]
fn clamp_range_to_parent_lifts_start_to_parent_body_start() {
    // Child range starts above parent body (e.g. stale LSP points at parent's
    // attribute line).
    let (s, e) = clamp_range_to_parent(0, 10, 1, 20);
    assert_eq!((s, e), (1, 10), "start must be lifted to parent body start");
}

#[test]
fn clamp_range_to_parent_passthrough_when_within_bounds() {
    let (s, e) = clamp_range_to_parent(5, 10, 1, 20);
    assert_eq!((s, e), (5, 10), "well-formed ranges must pass through");
}

#[test]
fn clamp_range_to_parent_preserves_start_le_end_invariant_on_collapse() {
    // Pathological input: start > parent body end. Clamp must not produce
    // end < start (would panic on `lines[start..end]`).
    let (s, e) = clamp_range_to_parent(25, 30, 1, 20);
    assert!(s <= e, "start must remain <= end after clamp, got {s}..{e}");
}

#[test]
fn clamp_range_to_parent_exact_fit_is_identity() {
    // Child exactly fills parent body.
    let (s, e) = clamp_range_to_parent(1, 20, 1, 20);
    assert_eq!((s, e), (1, 20));
}

#[test]
fn clamp_range_to_parent_simulates_bug_044_impl_method_overshoot() {
    // BUG-044 repro at the pure-logic layer:
    //   impl LeafOp {        // line 0
    //       fn parse(...)    // lines 1–9
    //       fn sql(...)      // lines 10–18
    //   }                    // line 19 (parent closer)
    //
    // Suppose LSP reports `parse` with range end_line = 18 (overshooting
    // into `sql`). Without the clamp the replacement eats `sql`. With the
    // clamp we stop at the method's own `}` — but that requires an AST
    // correction too. The clamp's role here is to make sure we never
    // *exceed* the parent closer even if AST also misfires.
    //
    // Simulating the worst case: end overshoots past the parent closer.
    let parent_body_start = 1;
    let parent_body_end_exclusive = 19; // `}` of impl LeafOp
    let (s, e) = clamp_range_to_parent(1, 22, parent_body_start, parent_body_end_exclusive);
    assert_eq!(
        e, 19,
        "must not extend past parent closer even under extreme overshoot"
    );
    assert_eq!(s, 1);
}

/// BUG-030 reproduction: replace_symbol on `mod tests` eats the preceding
/// function. editing_start_line with range_start_line pointing to `#[cfg(test)]`
/// should NOT walk back past the blank line into `write_message`'s closing `}`.
#[test]
fn editing_start_line_mod_tests_does_not_eat_preceding_function() {
    let sym = crate::lsp::SymbolInfo {
        name: "tests".to_string(),
        name_path: "tests".to_string(),
        kind: crate::lsp::SymbolKind::Module,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 7, // `mod tests {`
        end_line: 15,
        start_col: 0,
        children: vec![],
        range_start_line: Some(6), // `#[cfg(test)]`
        detail: None,
    };
    let lines = vec![
        "pub async fn write_message() -> Result<()> {", // 0
        "    let body = serde_json::to_string(msg)?;",  // 1
        "    let header = format!(\"Content-Length: {}\\r\\n\\r\\n\", body.len());", // 2
        "    writer.write_all(header.as_bytes()).await?;", // 3
        "}",                                            // 4
        "",                                             // 5 — blank line
        "#[cfg(test)]",                                 // 6 ← range_start_line
        "mod tests {",                                  // 7 ← start_line
        "    use super::*;",                            // 8
        "    #[test]",                                  // 9
        "    fn test_foo() {}",                         // 10
        "}",                                            // 11
    ];
    // Must return 6 (#[cfg(test)]), NOT walk back past blank line to 4 or earlier
    let result = editing_start_line(&sym, &lines);
    assert_eq!(
        result, 6,
        "editing_start_line should stop at #[cfg(test)] (6), got {result}"
    );
}
/// BUG-030 variant: range_start_line is None (workspace/symbol or tree-sitter).
/// find_insert_before_line must stop at the blank line and not consume
/// the preceding function's closing `}`.
#[test]
fn editing_start_line_mod_tests_no_range_stops_at_blank_line() {
    let sym = crate::lsp::SymbolInfo {
        name: "tests".to_string(),
        name_path: "tests".to_string(),
        kind: crate::lsp::SymbolKind::Module,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 5, // `mod tests {` (0-indexed)
        end_line: 8,
        start_col: 0,
        children: vec![],
        range_start_line: None, // No range info
        detail: None,
    };
    let lines = vec![
        "pub async fn write_message() -> Result<()> {", // 0
        "    let body = \"hello\";",                    // 1
        "}",                                            // 2
        "",                                             // 3 — blank line
        "#[cfg(test)]",                                 // 4
        "mod tests {",                                  // 5 ← start_line
        "    #[test]",                                  // 6
        "    fn test_foo() {}",                         // 7
        "}",                                            // 8
    ];
    // Heuristic walks back from line 5 past #[cfg(test)] to line 4,
    // stops at blank line 3. Must return 4, NOT 2 or earlier.
    let result = editing_start_line(&sym, &lines);
    assert_eq!(result, 4, "should stop at #[cfg(test)] (4), got {result}");
}

/// BUG-030 variant: NO blank line between preceding function and #[cfg(test)].
/// This is the dangerous case — find_insert_before_line might walk into the
/// preceding function's closing `}`.
#[test]
fn editing_start_line_mod_tests_no_blank_line_between_functions() {
    let sym = crate::lsp::SymbolInfo {
        name: "tests".to_string(),
        name_path: "tests".to_string(),
        kind: crate::lsp::SymbolKind::Module,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 4, // `mod tests {`
        end_line: 8,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let lines = vec![
        "pub fn write_message() -> Result<()> {", // 0
        "    let body = \"hello\";",              // 1
        "}",                                      // 2
        "#[cfg(test)]",                           // 3 — NO blank line before this
        "mod tests {",                            // 4 ← start_line
        "    #[test]",                            // 5
        "    fn test_foo() {}",                   // 6
        "}",                                      // 7
    ];
    // Walk back from 4 past #[cfg(test)] to 3. Line 2 is `}` — code, must stop.
    let result = editing_start_line(&sym, &lines);
    assert_eq!(
        result, 3,
        "should stop at #[cfg(test)] (3), not eat into write_message; got {result}"
    );
}
/// BUG-032: validate_symbol_position detects stale LSP positions.
/// After removing lines from a file, LSP may return positions from the
/// pre-removal state. The validation catches this mismatch.
#[test]
fn validate_symbol_position_detects_stale_positions() {
    // Original file: enum SourceFilter at lines 0-4, impl SourceFilter at lines 6-11
    let original_lines = vec![
        "pub enum SourceFilter {",                                   // 0
        "    All,",                                                  // 1
        "    SourceOnly,",                                           // 2
        "    NonSourceOnly,",                                        // 3
        "}",                                                         // 4
        "",                                                          // 5
        "impl SourceFilter {",                                       // 6
        "    pub fn as_sql_filter(&self) -> Option<&'static str> {", // 7
        "        None",                                              // 8
        "    }",                                                     // 9
        "}",                                                         // 10
        "",                                                          // 11
        "pub fn project_db_path() {}",                               // 12
    ];

    // Symbol for impl SourceFilter — correct in original file
    let sym_impl = crate::lsp::SymbolInfo {
        name: "SourceFilter".to_string(),
        name_path: "impl SourceFilter".to_string(),
        kind: crate::lsp::SymbolKind::Struct,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 6, // `impl SourceFilter {` in ORIGINAL file
        end_line: 10,
        start_col: 0,
        children: vec![],
        range_start_line: Some(6),
        detail: None,
    };

    // Validates fine against original file
    assert!(
        validate_symbol_position(&sym_impl, &original_lines).is_ok(),
        "should be valid against original file"
    );

    // Now simulate removing enum (lines 0-5): file shifts up by 6 lines
    let after_removal = vec![
        "impl SourceFilter {",                                       // 0 (was 6)
        "    pub fn as_sql_filter(&self) -> Option<&'static str> {", // 1
        "        None",                                              // 2
        "    }",                                                     // 3
        "}",                                                         // 4
        "",                                                          // 5
        "pub fn project_db_path() {}",                               // 6 (was 12)
    ];

    // LSP still reports start_line=6 (stale) — but line 6 is now project_db_path
    let result = validate_symbol_position(&sym_impl, &after_removal);
    assert!(
        result.is_err(),
        "should detect stale position: 'SourceFilter' not at line 6 in modified file"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("stale"),
        "error should mention stale; got: {msg}"
    );
}
/// BUG-036: validate_symbol_position catches stale start_line inside preceding function.
/// insert_code before: mod tests can land inside the preceding function when the LSP
/// returns a start_line that points inside that function's body (stale after a large
/// insertion above). The old check (name anywhere in [range_start..end_line]) missed
/// this because the name still appeared at the true declaration line later in the window.
/// The tighter [start_line..start_line+3] window catches it.
#[test]
fn validate_symbol_position_catches_start_line_inside_preceding_function() {
    let lines = vec![
        "pub fn read(&self) -> Result<Summary> {", // 0
        "    let data = self.load()?;",            // 1
        "    Ok(Summary { data })",                // 2
        "}",                                       // 3
        "",                                        // 4
        "#[cfg(test)]",                            // 5
        "mod tests {",                             // 6
        "    use super::*;",                       // 7
        "    #[test]",                             // 8
        "    fn test_read() {}",                   // 9
        "}",                                       // 10
    ];

    // Correct position: start_line=6, range_start_line=5
    let sym_correct = crate::lsp::SymbolInfo {
        name: "tests".to_string(),
        name_path: "tests".to_string(),
        kind: crate::lsp::SymbolKind::Module,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 6,
        end_line: 10,
        start_col: 0,
        children: vec![],
        range_start_line: Some(5),
        detail: None,
    };
    assert!(
        validate_symbol_position(&sym_correct, &lines).is_ok(),
        "correct position should validate"
    );

    // Stale position: start_line=2 (inside preceding function body).
    // Old check: "tests" appears at line 6 which is within [min(5,2)..11] → passes WRONGLY.
    // New check: [2..5] does not contain "tests" → correctly detected as stale.
    let sym_stale = crate::lsp::SymbolInfo {
        name: "tests".to_string(),
        name_path: "tests".to_string(),
        kind: crate::lsp::SymbolKind::Module,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 2, // stale — points inside `read` method body
        end_line: 10,
        start_col: 0,
        children: vec![],
        range_start_line: Some(2),
        detail: None,
    };
    let result = validate_symbol_position(&sym_stale, &lines);
    assert!(
        result.is_err(),
        "stale start_line inside preceding function should be detected"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("stale"),
        "error should mention stale; got: {msg}"
    );
}
/// Lead-in case: LSP returns start_line at a closing `})` of preceding macro,
/// real symbol is 3 lines below. Must accept (matches existing
/// `replace_symbol_trusts_lsp_start_with_paren_close` expectation).
#[test]
fn validate_symbol_position_accepts_lead_in_paren_close() {
    let lines = vec![
        "        })",          // 0 — start_line (lead-in: closing paren of preceding macro)
        "    }",               // 1 — closing brace of preceding method
        "",                    // 2 — blank line
        "    fn target() {",   // 3 — actual declaration
        "        old_body();", // 4
        "    }",               // 5
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "target".to_string(),
        name_path: "target".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    assert!(
        validate_symbol_position(&sym, &lines).is_ok(),
        "lead-in `}})` at start_line should be accepted"
    );
}

/// Lead-in case: start_line on a blank line, name a few lines below.
#[test]
fn validate_symbol_position_accepts_lead_in_blank_line() {
    let lines = vec![
        "",              // 0 — start_line (blank)
        "fn target() {", // 1
        "    body();",   // 2
        "}",             // 3
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "target".to_string(),
        name_path: "target".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 3,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    assert!(validate_symbol_position(&sym, &lines).is_ok());
}

/// Lead-in case: start_line on a `#[cfg(test)]` attribute, name below.
#[test]
fn validate_symbol_position_accepts_lead_in_rust_attribute() {
    let lines = vec![
        "#[cfg(test)]", // 0 — start_line (attribute)
        "mod tests {",  // 1
        "}",            // 2
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "tests".to_string(),
        name_path: "tests".to_string(),
        kind: crate::lsp::SymbolKind::Module,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 2,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    assert!(validate_symbol_position(&sym, &lines).is_ok());
}

/// Lead-in case: start_line on a Python `@decorator`, name on def line below.
#[test]
fn validate_symbol_position_accepts_lead_in_python_decorator() {
    let lines = vec![
        "@decorator",     // 0 — start_line (decorator)
        "def my_func():", // 1
        "    pass",       // 2
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "my_func".to_string(),
        name_path: "my_func".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.py"),
        start_line: 0,
        end_line: 2,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    assert!(validate_symbol_position(&sym, &lines).is_ok());
}

/// Lead-in case: KDoc continuation line (Kotlin LSP BUG-027 quirk where
/// range.start lands on a `* @param` line inside a `/** */` block).
/// `start_line` could land on a `*` continuation line — name on the actual
/// `fun` declaration a few lines below.
#[test]
fn validate_symbol_position_accepts_lead_in_kdoc_continuation() {
    let lines = vec![
        "/**",                   // 0
        " * @param x the param", // 1 — start_line (KDoc continuation)
        " */",                   // 2
        "fun target() {}",       // 3
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "target".to_string(),
        name_path: "target".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.kt"),
        start_line: 1,
        end_line: 3,
        start_col: 0,
        children: vec![],
        range_start_line: Some(1),
        detail: None,
    };
    assert!(validate_symbol_position(&sym, &lines).is_ok());
}

/// BUG-036 variant: lead-in claim but name not within window — still stale.
/// start_line on a blank line, but the actual symbol is 10+ lines below
/// (way beyond the lead-in window).
#[test]
fn validate_symbol_position_catches_lead_in_with_distant_name() {
    let lines = vec![
        "",                     // 0 — start_line (lead-in)
        "fn unrelated_one() {", // 1
        "    do_thing();",      // 2
        "}",                    // 3
        "",                     // 4
        "fn unrelated_two() {", // 5
        "    do_other();",      // 6
        "}",                    // 7
        "",                     // 8
        "fn target() {}",       // 9 — too far for the 6-line window
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "target".to_string(),
        name_path: "target".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 9,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let result = validate_symbol_position(&sym, &lines);
    assert!(
        result.is_err(),
        "name 9 lines below lead-in should be detected as stale"
    );
    assert!(result.unwrap_err().to_string().contains("stale"));
}

/// Multi-line Rust signature: name on start_line, args wrapped below.
#[test]
fn validate_symbol_position_accepts_multiline_signature() {
    let lines = vec![
        "pub fn long_name(", // 0 — start_line, name here
        "    arg1: T,",      // 1
        "    arg2: U,",      // 2
        ") -> R {",          // 3
        "    body()",        // 4
        "}",                 // 5
    ];
    let sym = crate::lsp::SymbolInfo {
        name: "long_name".to_string(),
        name_path: "long_name".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 0,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    assert!(validate_symbol_position(&sym, &lines).is_ok());
}

/// `is_lead_in_line` unit cases — boundary behaviour.
#[test]
fn is_lead_in_line_classification() {
    // True (lead-in)
    assert!(is_lead_in_line(""));
    assert!(is_lead_in_line("    "));
    assert!(is_lead_in_line("}"));
    assert!(is_lead_in_line("    }"));
    assert!(is_lead_in_line("})"));
    assert!(is_lead_in_line("        })"));
    assert!(is_lead_in_line("});"));
    assert!(is_lead_in_line("})?;"));
    assert!(is_lead_in_line("},"));
    assert!(is_lead_in_line("// a comment"));
    assert!(is_lead_in_line("/// doc comment"));
    assert!(is_lead_in_line("/* block */"));
    assert!(is_lead_in_line(" * KDoc continuation"));
    assert!(is_lead_in_line("*/"));
    assert!(is_lead_in_line("@decorator"));
    assert!(is_lead_in_line("@Override"));
    assert!(is_lead_in_line("#[cfg(test)]"));
    assert!(is_lead_in_line("#![allow(unused)]"));

    // False (real code)
    assert!(!is_lead_in_line("fn foo() {"));
    assert!(!is_lead_in_line("    let x = 1;"));
    assert!(!is_lead_in_line("class Foo {"));
    assert!(!is_lead_in_line("def bar():"));
    assert!(!is_lead_in_line("    return value"));
    assert!(!is_lead_in_line("pub mod tests;"));
}

/// validate_symbol_position accepts valid positions within ±2 line window.
#[test]
fn validate_symbol_position_accepts_valid_position() {
    let lines = vec!["/// doc comment", "pub fn my_function() {", "    body", "}"];
    let sym = crate::lsp::SymbolInfo {
        name: "my_function".to_string(),
        name_path: "my_function".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 1,
        end_line: 3,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    assert!(validate_symbol_position(&sym, &lines).is_ok());
}

#[test]
fn editing_start_line_discards_walkback_when_no_block_comment_opener() {
    // Validate the safety net: if range_start_line points to a `*`-prefixed line
    // that is NOT inside a /** */ block (e.g. a Rust dereference or raw pointer),
    // the walk-back should be discarded and the original range_start_line returned.
    let sym = crate::lsp::SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("test.rs"),
        start_line: 3,
        end_line: 5,
        start_col: 0,
        children: vec![],
        range_start_line: Some(2), // points to `*mut u8` — NOT a doc comment
        detail: None,
    };
    let lines = vec![
        "use std::ptr;", // 0
        "",              // 1 — blank line stops heuristic walk-back
        "*mut u8",       // 2 ← range_start_line (hypothetical: `*`-prefixed non-comment)
        "fn foo() {",    // 3 ← start_line
        "    body",      // 4
        "}",             // 5
    ];
    // Walk-back reaches line 2, doesn't find /** or /*, so discards and returns 2
    assert_eq!(editing_start_line(&sym, &lines), 2);
}

// ── symbol_to_json body extraction: full-range (includes attributes) ─────

#[test]
fn symbol_to_json_body_includes_attributes_when_range_start_line_set() {
    let source = "#[test]\n/// A doc comment\nfn foo() {\n    body();\n}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 2, // fn keyword (0-indexed)
        end_line: 4,   // closing }
        start_col: 0,
        children: vec![],
        range_start_line: Some(0), // #[test] line
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    let body = json["body"].as_str().unwrap();
    assert!(
        body.contains("#[test]"),
        "body should include #[test] attribute; got:\n{body}"
    );
    assert!(
        body.contains("/// A doc comment"),
        "body should include doc comment; got:\n{body}"
    );
    assert!(
        body.contains("fn foo()"),
        "body should include fn declaration; got:\n{body}"
    );
}

#[test]
fn symbol_to_json_includes_body_start_line() {
    let source = "#[test]\nfn foo() {}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 1,
        end_line: 1,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    // body_start_line should be 1 (1-indexed, the #[test] line)
    assert_eq!(
        json["body_start_line"].as_u64(),
        Some(1),
        "body_start_line should be 1-indexed line where body begins (the attribute line)"
    );
}

#[test]
fn symbol_to_json_body_uses_heuristic_when_range_start_line_none() {
    let source = "#[test]\nfn foo() {\n    body();\n}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 1, // fn keyword
        end_line: 3,
        start_col: 0,
        children: vec![],
        range_start_line: None, // tree-sitter / workspace/symbol path
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    let body = json["body"].as_str().unwrap();
    assert!(
        body.contains("#[test]"),
        "body should include #[test] via heuristic fallback; got:\n{body}"
    );
}

#[test]
fn symbol_to_json_body_start_line_equals_start_line_when_no_attributes() {
    let source = "fn foo() {\n    body();\n}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 0,
        end_line: 2,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0), // same as start_line — no attributes
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    assert_eq!(
        json["body_start_line"].as_u64(),
        Some(1),
        "body_start_line should equal start_line when no attributes"
    );
    assert_eq!(
        json["start_line"].as_u64(),
        Some(1),
        "start_line should be 1 (1-indexed)"
    );
}

#[test]
fn symbol_to_json_no_body_start_line_when_include_body_false() {
    let source = "#[test]\nfn foo() {}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 1,
        end_line: 1,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0),
        detail: None,
    };
    let json = symbol_to_json(&sym, false, Some(source), 0, false);
    assert!(
        json.get("body").is_none(),
        "body should not be present when include_body=false"
    );
    assert!(
        json.get("body_start_line").is_none(),
        "body_start_line should not be present when include_body=false"
    );
}

#[test]
fn symbol_to_json_body_includes_only_doc_comments() {
    // Symbol with only doc comments (no attributes)
    let source = "/// Doc line 1\n/// Doc line 2\nfn foo() {}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 2, // fn keyword
        end_line: 2,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0), // includes doc comments
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    let body = json["body"].as_str().unwrap();
    assert!(
        body.contains("/// Doc line 1"),
        "body should include first doc line; got:\n{body}"
    );
    assert!(
        body.contains("/// Doc line 2"),
        "body should include second doc line; got:\n{body}"
    );
    assert!(
        body.contains("fn foo()"),
        "body should include fn declaration; got:\n{body}"
    );
    assert_eq!(json["body_start_line"].as_u64(), Some(1));
    assert_eq!(json["start_line"].as_u64(), Some(3)); // fn keyword is line 3 (1-indexed)
}

#[test]
fn symbol_to_json_body_includes_multiline_attribute() {
    let source = "#[cfg(\n    target_os = \"linux\"\n)]\nfn foo() {}\n";
    let sym = crate::lsp::SymbolInfo {
        name: "foo".into(),
        name_path: "foo".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 3, // fn keyword
        end_line: 3,
        start_col: 0,
        children: vec![],
        range_start_line: Some(0), // includes #[cfg(
        detail: None,
    };
    let json = symbol_to_json(&sym, true, Some(source), 0, false);
    let body = json["body"].as_str().unwrap();
    assert!(
        body.contains("#[cfg("),
        "body should include multiline attribute opener; got:\n{body}"
    );
    assert!(
        body.contains("target_os"),
        "body should include attribute content; got:\n{body}"
    );
    assert!(
        body.contains(")]"),
        "body should include attribute closer; got:\n{body}"
    );
    assert_eq!(json["body_start_line"].as_u64(), Some(1));
}

#[test]
fn symbol_to_json_child_body_also_uses_full_range() {
    // Parent with a child that has its own attributes
    let source = "impl Foo {\n    #[test]\n    fn bar() {}\n}\n";
    let child = crate::lsp::SymbolInfo {
        name: "bar".into(),
        name_path: "Foo/bar".into(),
        kind: crate::lsp::SymbolKind::Function,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 2, // fn bar
        end_line: 2,
        start_col: 0,
        children: vec![],
        range_start_line: Some(1), // #[test]
        detail: None,
    };
    let parent = crate::lsp::SymbolInfo {
        name: "Foo".into(),
        name_path: "Foo".into(),
        kind: crate::lsp::SymbolKind::Struct,
        file: std::path::PathBuf::from("src/lib.rs"),
        start_line: 0,
        end_line: 3,
        start_col: 0,
        children: vec![child],
        range_start_line: Some(0),
        detail: None,
    };
    // depth=1 to include children
    let json = symbol_to_json(&parent, true, Some(source), 1, false);
    let child_body = json["children"][0]["body"].as_str().unwrap();
    assert!(
        child_body.contains("#[test]"),
        "child body should include its attribute; got:\n{child_body}"
    );
    assert!(
        child_body.contains("fn bar()"),
        "child body should include fn declaration; got:\n{child_body}"
    );
}

#[test]
fn find_insert_before_line_walks_past_multiline_attribute() {
    // #[cfg(
    //     target_os = "linux"
    // )]
    // fn foo() {}
    let lines = vec![
        "other code",
        "#[cfg(",
        "    target_os = \"linux\"",
        ")]",
        "fn foo() {}",
    ];
    assert_eq!(find_insert_before_line(&lines, 4), 1);
}

#[test]
fn find_insert_before_line_walks_past_nested_multiline_attributes() {
    // #[cfg(all(
    //     target_os = "linux",
    //     feature = "nightly"
    // ))]
    // #[inline]
    // fn foo() {}
    let lines = vec![
        "other code",
        "#[cfg(all(",
        "    target_os = \"linux\",",
        "    feature = \"nightly\"",
        "))]",
        "#[inline]",
        "fn foo() {}",
    ];
    assert_eq!(find_insert_before_line(&lines, 6), 1);
}

#[test]
fn find_insert_before_line_walks_past_python_multiline_decorator() {
    // @app.route(
    //     "/api/v1/users",
    //     methods=["GET"]
    // )
    // def get_users():
    let lines = vec![
        "other code",
        "@app.route(",
        "    \"/api/v1/users\",",
        "    methods=[\"GET\"]",
        ")",
        "def get_users():",
    ];
    // The `)` on line 4 is recognized as a bracket closer, triggering
    // upward scanning through the multi-line decorator.
    assert_eq!(find_insert_before_line(&lines, 5), 1);
}

#[test]
fn find_references_format_compact_shows_count() {
    use serde_json::json;
    let tool = References;
    let result = json!({ "references": [{"file":"a.rs","line":10}], "total": 1 });
    let text = tool.format_compact(&result).unwrap();
    assert!(text.contains("1 ref"), "got: {text}");
}

#[test]
fn rename_symbol_format_compact_shows_sites() {
    use serde_json::json;
    let tool = EditCode;
    let result = json!({ "total_edits": 5, "textual_match_count": 1, "files_changed": 2, "new_name": "bar" });
    let text = tool.format_compact(&result).unwrap();
    assert!(text.contains("bar"), "got: {text}");
}

#[test]
fn insert_code_format_compact_shows_location() {
    use serde_json::json;
    let tool = EditCode;
    let result = json!({ "status": "ok", "inserted_at_line": 42, "position": "after" });
    let text = tool.format_compact(&result).unwrap();
    assert!(text.contains("42"), "got: {text}");
}

#[test]
fn replace_symbol_format_compact_shows_range() {
    let tool = EditCode;
    let r = json!({ "status": "ok", "replaced_lines": "124-145" });
    let t = tool.format_compact(&r).unwrap();
    assert!(t.contains("L124"), "got: {t}");
}

#[test]
fn remove_symbol_format_compact_shows_range() {
    let tool = EditCode;
    let r = json!({ "status": "ok", "removed_lines": "201-215", "line_count": 14 });
    let t = tool.format_compact(&r).unwrap();
    assert!(t.contains("201"), "got: {t}");
    assert!(t.contains("14"), "got: {t}");
}

#[test]
fn symbol_at_requires_lsp() {
    let off = crate::tools::ToolCapabilities {
        has_lsp: false,
        has_embeddings: false,
        has_git_remote: false,
        has_libraries: false,
    };
    let on = crate::tools::ToolCapabilities {
        has_lsp: true,
        ..off
    };
    let t = SymbolAt;
    assert!(!t.availability(&off).is_available(&off));
    assert!(t.availability(&on).is_available(&on));
}

// --- format_goto_definition tests ---

#[test]
fn goto_single_project_definition() {
    let val = serde_json::json!({
        "definitions": [{
            "file": "src/tools/output.rs",
            "line": 35,
            "end_line": 41,
            "context": "pub struct OutputGuard {",
            "source": "project"
        }],
        "from": "symbol.rs:120"
    });
    let result = format_goto_definition(&val);
    assert_eq!(
        result,
        "src/tools/output.rs:35\n\n  pub struct OutputGuard {"
    );
}

#[test]
fn goto_single_external_definition() {
    let val = serde_json::json!({
        "definitions": [{
            "file": "/home/user/.rustup/toolchains/stable/lib.rs",
            "line": 100,
            "end_line": 110,
            "context": "pub enum Option<T> {",
            "source": "external"
        }],
        "from": "main.rs:5"
    });
    let result = format_goto_definition(&val);
    assert!(result.contains("(external)"));
    assert!(result.contains(":100"));
    assert!(result.contains("pub enum Option<T> {"));
}

#[test]
fn goto_multiple_definitions() {
    let val = serde_json::json!({
        "definitions": [
            { "file": "src/a.rs", "line": 10, "end_line": 15, "context": "fn foo()", "source": "project" },
            { "file": "src/b.rs", "line": 20, "end_line": 25, "context": "fn foo()", "source": "project" }
        ],
        "from": "main.rs:1"
    });
    let result = format_goto_definition(&val);
    assert!(result.starts_with("2 definitions"));
    assert!(result.contains("src/a.rs:10"));
    assert!(result.contains("src/b.rs:20"));
}

#[test]
fn goto_empty_definitions() {
    let val = serde_json::json!({ "definitions": [] });
    assert_eq!(format_goto_definition(&val), "");
}

#[test]
fn goto_empty_definitions_with_hint() {
    let val = serde_json::json!({
        "definitions": [],
        "from": "main.rs:42",
        "hint": "no definition resolvable at this position",
    });
    let out = format_goto_definition(&val);
    assert_eq!(out, "no definition resolvable at this position");
}

#[test]
fn goto_no_context() {
    let val = serde_json::json!({
        "definitions": [{
            "file": "src/lib.rs",
            "line": 1,
            "end_line": 1,
            "context": "",
            "source": "project"
        }],
        "from": "main.rs:1"
    });
    let result = format_goto_definition(&val);
    assert_eq!(result, "src/lib.rs:1");
}

#[test]
fn goto_multiple_with_external() {
    let val = serde_json::json!({
        "definitions": [
            { "file": "src/a.rs", "line": 10, "end_line": 10, "context": "fn foo()", "source": "project" },
            { "file": "/ext/lib.rs", "line": 20, "end_line": 20, "context": "fn foo()", "source": "lib:serde" }
        ],
        "from": "main.rs:1"
    });
    let result = format_goto_definition(&val);
    assert!(result.contains("2 definitions"));
    assert!(result.contains("src/a.rs:10"));
    assert!(result.contains("(lib:serde)"));
}

// --- format_hover tests ---

#[test]
fn hover_with_code_fence() {
    let val = serde_json::json!({
        "content": "```rust\npub struct OutputGuard {\n    mode: OutputMode,\n}\n```\n\nProgressive disclosure guard.",
        "location": "output.rs:35"
    });
    let result = format_hover(&val);
    assert!(result.starts_with("output.rs:35"));
    assert!(result.contains("  pub struct OutputGuard {"));
    assert!(result.contains("  Progressive disclosure guard."));
    assert!(!result.contains("```"));
}

#[test]
fn hover_plain_text_no_fences() {
    let val = serde_json::json!({
        "content": "Some plain documentation.",
        "location": "lib.rs:10"
    });
    let result = format_hover(&val);
    assert_eq!(result, "lib.rs:10\n\n  Some plain documentation.");
}

#[test]
fn hover_no_location() {
    let val = serde_json::json!({
        "content": "```rust\nfn main() {}\n```"
    });
    let result = format_hover(&val);
    assert!(!result.contains("```"));
    assert!(result.contains("  fn main() {}"));
}

#[test]
fn hover_empty_content() {
    let val = serde_json::json!({});
    assert_eq!(format_hover(&val), "");
}

#[test]
fn hover_null_content_with_hint_surfaces_hint() {
    let val = serde_json::json!({
        "content": null,
        "location": "lib.rs:10",
        "hint": "no hover info at this position",
    });
    let out = format_hover(&val);
    assert!(out.starts_with("lib.rs:10"));
    assert!(out.contains("no hover info at this position"));
}

#[test]
fn hover_null_content_hint_only() {
    let val = serde_json::json!({
        "content": null,
        "hint": "lone hint",
    });
    assert_eq!(format_hover(&val), "lone hint");
}

#[test]
fn hover_multiline_doc() {
    let val = serde_json::json!({
        "content": "```rust\nfn add(a: i32, b: i32) -> i32\n```\n\nAdds two numbers.\n\nReturns the sum.",
        "location": "math.rs:5"
    });
    let result = format_hover(&val);
    assert!(result.contains("  fn add(a: i32, b: i32) -> i32"));
    assert!(result.contains("  Adds two numbers."));
    assert!(result.contains("  Returns the sum."));
    assert!(!result.contains("```"));
}

// --- format_search_symbols tests ---

#[test]
fn symbols_no_body() {
    let val = serde_json::json!({
        "symbols": [
            {
                "name": "OutputGuard", "symbol": "OutputGuard",
                "kind": "Struct", "file": "src/tools/output.rs",
                "start_line": 35, "end_line": 50
            },
            {
                "name": "cap_items", "symbol": "OutputGuard/cap_items",
                "kind": "Function", "file": "src/tools/output.rs",
                "start_line": 55, "end_line": 80
            }
        ],
        "total": 2
    });
    let result = format_search_symbols(&val);
    assert!(result.starts_with("2 matches\n"));
    assert!(result.contains("Struct"));
    assert!(result.contains("Function"));
    assert!(result.contains("OutputGuard"));
    assert!(result.contains("OutputGuard/cap_items"));
    assert!(result.contains("src/tools/output.rs:35-50"));
    assert!(result.contains("src/tools/output.rs:55-80"));
}

#[test]
fn symbols_with_body() {
    let val = serde_json::json!({
        "symbols": [
            {
                "name": "cap_items", "symbol": "OutputGuard/cap_items",
                "kind": "Function", "file": "src/tools/output.rs",
                "start_line": 55, "end_line": 80,
                "body": "pub fn cap_items(&self) -> Option<OverflowInfo> {\n    // impl\n}"
            }
        ],
        "total": 1
    });
    let result = format_search_symbols(&val);
    assert!(result.starts_with("1 match\n"));
    assert!(result.contains("Function"));
    assert!(result.contains("OutputGuard/cap_items"));
    assert!(result.contains("      pub fn cap_items(&self) -> Option<OverflowInfo> {"));
    assert!(result.contains("      // impl"));
    assert!(result.contains("      }"));
}

#[test]
fn symbols_with_long_body_shows_hint_not_truncated_body() {
    // A body > 500 chars should not be inlined — it would get truncated by
    // COMPACT_SUMMARY_MAX_BYTES mid-function, misleading agents into thinking
    // the body is incomplete. Instead, show a navigation hint.
    let long_body = "fun convert() {\n".to_string() + &"    val x = 1\n".repeat(50) + "}";
    assert!(
        long_body.len() > 500,
        "test body should exceed INLINE_BODY_LIMIT"
    );
    let val = serde_json::json!({
        "symbols": [
            {
                "name": "convert", "symbol": "Stage1ToStage2Converter/convert",
                "kind": "Method", "file": "src/Converter.kt",
                "start_line": 160, "end_line": 490,
                "body": long_body
            }
        ],
        "total": 1
    });
    let result = format_search_symbols(&val);
    // Must mention the line count and the extraction path
    assert!(
        result.contains("52-line body"),
        "expected line count in hint, got: {result}"
    );
    assert!(
        result.contains("$.symbols[0].body"),
        "expected json_path hint, got: {result}"
    );
    // Must NOT inline the body content
    assert!(
        !result.contains("val x = 1"),
        "body content must not appear inline"
    );
}

#[test]
fn symbols_with_overflow() {
    let val = serde_json::json!({
        "symbols": [
            {
                "name": "foo", "symbol": "foo",
                "kind": "Function", "file": "src/a.rs",
                "start_line": 10, "end_line": 10
            }
        ],
        "total": 100,
        "overflow": {
            "shown": 20, "total": 100,
            "hint": "narrow with path=",
            "by_file": [["src/a.rs", 50], ["src/b.rs", 30]]
        }
    });
    let result = format_search_symbols(&val);
    assert!(result.contains("20 matches (100 total)"));
    assert!(result.contains("20 of 100"));
    assert!(result.contains("narrow with path="));
}

#[test]
fn symbols_empty() {
    let val = serde_json::json!({
        "symbols": [],
        "total": 0
    });
    assert_eq!(format_search_symbols(&val), "0 matches");
}

#[test]
fn symbols_missing_symbols_key() {
    let val = serde_json::json!({});
    assert_eq!(format_search_symbols(&val), "");
}

#[test]
fn symbols_alignment() {
    let val = serde_json::json!({
        "symbols": [
            {
                "name": "Foo", "symbol": "Foo",
                "kind": "Struct", "file": "src/a.rs",
                "start_line": 1, "end_line": 5
            },
            {
                "name": "bar_baz", "symbol": "bar_baz",
                "kind": "Function", "file": "src/very/long/path.rs",
                "start_line": 100, "end_line": 200
            }
        ],
        "total": 2
    });
    let result = format_search_symbols(&val);
    assert!(result.contains("Struct  "));
    assert!(result.contains("Function"));
    assert!(result.contains("src/a.rs:1-5"));
    assert!(result.contains("src/very/long/path.rs:100-200"));
}

#[test]
fn symbols_single_line_location() {
    let val = serde_json::json!({
        "symbols": [
            {
                "name": "X", "symbol": "X",
                "kind": "Constant", "file": "src/lib.rs",
                "start_line": 42, "end_line": 42
            }
        ],
        "total": 1
    });
    let result = format_search_symbols(&val);
    assert!(result.contains("src/lib.rs:42"));
    assert!(!result.contains("42-42"));
}

// --- format_overview_symbols tests ---

#[test]
fn symbols_overview_file_mode() {
    let val = serde_json::json!({
        "file": "src/tools/output.rs",
        "symbols": [
            {
                "name": "OutputMode", "symbol": "OutputMode",
                "kind": "Enum", "start_line": 10, "end_line": 15,
                "children": [
                    { "name": "Exploring", "kind": "EnumMember", "start_line": 11, "end_line": 11 },
                    { "name": "Focused", "kind": "EnumMember", "start_line": 12, "end_line": 12 }
                ]
            },
            {
                "name": "OutputGuard", "symbol": "OutputGuard",
                "kind": "Struct", "start_line": 35, "end_line": 50
            }
        ]
    });
    let result = format_overview_symbols(&val);
    assert!(result.starts_with("src/tools/output.rs — 2 symbols\n"));
    assert!(result.contains("Enum"));
    assert!(result.contains("OutputMode"));
    assert!(result.contains("L10-15"));
    assert!(result.contains("Exploring"));
    assert!(result.contains("L11"));
    assert!(result.contains("Focused"));
    assert!(result.contains("L12"));
    assert!(result.contains("Struct"));
    assert!(result.contains("OutputGuard"));
    assert!(result.contains("L35-50"));
    assert!(!result.contains("EnumMember"));
}

#[test]
fn symbols_overview_directory_mode() {
    let val = serde_json::json!({
        "directory": "src/tools",
        "files": [
            {
                "file": "src/tools/ast.rs",
                "symbols": [
                    { "name": "ListFunctions", "symbol": "ListFunctions", "kind": "Struct", "start_line": 10, "end_line": 20 }
                ]
            },
            {
                "file": "src/tools/config.rs",
                "symbols": [
                    { "name": "GetConfig", "symbol": "GetConfig", "kind": "Struct", "start_line": 5, "end_line": 15 },
                    { "name": "ActivateProject", "symbol": "ActivateProject", "kind": "Struct", "start_line": 20, "end_line": 30 }
                ]
            }
        ]
    });
    let result = format_overview_symbols(&val);
    assert!(result.starts_with("src/tools\n"));
    assert!(result.contains("src/tools/ast.rs — 1 symbol\n"));
    assert!(result.contains("src/tools/config.rs — 2 symbols\n"));
    assert!(result.contains("ListFunctions"));
    assert!(result.contains("GetConfig"));
    assert!(result.contains("ActivateProject"));
}

#[test]
fn symbols_overview_pattern_mode() {
    let val = serde_json::json!({
        "pattern": "src/**/*.rs",
        "files": [
            {
                "file": "src/main.rs",
                "symbols": [
                    { "name": "main", "symbol": "main", "kind": "Function", "start_line": 1, "end_line": 10 }
                ]
            }
        ]
    });
    let result = format_overview_symbols(&val);
    assert!(result.starts_with("src/**/*.rs\n"));
    assert!(result.contains("src/main.rs — 1 symbol\n"));
    assert!(result.contains("main"));
}

#[test]
fn symbols_overview_empty_file() {
    let val = serde_json::json!({
        "file": "src/empty.rs",
        "symbols": []
    });
    let result = format_overview_symbols(&val);
    assert!(result.contains("0 symbols"));
}

#[test]
fn symbols_overview_empty_directory() {
    let val = serde_json::json!({
        "directory": "src/empty",
        "files": []
    });
    let result = format_overview_symbols(&val);
    assert_eq!(result, "src/empty — 0 symbols");
}

#[test]
fn symbols_overview_with_overflow() {
    let val = serde_json::json!({
        "directory": "src",
        "files": [
            {
                "file": "src/a.rs",
                "symbols": [
                    { "name": "Foo", "symbol": "Foo", "kind": "Struct", "start_line": 1, "end_line": 5 }
                ]
            }
        ],
        "overflow": { "shown": 10, "total": 50, "hint": "Narrow with a more specific glob or file path" }
    });
    let result = format_overview_symbols(&val);
    assert!(result.contains("10 of 50"));
    assert!(result.contains("Narrow with a more specific glob"));
}

#[test]
fn symbols_overview_children_with_fields() {
    let val = serde_json::json!({
        "file": "src/model.rs",
        "symbols": [
            {
                "name": "Config", "symbol": "Config",
                "kind": "Struct", "start_line": 1, "end_line": 10,
                "children": [
                    { "name": "port", "kind": "Field", "start_line": 2, "end_line": 2 },
                    { "name": "host", "kind": "Field", "start_line": 3, "end_line": 3 }
                ]
            }
        ]
    });
    let result = format_overview_symbols(&val);
    assert!(!result.contains("Field"));
    assert!(result.contains("port"));
    assert!(result.contains("host"));
    assert!(result.contains("L2"));
    assert!(result.contains("L3"));
}

#[test]
fn symbols_overview_children_with_methods() {
    let val = serde_json::json!({
        "file": "src/service.rs",
        "symbols": [
            {
                "name": "Server", "symbol": "Server",
                "kind": "Struct", "start_line": 1, "end_line": 50,
                "children": [
                    { "name": "new", "kind": "Function", "start_line": 5, "end_line": 10 },
                    { "name": "run", "kind": "Function", "start_line": 12, "end_line": 40 }
                ]
            }
        ]
    });
    let result = format_overview_symbols(&val);
    assert!(result.contains("Function  new"));
    assert!(result.contains("Function  run"));
}

#[test]
fn symbols_overview_missing_symbols_key() {
    let val = serde_json::json!({});
    assert_eq!(format_overview_symbols(&val), "");
}

#[test]
fn symbols_overview_singular_symbol_word() {
    let val = serde_json::json!({
        "file": "src/single.rs",
        "symbols": [
            { "name": "main", "symbol": "main", "kind": "Function", "start_line": 1, "end_line": 5 }
        ]
    });
    let result = format_overview_symbols(&val);
    assert!(result.contains("1 symbol\n"));
    assert!(!result.contains("1 symbols"));
}

// --- format_find_references tests ---

#[test]
fn find_references_basic() {
    let result = serde_json::json!({
        "references": [
            {"file": "src/foo.rs", "line": 10, "kind": "usage"},
            {"file": "src/bar.rs", "line": 20, "kind": "usage"},
            {"file": "src/foo.rs", "line": 30, "kind": "usage"}
        ],
        "total": 3
    });
    let text = format_find_references(&result);
    assert!(text.contains("3"), "should mention count");
    assert!(
        text.contains("refs") || text.contains("reference"),
        "should say refs or reference(s)"
    );
}

#[test]
fn find_references_empty() {
    let result = serde_json::json!({ "references": [], "total": 0 });
    let text = format_find_references(&result);
    assert!(
        text.contains("No"),
        "should say 'No references found.', got: {}",
        text
    );
}

#[test]
fn format_find_references_shows_locations() {
    let result = serde_json::json!({
        "total": 8,
        "references": [
            {"file": "src/tools/symbol.rs", "line": 142},
            {"file": "src/tools/symbol.rs", "line": 198},
            {"file": "src/server.rs", "line": 87},
            {"file": "src/agent.rs", "line": 210},
            {"file": "src/main.rs", "line": 45},
            {"file": "src/config.rs", "line": 12}
        ]
    });
    let out = format_find_references(&result);
    assert!(out.contains("8 refs"), "should show total");
    assert!(
        out.contains("src/tools/symbol.rs:142"),
        "should show locations"
    );
    assert!(out.contains("src/server.rs:87"), "should show locations");
    assert!(out.contains("more"), "should show trailer for hidden refs");
    assert!(!out.contains("src/config.rs"), "should cap at 5");
}

#[test]
fn format_find_references_five_or_fewer_no_trailer() {
    let result = serde_json::json!({
        "total": 3,
        "references": [
            {"file": "src/a.rs", "line": 1},
            {"file": "src/b.rs", "line": 2},
            {"file": "src/c.rs", "line": 3}
        ]
    });
    let out = format_find_references(&result);
    assert!(out.contains("src/a.rs:1"));
    assert!(!out.contains("more"), "no trailer when all fit");
}

#[tokio::test]
async fn symbols_falls_back_to_document_symbols_on_bad_workspace_range() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider, SymbolInfo, SymbolKind};

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(
        &file,
        "fn helper(x: i32) -> i32 {\n    let y = x + 1;\n    y * 2\n}\n",
    )
    .unwrap();

    // workspace/symbol returns degenerate range (start == end)
    let degenerate = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    // document_symbols returns correct range
    let correct = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 3,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    let mock = MockLspClient::new()
        .with_workspace_symbols(vec![degenerate])
        .with_symbols(&file, vec![correct]);
    let lsp = MockLspProvider::with_client(mock);

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = Symbols
        .call(
            json!({
                "query": "helper",
                "include_body": true,
            }),
            &ctx,
        )
        .await;

    let val = result.expect("symbols should recover via document_symbols fallback");
    let symbols = val["symbols"].as_array().expect("symbols array");
    assert_eq!(symbols.len(), 1, "should find exactly one symbol");

    let body = symbols[0]["body"].as_str().expect("body should be present");
    assert!(
        body.contains("let y = x + 1"),
        "body should contain function contents; got: {body}"
    );
}

#[tokio::test]
async fn symbol_at_def_uses_col_param_over_identifier() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider};
    use crate::tools::symbol::SymbolAt;

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    // line 0: `fn helper(x: i32) {}` — identifier `helper` at byte col 3
    std::fs::write(&file, "fn helper(x: i32) {}\n").unwrap();

    // Register definition keyed at (line=0, col=3) only.
    // Caller passes col=4 (1-indexed) → 3 (0-indexed) plus a bogus identifier
    // that doesn't exist on the line. With col-priority, identifier is ignored.
    let target = lsp_types::Location {
        uri: url::Url::from_file_path(&file)
            .unwrap()
            .as_str()
            .parse()
            .unwrap(),
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 5,
                character: 0,
            },
            end: lsp_types::Position {
                line: 5,
                character: 6,
            },
        },
    };
    let mock = MockLspClient::new().with_definitions(0, 3, vec![target]);
    let lsp = MockLspProvider::with_client(mock);

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = SymbolAt
        .call(
            json!({
                "path": "src/lib.rs",
                "line": 1,
                "col": 4,
                "identifier": "DOES_NOT_EXIST_ON_LINE",
                "fields": ["def"],
            }),
            &ctx,
        )
        .await
        .expect("col should win over identifier and resolve");
    let defs = result["def"]["definitions"].as_array().unwrap();
    assert_eq!(defs.len(), 1, "exactly one definition expected");
    assert_eq!(defs[0]["line"].as_u64(), Some(6)); // 5 + 1 (1-indexed)
}

#[tokio::test]
async fn symbol_at_hover_returns_ok_with_null_content_when_lsp_empty() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider};
    use crate::tools::symbol::SymbolAt;

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(&file, "fn x() {}\n").unwrap();

    // MockLspClient::hover always returns Ok(None) — exercises empty-result path.
    let mock = MockLspClient::new();
    let lsp = MockLspProvider::with_client(mock);

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = SymbolAt
        .call(
            json!({
                "path": "src/lib.rs",
                "line": 1,
                "col": 4,
                "fields": ["hover"],
            }),
            &ctx,
        )
        .await
        .expect("empty hover must be Ok, not Err (no misclassification)");
    assert!(
        result["hover"]["content"].is_null(),
        "content should be null on empty"
    );
    assert!(
        result["hover"]["hint"].as_str().is_some(),
        "hint should be present to guide caller"
    );
}

#[tokio::test]
async fn symbol_at_hover_col_zero_rejected() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider};
    use crate::tools::symbol::SymbolAt;

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(&file, "fn x() {}\n").unwrap();

    let lsp = MockLspProvider::with_client(MockLspClient::new());
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let err = SymbolAt
        .call(
            json!({"path": "src/lib.rs", "line": 1, "col": 0, "fields": ["hover"]}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("'col' must be >= 1"), "got: {err}");
}

#[tokio::test]
async fn symbol_at_hover_retries_once_on_mux_disconnect() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider};
    use crate::tools::symbol::SymbolAt;

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(&file, "fn x() {}\n").unwrap();

    // First hover errors with the magic mux-disconnect string; helper retries
    // and the second call succeeds with real content.
    let mock = MockLspClient::new().with_hover_responses(vec![
        Err(anyhow::anyhow!("Mux connection lost")),
        Ok(Some("```rust\nfn x()\n```".to_string())),
    ]);
    let lsp = MockLspProvider::with_client(mock);

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = SymbolAt
        .call(
            json!({"path": "src/lib.rs", "line": 1, "col": 4, "fields": ["hover"]}),
            &ctx,
        )
        .await
        .expect("transient mux disconnect should be retried, not surfaced");
    assert_eq!(
        result["hover"]["content"].as_str(),
        Some("```rust\nfn x()\n```"),
        "second-attempt content should be returned"
    );
}

#[tokio::test]
async fn symbol_at_hover_does_not_retry_non_disconnect_errors() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider};
    use crate::tools::symbol::SymbolAt;

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(&file, "fn x() {}\n").unwrap();

    let mock = MockLspClient::new()
        .with_hover_responses(vec![Err(anyhow::anyhow!("some unrelated LSP error"))]);
    let lsp = MockLspProvider::with_client(mock);

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let err = SymbolAt
        .call(
            json!({"path": "src/lib.rs", "line": 1, "col": 4, "fields": ["hover"]}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("some unrelated LSP error"),
        "non-disconnect errors must surface immediately, got: {err}"
    );
}

#[tokio::test]
async fn symbol_at_returns_both_fields_by_default() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider};
    use crate::tools::symbol::SymbolAt;

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(&file, "fn helper(x: i32) {}\n").unwrap();

    // Hover returns Some(...), goto_definition returns one location.
    let target = lsp_types::Location {
        uri: url::Url::from_file_path(&file)
            .unwrap()
            .as_str()
            .parse()
            .unwrap(),
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 3,
            },
            end: lsp_types::Position {
                line: 0,
                character: 9,
            },
        },
    };
    let mock = MockLspClient::new()
        .with_definitions(0, 3, vec![target])
        .with_hover_responses(vec![Ok(Some(
            "```rust\nfn helper(x: i32)\n```".to_string(),
        ))]);
    let lsp = MockLspProvider::with_client(mock);

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = SymbolAt
        .call(json!({"path": "src/lib.rs", "line": 1, "col": 4}), &ctx)
        .await
        .expect("symbol_at with default fields should succeed");
    assert!(
        result.get("def").is_some(),
        "default fields should include def; got: {result:?}"
    );
    assert!(
        result.get("hover").is_some(),
        "default fields should include hover; got: {result:?}"
    );
}

#[test]
fn find_matching_symbol_finds_top_level() {
    use crate::lsp::SymbolKind;
    let symbols = vec![SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: SymbolKind::Function,
        file: PathBuf::from("lib.rs"),
        start_line: 10,
        end_line: 20,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    }];
    let result = find_matching_symbol(&symbols, "foo", 10);
    assert!(result.is_some());
    assert_eq!(result.unwrap().end_line, 20);
}

#[test]
fn find_matching_symbol_finds_nested_child() {
    use crate::lsp::SymbolKind;
    let child = SymbolInfo {
        name: "bar".to_string(),
        name_path: "Foo/bar".to_string(),
        kind: SymbolKind::Function,
        file: PathBuf::from("lib.rs"),
        start_line: 15,
        end_line: 18,
        start_col: 4,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    let parent = SymbolInfo {
        name: "Foo".to_string(),
        name_path: "Foo".to_string(),
        kind: SymbolKind::Struct,
        file: PathBuf::from("lib.rs"),
        start_line: 10,
        end_line: 20,
        start_col: 0,
        children: vec![child],
        range_start_line: None,
        detail: None,
    };
    let result = find_matching_symbol(&[parent], "bar", 15);
    assert!(result.is_some());
    assert_eq!(result.unwrap().end_line, 18);
}

#[test]
fn find_matching_symbol_returns_none_on_name_mismatch() {
    use crate::lsp::SymbolKind;
    let symbols = vec![SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: SymbolKind::Function,
        file: PathBuf::from("lib.rs"),
        start_line: 10,
        end_line: 20,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    }];
    let result = find_matching_symbol(&symbols, "bar", 10);
    assert!(result.is_none());
}

#[test]
fn find_matching_symbol_returns_none_when_line_too_far() {
    use crate::lsp::SymbolKind;
    let symbols = vec![SymbolInfo {
        name: "foo".to_string(),
        name_path: "foo".to_string(),
        kind: SymbolKind::Function,
        file: PathBuf::from("lib.rs"),
        start_line: 10,
        end_line: 20,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    }];
    // lsp_start=13 → abs_diff(10, 13) = 3 > 1 → no match
    let result = find_matching_symbol(&symbols, "foo", 13);
    assert!(result.is_none());
}

#[tokio::test]
async fn symbols_propagates_error_when_fallback_also_fails() {
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider, SymbolInfo, SymbolKind};

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("lib.rs");
    std::fs::write(
        &file,
        "fn helper(x: i32) -> i32 {\n    let y = x + 1;\n    y * 2\n}\n",
    )
    .unwrap();

    // workspace/symbol returns degenerate range
    let degenerate = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: file.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };

    // document_symbols returns NOTHING — fallback will fail
    let mock = MockLspClient::new().with_workspace_symbols(vec![degenerate]);
    // Note: NOT calling .with_symbols() — document_symbols will return empty vec
    let lsp = MockLspProvider::with_client(mock);

    // Use the same ToolContext setup pattern as the other test
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = Symbols
        .call(
            json!({
                "query": "helper",
                "include_body": true,
            }),
            &ctx,
        )
        .await;

    // Should fail with the original RecoverableError
    let err = result.expect_err("should propagate error when fallback fails");
    let msg = err.to_string();
    assert!(
        msg.contains("suspicious range"),
        "error should mention suspicious range; got: {msg}"
    );
}

// ── resolve_library_roots ────────────────────────────────────────────────

#[tokio::test]
async fn resolve_library_roots_empty_when_no_libraries() {
    let dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let roots = resolve_library_roots(&crate::library::scope::Scope::Libraries, &agent)
        .await
        .unwrap();
    assert!(roots.is_empty());
}

#[tokio::test]
async fn resolve_library_roots_returns_registered_libraries() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "mylib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }
    let roots = resolve_library_roots(&crate::library::scope::Scope::Libraries, &agent)
        .await
        .unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].0, "mylib");
    assert_eq!(roots[0].1, lib_dir.path().to_path_buf());
}

#[tokio::test]
async fn resolve_library_roots_filters_by_name() {
    let dir = tempdir().unwrap();
    let lib1 = tempdir().unwrap();
    let lib2 = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "alpha".to_string(),
            lib1.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
        project.library_registry.register(
            "beta".to_string(),
            lib2.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }
    let roots = resolve_library_roots(
        &crate::library::scope::Scope::Library("alpha".to_string()),
        &agent,
    )
    .await
    .unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].0, "alpha");
}

#[tokio::test]
async fn resolve_library_roots_project_scope_returns_empty() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "mylib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }
    let roots = resolve_library_roots(&crate::library::scope::Scope::Project, &agent)
        .await
        .unwrap();
    assert!(roots.is_empty());
}

#[tokio::test]
async fn resolve_library_roots_all_scope_returns_all() {
    let dir = tempdir().unwrap();
    let lib1 = tempdir().unwrap();
    let lib2 = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "alpha".to_string(),
            lib1.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
        project.library_registry.register(
            "beta".to_string(),
            lib2.path().to_path_buf(),
            "python".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }
    let roots = resolve_library_roots(&crate::library::scope::Scope::All, &agent)
        .await
        .unwrap();
    assert_eq!(roots.len(), 2);
}

#[tokio::test]
async fn resolve_library_roots_excludes_source_unavailable() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "available".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
        project.library_registry.register(
            "unavailable".to_string(),
            PathBuf::new(),
            "java".to_string(),
            crate::library::registry::DiscoveryMethod::ManifestScan,
            false,
        );
    }
    // Explicit library scope for unavailable lib should error
    let result = resolve_library_roots(
        &crate::library::scope::Scope::Library("unavailable".to_string()),
        &agent,
    )
    .await;
    assert!(
        result.is_err(),
        "should return error for source-unavailable library"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains("source code is not available"), "error: {err}");

    // All scope should silently skip unavailable
    let roots = resolve_library_roots(&crate::library::scope::Scope::All, &agent)
        .await
        .unwrap();
    assert_eq!(
        roots.len(),
        1,
        "All scope should only return available libs"
    );
    assert_eq!(roots[0].0, "available");
}

// ── format_library_path ──────────────────────────────────────────────────

#[test]
fn format_library_path_strips_root() {
    let lib_root = PathBuf::from("/home/user/.cargo/registry/src/serde-1.0");
    let file = PathBuf::from("/home/user/.cargo/registry/src/serde-1.0/src/lib.rs");
    let result = format_library_path("serde", &lib_root, &file);
    assert_eq!(result, "lib:serde/src/lib.rs");
}

#[test]
fn format_library_path_fallback_for_outside_root() {
    let lib_root = PathBuf::from("/home/user/.cargo/registry/src/serde-1.0");
    let file = PathBuf::from("/somewhere/else/lib.rs");
    let result = format_library_path("serde", &lib_root, &file);
    assert_eq!(result, "/somewhere/else/lib.rs");
}

// ── classify_reference_path ──────────────────────────────────────────────

#[test]
fn classify_reference_path_project() {
    let root = PathBuf::from("/project");
    let libs = vec![("mylib".to_string(), PathBuf::from("/libs/mylib"))];
    let path = PathBuf::from("/project/src/main.rs");
    let (classification, display) = classify_reference_path(&path, &root, &libs);
    assert_eq!(classification, "project");
    assert_eq!(display, "src/main.rs");
}

#[test]
fn classify_reference_path_library() {
    let root = PathBuf::from("/project");
    let libs = vec![("mylib".to_string(), PathBuf::from("/libs/mylib"))];
    let path = PathBuf::from("/libs/mylib/src/lib.rs");
    let (classification, display) = classify_reference_path(&path, &root, &libs);
    assert_eq!(classification, "lib:mylib");
    assert_eq!(display, "lib:mylib/src/lib.rs");
}

#[test]
fn classify_reference_path_external() {
    let root = PathBuf::from("/project");
    let libs = vec![("mylib".to_string(), PathBuf::from("/libs/mylib"))];
    let path = PathBuf::from("/somewhere/else.rs");
    let (classification, display) = classify_reference_path(&path, &root, &libs);
    assert_eq!(classification, "external");
    assert_eq!(display, "/somewhere/else.rs");
}

fn test_ctx_with_agent(agent: Agent) -> ToolContext {
    ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    }
}

#[tokio::test]
async fn symbols_overview_scope_libraries_includes_library_files() {
    let project_dir = tempdir().unwrap();
    std::fs::create_dir_all(project_dir.path().join(".codescout")).unwrap();
    let lib_dir = tempdir().unwrap();
    let lib_src = lib_dir.path().join("src");
    std::fs::create_dir_all(&lib_src).unwrap();
    std::fs::write(lib_src.join("lib.rs"), "pub fn hello() {}\n").unwrap();

    let agent = Agent::new(Some(project_dir.path().to_path_buf()))
        .await
        .unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = Symbols;
    let result = tool
        .call(json!({"scope": "libraries"}), &ctx)
        .await
        .unwrap();

    let files = result["files"].as_array().unwrap();
    assert!(!files.is_empty(), "should find library files");
    let first_file = files[0]["file"].as_str().unwrap();
    assert!(
        first_file.starts_with("lib:testlib/"),
        "library file should have lib: prefix, got: {}",
        first_file
    );
}

#[tokio::test]
async fn symbols_overview_scope_project_excludes_libraries() {
    let project_dir = tempdir().unwrap();
    std::fs::create_dir_all(project_dir.path().join(".codescout")).unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(lib_dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    std::fs::write(project_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let agent = Agent::new(Some(project_dir.path().to_path_buf()))
        .await
        .unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = Symbols;
    let result = tool.call(json!({"scope": "project"}), &ctx).await.unwrap();

    let empty = vec![];
    let files = result["files"].as_array().unwrap_or(&empty);
    for f in files {
        let path = f["file"].as_str().unwrap();
        assert!(
            !path.starts_with("lib:"),
            "project scope should not include library files: {}",
            path
        );
    }
}

#[tokio::test]
async fn symbols_scope_libraries_searches_library_dirs() {
    let project_dir = tempdir().unwrap();
    std::fs::create_dir_all(project_dir.path().join(".codescout")).unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(
        lib_dir.path().join("src/lib.rs"),
        "pub fn library_unique_symbol_xyz() {}\n",
    )
    .unwrap();

    let agent = Agent::new(Some(project_dir.path().to_path_buf()))
        .await
        .unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = Symbols;
    let result = tool
        .call(
            json!({
                "query": "library_unique_symbol_xyz",
                "scope": "libraries"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(!symbols.is_empty(), "should find symbol in library");
    let file = symbols[0]["file"].as_str().unwrap();
    assert!(
        file.starts_with("lib:testlib/"),
        "file path should have lib: prefix: {}",
        file
    );
}

#[tokio::test]
async fn symbols_scope_all_searches_both() {
    let project_dir = tempdir().unwrap();
    std::fs::create_dir_all(project_dir.path().join(".codescout")).unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::write(project_dir.path().join("main.rs"), "fn project_func() {}\n").unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(lib_dir.path().join("src/lib.rs"), "pub fn lib_func() {}\n").unwrap();

    let agent = Agent::new(Some(project_dir.path().to_path_buf()))
        .await
        .unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = Symbols;
    let result = tool
        .call(
            json!({
                "query": "func",
                "scope": "all"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    let files: Vec<&str> = symbols.iter().filter_map(|s| s["file"].as_str()).collect();
    assert!(
        files.iter().any(|f| f.starts_with("lib:testlib/")),
        "should include library symbol"
    );
    assert!(
        files.iter().any(|f| !f.starts_with("lib:")),
        "should include project symbol"
    );
}

#[tokio::test]
async fn symbols_scope_project_default_excludes_libraries() {
    let project_dir = tempdir().unwrap();
    std::fs::create_dir_all(project_dir.path().join(".codescout")).unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::write(project_dir.path().join("main.rs"), "fn my_func() {}\n").unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(lib_dir.path().join("src/lib.rs"), "pub fn my_func() {}\n").unwrap();

    let agent = Agent::new(Some(project_dir.path().to_path_buf()))
        .await
        .unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
            true,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = Symbols;
    let result = tool
        .call(
            json!({
                "query": "my_func",
                "scope": "project"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    for s in symbols {
        let file = s["file"].as_str().unwrap();
        assert!(
            !file.starts_with("lib:"),
            "project scope should not include library: {}",
            file
        );
    }
}

/// symbols with multiple matches returns all of them.
#[tokio::test]
async fn symbols_with_multiple_matches_returns_all() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/a")).unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // Three files each defining a function named `process_*` — guarantees 3+ matches.
    std::fs::write(
        dir.path().join("src/a/alpha.rs"),
        "pub fn process_alpha() -> i32 { 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/a/beta.rs"),
        "pub fn process_beta() -> i32 { 2 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/a/gamma.rs"),
        "pub fn process_gamma() -> i32 { 3 }\n",
    )
    .unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None, // no elicitation peer
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = Symbols
        .call(json!({ "query": "process" }), &ctx)
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    // With no peer, ALL matches must be returned (no disambiguation prompt).
    assert!(
        symbols.len() >= 3,
        "should return all matches when peer=None, got {} symbols: {:?}",
        symbols.len(),
        result
    );
    // The total field must also reflect >= 3 results.
    let total = result["total"].as_u64().unwrap_or(0);
    assert!(total >= 3, "total should be >= 3 with no peer, got {total}");
}

#[tokio::test]
async fn symbols_rejects_regex_alternation() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    let err = Symbols
        .call(json!({"query": "foo|bar"}), &ctx)
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("should be RecoverableError");
    assert!(
        rec.message.contains("regex"),
        "message should mention regex, got: {}",
        rec.message
    );
    assert!(
        rec.hint().unwrap_or("").contains("grep"),
        "hint should mention grep, got: {:?}",
        rec.hint()
    );
}

#[tokio::test]
async fn symbols_rejects_regex_wildcard() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    let err = Symbols
        .call(json!({"query": "foo.*bar"}), &ctx)
        .await
        .unwrap_err();

    assert!(
        err.downcast_ref::<crate::tools::RecoverableError>()
            .is_some(),
        "should be RecoverableError, got: {}",
        err
    );
}

#[tokio::test]
async fn symbols_allows_plain_pattern() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("test.rs"), "fn my_function() {}\n").unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    let result = Symbols.call(json!({"query": "my_function"}), &ctx).await;
    assert!(result.is_ok(), "plain pattern should not be rejected");
}

#[tokio::test]
async fn symbols_allows_name_path_with_regex_chars() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);

    let result = Symbols.call(json!({"symbol": "foo|bar"}), &ctx).await;
    assert!(
        result.is_ok(),
        "name_path should skip regex check, got err: {:?}",
        result.err()
    );
}

#[test]
fn find_split_point_collapses_single_child_chain() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // a/ → b/ → c/ (three files directly in c/) — should collapse to c/
    std::fs::create_dir_all(root.join("a/b/c")).unwrap();
    for i in 0..3 {
        std::fs::write(root.join(format!("a/b/c/file{i}.rs")), "").unwrap();
    }
    let split = find_split_point(root);
    assert_eq!(split, root.join("a/b/c"));
}

#[test]
fn find_split_point_stops_at_branch() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::create_dir_all(root.join("a/c")).unwrap();
    std::fs::write(root.join("a/b/file.rs"), "").unwrap();
    std::fs::write(root.join("a/c/file.rs"), "").unwrap();
    let split = find_split_point(root);
    assert_eq!(split, root.join("a"), "should stop at branching dir");
}

#[test]
fn find_split_point_stops_when_dir_has_direct_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // a/ has one child b/ but also a direct source file — stop here
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("a/root.rs"), "").unwrap();
    std::fs::write(root.join("a/b/file.rs"), "").unwrap();
    let split = find_split_point(root);
    assert_eq!(split, root.join("a"), "mixed dir stops descent");
}

#[test]
fn count_files_by_subdir_groups_and_sorts() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("sub_a")).unwrap();
    for i in 0..3 {
        std::fs::write(root.join(format!("sub_a/file{i}.rs")), "").unwrap();
    }
    std::fs::create_dir_all(root.join("sub_b")).unwrap();
    for i in 0..5 {
        std::fs::write(root.join(format!("sub_b/file{i}.rs")), "").unwrap();
    }
    // 1 file directly in root (counted in total, not in subdirs)
    std::fs::write(root.join("root.rs"), "").unwrap();

    let (total, subdirs) = count_files_by_subdir(root, root);

    assert_eq!(total, 9);
    assert_eq!(subdirs.len(), 2);
    assert!(subdirs[0].0.contains("sub_b"), "largest subdir first");
    assert_eq!(subdirs[0].1, 5);
    assert!(subdirs[1].0.contains("sub_a"));
    assert_eq!(subdirs[1].1, 3);
}

#[test]
fn count_files_by_subdir_collapses_passthrough() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // kotlin/ → edu/ → planner/ → [api/(3), domain/(2)]
    for (sub, n) in &[("api", 3usize), ("domain", 2)] {
        std::fs::create_dir_all(root.join(format!("kotlin/edu/planner/{sub}"))).unwrap();
        for i in 0..*n {
            std::fs::write(root.join(format!("kotlin/edu/planner/{sub}/f{i}.rs")), "").unwrap();
        }
    }
    let (total, subdirs) = count_files_by_subdir(root, &root.join("kotlin"));
    assert_eq!(total, 5);
    assert_eq!(subdirs.len(), 2, "collapsed to planner/ children, not edu/");
    assert!(subdirs[0].0.contains("api"), "api (3) before domain (2)");
    assert_eq!(subdirs[0].1, 3);
}

#[test]
fn count_files_by_subdir_flat_dir_returns_empty_subdirs() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    for i in 0..4 {
        std::fs::write(root.join(format!("file{i}.rs")), "").unwrap();
    }
    let (total, subdirs) = count_files_by_subdir(root, root);
    assert_eq!(total, 4);
    assert!(subdirs.is_empty());
}

#[test]
fn count_files_by_subdir_ignores_non_source_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("sub/README.md"), "").unwrap(); // ignored
    std::fs::write(root.join("sub/build.rs"), "").unwrap(); // counted
    let (total, _subdirs) = count_files_by_subdir(root, root);
    assert_eq!(total, 1, "markdown should not be counted as source");
}

#[test]
fn ast_class_names_for_dir_extracts_class_like_symbols() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("types.rs"),
        r#"
struct Foo { x: i32 }
struct Bar;
enum Baz { A, B }
fn not_a_class() {}
const SKIP: i32 = 1;
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("README.md"), "# hi").unwrap();

    let names = ast_class_names_for_dir(dir.path());

    assert!(names.contains(&"Foo".to_string()));
    assert!(names.contains(&"Bar".to_string()));
    assert!(names.contains(&"Baz".to_string()));
    assert!(!names.contains(&"not_a_class".to_string()));
    assert!(!names.contains(&"SKIP".to_string()));
    // sorted
    assert_eq!(names, {
        let mut v = names.clone();
        v.sort();
        v
    });
}

#[test]
fn ast_class_names_for_dir_does_not_recurse_into_subdirs() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/deep.rs"), "struct DeepClass;").unwrap();
    std::fs::write(dir.path().join("top.rs"), "struct TopClass;").unwrap();

    let names = ast_class_names_for_dir(dir.path());

    assert!(names.contains(&"TopClass".to_string()));
    assert!(!names.contains(&"DeepClass".to_string()));
}

#[tokio::test]
async fn symbols_overview_nested_dir_returns_overview_mode() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    // sub_a and sub_b each with 20 Rust files (total=40 > RECURSE_SMALL=30)
    for sub in &["sub_a", "sub_b"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
        for i in 0..20 {
            std::fs::write(root.join(format!("{sub}/f{i}.rs")), "pub struct S;").unwrap();
        }
    }
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);
    let result = Symbols.call(json!({ "path": "." }), &ctx).await.unwrap();

    // 40 files in two subdirs → class_overview (31–80 range)
    assert_eq!(result["mode"].as_str(), Some("class_overview"));
    let subdirs = result["subdirectories"].as_array().unwrap();
    assert_eq!(subdirs.len(), 2);
    assert_eq!(result["total_files"].as_u64(), Some(40));
    let sub_a = subdirs
        .iter()
        .find(|s| s["path"].as_str().unwrap_or("").contains("sub_a"))
        .unwrap();
    assert!(
        sub_a["classes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c.as_str() == Some("S")),
        "AST class names extracted"
    );
}

#[tokio::test]
async fn symbols_overview_force_mode_symbols_bypasses_threshold() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    for sub in &["sub_a", "sub_b"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
        for i in 0..20 {
            std::fs::write(root.join(format!("{sub}/f{i}.rs")), "pub struct S;").unwrap();
        }
    }
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = test_ctx_with_agent(agent);
    let result = Symbols
        .call(json!({ "path": ".", "force_mode": "symbols" }), &ctx)
        .await
        .unwrap();

    // force_mode: "symbols" → no "mode" key, returns files array
    assert!(result["mode"].is_null(), "no mode field in symbols output");
    assert!(result["files"].is_array(), "files array present");
}
