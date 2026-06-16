# The Lite Stack (daemon-free)

> **The default `codescout` build runs the lite stack.** Code search and memory
> run in-process on `sqlite-vec`; dense embeddings come from one remote
> OpenAI-compatible endpoint. No Docker, no Qdrant, no sparse or reranker
> services. `cargo install codescout` gives you this stack.

codescout has two retrieval backends. The **lite** stack is the default; the
**[server stack](retrieval-stack.md)** (Qdrant + sparse + reranker) is opt-in.
If you installed from crates.io or built with `cargo build --release`, you are
running lite.

## Two stacks, one switch

| | Lite (default) | [Server](retrieval-stack.md) (opt-in) |
|---|---|---|
| Vector store | in-process `sqlite-vec` (statically linked) | Qdrant (daemon) |
| Embeddings | one remote OpenAI-compatible endpoint | dense embedder service |
| Ranking | dense KNN only | dense + SPLADE sparse + cross-encoder rerank (hybrid RRF) |
| Footprint | single binary + 1 endpoint | 4 containers |
| Build | `cargo build --release` (default) | `cargo build --release --features server-stack` |

**Build-time** selects which vector store is compiled in: the default build
links only `sqlite-vec`; `--features server-stack` adds the Qdrant client.

**Runtime** selects which one runs, via `CODESCOUT_VECTOR_BACKEND`
(`sqlite-vec` | `qdrant`). The default build always runs lite — asking for
`qdrant` there returns a clear "rebuild with `--features server-stack`" error.
A `server-stack` build defaults to Qdrant.

## Why lite is the default

The lite stack exists because a locked-down corporate VDI (CrowdStrike EDR,
no Docker) cannot run Qdrant or load a foreign embedding DLL. `sqlite-vec` is
statically linked into the binary — nothing for an EDR to quarantine — so the
same daemon-free path that rescues the VDI is also the lowest-friction default
for everyone else: one binary, one HTTP endpoint, no container orchestration.
The heavy `qdrant-client` dependency is no longer compiled unless you ask for
it. See [EDR-Constrained Windows](../configuration/embeddings-edr-windows.md)
for the hardening details that drove this design.

## Quickstart

Point codescout at any OpenAI-compatible embedding endpoint (a corporate
embedding API, Ollama, llama.cpp, OpenAI) and select the `sqlite-vec` backend:

```bash
# or copy .env.lite from the repo root and `source` it
export CODESCOUT_VECTOR_BACKEND=sqlite-vec
export CODESCOUT_EMBEDDER_URL=https://embed.example/v1
export CODESCOUT_EMBEDDER_MODEL_NAME=your-model-name
export CODESCOUT_MODEL_DIM=768
export EMBED_API_KEY=...   # sent only over HTTPS (loopback hosts exempt)
```

Per-project indexes live under `CODESCOUT_SQLITE_DIR` (default
`<home>/.codescout/embeddings`). Run `index(action='build')` once per project,
then `semantic_search` works. With no endpoint configured, `semantic_search`
degrades to lexical (SQL `LIKE`) keyword search.

The `[embeddings]` table in `<project>/.codescout/project.toml` is the
per-project equivalent of the `CODESCOUT_EMBEDDER_*` variables — see
[EDR-Constrained Windows](../configuration/embeddings-edr-windows.md#fix-remote-embeddings-no-onnx-on-the-box).

## The tradeoff

Lite ranks dense-KNN only — it drops the SPLADE exact-token leg and the
cross-encoder reranker that the [server stack](retrieval-stack.md) adds. The
loss is worst for exact-identifier matches, which code search leans on. Mitigate
with a strong remote code-embedding model (CodeRankEmbed-class) on the endpoint.
If you have GPU and can run Docker, the server stack scores higher; lite trades
that for zero daemons.

Design and rationale: `docs/plans/2026-06-16-two-stack-retrieval-lite.md`.
