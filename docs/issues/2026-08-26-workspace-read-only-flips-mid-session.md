---
id: c752708c2757e139
kind: bug
status: open
title: Workspace read_only flipped to true twice mid-session with no activate call from this session, silently blocking every write
tags:
- workspace
- read-only
- concurrency
- multi-session
- tool-quirk
opened: 2026-08-26
owner: marius
related:
- '4574d18db7aacec8'
severity: medium
unverified: 'The cause is NOT established. I did not determine who issued the activation: a peer session, one of my own subagents, or the server itself. I did not check whether `read_only` is process-global or per-session in the current build, nor whether a subagent''s `workspace=` pin can flip it. The peer-session hypothesis rests on a live peer plus one commit of theirs on the same branch, not on any log tying an activate call to it.'
---

## Summary

Twice in one long session the active project (`codescout`) went `read_only`, blocking every
write path, **without this session issuing any `workspace(activate)` call between the last
successful write and the failure.** Both times the remedy was
`workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)`,
which succeeded immediately and restored writes.

The failure is loud at the call site but silent as a state change: nothing announces the
flip, so the first symptom is an unrelated-looking write refusal.

## Symptom (Effect)

```
File writes are disabled for this project. If this project was activated in
read-only mode, call workspace(action='activate', read_only: false) to enable writes.
```

The message is accurate and its hint is the correct remedy. The problem is not the message —
it is that the state changed underneath a session that never asked for it.

## Reproduction

Not reproduced on demand. Two observations from session `b02898c3` on 2026-08-26:

1. **Occurrence 1** — `create_file` to an absolute path in the session scratchpad
   (*outside* the project root). Immediately followed the completion of a background
   implementer subagent. Many `edit_markdown`, `artifact(update)` and `create_file` writes
   had succeeded earlier in the same session.
2. **Occurrence 2** — `edit_markdown` on `.superpowers/sdd/…/progress.md`, immediately after
   dispatching another implementer subagent. Between the two occurrences this session had
   completed several successful writes and two git commits.

In both cases the very next call — `workspace(activate, read_only=false)` — returned
`"read_only": false` and the retried write succeeded unchanged.

## Environment

- codescout MCP server shared across three Claude Code profiles (`~/.claude`,
  `~/.claude-sdd`, `~/.claude-kat`).
- **A peer session was live on this machine throughout** (`lang-pal-engine-3a`), and
  committed `20d5d43f` to codescout's `experiments` branch during this session — so it was
  not merely open, it was actively working in the same repo.
- This session had background subagents running against a *different* repo
  (`prompt-engineering`), pinned via the `workspace=` parameter rather than by activation.

## Hypothesis (NOT established)

The nearest prior art is `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md`
(`4574d18db7aacec8`, **fixed**): *"shared codescout server has one process-global active
project — concurrent activations silently cross-contaminate reads."* That fix addressed the
project **identity** being process-global. The plausible reading here is that the
`read_only` **flag** shares the same scope, so a peer session activating codescout read-only
flips it for every session on the process.

This is a hypothesis with a motive and an opportunity, and no evidence tying an activate call
to any particular caller. It should be checked, not assumed — see `unverified:`.

## Hypotheses tried

- **"A subagent did it."** Plausible on timing — both occurrences sit next to subagent
  lifecycle events. But my subagents were told to pin with `workspace=`, not to activate, and
  they were operating on `prompt-engineering`, not codescout. Neither confirmed nor excluded.
- **"The scratchpad path is outside the project, so the guard is right."** Refuted by
  occurrence 2: `.superpowers/…` is inside the project root and was refused identically.

## Impact

Medium. Every write path refuses, and the refusal names a cause (`if this project was
activated in read-only mode`) that reads as speculative rather than diagnostic — so the
natural response is to doubt the path or the tool rather than the workspace state. The
recovery is one call and is stated in the hint, which is why this is not high severity. The
cost is a wasted call plus the risk that an agent mid-task interprets it as a permissions
problem and works around it, or a subagent silently gives up on a write it was asked to make.

## Fix

Not attempted. Two things worth checking before any change:

1. Whether `read_only` is per-session or process-global in the current build. If global,
   this is the unfinished half of `4574d18db7aacec8`.
2. Whether the state change can be made **legible** regardless of scope — an activation that
   flips `read_only` for a session that did not request it should be visible in that
   session's next call, not discovered by a write refusal.

## Workarounds

`workspace(action="activate", path="<project root>", read_only=false)` restores writes
immediately. Cheap, and safe to issue whenever the refusal appears.

## References

- `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md` — the fixed
  process-global active-project race; same architecture, different field.
- `prompt-surface-measurement-session-log:F-3` — *"A subagent's `workspace(activate)` mutated
  the parent's active project"*, still `open`. Same surface, and the reason "a subagent did
  it" could not be dismissed out of hand here.

