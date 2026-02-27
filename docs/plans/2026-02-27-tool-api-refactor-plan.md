# Tool API Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename 9 tools for consistency, standardize pagination parameters, and rewrite server instructions.

**Architecture:** Mechanical renames of `name()` return strings across tool files, updating tests and security gates to match. No struct renames, no behavioral changes.

**Tech Stack:** Rust, cargo test/clippy/fmt

---

### Task 1: Rename tool name strings (all 9 tools across 5 files)

**Files:**
- Modify: `src/tools/file.rs` — lines 223, 298 (name methods)
- Modify: `src/tools/symbol.rs` — lines 214, 619, 706 (name methods)
- Modify: `src/tools/workflow.rs` — lines 264, 307 (name methods)
- Modify: `src/tools/ast.rs` — line 92 (name method)
- Modify: `src/tools/config.rs` — line 52 (name method)

**Step 1: Apply all 9 name changes**

Each change is a single-line string replacement in the tool's `name()` method:

| File | Line | Old | New |
|---|---|---|---|
| `src/tools/file.rs` | 223 | `"search_for_pattern"` | `"search_pattern"` |
| `src/tools/file.rs` | 298 | `"create_text_file"` | `"create_file"` |
| `src/tools/symbol.rs` | 214 | `"get_symbols_overview"` | `"list_symbols"` |
| `src/tools/symbol.rs` | 619 | `"find_referencing_symbols"` | `"find_references"` |
| `src/tools/symbol.rs` | 706 | `"replace_symbol_body"` | `"replace_symbol"` |
| `src/tools/workflow.rs` | 264 | `"check_onboarding_performed"` | `"is_onboarded"` |
| `src/tools/workflow.rs` | 307 | `"execute_shell_command"` | `"run_command"` |
| `src/tools/ast.rs` | 92 | `"extract_docstrings"` | `"list_docs"` |
| `src/tools/config.rs` | 52 | `"get_current_config"` | `"get_config"` |

**Step 2: Run tests to verify they fail**

Run: `cargo test server_registers_all_tools 2>&1 | head -30`
Expected: FAIL — the test expects old names that no longer match.

---

### Task 2: Update server registration test

**Files:**
- Modify: `src/server.rs` — lines 412-445 (`expected_tools` array)

**Step 1: Update the expected tool name strings**

In the `server_registers_all_tools` test, replace these 9 strings in the `expected_tools` array:

```rust
let expected_tools = [
    "read_file",
    "list_dir",
    "search_pattern",         // was "search_for_pattern"
    "create_file",            // was "create_text_file"
    "find_file",
    "edit_lines",
    "run_command",            // was "execute_shell_command"
    "onboarding",
    "is_onboarded",           // was "check_onboarding_performed"
    "find_symbol",
    "find_references",        // was "find_referencing_symbols"
    "list_symbols",           // was "get_symbols_overview"
    "replace_symbol",         // was "replace_symbol_body"
    "insert_before_symbol",
    "insert_after_symbol",
    "rename_symbol",
    "list_functions",
    "list_docs",              // was "extract_docstrings"
    "git_blame",
    "git_log",
    "git_diff",
    "write_memory",
    "read_memory",
    "list_memories",
    "delete_memory",
    "semantic_search",
    "index_project",
    "index_status",
    "check_drift",
    "activate_project",
    "get_config",             // was "get_current_config"
    "list_libraries",
    "index_library",
];
```

**Step 2: Run the registration test**

Run: `cargo test server_registers_all_tools -- --nocapture`
Expected: PASS

---

### Task 3: Update security gate — check_tool_access

**Files:**
- Modify: `src/util/path_security.rs` — lines 246-284 (`check_tool_access` function)

Three tool names in the match arms need updating:

**Step 1: Update the match arms**

In `check_tool_access`, change:
- `"execute_shell_command"` → `"run_command"` (shell gate, line ~247)
- `"create_text_file"` → `"create_file"` (file_write gate, line ~256)
- `"replace_symbol_body"` → `"replace_symbol"` (file_write gate, line ~258)

