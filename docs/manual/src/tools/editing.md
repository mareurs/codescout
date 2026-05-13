# Editing

codescout provides two categories of editing tools:

- **Text-level editing** — `edit_file` finds and replaces an exact string in a file.
- **Symbol-level editing** — `edit_code` mutates named code symbols located via the LSP (action: `replace`, `insert`, `remove`, `rename`). This is the preferred tool for editing source code.

All write operations are restricted to the active project root. Attempts to write outside the project root, or to paths on the security deny-list, are rejected with an error.

> **See also:** [Git Worktrees](../concepts/worktrees.md) — the worktree write
> guard that protects against silent edits to the wrong repository tree.

---

## When to use which editing tool

| Situation | Recommended tool |
|-----------|-----------------|
| Rewrite a function or method body | `edit_code(action="replace")` |
| Add a new function next to an existing one | `edit_code(action="insert")` |
| Delete a function, struct, or method | `edit_code(action="remove")` |
| Rename a symbol everywhere it is used | `edit_code(action="rename")` |
| Change a string, constant, or small code fragment | `edit_file` |
| Edit a config file, Markdown, or other non-code file | `edit_file` |
| Create a new file | `create_file` |

For source code, prefer `edit_code`. It addresses code by name rather than by
position, which means your edit remains correct even if the file was modified
since you last read it. Fall back to `edit_file` when you need to change
something small that is not naturally symbol-scoped.

---
## `create_file`

**Purpose:** Create a new file, or completely overwrite an existing one, with the supplied content. Parent directories are created automatically.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `path` | string | yes | — | Destination path, relative to project root |
| `content` | string | yes | — | Full content to write |

**Example:**

```json
{
  "path": "src/util/helpers.rs",
  "content": "pub fn clamp(v: i32, lo: i32, hi: i32) -> i32 {\n    v.max(lo).min(hi)\n}\n"
}
```

**Output:**

```json
{
  "status": "ok",
  "path": "/home/user/project/src/util/helpers.rs",
  "bytes": 58
}
```

**Tips:**

- Use this tool when you are generating a file from scratch. If the file already exists and you only need to change part of it, use `edit_file` or a symbol tool instead — those tools are less likely to accidentally discard content you did not intend to touch.
- The path is resolved relative to the active project root, so `src/util/helpers.rs` and `./src/util/helpers.rs` both work. Absolute paths that fall inside the project root are also accepted.

---

## `edit_file`

**Purpose:** Find-and-replace editing. Locates `old_string` in the file and replaces it with `new_string`. Works on any file type. Alternatively, prepend or append text without a match string.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `path` | string | yes | — | File to edit, relative to project root |
| `old_string` | string | yes* | — | Exact text to find, including whitespace and indentation. *Not required when using `insert`. |
| `new_string` | string | yes | — | Replacement text. Set to `""` to delete the match. |
| `replace_all` | boolean | no | `false` | Replace every occurrence instead of requiring a unique match. |
| `insert` | string | no | — | `"prepend"` or `"append"` — add text at the start or end of the file without a match string. |

**Example — change a constant value:**

```json
{
  "path": "src/config.rs",
  "old_string": "    pub const MAX_RETRIES: u32 = 3;",
  "new_string": "    pub const MAX_RETRIES: u32 = 5;"
}
```

**Example — add an import at the top of a file:**

```json
{
  "path": "src/lib.rs",
  "insert": "prepend",
  "new_string": "use std::collections::HashMap;\n"
}
```

**Example — delete a block of lines:**

```json
{
  "path": "src/config.rs",
  "old_string": "    // TODO: remove this\n    legacy_init();\n",
  "new_string": ""
}
```

**Example — replace all occurrences of a deprecated API:**

```json
{
  "path": "src/util.rs",
  "old_string": "old_function()",
  "new_string": "new_function()",
  "replace_all": true
}
```

**Output:**

```json
"ok"
```

If `old_string` is not found, or appears multiple times without `replace_all: true`, the tool returns a recoverable error with the line numbers of all matches.

**Multi-line edits on source files:** When `old_string` spans multiple lines and contains a
definition keyword (`fn`, `class`, `def`, etc.) in an LSP-supported language, the tool
**blocks the edit** and returns a `RecoverableError` suggesting the correct symbol tool.
There is no bypass — use `edit_code` (with the appropriate action) instead.
See [Structural Edit Gate](edit-file-structural-gate.md) for the full keyword table and
gate logic.

**Tips:**

- `old_string` must match exactly — include any leading whitespace and indentation.
- Use `grep` first to verify the exact text if you are unsure what to match.
- If you get a multiple-matches error, expand `old_string` to include enough surrounding context to make it unique, or use `replace_all: true`.
- Use `insert: "prepend"` or `insert: "append"` to add content at the start or end of a file when there is no anchor string to match.
- For adding a completely new top-level definition adjacent to an existing one, `edit_code(action="insert")` is more convenient — it addresses the location by symbol name rather than requiring an exact text match.

---

## Symbol editing — `edit_code`

The standalone `replace_symbol`, `insert_code`, `rename_symbol`, and
`remove_symbol` tools were consolidated in v0.11 into the single
action-dispatched **`edit_code`** tool. It addresses code by symbol name
via the LSP and works with any language that has an LSP server configured.
See [Symbol Navigation](./symbol-navigation.md) for background on how
symbols are identified.

For full parameter reference, examples for each action, and the
`edit_code` vs `edit_file` decision table, see the canonical page:

> **[`edit_code` documentation →](edit-code.md)**

Quick mapping of the four actions:

| Action | Purpose |
|---|---|
| `replace` | Overwrite the body of a named symbol |
| `insert` | Inject code before or after a named symbol (`position: "before"\|"after"`) |
| `remove` | Delete a named symbol and its full body |
| `rename` | Rename a symbol across the codebase via LSP `workspace/rename` |

All four require `symbol` (e.g. `"MyStruct/my_method"`) and `path` (file
relative to project root).
