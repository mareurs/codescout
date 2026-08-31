---
id: c611a3dce4f05d45
kind: bug
status: fixed
title: 'BUG: the worktree-divergence guard covers writes but not reads, so a read after a worktree switch silently resolves against the main checkout'
owners:
- marius
tags:
- worktree
- workspace-activation
- guards
- silent-wrong-answer
- agent-agnostic
- cluster/guard-narrower-than-its-name
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

**Implemented 2026-08-16, as a notice — the shape this file argued for.** When
the two facts hold, the first read carries a `_workspace_notice` field naming
the root reads resolved against, the linked worktrees that also exist, and the
`workspace(action='activate')` call that would pin one. Reads keep working; the
silence ends.

**Both blocking dilemmas this file recorded dissolved on contact with the code.**

### The one-shot mechanism: neither of the two options

This file framed a choice between a new `ToolContext` field and a sentinel key
in `guide_hints_emitted`. Both were wrong, and the measurement corrected the
file in both directions:

- **The `ToolContext` cost was understated by 3.5×.** Not 38 sites — a
  field-specific grep on `guide_hints_emitted:` returns 134 matches across 29
  files, one of them the declaration, so **133 construction sites**.
- **The sentinel was not a semantic compromise but a live regression.**
  `src/tools/core/types.rs:626` reads `if emitted.is_empty()` — that set being
  empty **is** the session-opening guide's trigger. A sentinel inserted there
  makes it false, suppressing `SESSION_OPENING_GUIDE` for exactly the sessions
  a notice fires in, since notices fire on the first eligible call. It would
  have silently undone the guide-delivery fix that shipped hours earlier.

The actual answer was a **third option neither considered**: a separate
`notices` set on `GuideLedger` itself. `GuideLedger` derives `Default`, so all
133 `ToolContext` literals are untouched; the only construction site that
changes is `load`, inside the ledger's own module. It never enters the topic
namespace, never affects `is_empty()`, and is not persisted — so the on-disk
JSON stays a bare `Vec<String>` with no migration. `clear()` clears it too:
an `activate` is precisely the act the notice asks for, and a post-compact
re-arm means the model no longer remembers being told.

### The predicate: this file's premise was wrong

This file said the write guard *"proves the condition is detectable, and then
only spends the detection on half the surface."* It is spent on **neither**
half. `guard_worktree_write` returns `Ok(())` on its first line in every real
session, because `is_project_explicitly_activated` is set at **startup** from
`run_server`'s `current_dir()` fallback (`src/server.rs:1429` →
`src/agent/mod.rs:480`). Measured, not read — a probe in a fixture with a
seeded linked worktree returned `explicitly_activated=true` with the root
resolved and the worktree found.

So the notice gates on a new, narrower predicate,
`Agent::is_project_chosen_this_session()` — false at startup however the root
was found, true only after an in-session `activate`.

The write guard was **left dormant on purpose**: re-arming a refusal path is a
behaviour change, not a bug fix, and this repo has two live worktrees, so it
would start refusing writes here immediately. Filed separately as
`docs/issues/2026-08-16-worktree-write-guard-is-dead-code-in-production.md`,
where it is a decision for the maintainer rather than a side effect of this one.

Agent-agnostic as required: both facts remain git/filesystem observations.
## Tests added

Three, in `src/tools/core/tests.rs` — the discriminating pair this file
specified, plus the common-case guard:

- `a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen` —
  worktrees exist, project not chosen: the first read carries the notice, names
  the worktree, and names a runnable `workspace(action='activate')` call; the
  second read does not. Mutation-verified: forcing `notice_once` to always
  return `true` fails the one-shot assertion.
- `an_explicitly_activated_project_gets_no_worktree_notice` — after `activate`,
  no notice at all.
- `a_repo_without_worktrees_gets_no_notice` — the overwhelmingly common case
  pays nothing.

`list_git_worktrees` short-circuits on a single `is_dir()` stat when no
`.git/worktrees` exists, so the per-call cost in an ordinary checkout is one
syscall until the notice fires or the session ends.
## Workarounds

Call `workspace(action='activate', path=...)` after switching trees. That is what
the write guard's message already tells you to do — the gap is only that nothing
tells you when you are reading.

## Resume

Closed. Gate green on `experiments` (3782 lib tests, clippy `-D warnings`
clean).

One open thread, deliberately not closed here: the write guard this file was
named after is dead code, and re-arming it is a decision —
`docs/issues/2026-08-16-worktree-write-guard-is-dead-code-in-production.md`.

The lesson worth keeping is about this file rather than the code. It recorded
two blocking dilemmas in good faith, and **both were artefacts of not having
read the consumer**. The 38-site figure was never counted; the sentinel's real
cost was one `is_empty()` call away; the "guard proves it's detectable" premise
held only until someone ran it. A cost estimate that defers work is a claim
like any other, and it deserves the same verification as the bug it defers.
