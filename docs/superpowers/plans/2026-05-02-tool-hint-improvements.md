# Tool Hint Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface advisory hints in `grep`, `read_file`, and the `run_command` source guard so agents are nudged toward `symbols`/`references`/`call_graph` when those tools would be more structured.

**Architecture:** Four independent, self-contained changes. A shared `is_identifier_pattern` helper in `path_security.rs` (already imported by grep and run_command paths) is extracted first and reused by the subsequent three changes. No new modules.

**Tech Stack:** Rust, serde_json, existing `file_summary::detect_file_type`, existing `path_security::check_source_file_access`.

---

## File Map

| File | Change |
|------|--------|
| `src/util/path_security.rs` | Add `is_identifier_pattern`, `extract_grep_pattern`, add `"grep"` to `SOURCE_ACCESS_COMMANDS`, add grep arm to `check_source_file_access` match |
| `src/tools/grep.rs` | Add `suggestion` field when pattern is identifier-like |
| `src/tools/read_file.rs` | Add `hint` field for small source file inline reads |
| `src/tools/edit_file/tests.rs` | New tests for grep suggestion + read_file source hint (all tool tests live here) |

---

## Task 1: `is_identifier_pattern` helper

**Files:**
- Modify: `src/util/path_security.rs`

- [ ] **Step 1: Write the failing test**

Add inside the existing `#[cfg(test)]` block in `src/util/path_security.rs`:

```rust
#[test]
fn is_identifier_pattern_accepts_single() {
    assert!(is_identifier_pattern("WriteMemory"));
    assert!(is_identifier_pattern("snake_case"));
    assert!(is_identifier_pattern("_private"));
    assert!(is_identifier_pattern("CamelCase123"));
}

#[test]
fn is_identifier_pattern_accepts_pipe_alternation() {
    assert!(is_identifier_pattern("WriteMemory|ReadMemory|ListMemories"));
}

#[test]
fn is_identifier_pattern_rejects_regex_and_empty() {
    assert!(!is_identifier_pattern(""));
    assert!(!is_identifier_pattern("foo.*bar"));
    assert!(!is_identifier_pattern("^start"));
    assert!(!is_identifier_pattern("foo(bar)"));
    assert!(!is_identifier_pattern("foo[0-9]"));
    assert!(!is_identifier_pattern("||")); // empty parts
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test --lib is_identifier_pattern 2>&1 | tail -5
```
Expected: `error[E0425]: cannot find function 'is_identifier_pattern'`

- [ ] **Step 3: Implement the helper**

Add after the `check_source_file_access` function (around line 610) in `src/util/path_security.rs`:

```rust
/// Returns true if `s` is a plain identifier or pipe-alternation of identifiers.
/// Used to decide whether to suggest symbol tools instead of grep.
pub fn is_identifier_pattern(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.split('|').all(|part| {
        if part.is_empty() {
            return false;
        }
        let mut chars = part.chars();
        match chars.next() {
            Some(c) if c.is_alphabetic() || c == '_' => {}
            _ => return false,
        }
        chars.all(|c| c.is_alphanumeric() || c == '_')
    })
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test --lib is_identifier_pattern 2>&1 | tail -5
```
Expected: `test result: ok. 3 passed`

- [ ] **Step 5: Clippy + commit**

```bash
cargo fmt && cargo clippy --lib -- -D warnings 2>&1 | tail -3
git add src/util/path_security.rs
git commit -m "feat(path_security): add is_identifier_pattern helper"
```

---

## Task 2: `run_command` guard — grep arm

**Files:**
- Modify: `src/util/path_security.rs`

- [ ] **Step 1: Write the failing tests**

Add inside the `#[cfg(test)]` block in `src/util/path_security.rs`:

```rust
#[test]
fn grep_on_source_with_identifier_gives_symbol_ladder() {
    let hint = check_source_file_access("grep WriteMemory src/tools/memory.rs").unwrap();
    assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
    assert!(hint.contains("references(symbol='WriteMemory')"), "got: {hint}");
    assert!(hint.contains("call_graph(symbol='WriteMemory'"), "got: {hint}");
}

#[test]
fn grep_on_source_with_regex_gives_generic_hint() {
    let hint = check_source_file_access("grep 'foo.*bar' src/main.rs").unwrap();
    assert!(hint.contains("grep(pattern"), "got: {hint}");
    // must NOT show symbol ladder for regex patterns
    assert!(!hint.contains("call_graph"), "got: {hint}");
}

#[test]
fn grep_pipe_alternation_uses_first_part_in_hint() {
    let hint =
        check_source_file_access("grep 'WriteMemory|ReadMemory' src/tools/memory.rs").unwrap();
    assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test --lib grep_on_source 2>&1 | tail -5
```
Expected: `FAILED` (grep not yet in SOURCE_ACCESS_COMMANDS)

- [ ] **Step 3: Add grep to SOURCE_ACCESS_COMMANDS**

