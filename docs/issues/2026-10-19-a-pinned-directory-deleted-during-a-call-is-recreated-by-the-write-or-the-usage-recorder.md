---
id: 13cffc336e09af35
kind: bug
status: wontfix
title: 'BUG: a pinned directory that is deleted 0-30 ms into a call is re-created by the call (create_file, memory write, usage.db, write.lock.holder)'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-01
opened: 2026-10-19
owner: marius
severity: low
unverified: 'WITHDRAWN 2026-11-01. Was: the reviewer''s 15-of-60 figure, not reproduced here. The wontfix rests on the cost and on the harm, not on the rate; a higher rate would not change it. Reopen on the triggers in § Decision 2026-11-01.'
---

# BUG: a pinned directory that is deleted 0-30 ms into a call is re-created by the call

## Summary

The existing-directory check for a `workspace=` pin runs once, at dispatch (`docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md`). A pinned directory that is removed after that check and before the call's own writes finish is re-created by them: the reviewer measured 15 re-creations in 60 attempts with the directory deleted 0-30 ms into the call, through `create_file`, `memory write`, the usage database and `write.lock.holder`.

## Class fit

An argument that names a target is validated once and then trusted for the rest of the call, so a change of the target between the check and the use is not seen. The fix for the pin being checked at all left this window open; it is the time-of-check to time-of-use twin of that bug.

## Symptom (Effect)

After a pinned call whose directory was deleted mid-call, the directory exists again with the files the call wrote (`.codescout/usage.db`, `.codescout/write.lock.holder`, the written file or memory topic).

## Reproduction

Not reproduced by the session that filed this. The reviewer's driver and probe tables are under `scratchpad/rv-pin2/` of the session that ran the review (`driver/*.py`, `probe*.json`). Note also that `a_pin_that_vanishes_before_the_write_lock_is_a_recoverable_refusal` in `src/server/workspace_pin_tests.rs` uses a path that never existed, not one that vanishes mid-call, so it does not cover this window.

## Environment

Linux, branch `fix/workspace-pin-2026-10-18` after commit `46329d271`.

## Root cause

Unknown. Hypothesis, not tested: the tools write through `create_dir_all` and the usage recorder opens its database under the project root without re-checking that the root still exists.

## Evidence

The reviewer's measurement quoted above; nothing else.

## Hypotheses tried

None by the filing session.

## Fix

PARTLY fixed; status stays `open`. Only the usage-recorder path is fixed: `open_db` (`src/usage/db.rs`) creates just the `.codescout` leaf with `create_dir`, tolerating AlreadyExists for a directory, so a vanished project root is no longer recreated by the recorder.

- Fix SHA for that part (on `experiments`; merged 2026-10-24 by fast-forward, tip `e0255583b`): `640ef2e79bf54e446ad73d81cca2b8a37499bd36`
- patch-id: `71ea06b50d6abac6b501a39a4be911f6dcb4ec1d`

These paths STILL create parents and can recreate a deleted root: `util::fs::write_utf8`'s `create_dir_all` (the `create_file` path the issue names); `MemoryStore::from_dir` and `open`; `WriteTransaction::prepare`; the write-lock file (`open_lock_file`); and `sqlite_vec_ext::open_conn`'s `create_dir_all`, which now runs on every memory call because the sqlite memory store is built per call, so that window is wider than before. Closing them needs a root-aware directory-creation helper threaded through the write paths: an owner design choice. A call-level probe (pin a resident project, delete its directory, `create_file` a nested path) passed BEFORE any fix, because the second pin check at the call already refuses a vanished root; the remaining window is microseconds to milliseconds between that check and the write's `create_dir_all`, and it was not hit deterministically.

## Tests added

N/A: not fixed; this file records a finding.

## Workarounds

Do not delete a project directory while a pinned call is running against it.

## Resume

Reproduce with a loop that deletes the pinned directory at a random 0-30 ms offset into a `create_file` call, then decide where the re-check belongs.

## Re-verification 2026-10-30

Checked, no code changed. `util::fs::write_utf8` still calls `create_dir_all(parent)` (`src/util/fs.rs` line 498), so the remaining window the Fix section lists is still there. The window is microseconds between the dispatch pin check and the write; it needs the root-aware directory-creation helper the Fix section names, which is an owner design choice. Verdict: needs design.


## Decision 2026-11-01 — wontfix for the paths that still create parents

The operator asked for a choice between `wontfix` and a design. The choice is `wontfix` for the remaining paths. The usage-recorder part stays fixed (`640ef2e79bf54e446ad73d81cca2b8a37499bd36`).

What the choice rests on:

- **Cost.** The fix is a root-aware directory-creation helper threaded through five write paths: `util::fs::write_utf8`, `MemoryStore::from_dir` and `open`, `WriteTransaction::prepare`, `open_lock_file`, and `sqlite_vec_ext::open_conn`. Each path needs its own test, and none of the five has a reproduction to be red against.
- **Harm.** The window needs a deliberate deletion of the project root while a pinned call runs. The result is a re-created directory with the files that call wrote. A `rm` removes it. No data is lost and no write leaves the project root.
- **Workaround.** Do not delete a project directory while a pinned call runs against it. The dispatch-time check already refuses a root that is gone before the call.

What the choice does not rest on: the rate. The filing session's call-level probe passed, and the reviewer measured 15 re-creations in 60 attempts. Those two results disagree, and no session has resolved the disagreement.

Reopen when one of these happens:

- A session reproduces the re-creation with a fixed driver, without a deliberate delay or loop that targets the window.
- A re-created directory causes a loss that `rm` does not repair, for example a stray `.git` or a registered worktree path.
- Another change adds a sixth path that calls `create_dir_all` on a parent of the project root.

Nothing was run for this decision and no code changed.

## References

## References

- `src/server.rs` (`call_tool_inner`, `refuse_pin`)
- `src/agent/mod.rs` (`ensure_resident`, `existing_project_dir`)
