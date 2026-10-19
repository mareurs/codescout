# codescout — Project Overview

## Purpose

codescout is a Rust MCP server (v0.15.0) that gives AI coding agents IDE-grade code intelligence.
It exposes tools for: symbol navigation (LSP + tree-sitter), semantic search (Qdrant + dense
embedder + cross-encoder reranker), persistent per-project memory (markdown on disk), and an
embedded workspace artifact registry ("librarian") that indexes markdown docs into SQLite.

## Tech Stack

- Language: Rust (edition 2021, cargo workspace)
- MCP transports: stdio + HTTP
- LSP: rust-analyzer, pyright-langserver, typescript-language-server, gopls, jdtls, kotlin-lsp, clangd, OmniSharp, solargraph, vscode-html/css-language-server, bash-language-server (defaults in `src/lsp/servers/mod.rs::default_config`)
- Semantic search: Qdrant + codescout-embed (local ONNX via fastembed or remote HTTP)
- Artifact registry: SQLite (librarian)
- Primary MCP client: Claude Code; also Gemini CLI, Cursor, custom agents

## Package

- Binary + lib crate: `codescout`
- Version: 0.15.0  (verified against Cargo.toml 2026-10-19)
- Workspace member: codescout (also includes codescout-embed)

## Key MCP Tool Categories

**21 tools, registered unconditionally.** Read the derivation, not just the number — the two
available methods answer different questions and this section has been wrong in both
directions before.

- **Source walk** (re-walked 2026-10-19; `server_tool_count_is_l3_target` pins the 19 core tools):
  `CodeScoutServer::from_parts_with_env` (`src/server.rs`, the `vec![…]` at the top of the fn)
  pushes 19 unconditionally — `ReadFile`, `Tree`, `Grep`, `CreateFile`, `EditFile`,
  `RunCommand`, `Onboarding`, `ApproveWrite`, `Symbols`, `References`, `SymbolAt`, `CallGraph`,
  `EditCode`, `Memory`, `SemanticSearch`, `Index`, `Workspace`, `Library`, `GetGuide` — plus 2
  from the librarian's `all_tools()` (`src/librarian/tools/mod.rs`, wrapped by `adapters_for` and
  extended in after the `#[cfg(feature = "librarian")]` gate): `Artifact`, advertised
  under the name `doc`, and `Librarian`. Both librarian tools require the feature compiled in
  and enabled at runtime; both are true by default.
- **Live handshake** (2026-09-03; re-observed 2026-10-19 as this session's own tool list): an MCP
  `tools/list` against the running binary returned the same 21.

**The handshake is the authoritative one and the source walk is the cheap one.** They answer
different questions: the walk says what this checkout WOULD advertise, the handshake says what
the process actually IS advertising. On a shared tree those diverge routinely — the running
server is whatever binary was last `cargo rb`'d, which lags HEAD by however many commits have
landed since. When the two disagree, the handshake is right about the session you are in.

The 2026-09-02/03 tool-surface collapse cut this from 26: `read_markdown` / `edit_markdown`
folded into `read_file` / `edit_file`, and `artifact` / `artifact_event` / `artifact_augment` /
`artifact_refresh` became actions on the single `doc` tool.

- **Symbol navigation:** `symbols`, `symbol_at`, `references`, `call_graph` (`src/tools/symbol/`)
- **Reading:** `read_file` — heading-addressed on markdown, `force=true` for a raw line range
- **Semantic search:** `semantic_search`, `index` (`src/tools/semantic/`)
- **Code editing:** `edit_code` (LSP-aware), `edit_file` (text + markdown heading grammar), `create_file`
- **Memory:** `memory` (read/write/list/delete/remember/recall/forget/refresh_anchors)
- **Librarian:** `doc` (find/get/create/update/move/append_entry/augment/gather/…), `librarian`
- **Workspace:** `workspace` (`src/tools/config/`), `onboarding`, `tree`, `grep`, `run_command`, `library`
- **Guidance + guards:** `get_guide` (`src/tools/guide.rs`), `approve_write`

**21 is a CEILING, not what every session sees**, and both directions of that have burned
someone. `call_graph`, `references` and `symbol_at` are `Availability::RequiresLsp` and are
filtered out of `tools/list` when no language server is available — a bare session
advertises **18**. Going the other way, two tools are **opt-in and sit outside the 21**:
`peer` (Unix-only, gated by `peer_enabled_at_runtime`, `src/server.rs`) and `probe`
(`CODESCOUT_PROBE=1`, debug-only, the `env.probe` branch of the same constructor). Counting either into the total, or reporting 18
as the surface, is the standing error this paragraph exists to prevent.

## Runtime Requirements

- `~/.cargo/bin/codescout` symlink → `target/release/codescout` (release build only)
- Qdrant (optional, for semantic search)
- LSP servers installed locally (per language)
