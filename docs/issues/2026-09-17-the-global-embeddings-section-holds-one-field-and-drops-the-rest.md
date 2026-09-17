---
id: da26a27026bb9f41
kind: bug
status: open
title: 'BUG: the global [embeddings] section holds one field and silently drops the rest'
tags:
- cluster/accepted-parameter-silently-dropped
- embeddings
- config
- global-config
---

## Summary

`GlobalEmbeddingsSection` (`src/config/global.rs:16-19`) declares exactly one
field, `model`. A user who writes `url` or `api_key` into
`~/.config/codescout/config.toml` gets them **silently discarded** by serde
before the global layer is merged — no warning, no unknown-key diagnostic. The
resulting failure then advises the user to set the very field they set.

## Symptom (Effect)

`~/.config/codescout/config.toml`:

```toml
[embeddings]
model = "MODEL-FROM-GLOBAL-CONFIG"
url = "http://127.0.0.1:48099/v1"
api_key = "KEY-FROM-GLOBAL-CONFIG"
```

with no `[embeddings]` in the project at all:

```
Error: could not build the 'MODEL-FROM-GLOBAL-CONFIG' embedder:
Unknown model 'MODEL-FROM-GLOBAL-CONFIG'. Options:
• Set url in [embeddings] to point at any OpenAI-compatible server
```

The `model` was read — it is quoted back in the error. `url` and `api_key` were
not. Zero requests reach the endpoint. The first remedy the error offers is
*"Set url in [embeddings]"*, which is what the user just did, one line below the
model it successfully read.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199`, release binary 2026-09-17.

```bash
mkdir -p $SP/xdg/codescout $SP/proj/.codescout $SP/proj/src
printf 'pub fn f() -> u32 { 42 }\n' > $SP/proj/src/lib.rs
printf '[project]\nname = "probeproj"\n' > $SP/proj/.codescout/project.toml
cat > $SP/xdg/codescout/config.toml <<'TOML'
[embeddings]
model = "MODEL-FROM-GLOBAL-CONFIG"
url = "http://127.0.0.1:48099/v1"
api_key = "KEY-FROM-GLOBAL-CONFIG"
TOML
: > $SP/empty.env
env -u CODESCOUT_EMBEDDER_URL -u CODESCOUT_EMBEDDER_MODEL_NAME \
  XDG_CONFIG_HOME=$SP/xdg CODESCOUT_ENV_FILE=$SP/empty.env \
  CODESCOUT_DISABLE_SPARSE=1 codescout index --project $SP/proj --force
```

## Environment

Linux 7.2.4-zen2-1-zen, `experiments`, release binary built by `cargo rb`.

## Root cause

`GlobalConfig::load_from_dir` (`src/config/global.rs:168-197`) deserialises into
`GlobalConfig`, whose `embeddings` is `GlobalEmbeddingsSection` — a struct with a
single `model: Option<String>` field (`src/config/global.rs:16-19`). Unknown TOML
keys are dropped at that point. `GlobalConfig::to_toml_value`
(`src/config/global.rs:210-212`) then re-serialises **the struct**, so only
`model` can possibly survive into the `global_base` that
`ProjectConfig::load_with_global_base` (`src/config/project.rs:517`) merges.

The merge itself is not at fault and is in fact better than documented:
`merge_toml` (`src/config/project.rs:461-476`) recurses, and
`merge_toml_base_fills_missing_key` (`:1222-1228`) pins that `[embeddings]`
merges field-by-field. The global layer is therefore *structurally* a strict
subset of the project layer — `EmbeddingsSection`
(`src/config/project.rs:80-158`) carries `model`, `url`, `api_key`,
`max_inflight`, `file_group_size`; the global twin carries one of the five.

**Measured 2026-09-17**: the repro above, run against a Python
`/v1/embeddings` server that logs every request — the server received nothing,
and the error quoted the global `model` back while ignoring the sibling keys.

## Evidence

### Global `model` survives, global `url` does not

The error string contains `'MODEL-FROM-GLOBAL-CONFIG'`, proving the global file
was found, parsed, and merged. The request log is empty, proving `url` never
reached `RetrievalConfig.embedder_url`.

### The asymmetry is visible in the type

```
src/config/global.rs:16-19   GlobalEmbeddingsSection { model }
src/config/project.rs:80-158 EmbeddingsSection { model, url, api_key,
                                                 max_inflight, file_group_size }
```

### The docs make it worse

`docs/manual/src/configuration/global-config.md:31-32` gives a global
`[embeddings]` example whose model value, `jinaai/jina-embeddings-v2-base-code`,
is not a resolvable model string in the current grammar
(`crates/codescout-embed/src/local.rs:234-255` lists the seven accepted names).
So the page's only embeddings example fails even for the one field that works.

## Hypotheses tried

1. **Hypothesis:** serde would reject the unknown keys loudly.
   **Test:** ran the repro; read `load_from_dir`.
   **Verdict:** rejected — there is no `deny_unknown_fields`, and
   `src/config/project.rs:834` documents the deliberate opposite policy for the
   project layer ("the stale key was skipped rather than the whole section being
   dropped"). Tolerance is intended; the absence of a *warning* is the defect.

## Fix

Plan, not yet implemented. Make the global layer the **same type** as the
project layer — one `EmbeddingsSection` with all-`Option` fields, used at both
levels — so the two cannot drift again by construction. This is the shape the
deep-merging `merge_toml` already assumes. Two real implementors exist (global,
project), so the shared type is earned rather than speculative.

Independently: emit a one-time warning listing unknown keys under
`[embeddings]` in either layer, so a typo or an unsupported key is visible
rather than inferred from a downstream failure.

SHA / patch-id: pending.

## Tests added

None yet. Owed: a test asserting a global `url` reaches
`RetrievalConfig.embedder_url` when the project sets none — asserted on the
resolution path, not on a re-implementation of the merge.

## Workarounds

Put `url` and `api_key` in each project's `.codescout/project.toml`, or export
`CODESCOUT_EMBEDDER_URL` / `EMBED_API_KEY` (e.g. via
`~/.config/codescout/.env`). Note that the env route has its own defect — it
outranks every project's own config; see the sibling bug.

## Resume

Replace `GlobalEmbeddingsSection` with a reuse of `EmbeddingsSection` whose
fields are all `Option`, adjusting `EmbeddingsSection::model` from `String` to
`Option<String>` and moving `default_embed_model()` to the point of resolution
rather than deserialisation. Check `GlobalConfig::to_toml_value`'s
`skip_serializing_if` behaviour still emits only set fields
(`src/config/global.rs:350-373` pins that).

## References

- `src/config/global.rs:16-19`, `:168-197`, `:210-212`
- `src/config/project.rs:80-158`, `:461-476`, `:517`, `:1222-1228`
- `docs/manual/src/configuration/global-config.md:31-32`, `:47-52`
