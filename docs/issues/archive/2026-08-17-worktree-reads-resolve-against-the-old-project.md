---
id: 3650e74d5331221e
kind: bug
status: mitigated
title: 'BUG: after EnterWorktree, MCP writes are blocked until activate but reads are not — git reconnaissance silently answers about the old checkout, and the notice that says so sits beside the answer'
tags:
- worktree
- workspace-state
- run-command
- guard
- silent-wrong-answer
- companion-plugin
opened: 2026-08-17
owner: marius
related:
- docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md
- docs/trackers/codescout-usage-frictions.md
severity: medium
---

## Summary

`EnterWorktree` moves the session's cwd into a new worktree and its post-hook blocks MCP
**write** tools until `workspace(action="activate")` runs. It does not block **reads**. In
that window `run_command` still resolves against the previously-active project, so git
reconnaissance run from inside the worktree reports the *main* checkout's branch, HEAD and
divergence — with no failure of any kind.

The response does carry a `_workspace_notice` saying precisely this. It sits in a sibling
field next to a confident, well-formed answer, and the answer wins attention.

Cost measured twice in one session: both times the conclusion "this worktree is based on
`experiments`, no reset needed" was drawn from the main checkout's state, while the worktree
was actually on `origin/master`, **1091 commits behind**.

## Symptom (Effect)

Run immediately after `EnterWorktree`, before `workspace(activate)`:

```
run_command("git log -1 --format='%h %s'; git rev-list --left-right --count experiments...HEAD")

→ 3ecb8730 docs(prompts): correct the /mcp-reconnect-refreshes-instructions claim
  0   0

  _workspace_notice: "Reads are resolving against \"/home/marius/work/claude/codescout\".
    This repo also has linked git worktrees [...] and no project has been explicitly
    activated, so results describe the main checkout even if you are working in a
    worktree. Call workspace(action='activate', path=...) to pin the tree you mean."
```

`0 0` reads as "this worktree is exactly `experiments`". The same commands after activating
the worktree:

```
pwd  → /home/marius/work/claude/codescout/.claude/worktrees/source-gate-newline-split
git log -1 --format='%h %s'  → eca9902e docs: workspace-aware release cycle ...
git merge-base --is-ancestor 4fad1aa4 HEAD; echo $?  → 1
wc -l src/util/path_security.rs  → 2199      (the file actually has 3403)
```

## Reproduction

1. In a repo with linked worktrees, `EnterWorktree(name="probe")`.
2. **Before** calling `workspace(action="activate", …)`, run
   `run_command("pwd; git log -1 --format=%h")`.
