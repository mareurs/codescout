---
id: '2b33e87e487ea469'
kind: bug
status: fixed
title: 'BUG: a configured url silently discards a `local-dir:` model — the offline guarantee fails open, and the crate''s own hard-error guard is unreachable from the retrieval path'
owners:
- marius
tags:
- embeddings
- local-onnx
- offline
- config-precedence
- guard-unbound
topic: local-onnx-embedding
closed: 2026-08-13
---

## Summary

`codescout-embed` treats `url` + a `local-dir:` model as a **hard error** — the two
are contradictory (one selects a network client, the other forces offline). The
retrieval path never reaches that guard: `build_embedder` branches on `url` first
and takes the HTTP backend *regardless of model*. So on a host where any url is
configured — including one supplied silently by the global startup dotenv — a
`local-dir:` model is discarded without error, warning, or log line, and embedding
goes over the network.

This inverts the feature's core safety property. `local-dir:` exists so a restricted
host embeds **without touching the network**; here it fails open to the network
instead of failing closed.

## Symptom (Effect)

Config: `CODESCOUT_EMBEDDER_MODEL=local-dir:<weights>` plus any url.

With an **unreachable** url the failure is at least loud, and names the network —
proving the local weights were never consulted:

```
Error: dense embed connect failed: http://127.0.0.1:9/v1/embeddings — the dense embedder is unreachable (connect/timeout). Check CODESCOUT_EMBEDDER_URL and that the embedder is running (`./scripts/retrieval-stack.sh ps`). (error sending request for url (http://127.0.0.1:9/v1/embeddings))
```

Exit code 1. Note the expected error — "Cannot combine url with a local-dir: model"
(`crates/codescout-embed/src/lib.rs:182-186`) — never appears.

With a **reachable** url it is silent. Exit 0, no warning, and the index is built
with the remote model's dimension:

```
INFO codescout::retrieval::sync: retrieval sync starting chunk_target=1200 flush_batch=256 force_reindex=false backend="sqlite-vec" sparse="SKIPPED"
INFO codescout::retrieval::sync: retrieval sync finished added=5 deleted=0 elapsed_ms=118
added=5 updated=0 deleted=0 elapsed_ms=118
```

The resulting `vec0` table is the **remote** model's width, not AllMiniLM's 384:

```
CREATE VIRTUAL TABLE code_vec USING vec0(
                 chunk_id TEXT PRIMARY KEY,
                 embedding FLOAT[768]
             )
```

There is no `local weights: descended into the sole snapshot directory` line —
the local embedder was never constructed.

## Reproduction

Commit: `6e1fa4fa` on `feat/local-onnx-query-path`. Binary built with
`cargo build --release --features server-stack,local-embed`.

Silent case — this is the one that matters, and it needs no contrived setup on a
host whose `~/.config/codescout/.env` already carries a url:

```
env -i HOME=$HOME PATH=/usr/bin:/bin RUST_LOG=info \
  CODESCOUT_VECTOR_BACKEND=sqlite-vec \
  CODESCOUT_EMBEDDER_MODEL="local-dir:$PWD/.fastembed_cache/models--Xenova--all-MiniLM-L6-v2" \
  ./target/release/codescout index -p /path/to/probe
```

Even under `env -i`, `load_startup_env` re-reads `~/.config/codescout/.env` from
disk and supplies `CODESCOUT_EMBEDDER_URL` for the key the caller left unset —
so the operator's explicit `local-dir:` is overridden by a file they did not
mention. Exit 0, 768-dim index, no warning.

Loud case — same command with `CODESCOUT_EMBEDDER_URL=http://127.0.0.1:9/v1`
added, which produces the connect error quoted above.

Control (proves the local path itself is fine): add
`CODESCOUT_ENV_FILE=/nonexistent` to neutralize the dotenv, and the same command
logs `local weights: descended into the sole snapshot directory`, builds
`FLOAT[384]`, and stores 5 rows in 38 ms with no network.

## Environment

Linux 7.1.5-zen1-2-zen, Rust release build, `feat/local-onnx-query-path` @ `6e1fa4fa`.
Backend `sqlite-vec` (lite stack). Host has `~/.config/codescout/.env` symlinked to
this repo's `.env.amd`, which sets `CODESCOUT_EMBEDDER_URL` and `CODESCOUT_MODEL_DIM=768`.

## Root cause

Two independent mechanisms compose into the failure.

