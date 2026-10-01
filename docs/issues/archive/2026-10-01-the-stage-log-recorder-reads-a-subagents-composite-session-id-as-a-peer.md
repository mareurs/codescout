---
id: 5e1c1668eb865d55
kind: bug
status: fixed
title: The stage-log recorder reads a subagent's composite session id as a peer, so a session's own subagent file is refused as foreign
owners:
- '3e2b9cc8-4e6d-4f8f-8e80-e1ece200e6af'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
---

# BUG: the stage-log recorder reads a subagent's composite session id as a peer, so a session's own subagent file is refused as foreign

## Summary

`usage.db` files a subagent's tool calls under `cc_session_id = "<parent session id>/<agent id>"`.
`foreign_writer` in `scripts/post-index-change-stage-log.sh` (`dd7b1590`, and `3dea3b88` before it)
compares that column whole against the stager's `CLAUDE_CODE_SESSION_ID` and against the
`Session-Id` trailers of earlier commits, and both of those name the PARENT alone. So a
subagent's write is never "the stager's", and never "cleared by its writer's commit".

## Symptom (Effect)

A session that delegates edits to a subagent and then stages the result by name has its OWN file
recorded as a peer's: owner = the composite string, route `named-foreign`. The foreign-index
guard then refuses the stager's bare commit, naming an owner that no registry row, socket or
trailer resolves — a party nobody can be asked. Refusing a routine commit of one's own work is
what teaches `--no-verify`.

## Reproduction

`tests/hooks-discrimination.sh` cases 22a-22d: a fixture `usage.db` row with
`cc_session_id = "$A/<agent>"` and `edit_file` of `f.txt`, then `CLAUDE_CODE_SESSION_ID=$A git add f.txt`.
Before the fix: owner is `$A/<agent>` and route `named-foreign` (want `$A` / `named`). Five
assertions red.

## Root cause

Read from the data, not assumed: 3312 of 9706 `tool_calls` rows of the last three days carry a `/`
in `cc_session_id` (measured 2026-10-01 against the live `.codescout/usage.db`; the `agent_id`
column holds the suffix and is set on every one of those rows). The recorder's SQL and its
`lastc` / `[ "$sid" = "$me" ]` comparisons treat the column as a session id.

The measured size on committed work: of 636 non-merge `(commit, path)` pairs with a `Session-Id`
trailer over three days (tree `8bfd3253`, instant 2026-10-01T16:53Z), 490 had a recorded write
by a session the commit names when the column is read whole and 587 when the agent suffix is
stripped — 97 pairs, 15% of the population, whose only recorded writer was the committing
session's own subagent. Each is a pair the lookup, had the path been staged by name, would have attributed
to a composite id no one can be asked, and refused the stager's bare commit over.

## Fix

Strip the agent suffix when a row is read (`${sid%%/*}`), so a subagent is the session that
dispatched it, which is the party the registry resolves, the trailer names and a human can be
asked. The same commit also reads `doc(create)` by its `rel_path` (see Fix provenance).

## Tests added

`tests/hooks-discrimination.sh` cases 22a-22d, red on the unchanged recorder (5 assertions: 22a's owner, both of 22b, 22c, 22d; 22a's route assertion passes there and rests on mutation). The suite is 224 passed and 0 failed.

**Mutation:** 41 sites in the recorder and guard, re-run after the last byte change; 40 killed. The two sites for this fix, the suffix strip and its inversion (keeping the agent id), are killed by 22a-22d. The survivor is the SQLite busy timeout, which is tuning and is annotated as such in the script. A `tool_name = 'doc'` conjunct on the sibling `doc create` clause survived its mutation, could not be false on any live input, and was deleted rather than given a test.

## Fix provenance

- **SHA:** `6e6dc887c9420aee73efd60fe0bb02f806ad5aad` (`experiments`)
- **patch-id:** `797557b820bdcd32fd9733355adc0bcd210480be`

The same commit also reads `doc(create)` by its `rel_path` (cases 23a-23c), which is a coverage gap in the parent record rather than this defect. Gate FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; `tests/commit-mine.sh` 53/0, `tests/install-hooks-check-population.sh` 33/0, `tests/pre-push-foreign-session-guard.sh` 136/0.

## References

- `docs/issues/2026-09-07-the-stage-log-records-the-stager-so-git-add--A-makes-you-the-owner.md` — the parent record; its § *Limits* names this lookup.
- `scripts/file-provenance.py` `record_session` — the same decision taken the other way: a subagent record's session is the parent's.
