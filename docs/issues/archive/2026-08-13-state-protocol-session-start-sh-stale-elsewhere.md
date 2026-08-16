---
id: '2859865900073454'
kind: bug
status: fixed
title: docs/state-protocol.md still names companion's session-start hook `.sh` in four other rows (actual file is `.mjs`)
tags:
- docs
- state-protocol
- drift
closed: 2026-08-14
opened: 2026-08-13
owner: marius
related: []
severity: low
---

# BUG: `docs/state-protocol.md` still names companion's session-start hook `.sh` in four other rows (actual file is `.mjs`)

## Summary

Noticed while fixing the `index-state.json` row (line 62) of
`docs/state-protocol.md`'s `.codescout/` files table for task-8 of the
`2026-08-13-worktree-semantic-search` plan. That row said the companion
reader was `session-start.sh`; confirmed via a scoped read of
`../claude-plugins/codescout-companion/hooks/session-start.mjs` (reading only —
that repo was out of scope for edits in this task) that the real, current
reader file is `session-start.mjs`, not `.sh`. The fix at line 62 corrected the
filename for that one row, but four other table rows in the same document
still say `session-start.sh` for the same companion component.

## Symptom (Effect)

`docs/state-protocol.md` lines 56, 57, 60, and 65 (pre-fix numbering) all read
`companion \`session-start.sh\`` in the Readers column — e.g. line 56:

```
| `system-prompt.md` | ... | companion `session-start.sh` (injects into Claude Code session) | ... |
```

but the actual hook file, confirmed live, is
`codescout-companion/hooks/session-start.mjs`.

## Reproduction

`grep -n "session-start" docs/state-protocol.md` inside the codescout repo;
cross-check against `codescout-companion/hooks/` in the `claude-plugins` repo
(file listing, not content edit).

## Environment

codescout repo, `experiments` branch; `claude-plugins` repo,
`codescout-companion/hooks/session-start.mjs` (read-only check, not edited).

## Root cause

Unknown in detail — most likely the companion plugin's hooks were migrated
from shell scripts to Node (`.mjs`) at some point and `docs/state-protocol.md`
was updated for the `index-state.json` schema content (Task 4 of this same
plan fixed lines 88-119) but the filename references elsewhere in the same
table were never swept. Not measured against a specific migration commit —
inferred from the current file list in `claude-plugins`, not investigated
further (out of scope for this task).

## Evidence

### grep hit confirming the live reader file

```
codescout-companion/hooks/session-start.mjs (workspace=/home/marius/work/claude/claude-plugins):
158:const indexState = join(csProjectDir, 'index-state.json');
...
172:lastCommit = JSON.parse(readFileSync(indexState, 'utf8')).last_indexed_commit || '';
```

Confirms both the filename (`.mjs`) and that it reads `last_indexed_commit`
today (not "planned").

### The other stale rows

`docs/state-protocol.md` (pre-fix line numbers) — rows for `system-prompt.md`,
`memories/<topic>.md`, `embeddings.db` (legacy), and `cc_session_id` all say
`companion \`session-start.sh\`` in their Readers column.

## Hypotheses tried

1. **Hypothesis:** these are a different, still-`.sh` companion script (not
   the same file as `session-start.mjs`). **Test:** none run — out of scope
   for this task, which was restricted to reading (not exploring the whole
   hooks directory). **Verdict:** deferred — plausible but unconfirmed; the
   task-8 brief for the "retire the contradiction" slice of this same plan
   already states hooks were migrated from `.sh` to `.mjs` "invoked via node"
   with only a few exceptions (`il3-deny-hook.sh`, `detect-tools.sh`,
   `*.test.sh`), which does not obviously carve out a separate
   `session-start.sh`.

## Fix

Fixed 2026-08-14 on `experiments`. All **nine** occurrences of `session-start.sh`
in `docs/state-protocol.md` corrected in one atomic `edit_markdown` batch —
verified afterwards by `grep 'session-start\.(sh|mjs)|run\.mjs session-start'`
returning 10 hits with **zero** `.sh` remaining (the 10th is line 62, already
correct before this fix).

Two classes, two different corrections:

| Rows | Section | Was | Now |
|---|---|---|---|
| 56, 57, 60, 65 | `## .codescout/` | companion `session-start.sh` | companion `session-start.mjs` |
| 139, 140, 141 | `## .buddy/` | buddy `session-start.sh` | buddy `run.mjs session-start` |
| 159 | `## ~/.claude/buddy/` | `session-start.sh` performs one-shot deletion | `run.mjs session-start` performs … |
| 223 | `## Backwards-compat fossils` | `session-start.sh` deletes on first run | `run.mjs session-start` deletes … |

**This bug undercounted its own scope by five rows.** The title says "four other
rows" and the Resume actively warned the fixer *off* the buddy rows, calling
buddy's hook "a different, legitimately-`.sh` component." That premise is false.
Measured 2026-08-14, `ls claude-plugins/buddy/hooks/` → `hook_dispatch.py`,
`hooks.json`, `judge.env`, `run.mjs` — there is no `session-start.sh` anywhere in
buddy. `buddy/hooks/hooks.json:4` dispatches
`node run.mjs session-start`, so the five buddy rows were stale in a *different*
way than the companion rows, not correct as the file claimed.

The original filing never ran `ls` on either hooks directory — it inferred
buddy's shape from the companion migration note in the task-8 brief. Hypothesis 1
was left `deferred` for the companion rows; the same uncertainty applied to the
buddy rows but was written down as a settled fact instead.
## Tests added

None. Doc-only content fix with no code surface to assert on — the file backs no
`include_str!` constant and no test reads it.

The real regression guard for this bug class already exists and is itself an open
bug: `librarian(action="audit_doc_refs")` lints markdown for stale code refs, but
per `docs/issues/2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing.md`
its file set has gaps. A stale *cross-repo* filename like this one is out of
reach for that lint regardless — the referenced file lives in `claude-plugins`,
which the audit does not traverse. That is a coverage limit worth knowing, not a
blocker for closing this bug.
## Workarounds

None needed — informational only, does not block any tool.

## Resume

N/A — fixed and verified. Do not re-open to "check the buddy rows"; they were
part of this fix.
## References

- `docs/state-protocol.md`
- `codescout-companion/hooks/session-start.mjs` (claude-plugins repo)
- `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-8-brief.md` (sibling slice, hook/companion-plugin.md corrections)
- Found during: `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-8-report.md`
