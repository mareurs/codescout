"""The study's own retrieval stack: copies of the operator's live services, never the live ones.

The live `codescout-qdrant` holds the live repository's index, which contains the answers, so
the study never reaches it. Every container here is a study-owned copy on the study's internal
Docker network, and every Qdrant a run sees opens a fresh volume copied from a template, so no
run sees what an earlier run wrote. Stdlib only; all docker calls go through `pilot.docker`.

Names (every object this module creates starts with `sab-`):

  sab-qdrant           template-building Qdrant from `stack_up`, on volume `sab-qv-index`
  sab-dense            copy of `codescout-dense-amd` (llama-server, CodeRankEmbed, ROCm GPU)
  sab-sparse           copy of `codescout-sparse-cpu` (TEI, Splade_PP_en_v1), offline
  sab-model-cache      copy of the live `codescout-retrieval_model_cache`, made once, read-only
  sab-qdrant-<run>     the per-run Qdrant from `qdrant_for_run`, on volume `sab-qv-<run>`

Which Qdrant the agent sees: during a run the agent's CODESCOUT_QDRANT_URL is the URL that
`qdrant_for_run` returns (`http://sab-qdrant-<run>:6334`), never `sab-qdrant`. `sab-qdrant` only
exists to build the index that the caller then copies into a template volume (stop it first so
the copy is consistent); the template volume is the caller's to name, and it must start with
`sab-` but not `sab-qv-`, so that `stack_down` leaves it in place.

Service URL -> codescout env key (the keys Task 4 rewrites in the manifest's `mcp_env`). The
live shapes were measured on 2026-10-27 in the kat/sdd `.claude.json` codescout env:

  CODESCOUT_QDRANT_URL           http://127.0.0.1:6334      -> http://sab-qdrant:6334 (gRPC)
  CODESCOUT_EMBEDDER_URL         http://127.0.0.1:48081     -> http://sab-dense:8080
  CODESCOUT_EMBED_URL            http://127.0.0.1:48081/v1  -> http://sab-dense:8080/v1
  LIBRARIAN_EMBED_URL            http://127.0.0.1:48081/v1  -> http://sab-dense:8080/v1
  CODESCOUT_SPARSE_EMBEDDER_URL  http://127.0.0.1:48084     -> http://sab-sparse:80
  CODESCOUT_RERANKER_URL         http://127.0.0.1:48083     -> never returned: the reranker is
                                 absent in daily use, so the study has no copy of it and the
                                 caller drops the key rather than point it anywhere.

`stack_up` returns these keys for every service it started; a manifest env may carry a subset
(the main profile has no CODESCOUT_EMBED_URL / LIBRARIAN_EMBED_URL), so the caller rewrites the
intersection and drops any URL key it has no study URL for.
"""
import re
import subprocess
import time

import pilot

# Short-lived helper for volume copies; pinned by the local repo digest (2026-10-27).
HELPER_IMAGE = "alpine@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6"

QDRANT_IMAGE = ("qdrant/qdrant:v1.17.0"
                "@sha256:f1c7272cdac52b38c1a0e89313922d940ba50afd90d593a1605dbbc214e66ffb")
DENSE_IMAGE = ("rocm/llama.cpp:llama.cpp-b6652.amd0_rocm7.0.0_ubuntu24.04_server"
               "@sha256:be175932c3c96e882dfbc7e20e0e834f58c89c2925f48b222837ee929dfc47ee")
SPARSE_IMAGE = ("ghcr.io/huggingface/text-embeddings-inference:cpu-1.8"
                "@sha256:8de25e75ce39617f17f2f6c77d60a4f75b65e779ed5005420eb1400072a15c1c")

LIVE_MODEL_CACHE = "codescout-retrieval_model_cache"
MODEL_CACHE = "sab-model-cache"
RUN_VOLUME_PREFIX = "sab-qv-"
INDEX_VOLUME = RUN_VOLUME_PREFIX + "index"
QDRANT_GRPC = 6334

STARTUP_TIMEOUT = 300.0
POLL_INTERVAL = 2.0
PROBE_TIMEOUT = 20

# Readiness probes run inside each container, copying the live containers' health checks
# (so each probe uses a tool the image is known to carry). Qdrant must also open gRPC 6334,
# the port codescout uses.
PROBES = {
    "qdrant": ["bash", "-c", "</dev/tcp/127.0.0.1/6333 && </dev/tcp/127.0.0.1/6334"],
    "dense": ["curl", "-fsS", "-o", "/dev/null", "http://127.0.0.1:8080/health"],
    "sparse": ["curl", "-fsS", "-m", "10", "-X", "POST", "http://127.0.0.1/embed_sparse",
               "-H", "Content-Type: application/json", "-d", '{"inputs":"health"}',
               "-o", "/dev/null"],
}

