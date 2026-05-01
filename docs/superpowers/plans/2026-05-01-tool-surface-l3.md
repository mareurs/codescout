# Tool Surface Compression (L3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce codescout's MCP tool count from 25 to 20 by merging overlapping tools, before introducing `call_graph` (spec A).

> **Note on count:** spec doc § "Final Tool Surface" lists 19 numbered rows + `onboarding` un-numbered (line: `| — | onboarding | unchanged |`). Since `onboarding` is registered via `Arc::new(Onboarding)`, the actual server tool count is 20. Plan and tripwire test use 20.

**Architecture:** Hard cutover. Each merged tool keeps its current capability — we change `name()`, `description()`, `input_schema()`, and dispatch logic. No new behaviors. Tests rename in lockstep with their tools. Companion plugin and three prompt surfaces update in the same PR.

**Tech Stack:** Rust, `Tool` trait in `src/tools/mod.rs`, schema via `serde_json::Value`. Companion plugin = bash hooks at `../claude-plugins/codescout-companion/hooks/`.

**Spec:** `docs/superpowers/specs/2026-05-01-tool-surface-l3-design.md`

---

## Conventions

- Branch: `experiments` (already on it).
- Pre-completion gate (every task): `cargo fmt && cargo clippy -- -D warnings && cargo test`. Failures = task incomplete.
- Commit style: matches recent commits (e.g. `feat(tools): rename find_references to references`).
- Refactor steps: write the code change, then run tests. Existing tests cover behavior — when a tool's `name()` changes, only tests asserting the literal name string need updates. The `prompt_surfaces_reference_only_real_tools` test acts as a tripwire.
- After each task: `cargo test` must be green before commit.

---

## Phase 0 — Baseline

### Task 0: Confirm baseline green

**Files:** none

- [ ] **Step 1: Verify clean working tree**

```bash
git status
```

Expected: `clean` (or only this plan file untracked, in which case continue).

- [ ] **Step 2: Run full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass. Any failures = abort plan, fix master first.

- [ ] **Step 3: Snapshot tool count**

```bash
grep -c 'Arc::new' src/server.rs | head -1
```

Note the count; spot check ≥ 25.

---

## Phase 1 — Pure renames

### Task 1: Rename `find_references` → `references`

**Files:**
- Modify: `src/tools/symbol/find_references.rs`
- Modify: `src/tools/symbol/mod.rs`
- Modify: `src/server.rs:1334` (test list) and any other usage
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/prompts/builders.rs` (if name appears)
- Modify: companion plugin hook scripts (see Task 15 — log here, batched there)

- [ ] **Step 1: Rename the file**

```bash
git mv src/tools/symbol/find_references.rs src/tools/symbol/references.rs
```

- [ ] **Step 2: Update `src/tools/symbol/mod.rs`**

Replace `pub mod find_references;` with `pub mod references;` and any `use` of the symbol.

- [ ] **Step 3: Rename the struct + name string**

In `src/tools/symbol/references.rs`:

```rust
pub struct References;

impl Tool for References {
    fn name(&self) -> &str {
        "references"
    }
    // ... rest unchanged
}
```

Find all `FindReferences` references in this file and rename to `References`.

- [ ] **Step 4: Update server registration**

In `src/server.rs`, replace `Arc::new(FindReferences)` (or similar) with `Arc::new(References)`. Find via:

```bash
grep -n "FindReferences\|find_references" src/server.rs
```

Update each hit.

- [ ] **Step 5: Update test name lists**

In `src/server.rs` test fixtures (lines around 1334, 1990, 2032, others — find them):

```bash
grep -n "find_references" src/server.rs src/tools/
```

Replace each `"find_references"` literal with `"references"`.

- [ ] **Step 6: Update prompt surfaces**

```bash
grep -rn "find_references" src/prompts/ docs/ CLAUDE.md README.md
```

Replace each occurrence with `references`. Note any surface-level surrounding text that needs polish (e.g. "find references via …" stays as English).

- [ ] **Step 7: Run tests**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
```

Expected: all green. The `prompt_surfaces_reference_only_real_tools` test should pass.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(tools): rename find_references to references

Part of L3 tool surface compression."
```

---

## Phase 2 — Polymorphic merges

### Task 2: Merge `goto_definition` + `hover` → `symbol_at`

**Files:**
- Create: `src/tools/symbol/symbol_at.rs`
- Delete: `src/tools/symbol/goto_definition.rs`, `src/tools/symbol/hover.rs`
- Modify: `src/tools/symbol/mod.rs`
- Modify: `src/server.rs` (registration + name lists)
- Modify: prompt surfaces

**Schema:**

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string", "description": "File path" },
    "line": { "type": "integer", "description": "1-indexed line number" },
    "fields": {
      "type": "array",
      "items": { "enum": ["def", "hover"] },
      "default": ["def", "hover"],
      "description": "Which LSP queries to run"
    }
  },
  "required": ["path", "line"]
}
```

