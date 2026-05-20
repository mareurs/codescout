---
status: open
opened: 2026-05-20
closed:
severity: low
owner: marius
related: []
tags: [tool-friction, sandbox, companion-hook]
kind: bug
---

# BUG: Cross-repo git ops require workspace switch — sandbox + Bash-block sandwich

## Summary

When working in codescout (or any project where the companion plugin is active) and the agent needs to perform git operations in a *sibling* repo (e.g. `~/work/claude/claude-plugins/`), both paths to the shell are closed by default. The agent has to either workspace-switch (polluting shared MCP state) or escalate to the user. Documented nowhere; rediscovered per session.

## Symptom (Effect)

Two consecutive blocks when trying to commit in `~/work/claude/claude-plugins/` while the active codescout workspace is `~/work/claude/code-explorer`:

1. Native `Bash` call refused by companion hook:

   ```
   This call is blocked because codescout offers a leaner path for shell work.
   Command: cd /home/marius/work/claude/claude-plugins && git status --short ...
   ```

2. codescout `run_command` with `cwd` parameter refused:

   ```
   {"ok": false, "error": "cwd '/home/marius/work/claude/claude-plugins' escapes project root",
    "hint": "The cwd must be a subdirectory within the project, or a path under the platform temp directory."}
   ```

Net: no shell path available for the sibling repo without a workspace switch.

## Reproduction

```
Active codescout workspace: ~/work/claude/code-explorer
Edit a file under ~/work/claude/claude-plugins/ via Edit tool.
Try to commit:
  - Bash("cd ~/work/claude/claude-plugins && git ...") → blocked by companion hook
  - run_command(command="git ...", cwd="/home/marius/work/claude/claude-plugins") → blocked by sandbox
```

## Environment

- Host: linux, bash shell
- Active workspace at time of friction: codescout (`/home/marius/work/claude/code-explorer`)
- Companion plugin: `codescout-companion@sdd-misc-plugins` enabled
- Companion hook config: default (Bash blocked, source-file Read/Grep/Glob blocked)
- Date: 2026-05-20

## Root cause

Two independent guard layers compose into a closed system:

1. **Companion `PreToolUse` hook on `Bash`** (`~/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`) — denies all `Bash` calls, redirects to codescout `run_command`. Project-agnostic deny; does not check whether the target path is inside the codescout project.
2. **codescout `run_command` sandbox** (cwd-escape check in `src/tools/run_command/`) — rejects any `cwd` outside the active project root, which is the right safety property for the tool but leaves no escape hatch for legitimate sibling-repo work.

Result: the only path to a sibling-repo shell is `workspace(action="activate", path=...)` to switch projects — which violates Iron Law 4 unless the agent remembers to restore, and pollutes shared MCP state for any concurrent session.

## Evidence

### codescout sandbox refusal

Direct tool output, this session 2026-05-20:

```
run_command(command="git status --short", cwd="/home/marius/work/claude/claude-plugins")
→ {"ok": false,
   "error": "cwd '/home/marius/work/claude/claude-plugins' escapes project root",
   "hint": "The cwd must be a subdirectory within the project, or a path under the platform temp directory."}
```

### Companion hook refusal

```
Bash("cd /home/marius/work/claude/claude-plugins && git status --short && ...")
→ "This call is blocked because codescout offers a leaner path for shell work."
```

## Hypotheses tried

None — friction is observable and reproducible on demand; no investigation required to confirm it.

## Fix

Plan-stage. Three viable directions:

1. **Companion hook becomes path-aware.** Allow `Bash` through when the command's `cd` target is a known sibling project (configured list, or detected via `claude mcp list` projects). Lowest blast radius; fix lives in the hook.
2. **codescout `run_command` adds an `external_cwd` mode.** Explicit opt-in, separate parameter from `cwd`, only honored for known workspace projects, audit-logged. Keeps the sandbox safe by default and adds a documented escape hatch.
3. **Document the workspace-switch dance.** Lowest-cost: add a section to `CLAUDE.md` and the companion plugin's docs explaining when the workspace switch is the intended path. No code change.

Recommend option 3 immediately, options 1 or 2 as a follow-up if the friction recurs across sessions.

## Tests added

N/A — no fix shipped yet.

## Workarounds

Pick the right shape for the situation:

- **One-off commit in sibling repo:** ask the user to run the command, or use the `!` prefix to suggest the user run it inline.
- **Multi-step work in sibling repo:** `workspace(action="activate", path="/path/to/sibling")` then restore at end of turn with `workspace(action="activate", path="/home/marius/work/claude/code-explorer")`. Per Iron Law 4, restore is mandatory.
- **Reading docs in sibling repo:** Edit/Read on `.md` files works (companion hook only blocks source-file reads); no shell needed.

## Resume

If/when this friction recurs in a third session, escalate to option 1 or 2:

- Inspect `~/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` to scope where path-aware allowlist would slot in.
- For option 2, the codescout source is `src/tools/run_command/` — read the cwd-escape check and consider an `external_cwd` parameter gated by an allowlist.

This session opted for workspace activation + restore as a one-off.

## References

- `~/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` — companion Bash deny hook.
- `src/tools/run_command/` — codescout sandbox enforcement.
- CLAUDE.md → "Companion Plugin: codescout-companion" — current documentation of the hook (does not mention cross-repo).
- CLAUDE.md → Iron Law 4 (workspace restore discipline).
