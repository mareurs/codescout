# Tool API Refactor Design

Date: 2026-02-27

## Problem

The 33-tool API surface has accumulated naming inconsistencies, parameter
mismatches, and stale documentation over rapid development. This creates
friction for Claude Code — the primary consumer — which sees tools as
`mcp__code-explorer__<tool_name>` in its context window.

### Specific Issues

1. **Naming inconsistency**: Mixed verbs (`get_*`, `check_*`, `execute_*`)
   where simpler forms exist. Some names are verbose
   (`find_referencing_symbols`, `execute_shell_command`,
   `check_onboarding_performed`).

2. **Parameter naming**: `find_referencing_symbols` uses `file` while sibling
   tools use `path`. Pagination uses `max_results` in some tools and `limit`
   in others.

3. **Stale server instructions**: `server_instructions.md` references wrong
   parameter names (`file` for editing tools), omits `name_path` on
   `find_symbol`, doesn't document `extract_docstrings`, and lists 4 missing
   parameters for `search_for_pattern`.

4. **Tool descriptions lack guidance**: Descriptions say *what* a tool does
   but not *when* to use it or *why* to prefer it over alternatives.

## Context: How Claude Code Consumes MCP Tools

Understanding the consumption context is critical for design decisions:

- **Tool naming**: Appears as `mcp__code-explorer__<tool_name>`. Shorter names
  reduce visual noise in tool lists and selection.

- **Server instructions**: Injected as system-level guidance. This is the
  primary place to teach tool selection strategy — descriptions on individual
  tools are secondary.

- **Schema + description per tool**: Each tool's JSON schema and description
  are presented individually. Descriptions must be self-contained because
  subagents get tools with fresh context (no server instructions carry-over
  unless the server re-injects them).

- **Output budget**: Claude Code warns at ~10K tokens of tool output. Our
  progressive disclosure already handles this, but tool descriptions should
  reinforce it.

- **Parallel tool calls**: Claude Code can fire multiple tool calls in
  parallel. `RecoverableError` (isError: false) doesn't abort sibling calls.
  Tool errors should be recoverable when possible.

- **33 tools total**: Each tool's schema costs context tokens. Concise
  descriptions and minimal required parameters matter.

## Approved Changes

### 1. Tool Renames (9 tools)

| Current Name | New Name | Rationale |
|---|---|---|
| `get_symbols_overview` | `list_symbols` | Consistent with `list_*` pattern (list_dir, list_functions, list_libraries, list_memories) |
| `search_for_pattern` | `search_pattern` | Shorter, parallel with `semantic_search` |
| `find_referencing_symbols` | `find_references` | Shorter, same concept |
| `extract_docstrings` | `list_docs` | Consistent with `list_*` pattern |
| `replace_symbol_body` | `replace_symbol` | Shorter, clear from context |
| `create_text_file` | `create_file` | Shorter, "text" is obvious |
| `execute_shell_command` | `run_command` | Shorter, idiomatic |
| `check_onboarding_performed` | `is_onboarded` | Boolean query → `is_*` pattern |
| `get_current_config` | `get_config` | Drop redundant "current" |

### 2. Parameter Standardization

**File path parameter**: All tools use `path` as primary, with `relative_path`
and `file` as backward-compatible fallbacks via `get_path_param()`. Already
done for symbol tools (commit `0f8c022`), needs verification for remaining
tools.

**Pagination parameter**: Unify `max_results` (in `search_for_pattern`,
`find_file`) to `limit` (matching `semantic_search`, `git_log`). Keep
`max_results` as backward-compatible fallback.

**Output control**: `detail_level` parameter is consistent across tools.
No changes needed.

### 3. Description Improvements

Each tool description should follow this template:
```
<one-line summary>. <when to use / when to prefer over alternatives>.
```

Example:
```
Before: "Find files matching a glob pattern."
After:  "Find files by glob pattern (e.g. '**/*.rs'). Use when you know the filename pattern; use semantic_search for concept-level queries."
```

### 4. Server Instructions Rewrite

