---
id: '51b907ca53158c87'
kind: bug
status: open
title: The stage-log recorder run in a linked worktree reads a usage.db that does not exist, so a staging there records only the legacy claim
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
---

## Observed

`foreign_writer` in `scripts/post-index-change-stage-log.sh` resolves its database as `${CODESCOUT_USAGE_DB:-$root/.codescout/usage.db}` with `root="$(git rev-parse --show-toplevel)"`. In a linked worktree `--show-toplevel` is the worktree, and a worktree carries no `usage.db`: codescout files a worktree call's row in the MAIN checkout's `.codescout/usage.db`, tagged with the worktree's own `project_root`. So the recorder's `sqlite3 -readonly` fails to open the file, prints nothing, and that reads as "nobody else wrote it": the staging is recorded by the legacy claim (the stager owns what they staged), which is the defect the lookup exists to fix.

Reproduced 2026-10-02 against tree `200959f8`:

- `git worktree list` holds `codescout.worktrees/mutation-slot-0` and `-slot-1`; `mutation-slot-0/.codescout/` has `audit`, `librarian.toml`, `memories`, `projects`, ... and **no `usage.db`**.
- `sqlite3 -readonly …/mutation-slot-0/.codescout/usage.db "select 1"` exits 1: `unable to open database file`.
- Each of those worktrees has its own `session-stage-log` (95 KB and 109 KB under `.git/worktrees/<name>/`), so staging does happen there and is recorded without the write lookup.
- In the main checkout's `usage.db`, 69 rows over 7 days carry a `project_root` under `.worktrees/`, from 3 sessions, so worktree sessions do make codescout calls that the lookup could attribute.

## Why this was not caught

`65d4e5dd` (bug `3094869ba182deab`) fixed the other half, that a worktree session's RELATIVE path was matched as this checkout's path, and tested every case against a database it was handed through `CODESCOUT_USAGE_DB`. A test that supplies the database cannot see that the recorder does not find one by itself in a worktree. Its Fix provenance names the gap as a residual.

## Scope, stated rather than assumed

Severity is low today and is not measured as zero: the two worktrees on this machine are `mutation-probe.sh` slots, whose sessions edit a throwaway copy. No incident is known. What is unexamined: whether a `doc` id from a worktree resolves (the id is `sha256` of the absolute path, which differs in a worktree), and which tree's `usage.db` the writer's rows land in when the worktree is nested under the checkout.

## Direction (not a fix plan; run the reproduction first)

Resolve the database from the main checkout, e.g. via `git rev-parse --git-common-dir`, and keep `CODESCOUT_USAGE_DB` as the override. Then the root predicate shipped in `65d4e5dd` is what separates a worktree's rows from this tree's, and its equality test must compare against the WORKTREE root when the recorder runs inside one. Pin both with a fixture that does not set `CODESCOUT_USAGE_DB`, so the lookup path itself is under test.
