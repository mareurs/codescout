# Tool API Redesign: Naming & Query-Shape Detection

Two improvements to the core tool API: consistent parameter naming across all
symbol and search tools, and automatic detection of regex intent in
`symbols` to prevent confusing errors.

---

## API Naming Changes

Parameter names have been standardized across symbol and search tools. Old names
are removed — update any saved prompts or scripts.

### Parameter renames

| Tool | Old param | New param |
|---|---|---|
| `symbols` | `name_path` | `symbol` |
| `symbols` | `pattern` | `query` |
| `references` | `name_path` | `symbol` |
| `symbol_at` | `name_path` | `symbol` |
| `replace_symbol` | `name_path` | `symbol` |
| `remove_symbol` | `name_path` | `symbol` |
| `rename_symbol` | `name_path` | `symbol` |
| `semantic_search` | `project` | `project_id` |
| `memory` | `project` | `project_id` |

### Tool renames

| Old name | New name |
|---|---|
| `search_pattern` | `grep` |
| `find_file` | `glob` |

The renamed tools (`grep`, `glob`) are shorter and match the mental model
agents already have for these operations.

---

## Query-Shape Detection in symbols

`symbols` now detects when a `query` looks like a regex pattern and
returns a corrective hint instead of silently returning wrong results.

### Problem it solves

Agents occasionally pass regex patterns to `symbols` expecting it to
match multiple symbols — but `symbols` does substring matching on symbol
names, not regex. A query like `handle_.*_event` matches nothing (or
coincidentally matches a symbol with `.*` in its name), giving a misleading
empty result.

### Behavior

If the query contains regex metacharacters (`.*`, `.+`, `^`, `$`, `\w`,
`\d`, `|`, `(...)`) it is flagged as regex-like and a `RecoverableError`
is returned:

```json
{
  "error": "query looks like a regex pattern — use grep(pattern=...) for regex search",
  "hint": "symbols matches by substring; grep searches file content by pattern"
}
```

### When it fires

| Query | Detected as | Action |
|---|---|---|
| `handle_event` | plain substring | normal symbol search |
| `handle_.*_event` | regex-like | RecoverableError → redirect to grep |
| `^MyStruct$` | regex-like | RecoverableError → redirect to grep |
| `foo\|bar` | regex-like | RecoverableError → redirect to grep |

### Correct tool for each intent

```
// Find a symbol by name substring
symbols(query="handle_event")

// Find code matching a pattern across files
grep(pattern="handle_.*_event")
```
