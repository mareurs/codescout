"""$0 smoke checks (final review I-8 and the private-path note): no token, no model call.

    init_capture(arm, image, manifest, out)      `claude -p` in a sab-smoke-* container of an arm
                                                 image, no credential and no network: the CLI
                                                 prints its stream-json init record, then fails
                                                 "Not logged in" before any request. Returns the
                                                 init summary (native tools, MCP tools, skills).
    ra_peak(image, manifest, out, calls)         codescout's MCP server (mcp_probe.py) in a
                                                 cs-arm container, the given LSP calls, then the
                                                 container cgroup's memory.peak.

The argv is the runner's own (arms.arm_invocation through run.cli_argv: stream-json --verbose
--include-hook-events, the Stage 3 per-call cap), the manifest the frozen one
(frozen.frozen_manifest), the image the bundle's verified arm image. The env file holds only
arms.CONTAINER_ENV: no token, no proxy; the container runs with `--network none`, so nothing can
reach the API even by mistake. Every container is `sab-smoke-<arm>-<uuid8>`, created and removed
here; no volume is created. Evidence goes under setup-ab-data/controls/ (never git).

    python3 smoke.py init --arm full --task burntsushi__ripgrep-2576 --out <dir>
    python3 smoke.py ra --task cs-p08 --out <dir>
"""
import argparse
import json
import subprocess
import time
import uuid
from pathlib import Path

import arms
import frozen
import pilot
import run
import tasks

HERE = Path(__file__).resolve().parent
MODEL = "claude-sonnet-5-5"
PROMPT = "List the files in the repository root.\n"
CREDENTIAL_ENV = ("CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN",
                  "HTTPS_PROXY", "HTTP_PROXY", "https_proxy", "http_proxy")
# Study-host placeholders (arms only lets sab- hosts into a container); nothing answers them
# under --network none.
SMOKE_URLS = {"CODESCOUT_QDRANT_URL": "http://sab-smoke-qdrant:6334",
              "CODESCOUT_EMBEDDER_URL": "http://sab-smoke-dense:8080",
              "CODESCOUT_SPARSE_EMBEDDER_URL": "http://sab-smoke-sparse:80"}


def docker(*args, **kw):
    return pilot.docker(*args, **kw)


def summarize(stdout):
    """The init record's tool, MCP-tool and skill lists, and the last record's shape."""
    events = []
    for line in stdout.splitlines():
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        if isinstance(rec, dict):
            events.append(rec)
    init = next((e for e in events if e.get("type") == "system" and e.get("subtype") == "init"),
                None)
    last = events[-1] if events else {}
    out = {"events": len(events), "init_emitted": init is not None,
           "last": {k: last.get(k) for k in ("type", "subtype", "is_error", "result",
                                             "total_cost_usd", "duration_api_ms")}}
    if init:
        tools = [t for t in init.get("tools") or [] if isinstance(t, str)]
        out.update(native_tools=sorted(t for t in tools if not t.startswith("mcp__")),
                   mcp_tools=sorted(t for t in tools if t.startswith("mcp__")),
                   mcp_servers=init.get("mcp_servers"),
                   skills=sorted(s for s in init.get("skills") or [] if isinstance(s, str)),
                   apiKeySource=init.get("apiKeySource"))
    return out


def _start(arm, image, manifest, run_dir, name):
    spec = arms.arm_invocation(arm, MODEL, PROMPT, manifest, SMOKE_URLS, run_dir,
                               session_id=str(uuid.uuid4()))
    env_file = run_dir / "smoke.env"
    env_file.write_text(arms.env_file_lines(spec))  # CONTAINER_ENV only: no token, no proxy
    docker(*arms.docker_run_args(name, image, "none", spec, env_file))
    env = docker("exec", name, "env", check=False).stdout
    leaked = [l.split("=")[0] for l in env.splitlines() if l.split("=")[0] in CREDENTIAL_ENV]
    if leaked:
        raise RuntimeError(f"credential or proxy env in {name}: {leaked}")
    return spec


