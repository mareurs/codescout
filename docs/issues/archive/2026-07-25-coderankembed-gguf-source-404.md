---
id: '19b108e4d047d0ac'
kind: bug
status: fixed
title: Documented CodeRankEmbed GGUF source nomic-ai/CodeRankEmbed-GGUF returns 401 — dense embedder cannot be provisioned from a clean checkout
tags:
- retrieval-stack
- docs-drift
- embeddings
- onboarding
closed: 2026-07-25
opened: 2026-07-25
owner: marius
related: []
severity: high
---

# BUG: documented CodeRankEmbed GGUF source is a dead repo — dense embedder unprovisionable from a clean checkout

## Summary

Both places that document how to obtain `CodeRankEmbed-Q4_K_M.gguf` point at
`nomic-ai/CodeRankEmbed-GGUF`, which returns HTTP 401 and does not appear in
HuggingFace search. Without that file the `dense-cpu` / `dense-gpu` container
crashloops, and with it the entire retrieval stack is unusable — `embed_batch`
joins the dense and sparse legs, so a dense failure fails the whole call. Anyone
provisioning this project on a new machine is blocked at exactly this step.

## Symptom (Effect)

Container crashloop, `Restarting (1)`, no port bound:

```
gguf_init_from_file: failed to open GGUF file '/models/CodeRankEmbed-Q4_K_M.gguf' (No such file or directory)
llama_model_load: error loading model: llama_model_loader: failed to load model from /models/CodeRankEmbed-Q4_K_M.gguf
common_init_from_params: failed to load model '/models/CodeRankEmbed-Q4_K_M.gguf'
srv    load_model: failed to load model, '/models/CodeRankEmbed-Q4_K_M.gguf'
main: exiting due to model loading error
```

Following the documented remedy fails:

```
$ curl -L --fail -sS -o CodeRankEmbed-Q4_K_M.gguf \
    https://huggingface.co/nomic-ai/CodeRankEmbed-GGUF/resolve/main/CodeRankEmbed-Q4_K_M.gguf
curl: (22) The requested URL returned error: 401
```

The repo is not merely gated — it is absent. HF returns 401 for both private and
nonexistent repos, so these were distinguished explicitly:

```
$ curl -s -o /dev/null -w '%{http_code}' https://huggingface.co/api/models/nomic-ai/CodeRankEmbed-GGUF
401
$ curl -s -o /dev/null -w '%{http_code}' https://huggingface.co/api/models/nomic-ai/CodeRankEmbed
200
```

A HF search for `CodeRankEmbed` returns 20 repos; `nomic-ai/CodeRankEmbed-GGUF`
is not among them. The base model `nomic-ai/CodeRankEmbed` is live, ungated
(`gated: False`), and has ~173k downloads — but ships safetensors, not GGUF.

## Reproduction

Commit `52fcaf0118d9a6388a8c5828f1447b818d05f360`, branch `experiments`.

1. Clean checkout on a machine with no `models/` directory.
2. `./scripts/retrieval-stack.sh up` (or `docker compose --profile cpu up -d`).
3. `docker logs codescout-dense-cpu` → crashloop on the missing GGUF.
4. Follow either documented instruction to fetch it → HTTP 401.

## Environment

Arch Linux (7.1.3-arch1-2), Docker 1:29.6.1-1, codescout 0.15.0.
`CODESCOUT_MODEL_DIR` resolves to `./models`; the compose bind mount is
`${CODESCOUT_MODEL_DIR:-./models}:/models:ro`.

## Root cause

Documentation drift, not a code defect. The upstream repo the docs were written
against is no longer reachable — either removed, renamed, or made private since
the instructions were authored. Two sources are affected:

- `.env.amd:17` —
  `#   CodeRankEmbed-Q4_K_M.gguf         (90 MB)  — nomic-ai/CodeRankEmbed-GGUF`
- `docs/manual/src/concepts/retrieval-stack.md:44-46` and `:90-91` —
  `huggingface-cli download nomic-ai/CodeRankEmbed-GGUF CodeRankEmbed-Q4_K_M.gguf --local-dir .`
  and the `wget https://huggingface.co/nomic-ai/CodeRankEmbed-GGUF/resolve/main/...` variant.

A secondary, aggravating factor: when the mount target does not exist, Docker
auto-creates `models/` as `root:root`, so the first fix attempt also hits a
permission error before the download is even reached.

## Evidence

### Community quants exist but are unvetted

HF search surfaces `brandtcormorant/CodeRankEmbed-Q4_K_M-GGUF` (resolves 200,
one file `coderankembed-q4_k_m.gguf`) — but **12 downloads, 0 likes, unknown
author, untouched since 2025-04-14**, and a filename that differs from what
compose expects. Substituting an unvetted third-party quant for the project's
benchmarked embedding model is a supply-chain decision, not a fix.

### Conversion from the official repo reproduces the documented artifact exactly

Converting the live, ungated `nomic-ai/CodeRankEmbed` with llama.cpp produced a
file matching the documented size to the byte-class:

```
llama_model_quantize_impl: model size  =   260.87 MiB (16.00 BPW)
llama_model_quantize_impl: quant size  =    85.33 MiB (5.24 BPW)

-rw-r--r-- 1 marius marius  90118048 Jul 25 14:13 models/CodeRankEmbed-Q4_K_M.gguf
```

90,118,048 bytes ≈ the "90 MB" the docs specify. Architecture detected as
`NomicBertModel`, 112 tensors. Verified end-to-end after the container came up:
768-dim output (matching `CODESCOUT_MODEL_DIM`), L2 norm 1.0, non-degenerate.

## Hypotheses tried

1. **Hypothesis:** the repo is gated and just needs an HF token.
   **Test:** checked for `~/.cache/huggingface/token` and `HF_TOKEN` (absent);
   queried the HF search API for `CodeRankEmbed`.
   **Verdict:** rejected — the repo is absent from search results entirely, and
   `nomic-ai/CodeRankEmbed` reports `gated: False`. This is a missing repo, not
   an auth problem.

2. **Hypothesis:** an equivalent GGUF is already cached locally from a prior run.
   **Test:** searched `models/`, `/home/marius/models` (the path `.env` points
   at — does not exist on this machine), and `~/.cache/huggingface/hub`.
   **Verdict:** rejected — the HF cache holds bge-reranker-v2-m3,
   bge-small-en-v1.5, MiniLM and docling models, no CodeRankEmbed.

## Fix

Applied 2026-07-25 (uncommitted on `experiments` at time of writing).

1. **`scripts/fetch-models.sh` (new, executable).** Encodes the working recipe so
   the instruction can no longer drift from the command. `./scripts/fetch-models.sh`
   builds the dense model; `--amd` additionally downloads the reranker. Idempotent
   (skips existing targets), honours `CODESCOUT_MODEL_DIR`, creates the bind-mount
   target itself to pre-empt Docker's `root:root` auto-create, and passes
   `--user "$(id -u):$(id -g)"` to the llama.cpp container so converted files are
   not written as root.
2. **`docs/manual/src/concepts/retrieval-stack.md`** — both blocks replaced with the
   script invocation (§ *Bring up the stack*, § *AMD ROCm profile*), plus an
   explanation of why the model is built rather than downloaded, and a callout
   about the `root:root` trap.
3. **`.env.amd`** — model list now points at the script and records that no GGUF
   repo exists for CodeRankEmbed.

**Reranker verified, contrary to the original concern.**
`gpustack/bge-reranker-v2-m3-GGUF` returns **200** and publishes exactly
`bge-reranker-v2-m3-Q4_K_M.gguf` (checked against the HF file list rather than
assumed — the third-party CodeRank quant surveyed earlier used a differently-cased
filename). The `amd` profile is therefore **not** blocked; only the dense model was
ever affected.

```
200  gpustack/bge-reranker-v2-m3-GGUF
401  nomic-ai/CodeRankEmbed-GGUF
```
## Tests added

`N/A` — documentation defect; there is no code path to regression-test.

Justification for the absence, per the template's requirement: the closest
equivalent would be a CI job HEAD-requesting the documented model URLs, which
introduces a network dependency and an external-availability failure mode into
every CI run. The mitigation taken instead is structural — moving the URLs out of
prose and into `scripts/fetch-models.sh`, so there is now exactly **one** place
that can rot, and it fails loudly (`curl --fail`, `set -euo pipefail`) at the
moment someone tries to use it rather than silently misleading a reader.

Smoke-tested: `bash -n` clean; idempotent re-run correctly skips the existing
dense model; `--bogus` exits 2 with usage on stderr.
## Workarounds

Use the conversion recipe under **Fix**. The container has
`restart: unless-stopped`, so it self-heals within its retry interval once the
file lands — no manual restart needed.

## Resume

`N/A` — fixed and shipped.

This section previously said the change was "uncommitted on `experiments`" and
that the file should be archived "only after the fix ships to `master`". Both
statements are stale, and the second reflects the older CLAUDE.md archive rule:

- **Committed** as **`4036bb9a`** *feat(retrieval): consolidate the stack on one
  GPU profile, add model fetching* (carries `scripts/fetch-models.sh` and the
  `retrieval-stack.md` / `.env.amd` rewrites), with follow-up **`6f13c171`**
  *docs(env): document the three-layer config precedence, warn on the dead
  template*.
- **Archive rule changed.** Current CLAUDE.md archives once the fix is verified
  on `experiments`; reaching `master` is no longer required. This file is
  correctly archived already.

Both SHAs are **`experiments`**-only (`git merge-base --is-ancestor 4036bb9a
master` → false). Per CLAUDE.md the master-side SHA still needs recording here
after cherry-pick — an `experiments` SHA orphans on rebase. That is the only
outstanding item.
## References

- [reindex reembed no-op without force](2026-07-25-reindex-reembed-noop-without-force.md)
  (`fc5dfce843caa841`) — the sibling bug found while restoring embeddings after
  this one was worked around
- `docs/manual/src/concepts/retrieval-stack.md` § Dense embedder — the benchmark
  that makes CodeRankEmbed Q4_K_M the champion (37) and pins no-prefix as default
- `.gitignore` — `/models/` added this session; the directory holds ~870 MB and
  was previously untracked-but-committable
