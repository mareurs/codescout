---
status: open
opened: 2026-09-24
closed:
severity: medium
owner: marius
related: []
tags: [cluster/selector-narrower-than-its-population]
kind: bug
---

# BUG: `file-provenance.py` reads no transcripts inside a worktree, so every write there is `UNKNOWN`

## Summary
The script derives the transcript directory from the git toplevel. Inside a linked worktree that
is the worktree path, but Claude stores a session's transcripts under its **cwd** (normally the
main checkout). The derived directory does not exist, zero transcripts are read, and every file is
reported `UNKNOWN` — so `fmt-mine.sh` refuses the session's own files and `attribute-red.py`'s
`wip_authors` says "no session on record" for a red the reader caused.

## Symptom (Effect)
From `./scripts/gate.sh` in `.worktrees/fix-lessons-friction`, on four files this session wrote:
```
fmt-mine: REFUSED — these need formatting and are not this session's to write:
UNKNOWN   src/ast/python_ranges.rs
          no record of any session writing this path in the window. ...
GATE EXITS -> FMT=1 CLIPPY=0 LEAN=0 DEFAULT=0
```
`src/ast/python_ranges.rs` did not exist before this session created it.

## Reproduction
Work in a linked worktree from a session whose cwd is the main checkout; edit a `.rs` file badly
formatted; run `./scripts/gate.sh`.

## Root cause
`scripts/file-provenance.py:265` — `slug = "-" + str(root).lstrip("/").replace("/", "-")`, with
`root` from `git rev-parse --show-toplevel` (`:233`). measured 2026-09-24:
`ls -d ~/.claude*/projects/*fix-lessons-friction*` → nothing; the session's transcripts are under
`~/.claude-mar/projects/-home-scurtuecaterina-Documents-Project-Codescout`.

Two effects, not one: `fmt-mine` over-refuses (loud, recoverable), and `wip_authors` under-reports
(silent — it reads as "nobody wrote this").

## Fix
Also scan the main checkout's slug (`git rev-parse --git-common-dir`'s parent) when the toplevel
is a linked worktree, and keep matching by repo-relative path.

## Tests added
_pending_

## Workarounds
`cargo fmt --all` in the worktree after confirming with `--check -l` that it names only your own
files — safe when the worktree is private to the session.

## Resume
Not started.

## References
- Found running the final gate on branch `fix/lessons-friction`.
