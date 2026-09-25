---
status: fixed
opened: 2026-09-24
closed: 2026-09-24
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

**A second defect on the same line**, found by the fix's own fixture: the slug replaced only
`/`, but Claude Code replaces every non-alphanumeric byte (measured 2026-09-24: 11 project dirs
across all profiles, none holding a character outside `[A-Za-z0-9-]`). So a session STARTED in
`.worktrees/x` — or any cwd with a dot, such as a `mktemp` dir — was missed as well.

**And the reverse error, pre-existing:** a relative write is ambiguous once two trees share a
transcript dir. Run from the main checkout, the script credited a session's WORKTREE writes to
the main checkout's same-named files, because `src/x.rs` matched either.

Two effects, not one: `fmt-mine` over-refuses (loud, recoverable), and `wip_authors` under-reports
(silent — it reads as "nobody wrote this").

## Fix

`scripts/file-provenance.py`:

- `claude_slug` — Claude's rule, `[^A-Za-z0-9]` → `-`.
- `main_checkout` + `transcript_roots` — in a linked worktree, also read the main checkout's
  transcript dirs (`git rev-parse --path-format=absolute --git-common-dir`).
- `scan` tracks, per transcript file, the tree a relative path names: the record's `cwd`, moved by
  `workspace(action="activate")` (`activated_tree`); `write_base` resolves codescout tools
  (`run_command` included) against the active tree unless `workspace=` pins the call, and native
  `Bash` against `cwd`. `normalize` takes that base. Records without `cwd` keep the old reading.

Known blind spot, stated at the site: an MCP server restart resets the active project to the cwd
and the transcript does not record it.

Fix: `d10d7f60` on branch `fix/lessons-friction` · patch-id `f6700f1da3b06df15dd6e52222da6f4b54724232`.
(Rebased onto `experiments` @ `ffff1dfb` on 2026-09-25, via `f918548c`. The first commit was `1f7c6e0c`,
patch-id `e6df17c8…`: the patch-id CHANGED because resolving the `tests/file-provenance.sh`
conflict moved the hunk's context, so the old value no longer identifies this fix.)
### Review follow-up (2026-09-25, independent Opus review)

- **The active tree is process-wide, not per transcript.** codescout's active project belongs to
  the MCP server, shared by a session and its subagents
  (`docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`),
  so per-file tracking credited a subagent's relative write to the wrong tree. `scan` is now two
  passes: `session_activations` collects per-session facts first, and a relative codescout write
  is credited to NEITHER tree where its tree is unknowable — from a subagent of a session that
  activated a non-root tree (or whose subagents activated one), or from the parent at or after
  its subagents' first activation. The parent's own activations are still tracked, which keeps
  the original worktree fix.
- **A subdirectory cwd lost every relative write**: the active tree now starts at the cwd's
  checkout root (`checkout_root`), not the raw cwd.
- **`run_command`'s `cwd=`** now resolves under the active tree.

Tests: 11 more assertions in `tests/file-provenance.sh` (179 total), each negative paired with a
positive control; 7/7 mutations killed.

Review fix: `c4043285` on branch `fix/lessons-friction` · patch-id `07184a2f8dfc2f054db97537536b619489cc4e95`.

## Tests added

`tests/file-provenance.sh` § *a linked worktree finds its transcripts* — 11 assertions, run
without `FILE_PROVENANCE_ROOTS` (every older case pins it, so none exercised discovery): a real
repo + worktree and a fake `HOME`. Each positive has a control for the opposite mis-filing. 6/6
mutations killed (old slug, no main scan, base ignored, `workspace=` ignored, activation ignored,
`Bash` resolved as active). Dependent suites green: `tests/fmt-mine.sh`, `tests/attribute-red.sh`
(41), `tests/pre-commit-ledger-divergence.sh`.

On real transcripts, 2026-09-24: from the worktree, `scripts/file-provenance.py` and
`tests/file-provenance.sh` → `MINE ... written by THIS session`; the same paths from the main
checkout (`REPO_ROOT=` main) → `UNKNOWN`, correctly — that copy was never edited.
## Workarounds
`cargo fmt --all` in the worktree after confirming with `--check -l` that it names only your own
files — safe when the worktree is private to the session.

## Resume

N/A once committed. Tag through the catalog after merge.
## References
- Found running the final gate on branch `fix/lessons-friction`.
