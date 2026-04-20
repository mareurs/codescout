//! Regression tests for tool misbehavior bugs (BUG-029, BUG-031, BUG-032).
//!
//! These tests use real LSP servers — they are skipped when the required LSP
//! is not installed. Run with:
//!
//!     cargo test --test bug_regression -- --ignored
//!
//! All tests follow the sandwich pattern:
//! 1. Setup a scenario that triggers the bug
//! 2. Call the tool
//! 3. Assert the output is correct (would have been wrong before the fix)

use codescout::agent::Agent;
use codescout::lsp::LspManager;
use codescout::tools::markdown::EditMarkdown;
use codescout::tools::output_buffer::OutputBuffer;
use codescout::tools::symbol::{InsertCode, RemoveSymbol, ReplaceSymbol};
use codescout::tools::{Tool, ToolContext};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

/// Create a project context with files pre-populated and a real LSP manager.
async fn project_with_files(files: &[(&str, &str)]) -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer: Arc::new(OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            codescout::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    (dir, ctx)
}

fn lsp_available(cmd: &str) -> bool {
    // Some LSP servers (e.g. pyright-langserver) don't support --version and exit 1.
    // Use `which` to check if the binary exists on PATH instead.
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ===========================================================================
// BUG-031: replace_symbol must not duplicate doc comments
// ===========================================================================

/// Rust: replace a function that has `///` doc comments.
/// Before fix: old doc comments left in place, new body's docs appended → duplication.
#[tokio::test]
#[ignore] // requires rust-analyzer
async fn bug031_replace_symbol_with_doc_comments_rust() {
    if !lsp_available("rust-analyzer") {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let src = r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#;
    let code = r#"/// Adds two numbers.
/// Returns the sum.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Subtracts b from a.
pub fn sub(a: i32, b: i32) -> i32 {
    a - b
}
"#;

    let (dir, ctx) = project_with_files(&[("Cargo.toml", src), ("src/lib.rs", code)]).await;

    // Replace `add` with a new implementation (including updated doc comments)
    let new_body = r#"/// Adds two numbers together.
/// Returns their sum.
pub fn add(a: i32, b: i32) -> i32 {
    let result = a + b;
    result
}"#;

    ReplaceSymbol
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "add",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();

    // Must not have duplicated doc comments
    assert_eq!(
        result.matches("/// Adds two numbers").count(),
        1,
        "doc comment must appear exactly once (no duplication); got:\n{result}"
    );
    assert!(
        result.contains("let result = a + b"),
        "new body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("a + b\n}"),
        "old body must be gone; got:\n{result}"
    );
    // `sub` must be untouched
    assert!(
        result.contains("pub fn sub"),
        "adjacent function must survive; got:\n{result}"
    );
}

/// Python: replace a function with a docstring.
#[tokio::test]
#[ignore] // requires pyright
async fn bug031_replace_symbol_with_doc_comments_python() {
    if !lsp_available("pyright-langserver") {
        eprintln!("Skipping: pyright-langserver not installed");
        return;
    }

    let code = r#"def add(a: int, b: int) -> int:
    """Add two numbers."""
    return a + b

def sub(a: int, b: int) -> int:
    """Subtract b from a."""
    return a - b
"#;

    let (dir, ctx) = project_with_files(&[("lib.py", code)]).await;

    let new_body = r#"def add(a: int, b: int) -> int:
    """Add two numbers together."""
    result = a + b
    return result"#;

    ReplaceSymbol
        .call(
            json!({
                "path": "lib.py",
                "symbol": "add",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("lib.py")).unwrap();
    assert_eq!(
        result.matches("def add").count(),
        1,
        "function signature must appear once; got:\n{result}"
    );
    assert!(
        result.contains("result = a + b"),
        "new body; got:\n{result}"
    );
    assert!(
        result.contains("def sub"),
        "adjacent function must survive; got:\n{result}"
    );
}

/// TypeScript: replace a function with JSDoc.
#[tokio::test]
#[ignore] // requires typescript-language-server
async fn bug031_replace_symbol_with_doc_comments_typescript() {
    if !lsp_available("typescript-language-server") {
        eprintln!("Skipping: typescript-language-server not installed");
        return;
    }

    let code = r#"/**
 * Add two numbers.
 * @param a First number
 * @param b Second number
 */
export function add(a: number, b: number): number {
    return a + b;
}

export function sub(a: number, b: number): number {
    return a - b;
}
"#;
    // tsconfig so the LSP doesn't complain
    let tsconfig = r#"{ "compilerOptions": { "strict": true } }"#;

    let (dir, ctx) = project_with_files(&[("src/lib.ts", code), ("tsconfig.json", tsconfig)]).await;

    let new_body = r#"/**
 * Add two numbers together.
 * @param a First number
 * @param b Second number
 */
export function add(a: number, b: number): number {
    const result = a + b;
    return result;
}"#;

    ReplaceSymbol
        .call(
            json!({
                "path": "src/lib.ts",
                "symbol": "add",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.ts")).unwrap();
    assert_eq!(
        result.matches("export function add").count(),
        1,
        "function must appear once; got:\n{result}"
    );
    assert_eq!(
        result.matches("/**").count(),
        1,
        "JSDoc opener must appear once (no duplication); got:\n{result}"
    );
    assert!(
        result.contains("const result = a + b"),
        "new body; got:\n{result}"
    );
    assert!(
        result.contains("export function sub"),
        "adjacent function must survive; got:\n{result}"
    );
}

/// Go: replace a function with a Go doc comment.
#[tokio::test]
#[ignore] // requires gopls
async fn bug031_replace_symbol_with_doc_comments_go() {
    if !lsp_available("gopls") {
        eprintln!("Skipping: gopls not installed");
        return;
    }

    let code = r#"package math

// Add returns the sum of a and b.
func Add(a, b int) int {
	return a + b
}

// Sub returns a minus b.
func Sub(a, b int) int {
	return a - b
}
"#;
    let go_mod = "module example.com/test\n\ngo 1.21\n";

    let (dir, ctx) = project_with_files(&[("math.go", code), ("go.mod", go_mod)]).await;

    let new_body = r#"// Add returns the sum of a and b.
func Add(a, b int) int {
	result := a + b
	return result
}"#;

    ReplaceSymbol
        .call(
            json!({
                "path": "math.go",
                "symbol": "Add",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("math.go")).unwrap();
    assert_eq!(
        result.matches("func Add").count(),
        1,
        "function must appear once; got:\n{result}"
    );
    assert_eq!(
        result.matches("// Add returns").count(),
        1,
        "doc comment must appear once; got:\n{result}"
    );
    assert!(
        result.contains("result := a + b"),
        "new body; got:\n{result}"
    );
    assert!(
        result.contains("func Sub"),
        "adjacent function must survive; got:\n{result}"
    );
}

// ===========================================================================
// BUG-029: insert_code "after" must place code AFTER the closing brace
// ===========================================================================

/// Rust: insert a new test function after an existing one inside mod tests.
#[tokio::test]
#[ignore] // requires rust-analyzer
async fn bug029_insert_code_after_nested_fn_rust() {
    if !lsp_available("rust-analyzer") {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let src = r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#;
    let code = r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, -2), -3);
    }
}
"#;

    let (dir, ctx) = project_with_files(&[("Cargo.toml", src), ("src/lib.rs", code)]).await;

    let new_test = r#"
    #[test]
    fn test_add_zero() {
        assert_eq!(add(0, 0), 0);
    }"#;

    InsertCode
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "tests/test_add_positive",
                "code": new_test,
                "position": "after"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();

    // The new test must be a sibling, not nested inside test_add_positive
    assert!(
        result.contains("fn test_add_zero"),
        "new test must be present; got:\n{result}"
    );
    // test_add_positive must remain intact — its body must not be split
    assert!(
        result.contains("assert_eq!(add(2, 3), 5);"),
        "test_add_positive body must be intact; got:\n{result}"
    );

    // Verify the file compiles by checking balanced braces
    let opens = result.matches('{').count();
    let closes = result.matches('}').count();
    assert_eq!(
        opens, closes,
        "braces must be balanced (opens={opens}, closes={closes}); got:\n{result}"
    );
}

/// TypeScript: insert a function after another in a module.
#[tokio::test]
#[ignore] // requires typescript-language-server
async fn bug029_insert_code_after_function_typescript() {
    if !lsp_available("typescript-language-server") {
        eprintln!("Skipping: typescript-language-server not installed");
        return;
    }

    let code = r#"export function first(): number {
    const x = 1;
    const y = 2;
    return x + y;
}

export function third(): number {
    return 3;
}
"#;
    let tsconfig = r#"{ "compilerOptions": { "strict": true } }"#;

    let (dir, ctx) = project_with_files(&[("src/lib.ts", code), ("tsconfig.json", tsconfig)]).await;

    let new_fn = r#"
export function second(): number {
    return 2;
}"#;

    InsertCode
        .call(
            json!({
                "path": "src/lib.ts",
                "symbol": "first",
                "code": new_fn,
                "position": "after"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.ts")).unwrap();
    assert!(
        result.contains("function second"),
        "new function; got:\n{result}"
    );
    // `first` must be complete — its body must not be split
    assert!(
        result.contains("return x + y;"),
        "first() body intact; got:\n{result}"
    );
    assert!(
        result.contains("function third"),
        "third() must survive; got:\n{result}"
    );
    let opens = result.matches('{').count();
    let closes = result.matches('}').count();
    assert_eq!(opens, closes, "braces balanced; got:\n{result}");
}

// ===========================================================================
// BUG-032: sequential remove_symbol must not corrupt file
// ===========================================================================

/// Rust: remove an enum and its impl block sequentially.
/// Before fix: second removal used stale line numbers, corrupting the next function.
#[tokio::test]
#[ignore] // requires rust-analyzer
async fn bug032_sequential_remove_symbol_rust() {
    if !lsp_available("rust-analyzer") {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let src = r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#;
    let code = r#"pub enum Filter {
    All,
    Active,
    Inactive,
}

impl Filter {
    pub fn is_all(&self) -> bool {
        matches!(self, Filter::All)
    }
}

pub fn process(items: &[i32]) -> Vec<i32> {
    items.iter().copied().collect()
}
"#;

    let (dir, ctx) = project_with_files(&[("Cargo.toml", src), ("src/lib.rs", code)]).await;

    // First removal: remove the enum
    RemoveSymbol
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "Filter"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let after_first = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        !after_first.contains("pub enum Filter"),
        "enum must be removed; got:\n{after_first}"
    );
    assert!(
        after_first.contains("pub fn process"),
        "process() must survive first removal; got:\n{after_first}"
    );

    // Second removal: remove the impl block
    // This is where BUG-032 would strike — stale line numbers
    let result = RemoveSymbol
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "impl Filter"
            }),
            &ctx,
        )
        .await;

    match result {
        Ok(_) => {
            let after_second = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
            assert!(
                !after_second.contains("impl Filter"),
                "impl block must be removed; got:\n{after_second}"
            );
            assert!(
                after_second.contains("pub fn process"),
                "process() must survive both removals; got:\n{after_second}"
            );
            // File must still be valid Rust
            let opens = after_second.matches('{').count();
            let closes = after_second.matches('}').count();
            assert_eq!(
                opens, closes,
                "braces must be balanced; got:\n{after_second}"
            );
        }
        Err(e) => {
            let msg = e.to_string();
            // RecoverableError about stale positions is acceptable — the guard caught it
            assert!(
                msg.contains("stale") || msg.contains("not found"),
                "error should be about stale data or missing symbol, got: {msg}"
            );
        }
    }
}

/// Python: sequential removal of a class and a standalone function.
#[tokio::test]
#[ignore] // requires pyright
async fn bug032_sequential_remove_symbol_python() {
    if !lsp_available("pyright-langserver") {
        eprintln!("Skipping: pyright-langserver not installed");
        return;
    }

    let code = r#"class Filter:
    ALL = "all"
    ACTIVE = "active"

def process(items: list[int]) -> list[int]:
    return list(items)

def helper() -> str:
    return "ok"
"#;

    let (dir, ctx) = project_with_files(&[("lib.py", code)]).await;

    // Remove the class
    RemoveSymbol
        .call(json!({ "path": "lib.py", "symbol": "Filter" }), &ctx)
        .await
        .unwrap();

    let after_first = std::fs::read_to_string(dir.path().join("lib.py")).unwrap();
    assert!(
        !after_first.contains("class Filter"),
        "class removed; got:\n{after_first}"
    );
    assert!(
        after_first.contains("def process"),
        "process survives; got:\n{after_first}"
    );

    // Remove process — uses positions that may be stale
    let result = RemoveSymbol
        .call(json!({ "path": "lib.py", "symbol": "process" }), &ctx)
        .await;

    match result {
        Ok(_) => {
            let after_second = std::fs::read_to_string(dir.path().join("lib.py")).unwrap();
            assert!(
                !after_second.contains("def process"),
                "process removed; got:\n{after_second}"
            );
            assert!(
                after_second.contains("def helper"),
                "helper survives; got:\n{after_second}"
            );
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("stale") || msg.contains("not found"),
                "acceptable error about stale data; got: {msg}"
            );
        }
    }
}

/// BUG-043: `replace` on a heading whose section contains sub-headings used to
/// silently wipe them. Guard rejects unless caller opts in with
/// `include_subsections: true`.
#[tokio::test]
async fn bug043_edit_markdown_replace_rejects_when_section_has_subsections() {
    let plan = "\
# Plan

## File Map
short map body

### Task A
work
### Task B
more work
### Task C
even more
";
    let (dir, ctx) = project_with_files(&[("plan.md", plan)]).await;

    let err = EditMarkdown
        .call(
            json!({
                "path": "plan.md",
                "heading": "## File Map",
                "action": "replace",
                "content": "new short map body\n"
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("### Task A") && msg.contains("### Task B") && msg.contains("### Task C"),
        "error must list the would-be-consumed subsections; got: {msg}"
    );
    assert!(
        msg.contains("include_subsections"),
        "error must point to the opt-in flag; got: {msg}"
    );

    // File must be unchanged on disk.
    let on_disk = std::fs::read_to_string(dir.path().join("plan.md")).unwrap();
    assert_eq!(on_disk, plan, "file must be untouched when guard fires");
}

/// `include_subsections: true` bypasses the BUG-043 guard — user explicitly
/// asked for the consume-children semantics.
#[tokio::test]
async fn bug043_edit_markdown_replace_allows_subsection_consumption_on_opt_in() {
    let plan = "\
# Plan

## File Map
old
### Task A
work
";
    let (dir, ctx) = project_with_files(&[("plan.md", plan)]).await;

    EditMarkdown
        .call(
            json!({
                "path": "plan.md",
                "heading": "## File Map",
                "action": "replace",
                "content": "new body\n",
                "include_subsections": true
            }),
            &ctx,
        )
        .await
        .expect("opt-in must succeed");

    let on_disk = std::fs::read_to_string(dir.path().join("plan.md")).unwrap();
    assert!(on_disk.contains("new body"));
    assert!(
        !on_disk.contains("### Task A"),
        "opt-in truly consumes subsections: {on_disk}"
    );
}

// ===========================================================================
// BUG-044: replace_symbol on a method inside an impl/class must not drop
// sibling methods, even when LSP ranges overshoot.
// ===========================================================================

/// Rust (rust-analyzer): impl block with two sibling methods. Replacing one
/// must leave the other intact.
#[tokio::test]
#[ignore] // requires rust-analyzer
async fn bug044_replace_symbol_preserves_sibling_method_rust() {
    if !lsp_available("rust-analyzer") {
        eprintln!("Skipping: rust-analyzer not installed");
        return;
    }

    let manifest = r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#;
    let code = r#"pub struct Foo;

impl Foo {
    pub fn alpha(&self) -> i32 {
        1
    }

    pub fn beta(&self) -> i32 {
        2
    }
}
"#;

    let (dir, ctx) = project_with_files(&[("Cargo.toml", manifest), ("src/lib.rs", code)]).await;

    let new_body = r#"    pub fn alpha(&self) -> i32 {
        99
    }"#;

    ReplaceSymbol
        .call(
            json!({
                "path": "src/lib.rs",
                "symbol": "impl Foo/alpha",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(
        result.contains("99"),
        "alpha must be replaced with new body; got:\n{result}"
    );
    assert!(
        result.contains("pub fn beta"),
        "sibling beta must survive the replacement; got:\n{result}"
    );
    assert!(
        result.contains('2'),
        "beta's body must survive; got:\n{result}"
    );
}

/// Python (pyright): class with two sibling methods. Replacing one must
/// leave the other intact.
#[tokio::test]
#[ignore] // requires pyright-langserver
async fn bug044_replace_symbol_preserves_sibling_method_python() {
    if !lsp_available("pyright-langserver") {
        eprintln!("Skipping: pyright-langserver not installed");
        return;
    }

    let code = "\
class Foo:
    def alpha(self) -> int:
        return 1

    def beta(self) -> int:
        return 2
";

    let (dir, ctx) = project_with_files(&[("main.py", code)]).await;

    let new_body = "    def alpha(self) -> int:\n        return 99";

    ReplaceSymbol
        .call(
            json!({
                "path": "main.py",
                "symbol": "Foo/alpha",
                "new_body": new_body
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = std::fs::read_to_string(dir.path().join("main.py")).unwrap();
    assert!(
        result.contains("return 99"),
        "alpha must be replaced; got:\n{result}"
    );
    assert!(
        result.contains("def beta"),
        "sibling beta must survive; got:\n{result}"
    );
    assert!(
        result.contains("return 2"),
        "beta's body must survive; got:\n{result}"
    );
}
