# AST-Aware Semantic Chunking

**Date:** 2026-02-25
**Status:** Approved

## Problem

The semantic search chunker (`src/embed/chunker.rs`) splits files by character
limits on line boundaries. This produces chunks that cut mid-function, separate
doc comments from their symbols, and create embeddings of incomplete code. The
AST infrastructure (`src/ast/parser.rs`) already has tree-sitter support for 7
languages but is disconnected from the embedding pipeline.

## Design

### Architecture

New module `src/embed/ast_chunker.rs` dispatches to the right splitting strategy
and delegates sub-splitting to the existing `chunker.rs`:

```
ast_chunker::split_file(source, lang, path, chunk_size, chunk_overlap)
  ├─ markdown           → chunker::split_markdown()
  ├─ registry language  → AST extraction with registered node types
  ├─ tree-sitter lang   → AST extraction with generic heuristic
  └─ else               → chunker::split() (line-based fallback)
```

**Files changed:**
- `src/embed/ast_chunker.rs` — new, all AST chunking logic
- `src/embed/mod.rs` — add `pub mod ast_chunker;`
- `src/embed/index.rs` — change `build_index()` call site (one line)
- `src/embed/chunker.rs` — untouched

### Language Registry

Static data structure mapping language to splittable node types and doc comment
patterns:

```rust
struct LanguageSpec {
    node_types: &'static [&'static str],
    doc_prefixes: &'static [&'static str],
}
```

Registered languages (7): Rust, Python, Go, TypeScript/JavaScript/TSX/JSX,
Java, Kotlin. Adding a language = adding one entry to the array.

### AST Node Extraction

1. Parse source with tree-sitter
2. Walk root children (recurse into module/namespace containers)
3. Collect nodes matching registered `node_types` (or generic heuristic)
4. For each node, expand doc comments upward by scanning backward for
   `doc_prefixes` lines
5. Collect gap text between nodes (imports, module-level constants) as separate
   chunks
6. Emit each node as a `RawChunk`; sub-split if oversized

### Generic Heuristic (Unregistered Languages)

For languages with a tree-sitter grammar but no registry entry, accept any
direct child of root that:
- Is a named node (not anonymous syntax)
- Spans 3+ lines
- Has at least one named child

### Doc Comment Expansion

Scan backward from a node's start line through source lines. While
`line.trim_start()` starts with any `doc_prefix`, include the line. Stop at
first non-doc, non-blank line. The raw source lines are preserved (no
stripping).

### Sub-splitting Oversized Nodes

When a node exceeds `chunk_size`:

1. Extract prefix = doc comment lines + signature (through first `{`/`:`/`=>`,
   or first 3 lines)
2. Sub-split the body via `chunker::split(body, chunk_size - prefix.len(),
   chunk_overlap)`
3. Prepend prefix + `// ... (continued)` marker to each sub-chunk
4. Adjust line numbers to file-level coordinates

### Overlap Rules

- Between separate AST nodes: **no overlap** (independent semantic units)
- Between sub-chunks of an oversized node: **overlap** (chunk_overlap param)
- In line-based fallback: **overlap** (same as current behavior)

### Error Handling

If tree-sitter parsing fails, log a warning and fall back to
`chunker::split()`. Never panic, never skip the file.

### Integration Point

Single change in `build_index()`:

```rust
// Before:
let chunks = if lang == "markdown" {
    chunker::split_markdown(&source, ...)
} else {
    chunker::split(&source, ...)
};

// After:
let chunks = ast_chunker::split_file(&source, lang, path, chunk_size, chunk_overlap);
```

### Public API

```rust
pub fn split_file(
    source: &str,
    lang: &str,
    path: &Path,
    chunk_size: usize,
    chunk_overlap: usize,
) -> Vec<RawChunk>
```

## Testing

1. Per-language AST splitting — each registered language with 2-3 functions +
   docs, verify one chunk per function with docs attached
2. Doc comment expansion — verify backward scanning for each comment style
3. Sub-splitting — verify prefix carried forward, overlap between sub-chunks,
   full body coverage
4. Generic heuristic — unregistered language still extracts multi-line nodes
5. Fallback — language without tree-sitter produces same output as
   `chunker::split()`
6. Error resilience — broken syntax falls back to line-based splitting

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Oversized functions | Sub-split with doc+sig prefix | Each sub-chunk understandable in isolation |
| Extensibility | Registry + generic heuristic | Precise for known langs, automatic for new ones |
| Doc attachment | Expand node span upward | One parse pass, raw source preserved |
| Overlap | Only within sub-splits and fallback | AST nodes are independent units |
| Architecture | New module, one call-site change | YAGNI, existing code untouched |
