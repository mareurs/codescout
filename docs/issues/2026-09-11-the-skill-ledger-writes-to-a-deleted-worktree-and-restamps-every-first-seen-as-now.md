---
id: '270b230c44ed8029'
kind: bug
status: open
title: The skill ledger writes to a deleted worktree and re-stamps every first-seen as now
owners:
- marius
tags:
- cluster/unclassified
topic: hook state resolution and worktree residue
opened: 2026-09-11
owner: marius
severity: low
---

## Summary

The buddy skill-ledger hook derives its output directory from the hook event's `cwd`
field, while the statusline that reads the same directory derives it from
`workspace.current_dir` with `cwd` only as a fallback. When those two disagree — as they
do for a session whose worktree has been deleted — one session ends up with **two**
`.buddy/<sid>/` directories under different roots. The second is created fresh, so the
ledger it writes re-stamps every skill's `first_ts` as the moment of creation: a
complete, plausible, wrong record rather than an error. As a side effect the deleted
worktree directory is recreated by `mkdir -p`, so removing a worktree does not stay
removed while the session that used it is alive.

## Symptom (Effect)

Deleting `.worktrees/doctor-per-project-isolation` succeeded; the directory was back
about ninety seconds later, holding only a freshly written skill ledger:

```
2026-09-11+12:38:12.1092534690  .worktrees/doctor-per-project-isolation
2026-09-11+12:38:12.1092534690  .worktrees/doctor-per-project-isolation/.buddy
2026-09-11+12:38:12.1093657410  .worktrees/doctor-per-project-isolation/.buddy/b80a27d4-9729-40ef-8c28-ad8982df6d13
2026-09-11+12:38:12.1093657410  .worktrees/doctor-per-project-isolation/.buddy/b80a27d4-9729-40ef-8c28-ad8982df6d13/loaded_skills.json
```

No error was emitted on either side. The write succeeded; the removal succeeded.

## Reproduction

Measured 2026-09-11 at `git rev-parse HEAD` = `aa39c1e0` on `experiments`.

1. Start a session whose working directory is a git worktree under `.worktrees/<name>/`.
2. Remove that worktree (`git worktree remove`, or unregister it and delete the directory).
3. Keep the session alive and send it another prompt.
4. `find .worktrees/<name>` — the directory is back, containing
   `.buddy/<sid>/loaded_skills.json` and nothing else.
5. Compare that file against `.buddy/<sid>/loaded_skills.json` under the main checkout:
   the resurrected copy has every `first_ts` equal to the instant of step 3.

Observed twice in one session, at 12:36 and again after the 12:39 sweep.

## Environment

Linux, codescout checkout at `/home/marius/work/claude/codescout`, branch `experiments`.
Claude Code profile `~/.claude-sdd`, session `b80a27d4-9729-40ef-8c28-ad8982df6d13`,
harness registry name `codescout-87`. The buddy plugin lives in the sibling
`claude-plugins` repo.

## Root cause

Two components of one plugin derive the same logical directory by different keys.

The writer takes `cwd` straight from the event and creates parents under it:

```
claude-plugins/buddy/scripts/skill_ledger.py:40    project_root / ".buddy" / session_id / LEDGER_FILENAME
claude-plugins/buddy/scripts/skill_ledger.py:181   cwd = event.get("cwd")
claude-plugins/buddy/scripts/skill_ledger.py:187   ledger_path(Path(cwd), sid)
```

The reader prefers a different field, falling back to `cwd` only if it is absent:

```
claude-plugins/buddy/scripts/statusline.py:268     cwd = (data.get("workspace") or {}).get("current_dir") or data.get("cwd")
claude-plugins/buddy/scripts/statusline.py:269     project_root = Path(cwd) if cwd else None
```

Two independent facts then compound. First, the two keys can name different directories,
so writer and reader address different files. Second, `load_ledger` treats a missing file
as "this session has loaded no skills yet" — which is indistinguishable from "the file I
looked for is not where I looked" — so the fresh ledger fabricates `first_ts` for every
skill it then observes in the transcript.

Measured 2026-09-11: the harness session registry
(`$CLAUDE_CONFIG_DIR/sessions/<pid>.json`) records `cwd` as the main checkout
`/home/marius/work/claude/codescout`, while the file actually landed under the worktree
path — so the event payload's `cwd` and the registry's `cwd` are not the same value
either. Not measured: which harness event field carried the worktree path, because the
hook payload is not retained anywhere this session can read.

## Evidence

### Two live ledgers for one session id, diverging

Main checkout, `.buddy/b80a27d4-9729-40ef-8c28-ad8982df6d13/loaded_skills.json`,
last written 2026-09-09 09:10 — five skills with five distinct first-seen stamps:

