# Progressive Disclosure & Token Efficiency Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an OutputGuard system so tools never produce unbounded output, default to compact "exploring" mode, and support paginated "focused" mode — then update server instructions to teach the LLM optimal tool selection.

**Architecture:** New `src/tools/output.rs` module with `OutputGuard` struct parsed from tool input JSON. Each unbounded tool constructs a guard and uses its helpers for capping, pagination, and overflow messages. Server instructions rewritten with tool selection decision tree and output mode documentation.

**Tech Stack:** Rust, serde_json, existing Tool trait pattern

---

### Task 1: Create OutputGuard module with tests

**Files:**
- Create: `src/tools/output.rs`
- Modify: `src/tools/mod.rs` (add `pub mod output;`)

**Step 1: Write the failing tests**

Add this to `src/tools/output.rs`:

```rust
//! Output guardrails: progressive disclosure via exploring/focused modes.
//!
//! Tools that can produce unbounded output construct an `OutputGuard` from
//! their input JSON.  The guard provides helpers for capping results,
//! controlling detail level, and producing standardized overflow messages.

use serde_json::{json, Value};

/// Output mode — controls detail level and capping behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Compact output: names + locations only, hard cap on items.
    Exploring,
    /// Full detail: bodies, children, paginated via offset/limit.
    Focused,
}

/// Overflow metadata appended when results are capped.
#[derive(Debug, Clone)]
pub struct OverflowInfo {
    pub shown: usize,
    pub total: usize,
    pub hint: String,
}

/// Guardrails for tool output.  Constructed from tool input JSON.
#[derive(Debug, Clone)]
pub struct OutputGuard {
    pub mode: OutputMode,
    /// Maximum number of files to process (exploring mode).
    pub max_files: usize,
    /// Maximum number of result items (exploring mode).
    pub max_results: usize,
    /// Pagination start index (focused mode).
    pub offset: usize,
    /// Page size (focused mode).
    pub limit: usize,
}

impl Default for OutputGuard {
    fn default() -> Self {
        Self {
            mode: OutputMode::Exploring,
            max_files: 200,
            max_results: 200,
            offset: 0,
            limit: 50,
        }
    }
}

impl OutputGuard {
    /// Parse guard parameters from tool input JSON.
    ///
    /// Reads `detail_level` ("full" → Focused, anything else → Exploring),
    /// `offset`, and `limit`.
    pub fn from_input(input: &Value) -> Self {
        let mode = match input["detail_level"].as_str() {
            Some("full") => OutputMode::Focused,
            _ => OutputMode::Exploring,
        };
        let offset = input["offset"].as_u64().unwrap_or(0) as usize;
        let limit = input["limit"].as_u64().unwrap_or(50) as usize;

        Self {
            mode,
            offset,
            limit,
            ..Default::default()
        }
    }

    /// Whether to include full bodies / deep children.
    pub fn should_include_body(&self) -> bool {
        self.mode == OutputMode::Focused
    }

    /// Cap a vec of items according to the current mode.
    ///
    /// - **Exploring:** keeps at most `max_results` items from the start.
    /// - **Focused:** applies `offset` then `limit` (pagination).
    ///
    /// Returns `(kept_items, optional_overflow_info)`.
    pub fn cap_items<T>(&self, items: Vec<T>, hint: &str) -> (Vec<T>, Option<OverflowInfo>) {
        let total = items.len();
        match self.mode {
            OutputMode::Exploring => {
                if total <= self.max_results {
                    (items, None)
                } else {
                    let kept: Vec<T> = items.into_iter().take(self.max_results).collect();
                    let overflow = OverflowInfo {
                        shown: self.max_results,
                        total,
                        hint: hint.to_string(),
                    };
                    (kept, Some(overflow))
                }
            }
            OutputMode::Focused => {
                let start = self.offset.min(total);
                let end = (start + self.limit).min(total);
                let page: Vec<T> = items.into_iter().skip(start).take(end - start).collect();
                let overflow = if end < total || start > 0 {
                    Some(OverflowInfo {
                        shown: page.len(),
                        total,
                        hint: hint.to_string(),
                    })
                } else {
                    None
                };
                (page, overflow)
            }
        }
    }

    /// Same as `cap_items` but for capping a list of file paths before processing.
    pub fn cap_files<T>(&self, files: Vec<T>, hint: &str) -> (Vec<T>, Option<OverflowInfo>) {
        let total = files.len();
        let max = match self.mode {
            OutputMode::Exploring => self.max_files,
            OutputMode::Focused => {
                // In focused mode, apply offset/limit to files
                let start = self.offset.min(total);
                let end = (start + self.limit).min(total);
                let page: Vec<T> = files.into_iter().skip(start).take(end - start).collect();
                let overflow = if end < total || start > 0 {
                    Some(OverflowInfo {
                        shown: page.len(),
                        total,
                        hint: hint.to_string(),
                    })
                } else {
                    None
                };
                return (page, overflow);
            }
        };
        if total <= max {
            (files, None)
        } else {
            let kept: Vec<T> = files.into_iter().take(max).collect();
            (
                kept,
                Some(OverflowInfo {
                    shown: max,
                    total,
                    hint: hint.to_string(),
                }),
            )
        }
    }

    /// Build standardized overflow JSON to append to tool output.
    pub fn overflow_json(info: &OverflowInfo) -> Value {
        json!({
            "shown": info.shown,
            "total": info.total,
            "hint": info.hint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_exploring() {
        let guard = OutputGuard::default();
        assert_eq!(guard.mode, OutputMode::Exploring);
        assert_eq!(guard.max_files, 200);
        assert_eq!(guard.max_results, 200);
    }

    #[test]
    fn from_input_default_exploring() {
        let guard = OutputGuard::from_input(&json!({}));
        assert_eq!(guard.mode, OutputMode::Exploring);
    }

    #[test]
    fn from_input_focused_mode() {
        let guard = OutputGuard::from_input(&json!({ "detail_level": "full" }));
        assert_eq!(guard.mode, OutputMode::Focused);
    }

    #[test]
    fn from_input_parses_offset_limit() {
        let guard = OutputGuard::from_input(&json!({
            "detail_level": "full",
            "offset": 100,
            "limit": 25
        }));
        assert_eq!(guard.offset, 100);
        assert_eq!(guard.limit, 25);
    }

    #[test]
    fn should_include_body_by_mode() {
        assert!(!OutputGuard::default().should_include_body());
        assert!(
            OutputGuard::from_input(&json!({ "detail_level": "full" })).should_include_body()
        );
    }

    #[test]
    fn cap_items_exploring_under_limit() {
        let guard = OutputGuard {
            max_results: 5,
            ..Default::default()
        };
        let items: Vec<i32> = vec![1, 2, 3];
        let (kept, overflow) = guard.cap_items(items, "narrow your query");
        assert_eq!(kept, vec![1, 2, 3]);
        assert!(overflow.is_none());
    }

    #[test]
    fn cap_items_exploring_over_limit() {
        let guard = OutputGuard {
            max_results: 3,
            ..Default::default()
        };
        let items: Vec<i32> = (1..=10).collect();
        let (kept, overflow) = guard.cap_items(items, "use a file path");
        assert_eq!(kept, vec![1, 2, 3]);
        let ov = overflow.unwrap();
        assert_eq!(ov.shown, 3);
        assert_eq!(ov.total, 10);
        assert_eq!(ov.hint, "use a file path");
    }

    #[test]
    fn cap_items_focused_pagination() {
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 3,
            limit: 4,
            ..Default::default()
        };
        let items: Vec<i32> = (1..=10).collect();
        let (kept, overflow) = guard.cap_items(items, "next page");
        assert_eq!(kept, vec![4, 5, 6, 7]);
        let ov = overflow.unwrap();
        assert_eq!(ov.shown, 4);
        assert_eq!(ov.total, 10);
    }

    #[test]
    fn cap_items_focused_last_page_no_overflow() {
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 0,
            limit: 50,
            ..Default::default()
        };
        let items: Vec<i32> = vec![1, 2, 3];
        let (kept, overflow) = guard.cap_items(items, "");
        assert_eq!(kept, vec![1, 2, 3]);
        assert!(overflow.is_none());
    }

    #[test]
    fn cap_files_exploring() {
        let guard = OutputGuard {
            max_files: 2,
            ..Default::default()
        };
        let files = vec!["a.rs", "b.rs", "c.rs", "d.rs"];
        let (kept, overflow) = guard.cap_files(files, "use a glob");
        assert_eq!(kept, vec!["a.rs", "b.rs"]);
        let ov = overflow.unwrap();
        assert_eq!(ov.total, 4);
    }

    #[test]
    fn overflow_json_format() {
        let info = OverflowInfo {
            shown: 10,
            total: 500,
            hint: "narrow with a file path".into(),
        };
        let j = OutputGuard::overflow_json(&info);
        assert_eq!(j["shown"], 10);
        assert_eq!(j["total"], 500);
        assert_eq!(j["hint"], "narrow with a file path");
    }
}
```

