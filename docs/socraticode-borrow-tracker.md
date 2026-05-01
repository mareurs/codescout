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

## L3 cleanup follow-ups

- **Internal helper renames** — `format_find_references`, `run_find_references`, `format_goto_definition`, `format_hover`, and tests like `find_references_format_compact_shows_count` / `find_referencing_symbols_schema_includes_scope` still carry old tool-concept names. Private only; no MCP impact. Cheap end-of-L3 cleanup commit after Task 16.
- **`symbol_at` minor follow-ups** — (1) add tests for `fields` validation paths (unknown-string, non-string-entry, empty-array, non-array); (2) extract shared col-resolution into `resolve_col(source_line, col_param, identifier, fallback)` to remove ~30 lines of duplication between `fetch_definition` / `fetch_hover`; (3) replace 5-tuple return of `read_position_inputs` with a `PositionInputs` struct.
- **`symbols` minor follow-ups** — (1) `format_compact` dispatch by JSON-key presence is brittle; add internal `_mode` discriminator OR regression test asserting both shapes route to the right formatter; (2) empty-string `name`/`query` silently returns all symbols (cap=50) — should treat as missing or `RecoverableError`; (3) `query` + `symbol` simultaneously: `pattern` follows `query` precedence but `is_name_path` follows `symbol` precedence → behavior mismatch; either reject or align precedence; (4) rename `FIND_SYMBOL_MAX_RESULTS` → `SYMBOLS_SEARCH_MAX_RESULTS`; (5) `description()` could mention `name_path` for symmetry with schema; (6) replace `"references"` placeholder in `src/usage/db.rs` and `src/tools/usage.rs` test fixtures with an obviously-fake string to prevent future overlap with real `references` usage stats.

## Spec/plan corrections discovered during implementation

- **Tool names**: spec referred to `find_file` and `search_pattern` but actual registered names are `glob` and `grep`. Plan tasks reference the wrong names; implementers should use real names.
- **Missed tools**: `edit_markdown` and `read_markdown` exist as registered tools and were absent from spec inventory. Both stay unchanged through L3.
- **Final count revision**: post-L3 tool count is **22**, not 20. Recount: 25 original − 13 removed + 6 merged + 1 stub (`call_graph`) + 2 missed (`edit_markdown`, `read_markdown` always there) − 1 onboarding accounting fix (already counted) = 22 effective. The `server_tool_count_is_l3_target` test in Task 9 must assert 22.
- **`grep` vs `search_pattern` naming**: spec listed `search_pattern` as unchanged. Actual name is `grep`. Decision (deferred): keep as `grep` for now (no rename in L3); revisit naming consistency at end of L3 or in a follow-up.

## Deferred decisions

Captured here so they don't get lost during sequencing.

- **`references` kind filter** — L3 ships `references(symbol)` returning all uses (current behavior, no filter). When A lands, evaluate adding `kind=call|noncall` filter via tree-sitter classifier so callers can ask "all imports/type refs but not calls". Cheap because A already needs call-site classification. Per-language rules for: rust, python, ts/js, kotlin, java.
- **`flow(from, to)` path search** — socraticode's `codebase_flow` finds call paths between two symbols (BFS). Different shape from `call_graph` traversal. Defer until A proves out; ship as separate tool if real demand.
- **L3 → 18 tools target vs 19 actual** — `references` retained alongside `call_graph` since LSP's undifferentiated refs cover imports/type uses that calls don't. Reconsider drop only after kind-filter classifier exists and `call_graph` covers calls.
