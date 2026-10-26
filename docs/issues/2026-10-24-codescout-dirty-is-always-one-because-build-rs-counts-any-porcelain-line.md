---
id: '5f247e864d241b0f'
kind: bug
status: fixed
title: codescout_dirty is 1 on 99.7% of usage rows because build.rs counts any porcelain line on a checkout that is never clean
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-10-26
fix_patch_id: aaa23a1918e7fdba94551a6a69618070d7dba1db
fix_sha_experiments: 5ae04c24c74f4572f21d6e441ab87d31d5d8374f
local_fix_branch: research/checked-real-task-20261024
local_fix_patch_id: aaa23a1918e7fdba94551a6a69618070d7dba1db
local_fix_sha: b1a030f8a1a43bf481362b589e50bfe363fadd48
opened: 2026-10-24
owner: marius
severity: low
unverified: 'CLEARED 2026-10-26. Was: the corrected value had not been observed in usage.db. A binary rebuilt at 750e3a320 wrote codescout_dirty=0 on its first row while three out-of-scope paths (.codescout/audit/ripper-65e654-202610.jsonl, docs/trackers/bug-fix-session-log.md, x/) were dirty, after the peer committed scripts/pre-commit-run.sh in 750e3a320. One row was observed, not a population. Standing limit: DIRTY_SCOPE includes scripts, so an uncommitted peer edit under scripts/ still sets the flag to 1.'
---

# BUG: `codescout_dirty` is 1 on 99.7% of rows because build.rs counts any porcelain line, on a checkout that is never clean

## Summary

`build.rs` `bake_git_sha` sets `CODESCOUT_GIT_DIRTY=1` when `git status --porcelain` prints anything. On the shared `experiments` checkout that is always true: untracked scratch (`x/`), docs and tracker edits by peer sessions, and `.codescout/audit/*.jsonl`, a TRACKED file that codescout itself appends to. The usage recorder stores the flag per call. In the codescout `usage.db`, **80,336 of 80,597 rows since 2026-09-24 (99.7%) carry `codescout_dirty=1`**; every project DB in the 2026-10-03..10-24 window is at or near 100%.

## Class fit

The value is correct in its own frame (the worktree had porcelain output at build time) but it is published under a name that states another frame ("the binary was built from modified source"). It differs from other IC-24 members because the producer's own runtime writes are what keep the frame permanently true.

## Symptom (Effect)

`scripts/friction-probe.py --split-at … --clean-only` was designed to drop calls served by a locally-modified build. With the flag at ~100% it drops essentially the whole corpus, so a before/after comparison has no clean arm. The memory `infra/friction-measurement` still cites "`codescout_dirty` went 4% → 22%" as a usable confound control.

## Reproduction

`sqlite3 .codescout/usage.db "SELECT codescout_dirty, COUNT(*) FROM tool_calls WHERE called_at>='2026-09-24' GROUP BY 1"` → `0|261`, `1|80336`.

## Environment

Shared checkout `/home/marius/work/claude/codescout`, branch `experiments`, builds via `./scripts/rb.sh`.

## Root cause

`build.rs:44-50`: `.args(["status", "--porcelain"])` … `if o.stdout.is_empty() { "0" } else { "1" }` — no pathspec, untracked files included.

## Evidence

Git status at the start of the 2026-10-24 session listed `M .codescout/audit/ripper-65e654-202610.jsonl` and `?? x/` among others.

## Hypotheses tried

N/A.

## Fix

Fixed on `experiments` at `5ae04c24c` (patch-id `aaa23a1918e7fdba94551a6a69618070d7dba1db`, cherry-picked from `b1a030f8a` on `research/checked-real-task-20261024`). It uses a conservative source scope that includes scripts and two embedded docs, counts untracked source, anchors Git to the manifest and refreshes metadata on incremental builds in ordinary and linked worktrees. No binary hash or exhaustive dependency fingerprint is claimed.

## Tests added

`tests/test_build_git_dirty.py` adds 24 methods that exercise the real `build.rs` through warm offline Cargo builds. On 2026-10-26 they passed on `experiments` together with `tests/test_friction_probe.py` (38 passed, 18 subtests). `./scripts/gate.sh` also passed on `experiments` (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`). The mutation counts recorded on the research branch (38 detected) are in `docs/research/2026-10-24-matched-checked-completion.md` on that branch only.

## Workarounds

Use `codescout_sha` alone and accept that it does not identify the source.

## Resume

Closed 2026-10-26. To see the effect in `usage.db`, run `./scripts/rb.sh` and restart with `/mcp`, then check that new rows carry `codescout_dirty=0` on a clean source tree. The research branch and its `/tmp` worktree are no longer needed for this fix.

## References

- `scripts/friction-probe.py` `--clean-only`
- `src/usage/db.rs` comment near line 269 on `CODESCOUT_GIT_DIRTY`
