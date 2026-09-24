---
id: '0cbb244ca3bff11e'
kind: bug
status: open
title: 'BUG: the foreign-index guard''s "re-stage by explicit path" remedy is a no-op when the blob is already staged'
tags:
- cluster/hint-composed-without-the-request
closed: null
opened: 2026-09-24
owner: marius
related: []
severity: low
---

## Summary

When the foreign-index pre-commit guard refuses because a path's stage-log row is
`unnamed` (a "blanket add"), its remedy text says *"Re-stage by explicit path and the bare
commit passes."* That is false whenever the content is already staged — the common case,
since the refused session just staged it. `git add -- <path>` of an identical blob does not
change the index, the stage-log records nothing new, the `unnamed` row stands, and the next
commit is refused again with the same text. What works is unstaging first
(`git reset -q -- <paths>`) and then adding by explicit path, which the text never says.

## Symptom (Effect)

Measured 2026-09-24 on a 169-path docs commit: staged with `git add --pathspec-from-file=<f>`
(recorded as `-` / `unnamed` — the recorder does not read the file), refused; re-staged with
every path named on the command line, refused identically, the stage log still holding
`-\t<blob>\t<path>\tunnamed` for every row; `git reset -q --pathspec-from-file=<f>` then the same
named `git add` produced `09093108…\t<blob>\t<path>\tnamed` rows and the bare commit passed
(`cb54d062`). Three commit attempts where the guard's own instruction promised one.

## Reproduction

1. Stage a path so the recorder marks it `unnamed` (e.g. `git add --pathspec-from-file=f`).
2. `git commit` → refused, remedy "Re-stage by explicit path".
3. `git add -- <the same path>` (content unchanged) → `grep <path> .git/session-stage-log` still
   shows only the `unnamed` row.
4. `git commit` → refused again.

## Root cause

The recorder attributes staging from index CHANGES (keyed on `(blob, path)`), and an identical
re-add is not a change. The remedy text is written as if any explicit `git add` re-records.
Separately, the "blanket add" cause line lists `-A`, `-u`, `.` and directories but not
`--pathspec-from-file`, so a reader cannot tell from the text that their form was the cause.

## Fix

Not attempted. Either make the remedy text say "unstage first (`git reset -q -- <paths>`, index
only), then add by explicit path", or have the recorder record a named re-add of an
already-staged blob. Adding `--pathspec-from-file` to the cause list is a one-line companion.

## Tests added

None.

## References

- `docs/conventions/shared-checkout-commit-sequence.md` — the sequence the refusal prints
