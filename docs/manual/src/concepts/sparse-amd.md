# SPLADE on ROCm (`sparse-amd`)

> **Status:** stable since v0.12.0. The image is built from source at TEI commit
> `1588129f93`, on the branch of upstream PR #860. That PR merged on 2026-09-15
> (`d246fbf`) but targets AMD Instinct (MI300/MI325), so upstream still ships no
> image for Radeon/RDNA cards. It runs on gfx1101 (RX 7800 XT); other RDNA3 arches
> are untested by us.

The `amd` profile runs SPLADE on the GPU through the `sparse-amd` service, which
builds [text-embeddings-inference][tei] (TEI) from source against ROCm 7.1 +
PyTorch 2.8. To free the card for something else, the `sparse-cpu` profile serves
the same model on the same port from TEI's CPU image; see
[Moving the sparse leg to CPU](#moving-the-sparse-leg-to-cpu).

On a 21k-chunk codescout reindex this drops sparse CPU usage from ~3200 %
(saturating 32 cores) to ~121 % (the Rust router thread plus light Python
overhead) and finishes the full re-embed in 6 m 36 s.

[tei]: https://github.com/huggingface/text-embeddings-inference

## Why this is experimental

- **Upstream ROCm support is Instinct-only.** The AMD path landed as PR
  [#860 (`fa-varlen` branch)][pr], merged 2026-09-15, with CI for MI300/MI325
  only. We pin commit `1588129f93…` from that branch because
  `requirements-amd.txt` and `Dockerfile-amd` landed there post-v1.9.3.
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
curl -X POST 127.0.0.1:48084/embed_sparse \
     -H 'Content-Type: application/json' \
     -d '{"inputs":"async fn cancel()"}'
# → [[{"index":..., "value":...}, ...]]   sparse activations
```

`/health` answers HTTP 200 with an **empty** body, and only says the router is up;
the `/embed_sparse` call above is the check that exercises the model, which is
why the compose healthcheck uses it.

The container logs `Python backend ready in 5.157s` and
`ROCm / HIP version: 7.1.25424` on startup. If you see
`torch.cuda.is_available=False`, the GPU passthrough is misconfigured —
check `/dev/kfd` and `/dev/dri` permissions on the host.

## Compose service

The service is `sparse-amd` in `docker-compose.yml`; this page deliberately does not
restate it, because a copy here drifted from the file once already (it showed a
`PYTORCH_ROCM_ARCH` build arg the Dockerfile never reads — its only `ARG` is
`TEI_REF`). Two parts of that definition are worth knowing before you edit it:

- **Numeric `group_add` (`"44"`, `"992"`).** Docker resolves group names against
  the **image's** `/etc/group`, not the host's. `rocm/pytorch` does not declare a
  `render` group, so passing the name fails with `Unable to find group render`.
  GIDs 44 (video) and 992 (render) match the defaults on Debian/Ubuntu hosts —
  adjust if your host differs (`getent group video render`).
- **`--max-batch-tokens 2048` and `--max-batch-requests 4`.** The token cap counts
  real tokens only, and nothing capped the batch *count*: the server grew from
  2.89 GiB right after a restart to 5.30 GiB after 12 days of serving (measured
  2026-09-26). The request cap bounds each padded batch at 4 × 512 tokens. Its
  effect on reindex throughput is not yet measured against the 6 m 36 s baseline
  above.

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

## Moving the sparse leg to CPU

When the card is needed for something else — a training run that needs the
~2.9 GiB SPLADE holds — move the sparse leg to TEI's CPU image instead of turning
it off. `sparse-cpu` serves the same model on the same host port, so codescout
keeps calling `127.0.0.1:48084` with no client change and no `/mcp` reconnect:

```bash
docker compose --profile amd stop sparse-amd
docker compose --profile sparse-cpu up -d --no-deps sparse-cpu
# ...and back:
docker compose --profile sparse-cpu stop sparse-cpu
docker compose --profile amd up -d --no-deps sparse-amd
```

**Why not `CODESCOUT_DISABLE_SPARSE=1` and stop the container?** Each codescout
server reads that flag once, at startup. Every open session would need `/mcp` to
pick it up, and any that did not would fail every `semantic_search` and index
build with `embed sparse send` until SPLADE came back — there is no dense-only
fallback. Chunks indexed while the flag is on also carry empty sparse vectors,
which needs `index(action="build", force=true)` after re-enabling.

**What the CPU path costs.** It runs float32 where the GPU runs float16, so a
query embedded on CPU differs slightly from chunks embedded on the GPU. Measured
2026-09-27 on five probes (code and prose): cosine ≥ 0.99997 between each probe's
two vectors, and the same token indices apart from one or two extra near-zero
weights (< 0.004) on the CPU side. Individual small weights can differ by more
than their own size, so compare vectors, not entries. A temporary swap needs no
reindex. Idle, the CPU server holds ~1.1 GiB of RAM and ~0.3% CPU; the swap itself
leaves `:48084` down for about a minute (57 s measured, mostly TEI's CPU warmup),
and a full reindex on CPU saturates the cores, which is what this page's GPU build
was made to avoid.

## Known issues

- **Image size ~12 GB.** Runtime stage carries the full `rocm/pytorch`
  base. A multi-stage trim that copies only `/opt/venv` + ROCm runtime
  libraries onto a smaller base is feasible but not yet attempted.
- **Cold start 5 s.** The Python backend imports torch + transformers at
  startup. Live latency after warm is in the same ballpark as TEI-on-CUDA.
- **Verified on gfx1101 (RX 7800 XT) only.** gfx1100, gfx1030 and the MI series
  should work (PyTorch SDPA is arch-agnostic) but have not been tested by us.
