---
status: fixed
opened: 2026-07-27
closed: 2026-07-27
severity: high
owner: marius
related:
  - docs/issues/2026-07-25-compose-gpu-profile-ampere-only.md
  - docs/issues/2026-07-25-coderankembed-gguf-source-404.md
tags: [retrieval, docker, gpu, reranker, vram]
kind: bug
---

# BUG: reranker-gpu (TEI + fp16 bge-reranker-v2-m3) OOMs on a 6 GiB card and crash-loops silently

## Summary

The `gpu` compose profile ran the reranker as unquantized TEI
(`BAAI/bge-reranker-v2-m3 --dtype float16`). On a 6 GiB GTX 1660 Ti it fails
CUDA warmup with `CUDA_ERROR_OUT_OF_MEMORY`, then crash-loops. Nothing surfaces
the failure to callers, so `semantic_search` silently degrades to dense+sparse
with no rerank stage. Found while probing an unrelated indexing job.

## Symptom (Effect)

Container never binds :80; health check fails forever.

```
2026-07-26T20:08:18.623568Z  INFO text_embeddings_router: Warming up model
Error: Model backend is not healthy

Caused by:
    DriverError(CUDA_ERROR_OUT_OF_MEMORY, "out of memory")
```

Health check, 497 consecutive failures at the time of observation:

```
curl: (7) Failed to connect to 127.0.0.1 port 80 after 0 ms: Connection refused
```

`docker inspect` at 2026-07-27 07:28 local:

```
1040 restarts | started 2026-07-26T20:08:20.033143544Z
```

Secondary effect — the instance alive at observation time was not even
crash-looping any more, it was **wedged**: last log line
`20:08:22Z Downloading 'config_sentence_transformers.json'`, then 8h20m of
silence. Process alive, `State: S`, 0.0% CPU, zero GPU memory allocated.

A non-fatal `404` also appears in the same startup path (TEI probes optional
files):

```
Download failed: request error: HTTP status client error (404 Not Found) for url
(https://huggingface.co/BAAI/bge-reranker-v2-m3/resolve/main/1_Pooling/config.json)
```

## Reproduction

Commit: `52fcaf01` (branch `experiments`).

```
docker compose --env-file .env.gpu --profile gpu up -d reranker-gpu
docker logs -f codescout-reranker-gpu
```

On any CUDA card with < ~8 GiB free VRAM, warmup dies with the error above.

## Environment

- Arch Linux, kernel 7.1.4-arch1-1
- NVIDIA GeForce GTX 1660 Ti, 6144 MiB, compute capability 7.5 (Turing)
- Driver 610.43.03, CUDA UMD 13.3
- Image: `ghcr.io/huggingface/text-embeddings-inference:turing-1.8@sha256:bd102b08…`
- Free VRAM at time of OOM: ~4.9 GiB (dense-gpu 392 MiB + sparse-gpu 726 MiB +
  desktop ~75 MiB were the only other consumers)

## Root cause

Model size was estimated from the wrong quantity, and the warmup allocation was
never bounded.

1. **Sizing error in the compose comment.** `docker-compose.yml:298` read
   `GPU: bge-reranker-v2-m3 (568MB, ~80ms p95 — full quality)`. 568 is the
   **parameter count in millions**, not a byte size. Measured artifact:

   ```
   /data/models--BAAI--bge-reranker-v2-m3/snapshots/*/model.safetensors
     2,271,071,852 bytes = 2.12 GiB (fp32)
   2,271,071,852 / 4 = 567,767,963 params
   ```

   At `--dtype float16` that is ~1.13 GiB of weights — ~2× the figure the
   comment implied, but still not by itself the OOM.

2. **Unbounded warmup activation.** The model is XLM-RoBERTa-large with
   `max_position_embeddings: 8194`, `num_hidden_layers: 24`,
   `num_attention_heads: 16`, `hidden_size: 1024` (from its `config.json`). TEI
   logged `Maximum number of tokens per request: 8192` with the default
   `max_batch_tokens: 16384`, and the Candle Bert path on Turing is documented
   upstream as having no flash-attention. A non-flash attention probability
   matrix at that sequence length is

   ```
   8192² × 16 heads × 2 bytes ≈ 2.15 GB, transient, per layer
   ```

   on top of the weights and the fp32→fp16 cast buffer. That is what exceeds
   ~4.9 GiB of headroom.

   **Confidence:** (1) is measured. (2) is inferred from the config, the
   documented no-flash-attention Turing caveat, and the crash point
   (`Warming up model` → immediate OOM). The allocation itself was not traced.

