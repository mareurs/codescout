code-explorer MCP server: high-performance semantic code intelligence for Claude Code.

## How to Explore Code

You have access to semantic tools that understand code structure. PREFER these over Claude Code's built-in Read/Grep/Glob for source files.

### Symbol-Level Navigation (most token-efficient)
- `find_symbol(pattern)` — find functions, classes, methods by name substring
- `get_symbols_overview(path)` — see all symbols in a file or directory (like a table of contents)
- `find_referencing_symbols(name_path, file)` — find all usages of a symbol across the project
- `list_functions(path)` — quick function/method signatures via tree-sitter (no LSP needed)

### Reading Source Code
- `find_symbol(pattern, include_body=true)` — read a specific symbol's full source code
- `get_symbols_overview(path, depth=1)` — see structure with direct children before diving in
- `read_file(path, start_line, end_line)` — targeted line ranges when you already know where to look

### Discovery & Search
- `semantic_search(query)` — find code by natural language description ("how are errors handled")
- `search_for_pattern(pattern)` — regex search across the project (for literal strings, config values)
- `find_file(pattern)` — find files by glob pattern (e.g. "**/*.rs", "src/**/mod.rs")

### Editing Code
- `replace_symbol_body(name_path, file, new_body)` — replace a function/method body
- `insert_before_symbol(name_path, file, code)` — insert code before a symbol
- `insert_after_symbol(name_path, file, code)` — insert code after a symbol
- `rename_symbol(name_path, file, new_name)` — rename across the codebase
- `replace_content(path, old, new)` — find-and-replace text in a file
- `create_text_file(path, content)` — create or overwrite a file

### Git Integration
- `git_blame(path)` — who changed each line and when
- `git_log(path?)` — commit history for a file or the whole project
- `git_diff(commit?, path?)` — show uncommitted changes or diff against a commit

### Project Memory
- `write_memory(topic, content)` — persist knowledge about the project
- `read_memory(topic)` — retrieve a stored memory entry
- `list_memories()` — see all available memory topics
- `delete_memory(topic)` — remove a memory entry

## Workflow Patterns

### Understand Before Editing
1. `get_symbols_overview(file)` — see what's in the file
2. `find_symbol(name, include_body=true)` — read the specific symbol you need
3. Edit using `replace_symbol_body` or `insert_after_symbol`

### Find Usages Before Refactoring
1. `find_symbol(name)` — locate the symbol definition
2. `find_referencing_symbols(name_path, file)` — find all references
3. `rename_symbol(name_path, file, new_name)` — safe cross-project rename

### Discover Then Drill Down
1. `semantic_search("how does X work")` — find relevant code by intent
2. `get_symbols_overview(found_file)` — understand the file structure
3. `find_symbol(specific_name, include_body=true)` — read the details

### Explore Unfamiliar Code
1. `list_dir(path, recursive=false)` — see directory structure
2. `get_symbols_overview(interesting_file)` — map the symbols
3. `find_symbol(key_type, include_body=true)` — read core abstractions
4. `find_referencing_symbols(key_type, file)` — see how it's used

## Rules

- PREFER `get_symbols_overview` + `find_symbol(include_body=true)` over reading entire source files
- Use `read_file` for non-code files (README, configs, docs, TOML, JSON, YAML) or targeted line ranges
- Use `semantic_search` for "how does X work?" questions; use `find_symbol` for "where is X defined?"
- Use `list_functions` for a quick overview when you just need signatures, not full symbol trees
- Use `extract_docstrings` to understand a file's API documentation
