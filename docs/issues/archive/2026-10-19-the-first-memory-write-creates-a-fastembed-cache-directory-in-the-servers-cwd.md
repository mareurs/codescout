---
id: b753d9ee6f8f9a95
kind: bug
status: fixed
title: 'BUG: the first memory write creates `.fastembed_cache/` in the server''s working directory'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-19
owner: marius
severity: low
unverified: No test checks that `new_blocking` actually passes the directory (deleting `.with_cache_dir(...)` keeps all three tests green; a real check needs the ONNX weights). The hub-network test `from_dir_matches_the_hub_path_for_the_same_model` was not run. A relative `FASTEMBED_CACHE_DIR` still resolves against the server's working directory.
---

# BUG: the first memory write creates `.fastembed_cache/` in the server's working directory

## Summary

Reported by the review of `bdf34bdb4..46329d271`: the first `memory` write through the server creates a `.fastembed_cache/` directory in the process's current working directory, not under the project or a cache directory. With a pinned call the cwd has nothing to do with the project, so a stray directory appears next to wherever the server was started.

## Class fit

A path that should be derived from the project or a per-user cache directory is derived from the process's cwd, so the side effect lands wherever the server happens to run. It is the same family as the pin bug that left `x/.codescout/` behind: a build-up of state at an ambient location.

## Symptom (Effect)

`<cwd>/.fastembed_cache/` exists after the first memory write of a server session whose cwd is not a project.

## Reproduction

Not reproduced by the session that filed this. The reviewer's observations are in `scratchpad/rv-pin2/probe1.json` and `probe1.table` (of the session that ran the review).

## Environment

Linux, debug build of branch `fix/workspace-pin-2026-10-18` after commit `46329d271`.

## Root cause

Unknown. Hypothesis, not tested: the embedding library's default cache directory is relative (`.fastembed_cache`) and the memory write path initialises it without setting an explicit cache dir. The embedder construction is in `crates/codescout-embed/`.

## Evidence

The reviewer's probe tables only.

## Hypotheses tried

None by the filing session.

## Fix

Fixed in `crates/codescout-embed/src/local.rs`. `LocalEmbedder::new_blocking` never set a cache directory, so fastembed used the RELATIVE `.fastembed_cache` under the server's working directory (the module doc's claim of `~/.cache/huggingface/hub/` was wrong). `model_cache_dir` now returns a non-empty `FASTEMBED_CACHE_DIR`, else `<user cache dir>/codescout/fastembed` (Linux `$XDG_CACHE_HOME` or `~/.cache`; macOS `~/Library/Caches`; Windows `%LOCALAPPDATA%`), else a temp dir. `HF_HOME`, when set, still replaces the cache directory inside fastembed (fastembed 5.13.0 `src/common.rs:170-172`), so it wins over both. Optional `dirs = "5"` is enabled by both local-embed features. Behaviour change: weights already in `<cwd>/.fastembed_cache` are downloaded once more unless `HF_HOME` or `FASTEMBED_CACHE_DIR` points at them; CI sets `FASTEMBED_CACHE_DIR`. The docs, the CI comment and CHANGELOG.md state the new location and the precedence.

- Fix SHA (on `experiments`; merged 2026-10-24 by fast-forward, tip `e0255583b`): `640ef2e79bf54e446ad73d81cca2b8a37499bd36`
- patch-id: `71ea06b50d6abac6b501a39a4be911f6dcb4ec1d`

## Fix provenance

- **SHA:** `640ef2e79bf54e446ad73d81cca2b8a37499bd36` (`experiments`)
- **patch-id:** `71ea06b50d6abac6b501a39a4be911f6dcb4ec1d`

## Tests added

In `crates/codescout-embed/src/local.rs`, pure functions of their arguments (no env mutation): `model_cache_dir_defaults_to_the_user_cache_not_the_working_directory` (red with the old behaviour stubbed), `model_cache_dir_honours_an_explicit_override`, `model_cache_dir_without_a_user_cache_falls_back_to_the_temp_dir`. Gate on the merged tree: exit 0.

## Workarounds

Start the server from a directory where a stray `.fastembed_cache/` does no harm, and delete it.

## Resume

Reproduce with a server started in an empty directory, one `memory` write, then `ls -A`; find where the embedder takes its cache path.

## References

- `crates/codescout-embed/src/lib.rs`
