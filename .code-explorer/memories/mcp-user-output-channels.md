# MCP User-Facing Output Channels — Research (2026-03-01)

## Problem
We want to show user-facing output (ANSI previews, status lines) from MCP tools
in Claude Code's terminal WITHOUT polluting the LLM's context window.

## Channels Investigated

### 1. Content audience filtering (`Role::User` blocks)
- **Status: BROKEN** — Claude Code issue #13600 (open)
- `Content::text(...).with_audience(vec![Role::User])` annotates blocks as user-only
- Claude Code does NOT filter by audience — LLM receives ALL content blocks regardless
- Confirmed by inspection: the LLM sees every block we return in `CallToolResult`

### 2. `notifications/message` (MCP logging channel)
- **Status: NOT DISPLAYED** — Claude Code issue #3174 (open)
- rmcp exposes `peer.notify_logging_message(LoggingMessageNotificationParam { level, logger, data })`
- Claude Code receives these notifications but silently swallows them (no terminal display)
- Also: MCP spec requires client to call `logging/setLevel` first before server sends logs
- Claude Code has not implemented `logging/setLevel`

### 3. `notifications/progress` (progress channel)
- **Status: NUMBERS ONLY** — no text field in rmcp-0.1.5's `ProgressNotificationParam`
- Only carries `step: u32` and `total: Option<u32>` — renders as spinner/progress bar
- The MCP spec progress notification does have a `message` string field, but rmcp-0.1.5 doesn't expose it

## Current Solution
`USER_OUTPUT_ENABLED: bool = false` const in `src/server.rs` (L42).
Strips all `Role::User`-audience blocks in `call_tool` dispatch before returning `CallToolResult`.
Single switch to flip when either issue is fixed upstream.

## Infrastructure Ready
- `ProgressReporter::report_text()` in `src/tools/progress.rs` — calls `notify_logging_message`, ready for when #3174 is fixed

## Note on format functions (2026-03-02)
`user_format.rs` was deleted as part of a refactor. All `format_compact()` helpers now
live as private `fn format_*()` functions in their respective tool files
(symbol.rs, file.rs, semantic.rs, git.rs, ast.rs, library.rs, memory.rs, usage.rs,
workflow.rs, config.rs). Shared helpers (`format_line_range`, `format_overflow`,
`truncate_path`) live in `src/tools/format.rs`.

## When to Revisit
- Claude Code fixes #13600 (audience filtering) → flip `USER_OUTPUT_ENABLED = true`
- Claude Code fixes #3174 (display notifications/message) → wire `report_text()` calls into tools
