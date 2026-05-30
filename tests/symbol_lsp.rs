//! Regression tests for LSP-backed symbol tools using a mock LSP client.
//!
//! These tests verify the "trust LSP" file-splice logic without requiring a live
//! language server. The mock returns pre-configured symbol positions that reproduce
//! LSP range quirks (over-extension, degenerate ranges, lead-in artifacts).

use codescout::agent::Agent;
use codescout::lsp::{MockLspClient, MockLspProvider, SymbolInfo, SymbolKind};
use codescout::tools::symbol::{EditCode, SymbolAt, Symbols};
use codescout::tools::{Tool, ToolContext};
use serde_json::json;

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Build a ToolContext with a mock LSP provider.
///
/// `files` are written relative to a fresh tempdir (which becomes the project root).
/// `build_mock` receives the absolute project root so it can pre-load symbols keyed
/// by their absolute paths (which is what the tool passes to `document_symbols`).
async fn ctx_with_mock(
    files: &[(&str, &str)],
    build_mock: impl FnOnce(&std::path::Path) -> MockLspClient,
) -> (tempfile::TempDir, ToolContext) {
    let dir = tempfile::tempdir().unwrap();
    // Canonicalize the project root so mock-keyed symbol paths match what
    // production code looks up after its own canonicalize() pass. On macOS
    // tempdir() returns `/var/folders/...` but Agent canonicalizes to
    // `/private/var/folders/...`; without this the mock lookup misses.
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    for (name, content) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
    let mock = build_mock(&root);
    let agent = Agent::new(Some(root.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: MockLspProvider::with_client(mock),
        output_buffer: std::sync::Arc::new(codescout::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            codescout::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    (dir, ctx)
}

/// Build a minimal SymbolInfo for use in mock fixtures (0-indexed lines).
fn sym(
    name: &str,
    start_line: u32,
    end_line: u32,
    path: impl Into<std::path::PathBuf>,
) -> SymbolInfo {
    SymbolInfo {
        name: name.to_string(),
        name_path: name.to_string(),
        kind: SymbolKind::Function,
        file: path.into(),
        start_line,
        end_line,
        start_col: 0,
        children: vec![],
        range_start_line: None,
        detail: None,
    }
}

/// Like `sym`, but with an explicit `range_start_line` (simulates documentSymbol
/// which provides both `selectionRange` and `range`).
fn sym_with_range(
    name: &str,
    start_line: u32,
    end_line: u32,
    range_start: u32,
    path: impl Into<std::path::PathBuf>,
) -> SymbolInfo {
    SymbolInfo {
        name: name.to_string(),
        name_path: name.to_string(),
        kind: SymbolKind::Function,
        file: path.into(),
        start_line,
        end_line,
        start_col: 0,
        children: vec![],
        range_start_line: Some(range_start),
        detail: None,
    }
}

// ── replace_symbol: trust LSP start_line ─────────────────────────────────────

/// With "trust LSP" design, when LSP says start_line=0 (the `}` of a preceding
/// method), we replace from line 0. The preceding `}` is replaced along with the
/// old body — there is no lead-in skipping.
#[tokio::test]
async fn replace_symbol_trusts_lsp_start_line() {
    // File layout (0-indexed):
    //  0: "    }"          ← closing brace of a preceding method (LSP start_line=0)
    //  1: ""               ← blank line
    //  2: "    fn target() {"
    //  3: "        old_body();"
    //  4: "    }"
    let src = "    }\n\n    fn target() {\n        old_body();\n    }\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(
            file.clone(),
            // LSP reports start_line=0 (the `}` line) — trust LSP, replace from there
            vec![sym("target", 0, 4, file)],
        )
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": "    fn target() {\n        new_body();\n    }"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    // With "trust LSP", the preceding `}` is within the LSP range and is replaced
    assert!(
        result.contains("new_body()"),
        "replacement body must be applied; got:\n{result}"
    );
    assert!(
        !result.contains("old_body()"),
        "old body must be gone; got:\n{result}"
    );
}

/// With "trust LSP" design, when LSP says start_line=0 (the `})` of a preceding
/// method), we replace from line 0. The `})` and `}` lines are gone — no lead-in
/// skipping.
#[tokio::test]
async fn replace_symbol_trusts_lsp_start_with_paren_close() {
    // File layout (0-indexed):
    //  0: "        })"     ← closing `)` of json! macro in the preceding method
    //  1: "    }"          ← closing brace of the preceding method
    //  2: ""               ← blank line
    //  3: "    fn target() {"
    //  4: "        old_body();"
    //  5: "    }"
    let src = "        })\n    }\n\n    fn target() {\n        old_body();\n    }\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(
            file.clone(),
            // LSP reports start_line=0 (the `})` line) — trust LSP, replace from there
            vec![sym("target", 0, 5, file)],
        )
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": "    fn target() {\n        new_body();\n    }"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    // With "trust LSP", lines 0-5 are replaced — `})` and `}` are gone
    assert!(
        result.contains("new_body()"),
        "replacement body must be applied; got:\n{result}"
    );
    assert!(
        !result.contains("old_body()"),
        "old body must be gone; got:\n{result}"
    );
}

/// Normal case: LSP start_line points directly at `fn` — no lead-in to skip.
#[tokio::test]
async fn replace_symbol_clean_start_line() {
    // File layout (0-indexed):
    //  0: "fn foo() {"
    //  1: "    old();"
    //  2: "}"
    let src = "fn foo() {\n    old();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("foo", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "foo",
                "action": "replace",
                "body": "fn foo() {\n    new();\n}"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("new()"),
        "replacement must apply; got:\n{result}"
    );
    assert!(
        !result.contains("old()"),
        "old body must be gone; got:\n{result}"
    );
}
// ── BUG-018: replace_symbol truncated end_line (inside body, misses closing `}`) ──

/// When LSP reports an end_line that lands inside the function body instead of
/// at the closing `}`, trusting that range causes replace_symbol to splice only
/// the first N lines and leave the tail of the old body in the file — stray
/// tokens, compilation failure, silent corruption.
///
/// validate_symbol_range must catch `end_line < AST end_line` and return
/// RecoverableError before touching the file. Regression test for BUG-018.
#[tokio::test]
async fn replace_symbol_rejects_truncated_end_line() {
    // File layout (0-indexed):
    //  0: "fn target() {"       ← LSP start=0 (correct)
    //  1: "    old_body();"     ← LSP end=1   (WRONG — truncated, misses `}`)
    //  2: "}"                   ← actual end=2, not covered by LSP range
    let src = "fn target() {\n    old_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(
            file.clone(),
            // end_line=1 is inside the body — truncated off-by-one (BUG-018 pattern)
            vec![sym("target", 0, 1, file)],
        )
    })
    .await;

    let err = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": "fn target() {\n    new_body();\n}"
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("suspicious range"),
        "expected suspicious range error, got: {msg}"
    );

    // File must be untouched — truncated splice would have left a stray `}`
    let content = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        content.contains("old_body()"),
        "file must be unmodified after truncated-range guard; got:\n{content}"
    );
}

// ── Read/write symmetry: symbols body → replace_symbol round-trip ────────

/// Round-trip: symbols(include_body) → modify → replace_symbol preserves attributes.
/// This is the bug that motivated the full-range body change: symbols returned
/// body from start_line (no attributes), but replace_symbol replaced from
/// editing_start_line (with attributes), consuming #[test] etc.
#[tokio::test]
async fn replace_symbol_round_trip_preserves_attributes() {
    // File layout (0-indexed):
    //  0: "#[test]"                     <- range_start = 0
    //  1: "/// A test function"
    //  2: "fn target() {"               <- selectionRange.start = 2
    //  3: "    old_body();"
    //  4: "}"                           <- end = 4
    let src = "#[test]\n/// A test function\nfn target() {\n    old_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 2, 4, 0, file)])
    })
    .await;

    // Step 1: Read the symbol body (simulates what the agent does)
    let find_result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.rs",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    // The body should include #[test] and /// doc
    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(
        body.contains("#[test]"),
        "symbols body should include attribute; got:\n{body}"
    );

    // Step 2: Agent modifies the body (changes old_body to new_body, keeps attrs)
    let new_body = body.replace("old_body()", "new_body()");

    // Step 3: Replace with the modified body
    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("#[test]"),
        "attribute must be preserved after round-trip; got:\n{result}"
    );
    assert!(
        result.contains("/// A test function"),
        "doc comment must be preserved after round-trip; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
    assert!(
        !result.contains("old_body()"),
        "old body must be gone; got:\n{result}"
    );
}

