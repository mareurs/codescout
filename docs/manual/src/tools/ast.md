# AST Analysis

> **Note:** The `list_functions` and `list_docs` tools were removed in the v1
> tool restructure. The tree-sitter layer still exists internally and powers
> richer symbol extraction for languages with grammar support (Rust, Python,
> TypeScript, Go) — but it is no longer exposed as a standalone MCP tool.

## What to Use Instead

| Old tool | Replacement | Notes |
|----------|-------------|-------|
| `list_functions` | `list_symbols` | Returns symbol tree with line ranges; requires LSP server |
| `list_docs` | `list_symbols` + `find_symbol(include_body=true)` | Read the symbol body to inspect doc comments |

`list_symbols` covers all 9 LSP-supported languages (not just the 4 with tree-sitter grammars) and returns richer output including types, nesting, and references. For languages where the LSP server hasn't started yet, `search_pattern` can locate doc comment blocks (`///`, `/**`) using a regex.

## Why They Were Removed

The offline advantage (no LSP startup) was outweighed by the maintenance cost of a parallel navigation path. `list_symbols` starts the language server on the first call and keeps it running — subsequent calls are instant. For the initial cold start, the latency difference is negligible for interactive use.

See [Symbol Navigation](symbol-navigation.md) for the full `list_symbols` reference.
