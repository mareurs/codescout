---
status: draft
opened: 2026-08-08
owner: marius
kind: spec
tags: [retrieval, embeddings, windows, vdi, lite-stack]
---

# Spec — Local ONNX embedder for the code-retrieval path

**Tracker:** WIN-26 (`docs/trackers/windows-platform-support.md`), extends
`docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md`
**Supersedes for this host:** WIN-22's "drop `local-embed*`, use a remote endpoint" mitigation

## Problem

On the Windows VDI, `semantic_search` and `memory(action="recall")` both fail:

```
dense embed connect failed: http://127.0.0.1:8081/v1/embeddings — the dense embedder
is unreachable (connect/timeout)
```

`CODESCOUT_EMBEDDER_URL` is unset, so both fall back to the `127.0.0.1:8081` default,
and nothing is listening there. The lite stack (Phase 1-4) removed the Qdrant daemon
but kept **remote embeddings as a hard requirement** — Stack B's embeddings row reads
"remote OpenAI-compatible only". That requirement is unsatisfiable here without running
a sidecar.

Three separate embedder configurations exist today, which is why the failure was hard
to read:

| Consumer | Config source | Resolves to | State |
|---|---|---|---|
| `semantic_search` | `CODESCOUT_EMBEDDER_URL` (default `:8081`) | `EmbedderHttp` | broken |
| `memory(recall)` | same retrieval path | `EmbedderHttp` | broken |
| librarian artifacts | `LIBRARIAN_EMBED_MODEL` + `LIBRARIAN_EMBED_URL` | RemoteEmbedder → Azure | working |

`.codescout/project.toml`'s `[embeddings] model = "local:AllMiniLML6V2Q"` selects
**nothing** — it feeds `EmbeddingsSection::effective_chunk_size` →
`chunk_size_for_model` (chunk sizing and drift detection only). `workspace(status)`
echoes it as `embeddings_model`, which reads as "the local model is in use" and is
false. Two people misread it in one session.

## Why this is now possible (it was not in June)

WIN-22 closed this path on the grounds that CrowdStrike quarantines the unsigned
`onnxruntime.dll` that `local-embed*` downloads. Measured 2026-08-08, that no longer
holds on this host:

- `.onnxruntime/onnxruntime.dll` (15,809,848 bytes) has been on disk since 2026-07-25 —
  14 days, unquarantined.
- `ctypes.CDLL(...)` on it → **loads successfully**. CyberArk EPM (WIN-35) does not
  block it either. Presence and loadability were tested separately, because WIN-35
  showed the two differ.

The remaining WIN-22 concern — first-use model download — is also moot: the weights
are **already side-loaded** at `.fastembed_cache/models--Xenova--all-MiniLM-L6-v2/`
(`snapshots/manual/`, `refs/main` → `manual`):

```
22,972,370  snapshots/manual/onnx/model_quantized.onnx
   711,661  snapshots/manual/tokenizer.json
       650  snapshots/manual/config.json
       125  snapshots/manual/special_tokens_map.json
       366  snapshots/manual/tokenizer_config.json
```

fastembed 5.13's `TextEmbedding::try_new_from_user_defined` builds the session via
`commit_from_memory(&model.onnx_file)` and loads the tokenizer from bytes — **no hub
access at all**, so Zscaler is irrelevant at runtime.

### Recovery source, if those files are ever lost

Measured 2026-08-08 from this host — every HuggingFace route is blocked by Zscaler,
so the usual instructions do not work here:

| Source | Result |
|---|---|
| `huggingface.co` hub / CDN / xethub | 403 (Zscaler firewall page) |
| `hf-mirror.com` | 308 → redirects to `huggingface.co` → 403 |
| `storage.googleapis.com/qdrant-fastembed` (legacy Python fastembed) | 403 |
| `chroma-onnx-models.s3.amazonaws.com/all-MiniLM-L6-v2/onnx.tar.gz` | **200, 83,178,821 bytes, gzip** |
| `raw.githubusercontent.com` (spring-ai vendored ONNX) | **200** |

The Chroma bundle is fp32, not the quantized variant cached here; with
`try_new_from_user_defined` either works, since the model is selected by *path*, not
by fastembed's model-name enum.

## Design

### §1 The seam

Two trait seams already exist and are already used as trait objects:

- `BatchEmbedder` (batch, dense+sparse) — `sync.rs:84,127` already take `&dyn BatchEmbedder`
- `DenseEmbedder` (single query, dense-only) — `Agent.memory_embedder` is already `Arc<dyn DenseEmbedder>`

The only thing forcing HTTP is the concrete field on `RetrievalClient`, plus two call
sites that reach past the traits.

