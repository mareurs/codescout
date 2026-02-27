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
