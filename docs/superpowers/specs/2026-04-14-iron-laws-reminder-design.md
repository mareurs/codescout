# Iron Laws Compression-Resilient Reminder

**Date:** 2026-04-14
**Branch:** experiments
**Status:** Design approved, pending implementation

## Motivation

MCP server instructions (`server_instructions.md`) are injected once per session.
Long sessions trigger context compression, which prunes them from the conversation
history. The model then falls back to native Read/Grep/Glob/Bash — behaviors the
companion plugin's `pre-tool-guard.sh` catches at the PreToolUse level, but only
after wasting a tool call.

Inspired by the caveman plugin's pattern of emitting full behavioral rules via
SessionStart hooks (which land in `system-reminder` blocks and survive compression),
this change adds a compressed reminder of the most-drifted rules to both session
start hooks in the companion plugin.

## Design Decisions

### D1: Emit compressed reminder, not full Iron Laws

Full Iron Laws (~27 lines) are already in the MCP server instructions for fresh
sessions. The hook backup only needs to anchor the most-drifted behaviors in compact
form. ~6 lines targeting the 3 behaviors the PreToolUse hook already guards against.

### D2: Hardcoded in hook, not file-based

The reminder is ~6 lines of stable text. Reading from a file (caveman's pattern)
adds maintenance cost for content that rarely changes. YAGNI.

### D3: Both SessionStart and SubagentStart

Subagents start with less context than the main session and are where drift is
worst. Both hooks get the same reminder.

## Reminder Content

```
CODESCOUT RULES (compression-resilient reminder):
• Source code: list_symbols + find_symbol, NOT read_file/Read
• Code edits: replace_symbol/insert_code/remove_symbol, NOT edit_file/Edit for structural changes
• Shell commands: run_command, NOT Bash — output buffers save tokens
• Markdown: read_markdown/edit_markdown, NOT read_file/edit_file
• Never pipe run_command output — query @ref buffers instead
```

## Files to Change

| File | Change | Risk |
|---|---|---|
| `hooks/session-start.sh` | Append reminder block to MSG before final output | Low — additive |
| `hooks/subagent-guidance.sh` | Same reminder block | Low — additive |

## What Stays Unchanged

- `detect-tools.sh` — no detection changes
- `hooks.json` — no new hooks
- `pre-tool-guard.sh` — still catches violations; reminder reduces but doesn't eliminate them
- System prompt injection — already works, unrelated

## Verification

1. Start a new Claude Code session in a codescout-configured project
2. Verify the `system-reminder` block contains the CODESCOUT RULES reminder
3. Dispatch a subagent and verify it also receives the reminder
4. Verify existing hook behavior (tool blocking, system prompt, memory hints) unchanged
