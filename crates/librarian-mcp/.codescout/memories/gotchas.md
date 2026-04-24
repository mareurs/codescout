# Cross-Project Gotchas

## Shared MCP Server State

- **Active project is global state.** All subagents share the same MCP server. Calling `activate_project` in one agent affects all others running concurrently. Always restore home project (`/home/marius/work/claude/code-explorer`) after activate_project to another workspace member.
- **Iron Law:** Every agent that calls `activate_project` to a non-home project MUST call it back at the end — even on error/cancellation.

## Path Resolution

- `list_dir(".")` and `read_file("relative/path")` resolve relative to the **currently active project root**, not the caller's CWD. With multiple activate_project calls in flight, this produces wrong results. Use absolute paths for all codescout tool calls in multi-project contexts.
- `list_symbols("src")` with a relative path while a different project is active returns symbols from that project, not the intended one.

## find_symbol Body Truncation

- `find_symbol(include_body=true)` uses LSP `workspace/symbol` which returns name-position (single line), not the full declaration range. Result: `start_line == end_line`, body = signature only. **Workaround:** Use `list_symbols(path)` first for correct line ranges, then targeted `find_symbol` for methods (which use `documentSymbol` and get full bodies). For top-level functions in large files, use `list_symbols` + `read_file` with line ranges.

## Kotlin LSP

- Kotlin LSP has known multi-instance conflicts. See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.
- Cold start behavior: first symbol request may time out; circuit breaker trips after repeated failures; retry after 30s.
- Do NOT start two concurrent kotlin-language-server instances for the same project root.

## Embedding Models

- `BGESmallENV15` and `BGESmallENV15Q` crash on CPU — GPU-only fastembed models. Do not recommend these.
- `RemoteEmbedder.dimensions()` returns 0 before first `embed()` call. Callers must handle zero.
- Fixture projects (java/kotlin/python/rust/typescript-library) are not in the main embedding index. `semantic_search` over fixtures falls through to the parent project's index or returns empty.

## librarian-mcp vs code-explorer Binaries

- Both are separate binaries in the same Cargo workspace. `cargo build --release` builds both.
- `librarian-mcp` is launched independently from codescout — it is not embedded in the MCP server.

## Three Prompt Surfaces

- `src/prompts/server_instructions.md`, `src/prompts/onboarding_prompt.md`, `src/prompts/builders.rs` — all three must be updated together when tools change. Test `prompt_surfaces_reference_only_real_tools` catches stale references. Failing this test = stale tool name in at least one surface.

## Memory Leak (Partial Fix Applied 2026-04-18)

- `build_index` peak-memory overlap from `flat_texts` clone + ptmalloc2 arena retention. See `project_memory_leak.md` in auto-memory.
