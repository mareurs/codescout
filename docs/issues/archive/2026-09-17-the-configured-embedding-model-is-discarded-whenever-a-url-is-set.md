---
id: aae547c917c329b5
kind: bug
status: fixed
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

Landed. `EmbedderHttp::new` gained a `dense_model_name: &str` parameter and **stopped
reading `CODESCOUT_EMBEDDER_MODEL_NAME`**; that read moved to
`RetrievalConfig::dense_model_name_override`, resolved by the pure
`RetrievalConfig::dense_model_name()`. Both production call sites
(`build_http_embedder`, `from_config_only`) pass it.

**The env read MOVED rather than merely gaining a fallback, and that is the fix.** A
fallback would have closed the symptom and left the cause: an env read inside a
constructor is only exercisable by mutating process env, which is UB against the suite's
concurrent `getenv` readers and banned by `docs/conventions/test-env-isolation.md` — so
the resolution had no reachable assertion, which is *why* it could be wrong for a
release behind a green suite. With the value on the config struct, both directions are
ordinary unit tests. The api_key sibling on the same constructor was already resolved at
the config edge and already had a test; that asymmetry is the whole story.

Prefix stripping moved to `codescout_embed::bare_model_name`, shared with
`create_embedder_with_config`'s url arm. Wiring config through the second path without
sharing the rule would have reproduced the defect one layer down — two paths agreeing
today and diverging on the next prefix added.

`CODESCOUT_EMBEDDER_MODEL_NAME` stays **on top**, per the 2026-09-17 ruling. Every stack
deployment sets it while leaving `[embeddings].model` at the built-in default, so letting
`model` win would have silently repointed all of them at `AllMiniLML6V2Q`. The shadowing
WARNING that makes the override visible is deliberately *not* here: `RetrievalConfig`
cannot yet distinguish a chosen `model` from a defaulted one, so a warning keyed on
difference alone would fire on every deployment that exists. That provenance is Task 5 of
`docs/plans/2026-09-17-embedding-config-consolidation.md`.

`require_model`'s message is unchanged and is **not** owed here: on this path it is now
unreachable, because `RetrievalConfig.model` always carries a value. It stays live for
`RemoteEmbedder::from_url`'s other callers.

- **SHA:** `654e1f187c63c79673c771ea25890bf517da6f43` (on `experiments`)
- **patch-id:** `2a456998545511824432cf69d9351f5af434b2ff`

## Tests added

All in `src/retrieval/client.rs` `selection_tests`, asserted through
`build_http_embedder` — the function `build_embedder`/`from_env` actually run — via a new
`EmbedderHttp::dense_model_name_for_test()` mirroring the existing `api_key_for_test()`.
Deliberately **not** through `EmbedderHttp::with_config`, whose every caller in the tree
is a test and which therefore covers no production path.

- `build_http_embedder_sends_the_configured_model_when_no_override_is_set` — the defect
  itself. Observed RED before the fix.
- `build_http_embedder_lets_the_env_override_win_over_the_configured_model` — pins the
  back-compat half so a later change cannot invert the ladder while the first test still
  passes. Observed RED before the fix.
- `a_routing_prefix_is_stripped_before_the_model_goes_on_the_wire` — observed RED before
  the fix, **but for the wrong reason**: it failed because the override was winning, not
  because stripping was absent, so that red was evidence for its siblings' claim and not
  its own. Settled by mutation instead — `scripts/mutation-probe.sh` replacing
  `bare_model_name`'s body with `model` reports KILLED (`local:AllMiniLML6V2Q` vs
  `AllMiniLML6V2Q`), so the assertion discriminates the stripping specifically.

Each test sets `dense_model_name_override` explicitly rather than inheriting it. This is
load-bearing, not hygiene: the ambient environment carries
`CODESCOUT_EMBEDDER_MODEL_NAME` on every host configured for the stack, so an inherited
override would make these tests assert nothing on exactly the machines where the defect
lives. That is the failure mode `tests/embedder_env_isolation.rs` records from the CI
side (`9c03b32f`, 36 hours red for CI and green for every developer).

**End-to-end, beyond the unit tests** — the probe that found the defect, re-run against
the fixed binary:

```json
{"path": "/v1/embeddings", "model": "DISTINCTIVE-MODEL-FROM-PROJECT-TOML",
 "auth": "Bearer KEY-FROM-PROJECT-TOML"}
```

Plus both back-compat directions: an env-driven stack config with no `[embeddings]` block
still sends `CodeRankEmbed-Q4_K_M.gguf`, and an exported-but-blank override falls back to
the configured value rather than winning.

## Workarounds

Set `CODESCOUT_EMBEDDER_MODEL_NAME` to the model name your endpoint expects,
in `~/.config/codescout/.env` or the MCP server's env block. This is what every
working deployment in this repo already does — which is why the defect survived:
`.env.amd`, `.env.gpu`, `.env.cpu` and `contrib/pi/mcp.json.example` all set it,
so no configuration in the tree exercises the broken path.

## Resume

N/A — fixed and verified.

The surrounding work continues in
`docs/plans/2026-09-17-embedding-config-consolidation.md`; the next task is the shared
`EmbeddingSettings` type (bug `da26a27026bb9f41`), and the shadowing warning this fix
deliberately defers is Task 5.

## References

- `src/retrieval/client.rs:231-250`, `:312-348`
- `src/retrieval/embedder.rs:155`, `:285`, `:316`, `:344`, `:461-464`
- `crates/codescout-embed/src/remote.rs:313-325`
- `crates/codescout-embed/src/lib.rs:197-326`
- `docs/superpowers/specs/2026-03-31-unified-embedding-config-design.md` § 1