/// Python: decorators above def are in range_start, docstrings are inside the body.
#[tokio::test]
async fn replace_symbol_round_trip_preserves_python_decorator() {
    // File layout (0-indexed):
    //  0: "@staticmethod"               <- range_start = 0
    //  1: "def target():"               <- selectionRange.start = 1
    //  2: "    old_body()"              <- end = 2
    let src = "@staticmethod\ndef target():\n    old_body()\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.py", src)], |root| {
        let file = root.join("src/lib.py");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 1, 2, 0, file)])
    })
    .await;

    let find_result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.py",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(
        body.contains("@staticmethod"),
        "body should include decorator; got:\n{body}"
    );

    let new_body = body.replace("old_body()", "new_body()");

    EditCode
        .call(
            json!({
                "path": "src/lib.py",
                "symbol": "target",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.py")).unwrap();
    assert!(
        result.contains("@staticmethod"),
        "decorator must survive round-trip; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
}

/// Java: @Override annotation + Javadoc above method.
#[tokio::test]
async fn replace_symbol_round_trip_preserves_java_annotation() {
    // File layout (0-indexed):
    //  0: "/** Javadoc comment */"       <- range_start = 0
    //  1: "@Override"
    //  2: "public void target() {"       <- selectionRange.start = 2
    //  3: "    oldBody();"
    //  4: "}"                            <- end = 4
    let src = "/** Javadoc comment */\n@Override\npublic void target() {\n    oldBody();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/Main.java", src)], |root| {
        let file = root.join("src/Main.java");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 2, 4, 0, file)])
    })
    .await;

    let find_result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/Main.java",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(
        body.contains("@Override"),
        "body should include annotation; got:\n{body}"
    );
    assert!(
        body.contains("/** Javadoc"),
        "body should include Javadoc; got:\n{body}"
    );

    let new_body = body.replace("oldBody()", "newBody()");

    EditCode
        .call(
            json!({
                "path": "src/Main.java",
                "symbol": "target",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/Main.java")).unwrap();
    assert!(
        result.contains("@Override"),
        "annotation must survive; got:\n{result}"
    );
    assert!(
        result.contains("/** Javadoc"),
        "Javadoc must survive; got:\n{result}"
    );
    assert!(
        result.contains("newBody()"),
        "new body applied; got:\n{result}"
    );
}

/// Clean round-trip with no attributes — no regression from the full-range change.
#[tokio::test]
async fn replace_symbol_round_trip_no_attributes() {
    let src = "fn target() {\n    old_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(
            file.clone(),
            // range_start == start — no attributes
            vec![sym_with_range("target", 0, 2, 0, file)],
        )
    })
    .await;

    let find_result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.rs",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    let new_body = body.replace("old_body()", "new_body()");

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
    assert!(
        !result.contains("old_body()"),
        "old body must be gone; got:\n{result}"
    );
    // File should have exactly the same number of lines
    assert_eq!(result.lines().count(), src.lines().count());
}

/// Guard: replace_symbol with body-only code (missing signature) is detected,
/// rejected, and the file is restored automatically.
#[tokio::test]
async fn replace_symbol_rejects_body_only_new_body_and_restores_file() {
    let src = "fn target() {\n    original_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 0, 2, 0, file)])
    })
    .await;

    // Pass body-only code — no `fn target()` signature.
    let body_only = "    new_body();\n";
    let err = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": body_only
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("dropped the symbol definition"),
        "error must mention dropped symbol; got: {msg}"
    );

    // File must be restored to original content.
    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert_eq!(
        result, src,
        "file must be restored to original after rollback"
    );
}

/// BUG-042 extension: body-only rejection must work for NESTED symbols too.
/// The original fix (2026-04-16) counted pre/post symbols at the flat top
/// level of the AST tree. That caught top-level Rust fns — and Rust `impl`
/// methods because `extract_rust_symbols` flattens them to top level — but
/// MISSED languages where class members stay in the `children` array:
/// Java, Kotlin, Python, TypeScript. This test uses Java, whose parser keeps
/// methods nested under the class.
#[tokio::test]
async fn replace_symbol_rejects_body_only_for_nested_method() {
    let src = "\
class Foo {
    void target() {
        originalBody();
    }
}
";

    let (dir, ctx) = ctx_with_mock(&[("src/Foo.java", src)], |root| {
        let file = root.join("src/Foo.java");
        // LSP reports the method as Foo/target (nested under the class).
        let mut sym = sym_with_range("target", 1, 3, 1, file.clone());
        sym.name_path = "Foo/target".to_string();
        MockLspClient::new().with_symbols(file, vec![sym])
    })
    .await;

    // Pass body-only code — no `void target()` signature.
    let body_only = "        newBody();\n";
    let err = EditCode
        .call(
            json!({
                "path": "src/Foo.java",
                "symbol": "Foo/target",
                "action": "replace",
                "body": body_only
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("dropped the symbol definition"),
        "nested method body-only must be caught; got: {msg}"
    );

    // File must be restored.
    let result = std::fs::read_to_string(dir.path().join("src/Foo.java")).unwrap();
    assert_eq!(
        result, src,
        "file must be restored to original after rollback"
    );
}

/// BUG-044 regression: if the LSP reports a child method's `range.end` as
/// overshooting into a sibling method inside the same `impl` block, the
/// symmetric parent clamp alone does not save us — the overshoot stops at the
/// parent's closer but still eats the sibling. The sibling-drop post-write
/// guard compares AST `name_path` sets pre/post-write and rolls back when a
/// sibling vanishes.
///
/// To keep the test deterministic we use a scenario in which `editing_end_line`
/// cannot correct the overshoot via AST: the LSP reports method names that do
/// not appear in the actual source (e.g. post-rename stale data). This leaves
/// the overshooting LSP `range.end` intact and exercises the sibling-drop
/// rollback path directly.
#[tokio::test]
async fn replace_symbol_rolls_back_when_sibling_method_would_be_dropped() {
    let src = "\
struct Foo;

impl Foo {
    fn alpha(&self) -> i32 {
        1
    }

    fn beta(&self) -> i32 {
        2
    }
}
";
    // Line indices (0-based):
    //   0 struct Foo;
    //   1
    //   2 impl Foo {
    //   3     fn alpha(&self) -> i32 {
    //   4         1
    //   5     }
    //   6
    //   7     fn beta(&self) -> i32 {
    //   8         2
    //   9     }
    //  10 }

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // Stale LSP: reports old names (`a`/`b`) that don't match the real source
        // (`alpha`/`beta`). This defeats AST end-line correction, leaving the
        // LSP-reported `range.end` of 9 (overshoot into beta) in place.
        let a = SymbolInfo {
            name: "a".to_string(),
            name_path: "impl Foo/a".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 3,
            end_line: 9, // overshoot — truthful end is 5
            start_col: 4,
            children: vec![],
            range_start_line: Some(3),
            detail: None,
        };
        let b = SymbolInfo {
            name: "b".to_string(),
            name_path: "impl Foo/b".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 7,
            end_line: 9,
            start_col: 4,
            children: vec![],
            range_start_line: Some(7),
            detail: None,
        };
        let impl_block = SymbolInfo {
            name: "impl Foo".to_string(),
            name_path: "impl Foo".to_string(),
            kind: SymbolKind::Class,
            file: file.clone(),
            start_line: 2,
            end_line: 10,
            start_col: 0,
            children: vec![a, b],
            range_start_line: Some(2),
            detail: None,
        };
        MockLspClient::new().with_symbols(file, vec![impl_block])
    })
    .await;

    let new_body = "    fn a(&self) -> i32 {\n        99\n    }";
    let err = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "impl Foo/a",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("dropped sibling symbols") || msg.contains("overshot"),
        "sibling-drop error expected; got: {msg}"
    );
    assert!(
        msg.contains("Foo/beta") || msg.contains("Foo/alpha"),
        "error must name the dropped sibling(s); got: {msg}"
    );

    // File must be untouched.
    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert_eq!(
        result, src,
        "file must be restored after sibling-drop rollback"
    );
}

