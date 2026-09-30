---
id: de44b3e0d02bd11f
kind: bug
status: open
title: The worktree read notice still names list[0] as the tree to activate, so every unpinned call points at a mutation-probe slot
tags:
- cluster/hint-composed-without-the-request
closed: ''
opened: 2026-09-30
owner: marius
related:
- the-worktree-write-block-names-an-arbitrary-worktree-as-the-remedy
severity: medium
---

# BUG: the worktree READ notice still names `list[0]` as the tree to activate, so every unpinned call points at a mutation-probe slot

## Summary

`worktree_read_notice` prepends a notice to every unpinned response while linked worktrees exist and no
project has been chosen. Its remedy tells the caller to
`workspace(action='activate', path="<list[0]>")`. `list[0]` is the first worktree in the order
`list_git_worktrees` returns them, which in this repo is a `mutation-probe.sh` pool slot. The write
guard's copy of this defect was fixed on 2026-09-18 (`fc6f5bb7`, *"the write block's remedy names the
main repo, not a filesystem-ordered tree"*). The read path is the second site of the same prescription,
and it was not repaired.

## Symptom (Effect)

Every unpinned codescout response in session `3c5b02df-b6ce-45f5-9d03-1194e38465c0` after a `/mcp`
reconnect on 2026-09-30 carried this text. For `run_command` it is prepended into `stdout`:

```
Reads are resolving against "/home/marius/work/claude/codescout". This repo also has linked git worktrees
[/home/marius/work/claude/codescout.worktrees/mutation-slot-0, /home/marius/work/claude/codescout.worktrees/mutation-slot-1]
and no project has been explicitly activated, so results describe the main checkout even if you are working
in a worktree. Call workspace(action='activate', path="/home/marius/work/claude/codescout.worktrees/mutation-slot-0")
to pin the tree you mean, or pass workspace="<abs path>" on a single call.
```

The session works in the main checkout. The notice's first prescription would activate a detached-HEAD
probe slot as the session's home project.

## Reproduction

1. Have at least one linked worktree (in this repo the mutation-probe pool keeps them permanently:
   `git worktree list` shows `mutation-slot-0` and `mutation-slot-1`, detached HEAD).
2. `/mcp` reconnect, which clears `project_chosen_this_session`.
3. Make any unpinned read, for example `grep(pattern="x", path="src")`. The response carries the notice
   naming `mutation-slot-0`.

## Environment

`experiments` at `12b7370f`; live MCP binary; Linux.

## Root cause

- `src/tools/core/types.rs:233-241` (`worktree_read_notice`): `format!(… "Call workspace(action='activate',
  path=\"{}\") to pin the tree you mean …", root.display(), list.join(", "), list[0])`. The third argument
  is `list[0]`.
- `fc6f5bb7` changed the same prescription in `guard_worktree_write` to lead with `root`. `e208bc25`
  (2026-09-19) later edited this function for the peer-serve branch and left the interactive branch's
  `list[0]` in place.
- The doc comment's premise that *"the condition is self-limiting"* (it "requires linked worktrees to
  EXIST") no longer holds here. The mutation-probe pool keeps its slots
  (`docs/issues/archive/2026-09-24-mutation-probe-worktrees-are-never-reclaimed.md`), so every session
  without an `activate` sees the notice on every unpinned call. The one remedy that silences it for the
  session is the `activate` the notice mis-targets.

measured 2026-09-30: `git worktree list` → main plus `mutation-slot-0`/`-1` (detached HEAD), and the
notice text above appeared on every unpinned call in this session. The function body was read with
`symbols(name="worktree_read_notice", include_body=true)`.

## Evidence

- `git log -3 -- src/tools/core/types.rs`: `e208bc25 fix(peer): a served read keeps the disclosure and
  loses the remedy it cannot perform` edited this function after `fc6f5bb7` and did not touch `list[0]`.
- Sibling fix: `docs/issues/archive/2026-09-18-the-worktree-write-block-names-an-arbitrary-worktree-as-the-remedy.md`,
  § Fix: "Lead with `root` — the main repo — and offer the worktree list second … name the caller's own
  … or name none."

## Hypotheses tried

1. **Hypothesis:** already fixed by `fc6f5bb7`. **Test:** read `worktree_read_notice` at HEAD.
   **Verdict:** rejected. That commit's subject names only "the write block". The read notice still
   formats `list[0]`.
2. **Hypothesis:** this is `184258b6a22ecfb5` (the notice prescribes calls a served peer cannot make).
   **Test:** read its archived file and the peer branch. **Verdict:** rejected. That bug is the
   peer-serve branch and is fixed. This is the interactive branch.

## Fix

Apply the `fc6f5bb7` remedy here too: name `root` as the tree to activate (it is the answer in nearly
every case), list the worktrees as the alternatives, and keep the `workspace=` per-call form. Better
still, share one helper for the prescription between `guard_worktree_write` and `worktree_read_notice`,
so a third site cannot drift. That is the *mutate once per guarded SITE* law applied to a remedy string.
Also correct the "self-limiting" paragraph of the doc comment. Whether a permanent probe pool should
count as "linked worktrees" for this notice at all is a separate question; raise it rather than fold it
in.

## Tests added

None yet. Owed: with two worktrees present and no project chosen, the notice's `activate` path is the
main root and not either worktree. Assert on the path argument inside `activate(…)` specifically,
because the worktree paths legitimately appear in the list, so a bare `contains(root)` is satisfiable by
accident (IC-9). Mutate `root` back to `list[0]` and confirm the test fails.

## Workarounds

Call `workspace(action='activate', path="/home/marius/work/claude/codescout")` (the main repo, not what
the notice names), or pass `workspace=` per call.

## Resume

Write the failing test beside the existing `worktree_read_notice` tests (find them with
`references(symbol="worktree_read_notice", path="src/tools/core/types.rs")`), then change the format
argument and share the prescription with `guard_worktree_write`.

## References

- `docs/issues/archive/2026-09-18-the-worktree-write-block-names-an-arbitrary-worktree-as-the-remedy.md`
- `docs/issues/archive/2026-09-02-the-worktree-notice-prescribes-two-calls-a-served-peer-cannot-make.md`
- `docs/issues/archive/2026-09-24-mutation-probe-worktrees-are-never-reclaimed.md`
