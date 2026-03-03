# find_symbol: Per-symbol document_symbols fallback for body extraction

**Date:** 2026-03-03
**Status:** Approved

## Problem

`find_symbol(pattern, include_body=true)` without a `path=` parameter uses the
`workspace/symbol` LSP query (fast path: one call per language). Some LSP
servers (notably rust-analyzer) return "selection ranges" (start==end, covering
only the function name) instead of full body ranges for certain symbols
(especially private/unexported functions). The current `validate_symbol_range`
correctly detects this and returns a `RecoverableError`, but this means
`find_symbol(include_body=true)` intermittently fails depending on LSP state.

The same query with `path=` always works because it uses
`textDocument/documentSymbol`, which returns full body ranges.

## Root Cause

`workspace/symbol` is designed for navigation (returns name positions), not
structure (full body ranges). Using its ranges for body extraction is
architecturally mismatched. krait solves this by always using
`textDocument/documentSymbol` for anything needing precise ranges.

## Design

### Approach: Per-symbol document_symbols fallback

When `include_body=true` and `workspace/symbol` gives a symbol with a suspicious
range (detected by `validate_symbol_range`), instead of returning a
`RecoverableError`:

1. Call `document_symbols` for that specific file
2. Match the symbol by name + start_line proximity
3. Use the document_symbols range for body extraction
4. If document_symbols also fails, *then* return the original RecoverableError

### New function: `resolve_range_via_document_symbols`

```rust
async fn resolve_range_via_document_symbols(
    sym: &SymbolInfo,
    ctx: &ToolContext,
) -> Option<SymbolInfo>
```

- Detects language from `sym.file` via `ast::detect_language`
- Calls `ctx.lsp.get_or_start(lang, root)` then `client.document_symbols(file, lang_id)`
- Walks the returned symbol tree recursively looking for a symbol matching
  `sym.name` within ±1 line of `sym.start_line`
- Returns `Some(corrected_sym)` or `None`

### Changes to FindSymbol::call()

In the workspace/symbol loop (around line 742-760), replace:

```rust
if include_body {
    validate_symbol_range(&sym)?;
}
```

With:

```rust
let sym = if include_body {
    match validate_symbol_range(&sym) {
        Ok(()) => sym,
        Err(_) => {
            match resolve_range_via_document_symbols(&sym, ctx).await {
                Some(resolved) => resolved,
                None => {
                    validate_symbol_range(&sym)?;
                    unreachable!()
                }
            }
        }
    }
} else {
    sym
};
```

### What stays the same

- `validate_symbol_range` itself — unchanged, still returns RecoverableError
- Write tools (`replace_symbol`, `insert_code`, `remove_symbol`) — keep hard
  error, they already use document_symbols
- `path=` code path in `find_symbol` — untouched, already uses document_symbols
- `find_symbol` without `include_body` — untouched, no body to extract

### Testing

- Existing `validate_symbol_range_*` unit tests remain unchanged
- New unit test: mock LSP returning degenerate workspace/symbol range but correct
  document_symbols range → verify find_symbol recovers
- Integration test: `find_symbol(pattern, include_body=true)` for a private
  function in a real Rust project → should succeed where it currently fails

### Alternatives considered

- **Per-file batch fallback**: Fetch document_symbols for entire file when any
  symbol has bad range. More complex, YAGNI unless many symbols per file fail.
- **Session-level document_symbols cache**: Cache document_symbols results.
  Adds state management complexity, premature optimization.
- **Always use document_symbols for include_body**: Skip workspace/symbol
  entirely. Most correct but slower for multi-language projects.
- **Split into two tools (find_symbol + find_symbol_body)**: Cleaner semantics
  but adds round-trip for the common "locate + read" case. Decided against to
  preserve backward compatibility and single-call ergonomics.