**1. The guard is unreachable from the retrieval path.** The contradiction check
lives in `crates/codescout-embed/src/lib.rs:180-186`, inside
`create_embedder_with_config`. `RetrievalClient::build_embedder`
(`src/retrieval/client.rs:131-163`) only calls that resolver in its `else` branch:

```
if let Some(url) = config.embedder_url.as_deref() {
    Ok(Arc::new(Self::build_http_embedder(url, config, dense_only)))
} else {
    let inner = codescout_embed::create_embedder_with_config(...)
```

Its own doc comment states the rule plainly — "a configured url always selects the
HTTP backend regardless of model". The url-wins routing is deliberate (design
Approach A, to preserve the connect-error marker `semantic_search` matches on);
what was never decided is that the *contradictory* combination should be silent.
The crate deliberately made it fatal, and the retrieval path simply never asks.

This is the same shape as F-1/W-2 in `docs/trackers/local-onnx-embedding-session-log.md`
— a guard that is correct, tested, and unbound to the call site that needs it.
`crates/codescout-embed/src/lib.rs:331-341` asserts the hard error at the crate
level and passes; nothing asserts it at the retrieval seam, so the gap is invisible
to the suite.

**2. The startup dotenv supplies the url the operator never set.**
`src/config/global.rs:109-116` applies dotenv precedence "already-set wins, unset
keys are filled". That is the right precedence, but it means an operator who sets
only `CODESCOUT_EMBEDDER_MODEL` inherits `CODESCOUT_EMBEDDER_URL` from
`~/.config/codescout/.env` — and mechanism 1 then makes that inherited url win
over the explicitly-passed model.

Measured 2026-08-13: both runs above executed on this host; the 768-dim table and
the connect error are copied from their actual output, not inferred.
`tests/cli_artifact.rs:18-24` already documents this dotenv trap for tests, which
is how the substrate was identified.

## Evidence

### Accidental silent reproduction

The first probe of this session set `local-dir:` under `env -i` and was believed
hermetic. It exited 0 and wrote `~/.codescout/embeddings/vdi-probe.db` with
`FLOAT[768]` and 5 rows in `code_vec_rowids`. Only the dimension revealed that the
local embedder had never run — the operator-visible output was indistinguishable
from success.

### Controlled confirmation

Re-run with an unreachable url produced `dense embed connect failed:
http://127.0.0.1:9/v1/embeddings`, exit 1 — the network was contacted despite
`local-dir:` naming on-disk weights.

### Control

With `CODESCOUT_ENV_FILE` pointed at a nonexistent path, the same invocation logged
`local weights: descended into the sole snapshot directory ... resolved=.../snapshots/751bff37182d3f1213fa05d7196b954e230abad9`,
created `FLOAT[384]`, stored 5 rows, 38 ms, no network.

## Hypotheses tried

1. **Hypothesis:** the 768-dim table meant sqlite-vec silently accepted mismatched
   384-dim vectors (silent corruption).
   **Test:** read `build_embedder`; check whether the local embedder was constructed
   at all (`resolve_weights_dir` INFO line absent).
   **Verdict:** rejected — no 384-dim vector was ever produced; the HTTP embedder ran
   and legitimately produced 768-dim vectors.

2. **Hypothesis:** `env -i` made the run hermetic, so the config came only from my flags.
   **Test:** `ls -la ~/.config/codescout/` → `.env -> /home/marius/work/claude/codescout/.env.amd`;
   read `src/config/global.rs` `load_startup_env`.
   **Verdict:** rejected — codescout re-reads the dotenv from disk after `env -i`.

3. **Hypothesis:** the documented crate guard fires and this is user error.
   **Test:** run with url + `local-dir:`; look for "Cannot combine url with a local-dir: model".
   **Verdict:** confirmed as the bug — that error never appears; `build_embedder`
   short-circuits before the resolver.

## Fix

Fixed on `experiments`-track branch `feat/local-onnx-query-path` at **`38e0980b`**.

`RetrievalClient::guard_local_model_with_url` (`src/retrieval/client.rs`) rejects a
configured url alongside a `local-dir:` model as a `RecoverableError` naming both
operands, and — because the url may well have arrived from the startup dotenv rather
than anything the operator typed — the hint points at `~/.config/codescout/.env` and
the `CODESCOUT_ENV_FILE` escape. It is called from `build_embedder` **before** the
`if let Some(url)` branch; checking after it would be unreachable, which is the whole
defect.

