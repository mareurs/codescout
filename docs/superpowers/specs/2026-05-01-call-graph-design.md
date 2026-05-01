# Call Graph (item A) — Design

**Status:** Design (post-brainstorm). Implementation plan to follow via `superpowers:writing-plans`.

**Inputs:**
- Pre-brainstorm context: `docs/superpowers/specs/2026-05-01-call-graph-brainstorm-context.md`
- Frozen tool surface (L3, v0.10.0): `call_graph(symbol, direction, max_depth?)` stub at `src/tools/symbol/call_graph.rs`

**Audience:** the LLM is the primary consumer of this tool. Every design choice below is justified from that consumer's standpoint — what minimizes round-trips, ambiguity, and token waste for an agent doing impact analysis or flow tracing.

---

## 1. Goals

- Answer "who calls this?" and "what does this call?" transitively, up to a bounded depth.
- Sub-second response on cached graphs; degrade gracefully on cold cache.
- One tool, two compose-friendly modes (compact summary; full edge list).
- Composable with future `references(kind="call")` — same underlying edge set.
- Multi-language: rust-analyzer / pyright / ts-server / jdtls / kotlin-lsp.

## 2. Non-Goals (deferred)

- `flow(from, to)` BFS path search between two symbols (separate tool, future).
- Graph visualization output (Cytoscape / Mermaid) — item E, post-A.
- Cycle detection beyond traversal-time visited-set — item G.
- Cross-project call edges. v1 is single-project; project_id scopes every row.
- `references(kind="call|noncall")` filter on the existing tool — depends on this design's classifier; will land after.

## 3. Architecture

```
┌──────────────────────┐
│ call_graph (tool)    │  src/tools/symbol/call_graph.rs
│  - param parse       │
│  - traversal driver  │
│  - format compact/full│
└────────┬─────────────┘
         │
         ▼
┌──────────────────────┐         ┌──────────────────────┐
│ traversal::bfs       │ ◄────── │ edge_cache (sqlite)  │
│  - visited-set       │         │  call_edges table    │
│  - depth-coherent cap│         │  (project DB)        │
└────────┬─────────────┘         └──────────────────────┘
         │ cache miss
         ▼
┌──────────────────────┐
│ call_edges::resolve  │  shared resolver — also feeds
│  one_hop(sym, dir)   │  future references(kind=call)
└────────┬─────────────┘
         │
    ┌────┴─────┐
    ▼          ▼
┌─────────┐ ┌──────────────┐
│ LSP     │ │ tree-sitter  │
│ call    │ │ classifier   │
│ hierarchy│ │ (fallback)   │
└─────────┘ └──────────────┘
```

### 3.1 Components

| Component | Path | Purpose |
|---|---|---|
| `call_graph` tool | `src/tools/symbol/call_graph.rs` | Public surface; param validation; calls traversal; formats output |
| Traversal engine | `src/tools/symbol/call_graph/traversal.rs` (new) | BFS with visited-set, depth-coherent cap |
| Edge resolver | `src/tools/symbol/call_edges.rs` (new) | One-hop edge fetch; LSP-first, ts-fallback. Shared with future `references(kind=call)` |
| Edge cache | `src/tools/symbol/call_edges/cache.rs` (new) | sqlite-backed read/write/invalidate over `call_edges` table |
| Tree-sitter classifier | `src/tools/symbol/call_edges/ts_classifier.rs` (new) | Determines whether a ref site is a call expression |
| LSP additions | `src/lsp/ops.rs`, `src/lsp/client.rs` | Add `prepare_call_hierarchy`, `incoming_calls`, `outgoing_calls` to `LspClientOps` |

### 3.2 Edge resolver — LSP first, tree-sitter fallback

For a symbol `S` and direction `d ∈ {callers, callees}`:

1. **LSP path.** Call `LspClientOps::prepare_call_hierarchy(S)` → returns `CallHierarchyItem`. Then `incoming_calls(item)` for callers or `outgoing_calls(item)` for callees. Tag edges `source: "lsp"`. If the LSP returns "method not supported," fall through to step 2.
2. **Tree-sitter fallback.** Call `LspClientOps::references(S)` → returns all ref sites. For each, run the classifier on the surrounding AST node. Keep only refs whose ancestor is a call-expression node. Tag edges `source: "ts"`.

The `source` field is exposed in the response. Downstream consumers (the LLM) treat `ts` edges as best-effort — possible false positives from shadowed names, dynamic dispatch, or macro hygiene. `lsp` edges are authoritative.

### 3.3 Tree-sitter classifier — hardcoded node-type map

| Language | Call-expression node types |
|---|---|
| Rust | `call_expression`, `method_call_expression`, `macro_invocation` |
| Python | `call` |
| TypeScript / JavaScript | `call_expression`, `new_expression` |
| Kotlin | `call_expression` |
| Java | `method_invocation`, `object_creation_expression` |

