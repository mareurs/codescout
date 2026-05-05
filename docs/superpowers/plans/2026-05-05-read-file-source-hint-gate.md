# read_file Source-Range Hint Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Block `read_file(source_file, start_line, end_line)` calls that overlap a named symbol body and redirect the agent to `symbols(include_body=true)`, with a `force: true` escape hatch.

**Architecture:** Add a synchronous tree-sitter symbol lookup (`find_symbols_for_range`) inside `read_with_line_range`. When the read range strictly contains or is strictly contained by a named symbol's body, return a `RecoverableError` naming the symbol and suggesting `symbols(include_body=true)`. `force: true` bypasses the gate entirely. Fail open (no hint) on parse error.

**Tech Stack:** Rust, tree-sitter (already wired via `src/ast/`), `serde_json`, existing `RecoverableError` pattern.

---

## File Map

| File | Change |
|---|---|
| `src/ast/mod.rs` | Add `pub fn extract_symbols_from_text(text, path)` — avoids re-reading already-loaded file |
| `src/tools/read_file.rs` | Add `flatten_symbols`, `find_symbols_for_range`; modify `read_with_line_range` + `call`; update schema + description |
| `src/tools/edit_file/tests.rs` | Add 4 new tests for the gate |

---

## Task 1: Write the 4 failing tests (TDD)

**Files:**
- Modify: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Add the 4 tests**

Append these tests inside the `tests` module in `src/tools/edit_file/tests.rs`, after the existing `ReadFile` tests block:

```rust
// ── ReadFile: source-range hint gate ─────────────────────────────────────

#[tokio::test]
async fn read_file_source_range_blocked_when_symbol_overlaps() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    // .rs extension → detected as Source
    let file = dir.path().join("fixture.rs");
    std::fs::write(
        &file,
        "use std::io;\n\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n",
    )
    .unwrap();
    let path = file.to_str().unwrap();

    // Lines 3–5 span the body of `fn greet` exactly.
    let result = ReadFile
        .call(
            json!({ "path": path, "start_line": 3, "end_line": 5 }),
            &ctx,
        )
        .await;

    assert!(result.is_err(), "range overlapping a symbol body must be blocked");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("greet"),
        "error must name the overlapping symbol, got: {msg}"
    );
}

#[tokio::test]
async fn read_file_source_range_force_bypasses_gate() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("fixture.rs");
    std::fs::write(
        &file,
        "use std::io;\n\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n",
    )
    .unwrap();
    let path = file.to_str().unwrap();

    // Same range as above, but force=true skips the gate.
    let result = ReadFile
        .call(
            json!({ "path": path, "start_line": 3, "end_line": 5, "force": true }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        result["content"].as_str().unwrap().contains("greet"),
        "force=true must return the raw content"
    );
}

#[tokio::test]
async fn read_file_source_range_not_blocked_for_imports() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("fixture.rs");
    std::fs::write(
        &file,
        "use std::io;\n\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n",
    )
    .unwrap();
    let path = file.to_str().unwrap();

    // Line 1 only (`use std::io;`) — no named symbol body spans it.
    let result = ReadFile
        .call(
            json!({ "path": path, "start_line": 1, "end_line": 1 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        result["content"].as_str().unwrap().contains("use std::io"),
        "import-only range must pass through"
    );
}

#[tokio::test]
async fn read_file_source_range_non_source_not_blocked() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    // .toml extension → not a Source file
    let file = dir.path().join("config.toml");
    std::fs::write(&file, "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n").unwrap();
    let path = file.to_str().unwrap();

    let result = ReadFile
        .call(
            json!({ "path": path, "start_line": 1, "end_line": 2 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        result["content"].as_str().unwrap().contains("package"),
        "non-source file line range must pass through"
    );
}
```

