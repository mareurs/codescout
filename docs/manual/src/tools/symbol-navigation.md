# Symbol Navigation

These eight tools give you IDE-grade code navigation backed by the Language
Server Protocol. They understand code structure — symbols, definitions,
references, types — not just bytes and lines.

**Supported languages:** Rust, Python, TypeScript, JavaScript, TSX, JSX, Go,
Java, Kotlin, C, C++, C#, Ruby.

All symbol navigation tools require an LSP server for the target language. If
the LSP server is not running or is still indexing, some tools fall back to
tree-sitter for basic results.

**Scope parameter:** `symbols`, `symbols`, and `references` accept an optional `scope` string to search library code as well as project code. See [Library Navigation](library-navigation.md) for the full scope reference.

### Workspace project scoping

In a [multi-project workspace](../concepts/multi-project-workspace.md), pass
`project` to scope operations to a specific project:

```json
{ "tool": "symbols", "arguments": { "pattern": "UserService", "project": "backend" } }
```

`scope` and `project` are independent axes: `scope` selects project vs library
code, `project` selects which project in the workspace. Omitting `project`
uses the workspace-level context.

> **See also:** [Tool Selection](../concepts/tool-selection.md) — when to reach
> for symbol tools vs semantic search vs text search. [Progressive Disclosure](../concepts/progressive-disclosure.md) — how `detail_level` controls output volume for these tools.

---

## `symbols`

**Purpose:** Return the symbol tree (functions, classes, methods, structs, etc.)
for a file, directory, or glob pattern. Use this to orient yourself before
reading or editing.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `relative_path` | string | no | project root | File, directory, or glob pattern (e.g. `src/**/*.rs`) |
| `depth` | integer | no | 1 | Depth of children to include (0 = top-level names only, 1 = direct children) |
| `detail_level` | string | no | exploring | `"full"` activates focused mode with symbol bodies |
| `offset` | integer | no | 0 | Skip this many files (focused mode pagination) |
| `limit` | integer | no | 50 | Max files per page |

**Example — outline a single file:**

```json
{
  "tool": "symbols",
  "arguments": {
    "relative_path": "src/auth/middleware.rs"
  }
}
```

**Output (exploring mode):**

```json
{
  "file": "src/auth/middleware.rs",
  "symbols": [
    { "name": "AuthMiddleware", "kind": "Struct", "start_line": 12, "end_line": 18 },
    { "name": "new",            "kind": "Function", "start_line": 21, "end_line": 28 },
    { "name": "handle",         "kind": "Function", "start_line": 31, "end_line": 74 },
    { "name": "verify_token",   "kind": "Function", "start_line": 77, "end_line": 102 }
  ]
}
```

**Output (focused mode — `detail_level: "full"`):**

Same structure, but each symbol includes a `"body"` field with the source lines
from `start_line` to `end_line`.

**Example — overview of a directory:**

```json
{
  "tool": "symbols",
  "arguments": {
    "relative_path": "src/handlers/"
  }
}
```

Returns one entry per file in the directory, each with its symbol list. At the
project root (`.`), walks the entire source tree recursively.

**Example — glob across multiple files:**

```json
{
  "tool": "symbols",
  "arguments": {
    "relative_path": "src/**/*.py",
    "depth": 0
  }
}
```

With `depth: 0`, returns only top-level symbol names, which is useful for a
very high-level map without the per-method detail.

**Tips:**

- Use this before reading or editing. Two tokens spent on the map saves dozens
  spent re-reading the wrong file.
- When the result overflows, the response includes an `overflow` object with
  a hint. Narrow with a more specific path or glob.
- For deep class hierarchies, increase `depth` to 2 or 3.
- Use `detail_level: "full"` only after you have identified the specific file
  you need — fetching bodies for an entire directory is expensive.

---

## `symbols`

