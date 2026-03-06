# Domain Glossary

Terms specific to codescout not fully explained in CLAUDE.md or docs.

**RecoverableError** — Tool error that maps to MCP `isError:false`. Agents see a JSON
`{ok:false, error, hint?}` body and can recover without aborting sibling parallel calls.
Use for: path not found, unsupported language, empty result. See `src/tools/mod.rs:78`.

**OutputGuard** — Progressive disclosure enforcer. Created via `OutputGuard::from_input(&input)`,
reads `detail_level`/`offset`/`limit` from tool input. See `src/tools/output.rs` and
`CLAUDE.md § Design Principles`.

**OverflowInfo** — Struct attached to responses when results exceed the cap. Contains `shown`,
`total`, `hint` (actionable narrowing advice), `by_file` (per-file match counts).

**`@tool_*` ref** — Buffer handle for large tool output (> MAX_INLINE_TOKENS). Stored in
`OutputBuffer` (50-slot ring, `src/tools/output_buffer.rs`). Query with `read_file("@tool_*")`.

**`@cmd_*` ref** — Buffer handle for `run_command` stdout. Plain text, not JSON.
Query with `run_command("grep pattern @cmd_*")`.

**`@ack_*` ref** — Acknowledgment handle returned by `edit_file` for large edits.
See `looks_like_ack_handle()` in `src/tools/workflow.rs`.

**ActiveProject** — The currently active project: `{root, config, memory, private_memory,
library_registry}`. Held in `Agent::inner` behind RwLock. See `src/agent.rs:48`.

**Scope** — Parameter on symbol/semantic tools: `"project"` (default), `"lib:name"`,
`"libraries"`, `"all"`. Parsed by `Scope` enum in `src/library/scope.rs`.

**LspProvider / LspClientOps** — Traits in `src/lsp/ops.rs` that decouple tools from the
concrete `LspClient`. `LspManager` implements `LspProvider`; `LspClient` implements
`LspClientOps`. `MockLspClient` / `MockLspProvider` used in tests.

**drift** — Per-file embedding staleness metric: how much a file's current content
diverges from what was indexed. `avg_drift` + `max_drift` per file. See `src/embed/drift.rs`.

**`tool_timeout_secs`** — Per-project tool execution timeout (`.codescout/project.toml`,
`ProjectSection`). Tools that skip it: `run_command`, `index_project` (see
`tool_skips_server_timeout()` in `src/server.rs:203`).