The updated function body:
```rust
pub fn check_tool_access(tool_name: &str, config: &PathSecurityConfig) -> Result<()> {
    match tool_name {
        "run_command" => {
            if !config.shell_enabled {
                bail!(
                    "Shell commands are disabled. Set security.shell_enabled = true in .code-explorer/project.toml to enable."
                );
            }
        }
        "create_file"
        | "edit_lines"
        | "replace_symbol"
        | "insert_before_symbol"
        | "insert_after_symbol"
        | "rename_symbol" => {
            if !config.file_write_enabled {
                bail!(
                    "File write tools are disabled. Set security.file_write_enabled = true in .code-explorer/project.toml to enable."
                );
            }
        }
        "git_blame" | "git_log" | "git_diff" => {
            if !config.git_enabled {
                bail!(
                    "Git tools are disabled. Set security.git_enabled = true in .code-explorer/project.toml to enable."
                );
            }
        }
        "semantic_search" | "index_project" | "index_status" => {
            if !config.indexing_enabled {
                bail!(
                    "Indexing tools are disabled. Set security.indexing_enabled = true in .code-explorer/project.toml to enable."
                );
            }
        }
        _ => {} // All other tools are always allowed
    }
    Ok(())
}
```

**Step 2: Run security tests to see them fail**

Run: `cargo test -p code-explorer check_tool_access 2>&1 | head -20`
Expected: FAIL — tests still reference old names.

---

### Task 4: Update security tests

**Files:**
- Modify: `src/util/path_security.rs` — tests at lines 544-583, 650-661

**Step 1: Update `shell_disabled_by_default` test (line ~547)**

Change `"execute_shell_command"` to `"run_command"`:
```rust
assert!(check_tool_access("run_command", &config).is_err());
```

**Step 2: Update `file_write_disabled_blocks_all_write_tools` test (line ~570-577)**

Change the tool name array:
```rust
for tool in &[
    "create_file",            // was "create_text_file"
    "edit_lines",
    "replace_symbol",         // was "replace_symbol_body"
    "insert_before_symbol",
    "insert_after_symbol",
    "rename_symbol",
] {
```

**Step 3: Update `check_tool_access_error_message_includes_config_hint` test (line ~652)**

Change `"execute_shell_command"` to `"run_command"`:
```rust
let err = check_tool_access("run_command", &config).unwrap_err();
```

**Step 4: Run all security tests**

Run: `cargo test -p code-explorer check_tool_access shell_disabled file_write_disabled -- --nocapture`
Expected: PASS

**Step 5: Run full test suite**

Run: `cargo test`
Expected: All ~432 tests PASS

**Step 6: Commit**

```bash
git add src/tools/file.rs src/tools/symbol.rs src/tools/workflow.rs src/tools/ast.rs src/tools/config.rs src/server.rs src/util/path_security.rs
git commit -m "refactor: rename 9 tools for API consistency

Renames:
- search_for_pattern → search_pattern
- create_text_file → create_file
- get_symbols_overview → list_symbols
- find_referencing_symbols → find_references
- replace_symbol_body → replace_symbol
- execute_shell_command → run_command
- check_onboarding_performed → is_onboarded
- extract_docstrings → list_docs
- get_current_config → get_config

Only name() strings change. Rust struct names unchanged.
Security gates and registration test updated.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 5: Add `limit` as alias for `max_results`

**Files:**
- Modify: `src/tools/file.rs` — SearchForPattern and FindFile (schemas + call methods)

**Step 1: Update SearchForPattern input_schema (line ~230)**

Add `limit` alongside `max_results`:
```rust
fn input_schema(&self) -> Value {
    json!({
        "type": "object",
        "required": ["pattern"],
        "properties": {
            "pattern": { "type": "string", "description": "Regex pattern" },
            "path": { "type": "string", "description": "File or directory to search (default: project root)" },
            "max_results": { "type": "integer", "default": 50, "description": "Maximum matches to return. Alias: limit" },
            "limit": { "type": "integer", "description": "Alias for max_results" }
        }
    })
}
```

**Step 2: Update SearchForPattern call to accept both (line ~255)**

Change:
```rust
let max = input["max_results"].as_u64().unwrap_or(50) as usize;
```
To:
```rust
let max = input["max_results"]
    .as_u64()
    .or_else(|| input["limit"].as_u64())
    .unwrap_or(50) as usize;
```

**Step 3: Update FindFile input_schema (line ~347)**

Same pattern — add `limit` property:
```rust
fn input_schema(&self) -> Value {
    json!({
        "type": "object",
        "required": ["pattern"],
        "properties": {
            "pattern": { "type": "string", "description": "Glob pattern" },
            "path": { "type": "string", "description": "Directory to search (default: current dir)" },
            "max_results": { "type": "integer", "default": 100, "description": "Maximum files to return. Alias: limit" },
            "limit": { "type": "integer", "description": "Alias for max_results" }
        }
    })
}
```

**Step 4: Update FindFile call to accept both (line ~372)**

Change:
```rust
let max = input["max_results"].as_u64().unwrap_or(100) as usize;
```
To:
```rust
let max = input["max_results"]
    .as_u64()
    .or_else(|| input["limit"].as_u64())
    .unwrap_or(100) as usize;
