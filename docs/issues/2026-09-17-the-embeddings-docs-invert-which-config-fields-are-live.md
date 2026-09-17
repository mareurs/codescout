---
id: efd14d6c5eb56905
kind: bug
status: open
title: 'BUG: the embeddings docs invert which config fields are live'
tags:
- cluster/doc-contradicted-by-code
- embeddings
- docs
- config
---

## Summary

`docs/manual/src/configuration/embeddings.md` tells readers that of the three
`[embeddings]` fields, only `model` is honoured and `url` / `api_key` are dead.
The truth is the exact inverse: `url` and `api_key` both work end-to-end, and
`model` is the one silently discarded on the url path. A reader who follows the
page configures the one field that does nothing and avoids the two that work.
Two further claims on neighbouring pages are also false in the same direction.

## Symptom (Effect)

`docs/manual/src/configuration/embeddings.md`, banner:

```
> **⚠ This page describes the pre-v0.12 single-service embedding model and is
> being phased out.** ... The `[embeddings]` config block still loads but only
> the `model = "local:..."` path is honoured — and only when the binary was
> built with the `local-embed` Cargo feature.
>
> **If you are upgrading from <v0.12:** the `model` / `url` / `api_key`
> fields in `project.toml` no longer drive search.
```

Measured behaviour on that exact block, 2026-09-17, against a logging
`/v1/embeddings` server:

| field | page says | measured |
|---|---|---|
| `[embeddings].url` | does not drive search | **drives search** — selects the endpoint |
| `[embeddings].api_key` | does not drive search | **drives search** — arrives as `Bearer KEY-FROM-PROJECT-TOML` |
| `[embeddings].model` | the only honoured field | **discarded whenever `url` is set** |

The banner is simultaneously the most prominent text on the page and wrong in
both directions.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199`. Set all three fields in
`.codescout/project.toml`, point `url` at a server that logs request bodies, run
`codescout index --force` with `CODESCOUT_ENV_FILE` neutralised. Observed wire:

```json
{"path": "/v1/embeddings", "model": "M", "auth": "Bearer KEY-FROM-PROJECT-TOML"}
```

— where `M` came from `CODESCOUT_EMBEDDER_MODEL_NAME` and `[embeddings].model`
was `DISTINCTIVE-MODEL-FROM-PROJECT-TOML`.

## Environment

Linux 7.2.4-zen2-1-zen, `experiments`, release binary built by `cargo rb`.

## Root cause

Doc-vs-code drift accumulated across the v0.12 retrieval-stack migration. The
page was updated to warn that `[embeddings]` was superseded, at a moment when
that was true; `url` and `api_key` were subsequently re-wired into the stack
path (`merge_embed_config`, `src/retrieval/config.rs:304-327`;
`RetrievalClient::guarded_api_key`, `src/retrieval/client.rs:209-223`) and the
warning was never revisited. `model` went the other way and was never re-wired.

Two more claims in the same family:

1. `docs/manual/src/configuration/global-config.md:47-52` — *"There is no
   deep-merge within nested tables — if a project config sets `[embeddings]`,
   the entire `[embeddings]` table from the global config is replaced, not
   merged field-by-field."* `merge_toml` (`src/config/project.rs:461-476`)
   recurses, and `merge_toml_base_fills_missing_key` (`:1222-1228`) pins
   field-by-field merging of `[embeddings]` specifically. The doc states the
   opposite of a behaviour a test guards — and it is the behaviour users
   actually want, so the page discourages the working pattern.
2. `src/config/project.rs:82` — the `EmbeddingsSection::model` doc comment opens
   `"ollama:<model>" → Ollama local daemon (default)`. The default is
   `local:AllMiniLML6V2Q` (`default_embed_model`, `:389-391`), which the same
   comment also marks `**default**` eight lines later. Two conflicting
   `(default)` markers in one doc block.

**Measured 2026-09-17** by the wire captures above and by reading
`merge_toml_base_fills_missing_key`; the doc claims were read at the cited lines
in tree `35b622f8`.

## Evidence

### api_key from project.toml reaches the wire

```json
{"path": "/v1/embeddings", "model": "M", "auth": "Bearer KEY-FROM-PROJECT-TOML"}
```

Run with no `EMBED_API_KEY` in the environment and `api_key` set only in
`.codescout/project.toml`.

### The only page documenting the working api_key path disclaims itself

Occurrence counts across the four embedding-related manual pages, 2026-09-17:

```
embedding-backends.md    api_key=0  EMBED_API_KEY=2  OPENAI_API_KEY=3
embeddings.md            api_key=4  EMBED_API_KEY=4  OPENAI_API_KEY=2
global-config.md         api_key=0  EMBED_API_KEY=0  OPENAI_API_KEY=0
retrieval-stack.md       api_key=0  EMBED_API_KEY=1  OPENAI_API_KEY=1
```

`embeddings.md` is the only page that documents `[embeddings].api_key` at all —
and it is the page whose banner says to treat its contents as historical. The
working authentication path is documented exclusively on the surface that
disavows it.

## Hypotheses tried

1. **Hypothesis:** the banner is stale but harmless, since the stack is
   env-configured anyway.
   **Test:** followed the page's own advice on a fresh project.
   **Verdict:** rejected — the advice is actively harmful. It steers a user to
   set `model` (inert on the url path) and to avoid `url`/`api_key` (the two
   that work), which is precisely the configuration that fails with
   *"embedding model name is required and was empty or blank"*.

## Fix

Documentation-only for this file; the code defects are filed separately and
their fixes will change what the corrected text should say. Sequence matters —
correct the docs **after** the model-discard fix lands, or the page will be
rewritten twice.

1. Rewrite `embeddings.md`'s banner to describe the real surface.
2. Correct `global-config.md` § Merge semantics to state deep-merge, and replace
   its non-resolvable `jinaai/jina-embeddings-v2-base-code` example.
3. Fix the duplicated `(default)` marker in `src/config/project.rs:82`.
4. Document the API-key path on a page that is not self-disclaiming.

SHA / patch-id: pending.

## Tests added

None yet. A prose page cannot be pinned without redding on every rewording
(`CLAUDE.md` § Testing Discipline). What **is** cheaply assertable and reds on
exactly the regression that happened: that `merge_toml`'s deep-merge behaviour
and the global-config page's description of it do not contradict — e.g. an
`audit_doc_refs`-style check, or at minimum keeping
`merge_toml_base_fills_missing_key` cited from the page so a reader lands on the
executable statement.

## Workarounds

Ignore `embeddings.md`'s banner. Set `url` and `api_key` in
`.codescout/project.toml`; set the model via `CODESCOUT_EMBEDDER_MODEL_NAME`
until the model-discard bug is fixed.

## Resume

Hold until the model-discard fix lands, then rewrite `embeddings.md` against the
then-current resolution ladder. `global-config.md` § Merge semantics and the
`src/config/project.rs:82` marker can be corrected immediately — neither depends
on the code fix.

## References

- `docs/manual/src/configuration/embeddings.md` (banner)
- `docs/manual/src/configuration/global-config.md:31-32`, `:47-52`
- `src/config/project.rs:82`, `:389-391`, `:461-476`, `:1222-1228`
- `src/retrieval/config.rs:304-327`; `src/retrieval/client.rs:209-223`
