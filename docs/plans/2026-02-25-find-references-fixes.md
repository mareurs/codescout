# Fix find_referencing_symbols Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix five issues in the `find_referencing_symbols` tool that cause incorrect behavior across languages: wrong cursor position, broken flat-symbol name_path resolution, misleading language support claims, missing test coverage, and stale context lines.

**Architecture:** All fixes are in `src/lsp/client.rs` (symbol conversion) and `src/tools/symbol.rs` (tool implementation). The LSP client's `convert_document_symbols` incorrectly uses `range` instead of `selection_range` for cursor positioning. The flat `SymbolInformation` fallback discards hierarchy. The tool's error message lists unsupported languages. Tests need a mock-based approach since integration tests require a live LSP server.

**Tech Stack:** Rust, lsp-types crate, serde_json, tempfile (tests), async_trait

---

### Task 1: Fix selection_range vs range in convert_document_symbols

The `convert_document_symbols` function in `src/lsp/client.rs:35-66` uses `ds.range.start` to set `start_line` and `start_col`. This is the full declaration range (which may start at a decorator, attribute, or doc comment). The LSP `textDocument/references` request needs the cursor on the **identifier name**, which is `ds.selection_range.start`.

**Files:**
- Modify: `src/lsp/client.rs:56-57` (convert_document_symbols)

**Step 1: Write the failing test**

Add to the existing `tests` module in `src/lsp/client.rs` (after line ~771):

```rust
#[test]
fn convert_document_symbols_uses_selection_range() {
    use lsp_types::{DocumentSymbol, Range, Position, SymbolKind as LspSymbolKind};

    let symbols = vec![DocumentSymbol {
        name: "my_func".to_string(),
        detail: None,
        kind: LspSymbolKind::FUNCTION,
        tags: None,
        deprecated: None,
        // Full range starts at line 5 (e.g. a doc comment)
        range: Range {
            start: Position { line: 5, character: 0 },
            end: Position { line: 10, character: 1 },
        },
        // The identifier "my_func" is at line 8, col 4
        selection_range: Range {
            start: Position { line: 8, character: 4 },
            end: Position { line: 8, character: 11 },
        },
        children: None,
    }];

    let path = std::path::PathBuf::from("/tmp/test.rs");
    let result = convert_document_symbols(&symbols, &path, "");

    assert_eq!(result.len(), 1);
    // start_line/start_col should come from selection_range for cursor positioning
    assert_eq!(result[0].start_line, 8, "start_line should use selection_range");
    assert_eq!(result[0].start_col, 4, "start_col should use selection_range");
    // end_line should still come from range (for body extent)
    assert_eq!(result[0].end_line, 10, "end_line should use range for body extent");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test convert_document_symbols_uses_selection_range -- --nocapture`
Expected: FAIL — `start_line` will be 5 (from `range`) instead of 8 (from `selection_range`).

**Step 3: Fix convert_document_symbols**

In `src/lsp/client.rs`, inside `convert_document_symbols`, change:

```rust
// BEFORE (line ~56-57):
start_line: ds.range.start.line,
start_col: ds.range.start.character,

// AFTER:
start_line: ds.selection_range.start.line,
start_col: ds.selection_range.start.character,
```

Keep `end_line: ds.range.end.line` unchanged — we want the full extent for body reads.

**Step 4: Run test to verify it passes**

Run: `cargo test convert_document_symbols_uses_selection_range -- --nocapture`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All passing. No other tests depend on `start_line` coming from `range.start`.

**Step 6: Commit**

```bash
git add src/lsp/client.rs
git commit -m "fix(lsp): use selection_range for symbol cursor position

convert_document_symbols was using range.start (full declaration extent)
instead of selection_range.start (identifier position). This caused
textDocument/references to receive wrong cursor positions when symbols
had decorators, attributes, or doc comments before the identifier."
```

---

### Task 2: Fix flat SymbolInformation fallback losing name_path hierarchy

When an LSP server returns `SymbolInformation[]` (flat format) instead of `DocumentSymbol[]` (hierarchical), the current code sets `name_path: si.name.clone()` — losing all hierarchy. A caller passing `name_path: "MyStruct/my_method"` will never match. The flat format includes a `container_name` field we can use to reconstruct partial hierarchy.

**Files:**
- Modify: `src/lsp/client.rs:451-466` (flat SymbolInformation fallback in document_symbols)

**Step 1: Write the failing test**

Add to `tests` module in `src/lsp/client.rs`:

