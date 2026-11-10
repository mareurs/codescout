---
id: '191a5b00c7ab02b6'
kind: bug
status: open
title: 'BUG: onboarding reports "not built" when the Qdrant probe fails, and Phase 1 then offers to build an index that exists'
tags:
- cluster/unclassified
- onboarding
- readiness
- prompts
closed: null
opened: 2026-11-11
owner: marius
severity: low
unverified: 'Found by reading the code at HEAD ab599d204. Not reproduced at runtime: no run with an unreachable Qdrant and an existing index was made.'
---

# BUG: onboarding reports "not built" when the Qdrant probe fails

## Summary

`probe_index_status` (`src/tools/onboarding.rs`) returns `{ready: false, files: 0, chunks: 0}` for four different situations: no project id, no retrieval client, a failed stats call, and a genuinely empty index. `build_onboarding_prompt` renders every one of them as `**Semantic index:** not built`. Phase 1 of the onboarding slice then tells the agent that semantic search is "not set up yet" and offers to build the index.

## Class fit

Unclassified. A negative result that does not name its scope: "not built" is asserted when the probe could not look. See `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. No declared cluster names this shape. It is the mirror of the "ready" overclaim in bug `db67deedbae5a09e`, which asserted a working index from stored chunks alone.

## Symptom (Effect)

With an index that exists but a retrieval store the probe cannot reach, the onboarding prompt carries `**Semantic index:** not built`. The agent is offered "Build now", "Build from CLI" or "Skip". The first option calls `index(action='build')` for an index that is already built.

## Reproduction

Not run. By reading: `probe_index_status` takes the `Err(_)` arm at `RetrievalClient::from_env` or at `project_index_stats` and returns `ready: false`; `index_ready` is then `false` in `OnboardingContext`, and the `else` branch of `build_onboarding_prompt` prints `not built`. A run needs a project with a stored index and a retrieval store that refuses the stats call.

## Environment

codescout `experiments` at HEAD `ab599d204`.

## Root cause

The `ready` field is a bool that cannot say "unknown". The function's doc comment states the intent: `ready: false` "when the retrieval stack is offline". That is a reasonable envelope for a best-effort probe, and the wrong input for a sentence that says "not built".

## Evidence

Read at the bytes this session: `probe_index_status` at `src/tools/onboarding.rs:980-1009` (the three `ready: false` returns and the `chunks > 0` arm), and `build_onboarding_prompt` in `src/prompts/mod.rs` (the `index_ready` branch). No runtime evidence.

## Hypotheses tried

None beyond reading the code.

## Fix

Open. The probe needs a third state, for example `ready: null` or a `probe_failed` flag, and the prompt needs a third line for it, such as "could not check the semantic index". Phase 1 then needs a branch that does not offer to build. This touches the `onboarding_prompt` slice, so it needs an `ONBOARDING_VERSION` bump.

## Tests added

None.

## Workarounds

The agent can run `index(action='status')` before choosing "Build now".

## Resume

Filed 2026-11-11 while rewording the "ready" wording (bug `db67deedbae5a09e`, commits `ad5fbcfc9` and `ab599d204`). No fix attempted.

## References

- Bug `db67deedbae5a09e`: the "ready" overclaim.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.
