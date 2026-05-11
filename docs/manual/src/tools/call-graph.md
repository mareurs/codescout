# `call_graph`

Transitive call graph for a symbol. Two directions:

- `callers` (default): who calls this symbol, transitively. Use for blast-radius
  before refactoring.
- `callees`: what does this symbol call, transitively. Use to trace flow.
- `both`: both, returned in separate keys.

## Schema

```json
{ "symbol": "Agent::new", "direction": "callers", "max_depth": 3 }
```

## Output

By default returns a compact summary: counts + `by_file` + `by_depth`. When the
total result has ≤ 30 edges, auto-promotes to full edge list. Use
`detail_level: "full"` to force full output on large graphs.

Each edge is tagged `source: "lsp"` (from `callHierarchy`, semantically
authoritative) or `"ts"` (from the tree-sitter classifier fallback —
best-effort, may include false positives on macros, shadowed names, or dynamic
dispatch).

## Caching

Edges are cached in the project sqlite DB (`call_edges` table). Caches are
invalidated per file on `did_change` notifications (triggered by write tools),
so the cache stays correct across multi-session edits.

## Known limitations

- Kotlin LSP coverage of `callHierarchy` is partial; expect `source: "ts"` edges
  for most Kotlin queries.
- Cross-project edges are not supported in v1.
- `direction="callees"` via tree-sitter fallback is not supported; a
  `RecoverableError` is returned for languages where LSP `callHierarchy` is
  unavailable. See `docs/issues/bug-tracker.md` (#14).
- Arrow function names in TypeScript/JavaScript show as `<anonymous>` in
  tree-sitter fallback mode.