**Step 2: Add the module declaration**

In `src/tools/mod.rs`, add after `pub mod memory;`:

```rust
pub mod output;
```

**Step 3: Run tests to verify they pass**

Run: `cargo test tools::output -- -v`
Expected: All 10 tests PASS

**Step 4: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All green

**Step 5: Commit**

```bash
git add src/tools/output.rs src/tools/mod.rs
git commit -m "feat: add OutputGuard module for progressive disclosure"
```

---

### Task 2: Wire OutputGuard into get_symbols_overview

**Files:**
- Modify: `src/tools/symbol.rs` — `GetSymbolsOverview::call()`

**Context:** `get_symbols_overview` currently has three code paths: glob expansion, single file, directory. All three need guardrails. The guard controls two things: (1) how many files to process, (2) whether to include bodies/deep children.

**Step 1: Write the failing test**

Add to `src/tools/symbol.rs` tests module (this test doesn't need LSP — it tests the output structure):

```rust
#[tokio::test]
async fn get_symbols_overview_accepts_detail_level() {
    // This just verifies the parameter is accepted without error
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
    };
    // Should error because no project, but NOT because of unknown param
    let err = GetSymbolsOverview
        .call(json!({ "relative_path": "x", "detail_level": "full" }), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("project"),
        "should fail on project, not param: {}",
        err
    );
}
```

**Step 2: Run test to verify it passes (it should already pass since we just ignore unknown fields)**

Run: `cargo test tools::symbol::tests::get_symbols_overview_accepts_detail_level -- -v`
Expected: PASS (the detail_level param is just ignored currently)

**Step 3: Implement OutputGuard integration**

In `src/tools/symbol.rs`, add the import at the top (after the other imports):

```rust
use super::output::OutputGuard;
```

Then modify `GetSymbolsOverview::call()`. The key changes:

1. Parse the guard from input
2. Update `input_schema` to include `detail_level`, `offset`, `limit`
3. In the glob path: use `guard.cap_files()` to limit files, `guard.should_include_body()` to control detail
4. In the directory path: same cap
5. In the single-file path: use `guard.should_include_body()` to decide body inclusion

Replace the `input_schema` for GetSymbolsOverview:

```rust
fn input_schema(&self) -> Value {
    json!({
        "type": "object",
        "properties": {
            "relative_path": { "type": "string", "description": "File or directory path relative to project root. Supports glob patterns (e.g. 'src/**/*.rs')" },
            "depth": { "type": "integer", "default": 1, "description": "Depth of children to include (0=none, 1=direct children)" },
            "detail_level": { "type": "string", "description": "Output detail: omit or 'exploring' for compact (default), 'full' for complete with bodies" },
            "offset": { "type": "integer", "description": "Skip this many files (focused mode pagination)" },
            "limit": { "type": "integer", "description": "Max files per page (focused mode, default 50)" }
        }
    })
}
```

In the `call` method, after parsing `rel_path` and `depth`:

```rust
let guard = OutputGuard::from_input(&input);
```

In the **glob branch**, after `resolve_glob` returns the files, add capping:

```rust
let files = resolve_glob(ctx, rel_path).await?;
let (files, file_overflow) = guard.cap_files(files, "Narrow with a more specific glob or file path");
```

And when building each file's symbols, use `guard.should_include_body()`:

```rust
let include_body = guard.should_include_body();
// ... in the map:
.map(|s| symbol_to_json(s, include_body, source.as_deref(), depth))
```

If there's file overflow, append it to the result:

```rust
let mut result_json = json!({ "pattern": rel_path, "files": result });
if let Some(ov) = file_overflow {
    result_json["overflow"] = OutputGuard::overflow_json(&ov);
}
return Ok(result_json);
```

Apply the same pattern to the **directory branch** — cap the walker results and use `guard.should_include_body()`.

For the **single file branch**, just respect `guard.should_include_body()` for the `include_body` argument to `symbol_to_json`.

**Step 4: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass, no warnings

**Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat: wire OutputGuard into get_symbols_overview"
```

---

### Task 3: Wire OutputGuard into find_symbol

**Files:**
- Modify: `src/tools/symbol.rs` — `FindSymbol::call()`

**Context:** `find_symbol` can walk the entire project and return unbounded results, especially with `include_body=true`. The guard caps results and controls body inclusion.

**Step 1: Update input_schema**

Add `detail_level`, `offset`, `limit` to FindSymbol's schema (same pattern as Task 2).

**Step 2: Implement capping**

In `FindSymbol::call()`, after building the `matches` vec:

```rust
let guard = OutputGuard::from_input(&input);
// In exploring mode, include_body defaults to false unless explicitly requested
let include_body = input["include_body"].as_bool().unwrap_or(guard.should_include_body());

// ... after collecting all matches ...

let (matches, overflow) = guard.cap_items(matches, "Restrict with a file path or glob pattern");
let mut result = json!({ "symbols": matches, "total": total_before_cap });
if let Some(ov) = overflow {
    result["overflow"] = OutputGuard::overflow_json(&ov);
}
Ok(result)
```

Note: `include_body` should still be explicitly settable — the guard only provides the default. If the user passes `include_body: true` in exploring mode, respect it.

**Step 3: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass

**Step 4: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat: wire OutputGuard into find_symbol"
```

---

### Task 4: Wire OutputGuard into find_referencing_symbols

**Files:**
- Modify: `src/tools/symbol.rs` — `FindReferencingSymbols::call()`

**Context:** References for popular symbols (like `new`, `handle`, `get`) can number in the thousands. Cap with the guard.

**Step 1: Add `detail_level`, `offset`, `limit` to schema**

**Step 2: Cap the `locations` vec after building it**

```rust
let guard = OutputGuard::from_input(&input);
// ... after building locations vec ...
let total = locations.len();
let (locations, overflow) = guard.cap_items(locations, "This symbol has many references. Use detail_level='full' with offset/limit to paginate");
let mut result = json!({ "references": locations, "total": total });
if let Some(ov) = overflow {
    result["overflow"] = OutputGuard::overflow_json(&ov);
}
Ok(result)
```

**Step 3: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`

**Step 4: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat: wire OutputGuard into find_referencing_symbols"
```

---

### Task 5: Wire OutputGuard into list_dir

**Files:**
- Modify: `src/tools/file.rs` — `ListDir::call()`

**Context:** `list_dir(recursive=true)` currently walks with `usize::MAX` depth and no entry cap. Can produce 50K+ entries in a monorepo.

**Step 1: Add imports and schema params**

Add `detail_level`, `offset`, `limit` to ListDir's `input_schema`. Add import for `OutputGuard`.

**Step 2: Cap entries**

```rust
let guard = super::output::OutputGuard::from_input(&input);
// ... after collecting entries ...
let (entries, overflow) = guard.cap_items(entries, "Use a more specific path or set recursive=false");
let mut result = json!({ "entries": entries });
if let Some(ov) = overflow {
    result["overflow"] = super::output::OutputGuard::overflow_json(&ov);
}
Ok(result)
```

**Step 3: Add a test for capped output**

In `src/tools/file.rs` tests:

```rust
#[tokio::test]
async fn list_dir_caps_output_in_exploring_mode() {
    // Create a dir with many files
    let dir = tempdir().unwrap();
    for i in 0..10 {
        std::fs::write(dir.path().join(format!("file_{}.txt", i)), "content").unwrap();
    }
    let ctx = ToolContext {
        agent: Agent::new(Some(dir.path().to_path_buf())).await.unwrap(),
        lsp: lsp(),
    };
    // With default exploring mode and our 200 cap, 10 files should be fine
    let result = ListDir
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let entries = result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 10);
    assert!(result.get("overflow").is_none());
}
```

**Step 4: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`

**Step 5: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: wire OutputGuard into list_dir"
```

---

### Task 6: Wire OutputGuard into git_diff and git_blame

**Files:**
- Modify: `src/tools/git.rs` — `GitDiff::call()` and `GitBlame::call()`

**Context:**
- `git_diff` returns the full diff string — can be huge. In exploring mode, truncate to ~50KB with a character cap.
- `git_blame` already supports line ranges, but without ranges returns full file blame. Apply `cap_items` on the lines vec.

**Step 1: git_diff — truncate diff string in exploring mode**

```rust
let guard = super::output::OutputGuard::from_input(&input);
let diff_text = crate::git::diff_workdir(&repo, file, commit)?;

match guard.mode {
    super::output::OutputMode::Exploring => {
        let max_chars = 50_000;
        if diff_text.len() > max_chars {
            let truncated = &diff_text[..max_chars];
            // Find last newline to avoid cutting mid-line
            let cut = truncated.rfind('\n').unwrap_or(max_chars);
            Ok(json!({
                "diff": &diff_text[..cut],
                "overflow": {
                    "shown_bytes": cut,
                    "total_bytes": diff_text.len(),
                    "hint": "Diff truncated. Use detail_level='full' for complete output, or restrict to a specific file with 'path'."
                }
            }))
        } else {
            Ok(json!({ "diff": diff_text }))
        }
    }
    super::output::OutputMode::Focused => {
        Ok(json!({ "diff": diff_text }))
    }
}
```

**Step 2: git_blame — cap lines without explicit range**

```rust
let guard = super::output::OutputGuard::from_input(&input);
// ... after building filtered vec ...
let total = filtered.len();
let (filtered, overflow) = guard.cap_items(filtered, "Use start_line/end_line to narrow, or detail_level='full' for all lines");
let mut result = json!({ "lines": filtered, "total": total });
if let Some(ov) = overflow {
    result["overflow"] = super::output::OutputGuard::overflow_json(&ov);
}
Ok(result)
```

**Step 3: Add `detail_level`, `offset`, `limit` to both schemas**

**Step 4: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`

**Step 5: Commit**

```bash
git add src/tools/git.rs
git commit -m "feat: wire OutputGuard into git_diff and git_blame"
```

---

### Task 7: Rewrite server_instructions.md

**Files:**
- Modify: `src/prompts/server_instructions.md`

**Context:** The current instructions list tools but don't teach decision-making. The rewrite adds: tool selection by knowledge level (know name → LSP, know concept → embeddings, know nothing → list+overview), output modes, progressive disclosure workflow, and overflow handling.

**Step 1: Replace server_instructions.md content**

See the full content in the design doc section 5. The key sections are:

1. **How to Choose the Right Tool** — three subsections: "you know the name", "you know the concept", "you know nothing"
2. **Output Modes** — exploring vs focused, progressive disclosure pattern, overflow messages
3. **Tool Reference** — grouped by category (symbol nav, reading/search, editing, git, memory, project mgmt)
4. **Rules** — 6 rules including "prefer symbol tools", "start with semantic search for how-does-X-work", "use exploring mode first", "respect overflow hints"

**Step 2: Update the static test in `src/prompts/mod.rs`**

The test `static_instructions_contain_key_sections` checks for `"## How to Explore Code"`, `"## Workflow Patterns"`, `"## Rules"`. Update to check for the new headings:

```rust
#[test]
fn static_instructions_contain_key_sections() {
    assert!(SERVER_INSTRUCTIONS.contains("## How to Choose the Right Tool"));
    assert!(SERVER_INSTRUCTIONS.contains("## Output Modes"));
    assert!(SERVER_INSTRUCTIONS.contains("## Tool Reference"));
    assert!(SERVER_INSTRUCTIONS.contains("## Rules"));
}
```

**Step 3: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`

**Step 4: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/mod.rs
git commit -m "docs: rewrite server instructions with tool selection decision tree and output modes"
```

---

### Task 8: Final verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass (150+)

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Run fmt**

Run: `cargo fmt`

**Step 4: Build release**

Run: `cargo build --release`
Expected: Clean build

**Step 5: Verify CLAUDE.md is accurate**

Read `CLAUDE.md` and verify:
- Test count is correct
- Project structure includes `output.rs`
- Design Principles section references correct file paths

Update the project structure in CLAUDE.md to include `output.rs`:
```
│   ├── tools/           # Tool implementations by category
│   │   ├── output.rs    #   OutputGuard for progressive disclosure
```

**Step 6: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final verification and CLAUDE.md cleanup"
```