**Output:**

```json
{
  "def": { "file": "...", "line": 42, "column": 10 },
  "hover": { "contents": "fn foo(x: i32) -> i32\n\nDoes the thing." }
}
```

Either field may be absent if not requested or LSP returned nothing.

- [ ] **Step 1: Create `symbol_at.rs`**

Read `src/tools/symbol/goto_definition.rs` and `src/tools/symbol/hover.rs` to understand current LSP call patterns. Then write `src/tools/symbol/symbol_at.rs` whose `call()` dispatches based on the `fields` array, calling the same underlying LSP helpers used by `GotoDefinition` and `Hover` today. Reuse the existing helpers — do not duplicate LSP call logic.

```rust
use crate::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{Value, json};

pub struct SymbolAt;

impl Tool for SymbolAt {
    fn name(&self) -> &str {
        "symbol_at"
    }

    fn description(&self) -> &str {
        "Inspect the symbol at a given file position. Returns LSP-resolved \
         definition location and/or hover text (type signature, docs). \
         Use when you have a file:line cursor and need 'what is this?'."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "line": { "type": "integer" },
                "fields": {
                    "type": "array",
                    "items": { "enum": ["def", "hover"] },
                    "default": ["def", "hover"]
                }
            },
            "required": ["path", "line"]
        })
    }

    fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let path = input["path"].as_str().ok_or_else(|| anyhow::anyhow!("path required"))?;
        let line = input["line"].as_u64().ok_or_else(|| anyhow::anyhow!("line required"))? as u32;
        let fields: Vec<String> = input["fields"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(|| vec!["def".into(), "hover".into()]);

        let mut out = serde_json::Map::new();
        if fields.iter().any(|f| f == "def") {
            // Reuse the existing definition-fetching logic from goto_definition.rs.
            // Extract that body into a free helper `fetch_definition(ctx, path, line) -> Result<Option<Value>>`
            // and call it here.
            if let Some(def) = crate::tools::symbol::symbol_at::fetch_definition(ctx, path, line)? {
                out.insert("def".into(), def);
            }
        }
        if fields.iter().any(|f| f == "hover") {
            if let Some(hover) = crate::tools::symbol::symbol_at::fetch_hover(ctx, path, line)? {
                out.insert("hover".into(), hover);
            }
        }
        Ok(Value::Object(out))
    }

    fn format_compact(&self, _result: &Value) -> Option<String> {
        None
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        // Match the most-restrictive of the two original tools (likely LSP-required).
        crate::tools::Availability::RequiresLsp
    }
}

pub(crate) fn fetch_definition(ctx: &ToolContext, path: &str, line: u32) -> Result<Option<Value>> {
    // Move the body of the old GotoDefinition::call() that produces the result Value here.
    todo!("port from goto_definition.rs")
}

pub(crate) fn fetch_hover(ctx: &ToolContext, path: &str, line: u32) -> Result<Option<Value>> {
    todo!("port from hover.rs")
}
```

Replace the two `todo!()` bodies with the actual LSP call code from `goto_definition.rs` and `hover.rs` respectively. The bodies that today live inside `GotoDefinition::call` / `Hover::call` move into these helpers, returning `Result<Option<Value>>` instead of building tool-shaped responses.

- [ ] **Step 2: Update `src/tools/symbol/mod.rs`**

Replace:

```rust
pub mod goto_definition;
pub mod hover;
```

with:

```rust
pub mod symbol_at;
```

- [ ] **Step 3: Delete the old files**

```bash
git rm src/tools/symbol/goto_definition.rs src/tools/symbol/hover.rs
```

- [ ] **Step 4: Update server registration**

In `src/server.rs`, replace the two `Arc::new(GotoDefinition)` and `Arc::new(Hover)` lines with one `Arc::new(SymbolAt)`. Update test name lists (search for `"goto_definition"` and `"hover"` and replace with `"symbol_at"`).

- [ ] **Step 5: Migrate the existing tests**

The existing `goto_definition_*` and `hover_*` tests live in `src/tools/symbol/tests.rs`. Update each to call the new tool name and the new schema (passing `fields: ["def"]` for the goto-only tests, `fields: ["hover"]` for hover-only tests). Add one new test:

```rust
#[test]
fn symbol_at_returns_both_fields_by_default() {
    let lsp = lsp();
    let ctx = ctx_with_lsp(lsp);
    let result = SymbolAt.call(
        json!({ "path": "src/lib.rs", "line": 10 }),
        &ctx,
    ).unwrap();
    assert!(result.get("def").is_some(), "def field should be present by default");
    assert!(result.get("hover").is_some(), "hover field should be present by default");
}
```