/// BUG-041: `textDocument/didChange` is a fire-and-forget notification, so the
/// LSP may still be reindexing when the next `documentSymbol` query arrives.
/// That query returns stale positions and any write based on them corrupts
/// the file. replace_symbol must detect staleness (name not found in the
/// reported range), fire a fresh `did_change`, and retry — the second fetch
/// sees the fresh positions and the write succeeds.
#[tokio::test]
async fn replace_symbol_retries_on_stale_lsp_positions_until_fresh() {
    let src = "\
fn filler1() { one(); }
fn filler2() { two(); }
fn target() {
    original();
}
";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // Stale: LSP reports target at line 0 (filler1's spot). `target` is
        // not in that range, so validate_symbol_position rejects it as stale.
        let stale = vec![sym_with_range("target", 0, 0, 0, file.clone())];
        // Fresh: LSP caught up; target is on line 2.
        let fresh = vec![sym_with_range("target", 2, 4, 2, file.clone())];
        MockLspClient::new().with_symbols_sequence(file, vec![stale, fresh])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": "fn target() {\n    new_body();\n}"
            }),
            &ctx,
        )
        .await
        .expect("retry must recover from a single stale LSP response");

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("new_body()"),
        "edit must apply; got:\n{result}"
    );
    assert!(
        !result.contains("original()"),
        "old body must be gone; got:\n{result}"
    );
}

/// If the LSP keeps returning stale positions across every retry, the tool
/// must surface a RecoverableError — don't silently fall through to a write
/// using stale offsets (which BUG-041 originally did).
#[tokio::test]
async fn replace_symbol_surfaces_stale_error_after_max_retries() {
    let src = "\
fn filler1() { one(); }
fn filler2() { two(); }
fn target() {
    original();
}
";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // Every call returns stale. did_change pops the queue but once a single
        // entry remains it sticks — so retries never see fresh data.
        let stale = vec![sym_with_range("target", 0, 0, 0, file.clone())];
        MockLspClient::new().with_symbols_sequence(file, vec![stale])
    })
    .await;

    let err = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": "fn target() {\n    new_body();\n}"
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("stale"),
        "error must still mention staleness when retries are exhausted; got: {msg}"
    );
}

/// Agent changes the attribute: #[test] → #[tokio::test]
#[tokio::test]
async fn replace_symbol_round_trip_agent_changes_attribute() {
    let src = "#[test]\nfn target() {\n    old_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 1, 3, 0, file)])
    })
    .await;

    let find_result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.rs",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    assert!(body.contains("#[test]"), "body should include attribute");

    // Agent changes the attribute AND the body
    let new_body = body
        .replace("#[test]", "#[tokio::test]")
        .replace("old_body()", "new_body()");

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("#[tokio::test]"),
        "new attribute must be present; got:\n{result}"
    );
    assert!(
        !result.contains("\n#[test]\n"),
        "old attribute must be gone; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
}

/// Agent modifies the doc comment during a refactor.
#[tokio::test]
async fn replace_symbol_round_trip_agent_changes_doc_comment() {
    let src = "/// Old documentation\nfn target() {\n    old_body();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 1, 3, 0, file)])
    })
    .await;

    let find_result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.rs",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let body = find_result["symbols"][0]["body"].as_str().unwrap();
    let new_body = body
        .replace(
            "/// Old documentation",
            "/// Updated documentation\n/// With extra detail",
        )
        .replace("old_body()", "new_body()");

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("/// Updated documentation"),
        "new doc must be present; got:\n{result}"
    );
    assert!(
        result.contains("/// With extra detail"),
        "extra doc line must be present; got:\n{result}"
    );
    assert!(
        !result.contains("/// Old documentation"),
        "old doc must be gone; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "new body must be applied; got:\n{result}"
    );
}

/// R-08 regression: When new_body does NOT contain the doc comment but the
/// symbol has one immediately above, `edit_code(replace)` must preserve the
/// existing doc comment rather than dropping it via the BUG-031 walk-back.
///
/// Surfaced by the edit_code eval (R-08, `replace_doc_adj.rs`). BUG-031's
/// walk-back exists to prevent doc-comment DUPLICATION when the LLM passes
/// a new_body that already contains the doc comment. But when the LLM passes
/// a new_body that intentionally omits the doc comment (e.g. only changing the
/// body), the walk-back dropped the original doc. Fix: detect whether new_body
/// leads with decorators; if not, anchor the replace at the keyword line.
#[tokio::test]
async fn replace_symbol_preserves_doc_when_new_body_has_no_doc_comment() {
    let src = "/// Doc that lives immediately above the target with no blank line.\npub fn documented() -> &'static str {\n    \"before\"\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // range_start_line=1 — rust-analyzer points at the `pub fn` line, skipping the doc.
        MockLspClient::new().with_symbols(
            file.clone(),
            vec![sym_with_range("documented", 1, 3, 1, file)],
        )
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "documented",
                "action": "replace",
                "body": "pub fn documented() -> &'static str {\n    \"after\"\n}",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("/// Doc that lives immediately above"),
        "doc comment must survive replace when new_body omits it; got:\n{result}"
    );
    assert!(
        result.contains("\"after\""),
        "new body must be applied; got:\n{result}"
    );
    assert!(
        !result.contains("\"before\""),
        "old body must be gone; got:\n{result}"
    );
}

#[tokio::test]
async fn insert_code_before_with_range_start_line_inserts_above_attribute() {
    // File layout (0-indexed):
    //  0: "#[test]"                     <- range_start = 0
    //  1: "fn target() {}"              <- selectionRange.start = 1
    let src = "#[test]\nfn target() {}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 1, 1, 0, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "position": "before",
                "action": "insert",
                "body": "// inserted above"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(
        lines[0], "// inserted above",
        "inserted code must be above #[test]; got:\n{result}"
    );
    // insert_code(before) adds a blank separator line after the inserted code
    assert_eq!(
        lines[1], "",
        "blank separator line after inserted code; got:\n{result}"
    );
    assert_eq!(
        lines[2], "#[test]",
        "#[test] must follow separator; got:\n{result}"
    );
    assert!(
        lines[3].contains("fn target()"),
        "fn must follow #[test]; got:\n{result}"
    );
}

/// symbols body_start_line field present and correct in integration context.
#[tokio::test]
async fn symbols_body_start_line_field_with_attributes() {
    let src = "#[test]\n/// doc\nfn target() {\n    body();\n}\n";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 2, 4, 0, file)])
    })
    .await;

    let result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.rs",
                "include_body": true
            }),
            &ctx,
        )
        .await
        .unwrap();

    let sym = &result["symbols"][0];
    // body_start_line = 1 (1-indexed, the #[test] line)
    assert_eq!(
        sym["body_start_line"].as_u64(),
        Some(1),
        "body_start_line should point to attribute line"
    );
    // start_line = 3 (1-indexed, the fn keyword line)
    assert_eq!(
        sym["start_line"].as_u64(),
        Some(3),
        "start_line should point to fn keyword"
    );
    // body should contain both attribute and fn
    let body = sym["body"].as_str().unwrap();
    assert!(
        body.starts_with("#[test]"),
        "body should start with attribute"
    );
}

#[tokio::test]
async fn symbols_no_body_start_line_without_include_body() {
    // Auto-inline (src/tools/symbol/symbols.rs:560) attaches `body` for small results
    // when `include_body` is not passed. Explicit `include_body=false` opts out of
    // auto-inline via the `include_body_explicit.is_some()` branch — that's the
    // contract this test now pins.
    let src = "#[test]\nfn target() {}\n";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("target", 1, 1, 0, file)])
    })
    .await;

    let result = Symbols
        .call(
            json!({
                "symbol": "target",
                "path": "src/lib.rs",
                "include_body": false
            }),
            &ctx,
        )
        .await
        .unwrap();

    let sym = &result["symbols"][0];
    assert!(
        sym.get("body").is_none(),
        "body should not be present with explicit include_body=false"
    );
    assert!(
        sym.get("body_start_line").is_none(),
        "body_start_line should not be present with explicit include_body=false"
    );
}

// ── BUG-010: insert_code "before" must walk past #[attr] and /// doc lines ────