**Scoped to `local-dir:` only — `local:` is deliberately excluded.** The first draft
covered both prefixes and broke
`agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`:
`default_embed_model()` **is** `"local:AllMiniLML6V2Q"` (`src/config/project.rs:384-386`),
so "url configured, model left unset" — an ordinary remote deployment — resolves to a
`local:` model the operator never chose. Rejecting that would break every such
deployment. `local-dir:` has no default, so it is unambiguously intent.

The residual gap is recorded rather than hidden: **a url still silently wins over an
explicitly-chosen `local:` model.** Closing it needs provenance ("was this model the
default?") threaded from `merge_embed_config` (`src/retrieval/config.rs`) into
`RetrievalConfig`. That is a separate change and is not attempted here.

Note the original Fix plan in this file proposed reusing `backend_is_local`. That was
wrong: `backend_is_local` requires `embedder_url.is_none()`, so it is false precisely
when the conflict exists. Caught while reading the code, before writing it.
## Tests added

All in `src/retrieval/client.rs`, module `selection_tests`:

- `build_embedder_rejects_a_url_combined_with_a_local_dir_model` — the regression test.
  Asserts the error **class** (`RecoverableError`, so isError: false keeps sibling
  parallel tool calls alive) and that the message names **both** the url and the model.
- `build_embedder_accepts_a_url_with_the_defaulted_local_model` — the complement, and a
  regression guard for the break the first draft caused. It asserts its own premise
  (`default_embed_model() == "local:AllMiniLML6V2Q"`) so that changing the default
  forces a re-derivation rather than silently invalidating the reasoning.
- `build_embedder_still_accepts_a_url_with_an_ordinary_model` — no-false-positive guard.

**Mutation-verified.** Deleting the `Self::guard_local_model_with_url(config)?;` call
site does **not** produce an `is_err()` failure — execution falls through to the url
branch, builds an `EmbedderHttp`, and returns `Ok`. The test dies at its
`Ok(_) => panic!` arm instead. This matters: an `is_err()`-only assertion would have
passed against the buggy code, because the bug's signature was that **nothing errored**.
Re-run under mutation 2026-08-13: exactly 1 of 19 `selection_tests` fails, the other two
new tests survive (confirming they test the complement, not the same thing).

End-to-end on the release binary, 2026-08-13:

- the original silent invocation now exits 1 with the conflict error
- the hermetic offline invocation still indexes 5 chunks in 39 ms at `FLOAT[384]`

Full suite with real ONNX weights and no skip flag: 3704 passed, 0 failed, 49 ignored.
Clippy clean on `--workspace --all-targets --features local-embed`.
## Workarounds

- Set `CODESCOUT_ENV_FILE` to a nonexistent path when running an offline/local-dir
  configuration, which makes `load_startup_env` a silent no-op.
- Or explicitly clear the url: `CODESCOUT_EMBEDDER_URL=` (empty is treated as unset
  by `non_empty` in `src/retrieval/config.rs`).
- Verify rather than trust: after indexing, check the stored width with
  `select sql from sqlite_master where sql like '%vec0%';` — `FLOAT[384]` means the
  local ONNX path ran; anything else means it did not.

## Resume

N/A for the filed defect — fixed and verified at `38e0980b`.

One deliberate residual, if it is ever worth closing: a url still silently overrides an
**explicitly-configured** `local:` model, because the config cannot distinguish that from
the default. The change would be to have `merge_embed_config` (`src/retrieval/config.rs`)
report whether the model came from env / project.toml or from `default_embed_model()`,
carry that on `RetrievalConfig`, and widen the guard to
`!model_was_defaulted && model_names_local_backend(...)`.
`build_embedder_accepts_a_url_with_the_defaulted_local_model` will fail the moment the
guard widens without that provenance — which is the intended tripwire, not an obstacle.
## References

- `crates/codescout-embed/src/lib.rs:180-186` — the crate-level hard error
- `src/retrieval/client.rs:131-163` — `build_embedder`, where url short-circuits
- `src/config/global.rs:109-116` — dotenv precedence
- `tests/cli_artifact.rs:18-24` — prior art documenting the dotenv trap
- `docs/trackers/local-onnx-embedding-session-log.md` — F-1 / W-2, the guard-unbound pattern
- `docs/superpowers/specs/2026-08-11-local-onnx-embedding-query-path-design.md` — Approach A