Note this is the same class of mistake already documented for `sparse-amd`
(`docker-compose.yml:257-262`), where SPLADE's vocab-sized projection forced
`--max-batch-tokens 2048` to bound VRAM. The GPU reranker never got the
equivalent cap.

## Evidence

### Container state (`docker inspect codescout-reranker-gpu`)

```
{"Status":"unhealthy","FailingStreak":497, …
  "Output":"curl: (7) Failed to connect to 127.0.0.1 port 80 …"}
1040 restarts | started 2026-07-26T20:08:20.033143544Z
```

### Measured artifact size (alpine container over the `model_cache` volume)

```
1.1G  /data/models--BAAI--bge-reranker-base
2.1G  /data/models--BAAI--bge-reranker-v2-m3
926M  /data/models--prithivida--Splade_PP_en_v1

-rw-r--r-- 1 root root  2271071852  model.safetensors
-rw-r--r-- 1 root root    17098273  tokenizer.json
```

### Model geometry (`config.json`)

```
"architectures": ["XLMRobertaForSequenceClassification"],
"hidden_size": 1024, "intermediate_size": 4096,
"num_attention_heads": 16, "num_hidden_layers": 24,
"max_position_embeddings": 8194, "vocab_size": 250002,
"torch_dtype": "float32"
```

### Pre-existing drift in the same file

`docker-compose.yml:17-21` already instructed the reader to place
`bge-reranker-v2-m3-Q4_K_M.gguf` in `${CODESCOUT_MODEL_DIR}` "(for gpu/amd
profiles)", and `scripts/fetch-models.sh` only fetched it behind `--amd`. The
header documented the GGUF path for GPU; the service definition never
implemented it.

## Hypotheses tried

1. **Hypothesis:** the reranker was competing for VRAM with the running index
   and simply lost a race.
   **Test:** read `nvidia-smi` at observation time and reconstruct occupancy at
   the OOM timestamp from container uptimes (dense up 36h, sparse up 35h — both
   predate the OOM).
   **Verdict:** rejected. ~4.9 GiB was free; 1.13 GiB of weights had ample room.
   The failure is in warmup sizing, not contention.

2. **Hypothesis:** the HuggingFace `404` on `1_Pooling/config.json` is the
   failure.
   **Verdict:** rejected. TEI logs it at `WARN` and continues to the next
   candidate file; the run that logged it proceeded to `Starting model backend`.

3. **Hypothesis:** the currently-alive instance is still crash-looping.
   **Test:** `/proc/1219857/status`, `docker logs --tail`, `nvidia-smi` process
   list.
   **Verdict:** rejected — it is wedged, not looping. `State: S`, 0.0% CPU, no
   GPU allocation, last log line 8h20m stale mid-download. Distinct secondary
   failure mode (no timeout on the HF fetch path); not yet root-caused.

## Fix

Mirror `reranker-amd` for CUDA: same `bge-reranker-v2-m3-Q4_K_M.gguf` (419 MB,
~0.5 GiB VRAM) on llama-server, swapping the ROCm image for
`ghcr.io/ggml-org/llama.cpp:server-cuda`. This also removes the TEI
compute-capability hardware pin from this service, since llama.cpp ships one
CUDA image with `ARCHS=500..1200`.

Changes (branch `experiments`, not yet on master):

- `docker-compose.yml` — `reranker-gpu` rewritten from TEI to llama-server;
  port mapping `48083:80` → `48083:8080`; health check target updated; RERANK
  section comment corrected (the `568MB` → parameter-count error).
- `.env.gpu` — added `CODESCOUT_RERANKER_PROTOCOL=llama-server`. Required:
  `Protocol::from_env` (`src/retrieval/reranker.rs:18-27`) defaults to
  `Protocol::Tei`, which posts `{query, texts}` and reads `[{index, score}]`.
  llama-server answers the Jina/Cohere shape that `Protocol::Infinity`
  (`src/retrieval/reranker.rs:44-60`) already implements.
- `scripts/fetch-models.sh` — reranker GGUF now fetched for `--gpu` as well as
  `--amd`.

**Verified 2026-07-27.** GGUF downloaded (438,376,864 bytes), service recreated,
healthy in ~25s, live rerank round-trip correct:

