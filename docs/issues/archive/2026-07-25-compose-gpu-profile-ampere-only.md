---
id: '2db2966ea4b0ee7a'
kind: bug
status: fixed
title: docker-compose `gpu` profile pins Ampere-only TEI images (`86-1.8`) — unusable on Turing/CC 7.5 cards
tags:
- docker-compose
- retrieval
- gpu
- portability
closed: 2026-07-28
opened: 2026-07-25
owner: marius
related:
- docs/issues/2026-07-25-coderankembed-gguf-source-404.md
severity: medium
---

# BUG: docker-compose `gpu` profile pins Ampere-only TEI images (`86-1.8`) — unusable on Turing/CC 7.5 cards

## Summary

`docker-compose.yml`'s `gpu` profile pins `sparse-gpu` and `reranker-gpu` to
`ghcr.io/huggingface/text-embeddings-inference:86-1.8` by digest. The `86` tag
targets CUDA compute capability **8.6** (Ampere). The dev box's card is a GTX
1660 Ti at **CC 7.5** (Turing), so two of the three GPU services in the profile
cannot run there. `dense-gpu` (llama.cpp CUDA) is unaffected. Net effect:
`docker compose --profile gpu up -d` on a Turing host is expected to bring the
dense leg up and leave sparse + rerank down — worse than the `cpu` profile it
replaces, because the sparse leg is the dominant CPU consumer and the reason to
move to GPU at all.

## Symptom (Effect)

Not yet triggered — caught during pre-switch reconnaissance, before running the
profile. The mismatch is static:

```
$ nvidia-smi --query-gpu=name,compute_cap,memory.total,memory.used --format=csv
name, compute_cap, memory.total [MiB], memory.used [MiB]
NVIDIA GeForce GTX 1660 Ti, 7.5, 6144 MiB, 9 MiB
```

against `docker-compose.yml:198-203`:

```yaml
  sparse-gpu:
    profiles: [gpu]
    image: ghcr.io/huggingface/text-embeddings-inference:86-1.8@sha256:65f792e790f976713a5d2ab2586d93d074203d1f0ec2045e87e60113fbd0e256
    container_name: codescout-sparse-gpu
    command: ["--model-id", "prithivida/Splade_PP_en_v1", "--pooling", "splade", "--dtype", "float16", "--auto-truncate"]
```

and `docker-compose.yml:309-314`:

```yaml
  reranker-gpu:
    profiles: [gpu]
    image: ghcr.io/huggingface/text-embeddings-inference:86-1.8@sha256:65f792e790f976713a5d2ab2586d93d074203d1f0ec2045e87e60113fbd0e256
    container_name: codescout-reranker-gpu
    command: ["--model-id", "BAAI/bge-reranker-v2-m3", "--dtype", "float16", "--auto-truncate"]
```

Neither TEI GPU image is present locally, so the failure would land after a
~2-3 GB pull:

```
$ docker images --format '{{.Repository}}:{{.Tag}} {{.Size}}'   # filtered
ghcr.io/ggml-org/llama.cpp:server-cuda 5.95GB
ghcr.io/huggingface/text-embeddings-inference:cpu-1.8 938MB
ghcr.io/huggingface/text-embeddings-inference:<none> 915MB
# no 86-1.8, no turing tag
```

**Correction (2026-07-25, after bringing `dense-gpu` up):** the `server-cuda`
line above does NOT mean the pinned image was local. The local **tag**
`server-cuda` resolved to a different digest than the compose pin
(`sha256:a04923d3…`), so `docker compose --profile gpu up -d dense-gpu` performed
a full fresh pull. Reading a tag row in `docker images` as satisfying a
digest-pinned reference is invalid — compare digests, not tags
(`docker images --digests`).

Expected runtime failure mode when it is attempted: CUDA kernel-image error
(`no kernel image is available for execution on the device`) at model load, i.e.
the container starts, fails the healthcheck, and restarts under
`restart: unless-stopped`.

## Reproduction

```bash
git rev-parse HEAD    # 52fcaf0118d9a6388a8c5828f1447b818d05f360 (branch: experiments)
nvidia-smi --query-gpu=compute_cap --format=csv,noheader   # must be < 8.6, e.g. 7.5
docker compose --profile gpu --env-file .env.gpu up -d
docker compose logs sparse-gpu reranker-gpu
```

