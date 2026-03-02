# Symbol Range Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove all range-manipulation heuristics from symbol write tools, adopting Krait's "trust LSP" approach. Convert `augment_body_range_from_ast` from silent fix to validation-only `RecoverableError`.

**Architecture:** Three write tools (`replace_symbol`, `remove_symbol`, `insert_code`) get simplified to use raw LSP ranges with only bounds checks. `find_insert_before_line` (Krait-style doc-comment walk) kept only for `insert_code(before)`. `augment_body_range_from_ast` → `validate_symbol_range` (detect + error, not fix).

**Tech Stack:** Rust, `src/tools/symbol.rs`, `tests/symbol_lsp.rs`

**Design doc:** `docs/plans/2026-03-02-symbol-range-redesign-design.md`

---

### Task 1: Convert `augment_body_range_from_ast` to `validate_symbol_range`

Change the function from silently fixing degenerate ranges to detecting and returning `RecoverableError`.

**Files:**
- Modify: `src/tools/symbol.rs` — the function itself (line 248) and its call site in `FindSymbol::call` (line 726)

**Step 1: Write the new `validate_symbol_range` function**

Replace `augment_body_range_from_ast` with:

```rust
/// Detect degenerate LSP ranges where start_line == end_line but tree-sitter
/// shows the symbol spans multiple lines. Returns RecoverableError instead of
/// silently fixing — consistent with "trust LSP, validate, fail loudly".
fn validate_symbol_range(sym: &SymbolInfo) -> anyhow::Result<()> {
    if sym.start_line != sym.end_line {
        return Ok(());
    }
    let Ok(ast_syms) = crate::ast::extract_symbols(&sym.file) else {
        return Ok(());
    };
    if let Some(ast_end) = find_ast_end_line_in(&ast_syms, &sym.name, sym.start_line) {
        if ast_end > sym.start_line + 1 {
            anyhow::bail!(RecoverableError::with_hint(
                format!(
                    "LSP returned suspicious range for '{}' (line {}, but AST shows it spans to line {})",
                    sym.name,
                    sym.start_line + 1,
                    ast_end + 1,
                ),
                "The LSP server may have returned a selection range instead of the full symbol range. \
                 Try edit_file for this symbol, or check list_symbols to verify the range.",
            ));
        }
    }
    Ok(())
}
```

