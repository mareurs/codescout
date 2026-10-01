---
id: 931c28128081ad46
kind: bug
status: fixed
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

Reproduced on demand 2026-10-01 outside the suite (`flake_probe`: the suite's setup, 150 iterations in throwaway repos with the real recorder as the `post-index-change` hook): the log was not recreated 8 times, 5.3%. The earlier "not reproduced on demand" in this record was true of the suite and false of the setup.

## Root cause

Confirmed by measurement rather than by reading: moving the staged file's mtime before the `git status` (`touch -d '2 hours ago'`), so its index entry is stale and `status` has something to refresh and therefore write, took the miss rate from 8 of 150 to 0 of 550. The same-tick reading of the cause (the add and the status inside one timestamp tick give the entry nothing to refresh) fits the numbers and the fix; the index mtime was not captured against the add.

## Fix

In `tests/hooks-discrimination.sh` § "stager wins": `touch -d '2 hours ago' s1.txt` before the cold-log `git status`, with the measurement in a comment on the line; the `log_recreated` assertion stays as the guard that says the setup worked. Done in the commit that fixes `981d0c717f6ce61f`, which edits the same file.

## Tests added

No new assertion: the evidence is the loop above (8 of 150 before, 0 of 550 after) and three consecutive green runs of the suite (284 passed). A suite-level red cannot be observed on demand, which is why the probe ran outside it.

## Fix provenance

- **SHA:** `65d4e5dd512bd6e2184016e00ec23963a4c8a238` (`experiments`)
- **patch-id:** `892b172ceb8ec395699d8d5ca798a00751c01716`

## References

- `docs/issues/archive/2026-10-01-a-recorded-write-stays-live-after-another-sessions-commit-swept-its-content.md` — found while running its suite and mutation set; the mutation probe reads these four failures as extra "kills" unless the names are filtered.