```

**Step 5: Run existing tests (they use `max_results` — should still pass)**

Run: `cargo test search_respects_max find_file_respects_max search_for_pattern_max -- --nocapture`
Expected: PASS (backward compat)

**Step 6: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: accept 'limit' as alias for 'max_results' in search/find tools

Both search_pattern and find_file now accept either 'limit' or
'max_results' for pagination. Existing max_results usage unchanged.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 6: Rewrite server_instructions.md

**Files:**
- Rewrite: `src/prompts/server_instructions.md`

**Step 1: Write the updated server instructions**

The full content must:
- Use all 9 new tool names consistently
- Fix `file` → `path` parameter references for editing tools
- Add `name_path` documentation for `find_symbol`
- Document `list_docs` (was missing as `extract_docstrings`)
- Add missing `search_pattern` params (was missing `max_results`/`limit`)
- Keep the "How to Choose" decision tree (it's effective)
- Tighten Rules section

Full replacement content:

```markdown
code-explorer MCP server: high-performance semantic code intelligence.
Provides file operations, symbol navigation (LSP), AST analysis (tree-sitter),
git history/blame, semantic search (embeddings), and project memory.

## How to Choose the Right Tool

### You know the name → use structure-aware tools
When you know the file path, function name, class name, or method name:
- `find_symbol(pattern)` — locate by name substring
- `list_symbols(path)` — see all symbols in a file/directory/glob
- `list_functions(path)` — quick signatures via tree-sitter (no LSP needed)
- `find_references(name_path, path)` — find all usages

### You know the concept → use semantic search first
When you're exploring by domain ("how are errors handled", "authentication flow"):
- `semantic_search(query)` — find relevant code by natural language
- Then drill down: `list_symbols(found_file)` → `find_symbol(name, include_body=true)`

### You know nothing → start with the map
When exploring an unfamiliar area:
1. `list_dir(path)` — see directory structure (shallow by default)
2. `list_symbols(interesting_file)` — see what's in each file
3. `semantic_search("what does this module do")` — get the high-level picture
4. Then drill into specifics with `find_symbol` once you know what to look for

### You want to know what changed meaningfully
After re-indexing with `index_project`, check `check_drift` to see which files
had significant semantic changes vs. trivial formatting/comment edits.

### You need to read library/dependency code
When you need to understand how a third-party library works:
1. Navigate to a library symbol via `find_symbol` — external paths are auto-discovered
2. `list_libraries` — see what's already registered
3. Use `scope: "lib:<name>"` on symbol tools to search within a specific library
4. `index_library(name)` then `semantic_search(query, scope: "lib:<name>")` for deeper exploration

## Output Modes

Tools default to **exploring** mode — compact output (names, locations, counts)
capped at 200 items.

When you need full detail (function bodies, all children, complete diffs):
- Pass `detail_level: "full"` to get focused mode
- Use `offset` and `limit` to paginate through large results
- Only switch to focused mode AFTER you've identified specific targets

### Progressive disclosure pattern
1. **Explore broadly:** `list_symbols("src/services/")` → compact map of all files
2. **Identify target:** spot the file/symbol you need from the overview
3. **Focus narrowly:** `find_symbol("handleAuth", path="src/services/auth.rs", include_body=true, detail_level="full")`

### Overflow messages
When results exceed the cap, you'll see:
```json
{ "overflow": { "shown": 47, "total": 312, "hint": "Narrow with a file path or glob pattern" } }
```
Follow the hint to refine your query.

## Tool Reference

### Symbol Navigation (LSP-backed)
- `find_symbol(pattern, [path], [include_body], [depth], [detail_level], [scope])` — find symbols by name. Also accepts `name_path` (exact path from list_symbols, e.g. 'MyStruct/my_method') as alternative to `pattern`.
- `list_symbols([path], [depth], [detail_level], [scope])` — symbol tree for file/dir/glob
- `find_references(name_path, path, [detail_level], [scope])` — find all usages of a symbol
- `list_functions(path, [scope])` — quick function signatures via tree-sitter (no LSP needed)

### Reading & Searching
- `read_file(path, [start_line], [end_line])` — read non-code files (README, configs, TOML, JSON, YAML). Blocked for source code files — use symbol tools instead.
- `semantic_search(query, [limit], [scope])` — find code by natural language description
- `search_pattern(pattern, [path], [limit])` — regex search across the project or within a specific file
- `find_file(pattern, [path], [limit])` — find files by glob pattern
- `check_drift([threshold], [path])` — query semantic drift scores from last index build *(opt out with `drift_detection_enabled = false` in `[embeddings]`)*