- [ ] **Step 6: Update prompt surfaces**

```bash
grep -rn "goto_definition\|hover" src/prompts/ docs/ CLAUDE.md README.md
```

Replace each tool-name reference. Where prose flows (e.g. "use hover to inspect types"), rewrite to "use symbol_at with `fields: ['hover']`".

- [ ] **Step 7: Tests**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(tools): merge goto_definition + hover into symbol_at"
```

---

### Task 3: Merge `find_symbol` + `list_symbols` → `symbols`

**Files:**
- Modify: `src/tools/symbol/find_symbol.rs` (becomes the merged tool — rename in place)
- Modify: `src/tools/symbol/mod.rs`
- Delete: `src/tools/symbol/list_symbols.rs`
- Modify: `src/server.rs`
- Modify: prompt surfaces

**Approach:** `FindSymbol` already supports the superset (path-scoping + name search + name_path + kind filter). The merge is mostly a rename + ensuring path-only-no-name behaves like current `list_symbols` (file overview, top-level cap of 100).

**Schema:**

```json
{
  "type": "object",
  "properties": {
    "path":          { "type": "string" },
    "name":          { "type": "string" },
    "name_path":     { "type": "string" },
    "kind":          { "enum": ["function","class","struct","interface","type","enum","module","constant"] },
    "include_body":  { "type": "boolean", "default": false },
    "depth":         { "type": "integer", "default": 1 }
  },
  "description": "Path only → file/dir overview. Name → search. Both → scoped search."
}
```

- [ ] **Step 1: Rename file and struct**

```bash
git mv src/tools/symbol/find_symbol.rs src/tools/symbol/symbols.rs
```

In the file, rename `pub struct FindSymbol;` → `pub struct Symbols;` (keep all impls otherwise intact). Change `name(&self) -> &str` to return `"symbols"`.

- [ ] **Step 2: Verify path-only-no-name path matches `list_symbols` behavior**

`FindSymbol::call` already handles `path` arg + missing `name`. Verify by reading it. If it differs from `ListSymbols`'s overview semantics (top-level cap 100, no recursive descent without `depth`), port the missing branches from `list_symbols.rs` into `symbols.rs`.

Concretely: read `src/tools/symbol/list_symbols.rs::ListSymbols::call`, identify any code path not present in `FindSymbol::call` for the path-only case, and copy it across with a clear comment:

```rust
// Path-only-no-name path: file/dir overview (was list_symbols).
```

- [ ] **Step 3: Update `src/tools/symbol/mod.rs`**

Remove `pub mod list_symbols;` and `pub mod find_symbol;`. Add `pub mod symbols;`.

- [ ] **Step 4: Delete `list_symbols.rs`**

```bash
git rm src/tools/symbol/list_symbols.rs
```

- [ ] **Step 5: Server registration + tests + prompts**

Replace `Arc::new(FindSymbol)` and `Arc::new(ListSymbols)` (one or both — find them) with a single `Arc::new(Symbols)`. Update name strings throughout `src/server.rs` (`"find_symbol"`, `"list_symbols"` → `"symbols"`). Update prompts.

- [ ] **Step 6: Migrate tests**

In `src/tools/symbol/tests.rs`, rename `find_symbol_*` and `list_symbols_*` test functions to `symbols_*` and update the tool struct name. Behavior assertions stay; only the entry-point changes.

- [ ] **Step 7: Tests**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(tools): merge find_symbol + list_symbols into symbols"
```

---

### Task 4: Merge `list_dir` + `find_file` → `tree`

**Files:**
- Create: `src/tools/tree.rs`
- Delete: `src/tools/list_dir.rs`, `src/tools/glob.rs` (or whichever holds `find_file`)
- Modify: `src/tools/mod.rs`
- Modify: `src/server.rs`
- Modify: prompt surfaces

**First confirm where `find_file` lives:**

```bash
grep -rn "fn name.*find_file\|FindFile" src/tools/
```

If `find_file` is in `src/tools/glob.rs`, that's the source file to merge.

**Schema:**

```json
{
  "type": "object",
  "properties": {
    "path":      { "type": "string", "description": "Subtree root (default: project root)" },
    "glob":      { "type": "string", "description": "When set, return matching paths instead of a directory listing" },
    "recursive": { "type": "boolean", "default": false },
    "max_depth": { "type": "integer" }
  },
  "description": "When `glob` set → file search. Otherwise → directory listing."
}
```

- [ ] **Step 1: Create `src/tools/tree.rs`**

Combine the two existing tool bodies into one struct. Skeleton:

```rust
use crate::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{Value, json};

pub struct Tree;

impl Tool for Tree {
    fn name(&self) -> &str { "tree" }

    fn description(&self) -> &str {
        "List a directory tree, or find files by glob. With `glob` set, returns \
         matching paths; without it, returns directory contents (optionally recursive)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":      { "type": "string" },
                "glob":      { "type": "string" },
                "recursive": { "type": "boolean", "default": false },
                "max_depth": { "type": "integer" }
            }
        })
    }

    fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        if let Some(g) = input.get("glob").and_then(Value::as_str) {
            // Delegate to existing find_file logic.
            crate::tools::tree::find_file_impl(ctx, &input, g)
        } else {
            crate::tools::tree::list_dir_impl(ctx, &input)
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // Reuse formatters from old list_dir.rs / glob.rs depending on shape.
        // Detect by presence of `entries` vs `matches`.
        if result.get("matches").is_some() {
            Some(format_find_file(result))
        } else {
            Some(format_list_dir(result))
        }
    }
}

pub(crate) fn list_dir_impl(ctx: &ToolContext, input: &Value) -> Result<Value> {
    // Move the body of `ListDir::call` here.
    todo!()
}

pub(crate) fn find_file_impl(ctx: &ToolContext, input: &Value, glob: &str) -> Result<Value> {
    // Move the body of `FindFile::call` here.
    todo!()
}

// Move format_list_dir and the find_file formatter here too.
```

Replace the two `todo!()`s with the actual ported bodies. Move `format_list_dir`, `format_list_dir_tree_body`, `common_path_prefix` from `list_dir.rs` into `tree.rs`. Same for the find_file formatter from `glob.rs`.

- [ ] **Step 2: Delete old files**

```bash
git rm src/tools/list_dir.rs src/tools/glob.rs
```

(Adjust `glob.rs` if `find_file` lives elsewhere.)

- [ ] **Step 3: Update `src/tools/mod.rs`**

Remove `pub mod list_dir;` and `pub mod glob;`. Add `pub mod tree;`.

- [ ] **Step 4: Server registration + tests + prompts**

Replace `Arc::new(ListDir)` and `Arc::new(FindFile)` (whatever its struct name is) with one `Arc::new(Tree)`. Search and replace `"list_dir"`, `"find_file"` → `"tree"` in `src/server.rs`, prompts, docs.

- [ ] **Step 5: Migrate tests**

Rename existing test functions, update tool struct, add one new test asserting both modes through one tool:

```rust
#[test]
fn tree_lists_when_no_glob() {
    let ctx = ctx();
    let r = Tree.call(json!({ "path": "src" }), &ctx).unwrap();
    assert!(r.get("entries").is_some());
}

#[test]
fn tree_finds_when_glob_set() {
    let ctx = ctx();
    let r = Tree.call(json!({ "path": ".", "glob": "**/*.rs" }), &ctx).unwrap();
    assert!(r.get("matches").is_some());
}
```

- [ ] **Step 6: Tests**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(tools): merge list_dir + find_file into tree"
```

---

## Phase 3 — Action-param merges

### Task 5: Merge `activate_project` + `project_status` → `workspace`

**Files:**
- Modify: `src/tools/config.rs`
- Modify: `src/server.rs`
- Modify: prompt surfaces

**Schema:**

```json
{
  "type": "object",
  "properties": {
    "action":    { "enum": ["activate", "status", "list_projects"] },
    "path":      { "type": "string", "description": "For activate: project path" },
    "read_only": { "type": "boolean", "description": "For activate" }
  },
  "required": ["action"]
}
```

- [ ] **Step 1: Add `Workspace` struct in `src/tools/config.rs`**

Add at top of file:

```rust
pub struct Workspace;

impl Tool for Workspace {
    fn name(&self) -> &str { "workspace" }