**Corrected 2026-08-08 during planning.** This section first proposed
`CodeEmbedder: BatchEmbedder + DenseEmbedder`. That does not work:
`DenseEmbedder::embed(&self, &str) -> Vec<f32>` collides by name with
`EmbedderHttp`'s *inherent* `embed(&self, &str) -> EmbedOutput`, and Rust resolves
inherent methods first — so `EmbedderHttp` would carry two same-named `embed`s with
different return types, and the one callers got would depend on whether they held a
concrete type or a trait object. The supertrait is replaced by distinct method names
plus a bridge adapter:

```rust
#[async_trait::async_trait]   // REQUIRED — see note below
pub trait CodeEmbedder: BatchEmbedder {
    /// Query-side embed returning dense (+ sparse when the impl has it).
    async fn embed_one(&self, text: &str) -> Result<EmbedOutput>;
    /// Dense-only query embed, for consumers that never rank on sparse.
    async fn embed_dense_one(&self, text: &str) -> Result<Vec<f32>>;
}

/// Bridges the code embedder into the `DenseEmbedder` seam memory already holds.
pub struct CodeDenseAdapter(pub Arc<dyn CodeEmbedder>);

pub struct RetrievalClient {
    pub(crate) code_store: Arc<dyn CodeVectorStore>,
    pub embedder: Arc<dyn CodeEmbedder>,   // was: EmbedderHttp
    ...
}
```

Implementations: `EmbedderHttp` (`embed_one` = its existing inherent `embed`) and a new
`LocalOnnxEmbedder` (dense-only; `sparse` always empty, which `lite` mode already
tolerates via `dense_only`).

Call sites that change:

| Site | Change |
|---|---|
| `src/retrieval/client.rs:16` | field type + `from_env` selection |
| `src/retrieval/search.rs:96` | `.embed(query)` → `.embed_one(query)` |
| `src/agent/mod.rs:1743` | `HttpDenseEmbedder::new(client.embedder)` → clone the `Arc` |
| `src/retrieval/sync.rs` | none — already `&dyn BatchEmbedder` |

`agent/mod.rs` is the load-bearing one: wrapping the shared `Arc` in `CodeDenseAdapter`
means memory recall inherits whatever code search uses, through the same instance.
Memory is fixed by construction, not by a duplicated selection path.

**`#[async_trait]` is not optional.** Native `async fn` in traits is not
dyn-compatible, and this trait is used exclusively as `dyn CodeEmbedder`. `BatchEmbedder`
already carries `#[async_trait]` for the same reason, and `embedder.rs:176` records a
prior rustc HRTB error from getting this wrong — so both the new trait and its two impls
must use it consistently.

### §2 Selection, config, dimensions

**Selection** — new env var `CODESCOUT_EMBEDDER_MODEL`:

| Value | Effect |
|---|---|
| unset (default) | `EmbedderHttp` at `CODESCOUT_EMBEDDER_URL` — today's behaviour, unchanged |
| `local:<dir>` | `LocalOnnxEmbedder` loading `model_quantized.onnx` + `tokenizer.json` from `<dir>` |

`<dir>` is the directory *containing* `onnx/model_quantized.onnx` and `tokenizer.json`
— i.e. the `snapshots/manual/` level, not the cache root. An absolute path is used
as-is; a relative path resolves against the **project root**, not the process CWD (the
MCP server's CWD is not the project's). A bare `local:` with no path is an error, not a
guess at the cache location — silently picking a directory is how the wrong weights get
loaded without anyone noticing.

An explicit local model **wins over** `embedder_url`. This inverts
`create_embedder_with_config`'s "url wins" precedence, deliberately: there, url is the
escape hatch; here, local is the thing being asked for, and a stale default URL must not
silently outrank it. Default-off, so no existing deployment changes behaviour.

Env rather than `[embeddings]` in `project.toml` because `RetrievalConfig::from_env()`
has no project context at its construction sites. Unifying the two config surfaces is
the right end state and is **out of scope** here — recorded as follow-up.

**Dimensions.** AllMiniLM is 384-d; `code_vec` is `FLOAT[768]` and `vec0` bakes the
dimension in at table creation. Switching requires:

1. `CODESCOUT_MODEL_DIM=384` (default is 768 — `config.rs:80`)
2. delete `.codescout/code-index.db` (312 MB) — it cannot be migrated in place
3. full reindex

The reindex is not a cost of this feature: the index is already 137 commits stale
(`last_indexed_commit 4167cbce`, head `6a1ca85c`). It is destructive, so a **startup
dimension guard** compares `config.model_dim` against the store's declared dim and
fails with an explicit "delete the DB and reindex" instruction, rather than letting
`vec0` raise an arity error or — worse — returning empty results silently.