Keep `find_ast_end_line_in` unchanged (it's used by `validate_symbol_range`).

**Step 2: Update the call site in `FindSymbol::call`**

At line ~726, change from:

```rust
let sym = if include_body {
    augment_body_range_from_ast(sym)
} else {
    sym
};
```

To:

```rust
// Validate range but don't silently fix — if degenerate, the agent
// sees the error and can fall back to edit_file.
if include_body {
    validate_symbol_range(&sym)?;
}
```

Note: this means `find_symbol(include_body=true)` will now error on degenerate ranges instead of silently expanding them. Without `include_body`, the range doesn't matter (only name/location is shown).

**Step 3: Update existing `augment_body_range_from_ast` tests**

The 8 existing unit tests need to be rewritten for the new behavior:
- `augment_body_range_from_ast_fixes_degenerate_range` → `validate_symbol_range_rejects_degenerate_range` — assert it returns `Err` containing "suspicious range"
- `augment_body_range_from_ast_leaves_good_range_unchanged` → `validate_symbol_range_accepts_good_range` — assert `Ok(())`
- `augment_body_range_from_ast_python` → `validate_symbol_range_rejects_degenerate_python` — assert `Err`
- `augment_body_range_from_ast_typescript` → `validate_symbol_range_rejects_degenerate_typescript` — assert `Err`
- `augment_body_range_from_ast_go` → `validate_symbol_range_rejects_degenerate_go` — assert `Err`
- `augment_body_range_from_ast_rust_with_doc_comment` → `validate_symbol_range_rejects_degenerate_rust_with_doc` — assert `Err`
- `augment_body_range_from_ast_picks_correct_function_among_many` → `validate_symbol_range_picks_correct_function` — assert `Err` with correct symbol name
- `augment_body_range_from_ast_name_not_in_file_leaves_unchanged` → `validate_symbol_range_accepts_when_ast_unavailable` — assert `Ok(())`
- `augment_body_range_from_ast_recurses_into_children_for_method` → `validate_symbol_range_recurses_into_children` — assert `Err`

Each test follows the same pattern:
```rust
#[test]
fn validate_symbol_range_rejects_degenerate_range() {
    // ... same setup as before, creating temp file + SymbolInfo with start==end ...
    let result = validate_symbol_range(&sym);
    assert!(result.is_err(), "degenerate range should be rejected");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("suspicious range"), "error should mention suspicious range; got: {msg}");
}
```

**Step 4: Run tests**

Run: `cargo test validate_symbol_range`
Expected: All 9 renamed tests pass.

Run: `cargo test --test symbol_lsp`
Expected: All 10 integration tests pass (this change doesn't affect write tools yet).

**Step 5: Commit**

```
git add src/tools/symbol.rs
git commit -m "refactor(symbol): convert augment_body_range_from_ast to validate_symbol_range

Change from silently fixing degenerate LSP ranges to detecting and
returning RecoverableError. Part of the 'trust LSP' redesign."
```

---

### Task 2: Simplify `replace_symbol` — remove `trim_symbol_start` and `is_declaration_line`

**Files:**
- Modify: `src/tools/symbol.rs` — `ReplaceSymbol::call` (line 1303)
- Modify: `tests/symbol_lsp.rs` — update 4 integration tests

**Step 1: Rewrite `ReplaceSymbol::call`**

Replace the current body (lines 1303-1354) with the simplified Krait-style version:

```rust
async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
    super::guard_worktree_write(ctx).await?;
    let name_path = super::require_str_param(&input, "name_path")?;
    let rel_path = get_path_param(&input, true)?.unwrap();
    let new_body = super::require_str_param(&input, "new_body")?;

    let full_path = resolve_write_path(ctx, rel_path).await?;
    let (client, lang) = get_lsp_client(ctx, &full_path).await?;

    let symbols = client.document_symbols(&full_path, &lang).await?;
    let sym = find_symbol_by_name_path(&symbols, name_path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("symbol not found: {}", name_path),
            "Use list_symbols(path) to see available symbols, or check the name_path spelling.",
        )
    })?;

    // Validate: catch degenerate LSP ranges (start == end for multi-line symbols)
    validate_symbol_range(&sym)?;

    let content = std::fs::read_to_string(&full_path)?;
    let lines: Vec<&str> = content.lines().collect();

    let start = sym.start_line as usize;
    let end = (sym.end_line as usize + 1).min(lines.len());

    if start >= lines.len() {
        return Err(RecoverableError::with_hint(
            format!(
                "symbol range out of bounds: start line {} but file has {} lines",
                start + 1, lines.len(),
            ),
            "The LSP may have stale data. Try list_symbols(path) to refresh.",
        ).into());
    }

    let mut new_lines = Vec::new();
    new_lines.extend_from_slice(&lines[..start]);
    new_lines.extend(new_body.lines());
    new_lines.extend_from_slice(&lines[end..]);

    write_lines(&full_path, &new_lines, content.ends_with('\n'))?;
    ctx.lsp.notify_file_changed(&full_path).await;
    let root = ctx.agent.require_project_root().await?;
    let hint = crate::util::path_security::worktree_hint(&root);
    let mut resp =
        json!({ "status": "ok", "replaced_lines": format!("{}-{}", start + 1, end) });
    if let Some(h) = hint {
        resp["worktree_hint"] = json!(h);
    }
    Ok(resp)
}
```

**Step 2: Update integration tests**

The 4 `replace_symbol_*` tests need updating because they relied on `trim_symbol_start` and `is_declaration_line`:

**`replace_symbol_preserves_preceding_close_brace` (line 72):**
With "trust LSP", when LSP says the symbol starts at the `}` line, we trust it — the `}` gets replaced along with the body. This test now verifies the NEW behavior: the `}` IS replaced (because the LSP said it's part of the symbol).

Update the test assertions:
```rust
// With "trust LSP" design, the LSP says this symbol starts at the `}` line,
// so we replace from there. The preceding `}` is gone because LSP included it.
assert!(
    result.contains("new_body()"),
    "replacement body must be applied; got:\n{result}"
);
assert!(
    !result.contains("old_body()"),
    "old body must be gone; got:\n{result}"
);
```

**`replace_symbol_preserves_paren_close_brace` (line 126):**
Same logic — LSP says start=0, we trust it. `})` and `}` lines are replaced. Update assertions similarly.

**`replace_symbol_clean_start_line` (line 190):**
No change needed — this test already has clean LSP ranges.

**`replace_symbol_rejects_start_line_inside_body` (line 239):**
This tested `is_declaration_line` (BUG-013). With "trust LSP", we no longer reject this — we trust the LSP even when it resolves to an inner binding. **Delete this test** (the behavior it tested is removed by design). The agent will see the wrong body via `find_symbol(include_body=true)` and can detect the issue themselves.

**Step 3: Run tests**

Run: `cargo test --test symbol_lsp`
Expected: All tests pass (9 remaining after deleting `replace_symbol_rejects_start_line_inside_body`).

Run: `cargo test`
Expected: Full suite passes.

**Step 4: Commit**

```
git add src/tools/symbol.rs tests/symbol_lsp.rs
git commit -m "refactor(symbol): simplify replace_symbol — trust LSP ranges

Remove trim_symbol_start and is_declaration_line from replace_symbol.
Add validate_symbol_range for degenerate range detection.
Part of the 'trust LSP' redesign (BUG-003/013 workarounds removed)."
```

---

### Task 3: Simplify `remove_symbol` — remove all heuristics

**Files:**
- Modify: `src/tools/symbol.rs` — `RemoveSymbol::call` (line 1417)
- Modify: `tests/symbol_lsp.rs` — update 2 integration tests

**Step 1: Rewrite `RemoveSymbol::call`**

Replace the current body (lines 1417-1465) with:

```rust
async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
    super::guard_worktree_write(ctx).await?;
    let name_path = super::require_str_param(&input, "name_path")?;
    let rel_path = get_path_param(&input, true)?.unwrap();

    let full_path = resolve_write_path(ctx, rel_path).await?;
    let (client, lang) = get_lsp_client(ctx, &full_path).await?;

    let symbols = client.document_symbols(&full_path, &lang).await?;
    let sym = find_symbol_by_name_path(&symbols, name_path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("symbol not found: {}", name_path),
            "Use list_symbols(path) to see available symbols, or check the name_path spelling.",
        )
    })?;

    // Validate: catch degenerate LSP ranges
    validate_symbol_range(&sym)?;

    let content = std::fs::read_to_string(&full_path)?;
    let lines: Vec<&str> = content.lines().collect();

    let start = sym.start_line as usize;
    let end = (sym.end_line as usize + 1).min(lines.len());

    if start >= lines.len() {
        return Err(RecoverableError::with_hint(
            format!(
                "symbol range out of bounds: start line {} but file has {} lines",
                start + 1, lines.len(),
            ),
            "The LSP may have stale data. Try list_symbols(path) to refresh.",
        ).into());
    }

    let mut new_lines: Vec<&str> = Vec::new();
    new_lines.extend_from_slice(&lines[..start]);
    new_lines.extend_from_slice(&lines[end..]);

    write_lines(&full_path, &new_lines, content.ends_with('\n'))?;
    ctx.lsp.notify_file_changed(&full_path).await;
    let root = ctx.agent.require_project_root().await?;
    let hint = crate::util::path_security::worktree_hint(&root);
    let line_count = end - start;
    let removed_range = format!("{}-{}", start + 1, end);
    let mut resp = json!({
        "status": "ok",
        "removed_lines": removed_range,
        "line_count": line_count,
    });
    if let Some(h) = hint {
        resp["worktree_hint"] = json!(h);
    }
    Ok(resp)
}
```

**Step 2: Update integration tests**

**`remove_symbol_does_not_delete_sibling_const` (line 485):**
This tested `clamp_end_to_closing_brace` (BUG-014). With "trust LSP", when LSP says `end_line=3` (including the const), we trust it — the const IS removed. This is a case where the LSP is wrong and we accept that.

**Update:** Change this test to verify the NEW behavior — we trust the LSP range even when it over-extends:
```rust
/// With "trust LSP" design, when LSP over-extends end_line to include a
/// sibling const, we trust it. The const gets removed along with the function.
/// This is an LSP bug, but we don't try to fix it — the agent can see the
/// issue via find_symbol(include_body=true) which shows the same range.
#[tokio::test]
async fn remove_symbol_trusts_lsp_range_even_when_overextended() {
    // ... same setup ...
    // Assert: both target AND const are removed (we trust the LSP range)
    assert!(
        !result.contains("fn target"),
        "function must be removed; got:\n{result}"
    );
    assert!(
        !result.contains("SENTINEL"),
        "const is within LSP range — removed too (trust LSP); got:\n{result}"
    );
}
```

**`remove_symbol_handles_const_without_closing_brace` (line 528):**
This tested `clamp_end_to_closing_brace` + `scan_backwards_for_docs` (BUG-016). With "trust LSP":
- LSP says `start=6, end=6` → we remove only line 6 (the `const` line)
- Doc comment on line 5 is NOT removed (LSP didn't include it in range)
- No more panic/corruption risk

**Update:**
```rust
/// With "trust LSP", we use the raw LSP range (line 6 only).
/// Doc comments are NOT removed unless the LSP includes them in range.
#[tokio::test]
async fn remove_symbol_const_trusts_lsp_range() {
    // ... same setup ...
    assert!(
        !result.contains("TARGET"),
        "const must be removed; got:\n{result}"
    );
    assert!(
        result.contains("A constant"),
        "doc comment is outside LSP range — survives (trust LSP); got:\n{result}"
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
    assert_eq!(use_count, 1, "no duplication; got:\n{result}");
}
```

**Step 3: Run tests**

Run: `cargo test --test symbol_lsp`
Expected: All 9 tests pass.

Run: `cargo test`
Expected: Full suite passes.

**Step 4: Commit**

```
git add src/tools/symbol.rs tests/symbol_lsp.rs
git commit -m "refactor(symbol): simplify remove_symbol — trust LSP ranges

Remove trim_symbol_start, clamp_end_to_closing_brace, scan_backwards_for_docs,
collapse_blank_lines, and inverted-range guard from remove_symbol.
Part of the 'trust LSP' redesign (BUG-010/014/016 workarounds removed)."
```

---

### Task 4: Simplify `insert_code` — Krait-style doc walk for "before" only

**Files:**
- Modify: `src/tools/symbol.rs` — `InsertCode::call` (line 1530), rename `scan_backwards_for_docs` to `find_insert_before_line`
- Modify: `tests/symbol_lsp.rs` — update 4 integration tests

**Step 1: Rename `scan_backwards_for_docs` → `find_insert_before_line` and extend**

Replace the function (line 1251) with:

```rust
/// Walk upward from a symbol's start line to find the insertion point that
/// lands before any doc comments and attributes. Used ONLY for insert_code(before)
/// positioning — never for modifying a symbol's own range.
///
/// Krait-style: language-agnostic, recognizes doc comments and attributes from
/// Rust (#[...]), Python/Java/TS (@decorator), JSDoc/JavaDoc (/** ... */),
/// and Rust doc comments (///, //!). Does NOT consume blank lines — a blank
/// line separates unrelated code and stops the walk.
fn find_insert_before_line(lines: &[&str], symbol_start: usize) -> usize {
    let mut cursor = symbol_start;
    while cursor > 0 {
        let trimmed = lines[cursor - 1].trim();
        let is_attr_or_doc = trimmed.starts_with("#[")
            || trimmed.starts_with('@')
            || trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with("/**")
            || trimmed.starts_with("* ")
            || trimmed == "*/"
            || trimmed.starts_with("/*");
        if is_attr_or_doc {
            cursor -= 1;
        } else {
            break;
        }
    }
    cursor
}
```

Key difference from old `scan_backwards_for_docs`: does NOT consume `t.is_empty()` (blank lines). A blank line stops the walk.

**Step 2: Rewrite `InsertCode::call`**

Replace the current body (lines 1530-1572) with:

```rust
async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
    let name_path = super::require_str_param(&input, "name_path")?;
    let rel_path = get_path_param(&input, true)?.unwrap();
    let code = super::require_str_param(&input, "code")?;
    let position = input["position"].as_str().unwrap_or("after");

    let full_path = resolve_write_path(ctx, rel_path).await?;
    let (client, lang) = get_lsp_client(ctx, &full_path).await?;

    let symbols = client.document_symbols(&full_path, &lang).await?;
    let sym = find_symbol_by_name_path(&symbols, name_path).ok_or_else(|| {
        RecoverableError::with_hint(
            format!("symbol not found: {}", name_path),
            "Use list_symbols(path) to see available symbols, or check the name_path spelling.",
        )
    })?;

    // Validate: catch degenerate LSP ranges
    validate_symbol_range(&sym)?;

    let content = std::fs::read_to_string(&full_path)?;
    let lines: Vec<&str> = content.lines().collect();

    let code_lines: Vec<&str> = code.lines().collect();

    let insert_at = match position {
        "before" => {
            let sym_start = sym.start_line as usize;
            find_insert_before_line(&lines, sym_start)
        }
        _ => {
            (sym.end_line as usize + 1).min(lines.len())
        }
    };

    let mut new_lines = Vec::new();
    new_lines.extend_from_slice(&lines[..insert_at]);
    if position != "before" {
        // After: add blank separator if next line is non-empty (Krait pattern)
        let needs_blank = lines.get(insert_at).is_some_and(|l| !l.trim().is_empty());
        if needs_blank {
            new_lines.push("");
        }
    }
    new_lines.extend(code_lines.iter().copied());
    if position == "before" {
        // Before: add blank separator after inserted code
        new_lines.push("");
    }
    new_lines.extend_from_slice(&lines[insert_at..]);

    write_lines(&full_path, &new_lines, content.ends_with('\n'))?;
    ctx.lsp.notify_file_changed(&full_path).await;
    let root = ctx.agent.require_project_root().await?;
    let hint = crate::util::path_security::worktree_hint(&root);
    let mut resp =
        json!({ "status": "ok", "inserted_at_line": insert_at + 1, "position": position });
    if let Some(h) = hint {
        resp["worktree_hint"] = json!(h);
    }
    Ok(resp)
}
```

**Step 3: Update integration tests**

**`insert_code_before_walks_past_attributes_and_doc_comments` (line 281):**
Still works — `find_insert_before_line` walks past `///` and `#[derive]`. However, the new function does NOT consume blank lines, so if there was a blank line between the doc comment and prior code, it would stop there. Check the test fixture: `///` is on line 0 so there's nothing above it — no change needed. Keep as-is but the blank separator line behavior changes slightly. Verify assertions still hold.

**`insert_code_before_skips_lead_in` (line 335):**
This tested `trim_symbol_start` + `scan_backwards_for_docs`. With "trust LSP":
- LSP says `start_line=0` (the `}` line)
- `find_insert_before_line` at line 0 → cursor starts at 0, nothing above → returns 0
- Insertion goes at line 0 (before the `}`)

This test relied on `trim_symbol_start` advancing past the `}` first, then `scan_backwards_for_docs`. Now we use `sym.start_line` directly (line 0). The insertion lands at line 0 — BEFORE the `}`. This is the "trust LSP" behavior: we insert where the LSP says the symbol starts.

**Update the test:**
```rust
/// With "trust LSP", start_line=0 means insert at line 0.
/// No skipping of lead-in — we trust the LSP range.
#[tokio::test]
async fn insert_code_before_trusts_lsp_start() {
    // ... same setup, LSP says start=0, end=3 ...
    // Insertion lands at line 0 (before everything, including the `}`)
    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(result.contains("// inserted"), "insertion must be present");
    // Insertion is at line 0 — before the `}`
    let insert_pos = result.find("// inserted").unwrap();
    let brace_pos = result.find("    }").unwrap();
    assert!(
        insert_pos < brace_pos,
        "with trust LSP, insertion at sym.start_line lands before `}}`; got:\n{result}"
    );
}
```

