# codescout (code-explorer) — Project Overview

## Purpose

codescout is a Rust MCP server (v0.14.0) that gives AI coding agents IDE-grade code intelligence.
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
- Version: 0.14.0
- Workspace member: code-explorer (also includes codescout-embed)

## Key MCP Tool Categories

- **Symbol navigation:** `symbols`, `symbol_at`, `references`, `call_graph`
- **Semantic search:** `semantic_search`
- **Code editing:** `edit_code` (LSP-aware), `edit_file`, `edit_markdown`, `create_file`
- **Memory:** `memory` (read/write/list/delete/remember/recall/forget)
- **Librarian:** `artifact`, `artifact_event`, `artifact_refresh`, `librarian`
- **Workspace:** `workspace`, `index`, `onboarding`, `tree`, `grep`, `run_command`

## Runtime Requirements

- `~/.cargo/bin/codescout` symlink → `target/release/codescout` (release build only)
- Qdrant (optional, for semantic search)
- LSP servers installed locally (per language)