/// `insert_code(position="before")` targeting a struct with a leading doc comment
/// and `#[derive]` attribute must insert the code BEFORE the `///` comment, not
/// between the attribute and the struct declaration.
#[tokio::test]
async fn insert_code_before_walks_past_attributes_and_doc_comments() {
    // File layout (0-indexed):
    //  0: "/// A useful struct."  <- doc comment
    //  1: "#[derive(Clone)]"      <- attribute
    //  2: "pub struct Foo {"      <- LSP start_line points here
    //  3: "    x: u32,"
    //  4: "}"
    //  5: ""
    //  6: "const SENTINEL: &str = \"survives\";"
    let src = "/// A useful struct.\n#[derive(Clone)]\npub struct Foo {\n    x: u32,\n}\n\nconst SENTINEL: &str = \"survives\";\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // LSP reports start_line=2 (struct declaration), not 0 (doc comment) — BUG-010
        MockLspClient::new().with_symbols(file.clone(), vec![sym("Foo", 2, 4, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "Foo",
                "position": "before",
                "action": "insert",
                "body": "const BEFORE: u32 = 1;\n"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    let before_pos = result.find("BEFORE").unwrap();
    let doc_pos = result.find("/// A useful").unwrap();
    let derive_pos = result.find("#[derive").unwrap();
    assert!(
        before_pos < doc_pos,
        "const must be inserted before the doc comment, got:\n{result}"
    );
    assert!(
        before_pos < derive_pos,
        "const must be inserted before #[derive], got:\n{result}"
    );
    assert!(
        result.contains("const SENTINEL"),
        "sentinel must survive; got:\n{result}"
    );
}

// ── insert_code: trust LSP start/end ─────────────────────────────────────────

/// With "trust LSP", start_line=0 means insert_code(before) inserts at line 0.
/// No lead-in skipping — find_insert_before_line starts from sym.start_line directly.
/// The `}` at line 0 is NOT skipped; insertion lands before everything.
#[tokio::test]
async fn insert_code_before_trusts_lsp_start() {
    // File layout (0-indexed):
    //  0: "    }"          ← LSP says start_line=0 — trust it, insert before line 0
    //  1: ""               ← blank line
    //  2: "    fn target() {"
    //  3: "    }"
    let src = "    }\n\n    fn target() {\n    }\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("target", 0, 3, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "position": "before",
                "action": "insert",
                "body": "    // inserted\n"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("// inserted"),
        "insertion must be present; got:\n{result}"
    );
    // With "trust LSP", insertion at sym.start_line=0 lands before the `}`
    let insert_pos = result.find("// inserted").unwrap();
    let brace_pos = result.find("    }").unwrap();
    assert!(
        insert_pos < brace_pos,
        "with trust LSP, insertion at sym.start_line=0 lands before `}}`; got:\n{result}"
    );
}

/// Normal "after" case: symbol at [0,1], insertion goes after line 1.
#[tokio::test]
async fn insert_code_after_lands_past_symbol() {
    let src = "fn foo() {\n}\n\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("foo", 0, 1, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "foo",
                "position": "after",
                "action": "insert",
                "body": "fn bar() {}\n"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("fn foo()"),
        "original must be present; got:\n{result}"
    );
    assert!(
        result.contains("fn bar()"),
        "insertion must be present; got:\n{result}"
    );
    let foo_pos = result.find("fn foo()").unwrap();
    let bar_pos = result.find("fn bar()").unwrap();
    assert!(
        bar_pos > foo_pos,
        "bar must be inserted after foo; got:\n{result}"
    );
}

/// BUG-023 regression: when LSP over-extends end_line to the next function's opening
/// line, editing_end_line() caps it to the AST-reported end, so insertion lands
/// between the closing `}` and the next function — NOT inside the next function body.
#[tokio::test]
async fn insert_code_after_caps_overextended_lsp_end() {
    // File layout (0-indexed):
    //  0: "fn target() {"
    //  1: "    body();"
    //  2: "}"
    //  3: "fn following() {"   ← LSP over-extends target's end_line to here
    //  4: "    inside();"
    //  5: "}"
    let src = "fn target() {\n    body();\n}\nfn following() {\n    inside();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // LSP reports end_line=3 (over-extended into fn following() { line)
        MockLspClient::new().with_symbols(file.clone(), vec![sym("target", 0, 3, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "position": "after",
                "action": "insert",
                "body": "// inserted\n"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("// inserted"),
        "insertion must be present; got:\n{result}"
    );
    // editing_end_line caps to AST end (line 2 = closing `}`),
    // so insertion goes after line 2 — between `}` and `fn following()`.
    let insert_pos = result.find("// inserted").unwrap();
    let following_fn = result.find("fn following()").unwrap();
    assert!(
        insert_pos < following_fn,
        "insertion should land before fn following(), not inside it; got:\n{result}"
    );
}

/// BUG-016 regression: insert_code(after) on a nested `mod tests` function
/// where LSP reports end_line as a line *inside* the body (truncated range).
/// validate_symbol_range catches ast_end > sym.end_line and returns
/// RecoverableError — the file is never corrupted.
#[tokio::test]
async fn insert_code_after_rejects_truncated_end_in_nested_fn() {
    // File layout (0-indexed):
    //  0: "#[cfg(test)]"
    //  1: "mod tests {"
    //  2: "    #[test]"
    //  3: "    fn target_test() {"
    //  4: "        let x = 1;"         <- LSP (wrongly) reports end_line here
    //  5: "        assert_eq!(x, 1);"
    //  6: "    }"                       <- true end (AST knows this)
    //  7: "}"
    let src =
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn target_test() {\n        let x = 1;\n        assert_eq!(x, 1);\n    }\n}\n";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // LSP reports end_line=4 (inside the body), not 6 (closing `}`)
        let inner = SymbolInfo {
            name: "target_test".to_string(),
            name_path: "tests/target_test".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 3,
            end_line: 4, // truncated — true end is line 6
            start_col: 4,
            children: vec![],
            range_start_line: None,
            detail: None,
        };
        let module = SymbolInfo {
            name: "tests".to_string(),
            name_path: "tests".to_string(),
            kind: SymbolKind::Module,
            file: file.clone(),
            start_line: 1,
            end_line: 7,
            start_col: 0,
            children: vec![inner],
            range_start_line: None,
            detail: None,
        };
        MockLspClient::new().with_symbols(file, vec![module])
    })
    .await;

    let result = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "tests/target_test",
                "position": "after",
                "action": "insert",
                "body": "    #[test]\n    fn new_test() {}\n"
            }),
            &ctx,
        )
        .await;

    // validate_symbol_range must catch ast_end (6) > sym.end_line (4)
    // and return RecoverableError — not silently insert mid-body
    let err = result.expect_err("should fail with RecoverableError for truncated end_line");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("suspicious range"),
        "error should mention suspicious range; got: {msg}"
    );
}

/// Regression for the strict-refuse path on `insert_code(position="after")`.
///
/// Scenario: LSP reports a stale name (`a` instead of the AST's `alpha`) so
/// `editing_end_line_strict` returns None. Before fix `201dcb5b`, a parented
/// symbol would silently fall back to LSP's overshoot and then rely on the
/// parent clamp — but the clamp only catches *over*-extension, not
/// *under*-extension into the same body. The fix removes that fallback;
/// insert-after now refuses with a RecoverableError naming the workarounds.
#[tokio::test]
async fn insert_code_after_refuses_when_ast_cannot_pin_symbol_end() {
    let src = "\
struct Foo;

impl Foo {
    fn alpha(&self) {}
}
";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        let alpha = SymbolInfo {
            name: "a".to_string(),
            name_path: "impl Foo/a".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 3,
            end_line: 10,
            start_col: 4,
            children: vec![],
            range_start_line: Some(3),
            detail: None,
        };
        let impl_block = SymbolInfo {
            name: "impl Foo".to_string(),
            name_path: "impl Foo".to_string(),
            kind: SymbolKind::Class,
            file: file.clone(),
            start_line: 2,
            end_line: 4,
            start_col: 0,
            children: vec![alpha],
            range_start_line: Some(2),
            detail: None,
        };
        MockLspClient::new().with_symbols(file, vec![impl_block])
    })
    .await;

    let err = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "impl Foo/a",
                "position": "after",
                "action": "insert",
                "body": "    fn beta(&self) {}\n"
            }),
            &ctx,
        )
        .await
        .expect_err("insert-after must refuse when AST cannot pin the symbol's end");

    let msg = err.to_string();
    assert!(
        msg.contains("cannot determine end") && msg.contains("AST parse failed"),
        "error must explain why it refused; got: {msg}"
    );

    let unchanged = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert_eq!(
        unchanged, src,
        "refused insert-after must leave the file unchanged"
    );
}