    fn description(&self) -> &str {
        "Project workspace operations. Actions: \
         `activate` (switch active project), \
         `status` (current project + index + memories), \
         `list_projects` (workspace members)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action":    { "enum": ["activate", "status", "list_projects"] },
                "path":      { "type": "string" },
                "read_only": { "type": "boolean" }
            },
            "required": ["action"]
        })
    }

    fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let action = input["action"].as_str().ok_or_else(|| anyhow::anyhow!("action required"))?;
        match action {
            "activate" => ActivateProject.call(input, ctx),
            "status"   => ProjectStatus.call(input, ctx),
            "list_projects" => {
                // Extract the workspace-listing branch from ProjectStatus::call into a
                // standalone helper or call ProjectStatus and extract the workspace field.
                let full = ProjectStatus.call(json!({}), ctx)?;
                Ok(json!({ "workspace": full.get("workspace") }))
            }
            other => anyhow::bail!("unknown workspace action: {}", other),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // Dispatch on shape; reuse format_activate_project / format_project_status.
        if result.get("auto_libs").is_some() || result.get("project_root").is_some() {
            Some(format_activate_project(result))
        } else {
            Some(format_project_status(result))
        }
    }
}
```

Keep `ActivateProject` and `ProjectStatus` structs **internal** (not registered) — `Workspace` delegates to them.

- [ ] **Step 2: Server registration**

In `src/server.rs`, replace `Arc::new(ActivateProject)` and `Arc::new(ProjectStatus)` with one `Arc::new(Workspace)`. Remove the two old name strings from test fixtures; add `"workspace"`.

- [ ] **Step 3: `src/server.rs::is_write_call`**

Find the function. `activate_project` may have appeared in a write-detect list (it can change config). Update the match arm: `"workspace" if input["action"] == "activate" => true` if needed; otherwise drop the entry.

- [ ] **Step 4: Prompts**

```bash
grep -rn "activate_project\|project_status" src/prompts/ docs/ CLAUDE.md README.md
```

Update each. Server-instructions prose: "Call `workspace(action='activate', path=...)` to switch projects."

**Important:** `activate_project` is referenced in many narrative spots ("after `activate_project`, …"). Rewrite each in line.

- [ ] **Step 5: Migrate tests**

The existing `activate_*` and `project_status_*` tests in `src/tools/config.rs` keep most of their bodies but invoke `Workspace.call(json!({ "action": "activate", ... }), &ctx)` etc. Add one test:

```rust
#[test]
fn workspace_action_unknown_errors() {
    let ctx = ctx();
    let err = Workspace.call(json!({ "action": "wat" }), &ctx).unwrap_err();
    assert!(err.to_string().contains("unknown workspace action"));
}
```

- [ ] **Step 6: Tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "refactor(tools): merge activate_project + project_status into workspace"
```

---

### Task 6: Merge `list_libraries` + `register_library` → `library`

**Files:**
- Modify: `src/tools/library.rs`
- Modify: `src/util/path_security.rs::check_tool_access`
- Modify: `src/server.rs`
- Modify: prompts

**Schema:**

```json
{
  "type": "object",
  "properties": {
    "action": { "enum": ["list", "register"] },
    "path":   { "type": "string", "description": "For register" },
    "name":   { "type": "string", "description": "For register" },
    "language": { "type": "string", "description": "For register" }
  },
  "required": ["action"]
}
```

- [ ] **Step 1: Add `Library` struct in `src/tools/library.rs`**

```rust
pub struct Library;

impl Tool for Library {
    fn name(&self) -> &str { "library" }

    fn is_write(&self, input: &Value) -> bool {
        input.get("action").and_then(Value::as_str) == Some("register")
    }

    fn description(&self) -> &str {
        "Library registry. Actions: `list` (show registered libraries), \
         `register` (add a library directory for cross-project search)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action":   { "enum": ["list", "register"] },
                "path":     { "type": "string" },
                "name":     { "type": "string" },
                "language": { "type": "string" }
            },
            "required": ["action"]
        })
    }

    fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        match input["action"].as_str() {
            Some("list") => ListLibraries.call(input, ctx),
            Some("register") => RegisterLibrary.call(input, ctx),
            _ => anyhow::bail!("library action must be 'list' or 'register'"),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        if result.get("libraries").is_some() {
            ListLibraries.format_compact(result)
        } else {
            RegisterLibrary.format_compact(result)
        }
    }

    fn availability(&self, caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        ListLibraries.availability(caps)
    }
}
```

- [ ] **Step 2: Update `check_tool_access`**

In `src/util/path_security.rs:379-405` replace `"register_library"` with `"library"` in the write-tools match arm:

```rust
"create_file" | "edit_file" | "replace_symbol" | "insert_code" | "rename_symbol"
| "remove_symbol" | "library" | "edit_markdown" => {
    if !config.file_write_enabled {
        bail!(
            "File writes are disabled for this project. If this project was activated \
             in read-only mode, call workspace with action='activate' and read_only: false to enable writes."
        );
    }
}
```

The error message must be updated for `activate_project` → `workspace(action='activate')`.

- [ ] **Step 3: Server registration + tests + prompts**

Replace `Arc::new(ListLibraries)` and `Arc::new(RegisterLibrary)` with one `Arc::new(Library)`. Search/replace `"list_libraries"`, `"register_library"` → `"library"`. Adjust the `is_write_call` test (line 2140 region) to assert `is_write_call("library", json!({"action": "register"}))` is true and `library` + `list` is false.

- [ ] **Step 4: Update existing library tests**

In `src/tools/library.rs::tests`, change each call from `ListLibraries.call(...)` and `RegisterLibrary.call(...)` to `Library.call(json!({ "action": "list", ... }))` etc.

