---
id: e8ba5df2b174fc4c
kind: bug
status: fixed
title: 'BUG: a pinned project''s memory vectors are written to the DEFAULT project''s `.codescout/embeddings/<name>.memories.db`'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-19
owner: marius
severity: low
unverified: 'No test goes red when one of the seven memory-tool call sites is switched back to `semantic_memory_store()`: the Agent-level behaviour is covered, the call-site wiring is not, and the onboarding wiring is not covered either. A memory call still resolves its root three times, so a concurrent `activate` can split a row from its database (docs/issues/2026-10-24-a-memory-call-resolves-its-root-three-times-so-a-concurrent-activate-can-split-the-row-from-its-database.md). `sqlite_vec_ext::open_conn` now runs on every memory call, which widens the window of the recreated-root issue 2026-10-19-a-pinned-directory-deleted-during-a-call-is-recreated-by-the-write-or-the-usage-recorder.md.'
---

# BUG: a pinned project's memory vectors are written to the DEFAULT project's embeddings database

## Summary

Reported by the review of `bdf34bdb4..46329d271`: a `memory` write pinned at project B stores its vectors in the session's DEFAULT project A, under `A/.codescout/embeddings/<name>.memories.db`, not under B. The reviewer's pointer is `src/agent/mod.rs` around lines 2416-2433 (the semantic memory store bootstrap).

## Class fit

A per-call target (the pin) is honoured for the file the tool writes but not for the side store that is derived from it, which still resolves through the session default. It is the pin-is-per-call property failing at a second consumer.

## Symptom (Effect)

After a pinned memory write, project B's `.codescout/embeddings/` has no memories database and project A's does, named after B's memory.

## Reproduction

Not reproduced by the session that filed this. The reviewer's observations are in `scratchpad/rv-pin2/probe1.json` and `probe3-base.json` (of the session that ran the review).

## Environment

Linux, debug build of branch `fix/workspace-pin-2026-10-18` after commit `46329d271`; the same was seen on the base commit `bdf34bdb4` (`probe3-base.json`).

## Root cause

Unknown. Hypothesis, not tested: the semantic memory store is a single lazily-built `OnceCell` on `Agent`, keyed to the default project when first built.

## Evidence

The reviewer's probe files only.

## Hypotheses tried

None by the filing session.

## Fix

Fixed in `src/agent/mod.rs`, `src/tools/memory/mod.rs`, `src/tools/semantic/index.rs`, `src/tools/onboarding.rs` and `src/prompts/builders.rs`. `Agent::semantic_memory_store_for` resolves the root of EVERY call (the pin via `require_project_root_for`, else the current default root) and builds a sqlite-vec store over that root's embeddings directory, per call (it only stores a path; the connection opens lazily and is dropped with the store). There is no "pin equals default" shortcut, because the default cell is filled once and never reset, so after an `activate` that shortcut sent the new project's vectors into the previous project's directory. `Agent::memory_embedder_for` follows the same root, so vectors are embedded with that project's own `[embeddings]` config (cached by url, model, dim pin and api key). Under Qdrant the single collection and the default embedder are unchanged. All memory-tool call sites, `IndexVerify` and the onboarding preferences section pass `ctx.workspace_override`.

- Fix SHA (branch `integration/fixes-20261024`, not yet on `experiments`): `640ef2e79bf54e446ad73d81cca2b8a37499bd36`
- patch-id: `71ea06b50d6abac6b501a39a4be911f6dcb4ec1d`

Two independent Opus reviews: the first found a stale default after `activate`, a forever-cache and a dimension-clash risk; the second found the Qdrant embedder/store split, the onboarding path, and that the per-root embedder test silently skipped on machines that export `CODESCOUT_EMBEDDER_URL` (including this one, through `~/.cargo/config.toml`). All are fixed in the commit.

## Tests added

In `src/agent/mod.rs`, none depending on process env and none returning early: `a_pinned_projects_memory_vectors_go_into_its_own_embeddings_dir`, `memory_vectors_follow_the_default_project_across_an_activate` (red on the old code), `memory_embedder_follows_the_resolved_roots_embedding_config`, `memory_embedders_are_cached_by_embedding_config`, `the_qdrant_backend_embeds_with_the_default_embedder_for_every_root`; and `prompts::builders::preferences_tests::the_section_lists_only_the_given_projects_preferences`. Mutating the cache key to ignore the model, or disabling the Qdrant arm, turns them red. Gate on the merged tree: exit 0.

## Workarounds

Activate the project instead of pinning it when its memory search results matter.

## Resume

Read `Agent::semantic_memory` and its bootstrap near `src/agent/mod.rs:2416`; write a test that pins a second project, writes a memory, and lists both projects' `.codescout/embeddings/`.

## References

- `src/agent/mod.rs`