```
$ curl -s :48083/rerank -d '{"query":"how do I parse a configuration file",
    "documents":["fn parse_config(path: &str) -> Config { … }",
                 "fn main() { println!(\"hello\"); }",
                 "The mitochondria is the powerhouse of the cell."]}'
{"model":"bge-reranker-v2-m3-Q4_K_M.gguf", "results":[
  {"index":0,"relevance_score":1.2113699913024902},
  {"index":1,"relevance_score":-8.401334762573242},
  {"index":2,"relevance_score":-10.944034576416016}]}
```

Measured VRAM: **340 MiB** (`nvidia-smi --query-compute-apps`), against the
~4.9 GiB the TEI fp16 path failed to fit into — a 14× reduction. Card total
after the change: 1637 MiB / 6144 MiB with the indexer still running.

One behaviour worth recording, verified against the live service:
llama-server's `/rerank` **mirrors the request dialect in its response**. Sent
`{query, documents}` it answers `{results:[{index, relevance_score}]}` (Cohere
shape); sent `{query, texts}` it answers `[{index, score}]` (TEI shape). So
`Protocol::Tei` would also have worked here — the explicit
`CODESCOUT_RERANKER_PROTOCOL=llama-server` is still correct and clearer, but it
is not the only working configuration.

The wedged-mid-download secondary failure (hypothesis 3) is **not** addressed by
this change; swapping to a bind-mounted GGUF removes the HF-fetch path from this
service entirely, which sidesteps it here but leaves it live for `sparse-gpu`
and both `reranker-cpu`/`sparse-cpu`.

## Tests added

None — this is compose/env configuration with no Rust surface. Verified by the
manual round-trip recorded under "Fix":

```
./scripts/fetch-models.sh --gpu
docker compose --env-file .env.gpu --profile gpu up -d reranker-gpu
curl -fsS 127.0.0.1:48083/health
curl -s 127.0.0.1:48083/rerank -H 'content-type: application/json' \
  -d '{"query":"parse a config file","documents":["fn parse_config()","fn main()"]}'
```

Expect `results[].relevance_score` with index 0 ranked first.

A cheap regression guard worth considering: assert `.env.gpu` sets
`CODESCOUT_RERANKER_PROTOCOL` whenever `reranker-gpu` is a llama-server image.
Currently nothing catches the mismatch — the wrong protocol fails at call time,
not at startup.

## Workarounds

- Point `CODESCOUT_RERANKER_URL` at the CPU reranker (`bge-reranker-base` via
  the `cpu` profile, ~250ms p95) until the GGUF path is verified.
- Or keep TEI and cap it: `--max-batch-tokens 2048`, mirroring what
  `sparse-amd` does for the same reason.
- Search still functions without a reranker — dense+sparse results are returned
  unranked by the rerank stage. Quality drops; nothing errors.

## Resume

Service-side work is done and verified. One item remains, not blocking:

1. ~~codescout's own `semantic_search` has not been exercised through the new
   reranker.~~ **Done 2026-07-29.** MCP server restarted at 20:38 (well after the
   `.env.gpu` change, so `Protocol::from_env` read the new value at startup);
   `semantic_search` returned ranked results with no `rerank status` / `rerank
   json` error. Archiving on that basis.
2. **Cherry-pick to master** and record the master-side SHA here. The SHA on this
   file is an `experiments` SHA and orphans on rebase.

Note for whoever picks this up: the reranker being *functional* is what this entry
tracks, and it is. Whether it should be enabled at all is a separate open question
— see `docs/issues/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`,
which measures it as strictly worse on both latency and score for the dense path.

Separately: open a follow-up for the wedged-mid-download failure mode
(hypothesis 3) affecting every TEI service that resolves a model from
HuggingFace at startup — `sparse-gpu` still carries that exposure.
## References

- `docker-compose.yml:296-360` — RERANK section, both GPU and AMD services
- `src/retrieval/reranker.rs:18-27, 44-60, 88-142` — protocol selection and both
  request shapes
- `scripts/fetch-models.sh:62-72` — `fetch_reranker`
- `.env.gpu`, `.env.amd` — client wiring for both profiles
- https://huggingface.co/gpustack/bge-reranker-v2-m3-GGUF — GGUF source
- `docs/issues/2026-07-25-compose-gpu-profile-ampere-only.md` — the TEI
  image-per-compute-capability pin that this change removes for the reranker