```rust
#[test]
fn flat_symbol_information_builds_name_path_from_container() {
    use lsp_types::{Location, Range, Position, SymbolInformation, SymbolKind as LspSymbolKind, Url};

    let uri = Url::parse("file:///tmp/test.rb").unwrap();
    let infos: Vec<SymbolInformation> = vec![
        SymbolInformation {
            name: "MyClass".to_string(),
            kind: LspSymbolKind::CLASS,
            tags: None,
            deprecated: None,
            location: Location {
                uri: uri.clone(),
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 20, character: 3 },
                },
            },
            container_name: None,
        },
        SymbolInformation {
            name: "my_method".to_string(),
            kind: LspSymbolKind::METHOD,
            tags: None,
            deprecated: None,
            location: Location {
                uri: uri.clone(),
                range: Range {
                    start: Position { line: 5, character: 2 },
                    end: Position { line: 10, character: 5 },
                },
            },
            container_name: Some("MyClass".to_string()),
        },
    ];

    let json_val = serde_json::to_value(&infos).unwrap();
    let parsed: Vec<SymbolInformation> = serde_json::from_value(json_val).unwrap();

    let file_path = std::path::PathBuf::from("/tmp/test.rb");
    let result: Vec<super::SymbolInfo> = parsed
        .iter()
        .map(|si| {
            let name_path = match &si.container_name {
                Some(container) if !container.is_empty() => {
                    format!("{}/{}", container, si.name)
                }
                _ => si.name.clone(),
            };
            super::SymbolInfo {
                name: si.name.clone(),
                name_path,
                kind: si.kind.into(),
                file: file_path.clone(),
                start_line: si.location.range.start.line,
                end_line: si.location.range.end.line,
                start_col: si.location.range.start.character,
                children: vec![],
            }
        })
        .collect();

    assert_eq!(result[0].name_path, "MyClass");
    assert_eq!(result[1].name_path, "MyClass/my_method");
}
```

**Step 2: Run test to verify it passes (this tests the desired logic inline)**

