---
id: db5fe93344e0371b
kind: bug
status: open
title: docs/state-protocol.md still names companion's session-start hook `.sh` in four other rows (actual file is `.mjs`)
tags:
- docs
- state-protocol
- drift
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

Not implemented — out of scope for the docs-only task that found it (that
task's explicit mandate was the single `index-state.json` row plus a search
for a third *statement of the schema*, not a full `.sh`/`.mjs` sweep of the
document). Likely belongs with the sibling "retire the contradiction" task-8
slice of the same plan, which already touches `docs/architecture/companion-plugin.md`
for an identical `.sh` → `.mjs` correction.

## Tests added

N/A — not fixed yet.

## Workarounds

None needed — informational only, does not block any tool.

## Resume

Sweep `docs/state-protocol.md` for every remaining `session-start.sh`
reference to the companion component (not buddy's own `session-start.sh`,
which is a different, legitimately-`.sh` component — see the `.buddy/` table
lower in the same file) and confirm against the current
`codescout-companion/hooks/` file list before editing.

## References

- `docs/state-protocol.md`
- `codescout-companion/hooks/session-start.mjs` (claude-plugins repo)
- `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-8-brief.md` (sibling slice, hook/companion-plugin.md corrections)
- Found during: `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-8-report.md`

