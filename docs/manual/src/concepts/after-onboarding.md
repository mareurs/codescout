# After Onboarding: Slimming Your Config File

Running `onboarding` for the first time writes memories for architecture, entry points,
conventions, and gotchas. Many projects also have a hand-written config file —
`CLAUDE.md`, `AGENTS.md`, `.cursorrules`, or `.github/copilot-instructions.md` — that
predates codescout and repeats a lot of the same information.

Keeping both is redundant. Every session the agent reads the config file eagerly (full
cost, always) and then reads memories on demand. When the same fact lives in both places,
you pay for it twice and get two sources of truth that can drift apart.

This page walks through how to trim your config file after onboarding so it carries only
what it should.

## What belongs where

| Config file (`CLAUDE.md` etc.) | codescout memories |
|---|---|
| Workflow preferences ("always run tests before committing") | Entry points and module responsibilities |
| Tool-specific instructions ("use `cargo clippy -- -D warnings`") | Naming conventions and code patterns |
| Team conventions that govern *agent behavior* | Architectural decisions and their rationale |
| Security or access rules | Gotchas and non-obvious interactions |
| Things that must fire before the agent does *anything* | Debugging insights from past sessions |

The rule of thumb: if a fact describes **how the codebase is built**, it belongs in
memory. If it describes **how you want the agent to behave**, it belongs in the config
file.

## The audit workflow

After onboarding completes, ask the agent to do a one-time audit:

```
Now that codescout memories are written, please audit CLAUDE.md:
1. List all memory topics with `memory(action: "list")`
2. Read each topic with `memory(action: "read", topic: "...")`
3. For each block in CLAUDE.md that is fully covered by a memory, either delete it
   or replace it with a one-line reference: "See codescout memory '<topic>'"
4. Keep anything that is a workflow rule or behavior instruction
```

The agent will compare each section against the memory store and propose edits. Review
and apply.

## Example: before and after

**Before** (verbose, duplicated in memory):

```markdown
## Project Structure

The main entry point is `src/main.rs`. The server is wired in `src/server.rs` via
`CodeScoutServer::from_parts`, which registers all 29 tools. Tools are implemented in
`src/tools/` grouped by category. The `Agent` struct in `src/agent.rs` holds project
state and is accessible to all tools via `ctx.agent`. Error routing goes through
`route_tool_error` in `src/server.rs`: `RecoverableError` maps to `isError: false`,
all other errors to `isError: true`.
```

**After** (slim, memory carries the detail):

```markdown
## Architecture

See codescout memory "architecture".
```

Or if the memory coverage is complete, delete the section entirely and trust the agent
to read the memory at session start.

## Per-agent config file names

| Agent | Config file |
|---|---|
| Claude Code | `CLAUDE.md` |
| GitHub Copilot | `.github/copilot-instructions.md` |
| Cursor | `.cursorrules` or `AGENTS.md` |
| Codex / OpenAI agents | `AGENTS.md` |
| Generic | `AGENTS.md` |

The audit workflow is the same regardless of which file your agent uses.

## When to repeat the audit

- After adding a significant new module or subsystem (onboarding will have written new
  memories — check if the config file still duplicates them)
- After onboarding a multi-project workspace (each project gets its own memories;
  workspace-level config can often shed per-project sections entirely)
- When context window pressure becomes noticeable — a bloated config file is often the
  first place to reclaim tokens

## Further reading

- [Memory](memory.md) — what makes a good memory entry and the full `memory` tool reference
- [Onboarding](../tools/workflow-and-config.md) — what `onboarding` writes and when to re-run it
- [Multi-Project Workspace](../experimental/multi-project-workspace.md) — per-project
  memory scoping in workspace setups