In `src/util/path_security.rs`, find line ~480:
```rust
const SOURCE_ACCESS_COMMANDS: &str = r"\b(cat|head|tail|sed|awk|less|more|wc)\b";
```
Change to:
```rust
const SOURCE_ACCESS_COMMANDS: &str = r"\b(cat|head|tail|sed|awk|less|more|wc|grep)\b";
```

- [ ] **Step 4: Add `extract_grep_pattern` helper**

Add just before `check_source_file_access`:

```rust
/// Extracts the pattern argument from a grep shell segment.
/// Skips the command name and any flag tokens (starting with `-`).
fn extract_grep_pattern(segment: &str) -> Option<&str> {
    segment
        .split_whitespace()
        .skip(1)
        .find(|t| !t.starts_with('-'))
        .map(|t| t.trim_matches('"').trim_matches('\''))
}
```

- [ ] **Step 5: Add grep arm to the match and change return type**

In `check_source_file_access`, the current `match first_cmd` returns `&str` literals and calls `.to_string()` at the end. Change it to return `String` directly so the grep arm can interpolate the symbol name:

Find:
```rust
    let hint = match first_cmd {
        "sed" | "awk" => {
            "use read_file(path, start_line, end_line), symbols(path), \
             symbols(name=..., include_body=true), or grep(regex) instead. \
             Re-run with acknowledge_risk: true if you need raw shell access."
        }
        _ => {
            "use read_file(path, start_line, end_line) or symbols(path) + \
             symbols(name=..., include_body=true) instead. \
             Re-run with acknowledge_risk: true if you need raw shell access."
        }
    };

    Some(hint.to_string())
```

Replace with:
```rust
    let hint: String = match first_cmd {
        "grep" => {
            let pat = extract_grep_pattern(blocked.as_str()).unwrap_or("");
            if is_identifier_pattern(pat) {
                let name = pat.split('|').next().unwrap_or(pat);
                format!(
                    "use symbols(name='{name}') for declarations, \
                     references(symbol='{name}') for direct callers, \
                     call_graph(symbol='{name}', direction='callers') for transitive blast radius. \
                     Re-run with acknowledge_risk: true if you need raw shell grep."
                )
            } else {
                "use grep(pattern, path) codescout tool instead. \
                 Re-run with acknowledge_risk: true if you need raw shell access."
                    .to_string()
            }
        }
        "sed" | "awk" => {
            "use read_file(path, start_line, end_line), symbols(path), \
             symbols(name=..., include_body=true), or grep(regex) instead. \
             Re-run with acknowledge_risk: true if you need raw shell access."
                .to_string()
        }
        _ => {
            "use read_file(path, start_line, end_line) or symbols(path) + \
             symbols(name=..., include_body=true) instead. \
             Re-run with acknowledge_risk: true if you need raw shell access."
                .to_string()
        }
    };

    Some(hint)
```

- [ ] **Step 6: Run tests to confirm they pass**

```bash
cargo test --lib grep_on_source 2>&1 | tail -5
cargo test --lib grep_pipe_alternation 2>&1 | tail -5
```
Expected: `test result: ok. 3 passed`

Also verify existing tests still pass:
```bash
cargo test --lib source_file_access_hint 2>&1 | tail -5
```
Expected: `test result: ok. 2 passed`

- [ ] **Step 7: Clippy + commit**

```bash
cargo fmt && cargo clippy --lib -- -D warnings 2>&1 | tail -3
git add src/util/path_security.rs
git commit -m "feat(path_security): block grep-on-source with 3-tier symbol hint"
```

---

## Task 3: `grep` MCP tool — advisory suggestion field

**Files:**
- Modify: `src/tools/grep.rs`
- Modify: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Write the failing tests**

Add to `src/tools/edit_file/tests.rs` (near the existing grep tests around line 777):

```rust
#[tokio::test]
async fn grep_identifier_pattern_adds_suggestion() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("code.rs"),
        "pub struct WriteMemory;\nimpl WriteMemory {}\n",
    )
    .unwrap();

    let result = Grep
        .call(
            json!({ "pattern": "WriteMemory", "path": dir.path().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();

    let suggestion = result["suggestion"].as_str().expect("suggestion field missing");
    assert!(suggestion.contains("symbols(name='WriteMemory')"), "got: {suggestion}");
    assert!(suggestion.contains("references(symbol='WriteMemory')"), "got: {suggestion}");
    assert!(suggestion.contains("call_graph(symbol='WriteMemory'"), "got: {suggestion}");
}

#[tokio::test]
async fn grep_regex_pattern_no_suggestion() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("code.rs"), "fn main() {}\n").unwrap();

    let result = Grep
        .call(
            json!({ "pattern": "fn.*main", "path": dir.path().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.get("suggestion").is_none(), "regex pattern should not add suggestion");
}

#[tokio::test]
async fn grep_pipe_alternation_suggestion_uses_first_part() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("code.rs"),
        "struct WriteMemory;\nstruct ReadMemory;\n",
    )
    .unwrap();

    let result = Grep
        .call(
            json!({ "pattern": "WriteMemory|ReadMemory", "path": dir.path().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();

    let suggestion = result["suggestion"].as_str().expect("suggestion field missing");
    assert!(suggestion.contains("symbols(name='WriteMemory')"), "got: {suggestion}");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test --lib grep_identifier_pattern_adds_suggestion 2>&1 | tail -5
```
Expected: `FAILED` with `suggestion field missing`

