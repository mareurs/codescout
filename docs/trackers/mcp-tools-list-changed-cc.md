---
kind: tracker
status: draft
title: CC notifications/tools/list_changed Support
owners: []
tags:
  - mcp
  - claude-code
  - monitoring
---

# Tracker: CC `notifications/tools/list_changed` Support

**Purpose:** Monitor when Claude Code implements handling for the MCP
`notifications/tools/list_changed` notification, which would allow codescout to
dynamically register librarian tools mid-session without a server restart.

**Why it matters:** With this feature, `librarian_activate` could flip a runtime
flag, push the 15 librarian tools into the live tool list, send the notification,
and have them immediately available — no `/mcp` restart needed.

**GitHub issues to watch:**
- https://github.com/anthropics/claude-code/issues/13646 (primary)
- https://github.com/anthropics/claude-code/issues/4118 (duplicate)

**Status:** `NOT SUPPORTED`

---

## Log

### 2026-05-02 — Initial research (manual)

- Both GitHub issues open, no handler registered in CC for the notification.
- Protocol is defined in CC's Zod schemas but never actioned.
- Gemini CLI and Spring AI have implementations; CC does not.
- Sources: web search + GitHub issue inspection.
- **Status: NOT SUPPORTED**