Complete rewrite of `src/prompts/server_instructions.md`:
- Update all tool names to post-rename values
- Fix all parameter names (`file` → `path`, `max_results` → `limit`)
- Add `name_path` documentation for `find_symbol`
- Add missing tools (`list_docs` / `extract_docstrings`)
- Add missing parameters for `search_pattern` / `search_for_pattern`
- Keep the "How to Choose the Right Tool" decision tree — it's effective
- Tighten the "Rules" section to be more actionable

## Naming Conventions (Post-Refactor)

| Pattern | Usage | Examples |
|---|---|---|
| `list_*` | Enumerate items | `list_dir`, `list_symbols`, `list_functions`, `list_docs`, `list_libraries`, `list_memories` |
| `find_*` | Search by criteria | `find_symbol`, `find_references`, `find_file` |
| `search_*` | Text/semantic search | `search_pattern`, `semantic_search` |
| `read_*` / `write_*` | CRUD on single items | `read_file`, `read_memory`, `write_memory` |
| `git_*` | Git operations | `git_blame`, `git_log`, `git_diff` |
| `index_*` | Build/query indexes | `index_project`, `index_library`, `index_status` |
| `is_*` | Boolean queries | `is_onboarded` |
| Verb alone | Action tools | `onboarding`, `replace_symbol`, `rename_symbol`, `create_file`, `run_command` |

## Complete Tool Inventory (Post-Refactor)

### File Operations (6)
1. `read_file` — read non-code files
2. `list_dir` — directory listing
3. `search_pattern` — regex search *(renamed)*
4. `find_file` — glob file search
5. `create_file` — create/overwrite file *(renamed)*
6. `edit_lines` — line-based splice edit

### Symbol Navigation (7)
7. `find_symbol` — find symbols by name/name_path
8. `find_references` — find all usages of a symbol *(renamed)*
9. `list_symbols` — symbol tree for file/dir/glob *(renamed)*
10. `replace_symbol` — replace function/method body *(renamed)*
11. `insert_before_symbol` — insert code before symbol
12. `insert_after_symbol` — insert code after symbol
13. `rename_symbol` — rename across codebase (LSP)

### AST Analysis (2)
14. `list_functions` — quick function signatures
15. `list_docs` — extract docstrings *(renamed)*

### Git (3)
16. `git_blame` — line-level blame
17. `git_log` — commit history
18. `git_diff` — uncommitted changes

### Memory (4)
19. `write_memory` — persist knowledge
20. `read_memory` — read stored knowledge
21. `list_memories` — list all topics
22. `delete_memory` — remove knowledge

### Semantic Search (4)
23. `semantic_search` — natural language code search
24. `index_project` — build/update embedding index
25. `index_status` — index stats
26. `check_drift` — semantic drift scores

### Library Navigation (2)
27. `list_libraries` — show registered libraries
28. `index_library` — build library index

### Project Management (4)
29. `onboarding` — first-time project discovery
30. `is_onboarded` — check onboarding status *(renamed)*
31. `run_command` — execute shell command *(renamed)*
32. `activate_project` — switch active project
33. `get_config` — show project config *(renamed)*

## Implementation Scope

Each rename requires updates in these locations:

1. **Tool struct + `impl Tool`** — `name()` return value
2. **`server.rs` `from_parts`** — struct name in `Arc::new()`
3. **`server_registers_all_tools` test** — expected name string
4. **`check_tool_access`** in `path_security.rs` — if write tool
5. **Corresponding `*_disabled_blocks_*` test** — if write tool
6. **`server_instructions.md`** — all references

Struct names in Rust stay PascalCase (e.g., `SearchPattern`, `CreateFile`).
Only the `name()` string changes.

## Out of Scope

- Tool consolidation/removal (no tools are being removed)
- New tool creation
- Behavioral changes to any tool
- Changes to progressive disclosure or output modes
- Changes to the Tool trait itself

## Risk Assessment

**Low risk**: All changes are mechanical renames. No behavioral changes.
Backward compatibility via `get_path_param()` fallbacks for parameter renames.

**Testing**: Existing 432+ tests validate behavior. Only `name()` strings
and parameter extraction need updating in tests.

**Downstream impact**: The companion `code-explorer-routing` plugin references
tool names in its hooks — these must be updated in sync. Server instructions
are the primary documentation and will be rewritten as part of this work.
