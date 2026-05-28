# Workspace state

How `workspace.activate` (alias `activate_project`) switches the codescout
MCP server's active project, and what state is shared across every tool
call — including across subagents that share the parent's MCP server.

## What `activate_project` does

A single call to `activate_project(path=...)` flips the server's active
project to the given root. Implementation: `src/tools/config/mod.rs`
(`ActivateProject` tool). The call has these side effects, in order:

1. **Clears `ctx.guide_hints_emitted`** — the per-session set tracking
   which `get_guide(topic)` topics the model has been hinted about. A
   fresh activation re-triggers first-call hints for the new project.
2. **Resolves the path.** Bare project IDs (no `/`) inside a workspace
   are focus-switches; absolute paths trigger full activation. Path
   must be an existing directory or you get a `RecoverableError`
   (`isError: false`, sibling calls survive).
3. **Prewarms LSP** for the project's languages (background — does not
   block the response).
4. **Auto-registers dependencies** for cross-project navigation
   (`crate::library::auto_register::auto_register_deps`).
5. **Resets `path_note_emitted_since_activation`** on
   `CodeScoutServer` — so the next non-`run_command` tool response
   that contains the new root re-emits `[codescout] paths are
   relative to <root>`.

The response includes `project_hints` (primary language, manifest,
entry points, build commands) so the model has orientation context
even without running `onboarding`.

## The home/foreign distinction

The **first** project activated in an MCP session is the **home**
project. Every subsequent activation to a different path is a **foreign**
activation. The `read_only` param defaults differ:

- **home**: `read_only = false` (mutations allowed by default)
- **foreign**: `read_only = true` (read-only by default; pass
  `read_only=false` explicitly to enable writes)

This matters because the MCP server is shared state across the session.
Activating a foreign project leaves the server pointed at it until you
explicitly restore home or end the session.

## Per-session state reset

Activation clears these per-session sets:

| State | Owner | Behavior |
|---|---|---|
| `guide_hints_emitted` | `CodeScoutServer` | Cleared on every activation. Next first-touch of a tool with `relevant_guide_topic()` re-emits `_guide_hint`. |
| `path_note_emitted_since_activation` | `CodeScoutServer` | Cleared on every activation. Next stripped response re-emits the path-relative banner. |
| `section_coverage` | `CodeScoutServer` | NOT cleared. Section-read tracking persists across activations. |
| Output buffers (`@tool_*`, `@cmd_*`) | `OutputBuffer` | NOT cleared. Buffers from before the switch remain readable. |

## Path-relative annotation

Every non-`run_command` tool response that contains paths under the
active project root is stripped to project-relative form. The first
stripped response since activation carries a trailing note:

```
[codescout] paths are relative to /home/user/<project>
```

Subsequent stripped responses in the same activation window do NOT
re-emit the note (novelty-gated). `run_command` output is exempt —
raw shell bytes, stripping would corrupt path literals. See
`docs/issues/2026-05-28-path-annotation-spam.md` for the rationale.

## Cross-project workflow pattern

When you need to work in a sibling project briefly:

```
1. workspace(activate, path="/home/user/other-project")
2. <do the work — any number of tool calls>
3. workspace(activate, path="/home/user/code-explorer")   # restore home
```

Skip step 3 and the server stays pointed at the foreign project. The
next session inherits the foreign root as "active." This is the
**workspace gate** from `server_instructions` — restore home before
the turn ends.

For one-off reads, prefer `read_only=true`:

```
workspace(activate, path="/sibling", read_only=true)
```

Read-only mode blocks writes at the agent layer regardless of the
caller's intent — defense against accidental edits while scouting.

## Subagent semantics

Subagents that share the parent's MCP server share:

- The same active project (no per-subagent override)
- The same `guide_hints_emitted` set (parent-triggered hints don't
  re-fire for subagents)
- The same `path_note_emitted_since_activation` flag

A subagent that needs the workspace pointed at a different root must
itself call `activate_project` — and then restore the parent's home
before returning. This is dangerous: the parent's next call after the
subagent returns will see whatever workspace state the subagent left.
Prefer not to switch workspace inside subagents; if you must, document
in the subagent's spawn prompt that it will restore home before exit.

## Anti-patterns

- **Forgetting to restore home.** Iron-Law-grade. Server is shared
  state; the next session sees your foreign activation as the active
  project. Symptoms: tools operate on the wrong codebase, semantic
  search returns unrelated results.
- **Switching workspaces inside a subagent without restoration.**
  Parent's next tool call lands in the subagent's workspace. Caller
  has no way to detect this without an extra `workspace(status)` call.
- **Relying on `guide_hints_emitted` to survive activation.** Every
  `activate_project` resets it. If a hint was useful, capture the
  guide content in the parent's prompt or call `get_guide(topic)`
  again after activation.
- **Treating `read_only=true` as a no-op.** It blocks mutations at
  the agent layer; tools that try to write will fail with
  `RecoverableError`. Use it deliberately for scout-only work.

## Related

- `get_guide("error-handling")` — `RecoverableError` routing for
  invalid paths and read-only violations
- `get_guide("progressive-disclosure")` — `[codescout] paths are
  relative to <root>` mechanics, path stripping, buffer behavior
- `docs/issues/2026-05-28-path-annotation-spam.md` — full rationale
  for the novelty-gated path annotation
- Iron Law 6 in `server_instructions` — subagent dispatch discipline
  (parent must brief subagents about workspace state, among other
  context)
