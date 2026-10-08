# `edit_code`

Mutate a symbol in the codebase. Four actions: `replace`, `insert`, `remove`, `rename`.

Consolidates the older individual tools (`replace_symbol`, `insert_code`, `rename_symbol`, `remove_symbol`) into a single action-dispatched tool.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `symbol` | string | yes | Symbol identifier — plain name (`my_fn`) or hierarchical (`MyStruct/my_method`) |
| `path` | string | yes | File containing the symbol |
| `action` | string | yes | One of `replace`, `insert`, `remove`, `rename` |
| `body` | string | action-dependent | `replace`: new full body; `insert`: code to inject |
| `position` | string | no | `insert` only — `"before"` or `"after"` the symbol (default `"after"`) |
| `reindent` | boolean | no | `replace` and `insert` only, default `true`. `false` splices `body` exactly as written, with no re-basing onto the symbol's column — see [Indentation](#indentation) |
| `new_name` | string | rename only | New identifier for the symbol |
| `force` | boolean | no | `replace` only, default `false`: acknowledge an intentional reduction of more than half the replaced symbol’s bytes or lines (original range at least 200 bytes) |

### Indentation

By default `edit_code` re-bases `body` so its least-indented line sits at the symbol's own column. That is what you want when you write a method at the left margin and mean it to become a class member: it is indented to fit.

It is wrong for a body that mixes levels. A method and a separate top-level class, both written at the left margin, are shifted together, and the class ends up nested inside the enclosing one. In Python that is still valid code, so nothing reports it. The tool cannot tell "both belong inside" from "one belongs outside" by reading the text.

Pass `reindent: false` and the body is spliced **exactly as written**, with your own indentation. A body whose first line is already at the symbol's column is left alone without the flag; `reindent: false` is for the case where it is not. A value that is not `true` or `false` is refused and nothing is written.

## Actions

### `replace`

Replaces the symbol’s complete declaration with the supplied `body`, including its signature. Read the full symbol first with `symbols(include_body=true)`.

A replacement that removes more than half the replaced range’s bytes or lines is refused before writing when the original range is at least 200 bytes. If that reduction is intentional, pass `force: true`. The name, syntax, and sibling checks still apply.

```json
{
  "symbol": "MyStruct/validate",
  "path": "src/model.rs",
  "action": "replace",
  "body": "    fn validate(&self) -> bool {\n        !self.name.is_empty()\n    }"
}
```

On success, returns a hint listing callers if any were found — use it to verify that the new body is compatible.

### `insert`

Injects code adjacent to a symbol. `"after"` (default) places the new code immediately below the symbol; `"before"` places it above. Use this to add a sibling method or a helper next to an existing definition.

```json
{
  "symbol": "MyStruct/validate",
  "path": "src/model.rs",
  "action": "insert",
  "body": "    fn is_empty(&self) -> bool {\n        self.name.is_empty()\n    }",
  "position": "after"
}
```

### `remove`

Deletes the symbol and its full body from the file.

```json
{
  "symbol": "MyStruct/deprecated_helper",
  "path": "src/model.rs",
  "action": "remove"
}
```

### `rename`

Renames the symbol across the entire codebase via LSP `workspace/rename`. Follows references through type aliases, trait implementations, and macro invocations. Also sweeps textual occurrences in comments and string literals.

```json
{
  "symbol": "process_payload",
  "path": "src/handler.rs",
  "action": "rename",
  "new_name": "handle_payload"
}
```

On success, reports how many files were changed and hints at verifying call sites.

## When to use `edit_code` vs `edit_file`

| Scenario | Tool |
|----------|------|
| Change a function or method body | `edit_code(action="replace")` |
| Add a sibling method or definition | `edit_code(action="insert")` |
| Delete a function, struct, or method | `edit_code(action="remove")` |
| Rename a symbol project-wide | `edit_code(action="rename")` |
| Change an import or `use` line | `edit_file` |
| Change a constant value | `edit_file` |
| Edit a config or data file | `edit_file` |

`edit_code` uses LSP for symbol resolution and is robust to line number shifts. `edit_file` is a plain text find-and-replace — use it for lines that are not part of a symbol body.
