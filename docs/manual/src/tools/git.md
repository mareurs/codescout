# Git

> **Note:** The `git_blame` tool was removed in the v1 tool restructure.
> Git history is still fully accessible via `run_command`.

## What to Use Instead

Use `run_command` with standard git commands for all git operations:

```json
{ "tool": "run_command", "arguments": { "command": "git blame src/auth.rs" } }
```

```json
{ "tool": "run_command", "arguments": { "command": "git log --oneline -20 src/auth.rs" } }
```

```json
{ "tool": "run_command", "arguments": { "command": "git diff HEAD~1 src/auth.rs" } }
```

## Why It Was Removed

`git_blame` returned structured JSON but the information was equally accessible through `run_command("git blame ...")`. Keeping a purpose-built tool for one specific git operation — while all other git operations already used `run_command` — was inconsistent. Consolidating to `run_command` gives a single mental model: git history queries go through the shell.

See [Workflow & Config](workflow-and-config.md) for the `run_command` reference.
