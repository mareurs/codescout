---
id: '03dbcc2de5bec43d'
kind: bug
status: open
title: Companion hook state written into a subdirectory is untracked but not ignored, so `git add -A` commits a peer session's live state
owners:
- marius
tags:
- git
- gitignore
- companion-hooks
- shared-checkout
- multi-session
---

## Summary

Companion-hook state written into a **subdirectory** of the repo is neither tracked
nor ignored, so `git add -A` stages another session's live scratch state.

Observed 2026-08-31 during the `cluster/` archive backfill. `docs/issues/` acquired
`.buddy/` and `.codescout/` at 22:35:53, holding session `3a6d634e-…` — not this
session's id, and created before this session's first CLI write at 22:47, so the
writer was a peer session in the same checkout.

Nine files, all reachable by `git add -A`:

```
docs/issues/.buddy/.current_session_id
docs/issues/.buddy/3a6d634e-…/{active_plan.json,cs_tool_log.jsonl,loaded_skills.json,narrative.jsonl,state.json}
docs/issues/.buddy/by-ppid/983773/{session_id,started_at}
docs/issues/.codescout/constitution-seen/3a6d634e-….json
```

## The mechanism, and why the .gitignore looks like it covers this

`.gitignore` already lists both directories. It does not help, and the reason is a
gitignore rule rather than a missing entry: **a pattern containing a `/` anywhere but
at the end is anchored to the directory of the `.gitignore` itself.** So

- `/.buddy/*` (`.gitignore:43`) — explicitly root-anchored.
- `.codescout/constitution-seen/` (`.gitignore:51`) — *looks* unanchored, and is not.
  The mid-string `/` anchors it to the repo root.

Verified at the bytes rather than read off the patterns, because the second one reads
as unanchored:

```
git check-ignore -q docs/issues/.codescout/constitution-seen/3a6d634e-….json  → NOT IGNORED
git add -An docs/issues/.buddy docs/issues/.codescout                          → lists all 9
```

Entries that end in a bare name (`.codescout/write.lock` → no; `write.lock` → yes) are
the only ones that would match at depth, and none of the state dirs use that form.

## Impact

Low severity, high nuisance, and it compounds an already-filed defect. A session
running `git add -A` — the ordinary way to stage a docs change — commits a peer's
in-flight session log, tool log and plan state under an unrelated message. That is the
same outcome as `2026-08-31-peer-commit-captures-another-sessions-working-tree.md`,
reached by a second route: there the captured file was a peer's *work product*, here it
is a peer's *runtime state*, which no reviewer would recognise as out of place.

Nothing corrupts, and the state is regenerable. The cost is a polluted history and
session ids of other agents landing in the repo.

## Not this bug

Why the hook writes to `docs/issues/` at all — presumably a CWD-relative resolution —
is unexamined here. This entry is about the ignore rules not covering the result. If
the hook should never write outside the repo root, that is the better fix and a
different file.

## Suggested fix

Add depth-matching entries (`**/.buddy/`, `**/.codescout/constitution-seen/`, or bare
directory names) so nested copies are ignored wherever they appear. One line each; no
code change.

## Resume

Decide whether to fix the ignore rules, the hook's write path, or both. Until then,
stage explicitly (`git add -u <path>`) rather than `git add -A` in this checkout.