Implementation: walk up from the ref's byte range; first ancestor matching one of the language's call-types ⇒ call site. New languages = one PR adding a match arm.

### 3.4 Cache schema

Single new table in the existing project sqlite DB:

```sql
CREATE TABLE call_edges (
  project_id   TEXT NOT NULL,
  caller_sym   TEXT NOT NULL,   -- fully-qualified symbol name
  callee_sym   TEXT NOT NULL,
  file         TEXT NOT NULL,   -- ref site path (relative to project root)
  line         INTEGER NOT NULL,
  col          INTEGER NOT NULL,
  source       TEXT NOT NULL,   -- 'lsp' | 'ts'
  computed_at  INTEGER NOT NULL,-- unix epoch
  PRIMARY KEY (project_id, caller_sym, callee_sym, file, line, col)
);

CREATE INDEX call_edges_caller   ON call_edges(project_id, caller_sym);
CREATE INDEX call_edges_callee   ON call_edges(project_id, callee_sym);
CREATE INDEX call_edges_file     ON call_edges(project_id, file);  -- invalidation
```

**Invalidation.** Hooked into the existing `notify_file_changed` / `did_change` pipeline:

```rust
fn on_file_changed(file: &Path) {
    DELETE FROM call_edges WHERE project_id = ? AND file = ?;
}
```

Invalidate by ref-site file (the `file` column), not by definition file. A function definition moving doesn't invalidate edges that point INTO it; only edits to the file containing the call sites do. Cross-session benefit: session B picks up session A's edits via the shared DB after `did_change` lands.

**Partial fills.** The traversal driver requests one hop at a time. A miss for one symbol triggers a single resolver call; the result is upserted; traversal continues. No "compute the whole graph" precomputation step.

### 3.5 Traversal engine

- **BFS.** Process symbols level-by-level. Each pop of the queue produces depth-N edges; only after the level is drained do we expand to depth N+1.
- **Visited-set.** Keyed by fully-qualified symbol name. Self-recursive edge marked `recursive: true` and not re-expanded.
- **Cap behavior.** Configurable thresholds (compact=200 edges, full=500). When the next-level expansion would exceed the cap, finish the current depth and stop. Set `truncated: true`, `truncated_at_depth: N`. **Depth-coherent halt** — the consumer always sees a complete level, never a partial fan-out.
- **Dedupe.** `(caller, callee)` pair appears once per direction with a `paths` count if reachable through multiple chains.

### 3.6 Output format

Two modes, gated by `detail_level`. Auto-promote to full when total edges ≤ 30 (single round-trip for small graphs).

**Compact (default for large graphs):**

```json
{
  "symbol": "src/agent.rs::Agent::new",
  "callers": {
    "count": 12,
    "by_file": { "src/server.rs": 7, "src/tools/mod.rs": 3, "tests/...": 2 },
    "by_depth": { "1": 7, "2": 5 }
  },
  "callees": {
    "count": 3,
    "by_file": { "src/lsp/ops.rs": 2, "src/embed/index.rs": 1 },
    "by_depth": { "1": 3 }
  },
  "truncated": false,
  "max_depth_reached": 2
}
```

**Full (or auto-promoted):**

```json
{
  "symbol": "...",
  "callers": [
    { "caller": "...", "callee": "...", "file": "...", "line": 42, "depth": 1, "source": "lsp", "paths": 1 },
    ...
  ],
  "callees": [...],
  "truncated": false,
  "auto_promoted": true   // present only when small-result auto-promote fired
}
```

When `direction="callers"` or `direction="callees"`, the unused side is omitted.

`OutputGuard` integration: standard exploring-mode pattern with caps above. `offset`/`limit` for pagination on the full edge list.

### 3.7 LSP surface additions

Add to `LspClientOps`:

```rust
fn prepare_call_hierarchy(
    &self, path: &Path, line: u32, col: u32, language_id: &str,
) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>>;

fn incoming_calls(
    &self, item: &lsp_types::CallHierarchyItem, language_id: &str,
) -> anyhow::Result<Vec<lsp_types::CallHierarchyIncomingCall>>;

fn outgoing_calls(
    &self, item: &lsp_types::CallHierarchyItem, language_id: &str,
) -> anyhow::Result<Vec<lsp_types::CallHierarchyOutgoingCall>>;
```

Each returns `Ok(None)` / `Ok(vec![])` if the server reports the method unsupported (capabilities check at `initialize`); the resolver then falls back to tree-sitter. `MockLspClient` gets matching mock impls.

## 4. Data Flow — Example

User calls `call_graph(symbol="Agent::new", direction="callers", max_depth=3)`.

