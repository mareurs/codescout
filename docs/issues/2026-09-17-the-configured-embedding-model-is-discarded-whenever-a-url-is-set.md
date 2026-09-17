---
id: a9c8df45aca1ede5
kind: bug
status: taken
title: 'BUG: the configured embedding model is discarded whenever a url is set'
tags:
- cluster/accepted-parameter-silently-dropped
- embeddings
- retrieval
- config
claimed_at: 2026-09-17
claimed_by: 458a8a26-c380-4f5b-b967-2181f592917e
---

## Summary

Whenever `[embeddings].url` (or `CODESCOUT_EMBEDDER_URL`) is set, the resolved
embedding **model name is discarded** and the value that goes on the wire comes
from one env var only: `CODESCOUT_EMBEDDER_MODEL_NAME`. The four layers that
compete to set the model — the built-in default, global `config.toml`,
`.codescout/project.toml`, and both `CODESCOUT_EMBED_MODEL` /
`CODESCOUT_EMBEDDER_MODEL` — all resolve correctly into `RetrievalConfig.model`
and are then dropped on the floor. With the env var unset, the documented
configuration fails outright and the error names none of the five knobs.

## Symptom (Effect)

`.codescout/project.toml` carrying the documented `model` + `url` pair:

```toml
[embeddings]
model = "DISTINCTIVE-MODEL-FROM-PROJECT-TOML"
url = "http://127.0.0.1:48099/v1"
```

```
Error: embedder rejected a minimal probe after every chunk in the batch failed —
treating it as unhealthy and aborting, rather than skipping real content

Caused by:
    embedding model name is required and was empty or blank — an
    OpenAI-compatible /v1/embeddings request carries `model` unconditionally, so
    an empty value goes on the wire as `"model": ""` ...
```

No HTTP request reaches the endpoint at all. The message is accurate about the
*state* and names no **knob**: not `[embeddings].model`, which is set two lines
above the `url` in the same file, and not `CODESCOUT_EMBEDDER_MODEL_NAME`, which
is the only thing that would fix it.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199` (`experiments`), release binary
built 2026-09-17 07:10. `$SP` is any scratch dir.

```bash
mkdir -p $SP/proj/.codescout $SP/proj/src
printf 'pub fn f() -> u32 { 42 }\n' > $SP/proj/src/lib.rs
cat > $SP/proj/.codescout/project.toml <<'TOML'
[project]
name = "probeproj"
[embeddings]
model = "DISTINCTIVE-MODEL-FROM-PROJECT-TOML"
url = "http://127.0.0.1:48099/v1"
TOML
: > $SP/empty.env

# neutralise the startup dotenv, which otherwise supplies CODESCOUT_EMBEDDER_*
env -u CODESCOUT_EMBEDDER_MODEL_NAME -u CODESCOUT_EMBEDDER_URL \
    -u CODESCOUT_EMBEDDER_MODEL -u CODESCOUT_EMBED_MODEL -u CODESCOUT_EMBED_URL \
  CODESCOUT_ENV_FILE=$SP/empty.env CODESCOUT_DISABLE_SPARSE=1 \
  codescout index --project $SP/proj --force
```

Neutralising the dotenv is **load-bearing** in the repro and is itself the
second defect here: on this host `~/.config/codescout/.env` is a symlink to the
repo's `.env.amd`, which sets `CODESCOUT_EMBEDDER_URL` — so without
`CODESCOUT_ENV_FILE` the project's own `url` never wins and the bug is masked by
a *different* one (see the sibling bug on layer precedence).

## Environment

Linux 7.2.4-zen2-1-zen, branch `experiments`, `codescout index` CLI path (same
`RetrievalClient::build_embedder` the MCP server uses). Release binary, feature
set as built by `cargo rb` (includes `server-stack`, `remote-embed`).

## Root cause

`RetrievalClient::build_embedder` (`src/retrieval/client.rs:312-348`) branches on
`config.embedder_url`. The url arm calls `build_embedder_for_url` →
`build_http_embedder` (`src/retrieval/client.rs:231-250`), which constructs
`EmbedderHttp::new(url, &config.sparse_embedder_url, config.model_dim...)` —
**a three-argument constructor with no model parameter**. `config.model` is in
scope and never passed.

`EmbedderHttp::new` (`src/retrieval/embedder.rs:285`) therefore sources the name
itself: `let dense_model_name = std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").unwrap_or_default();`
and that field reaches `RemoteEmbedder::from_url(&self.dense_base, &self.dense_model_name, ...)`
at `src/retrieval/embedder.rs:461-464`. `require_model`
(`crates/codescout-embed/src/remote.rs:313-325`) rejects the resulting blank.

`dense_model_name` has exactly four occurrences in `src/retrieval/embedder.rs`
(155, 285, 316, 344) — enumerated, not sampled — and the env read at 285 is the
only write on the `new()` path. The non-env seam `with_config` takes the name as
a parameter, but **every** caller of `with_config` is a test: the only two
production callers in the tree are `EmbedderHttp::new` at
`src/retrieval/client.rs:232` and `:406`.