/// BUG-051 residual: a top-level symbol with no parent has no clamp safety net,
/// so when AST cannot pinpoint its end (broken parse, ambiguous match, etc.)
/// `do_insert` "after" must refuse rather than fall back to LSP's
/// possibly-corrupted `end_line` and risk splicing new code mid-function.
#[tokio::test]
async fn insert_code_after_refuses_when_ast_fails_and_no_parent_clamp() {
    let src = "\
fn alpha() {
    println!(\"hello\");
}
";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // Top-level fn with a name AST can't match. No parent in the symbol
        // tree, so the parent clamp cannot recover.
        let alpha = SymbolInfo {
            name: "a".to_string(),
            name_path: "a".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 0,
            end_line: 1, // suspiciously short — under-extension scenario
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        MockLspClient::new().with_symbols(file, vec![alpha])
    })
    .await;

    let result = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "a",
                "position": "after",
                "action": "insert",
                "body": "fn beta() {}\n"
            }),
            &ctx,
        )
        .await;

    let err = result.expect_err("must refuse when AST fails and no parent clamp is available");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("AST parse failed") || msg.contains("cannot determine end"),
        "error should explain AST failure; got: {msg}"
    );
}

// ── remove_symbol: trust LSP ranges ──────────────────────────────────────────

/// BUG-024 regression (remove_symbol): when LSP over-extends `end_line` to include a
/// sibling `const`, editing_end_line() caps to the AST-reported end so the const survives.
#[tokio::test]
async fn remove_symbol_caps_overextended_lsp_end() {
    // File layout (0-indexed):
    //  0: "fn target() {"
    //  1: "    // body"
    //  2: "}"
    //  3: "const SENTINEL: &str = \"survives\";"  <- LSP over-extends end_line here
    let src = "fn target() {\n    // body\n}\nconst SENTINEL: &str = \"survives\";\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // LSP reports end_line=3 (over-extended to const line — true end is line 2)
        MockLspClient::new().with_symbols(file.clone(), vec![sym("target", 0, 3, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "remove"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        !result.contains("fn target"),
        "function must be removed; got:\n{result}"
    );
    // editing_end_line caps to AST end (line 2) — SENTINEL is outside the range and survives
    assert!(
        result.contains("SENTINEL"),
        "SENTINEL must survive — it is outside the true symbol range; got:\n{result}"
    );
}

/// When `range_start_line` is set (documentSymbol path), remove_symbol uses it
/// to include attributes and doc comments in the removal range.
#[tokio::test]
async fn remove_symbol_uses_range_start_line_to_include_doc_comment() {
    // File layout (0-indexed):
    //  0: "fn preceding() {"
    //  1: "    // body"
    //  2: "}"
    //  3: "use std::fmt;"
    //  4: ""                                   <- blank line
    //  5: "/// A constant."                    <- range.start = 5
    //  6: "const TARGET: bool = false;"        <- selectionRange.start = 6, end = 6
    //  7: "fn following() {}"
    let src = "fn preceding() {\n    // body\n}\nuse std::fmt;\n\n/// A constant.\nconst TARGET: bool = false;\nfn following() {}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // range_start=5 includes the doc comment
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("TARGET", 6, 6, 5, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "TARGET",
                "action": "remove"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        !result.contains("TARGET"),
        "const must be removed; got:\n{result}"
    );
    assert!(
        !result.contains("A constant"),
        "doc comment included in range_start_line — must also be removed; got:\n{result}"
    );
    assert!(
        result.contains("fn preceding()"),
        "preceding function must survive; got:\n{result}"
    );
    assert!(
        result.contains("fn following()"),
        "following function must survive; got:\n{result}"
    );
    let use_count = result.matches("use std::fmt;").count();
    assert_eq!(
        use_count, 1,
        "use import must not be duplicated; found {use_count} occurrences in:\n{result}"
    );
}

/// When `range_start_line` is `None` (workspace/symbol or tree-sitter), the
/// heuristic fallback walks backwards past doc comments and attributes.
#[tokio::test]
async fn remove_symbol_heuristic_fallback_includes_doc_comment() {
    let src = "fn preceding() {\n    // body\n}\nuse std::fmt;\n\n/// A constant.\nconst TARGET: bool = false;\nfn following() {}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // No range_start_line — triggers heuristic fallback
        MockLspClient::new().with_symbols(file.clone(), vec![sym("TARGET", 6, 6, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "TARGET",
                "action": "remove"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        !result.contains("TARGET"),
        "const must be removed; got:\n{result}"
    );
    assert!(
        !result.contains("A constant"),
        "heuristic should walk back past doc comment; got:\n{result}"
    );
    assert!(
        result.contains("fn preceding()"),
        "preceding function must survive; got:\n{result}"
    );
    assert!(
        result.contains("fn following()"),
        "following function must survive; got:\n{result}"
    );
}

/// When `range_start_line` explicitly excludes the doc comment, but doc comments
/// exist directly above, editing_start_line walks back to include them (BUG-031 fix).
/// Orphaned doc comments after symbol removal are worse than removing them.
#[tokio::test]
async fn remove_symbol_range_start_line_excludes_doc_comment() {
    let src = "fn preceding() {\n    // body\n}\nuse std::fmt;\n\n/// A constant.\nconst TARGET: bool = false;\nfn following() {}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // range_start=6 (same as selectionRange) — LSP explicitly says no doc comment
        MockLspClient::new()
            .with_symbols(file.clone(), vec![sym_with_range("TARGET", 6, 6, 6, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "TARGET",
                "action": "remove"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        !result.contains("TARGET"),
        "const must be removed; got:\n{result}"
    );
    // BUG-031 fix: editing_start_line now walks back past `///` doc comments
    // even when range_start_line points to the keyword line. This prevents
    // orphaned doc comments and fixes replace_symbol duplication.
    assert!(
        !result.contains("A constant"),
        "doc comment should also be removed (BUG-031 fix); got:\n{result}"
    );
}

// ── symbols: name_path exact match (BUG-011) ──────────────────────────────

/// Searching by name_path must return only the exact symbol, not child symbols
/// whose name_path happens to contain the query as a substring.
///
/// Regression for BUG-011: `collect_matching` used `contains()`, so a Variable
/// child with name_path "my_fn/local_var" matched a query for "my_fn".
#[tokio::test]
async fn symbols_name_path_does_not_return_local_variable_children() {
    use codescout::lsp::SymbolKind;

    let src = "fn my_fn() {\n    let local_var = 1;\n}\n";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        let child = SymbolInfo {
            name: "local_var".to_string(),
            name_path: "my_fn/local_var".to_string(),
            kind: SymbolKind::Variable,
            file: file.clone(),
            start_line: 1,
            end_line: 1,
            start_col: 4,
            children: vec![],
            range_start_line: None,
            detail: None,
        };
        let parent = SymbolInfo {
            name: "my_fn".to_string(),
            name_path: "my_fn".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 0,
            end_line: 2,
            start_col: 0,
            children: vec![child],
            range_start_line: None,
            detail: None,
        };
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    let result = Symbols
        .call(
            json!({
                "symbol": "my_fn",
                "path": "src/lib.rs"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert_eq!(
        symbols.len(),
        1,
        "name_path lookup must return exactly the matching symbol, not its Variable children; got: {symbols:?}"
    );
    assert_eq!(symbols[0]["name"], "my_fn");
}

// ── symbol_at def: no-identifier fallback (BUG-012) ───────────────────────────

/// When `identifier` is omitted, `symbol_at` (def) must use the first
/// non-whitespace column of the line, not error with "identifier not found".
///
/// Regression for BUG-012: the old code called `str::find(ident)` which returned
/// `None` for nearly every call, causing a 100% error rate.
#[tokio::test]
async fn symbol_at_def_unknown_identifier_falls_back_to_first_nonwhitespace() {
    // Line 0 (1-indexed: line 1) has 4 spaces of indent before "let".
    // First non-whitespace column = 4.
    let src = "    let foo = 1;\n";

    let (_dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let def_path = root.join("src/lib.rs");
        // Configure a definition at exactly (line=0, col=4) — the expected column.
        // If the tool uses any other column (e.g. 0), the mock returns [] and
        // the tool fails with "no definition found" instead of the expected result.
        MockLspClient::new().with_definitions(
            0,
            4,
            vec![lsp_types::Location {
                uri: url::Url::from_file_path(&def_path)
                    .unwrap()
                    .as_str()
                    .parse()
                    .unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 0,
                        character: 4,
                    },
                    end: lsp_types::Position {
                        line: 0,
                        character: 7,
                    },
                },
            }],
        )
    })
    .await;

    let result = SymbolAt
        .call(
            json!({
                "path": "src/lib.rs",
                "line": 1,
                "fields": ["def"]
                // no "identifier" — must fall back to first-nonwhitespace column
            }),
            &ctx,
        )
        .await
        .expect("should succeed: omitting identifier must not cause 'identifier not found'");

    let defs = result["def"]["definitions"].as_array().unwrap();
    assert_eq!(
        defs.len(),
        1,
        "mock should return the pre-configured definition at col=4; \
         if col is wrong the mock returns [] and the tool errors instead"
    );
}

// ── replace_symbol: language-agnostic (BUG-019 regression) ───────────────────
//
// The old `is_valid_symbol_start_line` guard had a Rust-only keyword allowlist
// (`fn `, `pub `, `struct `, `impl `, …). Any LSP start_line whose content did not
// match was rejected as "symbol location appears stale", breaking replace_symbol
// for every non-Rust language.
//
// Fix: the guard was removed. `validate_symbol_range` (AST cross-check) is the
// canonical staleness defense and is language-agnostic.
//
// Each test below is a sandwich regression:
//   Baseline  — Rust `fn` continues to work (covered by replace_symbol_clean_start_line).
//   Stale     — the language's function keyword was NOT in the old Rust allowlist, so
//               is_valid_symbol_start_line would have returned false and EditCode
//               would have returned Err("symbol location appears stale").
//   Fixed     — ReplaceSymbol now succeeds and the new body appears in the file.

/// Python: `def` was not in the Rust keyword allowlist → old code rejected it.
#[tokio::test]
async fn replace_symbol_works_for_python() {
    // 0: "def greet():"
    // 1: "    return 'old'"
    let src = "def greet():\n    return 'old'\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.py", src)], |root| {
        let file = root.join("greet.py");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 1, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.py", "symbol": "greet",
                    "action": "replace",
                "body": "def greet():\n    return 'new'" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.py")).unwrap();
    assert!(
        result.contains("'new'"),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("'old'"),
        "old body must be gone; got:\n{result}"
    );
}

/// TypeScript: `function` was not in the Rust keyword allowlist → old code rejected it.
#[tokio::test]
async fn replace_symbol_works_for_typescript() {
    // 0: "function greet(): string {"
    // 1: "    return 'old';"
    // 2: "}"
    let src = "function greet(): string {\n    return 'old';\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.ts", src)], |root| {
        let file = root.join("greet.ts");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.ts", "symbol": "greet",
                    "action": "replace",
                "body": "function greet(): string {\n    return 'new';\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.ts")).unwrap();
    assert!(
        result.contains("'new'"),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("'old'"),
        "old body must be gone; got:\n{result}"
    );
}

/// JavaScript: `function` keyword → same Rust-allowlist rejection as TypeScript.
#[tokio::test]
async fn replace_symbol_works_for_javascript() {
    let src = "function greet() {\n    return 'old';\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.js", src)], |root| {
        let file = root.join("greet.js");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.js", "symbol": "greet",
                    "action": "replace",
                "body": "function greet() {\n    return 'new';\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.js")).unwrap();
    assert!(
        result.contains("'new'"),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("'old'"),
        "old body must be gone; got:\n{result}"
    );
}

