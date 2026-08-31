# AST Analysis

> **Removed 2026-09-01.** The `list_functions` and `list_docs` tools no longer exist —
> `src/tools/ast.rs` was deleted along with them. Use `symbols`. This page is kept as a
> redirect because older docs, plans and session logs link to it.

They had been documented here as *"still registered for backward compatibility"*, and that
sentence was wrong in a way worth recording: they were never registered at all. Both
implemented the `Tool` trait and neither was ever added to the registry in `src/server.rs`,
so no agent could reach either one — while 13 tests exercised them by name and passed. See
[`docs/issues/archive/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md`](https://github.com/mareurs/codescout/blob/master/docs/issues/archive/2026-09-01-listfunctions-and-listdocs-are-unregistered-tools.md)
and CLAUDE.md § *Testing Discipline*, where this is the worked example of a green suite
guarding code no observer can reach.

## What to Use Instead

| Removed tool | Replacement | Notes |
|----------|-------------|-------|
| `list_functions` | `symbols` | Returns symbol tree with line ranges; requires LSP server |
| `list_docs` | `symbols(include_docs=true)` | Returns a `docstrings` array keyed by symbol name |

`symbols` covers all 9 LSP-supported languages (not just the 4 with tree-sitter grammars) and returns richer output including types, nesting, and references. For languages where the LSP server hasn't started yet, `grep` can locate doc comment blocks (`///`, `/**`) using a regex.

## Why They Were Removed

The offline advantage (no LSP startup) was outweighed by the maintenance cost of a parallel navigation path — and in the end nothing was paying that cost's benefit, because neither tool was reachable. `symbols` starts the language server on the first call and keeps it running — subsequent calls are instant. The first call on a cold server is budgeted (2s): if the LSP isn't ready in time, `symbols` overview falls back to tree-sitter output and the response carries `"lsp": "warming"` plus a hint field. The compact text surface renders this too — file mode appends a trailing `[lsp warming] <hint>` line, and pattern/glob mode appends a ` (lsp warming)` suffix to the affected file's summary line. Re-run shortly for LSP-grade detail once the server finishes starting.

The tree-sitter layer itself (`crate::ast`) is untouched and remains load-bearing — `symbols`, `grep`, `read_file`, the embedding chunker and the doc-ref auditor all use it. Only the two *tool* wrappers went.

See [Symbol Navigation](symbol-navigation.md) for the full `symbols` reference.