- [ ] **Step 5: Tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "refactor(tools): merge list_libraries + register_library into library"
```

---

### Task 7: Merge `index_project` + `index_status` → `index`

**Files:**
- Modify: `src/tools/semantic.rs` (or wherever IndexProject/IndexStatus live — find them)
- Modify: `src/util/path_security.rs::check_tool_access`
- Modify: `src/server.rs` (registration, `tool_skips_server_timeout` line ~573, prompts)
- Modify: prompt surfaces

**Find current location:**

```bash
grep -rn "fn name.*\"index_project\"\|fn name.*\"index_status\"\|IndexProject\|IndexStatus" src/tools/
```

**Schema:**

```json
{
  "type": "object",
  "properties": {
    "action":    { "enum": ["build", "status"] },
    "path":      { "type": "string", "description": "For build: scope path" },
    "threshold": { "type": "integer", "description": "For status: drift threshold" }
  },
  "required": ["action"]
}
```

- [ ] **Step 1: Add `Index` struct**

In the same file as the originals:

```rust
pub struct Index;

impl Tool for Index {
    fn name(&self) -> &str { "index" }

    fn description(&self) -> &str {
        "Semantic index operations. Actions: \
         `build` (index project for semantic_search), \
         `status` (indexed counts + drift)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action":    { "enum": ["build", "status"] },
                "path":      { "type": "string" },
                "threshold": { "type": "integer" }
            },
            "required": ["action"]
        })
    }

    fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        match input["action"].as_str() {
            Some("build")  => IndexProject.call(input, ctx),
            Some("status") => IndexStatus.call(input, ctx),
            _ => anyhow::bail!("index action must be 'build' or 'status'"),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // Dispatch on shape (status returns counts; build returns build report).
        if result.get("indexed").is_some() && result.get("queued").is_some() {
            IndexStatus.format_compact(result)
        } else {
            IndexProject.format_compact(result)
        }
    }
}
```

- [ ] **Step 2: Update `check_tool_access`**

Replace `"index_project"` with `"index"` in the indexing match arm:

```rust
"semantic_search" | "index" => { ... }
```

The check fires for both actions; if `status` should bypass the indexing-disabled gate, add:

```rust
"index" if action_is_build(&input) => { ... }
```

For simplicity, gate both actions identically (matches current behavior).

- [ ] **Step 3: Update `tool_skips_server_timeout`**

In `src/server.rs:573`:

```rust
matches!(name, "index" | "index_library" | "run_command")
```

(replacing `"index_project"`).

- [ ] **Step 4: Server registration**

Replace `Arc::new(IndexProject)` + `Arc::new(IndexStatus)` with `Arc::new(Index)`. Search/replace `"index_project"`, `"index_status"` → `"index"`. The `is_write_call` test (line 2138): `is_write_call("index", json!({"action": "build"}))` should be true.

- [ ] **Step 5: Update tests**

Migrate `index_project_*` and `index_status_*` tests in their current file to call `Index.call(json!({"action":"build", ...}))` etc.

- [ ] **Step 6: Tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "refactor(tools): merge index_project + index_status into index"
```

---

## Phase 4 — Stub for spec A

### Task 8: Add `call_graph` placeholder

**Files:**
- Create: `src/tools/symbol/call_graph.rs`
- Modify: `src/tools/symbol/mod.rs`
- Modify: `src/server.rs`
- Modify: prompts

- [ ] **Step 1: Write the stub**

```rust
// src/tools/symbol/call_graph.rs

use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde_json::{Value, json};

pub struct CallGraph;

impl Tool for CallGraph {
    fn name(&self) -> &str { "call_graph" }

    fn description(&self) -> &str {
        "Transitive call graph for a symbol. Direction `callers` (blast radius) \
         or `callees` (flow) or `both`. NOT YET IMPLEMENTED — see \
         docs/superpowers/specs/<A>."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol":    { "type": "string" },
                "direction": { "enum": ["callers", "callees", "both"], "default": "callers" },
                "max_depth": { "type": "integer", "default": 3 }
            },
            "required": ["symbol"]
        })
    }

    fn call(&self, _input: Value, _ctx: &ToolContext) -> Result<Value> {
        Err(RecoverableError::new(
            "call_graph is not yet implemented. \
             Tracked in docs/socraticode-borrow-tracker.md (item A). \
             Use `references` for one-hop call sites in the meantime."
        ).into())
    }

    fn format_compact(&self, _result: &Value) -> Option<String> { None }
}
```

- [ ] **Step 2: Wire it up**

`src/tools/symbol/mod.rs`: add `pub mod call_graph;`. `src/server.rs`: add `Arc::new(CallGraph)` and `"call_graph"` to test name lists.

- [ ] **Step 3: Prompts**

In `src/prompts/server_instructions.md`, add a one-line mention of `call_graph` next to `references` saying it's reserved for transitive call queries (currently stub).

