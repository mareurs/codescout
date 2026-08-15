---
id: '320b97eb87548663'
kind: bug
status: open
title: 'BUG: the worktree-divergence guard covers writes but not reads, so a read after a worktree switch silently resolves against the main checkout'
owners:
- marius
tags:
- worktree
- workspace-activation
- guards
- silent-wrong-answer
- agent-agnostic
---

## Summary

Split out of
`docs/issues/archive/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md`
(half 1) on 2026-08-15, per that file's own § Resume. Half 2 of the parent shipped;
this half and the memory/topology half are independent work and were blocking the
parent's archival.

`guard_worktree_write` (`src/tools/core/guards.rs:14`) blocks **writes** when git
worktrees exist and no project has been explicitly activated. Reads have no
equivalent, so after a session switches into a linked worktree, `symbols`, `grep`,
`read_file` and friends keep resolving against the **main checkout** — and say
nothing about it.

The failure mode is a *silent wrong answer*, which is the expensive kind: the
agent reads plausible code from the wrong tree and acts on it. A refusal would be
loud and cheap by comparison.

## Symptom (Effect)

A session enters a linked worktree. Native tools operate on the worktree. codescout
reads the main checkout. Nothing in any read response distinguishes the two, so
symbol bodies, grep hits and line numbers all describe a tree the agent is not
editing.

Writes are safe — `guard_worktree_write` refuses them with a good message naming
the worktrees and the `workspace(action='activate')` call to make. That asymmetry
is the bug: the guard proves the condition is *detectable*, and then only spends
the detection on half the surface.

## Root cause

`guard_worktree_write` fires on exactly two facts, both cheap:

1. `ctx.agent.is_project_explicitly_activated()` is false — the project was set
   implicitly at startup, not chosen.
2. `list_git_worktrees(&root)` is non-empty — there is more than one plausible
   tree.

Both are available on every read path. Nothing consumes them there.

## Fix

**Not implemented — deliberately, and the reasoning is the useful part.**

The obvious symmetry (make reads refuse too) is the wrong fix. Every read in a
worktree-bearing repo would fail until `workspace(action='activate')` is called,
including the reads an agent makes *while orienting* — and orientation is exactly
when it does not yet know which tree it wants. A guard that fires before the
caller can plausibly satisfy it trains callers to route around it.

The right shape is a **notice, not a refusal**: when the two facts above hold,
attach a one-shot per-session field to the first read response naming the root
that reads are resolving against and the `workspace(action='activate')` call that
would pin it. Reads keep working; the silence ends. Same philosophy as
`removed_attributes` / `removed_descendants` — the operation is allowed, and it
says what it did.

**Agent-agnostic by construction.** Both facts are git/filesystem observations.
Nothing here learns any specific harness's worktree tool by name, which is the
constraint that shaped the parent bug's whole design.

**The implementation cost is one-shot state, and that is what deferred it.** A
per-session "already told them" flag has to live on `ToolContext`, which is
constructed at many sites (tests included), so adding a field is a wide mechanical
change. `guide_hints_emitted` is an existing per-session emitted-once ledger with
the right semantics and could carry a sentinel key instead — cheaper, slightly
abusive of that field's meaning. **Pick one deliberately; do not add a second
one-shot mechanism.**

## Tests added

None yet. When implemented, the discriminating pair is:

- worktrees exist + project NOT explicitly activated → first read carries the
  notice, second read does not (one-shot, or it becomes noise on every call —
  the failure mode `removed_attributes` was designed around);
- worktrees exist + project explicitly activated → no notice at all, since the
  caller has already made the choice the notice would ask for.

## Workarounds

Call `workspace(action='activate', path=...)` after switching trees. That is what
the write guard's message already tells you to do — the gap is only that nothing
tells you when you are reading.

## Resume

Open. Independent of the memory/sub-project topology half, which is filed
separately.

Do not re-derive the detection: `guard_worktree_write` already has it, tested and
shipped. The open question is only *where the notice attaches* and *which one-shot
mechanism carries it*.