- [ ] **Step 3: Add suggestion field to `grep.rs`**

In `src/tools/grep.rs`, in `impl Tool for Grep/call`, find the final block just before `Ok(result)`:

```rust
        if is_literal_fallback {
            result["mode"] = json!("literal_fallback");
            result["reason"] = json!("pattern was not valid regex — searched as literal text");
        }
        Ok(result)
```

Replace with:

```rust
        if is_literal_fallback {
            result["mode"] = json!("literal_fallback");
            result["reason"] = json!("pattern was not valid regex — searched as literal text");
        }
        if crate::util::path_security::is_identifier_pattern(pattern) {
            let name = pattern.split('|').next().unwrap_or(pattern);
            result["suggestion"] = json!(format!(
                "Pattern looks like a symbol name. Consider: \
                 symbols(name='{name}') for declarations, \
                 references(symbol='{name}') for direct callers, \
                 call_graph(symbol='{name}', direction='callers') for transitive blast radius."
            ));
        }
        Ok(result)
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test --lib grep_identifier_pattern 2>&1 | tail -5
cargo test --lib grep_pipe_alternation_suggestion 2>&1 | tail -5
cargo test --lib grep_regex_pattern_no_suggestion 2>&1 | tail -5
```
Expected: `test result: ok. 3 passed` for all three

- [ ] **Step 5: Clippy + commit**

```bash
cargo fmt && cargo clippy --lib -- -D warnings 2>&1 | tail -3
git add src/tools/grep.rs src/tools/edit_file/tests.rs
git commit -m "feat(grep): advisory suggestion for identifier-like patterns"
```

---

## Task 4: `read_file` — source hint for small files

**Files:**
- Modify: `src/tools/read_file.rs`
- Modify: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Write the failing tests**

Add to `src/tools/edit_file/tests.rs` (near the existing `read_file_small_file_returns_content_directly` test around line 1568):

```rust
#[tokio::test]
async fn read_file_small_source_file_has_hint() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("small.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();

    let result = ReadFile
        .call(json!({ "path": path.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let hint = result["hint"].as_str().expect("hint field missing for source file");
    assert!(hint.contains("symbols(path)"), "got: {hint}");
    assert!(hint.contains("include_body=true"), "got: {hint}");
}

#[tokio::test]
async fn read_file_small_non_source_file_no_hint() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("config.json");
    std::fs::write(&path, "{\"key\": \"value\"}\n").unwrap();

    let result = ReadFile
        .call(json!({ "path": path.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert!(result.get("hint").is_none(), "non-source file should not have hint");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test --lib read_file_small_source_file_has_hint 2>&1 | tail -5
```
Expected: `FAILED` with `hint field missing for source file`

- [ ] **Step 3: Add hint in `read_full_file`**

In `src/tools/read_file.rs`, in `read_full_file`, find the final inline-return block:

```rust
    let mut result = json!({ "content": text, "total_lines": total_lines });
    if source_tag != "project" {
        result["source"] = json!(source_tag);
    }
    if let Some(c) = md_cov {
        result["coverage"] = c;
    }
    Ok(result)
```

Replace with:

```rust
    let mut result = json!({ "content": text, "total_lines": total_lines });
    if source_tag != "project" {
        result["source"] = json!(source_tag);
    }
    if crate::tools::file_summary::detect_file_type(&resolved.to_string_lossy())
        == crate::tools::file_summary::FileSummaryType::Source
    {
        result["hint"] = json!(
            "Source file — prefer symbols(path) for overview, \
             symbols(name='...', include_body=true) for specific functions."
        );
    }
    if let Some(c) = md_cov {
        result["coverage"] = c;
    }
    Ok(result)
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test --lib read_file_small_source_file_has_hint 2>&1 | tail -5
cargo test --lib read_file_small_non_source_file_no_hint 2>&1 | tail -5
```
Expected: `test result: ok. 2 passed` for both

- [ ] **Step 5: Full test suite + clippy**

```bash
cargo fmt && cargo clippy --lib -- -D warnings 2>&1 | tail -3
cargo test --lib 2>&1 | tail -5
```
Expected: all tests pass, no warnings

- [ ] **Step 6: Commit**

```bash
git add src/tools/read_file.rs src/tools/edit_file/tests.rs
git commit -m "feat(read_file): hint toward symbol tools for small source files"
```