```
"transcript_offset": 5010132,
"codescout-companion:reconnaissance":        { "first_ts": 1788924404, "count": 1 }
"superpowers:writing-plans":                 { "first_ts": 1788925414, "count": 1 }
"codescout-companion:reaching-peer-sessions":{ "first_ts": 1788926290, "count": 1 }
"superpowers:subagent-driven-development":   { "first_ts": 1788928920, "count": 1 }
"superpowers:using-git-worktrees":           { "first_ts": 1788929164, "count": 1 }
```

Resurrected worktree path, same session id, written 2026-09-11 12:38:12 — the same five
skills, every stamp identical and equal to the write instant (1789119492):

```
"transcript_offset": 22183479,
"codescout-companion:reconnaissance":        { "first_ts": 1789119492, "count": 1 }
"superpowers:writing-plans":                 { "first_ts": 1789119492, "count": 1 }
"codescout-companion:reaching-peer-sessions":{ "first_ts": 1789119492, "count": 1 }
"superpowers:subagent-driven-development":   { "first_ts": 1789119492, "count": 1 }
"superpowers:using-git-worktrees":           { "first_ts": 1789119492, "count": 1 }
```

The collapse of five distinct values onto one is the tell. A ledger that had merely been
*moved* would carry the original stamps.

### The rest of the session's buddy state went to the other root

At 12:40:15 — ninety seconds after the resurrection — `state.json`, `narrative.jsonl`
and `cs_tool_log.jsonl` were all written under the **main checkout's**
`.buddy/<sid>/`, while `loaded_skills.json` there still carried its 2026-09-09
timestamp. So the split is live and ongoing, not a one-off leftover.

### No process held the deleted path

Checked before removal: zero processes had any `.worktrees/` path as `cwd`. The
recreation is not a running process writing through a held inode; it is `mkdir -p`
against a path string.

## Hypotheses tried

1. **Hypothesis:** the directory survives because a process holds it as `cwd`.
   **Test:** walked `/proc/*/cwd` for every pid.
   **Verdict:** rejected for the recreation — but *confirmed for a separate finding*: an
   orphaned monitor (pid 435739, parent `systemd`, started 2026-09-10 10:48) held the
   original inode as `cwd=... (deleted)`. Killing it released the inode; the directory
   still came back afterwards, which is what separated the two mechanisms.

2. **Hypothesis:** the harness records the worktree as the session's cwd, and hooks
   inherit that.
   **Test:** read `sessionId`/`cwd` from the profile's `sessions/872862.json`.
   **Verdict:** rejected — the registry records the **main checkout**. This is what
   forced the search to the hook payload rather than the registry, and is why the two
   resolution rules in the plugin came into view at all.

3. **Hypothesis:** writer and reader agree on the directory, so the split must be
   temporal.
   **Test:** read the path expressions at both sites.
   **Verdict:** rejected — they read different keys (`cwd` vs
   `workspace.current_dir or cwd`).

## Fix

Not attempted. The change belongs in the `claude-plugins` repo, not this one, and the
right shape is a judgement call this bug does not make for the owner: either both sites
resolve through one shared helper, or the ledger writer refuses to create a directory
that does not already exist — the latter also removes the resurrection half, since a
`mkdir -p` against a deleted tree is never the intended write.

The fabricated-`first_ts` half wants its own answer regardless of which path rule wins:
`load_ledger` cannot currently distinguish "no skills loaded yet" from "wrong directory",
and both produce the same empty dict.

## Tests added

None — no fix was made, so there is nothing to regress-test yet. The reproduction in this
file is deterministic and needs no fixture beyond a deleted worktree and a live session.

## Workarounds

Delete the resurrected directory after the session that owns it has exited; while that
session is alive, every prompt it receives may recreate it. The content is ~4 KB and the
path is gitignored, so leaving it in place costs nothing but a misleading directory
listing for anyone auditing worktree residue.

For the statusline badge: nothing. The skill first-seen times it shows for an affected
session are wrong and there is no user-facing way to correct them.

## Resume

Decide in the `claude-plugins` repo whether the ledger writer should refuse a
non-existent directory or share a resolver with the statusline, then make `load_ledger`
distinguish absent-file from empty-ledger so a mislocated read cannot silently re-stamp
`first_ts`. The two path expressions to reconcile are cited under **Root cause** above.

## References

- Archived sibling, same family of worktree residue but a different mechanism (a closure
  recorded a removal that never happened):
  `docs/issues/archive/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md`
- `docs/issues/archive/2026-08-31-nested-hook-state-dirs-are-untracked-but-not-ignored.md`
- Cluster rejected on inspection: `IC-12` (`transient-shared-state-lies-to-readers`)
  needs state that is shared between readers and transient; this is one session's own
  state split across two roots by two resolution rules, neither shared nor transient.
  `IC-15` (`accepted-parameter-silently-dropped`) — nothing is dropped. `IC-8`
  (`record-asserts-an-unchecked-completion`) — the false value is a first-seen timestamp,
  not an assertion of completion. Filed under the `cluster/unclassified` escape hatch,
  which drives no promotion threshold, rather than forcing a member into a class whose
  count would then be wrong.