3. `pwd` reports the worktree (the shell's cwd did move); `git log` reports the *main*
   checkout's HEAD. The two disagree in the same response.
4. `workspace(action="activate", path="<worktree root>", read_only=false)`, then re-run.
   Now both agree.

Step 3 is the whole bug: one command, two different trees, one of them silent about it.

## Environment

codescout MCP server, `run_command`; Claude Code `EnterWorktree`; the
`codescout-companion` `PostToolUse:EnterWorktree` hook. `worktree.baseRef` at its `fresh`
default, which is why the divergence is large enough to notice at all.

## Root cause

Measured 2026-08-17 by the disagreeing-pair above; not traced to a line this pass.

Two independent contributors:

**1. The write guard is armed and the read path is not.** The hook's own text draws the line
explicitly — *"MCP write tools (edit_code, edit_file, edit_markdown, create_file) are
BLOCKED until workspace is called — they would otherwise silently write to the wrong
repo."* The stated reason ("silently … the wrong repo") applies verbatim to a read that
answers about the wrong repo, and reads are left open.

This is the same argument as
`docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md`, which was
closed on the write half. The read half is still open, now with a second measured instance.

**2. The hook names the wrong path.** It instructs
`workspace(action="activate", path="<MAIN root>")` at the moment the session has just moved
*into* a worktree. Following it literally leaves every path resolving to the checkout you
deliberately left. The correct call is the worktree root **plus `read_only=false`**, because
a foreign activation defaults to read-only (`get_guide("workspace-state")` §
*The home/foreign distinction*). I overrode the hook on both occasions.

**Why the existing notice is not sufficient.** It is accurate and well-worded, and it was
present in the very response that misled me. The failure is placement, not content: a
sibling field alongside a plausible answer reads as metadata, while the answer reads as the
result. Compare the write path, which fails loudly and cannot be skipped. The asymmetry is
the defect — the louder hazard is guarded and the quieter one is annotated.

## Evidence

### One response, two trees

`pwd` and `git log` in the same `run_command` disagree: the shell's cwd is the worktree,
because `EnterWorktree` moved it; the resolved project is main, because nothing activated
yet. Nothing in the exit code or the stdout marks the split.

### The detection that actually worked, both times

Not the notice. First occurrence: `wc -l` on the target file returned 2199 where the file
has 3403 — a line count that looked wrong. Second occurrence: `edit_code` returned
`symbol not found` for a test committed hours earlier. Both are accidents of the task, not
checks anyone designed.

## Hypotheses tried

1. **Hypothesis:** `EnterWorktree` had branched from `experiments` the second time, since the
   reported HEAD matched it — the earlier `origin/master` base was a one-off.
   **Test:** `pwd` plus `git log -1` after activating the worktree.
   **Verdict:** rejected. Both worktrees branched from `origin/master`; the second reading
   simply came from the main checkout. The apparent difference between the two sessions was
   an artifact of *when* I ran the check relative to activation, not of `baseRef`.
   **Evidence link:** *One response, two trees*.

2. **Hypothesis:** the notice is missing for this command shape.
   **Test:** re-read the raw response.
   **Verdict:** rejected — it was there, complete and correct. This is what moves the fix
   from "add a warning" to "change where the warning lives, or guard the read".
   **Evidence link:** Symptom, which quotes it verbatim.

## Fix

Not yet implemented. In preference order:

1. **Have `EnterWorktree` activate the worktree itself** — it knows the path, having just
   created it. The manual follow-up step is the only reason the window exists. This is a
   `codescout-companion` hook change (it already fires `PostToolUse` on `EnterWorktree`),
   and it also removes contributor 2 by construction.
2. **Guard reads the way writes are guarded** when linked worktrees exist and no project has
   been chosen this session. `guard_worktree_write` already refuses on exactly this
   ambiguity; extend the same predicate to project-resolving reads, or at least to
   `run_command`, whose answers most often *are* the reconnaissance.
3. **Failing both, move the notice into the answer** — prefix `stdout` rather than adding a
   sibling key, so it cannot be read past.

Fix 2 has a cost worth stating: it makes a read fail where today it merely misleads, which
is more friction for the common single-checkout case. Note that `guard_worktree_write`
already returns `Ok` immediately when no worktrees exist, so that case stays untouched.

**Decided, 2026-08-18: Fix 3, not Fix 2.** Reconnaissance on `worktree_read_notice`
(`src/tools/core/types.rs`) turned up its own doc comment arguing explicitly against
refusing reads: *"Refusing reads would fire while the agent is still orienting — exactly
when it cannot yet know which tree it wants — and a guard that fires before the caller can
plausibly satisfy it trains callers to route around it."* That reasoning is sound and
predates this bug; overriding it would need a stronger argument than "the notice is easy to
miss," which Fix 3 addresses directly without touching the refusal question. Fix 1
(`EnterWorktree` self-activates) remains the most complete fix but lives in a different
repo (`codescout-companion`) — out of scope here, left as the open half.

## Tests added

`a_worktree_notice_is_prefixed_into_stdout_when_the_response_carries_one`
(`src/tools/core/tests.rs`) — an `EchoTool` returning `{"stdout": ...}` through
`call_content` with an unchosen worktree present; asserts the sibling
`_workspace_notice` field still appears AND the `stdout` field is prefixed with the
same notice, verbatim tail preserved. Mutation-tested: replaced the prefix branch
with a no-op, confirmed the test failed with the predicted message, restored the
fix.

Not added: a `run_command`-level integration test. The fix lives entirely in the
shared `inject_notice` helper inside `Tool::call_content`, which is the one place
every tool's response passes through — the existing sibling-field test
(`a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen`) already
exercises that same call path with the same `EchoTool` pattern, so a second test at
the `run_command` shell layer would duplicate coverage of shell execution, not of
this fix.
## Workarounds

**Make `pwd` the first command after entering a worktree, and compare it to where you think
you are.** It is one token of output and the only part of a response that cannot be about
the wrong tree. Then activate explicitly:

```
workspace(action="activate", path="<worktree root>", read_only=false)
```

— the worktree root, not the main root the hook names, and `read_only=false` because a
foreign activation is read-only by default.

Do **not** trust a `git log` / `git rev-list` / `git status` result taken before that
activation, however confident it looks.

## Resume

Decide between Fix 1 (hook auto-activates) and Fix 2 (guard reads). Fix 1 is smaller, lands
in `codescout-companion`, and carries the version-bump trap — three profiles, three
version-keyed caches, so a content-only edit is live in none of them until
`.claude-plugin/plugin.json` is bumped. Fix 2 lands here and closes the read half of
`docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md`, which is the
more durable outcome; the two are complementary rather than alternatives.

## References

- `docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md` — the same
  read/write asymmetry, closed on the write half; this is the read half with a second
  measured instance.
- `docs/trackers/codescout-usage-frictions.md` — U-47 (this friction, caller's side), U-14
  (a companion-plugin worktree matcher citing tools that do not exist).
- `get_guide("workspace-state")` § *The home/foreign distinction* — why `read_only=false` is
  required on the activation.
- `src/tools/core/guards.rs` — `guard_worktree_write`, the precedent predicate.