- [ ] **Step 4: Tests**

Add to the appropriate tests file:

```rust
#[test]
fn call_graph_stub_returns_recoverable_error() {
    let ctx = ctx();
    let err = CallGraph.call(json!({ "symbol": "foo" }), &ctx).unwrap_err();
    assert!(err.to_string().contains("not yet implemented"));
}
```

- [ ] **Step 5: Run tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "feat(tools): add call_graph stub (impl deferred to spec A)"
```

---

## Phase 5 — Surface, docs, infra

### Task 9: Audit & assert tool count

**Files:**
- Modify: `src/server.rs` test module (add count tripwire)

- [ ] **Step 1: Add tripwire test**

In `src/server.rs` near the existing `server_registers_all_tools` test, add:

```rust
#[test]
fn server_tool_count_is_l3_target() {
    let server = build_server_for_tests();
    let names: Vec<&str> = server.tools().iter().map(|t| t.name()).collect();
    assert_eq!(
        names.len(), 20,
        "L3 target is 20 tools (19 + onboarding); got {}: {:?}", names.len(), names
    );
}
```

(Adjust the `build_server_for_tests` helper name to whatever exists in this file.)

- [ ] **Step 2: Update `server_registers_all_tools` allowlist**

Replace the old name list (around line 1324–1349) with the final 19 names:

```rust
let expected = [
    "read_file", "tree", "search_pattern", "create_file", "edit_file",
    "run_command", "onboarding", "memory", "semantic_search",
    "symbols", "symbol_at", "references", "call_graph",
    "replace_symbol", "insert_code", "rename_symbol", "remove_symbol",
    "workspace", "library", "index",
    "onboarding",
];
```

(Count: 20.)

- [ ] **Step 3: Tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "test(server): assert L3 tool count tripwire"
```

---

### Task 10: Sweep prompt surfaces for stragglers

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/prompts/builders.rs`

- [ ] **Step 1: Find any stale tool names**

```bash
grep -rn 'find_symbol\|list_symbols\|find_references\|goto_definition\|hover\|list_dir\|find_file\|activate_project\|project_status\|list_libraries\|register_library\|index_project\|index_status' src/prompts/
```

Expected: zero hits. Each hit is a bug.

- [ ] **Step 2: Fix each hit**

Replace tool names per the migration table in the spec. Be careful with surrounding prose — sentences may need restructuring.

- [ ] **Step 3: Run prompt-surface test**

```bash
cargo test prompt_surfaces_reference_only_real_tools 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 4: Full test suite + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "docs(prompts): finalize tool name references for L3"
```

---

### Task 11: Bump `ONBOARDING_VERSION`

**Files:**
- Modify: `src/tools/onboarding.rs`

- [ ] **Step 1: Find the constant**

```bash
grep -n "ONBOARDING_VERSION" src/tools/onboarding.rs
```

- [ ] **Step 2: Bump it**

Increment the integer by 1. Add a comment above it:

```rust
// Bumped for L3 tool surface compression (2026-05-01) — generated system
// prompts reference renamed tools.
const ONBOARDING_VERSION: u32 = N + 1;
```

- [ ] **Step 3: Tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "chore(onboarding): bump version for L3 tool renames"
```

---

