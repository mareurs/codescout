---
id: '981d0c717f6ce61f'
kind: bug
status: open
title: The recorder's write lookup matches a relative path from a call made in another tree, so a worktree edit can name its writer as the owner of the same path here
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
---

# BUG: the recorder's write lookup matches a relative path from a call made in another tree, so a worktree edit can name its writer as the owner of the same path here

## Summary

`foreign_writer` in `scripts/post-index-change-stage-log.sh` matches a relative `path`, `rel_path` or
`new_rel_path` against this checkout's path with no regard to WHICH tree the call ran in. A session
editing `src/x.rs` inside a linked worktree (`<repo>.worktrees/<name>`) leaves a row whose relative
path is byte-identical to this checkout's `src/x.rs`. If another session then stages `src/x.rs`
here, the lookup names the worktree's writer as the path's owner (route `named-foreign`) although
the write never touched this tree.

## Symptom (Effect)

Not observed as a refusal. What is measured is the population that makes it possible.

## Evidence

Measured 2026-10-01, about 17:00 UTC, against the live `.codescout/usage.db`, tree `8bfd3253`, unit = one
successful `edit_file`/`edit_code`/`create_file` row in the last three days: 1620 write rows, 1106 of
them with a relative `path`; 27 of those (2.4%) ran in a tree other than this checkout (the
`workspace` argument when the call carries one, else the `project_root` column), and all 27 name a
path that is tracked here. The trees: `codescout.worktrees/mutation-slot-1`, `port-28-test`, `il3-amp`
(sessions `e41af068` and `0b05903e`; 18 of the rows are `il3-amp`). Whether any of those paths was
staged here inside its three-day window was not searched for, so this is a latent defect with a
measured population, not an incident.

The guard's direction matters: this OVER-attributes a path to a writer who never touched it, a
false refusal of the stager's routine commit. The recorder already prefers the quiet old claim to a
loud wrong one for every other failure to read the record.

## Fix (not done)

Restrict the relative-path clauses to rows whose effective root is this checkout, where the effective
root is the call's `workspace` argument when present and else `project_root`, and a row with neither
is taken as this checkout. Two cautions found while reading, so the next author does not rediscover
them: an absolute `path` and a `doc` id name this checkout unambiguously and must keep matching
whatever tree the call ran in (a call made from a worktree can write here by absolute path); and a
`project_root` column may be absent from a database an older build created (the table's columns were
appended over time; I did not check which build added it), and the recorder's query fails OPEN and
SILENTLY on any SQL error, so a naive predicate on a missing column would disable the whole lookup
on such a database while the suite, whose fixture would carry the column, stays green. A fixture
needs a database WITHOUT the column as well.

## References

- `docs/issues/2026-09-07-the-stage-log-records-the-stager-so-git-add--A-makes-you-the-owner.md` — the parent record.
- `docs/issues/archive/2026-10-01-the-stage-log-recorder-reads-a-subagents-composite-session-id-as-a-peer.md` — found in the same pass, by the same population measurement.
