# Embedding without a server

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

codescout can embed in-process with a small ONNX model — no GPU, no embedding
server, no daemon. This is the recommended setup for a laptop, a CI box, or any
host that cannot reach an embedding endpoint.

## Build with the local backend

Local ONNX is opt-in at compile time:

```bash
cargo build --release --features local-embed
```

On windows-gnu (MinGW), `ort` ships no prebuilt runtime — use
`--features local-embed-dynamic` and supply `onnxruntime.dll` at runtime via
`ORT_DYLIB_PATH`.

Check what your binary actually has:

```
workspace(action="status")   →   embedding_compiled_in
```

If that reads `["remote"]`, the binary cannot embed locally no matter what the
config says — `embedding_backend` will read `"unavailable"` and
`embedding_hint` explains the fix (rebuild with `local-embed`, or point
`[embeddings].url` at an OpenAI-compatible endpoint instead).

## Configure the model

```toml
# .codescout/project.toml
[embeddings]
model = "local:AllMiniLML6V2Q"   # 384d, INT8-quantized, ~22MB
```

On first use this fetches the weights to the HuggingFace cache. **On the
default lite build** (`cargo build --release --features local-embed`, no
`server-stack` feature) nothing else is needed — leave `url` unset, and both
`semantic_search` and `memory(recall)` will embed in-process.

**On a `server-stack` build** (`cargo rb --features local-embed` — this
project's own local build), that is not the whole story: `VectorBackend::resolve()`
defaults to Qdrant instead of the in-process `sqlite-vec` store, and with the hybrid sparse leg
enabled a local embedder — which produces no sparse vector — fails at client
construction instead of embedding in-process. Two ways to make a local model
work on a `server-stack` build: set `CODESCOUT_VECTOR_BACKEND=sqlite-vec` to
use the lite store, or keep Qdrant and set `CODESCOUT_DISABLE_SPARSE=1` to run
dense-only.

### Env vars can override `.codescout/project.toml` — and there are two of them

Two independently-named environment variables can override `[embeddings].model`,
at two different layers, in this precedence order (highest first):

1. `CODESCOUT_EMBEDDER_MODEL`
2. `CODESCOUT_EMBED_MODEL` — applied earlier, while `.codescout/project.toml` itself loads
3. `[embeddings].model` in `.codescout/project.toml`
4. the built-in default (`local:AllMiniLML6V2Q`)

The same precedence applies to the endpoint URL: `CODESCOUT_EMBEDDER_URL` before
`CODESCOUT_EMBED_URL` before `[embeddings].url`. Both variables in each pair are
real and both take effect — a shell that exports `CODESCOUT_EMBED_URL` for some
other tool will silently redirect codescout's embedding calls too. If your
`.codescout/project.toml` setting seems to be ignored, check both variable names
before suspecting the config file.

## Hosts with no network

Download the five files on a machine that has network, copy the directory over,
and point the config at it:

```
<dir>/onnx/model_quantized.onnx
<dir>/tokenizer.json
<dir>/config.json
<dir>/special_tokens_map.json
<dir>/tokenizer_config.json
```

```toml
[embeddings]
model = "local-dir:/opt/codescout/weights/all-MiniLM-L6-v2"
```

`local-dir:` never contacts HuggingFace — it loads exactly the five files above
and nothing else. If you point it at a HuggingFace model-repo directory
(`models--<org>--<name>/`) instead of the `snapshots/<hash>/` directory one
level below, codescout descends into that snapshot automatically when there is
exactly one candidate holding the expected files, and logs what it resolved. Two
or more candidates is not one correct reading, so codescout leaves the given
directory alone and reports the ordinary missing-file error instead of
guessing.

## Switching models on an existing index

The vector table bakes its dimension in at creation. Changing to a model with a
different dimension makes codescout refuse the next time you index or search —
not at startup, since that is the first point codescout knows which project's
index to check — naming both the dimension the index was built at and the
dimension the newly configured model produces, rather than failing later with
a confusing distance-calculation error.

Two ways out:

- Delete the index and reindex with the new model.
- Set `[embeddings].model` back to the model the index was built with, if you
  changed it by accident — often the cheaper fix.