Run: `cargo test flat_symbol_information_builds_name_path -- --nocapture`
Expected: PASS (this validates the logic we'll put into the code).

**Step 3: Apply the fix to document_symbols**

In `src/lsp/client.rs`, replace the flat `SymbolInformation` fallback block (lines ~451-466):

```rust
// BEFORE:
if let Ok(infos) = serde_json::from_value::<Vec<lsp_types::SymbolInformation>>(result) {
    return Ok(infos
        .iter()
        .map(|si| super::SymbolInfo {
            name: si.name.clone(),
            name_path: si.name.clone(),
            kind: si.kind.into(),
            file: file_path.clone(),
            start_line: si.location.range.start.line,
            end_line: si.location.range.end.line,
            start_col: si.location.range.start.character,
            children: vec![],
        })
        .collect());
}

// AFTER:
if let Ok(infos) = serde_json::from_value::<Vec<lsp_types::SymbolInformation>>(result) {
    return Ok(infos
        .iter()
        .map(|si| {
            let name_path = match &si.container_name {
                Some(container) if !container.is_empty() => {
                    format!("{}/{}", container, si.name)
                }
                _ => si.name.clone(),
            };
            super::SymbolInfo {
                name: si.name.clone(),
                name_path,
                kind: si.kind.into(),
                file: file_path.clone(),
                start_line: si.location.range.start.line,
                end_line: si.location.range.end.line,
                start_col: si.location.range.start.character,
                children: vec![],
            }
        })
        .collect());
}
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All passing.

**Step 5: Commit**

```bash
git add src/lsp/client.rs
git commit -m "fix(lsp): build name_path from container_name in flat symbol format

When LSP servers return SymbolInformation[] (flat) instead of
DocumentSymbol[] (hierarchical), we now construct name_path from
container_name (e.g. 'MyClass/my_method') instead of using bare name.
This fixes find_referencing_symbols for servers like solargraph (Ruby)
and some jdtls (Java) configurations."
```

---

### Task 3: Align language detection error message with actual LSP support

`get_lsp_client` in `src/tools/symbol.rs:90-110` lists 20+ languages in its error message including php, swift, scala, elixir, haskell, lua, bash, markdown — but only 9 have LSP configs in `servers.rs`. The error misleads users. Fix: only list languages with actual LSP server configs.

**Files:**
- Modify: `src/tools/symbol.rs:99-105` (get_lsp_client error message)

**Step 1: Fix the error message**

In `src/tools/symbol.rs`, change the error in `get_lsp_client`:

```rust
// BEFORE:
let lang = ast::detect_language(path).ok_or_else(|| {
    anyhow!(
        "unsupported file type: {:?}. Supported languages: \
         rust, python, typescript, tsx, javascript, jsx, go, java, kotlin, \
         c, cpp, csharp, ruby, php, swift, scala, elixir, haskell, lua, bash, markdown",
        path
    )
})?;

// AFTER:
let lang = ast::detect_language(path).ok_or_else(|| {
    anyhow!(
        "unsupported file type: {:?}. Languages with LSP support: \
         rust, python, typescript, tsx, javascript, jsx, go, java, kotlin, \
         c, cpp, csharp, ruby",
        path
    )
})?;
```

**Step 2: Run tests**

Run: `cargo test`
Expected: All passing.

**Step 3: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "fix(tools): align error message with actual LSP-supported languages

The error listed 20+ languages from detect_language() but only 9 have
LSP server configs. Now lists only languages with actual LSP support."
```

---

### Task 4: Add unit tests for find_symbol_by_name_path edge cases

The recursive tree walker `find_symbol_by_name_path` is critical for all symbol tools. Add tests covering: exact name_path match, bare name match, nested children, and miss cases.

**Files:**
- Modify: `src/tools/symbol.rs` (add to tests module)

**Step 1: Write the tests**

Add to the `tests` module in `src/tools/symbol.rs`:

```rust
#[test]
fn find_symbol_by_name_path_exact_match() {
    let symbols = vec![SymbolInfo {
        name: "MyStruct".to_string(),
        name_path: "MyStruct".to_string(),
        kind: crate::lsp::SymbolKind::Struct,
        file: PathBuf::from("/tmp/test.rs"),
        start_line: 0,
        end_line: 10,
        start_col: 0,
        children: vec![SymbolInfo {
            name: "my_method".to_string(),
            name_path: "MyStruct/my_method".to_string(),
            kind: crate::lsp::SymbolKind::Method,
            file: PathBuf::from("/tmp/test.rs"),
            start_line: 2,
            end_line: 5,
            start_col: 4,
            children: vec![],
        }],
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
```

**Step 2: Run test**

Run: `cargo test find_symbol_by_name_path_exact_match -- --nocapture`
Expected: PASS

**Step 3: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "test(tools): add unit tests for find_symbol_by_name_path

Covers exact name_path match, bare name fallback, nested children
traversal, and miss cases."
```

---

### Task 5: Add integration test for FindReferencingSymbols happy path

The existing test infrastructure in `symbol.rs` creates a temp Cargo project and uses rust-analyzer. Add a test that actually exercises the find_referencing_symbols flow end-to-end. This test skips if rust-analyzer is not installed (matching the pattern of existing tests).

**Files:**
- Modify: `src/tools/symbol.rs` (add to tests module)

**Step 1: Write the integration test**

The test project needs a function that's called from multiple places so references can be found. Update the `rust_project_ctx` fixture to write a richer `main.rs`, or create a second fixture. Since modifying the shared fixture could break other tests, we'll write a dedicated one.

Add to the `tests` module:

```rust
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
    std::fs::create_dir_all(dir.path().join(".code-explorer")).unwrap();
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
    let ctx = ToolContext { agent, lsp: lsp() };

    let result = FindReferencingSymbols
        .call(
            json!({
                "name_path": "add",
                "relative_path": "src/main.rs"
            }),
            &ctx,
        )
        .await;

    // If LSP startup fails (e.g. cargo not in PATH), skip gracefully
    let result = match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Skipping: LSP error: {}", e);
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
```

**Step 2: Run test**

Run: `cargo test find_referencing_symbols_returns_references -- --nocapture`
Expected: PASS (or skip if rust-analyzer not installed). This test exercises the full pipeline: document_symbols → find_symbol_by_name_path → references → context line reading → OutputGuard.

**Step 3: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "test(tools): add integration test for find_referencing_symbols

Exercises the full flow: LSP document_symbols → name_path resolution →
textDocument/references → context line reading → OutputGuard capping.
Skips gracefully if rust-analyzer is not installed."
```

---

## Task Dependency Graph

```
Task 1 (selection_range)  ──┐
Task 2 (flat name_path)  ───┤── independent, can parallelize
Task 3 (error message)  ────┤
Task 4 (unit tests)  ───────┘
                              │
Task 5 (integration test) ────── depends on Tasks 1-2 being fixed
                                 (otherwise the test might fail on
                                  edge cases the test hits)
```

## Not Addressed (deferred)

**Stale context lines from disk** (issue #5 from analysis): This is low severity and the fix would involve either reading from the LSP's open document buffer (requires protocol changes) or accepting the minor inconsistency. Deferred until it causes a real user-facing issue.
