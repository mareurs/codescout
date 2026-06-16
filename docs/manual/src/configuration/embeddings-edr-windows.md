# EDR-Constrained Windows (CrowdStrike): Remote Embeddings

On a locked-down corporate Windows machine — typically a VDI running CrowdStrike
Falcon or a similar EDR/AV — the **local** ONNX embedding backend does not work.
The `local-embed` / `local-embed-dynamic` features make `ort`/fastembed download
an **unsigned** `onnxruntime.dll`, and the EDR's *quarantine-on-write* deletes it
as malware seconds after it lands on disk — the same heuristic that flags
freshly-built unsigned executables. Semantic search then silently degrades to the
lexical (SQL `LIKE`) keyword fallback.

You usually **cannot** fix this by whitelisting — AV exclusions are out of your
control on a managed VDI. The fix is to **keep ONNX off the box entirely** and run
embeddings against a remote endpoint.

## Fix: remote embeddings, no ONNX on the box

codescout's `Embedder` is backend-agnostic. Point it at any OpenAI-compatible
embedding endpoint — a corporate internal embedding API, Ollama, TEI, etc. — and
the local ONNX runtime is never compiled or downloaded.

### 1. Build without the local ONNX feature

The **default** feature set already excludes local ONNX:

```bash
cargo build --release        # default features = remote-embed, http, librarian
```

Do **not** pass `--features local-embed` or `--features local-embed-dynamic`.
After building, confirm there is no `onnxruntime.dll` anywhere under `target/` and
that `ort` / `fastembed` were not compiled. Now the EDR has nothing to quarantine.

### 2. Point at the embedding endpoint

In `<project>/.codescout/project.toml`:

```toml
[embeddings]
model   = "your-model-name"                # the bare model name the API expects
url     = "https://embed.corp.example/v1"  # OpenAI-compatible; /v1/embeddings is appended
api_key = "..."                            # or set EMBED_API_KEY in the environment
```

Setting `url` takes priority over the model prefix, so resolution routes straight
to the remote backend (`RemoteEmbedder`) — any `local:` prefix on `model` is
stripped. Equivalent environment variables:
`CODESCOUT_EMBED_URL`, `CODESCOUT_EMBED_MODEL`, `EMBED_API_KEY`.

> **HTTPS is required when `api_key` is set.** codescout refuses to send an API key
> over plaintext HTTP. Loopback hosts (`localhost`, `127.0.0.1`, `[::1]`) are
> exempt — that carve-out is for local Ollama / llama.cpp, not a network endpoint.
> Use an `https://` URL for a corporate API.

### 3. Reindex (do not skip)

The default local model (`AllMiniLML6V2Q`) is **384-dimensional**; your remote
model almost certainly emits a different dimension (often 768 / 1024 / 1536). The
vector index is dimension-specific, so a stale index built with the old dimension
produces wrong or empty results. **Rebuild the index after switching embedders**
(the `index` tool / your normal reindex path) — config change first, reindex
second.

### 4. Verify

- `semantic_search` returns results.
- No `onnxruntime.dll` exists under `target/`.
- The EDR quarantines nothing during build or run.

## Daemon-free: the lite stack (no Qdrant, no sparse, no reranker)

> Canonical setup and the two-stack overview now live in
> [The Lite Stack](../concepts/lite-stack.md) — it is the default build, not just
> the EDR case. This section keeps the EDR-specific framing.

If the VDI cannot run Docker or Qdrant at all — the common locked-down case —
use the **lite stack**. Code search and memory run entirely in-process on
`sqlite-vec` (a statically-linked `vec0` table — no foreign DLL for the EDR to
quarantine, unlike `onnxruntime.dll`), with dense embeddings from your remote
endpoint. No Qdrant, no sparse SPLADE server, no cross-encoder reranker.

```bash
# or copy .env.lite from the repo root and `source` it
export CODESCOUT_VECTOR_BACKEND=sqlite-vec
export CODESCOUT_EMBEDDER_URL=https://embed.corp.example/v1
export CODESCOUT_EMBEDDER_MODEL_NAME=your-model-name
export CODESCOUT_MODEL_DIM=768
export EMBED_API_KEY=...   # sent only over HTTPS (loopback exempt)
```

Per-project indexes live under `CODESCOUT_SQLITE_DIR` (default
`<home>/.codescout/embeddings`). Then run `index(action='build')` once per
project. The trade-off versus the full server stack is dense-only ranking — no
sparse exact-token leg, no rerank — so exact-identifier recall is weaker; pair
it with a strong code-embedding model on the endpoint. Design + rationale:
`docs/plans/2026-06-16-two-stack-retrieval-lite.md`.

## Why not just sign the DLL?

Authenticode-signing the ONNX runtime would beat the *quarantine-on-write* of the
unsigned file, but CrowdStrike Falcon is behavior-first: ML runtimes that allocate
`PAGE_EXECUTE_READWRITE` memory or use dynamic `LoadLibrary` can still trip the
behavioral engine even when signed. Remote embeddings remove the runtime from the
box altogether, so there is nothing to sign, load, or quarantine.

## If no endpoint is reachable (air-gapped)

If the machine is fully air-gapped with no reachable embedding endpoint:

- **Lexical fallback** — with no embedder configured, `semantic_search` falls back
  to SQL `LIKE` keyword search. Functional, but not semantic.
- **Pure-Rust embeddings (candle)** — a future `candle`-based local backend would
  run the model in-process with **no foreign DLL** (pure-Rust `gemm`, no
  `PAGE_EXECUTE_READWRITE` codegen), compiling into the large `codescout.exe` that
  already survives the EDR's tiny-PE heuristic. Not yet implemented — tracked as a
  candidate backend.
