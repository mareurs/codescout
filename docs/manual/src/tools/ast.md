# AST Analysis

> **Note:** The `list_functions` and `list_docs` tools are still registered
> for backward compatibility, but new code should use `symbols` (and
> `symbols(include_body=true)` to inspect doc comments). The tree-sitter
> layer powers richer symbol extraction internally for languages with grammar
> support — it is no longer the recommended public entrypoint.

## What to Use Instead

| Old tool | Replacement | Notes |
|----------|-------------|-------|
| `list_functions` | `symbols` | Returns symbol tree with line ranges; requires LSP server |
| `list_docs` | `symbols` + `symbols(include_body=true)` | Read the symbol body to inspect doc comments |

`symbols` covers all 9 LSP-supported languages (not just the 4 with tree-sitter grammars) and returns richer output including types, nesting, and references. For languages where the LSP server hasn't started yet, `grep` can locate doc comment blocks (`///`, `/**`) using a regex.

## Why They Were Removed

The offline advantage (no LSP startup) was outweighed by the maintenance cost of a parallel navigation path. `symbols` starts the language server on the first call and keeps it running — subsequent calls are instant. For the initial cold start, the latency difference is negligible for interactive use.

See [Symbol Navigation](symbol-navigation.md) for the full `symbols` reference.

## `list_functions`

Backward-compatible alias retained for tree-sitter-only callsites that need
function extraction without starting an LSP server. New code should use
`symbols` — it covers more languages and returns richer output.

## `list_docs`

Backward-compatible alias retained for tree-sitter doc-comment extraction
without an LSP. New code should use `symbols(include_body=true)` and inspect
the body for `///` / `/**` blocks.