_RUN = re.compile(r"[a-z0-9][a-z0-9_.-]*")


def _qdrant_args(name, volume):
    return ["--name", name, "-v", f"{volume}:/qdrant/storage", "-e", "QDRANT__LOG_LEVEL=INFO",
            QDRANT_IMAGE]


def _dense_args():
    # Copy of codescout-dense-amd (docker inspect, 2026-10-27); the models mount is read-only.
    return ["--name", "sab-dense", "--entrypoint", "/app/llama-server",
            "--device", "/dev/kfd", "--device", "/dev/dri",
            "--group-add", "video", "--group-add", "render",
            "-v", "/home/marius/models:/models:ro", "-e", "LLAMA_ARG_HOST=0.0.0.0",
            DENSE_IMAGE,
            "--model", "/models/CodeRankEmbed-Q4_K_M.gguf", "--host", "0.0.0.0", "--port", "8080",
            "--embedding", "--pooling", "mean", "--ctx-size", "65536", "--batch-size", "4096",
            "--ubatch-size", "4096", "--parallel", "16", "--n-gpu-layers", "999",
            "--flash-attn", "on"]


def _sparse_args():
    # Copy of codescout-sparse-cpu; the container is offline, so the hub must not be tried.
    return ["--name", "sab-sparse", "--entrypoint", "text-embeddings-router",
            "-v", f"{MODEL_CACHE}:/data:ro",
            "-e", "HUGGINGFACE_HUB_CACHE=/data", "-e", "PORT=80", "-e", "HF_HUB_OFFLINE=1",
            SPARSE_IMAGE,
            "--model-id", "prithivida/Splade_PP_en_v1", "--pooling", "splade",
            "--dtype", "float32", "--auto-truncate"]


def _urls(service, host):
    if service == "qdrant":
        return {"CODESCOUT_QDRANT_URL": f"http://{host}:{QDRANT_GRPC}"}
    if service == "dense":
        base = f"http://{host}:8080"
        return {"CODESCOUT_EMBEDDER_URL": base, "CODESCOUT_EMBED_URL": base + "/v1",
                "LIBRARIAN_EMBED_URL": base + "/v1"}
    if service == "sparse":
        return {"CODESCOUT_SPARSE_EMBEDDER_URL": f"http://{host}:80"}
    raise ValueError(f"no study copy of service {service!r}")


def _docker(docker):
    return docker if docker is not None else pilot.docker


def _volume_exists(docker, name):
    return docker("volume", "inspect", name, check=False).returncode == 0


def _fresh_volume(docker, name):
    """Create `name`, refusing one that already exists: a run never opens earlier writes."""
    if _volume_exists(docker, name):
        raise RuntimeError(f"volume {name} already exists; run stack_down() first, a run "
                           f"never reuses a volume an earlier run wrote")
    docker("volume", "create", name)


def copy_volume(src, dst, *, docker=None):
    """Copy every file of volume `src` (mounted read-only) into volume `dst`."""
    _docker(docker)("run", "--rm", "--network", "none", "-v", f"{src}:/s:ro", "-v", f"{dst}:/d",
                    HELPER_IMAGE, "cp", "-a", "/s/.", "/d/", timeout=1800)


def _require_internal(docker, network):
    internal = docker("network", "inspect", "-f", "{{.Internal}}", network).stdout.strip()
    if internal != "true":
        raise RuntimeError(f"network {network} is not internal (Internal={internal!r}); the "
                           f"study stack only runs on an internal network")


def _start(docker, network, args):
    docker("run", "-d", "--network", network, *args)


def _ready(docker, name, probe):
    try:
        return docker("exec", name, *probe, check=False, timeout=PROBE_TIMEOUT).returncode == 0
    except subprocess.TimeoutExpired:
        return False