- [ ] **Step 2: Run the tests to confirm they fail (not compile-error — the gate doesn't exist yet)**

```bash
cargo test read_file_source_range 2>&1 | tail -20
```

Expected: 1 test passes (`non_source_not_blocked` and `not_blocked_for_imports` may also pass since no gate exists yet), but `blocked_when_symbol_overlaps` passes with `result.is_ok()` — which means the assert `result.is_err()` fails. Confirm the right tests fail before continuing.

---

## Task 2: Add `extract_symbols_from_text` to `src/ast/mod.rs`

**Files:**
- Modify: `src/ast/mod.rs`

This avoids re-reading the file from disk in `find_symbols_for_range` since the text is already in memory.

- [ ] **Step 1: Add the function**

In `src/ast/mod.rs`, add after the existing `extract_symbols` function (around line 35):

```rust
/// Extract symbols from already-loaded source text using tree-sitter.
///
/// Prefer this over `extract_symbols` when the file content is already in memory
/// to avoid a second disk read.
pub fn extract_symbols_from_text(text: &str, path: &Path) -> Result<Vec<SymbolInfo>> {
    let language = detect_language(path);
    parser::extract_symbols_from_source(text, language, path)
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check 2>&1 | grep -E "error|warning" | head -20
```

Expected: no errors. Warnings about unused function are fine at this stage.

- [ ] **Step 3: Commit the AST helper**

```bash
git add src/ast/mod.rs
git commit -m "feat(ast): add extract_symbols_from_text for in-memory text parsing"
```

---

## Task 3: Add helpers to `src/tools/read_file.rs`

**Files:**
- Modify: `src/tools/read_file.rs`

- [ ] **Step 1: Add `flatten_symbols` and `find_symbols_for_range`**

Add these two private functions near the bottom of `src/tools/read_file.rs`, before the `#[cfg(test)]` block if one exists, or at end of file:

```rust
/// Recursively flatten a symbol tree into a single Vec of references.
fn flatten_symbols<'a>(syms: &'a [crate::lsp::SymbolInfo], out: &mut Vec<&'a crate::lsp::SymbolInfo>) {
    for sym in syms {
        out.push(sym);
        flatten_symbols(&sym.children, out);
    }
}

/// Return the `name_path` of every symbol whose body strictly contains the
/// read range, or whose body is strictly contained by the read range.
///
/// `start` and `end` are 1-indexed (as received from tool input).
/// `SymbolInfo.start_line` / `end_line` are 0-indexed.
/// Returns an empty Vec on parse error (fail open).
fn find_symbols_for_range(text: &str, resolved: &std::path::Path, start: u64, end: u64) -> Vec<String> {
    let syms = match crate::ast::extract_symbols_from_text(text, resolved) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let mut flat = Vec::new();
    flatten_symbols(&syms, &mut flat);

    let s0 = (start.saturating_sub(1)) as u32;
    let e0 = (end.saturating_sub(1)) as u32;

    flat.into_iter()
        .filter(|sym| {
            // symbol body contains read range
            (sym.start_line <= s0 && e0 <= sym.end_line)
            // read range contains symbol body
            || (s0 <= sym.start_line && sym.end_line <= e0)
        })
        .map(|sym| sym.name_path.clone())
        .collect()
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check 2>&1 | grep "^error" | head -20
```

Expected: no errors.

---

## Task 4: Wire the gate + update schema and description

**Files:**
- Modify: `src/tools/read_file.rs`

- [ ] **Step 1: Add `force: bool` param to `read_with_line_range`**

Find the function signature of `read_with_line_range` (around line 376) and add `force: bool` as the last parameter:

```rust
fn read_with_line_range(
    path: &str,
    text: &str,
    resolved: &std::path::PathBuf,
    start: u64,
    end: u64,
    source_tag: &str,
    ctx: &ToolContext,
    force: bool,
) -> Result<Value> {
```

- [ ] **Step 2: Add the gate logic inside `read_with_line_range`**

After the existing `if start == 0 || end < start { ... }` validation block (around line 395), add:

```rust
    if !force
        && crate::tools::file_summary::detect_file_type(path)
            == crate::tools::file_summary::FileSummaryType::Source
    {
        let matches = find_symbols_for_range(text, resolved, start, end);
        if !matches.is_empty() {
            let names: Vec<_> = matches.iter().take(3).map(|s| format!("'{s}'")).collect();
            let mut label = names.join(", ");
            if matches.len() > 3 {
                label.push_str(&format!(" and {} more", matches.len() - 3));
            }
            let first = &matches[0];
            return Err(RecoverableError::with_hint(
                format!("source range overlaps named symbol(s): {label}"),
                format!(
                    "Use symbols(name='{first}', include_body=true) to read the body directly. \
                     Pass force=true to read the raw line range anyway."
                ),
            )
            .into());
        }
    }
```

- [ ] **Step 3: Extract `force` in `call()` and pass it down**

In the `call` method of `impl Tool for ReadFile`, find the line that calls `read_with_line_range` (around line 95–96). It currently looks like:

```rust
        if let (Some(start), Some(end)) = (start_line, end_line) {
            return read_with_line_range(path, &text, &resolved, start, end, &source_tag, ctx);
        }
```

Replace with:

```rust
        let force = input["force"].as_bool().unwrap_or(false);

        if let (Some(start), Some(end)) = (start_line, end_line) {
            return read_with_line_range(path, &text, &resolved, start, end, &source_tag, ctx, force);
        }
```

- [ ] **Step 4: Add `force` to `input_schema`**

In `input_schema()`, add to the `properties` object (after `toml_key`):

```rust
                "force": {
                    "type": "boolean",
                    "description": "Skip source-symbol hint and read the raw line range."
                }
```

- [ ] **Step 5: Update `description()`**

Change the `description` return value from:

```rust
        "Read a file. Large files return a summary + @file_* handle. \
         Format-aware: json_path (JSON), toml_key (TOML/YAML). Use read_markdown for .md files."
```

to:

```rust
        "Read a file. Large files return a summary + @file_* handle. \
         Format-aware: json_path (JSON), toml_key (TOML/YAML). Use read_markdown for .md files. \
         Source files: a start_line+end_line range overlapping a named symbol is redirected \
         to symbols(include_body=true); pass force=true to bypass."
```

- [ ] **Step 6: Run the 4 new tests**

```bash
cargo test read_file_source_range 2>&1 | tail -20
```

Expected: all 4 tests pass.

- [ ] **Step 7: Run the full read_file test suite to check for regressions**

```bash
cargo test read_file 2>&1 | tail -30
```

Expected: all existing tests still pass.

**If `read_file_real_file_range_*` tests fail:** those tests read Rust fixture files with line ranges that now overlap symbol bodies. Add `"force": true` to their input JSON — the gate educates agents, not test infrastructure:
```rust
json!({ "path": path, "start_line": N, "end_line": M, "force": true })
```

---

## Task 5: Final checks and commit

- [ ] **Step 1: Format**

```bash
cargo fmt
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep "^error" | head -20
```

Expected: no errors. Fix any warnings about unused imports or variables.

- [ ] **Step 3: Full test suite**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass. Note the test count in the output.

- [ ] **Step 4: Commit**

```bash
git add src/ast/mod.rs src/tools/read_file.rs src/tools/edit_file/tests.rs
git commit -m "feat(read_file): gate source range reads that overlap named symbols

When read_file is called with start_line+end_line on a source file and the
range strictly overlaps a named symbol body (tree-sitter), return a
RecoverableError naming the symbol and suggesting symbols(include_body=true).
Pass force=true to bypass. Fails open on parse error."
```
