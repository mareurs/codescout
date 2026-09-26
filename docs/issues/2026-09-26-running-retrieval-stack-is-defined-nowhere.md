---
id: '1e385e13419237c6'
kind: bug
status: open
title: 'BUG: the retrieval stack every session uses is defined nowhere, and six docs describe one that isn''t running'
tags:
- cluster/doc-contradicted-by-code
closed: ''
opened: 2026-09-26
owner: marius
related: []
severity: medium
---

# BUG: the retrieval stack every session uses is defined nowhere, and six docs describe one that isn't running

## Summary

On this workstation, codescout's live retrieval stack is the **deleted `amd` profile**:
- `codescout-sparse-amd` runs SPLADE on TEI, built for ROCm;
- `codescout-dense-amd` and `codescout-reranker-amd` run llama.cpp b6652 on ROCm;
- all three are on the RX 7800 XT, beside `codescout-qdrant`.

The compose file they were created from, `/home/marius/work/claude/code-explorer/docker-compose.yml`, no longer exists: the checkout was renamed. The repo's `docker-compose.yml` removed the `amd` profile in `4036bb9a` (2026-07-27, *"consolidate the stack on one GPU profile"*), and now says it *"targets one NVIDIA host"*. Six documentation surfaces still describe the `amd` profile, a disabled sparse leg, or output the running stack doesn't produce. Each statement was true when written: the code lost the capability afterwards.

## Symptom (Effect)

- **Nothing in the repo can recreate, edit or reason about the containers every session's `semantic_search` depends on.** Any change to them, such as capping SPLADE's batch count, first means reconstructing their definition from `docker inspect`.
- **Following the committed compose on this host is hazardous.** Both hazards are inferred from compose semantics and the file's contents, not run:
  - `docker compose --profile gpu up -d --remove-orphans` would **remove** `codescout-sparse-amd`, `-dense-amd` and `-reranker-amd`. They carry the project label `codescout-retrieval`, but match no service in the current file.
  - `docker compose --profile gpu up -d` would try to bind 127.0.0.1:48081, :48083 and :48084, which the live containers hold (`docker-compose.yml:97`, `:177`, `:256`). It would also put the stack on the NVIDIA card, which on this host runs training, using a TEI image pinned to `turing-1.8` (compute capability 7.5), while this host's NVIDIA card is an Ampere RTX A5000 (8.6).
- **The manual sends a reader to a profile that doesn't exist:** `docker compose --profile amd up -d`, and `--profile amd up -d --build sparse-amd`.

## Reproduction

Read-only, run 2026-09-26:

```bash
docker compose --profile gpu config --services
# dense-gpu  qdrant  reranker-gpu  sparse-gpu

docker ps -a --filter label=com.docker.compose.project=codescout-retrieval \
  --format '{{.Names}} service={{.Label "com.docker.compose.service"}} config={{.Label "com.docker.compose.project.config_files"}} created={{.CreatedAt}}'
# codescout-sparse-amd   service=sparse-amd   config=/home/marius/work/claude/code-explorer/docker-compose.yml created=2026-05-26
# codescout-qdrant       service=qdrant       config=…/code-explorer/docker-compose.yml                  created=2026-05-20
# codescout-dense-amd    service=dense-amd    config=…/code-explorer/docker-compose.yml                  created=2026-05-20
# codescout-reranker-amd service=reranker-amd config=…/code-explorer/docker-compose.yml                  created=2026-05-20

ls /home/marius/work/claude/code-explorer
# No such file or directory

git show -s --format='%h %ad %s' --date=short 4036bb9a
# 4036bb9a 2026-07-27 feat(retrieval): consolidate the stack on one GPU profile, add model fetching
```

## Environment

- codescout `experiments` at `2f9e3a2c`; Docker Compose 5.5.1.
- The containers were created by an earlier Compose; their label records 5.1.4.
- Host: RTX A5000 plus RX 7800 XT (gfx1101), ROCm 7.2.4. Restart policy `unless-stopped`, so the containers survive reboots.

## Root cause

- `4036bb9a` consolidated the stack onto one NVIDIA profile for the machine the repo targeted then.
- This host kept running containers created earlier (2026-05-20/26) from the pre-rename checkout path.
- Compose records a container's config path only in a label. So nothing noticed the definition disappear twice, first through the rename and then through the profile removal.
- The docs below were each true while the `amd` profile existed, and no check compares prose against the deployed state (`IC-11`).

## Evidence

Each surface was verified 2026-09-26:

| # | surface | says | reality |
|---|---|---|---|
| 1 | `docker-compose.yml:3-12` | *"Single-profile retrieval stack: gpu (NVIDIA CUDA) … this repo targets one NVIDIA host"* | this host runs the `amd` stack on the RX 7800 XT |
| 2 | `docker-compose.yml:130-133` | sparse *"DISABLED 2026-07-28 — `CODESCOUT_DISABLE_SPARSE=1` is set in .env.gpu/.env.amd … this service is stopped"* | `.env.amd:70` and `.env:36` have it commented out; the sparse leg serves `/embed_sparse` |
| 3 | `docs/manual/src/concepts/sparse-amd.md:8`, `:37-40` | *"The default `amd` profile keeps SPLADE on CPU"*; *"part of the `amd` profile … `--profile amd up -d --build sparse-amd`"* | there is no `amd` profile, and SPLADE runs on the GPU |
| 4 | `docs/manual/src/concepts/retrieval-stack.md:71-83`, `:134` | *"AMD ROCm profile (`docker compose --profile amd`) … runs every leg of the retrieval stack on the GPU … recommended path"* | the profile doesn't exist; this also contradicts #3 |
| 5 | `sparse-amd.md:49` | `curl 127.0.0.1:48084/health   # {"status":"Ok"}` | HTTP 200 with an **empty** body (TEI 1.9.3 per `/info`) |
| 6 | `sparse-amd.md:71` (compose snippet) | build arg `PYTORCH_ROCM_ARCH: gfx1100` | `docker/sparse-amd/Dockerfile`'s only `ARG` is `TEI_REF` (`:20`); nothing reads it |
| 7 | `.env.amd:19`, `:24`, `:29` | *"qdrant + sparse-cpu + dense-amd + reranker-amd"*, `cd ~/work/claude/code-explorer`, *"sparse SPLADE (CPU)"* | sparse runs as `sparse-amd` on the GPU, and that path is gone |

**Not verified:** `docker-compose.yml:150-153` says *"TEI derives the client cap as max_batch_tokens / max_input_length"*. The live server passes `--max-client-batch-size 8` explicitly and `/info` reports 8, so this host can't test the claim. A peer investigation read TEI's `router/src/lib.rs:331` at `1588129f` as passing the flag through unchanged.

**Outside this repo** (not codescout's to fix, noted for the operator): `~/agents/llm` `docs/codescout.md` and `docs/models.md` still point at `~/work/claude/code-explorer`. A peer investigation reported this; I didn't re-check it.

## Hypotheses tried

N/A. This is a drift record, and the reproduction is the whole diagnosis.

## Fix

Proposed; needs an operator decision.

1. **Decide what this repo defines for this host.** Either:
   - **(a)** restore the `amd` services from `4036bb9a^:docker-compose.yml`, in the file or as a separate `docker-compose.amd.yml`, updated to what the live containers run (`--max-batch-tokens 2048 --max-client-batch-size 8`, `PYTORCH_HIP_ALLOC_CONF`), so the running containers have a definition again; or
   - **(b)** keep the repo NVIDIA-only and state, in the compose header and the manual, that an AMD host's stack is maintained outside it.
2. **Correct surfaces 1–7 in one pass** against the chosen state. Make one manual page authoritative, and have the other link to it rather than restate it.
3. **Re-verify:** `docker compose --profile <p> config --services` lists every running service of project `codescout-retrieval`, and no running container of that project is an orphan.

Relevant once a definition exists: the live SPLADE has no cap on batch count (`warmup_rocm{… max_bs=None}` in its logs), and `--max-batch-requests` is the candidate cap. A card-level analysis lives outside this repo, in `~/work/claude/gpu-tuning`, `docs/trackers/research.md` R-5 and Q-1.

## Tests added

N/A, with reason: this is documentation and deployment drift, with no code path to test. Fix step 3 is the check.

## Workarounds

- In this checkout on this host, don't run `docker compose … --remove-orphans` or `docker compose --profile gpu up`.
- Recreate or reconfigure a live container only from `docker inspect <container>`.

## Resume

The operator chooses (a) or (b) in Fix step 1. Nothing is claimed.

## References

- `4036bb9a` — the profile consolidation.
- `docker-compose.yml`, `.env.amd`, `docker/sparse-amd/Dockerfile`, `docs/manual/src/concepts/sparse-amd.md`, `docs/manual/src/concepts/retrieval-stack.md`.
- Class: `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md`.