**Not yet actually run** — the reconnaissance stopped short of it deliberately,
since a failed sparse leg takes retrieval down. Confirming the exact error
string is the first Resume step.

## Environment

- Linux 7.1.4-arch1-1, 16 cores, 31.5 GiB RAM
- NVIDIA GeForce GTX 1660 Ti (Turing TU116), **CC 7.5**, 6144 MiB VRAM
- codescout `experiments` @ `52fcaf01`
- `docker-compose.yml` last touched 2026-05-26

## Root cause

Hardware-targeted container tags were chosen for one machine's card and pinned
by digest, with no fallback for lower compute capabilities. TEI publishes
per-architecture images because it ships precompiled CUDA kernels; a `86` build
carries no CC 7.5 kernels.

**Verified:**

- `docker-compose.yml:200` and `:311` both pin tag `86-1.8` (digest
  `sha256:65f792e7…`, identical for both services).
- The host card is CC 7.5 (`nvidia-smi` output above).
- Neither image is in the local store.
- `dense-gpu` (`docker-compose.yml:88`,
  `ghcr.io/ggml-org/llama.cpp:server-cuda@sha256:a04923d3…`) is not
  architecture-pinned in the same way — its CUDA backend ships **multi-arch**
  and includes Turing. Confirmed at runtime:
  `CUDA : ARCHS = 500,610,700,750,800,860,890,1200` (750 = CC 7.5), and
  `load_tensors: offloaded 13/13 layers to GPU` with
  `llama_context: flash_attn = enabled`. **This multi-arch vs single-arch
  difference is the crux of the bug:** llama.cpp ships one image covering many
  capabilities; TEI ships one image per capability, so a TEI pin is a hardware
  pin while a llama.cpp pin is not.
  (Correction: an earlier revision claimed this image was already local. It was
  not — see the correction note in Symptom.)
- The `amd` profile has the mirror-image problem baked in as a comment
  (`docker-compose.yml:225-234`): `sparse-amd` must be *built* from
  `./docker/sparse-amd/Dockerfile` because HF publishes no prebuilt TEI ROCm
  image, with `PYTORCH_ROCM_ARCH: gfx1101` hardcoded. Same class of defect,
  already known on the AMD side.

**Hypothesis — now CONFIRMED (2026-07-25).** That `86` denotes CC 8.6 in TEI's
tagging scheme and that a Turing (CC 7.5) equivalent exists was asserted from
prior knowledge and flagged here as unverified. It has since been checked
against the registry and at runtime:

```
$ for t in turing-1.8 turing-1.7 turing-1.6 turing-latest 86-1.8; do
    docker manifest inspect ghcr.io/huggingface/text-embeddings-inference:$t >/dev/null 2>&1 \
      && echo "$t EXISTS" || echo "$t MISSING"; done
turing-1.8      EXISTS
turing-1.7      EXISTS
turing-1.6      EXISTS
turing-latest   EXISTS
86-1.8          EXISTS
```

`turing-1.8` resolves to
`sha256:bd102b08fbdb23fa2a0c747c8b2d154c521e3fce20441266c005ad7a101143a0`
(~2 GB pull). **`sparse-gpu` on that image reached `healthy` on the GTX 1660 Ti**,
which settles the CC-7.5 compatibility question empirically — not just the tag's
existence. No `no kernel image is available` error occurred.

## Evidence

### GPU capability

`nvidia-smi --query-gpu=name,compute_cap,memory.total,memory.used --format=csv`
→ `NVIDIA GeForce GTX 1660 Ti, 7.5, 6144 MiB, 9 MiB`. VRAM essentially free, so
capacity is not the constraint — only kernel compatibility is.

### Compose profile inventory

`grep(pattern="profiles:", path="docker-compose.yml", context_lines=10)` — nine
profile-gated services: `dense-{cpu,gpu,amd}` (`:50,:87,:133`),
`sparse-{cpu,gpu,amd}` (`:181,:199,:224`), `reranker-{cpu,gpu,amd}`
(`:292,:310,:335`). All three `gpu` services declare
`deploy.resources.reservations.devices: [{driver: nvidia, count: 1,
capabilities: [gpu]}]` (`:123-129`, `:214-220`, `:325-331`).