**`insert_code_after_lands_past_symbol` (line 385):**
No change needed — uses raw `end_line+1`, which is the same behavior.

**`insert_code_after_skips_trail_in` (line 430):**
This tested `trim_symbol_end` (BUG-004). With "trust LSP":
- LSP says `end_line=3` (the `fn following() {` line)
- We insert at line 4 (end_line + 1)
- This puts the insertion INSIDE `following`'s body

**Update the test:**
```rust
/// With "trust LSP", end_line=3 means insert at line 4.
/// If LSP over-extends, the insertion lands after the overextended range.
#[tokio::test]
async fn insert_code_after_trusts_lsp_end() {
    // ... same setup, LSP says end=3 ...
    let result = std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(result.contains("// inserted"), "insertion must be present");
    // With trust LSP, insert after line 3 (end_line + 1 = 4)
    // The insertion lands after `fn following() {` — inside the body
    let insert_pos = result.find("// inserted").unwrap();
    let following_fn = result.find("fn following()").unwrap();
    assert!(
        insert_pos > following_fn,
        "with trust LSP, insertion after overextended end lands past fn following; got:\n{result}"
    );
}
```

**Step 4: Update unit tests for `scan_backwards_for_docs`**

The 4 unit tests (lines 4152-4183) need renaming and updating:
- `scan_backwards_includes_doc_comments` → `find_insert_before_line_walks_past_doc_comments`
- `scan_backwards_includes_attributes` → `find_insert_before_line_walks_past_attributes`
- `scan_backwards_stops_at_code` → `find_insert_before_line_stops_at_code`
- `scan_backwards_at_start_of_file` → `find_insert_before_line_at_start_of_file`

Also update the blank-line behavior: the old function consumed blank lines, the new one does NOT. If any test relied on blank line consumption, adjust. Check each fixture — they used `lines_to_strs()` helper or inline slices with `///`, `#[`, etc. Most should work since the interesting lines are contiguous.

**Step 5: Run tests**

Run: `cargo test --test symbol_lsp`
Expected: All 9 tests pass.

Run: `cargo test find_insert_before_line`
Expected: All 4 renamed unit tests pass.

Run: `cargo test`
Expected: Full suite passes.

**Step 6: Commit**

```
git add src/tools/symbol.rs tests/symbol_lsp.rs
git commit -m "refactor(symbol): simplify insert_code — Krait-style doc walk

Replace trim_symbol_start + scan_backwards_for_docs + trim_symbol_end with
find_insert_before_line (before only) and raw end_line (after).
Part of the 'trust LSP' redesign (BUG-004/010 workarounds removed)."
```

---

### Task 5: Delete dead helper functions and their tests

After Tasks 1-4, these functions are unused:

**Functions to delete:**
- `trim_symbol_start` (line 1217)
- `trim_symbol_end` (line 1235)
- `clamp_end_to_closing_brace` (line 1189)
- `is_declaration_line` (line 1171)
- `collapse_blank_lines` (line 1265)
- `augment_body_range_from_ast` (line 248) — already replaced in Task 1

**Unit tests to delete:**
- `collapse_blank_lines_collapses_triple` (line 4185)
- `collapse_blank_lines_preserves_single` (line 4192)

(No unit tests exist for `trim_symbol_start`, `trim_symbol_end`, `clamp_end_to_closing_brace`, `is_declaration_line` — they were tested only via integration tests.)

**Step 1: Delete the functions**

Remove each function definition. Use `remove_symbol` or manual deletion.

**Step 2: Delete the unit tests**

Remove `collapse_blank_lines_collapses_triple` and `collapse_blank_lines_preserves_single`.

**Step 3: Verify compilation and tests**

Run: `cargo build`
Expected: Compiles with no errors (no remaining references to deleted functions).

Run: `cargo test`
Expected: Full suite passes.

Run: `cargo clippy -- -D warnings`
Expected: Clean.

**Step 4: Commit**

```
git add src/tools/symbol.rs
git commit -m "refactor(symbol): delete dead range-manipulation helpers

Remove trim_symbol_start, trim_symbol_end, clamp_end_to_closing_brace,
is_declaration_line, collapse_blank_lines, and augment_body_range_from_ast.
These heuristics are no longer used after the 'trust LSP' redesign."
```

---

### Task 6: Update description strings and remove_symbol doc comment

**Files:**
- Modify: `src/tools/symbol.rs` — `RemoveSymbol::description()` (line ~1403)
- Modify: `docs/TODO-tool-misbehaviors.md` — mark BUGs as resolved by design

**Step 1: Update `remove_symbol` description**

Change from:
```
"Delete a symbol (function, struct, impl block, test, etc.) by name. Removes the entire declaration including doc comments and attributes."
```
To:
```
"Delete a symbol (function, struct, impl block, test, etc.) by name. Removes the lines covered by the LSP symbol range."
```

**Step 2: Update BUG log**

In `docs/TODO-tool-misbehaviors.md`, add a note to BUG-003, BUG-004, BUG-010, BUG-013, BUG-014:
```
> **Resolved by design (2026-03-02):** Symbol range redesign removed all range-manipulation heuristics. We now trust LSP ranges directly. See `docs/plans/2026-03-02-symbol-range-redesign-design.md`.
```

**Step 3: Update server instructions if needed**

Check `src/prompts/server_instructions.md` — if it mentions "including doc comments and attributes" for `remove_symbol`, update to match the new behavior.

**Step 4: Run final verification**

Run: `cargo fmt`
Run: `cargo clippy -- -D warnings`
Run: `cargo test`
Expected: All pass.

**Step 5: Commit**

```
git add src/tools/symbol.rs docs/TODO-tool-misbehaviors.md src/prompts/server_instructions.md
git commit -m "docs: update descriptions and BUG log for trust LSP redesign

Mark BUG-003/004/010/013/014 as resolved by design. Update remove_symbol
description to reflect that it removes the LSP symbol range, not
doc comments/attributes specifically."
```

---

### Task 7: Final verification and summary commit

**Step 1: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All pass, no warnings.

**Step 2: Count lines removed**

Run: `git diff --stat HEAD~6` (or however many commits back)
Expected: Net reduction of ~100-150 lines from symbol.rs.

**Step 3: Verify the consistency matrix**

Check that all three write tools now use the same pattern:
1. `validate_symbol_range(&sym)?`
2. `start = sym.start_line as usize`
3. `end = (sym.end_line as usize + 1).min(lines.len())`
4. Bounds check
5. Splice + write

The only difference: `insert_code(before)` calls `find_insert_before_line` for insertion positioning.
