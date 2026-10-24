---
status: open
opened: 2026-10-24
closed:
severity: low
owner: marius
related:
- docs/issues/2026-10-19-a-pinned-projects-memory-vectors-go-into-the-default-projects-embeddings-database.md
tags:
- cluster/unclassified
kind: bug
unverified: 'Read from the code only; the bug was not reproduced at runtime before the fix. Found by an independent review of the pinned-memory fix. The fix has a unit test for the store read only; its wiring to the onboarding call is not covered end to end.'
---

# BUG: onboarding's `## User Preferences` section is read from a store and project id that do not follow the call's pin

## Summary

`onboarding` built the rest of its output from the pinned project, but `append_preferences_section` took the project id from the session default and read the semantic memory store from `Agent::semantic_memory_store()`, a cell built once for whichever project first filled it. So the "User Preferences" section was the wrong project's preferences (or none), and after an `activate` the read went to the PREVIOUS project's directory.

**Fixed in the working tree that adds `Agent::semantic_memory_store_for`** (see Fix); `status` and `closed` are set with the commit.

## Class fit

A per-call target (the pin) is honoured by the tool body but not by a helper it calls, which resolved the project through shared session state. It is the same trait as the other pin bugs: the pin is per-call and one consumer still read the shared default.

## Symptom (Effect)

For `onboarding(workspace=B)` while the session default is A, the draft's `## User Preferences` listed memories of the `preferences` bucket stored under A's project id, or was absent when A had none, whatever B held. After an `activate`, the read also went to the directory the store cell had first been built for, not the active project's: with sqlite-vec, `open_conn` creates `<that dir>/<id>.memories.db`, so a read could create a database file in a project that is no longer the active one.

## Reproduction

Not reproduced before the fix. Best lead: session default A with a `preferences`-bucket memory, project B pinned with a different one, `onboarding(workspace=B)`, then read the draft's `## User Preferences` section.

## Environment

Linux, branch `fix/b-20261024` from `experiments` 30f3e728d.

## Root cause

Inferred from the code, not measured:

- `src/tools/onboarding.rs:1229` called `crate::prompts::builders::append_preferences_section(&ctx.agent, &mut system_prompt_draft)`. It passed the agent and not `ctx.workspace_override`, while the same module uses `ctx.workspace_override` elsewhere (`record_refresh_request`, `stamp_witnessed_refresh`).
- `src/prompts/builders.rs` `append_preferences_section` took the project id from `agent.inner.read().await.active_project()` (the default project's `config.project.name`) and its store from `agent.semantic_memory_store()`. That is one `OnceCell` built from the root at first use and never reset; it is not "the default project's" store, it is the store of whichever project first filled the cell.

## Evidence

The two code sites above; nothing else.

## Hypotheses tried

None.

## Fix

`append_preferences_section` now takes the call's `workspace_override`, reads the project id with `with_project_at(workspace_override, ..)` and the store from `Agent::semantic_memory_store_for(workspace_override)`; `onboarding` passes `ctx.workspace_override.as_deref()`. The store read is split out as `append_preferences_from_store`.

## Tests added

`prompts::builders::preferences_tests::the_section_lists_only_the_given_projects_preferences` pins the rendering and the project scoping of the store read. The store resolution it relies on is covered by `agent::tests::a_pinned_projects_memory_vectors_go_into_its_own_embeddings_dir` and `memory_vectors_follow_the_default_project_across_an_activate`. The call from `onboarding` to the new signature is NOT covered by a test.

## Workarounds

Activate the project instead of pinning it when onboarding should carry its preferences (before the fix).

## Resume

Add an end-to-end test that pins a second project under the sqlite-vec backend and asserts which project's preferences land in the onboarding draft, if the backend can be injected without the process environment.

## References

- `src/tools/onboarding.rs`
- `src/prompts/builders.rs`
- `src/agent/mod.rs` (`semantic_memory_store_for`)