**Purpose:** Find symbols by name pattern across the project or within a
specific file. Returns matching symbols with their location and, optionally,
their source body.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `pattern` | string | yes | — | Symbol name or substring (case-insensitive) |
| `relative_path` | string | no | — | Restrict to this file or glob pattern |
| `include_body` | boolean | no | false | Include source body in results |
| `depth` | integer | no | 0 | Depth of children to include |
| `detail_level` | string | no | exploring | `"full"` for bodies and pagination |
| `offset` | integer | no | 0 | Skip this many results |
| `limit` | integer | no | 50 | Max results per page |

**Example — find a function anywhere in the project:**

```json
{
  "tool": "symbols",
  "arguments": {
    "pattern": "authenticate_user"
  }
}
```

**Output (exploring mode):**

```json
{
  "symbols": [
    {
      "name": "authenticate_user",
      "name_path": "AuthService/authenticate_user",
      "kind": "Function",
      "file": "src/auth/service.rs",
      "start_line": 44,
      "end_line": 89
    }
  ],
  "total": 1
}
```

**Example — find with body, restricted to one file:**

```json
{
  "tool": "symbols",
  "arguments": {
    "pattern": "authenticate_user",
    "relative_path": "src/auth/service.rs",
    "include_body": true,
    "detail_level": "full"
  }
}
```

**Output (focused mode with body):**

```json
{
  "symbols": [
    {
      "name": "authenticate_user",
      "name_path": "AuthService/authenticate_user",
      "kind": "Function",
      "file": "src/auth/service.rs",
      "start_line": 44,
      "end_line": 89,
      "body": "pub fn authenticate_user(&self, credentials: Credentials) -> Result<Session> {\n    let user = self.user_store.find_by_email(&credentials.email)?;\n    ..."
    }
  ],
  "total": 1
}
```

**Example — find all test functions in test files:**

```json
{
  "tool": "symbols",
  "arguments": {
    "pattern": "test_",
    "relative_path": "tests/**/*.rs"
  }
}
```

**Tips:**

- Pattern matching is case-insensitive substring matching. `"auth"` matches
  `AuthService`, `authenticate_user`, and `reauth_token`.
- Without `relative_path`, uses `workspace/symbol` (one LSP request per
  language), which is fast. With `relative_path`, uses per-file document
  symbols, which is slower but scoped.
- `name_path` in the result uses `/` as a separator for nested symbols, e.g.
  `AuthService/authenticate_user`. You need this value for
  `references`, `edit_code`, and related editing tools.
- Use `include_body: true` in the same call to avoid a separate read step when
  you already know the symbol name.

---

## `references`

**Purpose:** Find all locations in the codebase that reference (call, use,
import) a given symbol. This is the "find all usages" feature from your IDE.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `name_path` | string | yes | — | Symbol identifier, e.g. `"MyStruct/my_method"` |
| `relative_path` | string | yes | — | File that contains the symbol definition |
| `detail_level` | string | no | exploring | `"full"` for pagination |
| `offset` | integer | no | 0 | Skip this many results |
| `limit` | integer | no | 50 | Max results per page |

**Example — find all callers of a method:**

```json
{
  "tool": "references",
  "arguments": {
    "name_path": "AuthService/authenticate_user",
    "relative_path": "src/auth/service.rs"
  }
}
```

**Output (exploring mode):**

```json
{
  "references": [
    {
      "file": "src/handlers/login.rs",
      "line": 38,
      "column": 18,
      "context": "        let session = self.auth.authenticate_user(credentials)?;"
    },
    {
      "file": "src/handlers/api.rs",
      "line": 204,
      "column": 22,
      "context": "        auth_service.authenticate_user(req.credentials.clone())?;"
    },
    {
      "file": "tests/auth_integration.rs",
      "line": 91,
      "column": 12,
      "context": "    let result = service.authenticate_user(bad_creds);"
    }
  ],
  "total": 3
}
```

**Example — paginate a high-reference symbol:**

```json
{
  "tool": "references",
  "arguments": {
    "name_path": "Logger/log",
    "relative_path": "src/logging.rs",
    "detail_level": "full",
    "offset": 0,
    "limit": 25
  }
}
```

**Tips:**

- Both `name_path` and `relative_path` are required. The LSP needs to locate
  the symbol's definition position before it can find references.
