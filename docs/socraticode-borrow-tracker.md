# Socraticode Borrow Tracker

Features worth porting from `../socraticode` to codescout. One at a time.

## Status legend
- 🔵 queued — agreed, not started
- 🟡 brainstorming — design in progress
- 🟢 in progress — implementing
- ✅ done
- ⚪ deferred / undecided

## Candidates

| ID | Status | Feature | Why |
|----|--------|---------|-----|
| **L3** | 🟡 | **Tool surface compression (25 → ~19)** — merge find_symbol+list_symbols, goto_definition+hover, list_dir+find_file, activate_project+project_status, list_libraries+register_library, index_project+index_status; introduce `call_graph`; keep `references` for non-call refs | Prereq for A. Reduces prompt-surface noise, removes overlaps revealed when designing call_graph. |
| A | 🔵 | **Code graph + blast radius** — file-import + symbol call graph; `impact(symbol)` returns transitive callers/callees. LSP-backed (A4), sqlite-cached (B), bidirectional (C), one tool `call_graph(symbol, direction, max_depth)`. Sequenced after L3. | Safety for AI edits — knows what breaks before mutating. Codescout's biggest gap. |
| B | 🔵 | **Hybrid search (BM25 + dense RRF)** | Better identifier/API recall than dense-only sqlite-vec. |
| C | 🔵 | **Context artifacts** — index non-code files (DB schema, OpenAPI, infra YAML) into separate semantic store | Bridge code ↔ external specs for AI tasks. |
| D | 🔵 | **File watcher auto-reindex** — debounced incremental updates on file change | Removes manual `index_project` burden, indexes stay fresh. |
| E | 🔵 | **Graph visualization** — Cytoscape.js HTML viewer + Mermaid output | Architecture review aid; pairs with A. |
| F | 🔵 | **Multiple embedding providers** — Ollama / OpenAI / Google pluggable | Escape locked-in embedded model; better quality on demand. |
| G | 🔵 | **Cycle detection + entry-point detection** | Architecture hygiene; cheap once A exists. |
| H | 🔵 | **Branch-aware indexing** — per-branch collections | Multi-worktree workflows without index thrash. |
| I | 🔵 | **Cross-project linking** — search across linked projects | Monorepo / multi-repo navigation. |

## Active

**L3 — Tool surface compression.** Brainstorming. (Blocks A.)

## Deferred decisions

Captured here so they don't get lost during sequencing.

- **`references` kind filter** — L3 ships `references(symbol)` returning all uses (current behavior, no filter). When A lands, evaluate adding `kind=call|noncall` filter via tree-sitter classifier so callers can ask "all imports/type refs but not calls". Cheap because A already needs call-site classification. Per-language rules for: rust, python, ts/js, kotlin, java.
- **`flow(from, to)` path search** — socraticode's `codebase_flow` finds call paths between two symbols (BFS). Different shape from `call_graph` traversal. Defer until A proves out; ship as separate tool if real demand.
- **L3 → 18 tools target vs 19 actual** — `references` retained alongside `call_graph` since LSP's undifferentiated refs cover imports/type uses that calls don't. Reconsider drop only after kind-filter classifier exists and `call_graph` covers calls.
