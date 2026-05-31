# Compact Tool Schemas & `workspace(action: activate)` Safety

Two related improvements land together: tool schema descriptions were trimmed by ~24% (~1,763
tokens), and a new Iron Law + server guidance was added for safe cross-project navigation with
`workspace(action: activate)`.

## Compact tool schemas

Parameter descriptions in all tool schemas previously repeated usage guidance that already
lives in `server_instructions.md`. They now answer only "what is this parameter?" — not "how
should you use the tool?". The system prompt is the right place for workflow guidance; schemas
are for type/shape information.

**Net effect:** ~1,763 fewer tokens injected on every MCP request, partially offset by ~448
tokens added for the new Iron Law. Net saving across a typical session is significant since
`server_instructions.md` is injected on every tool call.

## `workspace(action: activate)` safety — Iron Law #4

A new Iron Law was added to `server_instructions.md`:

> **ALWAYS RESTORE THE ACTIVE PROJECT.** After `workspace(action: activate)` to a different project,
> you MUST `workspace(action: activate)` back to the original before finishing your task. The MCP server
> is shared state — forgetting to return silently breaks all subsequent tool calls for the
> parent conversation.

### Cross-project navigation patterns

The supported cross-project navigation patterns, by need:

| Need | Pattern |
|---|---|
| Quick lookup / one-off cross-project call | Pass `workspace: "<absolute path>"` on the tool call — pins that one call, no state change, no risk |
| Sustained exploration (single agent) | `workspace(action: activate, path: "<other>")` → work → `workspace(action: activate)` back |
| Concurrent subagents on *different* workspaces | Each subagent passes `workspace: "<absolute path>"` per call (see below) — do **not** rely on `activate` |

**Subagents are especially risky** — they share the MCP server instance with their parent
conversation. A subagent that calls `workspace(action: activate)` and exits without restoring leaves the
parent's subsequent tool calls operating against the wrong project root, with no error.

### Concurrent subagents — per-request `workspace` pinning

Iron Law #4 (restore the active project) is enough for *sequential* cross-project work by a
single agent. It is **not** enough when parallel subagents share one server and operate on
*different* workspaces. The active project is process-global, so two subagents that
`workspace(action: activate)` different paths race — last writer wins, and the loser silently
reads or writes the wrong workspace. "Restore when done" cannot help: the corruption happens
mid-flight, in the window between the two activations.

The fix is to **not activate at all** in that case. Every pinnable tool accepts an optional
`workspace` parameter — an absolute path that pins *that one call* to the named workspace,
regardless of the shared active project:

```
symbols(name: "Foo", workspace: "/home/me/project-a")
run_command(command: "cargo test", workspace: "/home/me/project-b")
```

Concurrent pins never collide — each call resolves independently against its own workspace.
Single-workspace work omits the parameter and is unaffected. The unpinned
`workspace(action: activate)` path still emits a `concurrent_activation_warning` on a rapid
foreign switch, now pointing at this pin as the primary remedy (separate Claude Code windows
remain a clean fallback — separate processes, separate active-project slots).

**Why a per-call parameter, and not a per-subagent active project?** The MCP `RequestContext`
exposes no stable per-subagent key, so there is nowhere to hang a "this subagent's project"
slot — the resolution has to travel *with the request*. The `workspace` parameter is that
request-scoped carrier.
### `workspace(action: activate)` response hint

When switching away from the home project, the `workspace(action: activate)` response now includes a
reminder to restore:

```
Active project: other-project (/path/to/other)
⚠ You switched away from home-project. Remember to workspace(action: activate) back when done.
```

### Workspace system prompt

`build_system_prompt_draft` (the generated per-project system prompt) now includes a
cross-project navigation section when the workspace has more than one project registered.
This ensures the guidance is present in project-specific contexts, not just the global
server instructions.