/// Go: `func` was not in the Rust keyword allowlist → old code rejected it.
#[tokio::test]
async fn replace_symbol_works_for_go() {
    let src = "func Greet() string {\n\treturn \"old\"\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.go", src)], |root| {
        let file = root.join("greet.go");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("Greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.go", "symbol": "Greet",
                    "action": "replace",
                "body": "func Greet() string {\n\treturn \"new\"\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.go")).unwrap();
    assert!(
        result.contains("\"new\""),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("\"old\""),
        "old body must be gone; got:\n{result}"
    );
}

/// Java: `public` at the start of a method was not in the allowlist → rejected.
/// (Note: `pub ` and `pub(` were in the allowlist but not `public `.)
#[tokio::test]
async fn replace_symbol_works_for_java() {
    let src = "public String greet() {\n    return \"old\";\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("Greet.java", src)], |root| {
        let file = root.join("Greet.java");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "Greet.java", "symbol": "greet",
                    "action": "replace",
                "body": "public String greet() {\n    return \"new\";\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("Greet.java")).unwrap();
    assert!(
        result.contains("\"new\""),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("\"old\""),
        "old body must be gone; got:\n{result}"
    );
}

/// Kotlin: `fun` was not in the Rust keyword allowlist → old code rejected it.
#[tokio::test]
async fn replace_symbol_works_for_kotlin() {
    let src = "fun greet(): String {\n    return \"old\"\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("Greet.kt", src)], |root| {
        let file = root.join("Greet.kt");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "Greet.kt", "symbol": "greet",
                    "action": "replace",
                "body": "fun greet(): String {\n    return \"new\"\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("Greet.kt")).unwrap();
    assert!(
        result.contains("\"new\""),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("\"old\""),
        "old body must be gone; got:\n{result}"
    );
}

/// C: return-type-first signatures were not in the Rust keyword allowlist → rejected.
#[tokio::test]
async fn replace_symbol_works_for_c() {
    let src = "int greet() {\n    return 0;\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.c", src)], |root| {
        let file = root.join("greet.c");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.c", "symbol": "greet",
                    "action": "replace",
                "body": "int greet() {\n    return 1;\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.c")).unwrap();
    assert!(
        result.contains("return 1"),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("return 0"),
        "old body must be gone; got:\n{result}"
    );
}

/// C++: same as C — return-type-first → rejected by old allowlist.
#[tokio::test]
async fn replace_symbol_works_for_cpp() {
    let src = "std::string greet() {\n    return \"old\";\n}\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.cpp", src)], |root| {
        let file = root.join("greet.cpp");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.cpp", "symbol": "greet",
                    "action": "replace",
                "body": "std::string greet() {\n    return \"new\";\n}" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.cpp")).unwrap();
    assert!(
        result.contains("\"new\""),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("\"old\""),
        "old body must be gone; got:\n{result}"
    );
}

