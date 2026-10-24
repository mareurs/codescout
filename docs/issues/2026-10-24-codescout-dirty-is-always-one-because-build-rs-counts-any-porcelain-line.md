---
id: '5f247e864d241b0f'
kind: bug
status: open
title: codescout_dirty is 1 on 99.7% of usage rows because build.rs counts any porcelain line on a checkout that is never clean
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-10-24
owner: marius
severity: low
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

Not started. Proposed: `git status --porcelain --untracked-files=no -- src crates build.rs Cargo.toml Cargo.lock` (the inputs that change the binary); optionally bake a hash of `git diff HEAD -- <those paths>` so two dirty builds can be told apart.

## Tests added

None yet.

## Workarounds

Use `codescout_sha` alone and accept that it does not identify the source.

## Resume

`bake_git_sha` in `build.rs`.

## References

- `scripts/friction-probe.py` `--clean-only`
- `src/usage/db.rs` comment near line 269 on `CODESCOUT_GIT_DIRTY`
