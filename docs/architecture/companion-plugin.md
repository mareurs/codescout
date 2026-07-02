# Companion Plugin: codescout-companion

codescout ships with a companion Claude Code plugin at
**`../claude-plugins/codescout-companion/`** that is **always active** when working
on codescout. `CLAUDE.md` carries only the one critical behavioral fact (native
file/shell tools on source are hard-denied — use codescout MCP tools); the full
hook inventory lives here. Source of truth is the plugin's own `hooks/hooks.json`.

## What it does (headline hooks)

- `SessionStart` hook (`hooks/session-start.sh`) — injects tool guidance + memory hints into every session
- `SubagentStart` hook (`hooks/subagent-guidance.sh`) — same for all subagents
- `PreToolUse` hook on `Grep|Glob|Read|Bash|Edit|Write` (`hooks/pre-tool-guard.sh`) — **hard-denies (`permissionDecision: deny`) native Read/Grep/Glob/Edit/Write on source files and native Bash**, redirecting to codescout MCP tools

## Full hook inventory (per `hooks/hooks.json`)

**PreToolUse (guards — hard `permissionDecision: deny`):**
- `mcp__codescout__(edit_code|edit_file|edit_markdown|create_file)` → `worktree-write-guard.sh` — blocks codescout write tools when in a git worktree until `workspace(activate)` has run (clears the `.cs-worktree-pending` marker).
- `Bash` → `git-worktree-guard.sh` — denies worktree-ambiguous destructive git verbs from Bash; requires `git -C <path>` (single-worktree repos carved out).
- `mcp__.*__read_file` → `il4-deny-hook.sh` — IL4: hard-denies `read_file` on `.md` paths, redirecting to `read_markdown`.

**PreToolUse (advisory — `exit 0` + injected hint):**
- `mcp__.*__run_command` → `il3-warn-hook.sh` — IL3: warns (does not block) when piping unbounded `run_command` output to a log-trimmer; points at the `@cmd_*` buffer. (`il3-deny-hook.sh` exists on disk but is **not** registered — IL3 is warn-only.)
- `Task` → `pre-task-hint.sh` — on the first subagent dispatch of a session, points at the `reconnaissance` skill.
- `mcp__codescout__edit_code` → `pre-edit-hint.sh` — on the first shape-changing edit of a session, points at recon-for-shape-changes.

**PostToolUse (state sync):**
- `EnterWorktree` → `worktree-activate.sh` — injects workspace guidance, drops the `.cs-worktree-pending` write-block marker, symlinks `.codescout/` into the worktree.
- `mcp__.*__workspace` → `cs-activate-project.sh` — records the declared workspace (statusline) and removes `.cs-worktree-pending` (unblocks write tools).

**Stop:**
- `goal-stop-hook.sh` — queries codescout goal-tracker artifacts at turn end and surfaces refresh-staleness in the stop reason; fail-open; disable via `.claude/codescout-companion.json {"goal_stop_hook": false}`.

## Critical implication for working on this codebase

The `PreToolUse` hook will **block** any attempt to use native `Read`, `Grep`, or `Glob` on source files (`.rs`, `.ts`, `.py`, etc) and **all native `Bash`**. You will see a `PreToolUse` hook deny. **Use codescout's MCP tools instead:**

- `symbols(path)` — all symbols in a file/dir
- `symbols(name=..., include_body=true)` — read a function body
- `grep(pattern)` — regex search
- `semantic_search(query)` — concept-level search
- `read_file(path)` — for non-source files (toml, json); `read_markdown(path)` for `.md`
- `run_command(command)` — shell, cwd sandboxed to the active project

## Cross-repo work (companion: hardened 2026-05-21)

The Bash branch of `pre-tool-guard.sh` no longer allows a `cd`-escape. **All native `Bash` is hard-denied and redirected to `run_command`**, whose cwd is sandboxed to the active project. For a sibling repo's git, run from the project root via `run_command(command="git -C /abs/path <subcommand>")` — no `cd` needed. For non-git work in a sibling (or out-of-shape commands like `pushd` / `bash -c '...'`), switch the codescout workspace explicitly:

```
workspace(action="activate", path="/path/to/sibling", read_only=false)
# ...do the work...
workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)
```

## Concurrent multi-workspace: one server, one active project

The codescout MCP server holds a single active project at a time. Parallel subagents
operating on different workspaces must pin each tool call with `workspace=<abs path>`
rather than calling `workspace(activate)` (which would race the global active-project
state). After any `workspace(activate, path=foreign)`, restore the home project before
finishing the turn. Full rules: `get_guide("workspace-state")`.