### Local image store

`docker images` shows a `llama.cpp:server-cuda` tag (5.95 GB) and
`text-embeddings-inference` only at `cpu-1.8` plus one untagged 915 MB layer —
no `86-1.8`, no Turing tag.

**Caveat learned the hard way:** the `server-cuda` tag row did not correspond to
the compose-pinned digest, and `dense-gpu` pulled ~5.95 GB fresh on first `up`.
A tag listing cannot answer "is this digest-pinned image local" — use
`docker images --digests` and compare against the `@sha256:` in the compose
file. Budget a pull for the TEI fix on that basis too.

## Hypotheses tried

1. **Hypothesis:** the whole `gpu` profile is ready to run since the dense image
   is already pulled.
   **Test:** `docker images` filtered for `text-embeddings|llama`; cross-checked
   each `gpu` service's image tag against `nvidia-smi` compute_cap.
   **Verdict:** rejected, and the premise was ALSO wrong — the two TEI services
   are CC-mismatched, and `dense-gpu`'s pinned digest was not local either
   (the matching *tag* was, which is not the same thing). Both halves of the
   original claim failed.
   **Evidence:** Evidence § local image store.

4. **Hypothesis:** `dense-gpu` cannot use flash-attention on CC 7.5, so
   `--flash-attn on` (`docker-compose.yml:112-113`) will fail or silently
   downgrade.
   **Test:** brought `dense-gpu` up on the GTX 1660 Ti and read its startup log.
   **Verdict:** rejected — `llama_context: flash_attn = enabled`, 13/13 layers
   offloaded, healthcheck green. Turing is fine for the dense leg.
   **Evidence:** Evidence § hybrid workaround, verified.

2. **Hypothesis:** VRAM (6 GiB) is the limiting factor for three GPU services.
   **Test:** `nvidia-smi` memory.used → 9 MiB of 6144.
   **Verdict:** rejected as the *primary* blocker. Worth re-checking after the
   kernel issue is resolved, since three CUDA contexts plus bge-reranker-v2-m3
   (568 MB fp16) and SPLADE fp16 on one 6 GiB card is plausible but untested.

3. **Hypothesis:** `86-1.8` will run on CC 7.5 via PTX JIT.
   **Test:** not run.
   **Verdict:** deferred. TEI ships precompiled kernels rather than PTX for
   these builds, so JIT fallback is unlikely, but this is the one path by which
   the current pin might work unchanged.

## Fix
**SHIPPED — confirmed by verify-open pass 2026-07-28.** The status sat at `open`
after the fix landed; `docker-compose.yml` already carries it:

- Line 139 pins
  `ghcr.io/huggingface/text-embeddings-inference:turing-1.8@sha256:bd102b08fbdb23fa2a0c747c8b2d154c521e3fce20441266c005ad7a101143a0`
  — CC 7.5, correct for this GTX 1660 Ti.
- Line 136 records the swap in place: `Was 86-1.8@sha256:65f792e7…`, citing this bug
  file.
- Lines 129-136 generalise the lesson so the next maintainer does not repeat it: TEI
  publishes **one image per CUDA compute capability**, so a TEI digest pin is a
  HARDWARE pin, and both the tag and the digest must be swapped together. Contrast
  llama.cpp, whose single CUDA image ships `ARCHS=500..1200` and runs anywhere —
  which is why the dense and reranker services (lines 55, 185) are not
  hardware-pinned at all.

Only the sparse (TEI) service was ever exposed to this. It is currently stopped and
`CODESCOUT_DISABLE_SPARSE=1` is set, so the pin is not exercised today — but it is
now correct for when it is.

Per CLAUDE.md the **master-side** SHA goes here after cherry-pick.

**Implemented 2026-07-25 on branch `experiments`** (not yet cherry-picked to
`master`; this file stays in `docs/issues/` until it is).

What was done:

1. Verified the tag hypothesis against the registry and captured the digest —
   see Root cause.
2. Replaced the `86-1.8` pins at `docker-compose.yml:200` and `:311` with
   `turing-1.8@sha256:bd102b08…`, digest-pinned as before.
