> ⚠ Experimental — may change without notice.

# Hard Gate for Structural Edits in `edit_file`

`edit_file` now **refuses** multi-line edits that contain definition keywords on
LSP-supported languages. Previously this was a soft warning that could be bypassed with
`acknowledge_risk: true`; the bypass has been removed.

## What triggers the gate

All four conditions must be true for the block to fire:

1. The edit spans **multiple lines** (single-line edits always pass through)
2. The target file is a **source file** (not markdown, TOML, JSON, etc.)
3. The language has **LSP support** in codescout (Rust, Python, Go, TypeScript/JS, Java, Kotlin, C/C++, C#, Ruby)
4. The `old_string` contains a **language-specific definition keyword**

The keywords are per-language — `fn` only blocks Rust edits, `def` only Python, `func` only
Go, and so on. This prevents false positives from comments and string literals that happen to
contain a keyword from another language.

### Language keyword table

| Language | Blocked keywords |
|---|---|
| Rust | `fn`, `async fn`, `struct`, `impl`, `trait`, `enum` |
| Python | `def`, `async def`, `class` |
| Go | `func`, `struct`, `interface` |
| TypeScript / JS | `function`, `async function`, `class`, `interface`, `enum` |
| Java | `class`, `interface`, `enum` |
| Kotlin | `fun`, `class`, `interface`, `enum` |
| C / C++ | `struct`, `class`, `enum` |
| C# | `class`, `struct`, `interface`, `enum` |
| Ruby | `def`, `class` |

Non-LSP languages (Lua, Bash, PHP, etc.) and **all single-line edits** pass through freely
regardless of content.

## What the error looks like

When the gate fires, `edit_file` returns a `RecoverableError` (not a fatal tool error) with
a message like:

```
edit_file blocked: old_string contains a Rust definition keyword ("fn ").
Structural edits must use symbol tools — the LSP knows the exact range.
Use: replace_symbol(name_path, path, new_body) — replaces the symbol body via LSP
```

The hint is inferred from the edit shape:

| Edit shape | Suggested tool |
|---|---|
| `new_string` is empty | `remove_symbol` |
| `new_string` is longer than `old_string` | `insert_code` |
| Replacing a function/struct body | `replace_symbol` |

## Why no bypass?

The previous `pending_ack + acknowledge_risk` mechanism was removed entirely. The rationale:
`edit_file` on a function body is always wrong for LSP-supported languages — the symbol tools
are LSP-range-aware and will never corrupt the file with off-by-one line numbers or
whitespace drift. There is no valid use case for bypassing this gate; if you find one, it
points to a missing symbol tool, not a reason to use string matching on source.
