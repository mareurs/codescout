---
id: '34d91e20a6896e28'
kind: bug
status: open
title: Managed worktree inventory retains an attachment whose directory is missing
tags:
- cluster/unclassified
- harness
opened: 2026-11-06
severity: low
---

# BUG: Managed inventory retains a missing worktree path

## Class fit

Unclassified harness metadata drift: a returned attachment path is no longer a usable checkout. Unlike code or Git corruption, this observation concerns inventory freshness only; the cause and intended archived-attachment semantics remain unverified.

## Observed evidence

mcp codex_tui list_worktrees returned task attachment01a1673f-2f50-7f80-a9d7-8c66fbfb8d0d with root/home/marius/.codex/worktrees/3940/codescout. Read-only git -C at that exact root failed128: No such file or directory. Inventory is attachment metadata, not proof that a worktree exists or is suitable to reuse.

## Scope

No cleanup or attachment mutation attempted. Could be a stale historical attachment after earlier operator cleanup, not proven filesystem data loss. Do not infer occupancy or suitability from this row.

## Workaround

Verify actual filesystem/git state, preserve qualified worktrees and create the adapter through the native tool from its required qualified ref.


## Resume

**Triage on 2026-11-07: the inventory is not produced by this repository.**
- `grep` for `list_worktrees` and `codex_tui` finds no source in this repo. The only hit is this file. The tool belongs to the Codex TUI harness.
- `git worktree list` here does not list `3940`. The directory `/home/marius/.codex/worktrees/3940` exists, but `/home/marius/.codex/worktrees/3940/codescout` does not (`ls` says: No such file or directory). So git does not carry the row. The stale row comes from the harness's own attachment metadata.
- Not run: the tool itself, and any Codex source. Why the harness keeps an attachment after its checkout is gone was not checked.

No change is possible in this repository. The report belongs to the Codex TUI maintainers, or the status may become `wontfix` here with that reason. Until then keep the workaround above.