/// Ruby: `def` (without parens) was not in the Rust keyword allowlist → rejected.
#[tokio::test]
async fn replace_symbol_works_for_ruby() {
    // Ruby methods end with `end`, not `}`
    // 0: "def greet"
    // 1: "  'old'"
    // 2: "end"
    let src = "def greet\n  'old'\nend\n";
    let (dir, ctx) = ctx_with_mock(&[("greet.rb", src)], |root| {
        let file = root.join("greet.rb");
        MockLspClient::new().with_symbols(file.clone(), vec![sym("greet", 0, 2, file)])
    })
    .await;

    EditCode
        .call(
            json!({ "path": "greet.rb", "symbol": "greet",
                    "action": "replace",
                "body": "def greet\n  'new'\nend" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("greet.rb")).unwrap();
    assert!(
        result.contains("'new'"),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("'old'"),
        "old body must be gone; got:\n{result}"
    );
}

/// BUG-024 regression: replace_symbol with over-extended LSP end range must not
/// consume the next function's opening line.
#[tokio::test]
async fn replace_symbol_caps_overextended_lsp_end() {
    // File layout (0-indexed):
    //  0: "fn target() {"
    //  1: "    old_body();"
    //  2: "}"
    //  3: "fn following() {"   <- LSP over-extends target's end_line to here
    //  4: "    inside();"
    //  5: "}"
    let src = "fn target() {\n    old_body();\n}\nfn following() {\n    inside();\n}\n";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // LSP reports end_line=3 (over-extended — true end is line 2)
        MockLspClient::new().with_symbols(file.clone(), vec![sym("target", 0, 3, file)])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "target",
                "action": "replace",
                "body": "fn target() {\n    new_body();\n}"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("fn following()"),
        "fn following() must still be present; got:\n{result}"
    );
    assert!(
        result.contains("new_body()"),
        "replacement body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("old_body()"),
        "old body must be gone; got:\n{result}"
    );
    // fn following() must appear after the replacement, not be eaten by it
    let replaced_pos = result.find("new_body()").unwrap();
    let following_pos = result.find("fn following()").unwrap();
    assert!(
        following_pos > replaced_pos,
        "fn following() must come after replacement; got:\n{result}"
    );
}

/// BUG-034 reproduction: replace_symbol on the first child inside `mod tests`
/// must NOT eat the parent's `#[cfg(test)]\nmod tests {` header, even when the
/// LSP reports a stale `range_start_line` that points to the parent's attribute.
#[tokio::test]
async fn replace_symbol_child_in_mod_tests_preserves_module_header() {
    // File layout (0-indexed):
    //  0: "#[cfg(test)]"               <- parent range_start = 0
    //  1: "mod tests {"                <- parent start_line = 1
    //  2: "    #[test]"                <- child range_start SHOULD be 2, but stale LSP says 0
    //  3: "    fn first_test() {"      <- child start_line = 3
    //  4: "        assert!(true);"
    //  5: "    }"                      <- child end = 5
    //  6: ""
    //  7: "    #[test]"
    //  8: "    fn second_test() {"
    //  9: "        assert!(false);"
    // 10: "    }"
    // 11: "}"
    let src = "\
#[cfg(test)]
mod tests {
    #[test]
    fn first_test() {
        assert!(true);
    }

    #[test]
    fn second_test() {
        assert!(false);
    }
}
";

    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        // Parent: mod tests — range starts at #[cfg(test)] (line 0), keyword at line 1
        let mut parent = SymbolInfo {
            name: "tests".to_string(),
            name_path: "tests".to_string(),
            kind: SymbolKind::Module,
            file: file.clone(),
            start_line: 1,
            end_line: 11,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        // Child: first_test — stale LSP reports range_start_line = 0 (#[cfg(test)])
        // instead of the correct 2 (#[test])
        let child1 = SymbolInfo {
            name: "first_test".to_string(),
            name_path: "tests/first_test".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 3,
            end_line: 5,
            start_col: 4,
            children: vec![],
            range_start_line: Some(0), // BUG: stale, points to parent's #[cfg(test)]
            detail: None,
        };
        let child2 = SymbolInfo {
            name: "second_test".to_string(),
            name_path: "tests/second_test".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 8,
            end_line: 10,
            start_col: 4,
            children: vec![],
            range_start_line: Some(7),
            detail: None,
        };
        parent.children = vec![child1, child2];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    // Replace first_test with a new body
    let new_body = "    #[test]\n    fn first_test() {\n        assert_eq!(1, 1);\n    }";
    let result = EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "tests/first_test",
                "action": "replace",
                "body": new_body,
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Verify the module header is preserved
    let content = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        content.contains("#[cfg(test)]"),
        "BUG-034: #[cfg(test)] must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("mod tests {"),
        "BUG-034: mod tests {{ must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("assert_eq!(1, 1)"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        content.contains("second_test"),
        "second_test must be preserved; got:\n{content}"
    );

    // Verify replaced_lines doesn't extend into module header
    let replaced = result["replaced_lines"].as_str().unwrap();
    let start_line: usize = replaced.split('-').next().unwrap().parse().unwrap();
    assert!(
        start_line >= 3, // 1-indexed: line 3 = #[test] for first_test
        "BUG-034: replaced_lines should start at or after #[test] (line 3), got: {replaced}"
    );
}

// ── BUG-034 guard: cross-language integration tests ──────────────────────────

/// BUG-034 guard: Rust child in `impl` block with stale range_start_line.
/// The guard must prevent eating `impl Foo {` when the child's range is stale.
#[tokio::test]
async fn bug034_guard_rust_child_in_impl_block_stale_range() {
    let src = "\
impl Foo {
    /// Does something.
    fn method(&self) {
        old_body();
    }

    fn other(&self) {}
}
";
    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        let mut parent = SymbolInfo {
            name: "Foo".to_string(),
            name_path: "Foo".to_string(),
            kind: SymbolKind::Object,
            file: file.clone(),
            start_line: 0,
            end_line: 7,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        // Stale: range_start_line=0 (impl Foo line) instead of correct 1 (/// doc)
        let child1 = SymbolInfo {
            name: "method".to_string(),
            name_path: "Foo/method".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 2,
            end_line: 4,
            start_col: 4,
            children: vec![],
            range_start_line: Some(0), // stale — points to parent's impl line
            detail: None,
        };
        let child2 = SymbolInfo {
            name: "other".to_string(),
            name_path: "Foo/other".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 6,
            end_line: 6,
            start_col: 4,
            children: vec![],
            range_start_line: Some(6),
            detail: None,
        };
        parent.children = vec![child1, child2];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "Foo/method",
                "action": "replace",
                "body": "    /// Does something.\n    fn method(&self) {\n        new_body();\n    }",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        content.contains("impl Foo {"),
        "impl Foo {{ must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("new_body()"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        content.contains("fn other"),
        "sibling must be preserved; got:\n{content}"
    );
}

/// BUG-034 guard: Rust child in `impl` with CORRECT range — verify no over-clamping.
#[tokio::test]
async fn bug034_guard_rust_impl_correct_range_no_overclamping() {
    let src = "\
impl Foo {
    /// A doc comment.
    #[inline]
    fn method(&self) {
        old_body();
    }
}
";
    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        let mut parent = SymbolInfo {
            name: "Foo".to_string(),
            name_path: "Foo".to_string(),
            kind: SymbolKind::Object,
            file: file.clone(),
            start_line: 0,
            end_line: 6,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        // Correct range: points to doc comment
        let child = SymbolInfo {
            name: "method".to_string(),
            name_path: "Foo/method".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 3,
            end_line: 5,
            start_col: 4,
            children: vec![],
            range_start_line: Some(1), // correct — points to `/// A doc comment.`
            detail: None,
        };
        parent.children = vec![child];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "Foo/method",
                "action": "replace",
                "body": "    /// Updated doc.\n    #[inline]\n    fn method(&self) {\n        new_body();\n    }",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        content.contains("impl Foo {"),
        "impl header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("Updated doc"),
        "new doc comment must be present; got:\n{content}"
    );
    assert!(
        !content.contains("A doc comment"),
        "old doc comment must be replaced; got:\n{content}"
    );
}