3. **Mutated the `gpu` profile in place rather than adding a `gpu-turing`
   profile.** An earlier revision of this section recommended the separate
   profile for portability. Reversed deliberately: this is a single-GPU-host
   stack (`.env.gpu` is titled "single CUDA card"), and a parallel profile would
   add three more service definitions to a compose file that already carries
   nine. The portability concern is real but is better served by *documentation*
   than by duplication — so instead of a second profile, a comment block above
   `sparse-gpu` now names both tags, their CC targets, and the
   `nvidia-smi --query-gpu=compute_cap` check to re-run when the card changes.
   This mirrors how the repo already handles the AMD arch constraint inline
   (`PYTORCH_ROCM_ARCH: gfx1101` with its explanatory block at `:225-234`).
4. Recorded the multi-arch/single-arch asymmetry in that comment — the point a
   future reader most needs: *a TEI digest pin is a hardware pin; a llama.cpp
   one is not.*

The swap sequence matters and is worth repeating verbatim, because the CPU
containers hold the published ports and the GPU pair cannot bind until they are
released:

```bash
docker stop codescout-sparse-cpu codescout-reranker-cpu
docker compose --profile gpu --env-file .env.gpu up -d sparse-gpu reranker-gpu
```

## Tests added

N/A — compose configuration, no Rust test surface. The verification is
operational: `docker compose --profile <gpu-variant> up -d` followed by
`docker compose ps` showing all three healthy, plus a live
`semantic_search` call. A CI check that every pinned image tag matches the
runner's compute capability would be over-engineering for a single-dev stack.

## Workarounds

Run the `cpu` profile, which works on any host:

```bash
docker compose --profile cpu --env-file .env.amd up -d
```

Cost: SPLADE at `--dtype float32` on CPU averages ~800% CPU under load (see
`docs/issues/2026-07-25-concurrent-index-no-project-lock.md`). Or run a hybrid —
`dense-gpu` (which does work on Turing) alongside `sparse-cpu` /
`reranker-cpu` — since all profiles bind the same host ports
(48081 / 48084 / 48083):

```bash
docker compose --profile gpu --env-file .env.gpu up -d dense-gpu
docker compose --profile cpu --env-file .env.amd up -d sparse-cpu reranker-cpu
```

That frees the ~400% the dense leg costs on CPU while leaving sparse where it
works.

**Verified working 2026-07-25.** Exact commands run (note `--env-file`, which
avoids the stale repo-root `.env` — see
`docs/issues/2026-07-25-env-copy-flow-stale-model-dir.md`):

```bash
docker compose --profile gpu --env-file .env.gpu up -d dense-gpu   # pulls ~5.95 GB first time
docker start codescout-sparse-cpu codescout-reranker-cpu
```

Result: all three healthy, 3/3 embedder ports listening, `13/13` layers on the
GPU at **379 MiB of 6144 MiB** VRAM. `docker start` (rather than compose) for
the two CPU services reuses their existing container config, so no interpolation
and no risk of picking up the stale `.env`.

VRAM headroom is much larger than expected: despite `--ctx-size 65536` and
`--parallel 16`, the breakdown is 67 MiB model + 216 MiB compute. An
embedding model with `--pooling mean` runs non-causally and allocates no KV
cache, so the ctx-size does not translate into VRAM the way it would for
generation. There is ample room to also host the two TEI services once the
tag issue is resolved.

## Resume

First: verify whether a CC 7.5 TEI 1.8 image exists and capture its digest —
this gates everything else and the current assumption is unverified. Then
attempt `docker compose --profile gpu --env-file .env.gpu up -d sparse-gpu`
alone (not the whole profile) and capture the verbatim failure from
`docker compose logs sparse-gpu` into the Symptom section, so the expected-error
claim above becomes an observed one. Keep `sparse-cpu` stopped but not removed
so rollback is a single `docker start`.

## References

- `docker-compose.yml:198-221` — `sparse-gpu`
- `docker-compose.yml:309-332` — `reranker-gpu`
- `docker-compose.yml:86-130` — `dense-gpu` (works on Turing; image local)
- `docker-compose.yml:223-234` — `sparse-amd`, same defect class, documented
- `.env.gpu` — profile env; `CODESCOUT_MODEL_DIR=./models`, endpoints
  48081 / 48084 / 48083
- `docs/issues/2026-07-25-concurrent-index-no-project-lock.md` — the CPU
  saturation incident that prompted the GPU switch