1. Tool resolves `Agent::new` to a (file, line, col) using existing symbol index.
2. Traversal seeds queue with `(Agent::new, depth=0)`.
3. Pops `Agent::new`, calls `call_edges::resolve_one_hop(Agent::new, callers)`:
   - Cache check: `SELECT * FROM call_edges WHERE callee_sym = "Agent::new"`.
   - Hit → return cached rows.
   - Miss → LSP `prepare + incoming_calls` → upsert rows → return them.
4. Each returned caller becomes a depth-1 node, queued.
5. Repeat for depth 2, 3. Visited-set prunes cycles.
6. If edge count crosses cap during depth 3 expansion, finish depth 2 fully, abort depth 3, set `truncated`.
7. Format: count, by_file, by_depth (compact); or full edge list with auto_promote check.

## 5. Error Handling

- **Symbol not found.** `RecoverableError` with hint to use `symbols(name=...)` to discover candidates.
- **LSP unavailable for language + no tree-sitter parser.** `RecoverableError("call_graph not supported for language <lang>")`.
- **LSP timeout / circuit breaker open.** Same as other LSP-backed tools — the existing LSP layer surfaces this; we propagate.
- **Cache write failure.** Log + continue (graph is computed; persistence is best-effort).
- **Genuine bugs (panics, schema corruption, internal invariant violations).** `anyhow::bail!` → fatal.

## 6. Testing Strategy

Per the project's `Testing Patterns` (CLAUDE.md):

1. **Unit: classifier.** Fixture files per language; assert each call-type and non-call ref classifies correctly.
2. **Unit: traversal.** Mock edge resolver; verify BFS order, visited-set, depth-coherent cap, dedupe.
3. **Integration: edge resolver with `MockLspClient`.** Mock returns known callHierarchy responses; assert correct rows persisted to cache.
4. **Integration: tree-sitter fallback.** `MockLspClient` reports method-unsupported; assert classifier path runs and produces same logical edges.
5. **Cache invalidation: three-query sandwich.**
   - Query graph for symbol X → record baseline.
   - Mutate file F (change a caller) on disk WITHOUT going through `did_change`.
   - Re-query → assert result is **stale** (proves bug exists).
   - Trigger `did_change` for F.
   - Re-query → assert result is **fresh** (callers reflect mutation).
6. **Live LSP smoke.** One per language (rust, py, ts, kt, java) — small fixture project, assert real LSP returns expected callers/callees. Behind `#[ignore]` flag.

## 7. Prompt Surface Updates

Per `CLAUDE.md § Prompt Surface Consistency` and the `ONBOARDING_VERSION` rule, these need coordinated updates when implementation lands:

- `src/prompts/server_instructions.md` — add `call_graph` to the navigation toolset; mention `call_graph(depth=1)` as the way to get call-filtered refs until `references(kind=call)` ships.
- `src/prompts/onboarding_prompt.md` — mention call_graph in the "navigation by knowledge level" section.
- `build_system_prompt_draft()` in `src/prompts/builders.rs` — same.
- Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs`.

Test `prompt_surfaces_reference_only_real_tools` will catch any stale references after rename or signature changes.

## 8. Open Items / Risks

| Item | Risk | Mitigation |
|---|---|---|
| Kotlin LSP callHierarchy support | Unknown; likely partial. May force ts-fallback for KT. | The fallback exists by design; no extra work needed. Validate empirically in step 6.6 (live smoke). |
| Macro/generic monomorphization | rust-analyzer's callHierarchy may or may not see through macros. | Documented in tool description (`source: "lsp"` ≠ omniscient); LLM consumer treats as best-effort even with `lsp`. |
| Cache size growth | Many edges across many sessions could bloat the DB. | Out of scope for v1. Add a size-based eviction policy in a follow-up if measured to matter. |
| Cross-project edges | Out of scope; could be useful in workspace-style projects. | Defer. project_id is in the schema, so multi-project joins are a future ALTER, not a rewrite. |

## 9. Implementation Sequencing (high-level)

Detailed plan goes in the next document via `superpowers:writing-plans`. Sketch:

1. LSP surface: add three callHierarchy methods to `LspClientOps`, `LspClient`, `MockLspClient`. Plumb through to rust-analyzer first to validate end-to-end.
2. Tree-sitter classifier with per-language node map. Unit tests per language.
3. Edge resolver (`call_edges::resolve_one_hop`) wrapping both paths.
4. sqlite cache table + read/write/invalidate. Wire `notify_file_changed` to invalidate.
5. Traversal engine (BFS, visited-set, depth-coherent cap).
6. Wire into `call_graph` tool: replace `RecoverableError` stub. Output formatter (compact / full / auto-promote).
7. Live-LSP smoke tests per language.
8. Prompt surface updates + `ONBOARDING_VERSION` bump.
9. Experimental docs page (`docs/manual/src/experimental/call-graph.md` + index.md entry).