/// BUG-034 guard: Python decorated method in class with stale range.
#[tokio::test]
async fn bug034_guard_python_decorated_method_stale_range() {
    let src = "\
class MyService:
    @staticmethod
    def handle(request):
        return old_response()

    def other(self):
        pass
";
    let (dir, ctx) = ctx_with_mock(&[("service.py", src)], |root| {
        let file = root.join("service.py");
        let mut parent = SymbolInfo {
            name: "MyService".to_string(),
            name_path: "MyService".to_string(),
            kind: SymbolKind::Class,
            file: file.clone(),
            start_line: 0,
            end_line: 6,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        // Stale: range_start_line=0 (class line) instead of correct 1 (@staticmethod)
        let child1 = SymbolInfo {
            name: "handle".to_string(),
            name_path: "MyService/handle".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 2,
            end_line: 3,
            start_col: 4,
            children: vec![],
            range_start_line: Some(0), // stale
            detail: None,
        };
        let child2 = SymbolInfo {
            name: "other".to_string(),
            name_path: "MyService/other".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 5,
            end_line: 6,
            start_col: 4,
            children: vec![],
            range_start_line: Some(5),
            detail: None,
        };
        parent.children = vec![child1, child2];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "service.py",
                "symbol": "MyService/handle",
                "action": "replace",
                "body": "    @staticmethod\n    def handle(request):\n        return new_response()",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("service.py")).unwrap();
    assert!(
        content.contains("class MyService:"),
        "class header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("new_response()"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        content.contains("def other"),
        "sibling must be preserved; got:\n{content}"
    );
}

/// BUG-034 guard: TypeScript method in class with stale range.
#[tokio::test]
async fn bug034_guard_typescript_method_stale_range() {
    let src = "\
export class UserService {
    private validate(input: string): boolean {
        return false;
    }

    public greet(): string {
        return 'hello';
    }
}
";
    let (dir, ctx) = ctx_with_mock(&[("service.ts", src)], |root| {
        let file = root.join("service.ts");
        let mut parent = SymbolInfo {
            name: "UserService".to_string(),
            name_path: "UserService".to_string(),
            kind: SymbolKind::Class,
            file: file.clone(),
            start_line: 0,
            end_line: 8,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        let child1 = SymbolInfo {
            name: "validate".to_string(),
            name_path: "UserService/validate".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 1,
            end_line: 3,
            start_col: 4,
            children: vec![],
            range_start_line: Some(0), // stale — points to class declaration
            detail: None,
        };
        let child2 = SymbolInfo {
            name: "greet".to_string(),
            name_path: "UserService/greet".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 5,
            end_line: 7,
            start_col: 4,
            children: vec![],
            range_start_line: Some(5),
            detail: None,
        };
        parent.children = vec![child1, child2];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "service.ts",
                "symbol": "UserService/validate",
                "action": "replace",
                "body": "    private validate(input: string): boolean {\n        return true;\n    }",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("service.ts")).unwrap();
    assert!(
        content.contains("export class UserService {"),
        "class header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("return true"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        content.contains("public greet"),
        "sibling must be preserved; got:\n{content}"
    );
}

/// BUG-034 guard: Java annotated method in class with stale range.
#[tokio::test]
async fn bug034_guard_java_annotated_method_stale_range() {
    let src = "\
public class Handler {
    @Override
    public void process(Request req) {
        oldLogic();
    }

    public void other() {}
}
";
    let (dir, ctx) = ctx_with_mock(&[("Handler.java", src)], |root| {
        let file = root.join("Handler.java");
        let mut parent = SymbolInfo {
            name: "Handler".to_string(),
            name_path: "Handler".to_string(),
            kind: SymbolKind::Class,
            file: file.clone(),
            start_line: 0,
            end_line: 7,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        let child1 = SymbolInfo {
            name: "process".to_string(),
            name_path: "Handler/process".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 2,
            end_line: 4,
            start_col: 4,
            children: vec![],
            range_start_line: Some(0), // stale — points to class declaration
            detail: None,
        };
        let child2 = SymbolInfo {
            name: "other".to_string(),
            name_path: "Handler/other".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 6,
            end_line: 6,
            start_col: 4,
            children: vec![],
            range_start_line: Some(6),
            detail: None,
        };
        parent.children = vec![child1, child2];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "Handler.java",
                "symbol": "Handler/process",
                "action": "replace",
                "body": "    @Override\n    public void process(Request req) {\n        newLogic();\n    }",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("Handler.java")).unwrap();
    assert!(
        content.contains("public class Handler {"),
        "class header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("newLogic()"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        content.contains("public void other"),
        "sibling must be preserved; got:\n{content}"
    );
}

/// BUG-034 guard: Kotlin annotated method in class with stale range.
#[tokio::test]
async fn bug034_guard_kotlin_annotated_method_stale_range() {
    let src = "\
class Repository {
    @Throws(IOException::class)
    fun load(id: String): Data {
        return oldLoad(id)
    }

    fun save(data: Data) {}
}
";
    let (dir, ctx) = ctx_with_mock(&[("Repository.kt", src)], |root| {
        let file = root.join("Repository.kt");
        let mut parent = SymbolInfo {
            name: "Repository".to_string(),
            name_path: "Repository".to_string(),
            kind: SymbolKind::Class,
            file: file.clone(),
            start_line: 0,
            end_line: 7,
            start_col: 0,
            children: vec![],
            range_start_line: Some(0),
            detail: None,
        };
        let child1 = SymbolInfo {
            name: "load".to_string(),
            name_path: "Repository/load".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 2,
            end_line: 4,
            start_col: 4,
            children: vec![],
            range_start_line: Some(0), // stale
            detail: None,
        };
        let child2 = SymbolInfo {
            name: "save".to_string(),
            name_path: "Repository/save".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 6,
            end_line: 6,
            start_col: 4,
            children: vec![],
            range_start_line: Some(6),
            detail: None,
        };
        parent.children = vec![child1, child2];
        MockLspClient::new().with_symbols(file, vec![parent])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "Repository.kt",
                "symbol": "Repository/load",
                "action": "replace",
                "body": "    @Throws(IOException::class)\n    fun load(id: String): Data {\n        return newLoad(id)\n    }",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("Repository.kt")).unwrap();
    assert!(
        content.contains("class Repository {"),
        "class header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("newLoad(id)"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        content.contains("fun save"),
        "sibling must be preserved; got:\n{content}"
    );
}

/// BUG-034 guard: deeply nested Rust — fn inside impl inside mod.
/// The guard should find the IMMEDIATE parent (impl), not the grandparent (mod).
#[tokio::test]
async fn bug034_guard_rust_deeply_nested_fn_in_impl_in_mod() {
    let src = "\
mod inner {
    pub struct Bar;

    impl Bar {
        pub fn do_thing(&self) {
            old();
        }
    }
}
";
    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        let method = SymbolInfo {
            name: "do_thing".to_string(),
            name_path: "inner/Bar/do_thing".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 4,
            end_line: 6,
            start_col: 8,
            children: vec![],
            range_start_line: Some(0), // extremely stale — points to `mod inner`
            detail: None,
        };
        let impl_block = SymbolInfo {
            name: "Bar".to_string(),
            name_path: "inner/Bar".to_string(),
            kind: SymbolKind::Object,
            file: file.clone(),
            start_line: 3,
            end_line: 7,
            start_col: 4,
            children: vec![method],
            range_start_line: Some(3),
            detail: None,
        };
        let struct_sym = SymbolInfo {
            name: "Bar".to_string(),
            name_path: "inner/Bar".to_string(),
            kind: SymbolKind::Struct,
            file: file.clone(),
            start_line: 1,
            end_line: 1,
            start_col: 4,
            children: vec![],
            range_start_line: Some(1),
            detail: None,
        };
        let module = SymbolInfo {
            name: "inner".to_string(),
            name_path: "inner".to_string(),
            kind: SymbolKind::Module,
            file: file.clone(),
            start_line: 0,
            end_line: 8,
            start_col: 0,
            children: vec![struct_sym, impl_block],
            range_start_line: Some(0),
            detail: None,
        };
        MockLspClient::new().with_symbols(file, vec![module])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "inner/Bar/do_thing",
                "action": "replace",
                "body": "        pub fn do_thing(&self) {\n            new();\n        }",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        content.contains("mod inner {"),
        "module header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("impl Bar {"),
        "impl header must be preserved; got:\n{content}"
    );
    assert!(
        content.contains("new()"),
        "new body must be applied; got:\n{content}"
    );
}

/// BUG-034 guard: top-level Rust function (no parent) — verify no regression.
/// find_parent_symbol returns None, guard doesn't fire.
#[tokio::test]
async fn bug034_guard_top_level_function_no_parent_no_regression() {
    let src = "\
/// Top-level function.
pub fn standalone() {
    old_impl();
}

pub fn other() {}
";
    let (dir, ctx) = ctx_with_mock(&[("src/lib.rs", src)], |root| {
        let file = root.join("src/lib.rs");
        let sym1 = sym_with_range("standalone", 1, 3, 0, file.clone());
        let sym2 = sym_with_range("other", 5, 5, 5, file.clone());
        MockLspClient::new().with_symbols(file, vec![sym1, sym2])
    })
    .await;

    EditCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "standalone",
                "action": "replace",
                "body": "/// Top-level function.\npub fn standalone() {\n    new_impl();\n}",
            }),
            &ctx,
        )
        .await
        .unwrap();

    let content = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        content.contains("new_impl()"),
        "new body must be applied; got:\n{content}"
    );
    assert!(
        !content.contains("old_impl()"),
        "old body must be gone; got:\n{content}"
    );
    assert!(
        content.contains("pub fn other"),
        "sibling must be preserved; got:\n{content}"
    );
}