### Task 12: Update top-level docs

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/PROGRESSIVE_DISCOVERABILITY.md`
- Modify: `docs/manual/src/**` (all pages mentioning renamed tools)

- [ ] **Step 1: Find every doc reference**

```bash
grep -rln 'find_symbol\|list_symbols\|find_references\|goto_definition\|hover\|list_dir\|find_file\|activate_project\|project_status\|list_libraries\|register_library\|index_project\|index_status' \
  CLAUDE.md README.md docs/
```

- [ ] **Step 2: Update each file**

Apply migration mapping; rewrite prose where necessary. The "25 tools" mention in `CLAUDE.md` becomes "19 tools".

- [ ] **Step 3: Add CHANGELOG.md entry**

If `CHANGELOG.md` exists, prepend an entry under unreleased / next-version:

```markdown
## [Unreleased]

### Breaking changes — tool surface compression (L3)

| Old name | New name |
|----------|----------|
| find_symbol, list_symbols | symbols |
| find_references | references |
| goto_definition, hover | symbol_at (fields: ["def", "hover"]) |
| list_dir, find_file | tree |
| activate_project, project_status | workspace (action: activate / status / list_projects) |
| list_libraries, register_library | library (action: list / register) |
| index_project, index_status | index (action: build / status) |

Added: `call_graph` (stub; implementation tracked separately).
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs: update tool names for L3 surface compression"
```

---

### Task 13: Update companion plugin

**Files:**
- Modify: `../claude-plugins/codescout-companion/hooks/semantic-tool-router.sh`
- Modify: `../claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`
- Modify: `../claude-plugins/codescout-companion/hooks/session-start.sh`
- Modify: `../claude-plugins/codescout-companion/hooks/subagent-guidance.sh`
- Modify: `../claude-plugins/codescout-companion/skills/**/*` (any tool-name mentions)

- [ ] **Step 1: Find every reference in companion**

```bash
cd ../claude-plugins/codescout-companion
grep -rln 'find_symbol\|list_symbols\|find_references\|goto_definition\|hover\|list_dir\|find_file\|activate_project\|project_status\|list_libraries\|register_library\|index_project\|index_status' .
```

- [ ] **Step 2: Apply the migration mapping**

Same table as Task 12. Hook scripts often have suggestion strings like `"USE ... INSTEAD"` — rewrite to point at new names.

- [ ] **Step 3: Bump companion version**

If the companion has a `version` field (in `plugin.json` / `package.json` / similar), bump it.

- [ ] **Step 4: Test the companion locally**

Restart Claude Code session in the codescout repo (or run `/mcp` restart per CLAUDE.md). Trigger one tool call per renamed tool to confirm hooks don't reject. If a hook still blocks, fix the script.

- [ ] **Step 5: Commit in the companion repo**

```bash
cd ../claude-plugins/codescout-companion
git add -A
git commit -m "chore: update tool names for codescout L3 surface compression"
```

(Push deferred until codescout side merges; treat companion + codescout as a coordinated release.)

---

### Task 14: Memory templates

**Files:**
- Modify: any project memory files referencing old tool names

- [ ] **Step 1: Find references**

```bash
grep -rln 'find_symbol\|list_symbols\|find_references\|goto_definition\|hover\|list_dir\|find_file\|activate_project\|project_status\|list_libraries\|register_library\|index_project\|index_status' \
  src/memory/ src/tools/onboarding/
```

(Adjust paths to wherever memory templates / generated-prompt fragments live.)

- [ ] **Step 2: Update each**

Apply migration mapping.

- [ ] **Step 3: Tests + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -30
git add -A
git commit -m "chore(memory): update built-in templates for L3 tool names"
```

---

### Task 15: Version bump + CHANGELOG finalize

**Files:**
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md` (if exists)

- [ ] **Step 1: Bump minor version in `Cargo.toml`**

Pre-1.0 minor bump (e.g. `0.5.x` → `0.6.0`).

- [ ] **Step 2: Update `Cargo.lock`**

```bash
cargo build --release
```

- [ ] **Step 3: Move "Unreleased" CHANGELOG entry under the new version heading**

(If a CHANGELOG exists.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: bump version for L3 tool surface compression"
```

---

### Task 16: Manual verification via live MCP

**Files:** none (procedural)

- [ ] **Step 1: Build release**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: success.

- [ ] **Step 2: Restart MCP server**

In Claude Code: `/mcp` and pick `codescout` → restart.

- [ ] **Step 3: Smoke-test each renamed/merged tool**

For each of: `symbols`, `symbol_at`, `references`, `call_graph`, `tree`, `workspace`, `library`, `index` — invoke once via the live server with a trivial input.

- For `call_graph`, expect a recoverable error containing "not yet implemented".
- For everything else, expect a successful response shaped like the old tool's output.

- [ ] **Step 4: Update tracker**

```bash
$EDITOR docs/socraticode-borrow-tracker.md
```

- Mark L3 row status as ✅
- Update Active section: `**A — Code graph + blast radius.** Ready to design.`
- Commit:

```bash
git add docs/socraticode-borrow-tracker.md
git commit -m "docs(tracker): mark L3 done; unblock A"
```

- [ ] **Step 5: Final test run**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -10
```

All green = L3 ready to cherry-pick to master per CLAUDE.md ship sequence.

---

## Self-Review Notes

- **Spec coverage:** Every line of the spec § "Surfaces to update" maps to a Task (1–16). The 19-tool target is enforced by Task 9's tripwire test.
- **Type consistency:** Tool struct names are uniform (`Symbols`, `SymbolAt`, `References`, `Tree`, `Workspace`, `Library`, `Index`, `CallGraph`). Method signatures use `Tool` trait so no per-tool type drift.
- **Placeholder scan:** No `TODO`s, no "implement later". Each merge task ships with full skeleton code; `todo!()` markers in code blocks are explicit "port-from-here" instructions naming the source file.
- **Risks per spec:** action-param confusion mitigated via explicit enum descriptions; `tree` polymorphism mitigated via Task 4's two new tests; `call_graph` stub mitigated via Task 8's RecoverableError text + tracker pointer.

## Out of Scope

Per spec § "Out of Scope": `references` kind filter, `flow(from, to)`, `index(action=remove)`, further compression. All tracked in `docs/socraticode-borrow-tracker.md`.