**Build.** `--features local-embed-dynamic` plus
`ORT_DYLIB_PATH=<repo>/.onnxruntime/onnxruntime.dll`. windows-gnu cannot use the static
`local-embed` variant, and this host has no MSVC linker. The two ONNX features are
already a compile-time mutual exclusion; nothing to add.

**In-scope correctness fix.** `workspace(status)` must report the *effective*
code-search embedder rather than echoing `[embeddings].model`. Without it this feature
ships alongside a status field that still lies — the same field that misled two readers
today.

### §3 Error handling

Every failure below is operator-fixable, so each is a `RecoverableError` with a hint,
per `get_guide("error-handling")`.

| Failure | Raw error | Surfaced as |
|---|---|---|
| Model files absent | `ort` file-not-found | Names expected `model_quantized.onnx` / `tokenizer.json` paths **and** the Chroma recovery URL (HF is blocked here) |
| `ORT_DYLIB_PATH` unset/missing | dylib load failure | Names the variable and the `.onnxruntime/onnxruntime.dll` path |
| DLL load denied by EDR | `os error 5` | OS error verbatim + points at CyberArk EPM — WIN-35's lesson: a permanent failure that reads as retryable burns a session |
| Dim mismatch | `vec0` arity error or silent empty | The §2 guard |
| `local:` without the feature | none (silent fallthrough) | "rebuild with `--features local-embed-dynamic`", mirroring `create_embedder_with_config` |

The ONNX session is constructed **once** behind a `OnceCell`. A 22 MB session per query
is a performance bug wearing a correctness bug's clothes.

### §4 Testing

This session produced two green-checks-that-could-not-fail (the MSYS test driving only
MSYS builtins; the WSL-exclusion test passing on its own capitalisation), so the split
below is explicit about which tests can actually fail where.

**Runs everywhere, no ONNX required** — where the logic lives:

- Selection/precedence: `CODESCOUT_EMBEDDER_MODEL` parsing, and that `local:` beats a
  set `embedder_url`. Env-mutating → `EnvGuard` + `#[serial]`.
- Dimension guard: mismatch yields the actionable error, **plus a positive control**
  that matching dims proceed — otherwise "always errors" passes as success.
- Missing-files error names the path and does not panic.

**Requires DLL + weights** — feature-gated, explicitly skipped in CI:

- Contract parity: `LocalOnnxEmbedder` satisfies the same `BatchEmbedder` assertions as
  the existing fake, pinning the trait contract from both impls.
- A real embed: fixed input → dimension is 384, and the vector is stable across two
  calls. The only test that catches a silently-wrong tokenizer.

CI cannot run the second group (no DLL, no weights, HF unreachable from runners).
Following the wine precedent, they get an explicit skip with the reason in the workflow
comment — **not** a self-skipping probe, which would quietly disarm them on this host,
where they are the entire point.

**Deliberately not tested:** retrieval *quality* at 384-d versus the previous 768-d
model. That is a benchmark question (`scripts/run-tc-benchmark.sh`), not a unit test.
See Risks.

## Risks

- **Quality regression.** Moving from a 768-d (likely code-specialised) model to
  384-d general-purpose AllMiniLM may measurably degrade code search. Dense-only
  already dropped the SPLADE exact-token leg and the reranker, which the lite plan
  notes hurts identifier matching worst — exactly what code search leans on. Quantify
  with the existing benchmark harness before treating this as the permanent default.
- **Destructive migration.** The 312 MB index must be deleted. Reversible only by
  reindexing against the old embedder, which requires the endpoint that is currently
  unreachable.
- **EDR is a moving target.** The DLL loads today; WIN-18/WIN-22/WIN-35 are all cases
  where corporate policy changed under the project. The error path in §3 is what keeps
  a future re-block legible.

## Out of scope

- Unifying `[embeddings]` in `project.toml` with `RetrievalConfig` (follow-up).
- Routing the librarian off Azure onto local ONNX — it works today; this changes only
  the two broken consumers.
- Restoring sparse/rerank on the lite stack.
- Any change to the remote path's default behaviour.

## Follow-ups

1. Unify the embedder config surfaces so one setting selects the embedder for all three
   consumers.
2. Benchmark 384-d local versus 768-d remote on the existing suites; record the delta.
3. Consider vendoring the recovery instructions into
   `docs/manual/src/configuration/embeddings-edr-windows.md`, which currently documents
   only the WIN-22 remote-endpoint mitigation.
