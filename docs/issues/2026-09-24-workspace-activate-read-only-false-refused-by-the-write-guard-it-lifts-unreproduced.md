---
id: '652abef29e8d8c45'
kind: bug
status: open
title: 'BUG: workspace(activate, read_only=false) refused with the write-guard''s own error, the remedy that error names (unreproduced on retry)'
tags:
- cluster/unclassified
---

# BUG: workspace(activate, read_only=false) refused with the write-guard's own error, the remedy that error names (unreproduced on retry)

## Summary

After a `/mcp` reconnect on 2026-09-24, `create_file` in the home checkout was refused with the `WriteBlockCause::ActivatedReadOnly` message (`src/util/path_security.rs:677`). That message names two remedies: pass `workspace=<abs path>`, or call `workspace(action='activate', path=…, read_only: false)`. **The second remedy was itself refused with the byte-identical write-refusal text.** Minutes later the same call, with the same arguments, succeeded.

## Symptom (Effect)

The refusal points the reader at a call that the same guard refuses. A reader who follows the error's own instruction gets the error again, with nothing to indicate why. This is the "remedy text sends you somewhere useless" failure (`CLAUDE.md` § Testing Discipline, loudness bullet), and here the remedy is not only useless but refused.

## Reproduction

**Not reproduced.** The sequence, all from session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`:

1. `workspace(post_compact=true)` after `/mcp` reconnect to a freshly rebuilt server (`git_sha 396f04c4`, dirty).
2. `create_file(path="scripts/…")` → refused, `ActivatedReadOnly` text naming `/home/marius/work/claude/codescout`.
3. `workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)` → **refused with the same text.**
4. `create_file(…, workspace="/home/marius/work/claude/codescout")` → succeeded (the pin works).
5. About 10 minutes later, the same call as step 3 → `status: ok`, `read_only: false`.

A background research subagent was running on the same MCP server between steps 3 and 5, and may have activated the project itself. That is unverified.

## Environment

codescout server pid 3289089, `git_sha 396f04c4` (dirty), profile `~/.claude-kat`, CLI Claude Code, 2026-09-24.

## Root cause

Unknown. **Lead, not established:** the successful activate response carries `wrote_to` / `abs_path`, so activation runs through the write path. If that path's guard checks the *current* activation's read-only state before applying the new `read_only=false`, then a read-only project guards against the call that lifts it. The success at step 5 argues against a deterministic version of this. It is consistent with a peer having re-activated in between, which changed the state the guard read.

## Evidence

The refusal text returned at step 3 is identical to step 2's, including "To lift the block here instead, call workspace(action='activate', …, read_only: false)".

## Hypotheses tried

None yet: no mutation or code read beyond locating the message.

## Fix

Not started. A fix would need a reproduction first (`CLAUDE.md` § Bug Tracking: run the reproduction before reading the fix plan).

## Tests added

None.

## Workarounds

Pass `workspace=<abs path>` per call. It worked first time and does not flip process-wide state under peers.

## Resume

Try to reproduce with a server whose home project is activated `read_only=true`, then call `workspace(activate, same path, read_only=false)`. Read where the activate handler invokes the write guard, relative to where it applies `read_only`.

## References

- `src/util/path_security.rs:677` (the refusal text)
- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
- `docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`