### Editing
- `replace_symbol(name_path, path, new_body)` — replace a function/method body
- `insert_before_symbol(name_path, path, code)` / `insert_after_symbol(...)` — insert code adjacent to a symbol
- `rename_symbol(name_path, path, new_name)` — rename across codebase (LSP)
- `edit_lines(path, start_line, delete_count, [new_text])` — line-based splice edit. Use for non-code files or intra-symbol edits where you already know the line numbers.
- `create_file(path, content)` — create or overwrite a file. Creates parent directories as needed.

### AST Analysis
- `list_functions(path, [scope])` — quick function signatures via tree-sitter
- `list_docs(path)` — extract docstrings and top-level comments with associated symbol names

### Git
- `git_blame(path, [start_line], [end_line], [detail_level])` — line-by-line blame
- `git_log([path], [limit])` — commit history (default: 20)
- `git_diff([commit], [path], [detail_level])` — uncommitted changes or diff against commit

### Project Memory
- `write_memory(topic, content)` / `read_memory(topic)` / `list_memories()` / `delete_memory(topic)`

### Library Navigation
- `list_libraries` — show all registered third-party libraries and their status
- `index_library(name, [force])` — build embedding index for a registered library

**Scope parameter:** Symbol and search tools accept an optional `scope` parameter to target library code:
- `"project"` (default) — only project code
- `"lib:<name>"` — a specific registered library (e.g. `"lib:serde"`)
- `"libraries"` — all registered libraries
- `"all"` — project + all libraries

Tools with `scope`: `find_symbol`, `list_symbols`, `find_references`, `list_functions`, `semantic_search`

**Auto-discovery:** Libraries are automatically discovered and registered when LSP returns paths outside the project root (e.g. via goto_definition). Discovery walks up parent directories looking for package manifests (Cargo.toml, package.json, pyproject.toml, go.mod).

**Source tagging:** All results include a `"source"` field: `"project"` or `"lib:<name>"` to distinguish origin.

### Project Management
- `onboarding` — first-time project discovery and memory creation
- `is_onboarded` — check if onboarding has been performed
- `run_command(command, [timeout_secs])` — run shell commands in project root
- `activate_project(path)` — switch active project
- `get_config` — show project config and server settings

## Rules

1. **PREFER symbol tools over reading entire files.** `list_symbols` + `find_symbol(include_body=true)` is almost always more efficient than `read_file`.
2. **`read_file` only works for non-code files** (README, configs, TOML, JSON, YAML). It will reject source code files — use `list_symbols` + `find_symbol(include_body=true)` instead.
3. **Start with semantic search for "how does X work?" questions.** Then drill into results with symbol tools.
4. **Use exploring mode first.** Only switch to `detail_level: "full"` after you've identified what you need.
5. **Respect overflow hints.** When a tool says "narrow with a file path or glob", do it — don't re-run the same broad query.
6. **Use `list_functions` for quick overviews** when you just need signatures, not full symbol trees.
7. **For edits to code files, prefer symbol tools** (`replace_symbol`, `insert_before_symbol`) over `edit_lines`. Use `edit_lines` for non-code files or intra-symbol edits where you already know the line numbers.
```

**Step 2: Run test suite to verify nothing breaks**

Run: `cargo test`
Expected: All tests PASS (server_instructions.md is read at runtime, not tested directly)

**Step 3: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs: rewrite server instructions with new tool names

Updates all tool names, fixes parameter references (file → path),
adds name_path docs for find_symbol, documents list_docs,
adds missing search_pattern parameters.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 7: Final verification and cleanup

**Step 1: Run full test suite + lint**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All clean, all tests pass

**Step 2: Grep for any stale old tool names**

Run these searches to ensure no references to old names remain:

```bash
# Check source files for stale names (excluding git history, docs/plans)
grep -rn '"search_for_pattern"\|"create_text_file"\|"get_symbols_overview"\|"find_referencing_symbols"\|"replace_symbol_body"\|"execute_shell_command"\|"check_onboarding_performed"\|"extract_docstrings"\|"get_current_config"' src/
```

Expected: No matches (all renamed)

**Step 3: Update MEMORY.md tool count**

The auto-memory file at `/home/marius/.claude-sdd/projects/-home-marius-work-claude-code-explorer/memory/MEMORY.md` says "30 tools" — update to "33 tools".

**Step 4: Update CLAUDE.md project structure if needed**

Check if `CLAUDE.md` references any old tool names and update.
