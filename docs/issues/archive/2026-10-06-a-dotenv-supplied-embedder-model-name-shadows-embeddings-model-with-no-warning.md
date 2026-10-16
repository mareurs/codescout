---
id: '95c954f2a7bd3f2d'
kind: bug
status: fixed
title: 'BUG: a dotenv-supplied CODESCOUT_EMBEDDER_MODEL_NAME silently outranks [embeddings].model, and the shadowing warning does not cover it'
tags:
- embeddings
- config
- dotenv
- cluster/guard-narrower-than-its-name
opened: 2026-10-06
closed: 2026-10-16
owner: marius
related:
- docs/issues/archive/2026-09-17-the-configured-embedding-model-is-discarded-whenever-a-url-is-set.md
severity: low
---

# BUG: a dotenv-supplied `CODESCOUT_EMBEDDER_MODEL_NAME` silently outranks `[embeddings].model`, and the shadowing warning does not cover it

## Summary

The embedding config consolidation (Task 5a) added a warning when a dotenv-supplied variable overrides a value that the user set in `config.toml` or `project.toml`. The warning covers `url`, `model` and `api_key`. It does not cover `CODESCOUT_EMBEDDER_MODEL_NAME`, the variable the original defect was about, so a dotenv value for it still wins with no warning.

## Symptom (Effect)

The plan (`docs/plans/2026-09-17-embedding-config-consolidation.md`, lines 127-130) says the shadowing warning for this override was deferred and "lands with Task 5". Task 5a is marked `LANDED 2026-09-17`. The warning it landed does not see this variable.

Read from the source at `10e935e3`:

- `src/retrieval/config.rs:252-254`: `dense_model_name_override: non_empty(std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").ok())`. It is read directly, with no provenance.
- `src/retrieval/config.rs:219-222`: `dense_model_name()` returns the override when set and the bare configured model otherwise. The override therefore wins the name that goes on the wire.
- `src/config/embedding_env.rs:58-61`: the `MODEL` setting has the canonical name `CODESCOUT_EMBEDDING_MODEL` and the aliases `CODESCOUT_EMBEDDER_MODEL` and `CODESCOUT_EMBED_MODEL`. `CODESCOUT_EMBEDDER_MODEL_NAME` is not among them, although the module header (line 12) lists it among the names that "reached" the model setting before the consolidation.
- `src/retrieval/config.rs:532-556`: `EmbedEnv::from_real_env` records dotenv provenance only for `URL`, `MODEL` and `API_KEY`.
- `src/retrieval/config.rs:501-529`: `dotenv_shadowed_fields` can name only `url`, `model` and `api_key`.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

Not run end to end. The steps below are the intended check:

1. Put `CODESCOUT_EMBEDDER_MODEL_NAME=model-a` in the startup dotenv, so it is injected at start.
2. Set `[embeddings] model = "model-b"` in `.codescout/project.toml`.
3. Start the server and read the startup log.
4. Expected by the plan: a warning that the dotenv value shadows the config. Expected from the code: none, and `model-a` is what goes on the wire.

## Environment

Linux, `experiments` at `10e935e3`. Every host configured for the embedding stack sets this variable (see the archived record below), so this is the common case.

## Root cause

Inferred from the five locations above — not measured at runtime.

`dense_model_name_override` was kept as its own read at the config edge, on top of the model name, by a ruling on 2026-09-17: letting `model` win would repoint every stack deployment at the default. The warning that makes that precedence visible needs provenance for the variable, and the provenance channel built in 5a was keyed on the `CODESCOUT_EMBEDDING_*` family. The variable stayed outside it.

## Evidence

The archived record `docs/issues/archive/2026-09-17-the-configured-embedding-model-is-discarded-whenever-a-url-is-set.md` says: "The shadowing WARNING that makes the override visible is deliberately *not* here ... That provenance is Task 5". The plan's Task 5a closes "the machine's `~/.config/codescout/.env` silently outranked every project's `[embeddings]`", and its 5b table lists no `_NAME` row.

## Hypotheses tried

1. **Hypothesis** — a dotenv-supplied `CODESCOUT_EMBEDDER_MODEL_NAME` shadows `[embeddings].model` with no warning. **Test** — read the five cited locations at `f5f1c6f7` (`RetrievalConfig::from_env_and_project` read the variable straight from `std::env`; `DotenvProvenance` had only `url`/`model`/`api_key`; `dotenv_shadowed_fields` could name only those three), then wrote tests that need the variable to be nameable. **Verdict** — confirmed. **Evidence** — the cited lines; the new tests do not compile before the fix (no `model_name` field) and, after it, each of the three conditions was mutated and killed (see Tests added). No end-to-end run on the real binary: the shadow decision is a pure predicate, and the warning is a `tracing::warn!` over its result.

## Fix

Implemented 2026-10-16 on branch `worktree-agent-a5de56cff4863b268`, commit `164e60d6` (not yet on `experiments`/master; re-cite the SHA after the merge).

- `src/retrieval/config.rs`: `EmbedEnv` gained `model_name` (read once at the edge, blank-is-absent) and `DotenvProvenance` gained `model_name` (true when the key is in `dotenv_injected_keys()`). `dotenv_shadowed_fields` names `"model_name"` under the same three conditions as the other fields, judged against the config layers' own `[embeddings].model`. `from_env_and_project` now takes `dense_model_name_override` from that same `EmbedEnv` read, so the wire value and the warning cannot diverge.
- `dotenv_shadow_warning(field)` owns the wording: `model_name` is printed as the variable shadowing `[embeddings].model`, not as a nonexistent `[embeddings].model_name` key, and its remedy does not say "move it to config.toml" (that would not carry the wire-name semantics).
- `src/config/embedding_env.rs`: module doc now says `_NAME` is deliberately not an alias in `MODEL` and where it is read.

Precedence is unchanged: the override still wins the wire name (pinned by the existing `build_http_embedder_lets_the_env_override_win_over_the_configured_model`).

## Tests added

All in `src/retrieval/config.rs`, `merge_tests`:

- `a_dotenv_model_name_that_overrides_a_configured_model_is_named`
- `an_exported_model_name_is_silent` (condition 1, provenance)
- `a_blank_dotenv_model_name_shadows_nothing` (condition 2, non-blank)
- `a_dotenv_model_name_with_no_configured_model_is_silent` (condition 3, config sets `model`)
- `model_name_provenance_is_independent_of_model_provenance`
- `the_model_name_warning_names_the_variable_not_a_config_key_that_does_not_exist`

Mutation checks, one per condition, each run and each killed: provenance replaced by `true` (2 tests red), configured-model operand replaced by a constant (1 red), non-blank check relaxed to `is_some()` (2 red, one of them the pre-existing url blank test). Mutants restored; `cargo test --lib -- retrieval::config config::` is green (218 passed).

## Workarounds

Export the variable in the MCP server's env block instead of the dotenv, where it is an intentional override.

## Resume

N/A — fixed. Archive to `docs/issues/archive/` once `164e60d6` (or its cherry-pick) is on master (`git branch --contains <sha>`).

## References

- `docs/plans/2026-09-17-embedding-config-consolidation.md`, the "Still owed" paragraph (line 127) and Task 5a (line 290).
- `docs/issues/archive/2026-09-17-the-configured-embedding-model-is-discarded-whenever-a-url-is-set.md`.
- Cluster `IC-14`: the warning is named for the shadowing of `[embeddings]`. It covers three of the four variables that shadow it.
