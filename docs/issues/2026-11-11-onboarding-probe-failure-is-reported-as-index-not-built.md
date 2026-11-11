---
id: '191a5b00c7ab02b6'
kind: bug
status: fixed
title: 'BUG: onboarding reports "not built" when the Qdrant probe fails, and Phase 1 then offers to build an index that exists'
tags:
- cluster/unclassified
- onboarding
- readiness
- prompts
closed: 2026-11-11
opened: 2026-11-11
owner: marius
severity: low
unverified: Not run with an unreachable retrieval store and an existing index; only the no-project arm is tested. Phase 1 of source.md has no branch for the new state and ONBOARDING_VERSION was not bumped, by choice.
---

# BUG: onboarding reports "not built" when the Qdrant probe fails

## Summary

`probe_index_status` (`src/tools/onboarding.rs`) returns `{ready: false, files: 0, chunks: 0}` for four different situations: no project id, no retrieval client, a failed stats call, and a genuinely empty index. `build_onboarding_prompt` renders every one of them as `**Semantic index:** not built`. Phase 1 of the onboarding slice then tells the agent that semantic search is "not set up yet" and offers to build the index.

## Class fit

Unclassified. A negative result that does not name its scope: "not built" is asserted when the probe could not look. See `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. No declared cluster names this shape. It is the mirror of the "ready" overclaim in bug `db67deedbae5a09e`, which asserted a working index from stored chunks alone.

## Symptom (Effect)

With an index that exists but a retrieval store the probe cannot reach, the onboarding prompt carries `**Semantic index:** not built`. The agent is offered "Build now", "Build from CLI" or "Skip". The first option calls `index(action='build')` for an index that is already built.

## Reproduction

Not run end to end with an unreachable store and an existing index. Re-read at the current HEAD on 2026-11-12: `probe_index_status` (`src/tools/onboarding.rs`) had the same three `ready: false` returns, and `build_onboarding_prompt` (`src/prompts/mod.rs`) printed `not built` for all of them. The no-project arm is now reproduced by a test (see § Tests added). The other two arms depend on the retrieval configuration and are not forced in a test.

## Environment

codescout `experiments` at HEAD `ab599d204`.

## Root cause

The `ready` field is a bool that cannot say "unknown". The function's doc comment states the intent: `ready: false` "when the retrieval stack is offline". That is a reasonable envelope for a best-effort probe, and the wrong input for a sentence that says "not built".

## Evidence

Read at the bytes this session: `probe_index_status` at `src/tools/onboarding.rs:980-1009` (the three `ready: false` returns and the `chunks > 0` arm), and `build_onboarding_prompt` in `src/prompts/mod.rs` (the `index_ready` branch). No runtime evidence.

## Hypotheses tried

Re-read 2026-11-12 on current code: the claims in § Summary hold. The fix choice was made with the owner: code only, no `ONBOARDING_VERSION` bump (see § Fix).

## Fix

The three non-answers of `probe_index_status` (no project, no retrieval client, a failed statistics call) now return `ready: false` plus `unchecked`, a reason. An empty index returns `ready: false` without it, which is an answer. `OnboardingContext` gained `index_unchecked: Option<&str>`. `build_onboarding_prompt` prints ``**Semantic index:** could not be checked (<reason>). Run `index(action='status')` before offering to build one, and offer it only if that reports no index.`` The empty index still reads `not built`. The `index_status` field of the onboarding response carries `unchecked` too.

Scope, chosen with the owner on 2026-11-12: code only, as `ad5fbcfc9`. Not done:

- Phase 1 of `src/prompts/source.md` has no branch of its own for this state (it still has `### If the index is NOT BUILT:`). The rendered line carries its own instruction instead.
- `ONBOARDING_VERSION` was not bumped. Per its comment, already-onboarded projects cache the rendered prompt and need a bump to get new wording, so they keep the old line until the next bump.

## Fix provenance

- **SHA:** `e99718b2c` (`experiments`)
- **patch-id:** `9af2ecf92d27e4bc579bd445cb713199a4c6cd4c`

## Tests added

- `build_onboarding_does_not_call_an_unchecked_index_not_built` (`src/prompts/mod.rs`): red before the change (the prompt said `not built`), green after. It also checks that an empty index keeps `not built` and does not say `could not be checked`.
- `a_probe_with_no_project_says_it_did_not_check_the_index` (`src/tools/onboarding.rs`): red before, green after. A context with no project takes the probe's first arm.

Not tested: the `no retrieval client` and `statistics call failed` arms. They use the same helper as the tested arm. Forcing them needs the retrieval configuration, which a test cannot change without mutating the environment.

Gate on 2026-11-12: clippy, LEAN and DEFAULT green in one run. The format lane refused a peer's file in that run, and a separate `./scripts/fmt-mine.sh --check` was green afterwards.

## Workarounds

The agent can run `index(action='status')` before choosing "Build now".

## Resume

Fixed in code and committed, with the scope above. Open: a Phase 1 branch in `source.md` with a version bump, if the rendered line proves too weak. A live check needs an onboarding run with an unreachable retrieval store and an existing index. Then clear the `unverified` field.

## References

- Bug `db67deedbae5a09e`: the "ready" overclaim.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.
