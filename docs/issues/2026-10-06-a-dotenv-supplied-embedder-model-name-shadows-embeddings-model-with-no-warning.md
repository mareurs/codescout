---
id: '95c954f2a7bd3f2d'
kind: bug
status: open
title: 'BUG: a dotenv-supplied CODESCOUT_EMBEDDER_MODEL_NAME silently outranks [embeddings].model, and the shadowing warning does not cover it'
tags:
- embeddings
- config
- dotenv
- cluster/guard-narrower-than-its-name
opened: 2026-10-06
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

None yet. A runtime check is the next step.

## Fix

Not started. Options:

- Record dotenv provenance for `CODESCOUT_EMBEDDER_MODEL_NAME` in `EmbedEnv` and add `"model_name"` to `dotenv_shadowed_fields`. The fire condition stays the same: from the dotenv, non-blank, and the config layers set `model`.
- Or state in the plan that the override is exempt, and say why.

Precedence must not change: the ruling keeps the override on top.

## Tests added

N/A — not fixed. A fix needs a test on the pure predicate, as the five existing ones, and a mutation check per condition.

## Workarounds

Export the variable in the MCP server's env block instead of the dotenv, where it is an intentional override.

## Resume

Run the reproduction on the real binary first. Then decide between the two fixes.

## References

- `docs/plans/2026-09-17-embedding-config-consolidation.md`, the "Still owed" paragraph (line 127) and Task 5a (line 290).
- `docs/issues/archive/2026-09-17-the-configured-embedding-model-is-discarded-whenever-a-url-is-set.md`.
- Cluster `IC-14`: the warning is named for the shadowing of `[embeddings]`. It covers three of the four variables that shadow it.
