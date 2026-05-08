# Project — Code Explorer Guidance

## Entry Points
- Explore with `tree(".")` then `symbols` on key files

## Key Abstractions
- [3-5 entries max. Each = one line: `TypeName` (`path/`) — one-line purpose only. No narrative.]

## Search Tips
- Use specific terms over generic ones (e.g., avoid 'data', 'utils')
- For call relationships and impact analysis: `call_graph(symbol, path)` — traces callers/callees

## Navigation Strategy
1. `memory(action="read", topic="architecture")` — orient yourself
2. `symbols("src/")` — see main structure
3. `semantic_search("your concept")` — find relevant code
4. `symbols(name="Name", include_body=true)` — read implementation
   - regex-like patterns belong in `grep`, not `symbols`
4b. `symbol_at(path, line)` — hover + type sig when you have an exact location from prior tool output; skip re-searching
4c. `references(symbol, path)` — all call sites before any edit
5. `call_graph(symbol="Name", direction="callers")` — transitive blast radius; `direction="callees"` for flow tracing
6. `memory(action="recall", query="...")` — search memories by meaning

7. `read_markdown("path/to/file.md")` — returns heading map + `@file_ref` for large files. **IRON LAW #6:** subsequent reads MUST use `@file_ref` (not the original path): `read_markdown("@file_ref", heading="## Section")` or `start_line=/end_line=`.

## MCP Resources
Extended docs and project context are available via MCP resources (`resources/read <uri>`):
- `doc://codescout-tool-guide` — long-form usage notes for every tool (examples, tradeoffs)
- `memory://<name>` — project memory files (architecture, conventions, gotchas)
- `project://summary` — active project + index + LSP snapshot

## Project Rules
- [Fill from Phase 1 exploration: linting, formatting, commit conventions]
