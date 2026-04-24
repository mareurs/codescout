# Cross-Project Gotchas

## Embedding Dimension Mismatch

Memory writes produce a `cross-embed failed: Dimension mismatch` warning when the semantic
index was built with one model (e.g., 384-dim) but the current embedder produces a different
dimensionality (e.g., 768-dim). The write still succeeds — the warning is non-blocking.
Fix: re-run `index_project` to rebuild the semantic index with the current model.

## Kotlin LSP Concurrent Instances

Kotlin language server conflicts when multiple instances target the same project root. codescout
uses a Unix-socket MUX proxy (`src/lsp/mux/`) to route multiple clients through a single
kotlin-language-server instance. Idle TTL: 2 hours for Kotlin, 30 minutes for all other LSPs.
Details: `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.

## MCP Server Uses Release Binary

`/mcp` restart in Claude Code picks up `target/release/codescout` — NOT a dev build.
Always run `cargo build --release` before testing tool changes via the live MCP server.

## Parallel Write Safety

Never dispatch parallel write tool calls (`edit_file`, `replace_symbol`, `insert_code`,
`create_file`, `replace_symbol`). rmcp 0.1.5 has a cancellation race; parallel writes can
produce inconsistent partial state. Always serialize write calls. See MEMORY.md BUG-021.

## fixture libraries: no LSP by default

Fixture projects have no LSP server auto-started. codescout starts the appropriate language
server on first LSP tool use per fixture. Cold-start latency can cause the first
`goto_definition`/`hover` call to time out and retry. The circuit breaker and cold-start
retry budget are in `src/lsp/`.

## librarian-mcp: Single SQLite Writer

`Catalog` wraps a single `rusqlite::Connection` behind `parking_lot::Mutex`. There is no
connection pool. Long embedding operations are done outside the lock, but the lock is held
for the full sync indexing phase. Do not attempt concurrent catalog mutations.

## codescout-embed: First-Use Model Download

`LocalEmbedder` downloads the ONNX model to `~/.cache/huggingface/hub/` on first use (22MB–300MB).
Subsequent starts are instant. `RemoteEmbedder` (Ollama) has a 300-second HTTP timeout to
survive GPU discovery delays; ensure Ollama is running before triggering indexing.

## Prompt Surface Stale References

Three surfaces reference tool names: `server_instructions.md`, `onboarding_prompt.md`,
`builders.rs`. Renaming or removing a tool without updating all three causes the test
`prompt_surfaces_reference_only_real_tools` to fail. Always update all three together.

## GitHub Tools Not Registered

`src/tools/github.rs` exists but `github_identity`, `github_issue`, `github_pr`, `github_file`,
`github_repo` are NOT registered in `server.rs`. Use the `gh` CLI via `run_command` instead.

## path_security check_tool_access Must Include New Write Tools

`check_tool_access()` in `src/util/path_security.rs` uses a hardcoded list of tool names per
category. Adding a write tool without adding it to this function bypasses access controls silently.
The `*_disabled_blocks_*` test catches this, but only if the test is also updated.