- `name_path` must match the `name_path` value from `symbols` or
  `symbols` output, not just the bare name. For a top-level function, the
  name_path is just the function name (e.g. `"validate_token"`). For a method,
  it is `"StructName/method_name"`.
- Each reference includes a `context` line showing the source at that location,
  so you can often determine the call pattern without reading the file.
- For symbols with many references (e.g. utility functions, common types),
  use `detail_level: "full"` with `offset`/`limit` pagination.

---

## `replace_symbol`

> **Renamed in v0.11.** The standalone `replace_symbol` tool was consolidated
> into the unified `edit_code` tool. Use `edit_code(action="replace", symbol, path, body)`
> instead — see [edit_code](edit-code.md) for parameters, examples, and the
> full action set (`replace` / `insert` / `remove` / `rename`).

---
## `insert_code`

> **Renamed in v0.11.** Consolidated into `edit_code`. Use
> `edit_code(action="insert", symbol, path, body, position="before"|"after")`
> instead — see [edit_code](edit-code.md).

---
## `rename_symbol`

> **Renamed in v0.11.** Consolidated into `edit_code`. Use
> `edit_code(action="rename", symbol, path, new_name)` instead — see
> [edit_code](edit-code.md). The implementation still goes through LSP
> `workspace/rename` and sweeps textual occurrences in comments and strings.

---
## `remove_symbol`

> **Renamed in v0.11.** Consolidated into `edit_code`. Use
> `edit_code(action="remove", symbol, path)` instead — see
> [edit_code](edit-code.md).

---
## `symbol_at`

**Purpose:** Inspect a symbol at a given position via LSP. Returns the symbol's
definition location(s) (`def`) and/or type information + doc comments (`hover`).
Pass the `fields` parameter to choose which queries to run; the default runs
both. When a definition lives outside the project root, the library is
auto-discovered and registered in `library(action: list)`.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `path` | string | yes | — | File path (relative or absolute) |
| `line` | integer | yes | — | 1-indexed line number |
| `col` | integer | no | — | 1-indexed column. Preferred when known — LSP-native, no identifier-mismatch risk |
| `identifier` | string | no | — | Optional identifier on the line to target (fallback when `col` not known) |
| `fields` | array of strings | no | `["def", "hover"]` | Which LSP queries to run. Allowed values: `"def"`, `"hover"` |

**Example — get both definition and hover info for a symbol on line 42:**

```json
{
  "tool": "symbol_at",
  "arguments": {
    "path": "src/tools/symbol.rs",
    "line": 42
  }
}
```

**Output:**

```json
{
  "def": {
    "definitions": [
      {
        "file": "src/lsp/symbols.rs",
        "line": 12,
        "end_line": 28,
        "context": "pub struct SymbolInfo {"
      }
    ],
    "from": "symbol.rs:42"
  },
  "hover": {
    "content": "pub struct SymbolInfo\n\nMetadata about a symbol returned by the LSP.",
    "location": "symbol.rs:42"
  }
}
```

When the definition is in a library (outside the project root), each entry in
`def.definitions` carries a `source` tag (e.g. `"lib:serde"`) and the library
is added to `library(action: list)`.

**Example — hover only:**

```json
{
  "tool": "symbol_at",
  "arguments": {
    "path": "src/tools/symbol.rs",
    "line": 55,
    "fields": ["hover"]
  }
}
```

**Tips:**

- Use `symbol_at` to quickly locate where a type, trait, or function is defined
  and to see its type signature in one round-trip.
- Pass `fields: ["def"]` when you only need the location, or
  `fields: ["hover"]` when you only need the type signature — saves an LSP
  round-trip.
- Supply `col` (1-indexed) when known for LSP-native targeting. Fall back to
  `identifier` to locate by name on the line; prefer `col` to avoid
  identifier-mismatch errors.
- When the definition is in an external library, run
  `index(action: build, scope: "lib:<name>")` to enable `semantic_search` across it.
- For hover, if `content` is null, the language server has no information at
  that position — try adjusting `line`/`col` or supplying `identifier`.

---
