# codescout — Project Overview

## Purpose

codescout is a Rust MCP server (v0.15.0) that gives AI coding agents IDE-grade code intelligence.
It exposes tools for: symbol navigation (LSP + tree-sitter), semantic search (Qdrant + dense
embedder + cross-encoder reranker), persistent per-project memory (markdown on disk), and an
embedded workspace artifact registry ("librarian") that indexes markdown docs into SQLite.

## Tech Stack

- Language: Rust (edition 2021, cargo workspace)
- MCP transports: stdio + HTTP
- LSP: rust-analyzer, jdtls, kotlin-language-server, pylsp, tsserver
- Semantic search: Qdrant + codescout-embed (local ONNX via fastembed or remote HTTP)
- Artifact registry: SQLite (librarian)
- Primary MCP client: Claude Code; also Gemini CLI, Cursor, custom agents

## Package

- Binary + lib crate: `codescout`
- Version: 0.15.0  (verified against Cargo.toml 2026-08-31)
- Workspace member: codescout (also includes codescout-embed)

## Key MCP Tool Categories

Enumerated from the `Tool::name` implementations 2026-08-31 — the earlier version of this
list omitted six registered tools, `read_file` and `read_markdown` among them.

- **Symbol navigation:** `symbols`, `symbol_at`, `references`, `call_graph` (`src/tools/symbol/`)
- **Reading:** `read_file`, `read_markdown` — the Iron-Law-preferred readers
- **Semantic search:** `semantic_search`, `index` (`src/tools/semantic/`)
- **Code editing:** `edit_code` (LSP-aware), `edit_file`, `edit_markdown`, `create_file`
- **Memory:** `memory` (read/write/list/delete/remember/recall/forget/refresh_anchors)
- **Librarian:** `artifact`, `artifact_event`, `artifact_refresh`, `artifact_augment`, `librarian`
- **Workspace:** `workspace` (`src/tools/config/`), `onboarding`, `tree`, `grep`, `run_command`, `library`
- **Guidance + guards:** `get_guide` (`src/tools/guide.rs`), `approve_write`
- **Instrumentation:** `probe`, `peer`, usage stats (`src/tools/usage.rs`)

## Runtime Requirements

- `~/.cargo/bin/codescout` symlink → `target/release/codescout` (release build only)
- Qdrant (optional, for semantic search)
- LSP servers installed locally (per language)