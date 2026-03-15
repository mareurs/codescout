> ⚠ Experimental — may change without notice.

# Compact Tool Schemas & `activate_project` Safety

Two related improvements land together: tool schema descriptions were trimmed by ~24% (~1,763
tokens), and a new Iron Law + server guidance was added for safe cross-project navigation with
`activate_project`.

## Compact tool schemas

Parameter descriptions in all tool schemas previously repeated usage guidance that already
lives in `server_instructions.md`. They now answer only "what is this parameter?" — not "how
should you use the tool?". The system prompt is the right place for workflow guidance; schemas
are for type/shape information.

**Net effect:** ~1,763 fewer tokens injected on every MCP request, partially offset by ~448
tokens added for the new Iron Law. Net saving across a typical session is significant since
`server_instructions.md` is injected on every tool call.

## `activate_project` safety — Iron Law #4

A new Iron Law was added to `server_instructions.md`:

> **ALWAYS RESTORE THE ACTIVE PROJECT.** After `activate_project` to a different project,
> you MUST `activate_project` back to the original before finishing your task. The MCP server
> is shared state — forgetting to return silently breaks all subsequent tool calls for the
> parent conversation.

### Cross-project navigation patterns

Two patterns are now documented and enforced via anti-pattern guidance:

| Need | Pattern |
|---|---|
| Quick lookup (1–3 calls) | Pass `project: "<id>"` on the tool call — no state change, no risk |
| Sustained exploration | `activate_project("<other>")` → work → `activate_project` back |

**Subagents are especially risky** — they share the MCP server instance with their parent
conversation. A subagent that calls `activate_project` and exits without restoring leaves the
parent's subsequent tool calls operating against the wrong project root, with no error.

### `activate_project` response hint

When switching away from the home project, the `activate_project` response now includes a
reminder to restore:

```
Active project: other-project (/path/to/other)
⚠ You switched away from home-project. Remember to activate_project back when done.
```

### Workspace system prompt

`build_system_prompt_draft` (the generated per-project system prompt) now includes a
cross-project navigation section when the workspace has more than one project registered.
This ensures the guidance is present in project-specific contexts, not just the global
server instructions.