def wait_ready(names, *, docker=None, clock=time.monotonic, sleep=time.sleep,
               timeout=STARTUP_TIMEOUT):
    """Poll each {container: probe} until it answers; raise naming the first that does not."""
    docker = _docker(docker)
    deadline = clock() + timeout
    pending = dict(names)
    while pending:
        for name, probe in list(pending.items()):
            if _ready(docker, name, probe):
                del pending[name]
                continue
            running = docker("inspect", "-f", "{{.State.Running}}", name, check=False)
            if running.stdout.strip() != "true":
                logs = docker("logs", "--tail", "40", name, check=False).stdout
                raise RuntimeError(f"{name} exited before it was ready:\n{logs}")
        if not pending:
            break
        if clock() >= deadline:
            raise RuntimeError(f"{', '.join(pending)} not ready after {timeout:.0f}s")
        sleep(POLL_INTERVAL)


def stack_up(network, manifest, *, docker=None, clock=time.monotonic, sleep=time.sleep,
             timeout=STARTUP_TIMEOUT):
    """Start one study container per service the manifest marks present; return env-key URLs."""
    retrieval = manifest["retrieval"]
    services = [s for s in ("qdrant", "dense", "sparse", "reranker") if retrieval[s]]
    if "reranker" in services:
        raise ValueError("manifest marks the reranker present, but the study has no copy of "
                         "it; re-measure (freeze) before inventing a reranker URL")
    docker = _docker(docker)
    _require_internal(docker, network)

    if "qdrant" in services:
        _fresh_volume(docker, INDEX_VOLUME)
    if "sparse" in services and not _volume_exists(docker, MODEL_CACHE):
        docker("volume", "create", MODEL_CACHE)
        try:
            copy_volume(LIVE_MODEL_CACHE, MODEL_CACHE, docker=docker)
        except BaseException:
            # A half-copied cache must not pass for a populated one next time.
            docker("volume", "rm", MODEL_CACHE, check=False)
            raise

    args = {"qdrant": lambda: _qdrant_args("sab-qdrant", INDEX_VOLUME),
            "dense": _dense_args, "sparse": _sparse_args}
    urls = {}
    for service in services:
        _start(docker, network, args[service]())
        urls.update(_urls(service, f"sab-{service}"))
    wait_ready({f"sab-{s}": PROBES[s] for s in services}, docker=docker, clock=clock,
               sleep=sleep, timeout=timeout)
    return urls


def qdrant_for_run(template_volume, run, *, network, docker=None, clock=time.monotonic,
                   sleep=time.sleep, timeout=STARTUP_TIMEOUT):
    """Copy `template_volume` into a fresh `sab-qv-<run>`, start Qdrant on it, return its URL.

    `network` is keyword-only and required (the brief's signature had none): the per-run Qdrant
    must join the same internal network the agent runs on.
    """
    if not template_volume.startswith("sab-"):
        raise ValueError(f"template volume {template_volume!r} is not a study (sab-) volume; "
                         f"a live volume is never copied into a run")
    if not _RUN.fullmatch(run):
        raise ValueError(f"run name {run!r} must match {_RUN.pattern}")
    docker = _docker(docker)
    if not _volume_exists(docker, template_volume):
        raise RuntimeError(f"template volume {template_volume} does not exist")
    _require_internal(docker, network)
    volume, name = RUN_VOLUME_PREFIX + run, f"sab-qdrant-{run}"
    _fresh_volume(docker, volume)
    copy_volume(template_volume, volume, docker=docker)
    _start(docker, network, _qdrant_args(name, volume))
    wait_ready({name: PROBES["qdrant"]}, docker=docker, clock=clock, sleep=sleep, timeout=timeout)
    return _urls("qdrant", name)["CODESCOUT_QDRANT_URL"]


def stack_down(prefix="sab-", *, docker=None):
    """Remove containers named `prefix*` and per-run volumes named `sab-qv-*` under `prefix`.

    Never anything else: Codex runs `pbr27-*`/`rae-*` containers on this host and the live
    stack is `codescout-*`. `docker ps --filter name=` is a substring match, so the prefix test
    is done here. The template volume and `sab-model-cache` are kept.
    """
    if not prefix.startswith("sab-"):
        raise ValueError(f"prefix {prefix!r} is outside the study namespace (sab-)")
    docker = _docker(docker)
    names = docker("ps", "-a", "--format", "{{.Names}}").stdout.split()
    containers = [n for n in names if n.startswith(prefix)]
    if containers:
        docker("rm", "-f", *containers)
    vols = docker("volume", "ls", "--format", "{{.Name}}").stdout.split()
    volumes = [v for v in vols if v.startswith(prefix) and v.startswith(RUN_VOLUME_PREFIX)]
    if volumes:
        docker("volume", "rm", *volumes)
    return {"containers": containers, "volumes": volumes}
