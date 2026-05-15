# SPLADE on ROCm (`sparse-amd`)

> **Status:** experimental. The image is built from a not-yet-merged upstream
> PR and verified on gfx1100. Other RDNA3 / CDNA arches will probably work but
> have not been tested by us.

The default `amd` profile keeps SPLADE on CPU because upstream
[text-embeddings-inference][tei] (TEI) does not ship a ROCm release. The
`sparse-amd` service brings sparse encoding onto the GPU by building TEI from
source against ROCm 7.1 + PyTorch 2.8.

On a 21k-chunk codescout reindex this drops sparse CPU usage from ~3200 %
(saturating 32 cores) to ~121 % (the Rust router thread plus light Python
overhead) and finishes the full re-embed in 6 m 36 s.

[tei]: https://github.com/huggingface/text-embeddings-inference

## Why this is experimental

- **Upstream PR not merged.** The AMD path lives on PR
  [#860 (`fa-varlen` branch)][pr]. We pin commit `1588129f93…` because
  `requirements-amd.txt` and `Dockerfile-amd` landed there post-v1.9.3. If
  PR #860 changes, you may need to rebuild.
- **gfx1100 has no upstream flash-attention.** Upstream PR #860 builds
  ROCm/flash-attention pinned to gfx942 (MI300). RDNA3 is not supported by
  that fork. Our Dockerfile skips the flash-attn build and relies on PyTorch
  SDPA fallback (wired by upstream PR #853). Functionally correct, slower
  than MI300 would be.
- **Heavy image.** ~12 GB because the runtime stage keeps the full
  `rocm/pytorch` base. A leaner runtime stage is on the TODO list.

[pr]: https://github.com/huggingface/text-embeddings-inference/pull/860

## Bring up

The service is part of the `amd` profile in `docker-compose.yml`:

```bash
docker compose --profile amd up -d --build sparse-amd
```

First build is ~25 minutes (Rust router compile + Python deps + ROCm
PyTorch). Subsequent runs reuse the image.

Verify:

```bash
curl 127.0.0.1:48084/health     # {"status":"Ok"}
curl -X POST 127.0.0.1:48084/embed_sparse \
     -H 'Content-Type: application/json' \
     -d '{"inputs":"async fn cancel()"}'
# → [[{"index":..., "value":...}, ...]]   sparse activations
```

The container logs `Python backend ready in 5.157s` and
`ROCm / HIP version: 7.1.25424` on startup. If you see
`torch.cuda.is_available=False`, the GPU passthrough is misconfigured —
check `/dev/kfd` and `/dev/dri` permissions on the host.

## Compose service

```yaml
sparse-amd:
  profiles: [amd]
  build:
    context: ./docker/sparse-amd
    dockerfile: Dockerfile
    args:
      TEI_REF: 1588129f932125a780ab97ccb300e7774b02d230
      PYTORCH_ROCM_ARCH: gfx1100
  image: codescout/sparse-amd:tei-1588129f93
  container_name: codescout-sparse-amd
  ports: ["127.0.0.1:48084:80"]
  devices: [/dev/kfd, /dev/dri]
  group_add: ["44", "992"]   # video, render — numeric: rocm/pytorch image lacks 'render' group
  shm_size: 8g
```

The numeric `group_add` is intentional. Docker resolves group names against
the **image's** `/etc/group`, not the host's. `rocm/pytorch` does not declare
a `render` group, so passing the name fails with
`Unable to find group render`. GIDs 44 (video) and 992 (render) match the
defaults on Debian/Ubuntu hosts — adjust if your host differs.

## Deviations from upstream PR #860

We follow upstream where possible. Three intentional differences:

1. **Skip the flash-attention build.** Upstream pins ROCm/flash-attention to
   gfx942. We delete that build step; PyTorch SDPA covers the gap.
2. **Force-reinstall numpy / scipy / scikit-learn after `make install`.**
   `requirements-amd.txt` pins `numpy==1.26.4` and an old `accelerate` that
   wants `numpy<2`. The `rocm/pytorch` base image ships numpy 2.x and
   scipy 1.15 already, so the downgrade leaves `scipy._fitpack_impl` linked
   against the wrong numpy ABI and import fails with a `TypeError`. We
   restore the base versions instead.
3. **Add three missing deps.** `more_itertools`, `psutil`, and
   `backports.tarfile` are transitive requirements of `transformers` that
   the rocm/pytorch slim env doesn't ship. Without them the Python backend
   crashes on import.

If you hit different ABI breakage, the upstream Makefile workflow
(`cd backends/python/server && make install`) is the reproducible
starting point.

## Wiring

The default `.env.amd` already points sparse at `127.0.0.1:48084`. No env
change is needed when you swap `sparse-cpu` → `sparse-amd`; the codescout
client only cares about the URL and the protocol (TEI's `/embed_sparse`),
both of which match.

```bash
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084
```

## Known issues

- **Image size ~12 GB.** Runtime stage carries the full `rocm/pytorch`
  base. A multi-stage trim that copies only `/opt/venv` + ROCm runtime
  libraries onto a smaller base is feasible but not yet attempted.
- **Cold start 5 s.** The Python backend imports torch + transformers at
  startup. Live latency after warm is in the same ballpark as TEI-on-CUDA.
- **gfx1100 only verified.** gfx1030, gfx1101, MI series should work
  (PyTorch SDPA is arch-agnostic) but have not been tested.
