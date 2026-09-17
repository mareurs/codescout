---
id: 4cd387ba07bc8d2a
kind: bug
status: fixed
title: 'BUG: the global [embeddings] section holds one field and silently drops the rest'
tags:
- cluster/accepted-parameter-silently-dropped
- embeddings
- config
- global-config
claimed_at: 2026-09-17
claimed_by: 458a8a26-c380-4f5b-b967-2181f592917e
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

Landed. `GlobalEmbeddingsSection` is now a **type alias** for
`crate::config::project::EmbeddingsSection` — not a second struct kept in step, because
"kept in step" is exactly what failed here. Two structs describing one config block
drift in one direction and silently: the level a developer is editing gains the field,
the other does not, and neither the type system nor the tests object. One type cannot
drift from itself.

`EmbeddingsSection::model` became `Option<String>`, and that was **forced, not
cosmetic**. `serde(default)` fires for an ABSENT `[embeddings]` table, so with the old
`#[serde(default = "default_embed_model")]` the now-shared struct would have made every
global config serialise `model = "local:AllMiniLML6V2Q"` into the merge base — a global
layer asserting a model nobody wrote, pinning every project to it. That would have been
a strictly worse bug than the one being fixed, produced BY the fix. The default moved to
resolution: `EmbeddingsSection::model_or_default()` and `merge_embed_config`.

The merge beneath was never at fault and is better than documented: `merge_toml` recurses
and `merge_toml_base_fills_missing_key` pins field-by-field merging of `[embeddings]`
specifically. Only the struct above it could not express the fields.

**Not done here, and listed rather than dropped:** the unknown-key warning. With the
shared type the remaining silent drop is a genuinely unknown key — a typo like
`modle = …` — which is a different residual from the one this bug names, and it needs a
warn-once surface plus the provenance channel that Task 5 of
`docs/plans/2026-09-17-embedding-config-consolidation.md` introduces anyway. Building
that surface here would mean building it twice.

- **SHA:** `de0a1e0703a79a458cd0293a225ce7cb970d8cb7` (on `experiments`)
- **patch-id:** `95e9301f095b51cd862e81f304a1f64d52dd8483`

## Tests added

Two, in `src/retrieval/config.rs` `merge_tests`:

- `a_global_url_and_key_survive_the_round_trip_into_the_resolved_config` — a global
  `url` and `api_key` reach the resolved config while the project's own `model` still
  wins.
- `the_global_layer_fills_a_gap_inside_the_project_embeddings_table` — the project sets
  `model` and inherits the global `url` sitting beside it in the same table. This is the
  half a per-field fix would miss, and the behaviour
  `docs/manual/src/configuration/global-config.md` § *Merge semantics* denies in writing.

**Both were written AFTER the fix, so neither had an observed red.** That is a real gap
in the evidence, not a formality, and it was closed by mutation rather than by asserting
harder — once per guarded **field**, since `url` and `api_key` are separate sites and one
probe would have proved only one of them:

| mutation (`scripts/mutation-probe.sh`, isolated worktree) | result |
|---|---|
| `skip_serializing` on `EmbeddingsSection::url` | **KILLED** — both tests, `left: None` |
| `skip_serializing` on `EmbeddingsSection::api_key` | **KILLED** — only the assertion naming it |

`left: None` is the exact historical symptom, and the second probe leaving the
gap-filling test green shows each assertion catches its own field rather than the pair
riding on one. The twelve pre-existing merge tests stayed green under both mutations,
confirming they were isolated to the global round trip.

**One design point worth keeping, because the obvious version of this test is vacuous:**
both route through `GlobalConfig::load_from_dir` + `to_toml_value`, never a hand-built
`toml::Value`. The defect lived in the **type** — serde discarded the keys at parse time
and `to_toml_value` re-serialised the struct — so a test that constructs the merge base
directly bypasses the broken component entirely and passes both before and after the fix,
asserting only about `merge_toml`, which was never broken.

**End-to-end**, against a logging `/v1/embeddings` server:

```
global: model+url+api_key   project: no [embeddings]  -> all three on the wire
                                                         (before: "Unknown model", 0 requests)
global: model+url+api_key   project: model only       -> model from project,
                                                         api_key from global
```

## Workarounds

Put `url` and `api_key` in each project's `.codescout/project.toml`, or export
`CODESCOUT_EMBEDDER_URL` / `EMBED_API_KEY` (e.g. via
`~/.config/codescout/.env`). Note that the env route has its own defect — it
outranks every project's own config; see the sibling bug.

## Resume

N/A — fixed and verified.

The unknown-key warning noted under *Fix* is carried on Task 5 of
`docs/plans/2026-09-17-embedding-config-consolidation.md`, not on this bug.

## References

- `src/config/global.rs:16-19`, `:168-197`, `:210-212`
- `src/config/project.rs:80-158`, `:461-476`, `:517`, `:1222-1228`
- `docs/manual/src/configuration/global-config.md:31-32`, `:47-52`