This is a half-finished migration, not a coding slip. The 2026-03-31 design
(`docs/superpowers/specs/2026-03-31-unified-embedding-config-design.md` § 1)
specifies *"`url` is set → `RemoteEmbedder` targeting that URL. `model` is the
model name sent in the request body"* — which is exactly what
`create_embedder_with_config` (`crates/codescout-embed/src/lib.rs:197-326`, arm 1)
still does. The v0.12 retrieval-stack work inserted `EmbedderHttp` in front of
that resolver for the url case, to gain the sparse leg; the branch moved and the
model did not follow.

**Measured 2026-09-17** (not inferred): three runs of the repro above against a
Python `/v1/embeddings` server logging each request body — blank env var → hard
failure and zero requests; `CODESCOUT_EMBEDDER_MODEL_NAME=MODEL-FROM-ENV-VAR` →
`added=1` and the wire carries the env var while `project.toml`'s model appears
nowhere.

## Evidence

### Wire capture, env var set, project.toml model ignored

```json
{"path": "/v1/embeddings", "model_field_present": true,
 "model": "MODEL-FROM-ENV-VAR", "auth": null, "n_inputs": 1}
```

`project.toml` said `model = "DISTINCTIVE-MODEL-FROM-PROJECT-TOML"` for this run.
The env var is the only difference between this run and the failing one.

### The seam that exists for api_key has no model twin

`src/retrieval/client.rs:966-980` holds
`build_http_embedder_never_sends_a_configured_key_over_plaintext_http`, which
inspects the constructed embedder via `http.api_key_for_test()`. There is no
`dense_model_name_for_test()` and no assertion anywhere that the configured model
reaches the wire on the url path — so the property is covered **zero** times, not
weakly. Grep for `dense_model_name` in `src/retrieval/client.rs`: no matches.

## Hypotheses tried

1. **Hypothesis:** `[embeddings].model` is simply unsupported on the stack path
   by design, as `docs/manual/src/configuration/embeddings.md` states.
   **Test:** read that page against the merge code.
   **Verdict:** rejected — the page says only `model` is honoured and `url` /
   `api_key` are not; the code does the exact opposite. `merge_embed_config`
   (`src/retrieval/config.rs:304-327`) resolves all three, and runtime confirms
   `url` and `api_key` both work. The doc is inverted (separate bug).
2. **Hypothesis:** the first repro failure was the project's `url` not being
   read.
   **Test:** re-ran with `CODESCOUT_ENV_FILE` pointed at an empty file.
   **Verdict:** confirmed as a *different* defect — the startup dotenv was
   supplying `CODESCOUT_EMBEDDER_URL` and outranking `project.toml`. Filed
   separately; it masks this one.

## Fix

Plan, not yet implemented. Give `EmbedderHttp::new` the model name as a
parameter and pass `config.model` from `build_http_embedder`
(`src/retrieval/client.rs:232`), keeping `CODESCOUT_EMBEDDER_MODEL_NAME` as a
deprecated override that warns when it shadows a configured value. `with_config`
already has the parameter, so the change is to `new()`'s signature and its two
production call sites.

The error text from `require_model` should also name the knob — it is correct
about the state and unactionable about the remedy, which is the
`CLAUDE.md` § *Testing Discipline* "loudness is a property of a path" rule: the
alarm fires, reaches the right person, and sends them nowhere.

SHA / patch-id: pending.

## Tests added

None yet. Owed, and the shape matters: a `dense_model_name_for_test()` accessor
plus an assertion on the **production** path (`build_http_embedder`), mirroring
the existing api_key test rather than re-implementing the resolution in the test.
An assertion over `with_config` would be testing the seam no production caller
uses.

## Workarounds

Set `CODESCOUT_EMBEDDER_MODEL_NAME` to the model name your endpoint expects,
in `~/.config/codescout/.env` or the MCP server's env block. This is what every
working deployment in this repo already does — which is why the defect survived:
`.env.amd`, `.env.gpu`, `.env.cpu` and `contrib/pi/mcp.json.example` all set it,
so no configuration in the tree exercises the broken path.

## Resume

Add `dense_model_name_for_test()` beside `api_key_for_test()` in
`src/retrieval/embedder.rs`, then a test in `src/retrieval/client.rs`'s test
module asserting `RetrievalClient::build_http_embedder(url, &cfg, false)` carries
`cfg.model`. Observe it RED first, then thread the parameter through
`EmbedderHttp::new`.

## References

- `src/retrieval/client.rs:231-250`, `:312-348`
- `src/retrieval/embedder.rs:155`, `:285`, `:316`, `:344`, `:461-464`
- `crates/codescout-embed/src/remote.rs:313-325`
- `crates/codescout-embed/src/lib.rs:197-326`
- `docs/superpowers/specs/2026-03-31-unified-embedding-config-design.md` § 1
