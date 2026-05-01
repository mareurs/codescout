# Call Graph (A) — Pre-Brainstorm Context

**Purpose:** Resume brainstorm for item A from `docs/socraticode-borrow-tracker.md` in a
fresh session. All decisions made so far are captured here so the session starts informed.

**Status:** L3 ✅ complete (v0.10.0). `call_graph` stub registered, schema frozen, returns
`RecoverableError("not yet implemented")`. Ready to design the implementation.

---

## What We've Already Decided

### Tool surface (frozen by L3)

```
call_graph(symbol: string, direction: "callers" | "callees" | "both", max_depth?: integer)
```

- Registered in `src/tools/symbol/call_graph.rs` → `src/tools/symbol/mod.rs` → `src/server.rs`
- Returns `RecoverableError` until implemented
- Schema is final — no bikeshedding needed

### Architecture choices (from original brainstorm)

| Axis | Decision | Rationale |
|------|----------|-----------|
| **Backend** | LSP-first (A4), tree-sitter fallback (A3) | LSP gives semantic precision; tree-sitter covers languages without LSP |
| **Cache** | sqlite (same `.codescout/` DB) | Already a dependency; avoids new infra |
| **Direction** | Bidirectional — callers + callees both | C direction chosen: blast radius AND flow tracing |
| **Depth** | `max_depth` param (default 3) | Bounded traversal prevents explosion on large graphs |

### What `call_graph` is NOT (deferred)

- **`references` kind filter** (`call|noncall`) — deferred to after A lands. A's call-site
  classifier is a prereq; then `references(symbol, kind="call")` becomes cheap.
- **`flow(from, to)` path search** — BFS path between two symbols (socraticode's
  `codebase_flow`). Different shape from traversal. Defer until A proves out.
- **Graph visualization** (item E) — Cytoscape.js / Mermaid output. Pairs with A but
  separate item.
- **Cycle detection** (item G) — cheap once A exists; separate item.

---

## What Still Needs Designing (brainstorm agenda)

### 1. LSP call-site extraction

LSP gives `textDocument/references` (all refs, undifferentiated) and
`callHierarchy/incomingCalls` / `callHierarchy/outgoingCalls` (call-specific, if server
supports it). Key questions:

- Which LSP methods does Rust Analyzer support for call hierarchy? Does rust-analyzer
  implement `callHierarchy/prepare` + `callHierarchy/incomingCalls`?
- Fallback when `callHierarchy` not supported: filter `references` results by checking
  whether the ref site is inside a call expression (tree-sitter classifier)?
- How to handle multi-language workspaces (each language has its own LSP client)?

### 2. Tree-sitter classifier for call sites

When LSP call hierarchy is unavailable, need to distinguish call refs from type refs /
import refs. Tree-sitter can parse the ref's surrounding AST node. Key questions:

- Which tree-sitter node types indicate a call in: Rust, Python, TypeScript, Kotlin, Java?
- How expensive is on-the-fly tree-sitter parsing per reference? Cache the AST?
- Same classifier needed for `references(kind=call|noncall)` — design it reusably.

### 3. Cache schema

Store the computed graph in sqlite so repeated queries are fast. Key questions:

- Schema: `(project_id, symbol_name, direction, max_depth, computed_at, graph_json)`?
  Or normalized edges table `(caller, callee, file, line)` + query-time traversal?
- Invalidation: invalidate on file change (integrate with index pipeline's changed-files
  tracking?), or invalidate by commit hash?
- Cross-project edges (socraticode had workspace-spanning graphs) — in scope for A?

### 4. Recursion / traversal engine

`max_depth` controls traversal. Key questions:

- BFS or DFS? BFS gives level-by-level output (better for "what's at distance 1 vs 3").
- Cycle handling: visited-set per traversal run.
- Output shape: flat list with `depth` field, or nested tree? Flat is easier for LLMs
  to summarize; nested is better for visualization (item E).
- Cap: what happens when the graph exceeds N nodes? Return partial + hint?

### 5. Output format

The LLM-facing result. Key questions:

- Group by depth? By file? By module?
- `format_compact` for quick blast-radius summary ("12 callers in 4 files")
- `detail_level: "full"` for complete edge list with file+line
- Integration with `OutputGuard` / progressive disclosure pattern

### 6. `call_graph` vs `references` overlap

`references` already gives one-hop call sites (if kind-filter lands). `call_graph` adds
transitivity. Ensure the two tools compose cleanly — `call_graph(depth=1)` should
produce a superset compatible with `references` output shape.

---

## Relevant Code Locations

| What | Where |
|------|-------|
| `call_graph` stub | `src/tools/symbol/call_graph.rs` |
| `references` impl (for comparison) | `src/tools/symbol/references.rs` |
| LSP client ops | `src/lsp/ops.rs`, `src/lsp/client.rs` |
| LSP provider trait | `src/lsp/mod.rs` |
| sqlite embed DB | `src/embed/index.rs`, `.codescout/embeddings/project.db` |
| Progressive disclosure pattern | `src/tools/output.rs` (OutputGuard) |
| socraticode call graph (reference impl) | `../socraticode/src/tools/` |

---

## Socraticode Reference

socraticode has a working call graph implementation. Before designing, review:

```
symbols("../socraticode/src/tools/", name="call_graph")      # or codebase_flow
semantic_search("call graph callers callees traversal", project_id="socraticode")
```

Key differences to be aware of:
- socraticode may not use LSP call hierarchy — may be pure tree-sitter or grep-based
- codescout has a more complete LSP abstraction (`LspClientOps` trait + mock)
- Cache strategy may differ (socraticode may not cache)

---

## How to Start the Brainstorm Session

1. Invoke `superpowers:brainstorming` skill
2. Tell it: "Continuing brainstorm for item A (call graph + blast radius) from
   `docs/superpowers/specs/2026-05-01-call-graph-brainstorm-context.md`. Decisions in
   that file are frozen — focus on the 6 open questions in 'What Still Needs Designing'."
3. Start with Q1 (LSP call hierarchy support) — it gates everything else.