def init_capture(arm, image, manifest, out, timeout=300):
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    name = f"sab-smoke-{arm}-{uuid.uuid4().hex[:8]}"
    record = {"arm": arm, "image": image, "container": name, "network": "none",
              "token": "none", "model": MODEL}
    try:
        spec = _start(arm, image, manifest, out / arm, name)
        argv = run.cli_argv(spec.cli_args, run.STAGE3_PER_CALL_USD)
        record["argv_flags"] = argv[:argv.index("--")]
        started = time.monotonic()
        try:
            p = subprocess.run(["docker", "exec", "-w", tasks.WORKDIR, name, *argv],
                               capture_output=True, text=True, timeout=timeout)
            stdout, stderr, rc = p.stdout, p.stderr, p.returncode
        except subprocess.TimeoutExpired as e:
            text = lambda b: b.decode(errors="replace") if isinstance(b, bytes) else (b or "")
            stdout, stderr, rc = text(e.stdout), text(e.stderr), "timeout"
        (out / f"{arm}.stream.jsonl").write_text(stdout)
        (out / f"{arm}.stderr.txt").write_text(stderr)
        record.update(exit_code=rc, seconds=round(time.monotonic() - started, 1),
                      **summarize(stdout))
    finally:
        record["rm"] = docker("rm", "-f", name, check=False).returncode
    (out / f"{arm}.summary.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def ra_peak(image, manifest, out, calls, timeout=1500, concurrent=None):
    """`concurrent`, when given, is a shell command started with `docker exec -d` in the
    workdir just before the probe (fix round 2: an agent's own `cargo build` beside
    rust-analyzer); its output goes to /tmp/concurrent.log, copied out at the end."""
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    name = f"sab-smoke-ra-{uuid.uuid4().hex[:8]}"
    record = {"image": image, "container": name, "network": "none", "calls": calls,
              "concurrent": concurrent, "resources": list(arms.AGENT_RESOURCES)}
    try:
        _start("cs", image, manifest, out / "cs", name)
        host = json.loads(docker("inspect", "-f", "{{json .HostConfig}}", name).stdout)
        record["host_config"] = {k: host.get(k) for k in ("Memory", "MemorySwap",
                                                          "MemorySwappiness", "NanoCpus")}
        (out / "calls.json").write_text(json.dumps(calls))
        docker("cp", str(HERE / "mcp_probe.py"), f"{name}:/tmp/mcp_probe.py")
        docker("cp", str(out / "calls.json"), f"{name}:/tmp/calls.json")
        started = time.monotonic()
        if concurrent:
            docker("exec", "-d", "-w", tasks.WORKDIR, name, "sh", "-c",
                   f"( {concurrent} ) > /tmp/concurrent.log 2>&1; echo exit=$? >> "
                   f"/tmp/concurrent.log")
        r = docker("exec", "-w", tasks.WORKDIR, "-e", f"MCP_PROBE_TIMEOUT={timeout}", name,
                   "python3", "/tmp/mcp_probe.py", arms.MCP_CONFIG, "/tmp/calls.json",
                   check=False, timeout=timeout + 120)
        record["seconds"] = round(time.monotonic() - started, 1)
        (out / "probe.json").write_text(r.stdout)
        log = docker("exec", name, "cat", "/tmp/mcp-probe-server.log", check=False).stdout
        (out / "server.log").write_text(log)
        if concurrent:
            clog = docker("exec", name, "cat", "/tmp/concurrent.log", check=False).stdout
            (out / "concurrent.log").write_text(clog)
            record["concurrent_tail"] = clog.splitlines()[-3:]
        record["probe_exit"] = r.returncode
        cg = {f: docker("exec", name, "cat", f"/sys/fs/cgroup/{f}", check=False).stdout.strip()
              for f in ("memory.peak", "memory.max", "memory.events", "memory.current",
                        "memory.stat", "memory.swap.peak", "memory.swap.max",
                        "memory.swap.current")}
        state = json.loads(docker("inspect", "-f", "{{json .State}}", name).stdout)
        record.update(cgroup=cg, oom_killed=state.get("OOMKilled"),
                      ps_after=docker("exec", name, "ps", "-eo", "pid,rss,comm",
                                      check=False).stdout)
        try:
            probe = json.loads(r.stdout)
            record["tools"] = probe.get("tools")
            record["call_results"] = [
                {"tool": c["tool"], "seconds": c["seconds"], "error": c["error"],
                 "probe_error": c["probe_error"],
                 "text_head": "\n".join(x.get("text", "") for x in
                                        (c.get("result") or {}).get("content", []))[:400]}
                for c in probe.get("calls", [])]
        except ValueError:
            record["probe_stdout_tail"] = r.stdout[-2000:]
    finally:
        record["rm"] = docker("rm", "-f", name, check=False).returncode
    (out / "ra-memory.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def _arm_image(task_id):
    return tasks.bundle_run_image(tasks.DATA / "tasks" / task_id / "bundle.json")


def _manifest(data):
    manifest = json.loads((Path(data) / "setup-manifest.json").read_text())
    rec = json.loads((frozen.FROZEN / frozen.RECORD).read_text())
    frozen.check_trees(frozen.FROZEN, rec["trees"])
    return frozen.frozen_manifest(manifest, frozen.FROZEN)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="action", required=True)
    i = sub.add_parser("init")
    i.add_argument("--arm", choices=arms.ARMS, required=True)
    i.add_argument("--task", required=True)
    i.add_argument("--out", type=Path, required=True)
    r = sub.add_parser("ra")
    r.add_argument("--task", required=True)
    r.add_argument("--out", type=Path, required=True)
    r.add_argument("--calls", type=Path, required=True, help="JSON [[tool, arguments], ...]")
    r.add_argument("--concurrent", default=None,
                   help="a shell command run beside the probe (e.g. a cargo build)")
    a = ap.parse_args(argv)
    manifest = _manifest(tasks.DATA / "study")
    image = _arm_image(a.task)
    if a.action == "init":
        rec = init_capture(a.arm, image, manifest, a.out)
    else:
        rec = ra_peak(image, manifest, a.out, json.loads(a.calls.read_text()),
                      concurrent=a.concurrent)
    print(json.dumps({k: v for k, v in rec.items() if k not in ("skills", "ps_after")},
                     indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
