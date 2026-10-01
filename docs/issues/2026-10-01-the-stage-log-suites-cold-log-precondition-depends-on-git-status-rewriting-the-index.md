---
id: '59d7bf0baf6f4b83'
kind: bug
status: open
title: The stage-log suite's cold-log precondition depends on git status rewriting the index, which git does not always do, so four assertions fail together at random
tags:
- cluster/unclassified
---

# BUG: the stage-log suite's cold-log precondition depends on `git status` rewriting the index, which git does not always do, so four assertions fail together at random

## Summary

`tests/hooks-discrimination.sh` § "stager wins" removes `.git/session-stage-log`, runs
`CLAUDE_CODE_SESSION_ID=$B git status --short`, and expects the `post-index-change` hook to fire and
recreate the log. A `git status` writes the index only when it has something to refresh (a racily
clean entry); when the preceding `git add` / `git rm --cached` and the `git status` land inside the same
timestamp tick there is nothing to refresh, git does not write the index, the hook does not fire, and
the log stays absent.

## Symptom (Effect)

Four assertions fail at once and the first names itself as the cause:
`precondition: peer status recreated the stage log`, then `cold log + peer status -> unknown, not the
passer-by` (want `-`, got `NO-LOG`), `unknown reads as foreign -> refuse` and `refusal names the staged
deletion`. They read as a regression in whatever was just changed; nothing in the diff under test is
involved.

## Reproduction

Not reproduced on demand. Observed 2026-10-01 while working on the stage-log recorder: 1 of 8 runs of
the unmutated suite failed exactly these four assertions (the next three runs on the same bytes were
green, 269 passed), and the same four names appear in the failure list of 9 of the 156 mutation runs
made that day. The mutation script prints at most four failing names per run, so that second count is
a floor, and the mutation runs are the loaded case.

## Root cause

Read from the suite and from git's documented behaviour, not isolated by experiment: the test
assumes `git status` writes the index, and git writes it only when entries need refreshing. I did not
capture the index mtime against the add to confirm the same-tick reading.

## Fix

Not done. The precondition should not depend on a git side effect: trigger the recorder directly, or
make the index stale first (`touch` the staged file after the add, so `status` has an entry to
refresh), and keep the existing `log_recreated` assertion as the guard that says the setup worked.

## Tests added

None.

## References

- `docs/issues/archive/2026-10-01-a-recorded-write-stays-live-after-another-sessions-commit-swept-its-content.md` — found while running its suite and mutation set; the mutation probe reads these four failures as extra "kills" unless the names are filtered.
