"""Arm invocations: the three arms differ only in Claude Code CLI flags and mounts.

  bare  the Codex `pilot.run_pilot` argument list (pilot.py:233-238) minus `--effort`, with
        `--model <model>`: native tools, `--strict-mcp-config` with no server, an empty
        CLAUDE_CONFIG_DIR, `--setting-sources local` and an empty `--settings` file.
  cs    bare + `--mcp-config /opt/sab/mcp.json` (codescout, its env rewritten to the study
        stack) + `--plugin-dir /opt/plugins/codescout-companion`.
  full  cs + one `--plugin-dir` per `plugins_full` entry (the companion is not repeated), with
        the frozen CLAUDE.md and settings.json copied into the run's config dir.

What is the same in every arm, by construction: the image (the `bare` arm runs in the same
agent-cs image, so the bytes the agent can read are the same), the CLI binary, the model, the
prompt bytes, the container env (`ArmSpec.env`), the resource flags (`RESOURCES`) and the
network. The codescout env lives in the mounted mcp.json, never in the container env.

URL rewrite (controller ruling): an `mcp_env` key is rewritten only when the study stack
returned a URL for it; `CODESCOUT_RERANKER_URL` is dropped (the reranker is absent in daily
use); any other value still naming the host (127.0.0.1, localhost, host.docker.internal) makes
`arm_invocation` raise, so a host URL never reaches a container.

Prepare time (`prepare_index`): the agent-cs image cannot build the index (a docker build cannot
join the study's internal network), so `codescout index -p /testbed` runs in a container of the
image on the study network and the result is committed as the run image all arms share.

Stdlib only. Never uses pilot.PREFIX/NETWORK/IMAGE/DATA/ORDER (ruling R1).
"""
import json
import shutil
import time
import uuid
from pathlib import Path
from typing import NamedTuple

import pilot

ARMS = ("bare", "cs", "full")

# Paths inside the agent container.
CLI = "/opt/claude"
SETTINGS = "/opt/settings.json"
CONFIG_DIR = "/run/config"
MCP_CONFIG = "/opt/sab/mcp.json"
PLUGINS_ROOT = "/opt/plugins"
COMPANION = "codescout-companion"
SAB_BIN = "/opt/sab/bin"
CODESCOUT_BIN = SAB_BIN + "/codescout"

# The daily MCP entry, measured 2026-10-27 in all three profiles' .claude.json:
# {"type": "stdio", "command": "/home/marius/.cargo/bin/codescout", "args": ["start", "--debug"]}.
# The manifest does not record args (Task 2), so they are pinned here.
CODESCOUT_ARGS = ["start", "--debug"]

# Copied from pilot.run_pilot (pilot.py:233-238).
NATIVE_TOOLS = "Bash,Read,Write,Edit,Glob,Grep"
MAX_BUDGET_USD = "1.25"
# Copied from pilot.run_pilot's `docker run` (--cpus 4 --memory 8g). Identical in every arm.
RESOURCES = ("--cpus", "4", "--memory", "8g")
# The non-secret part of pilot's private.env; the token and proxy are the runner's (Task 8).
CONTAINER_ENV = {"CLAUDE_CONFIG_DIR": CONFIG_DIR, "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "8192",
                 "DISABLE_AUTOUPDATER": "1"}

DROPPED_ENV_KEYS = ("CODESCOUT_RERANKER_URL",)
HOST_MARKERS = ("127.0.0.1", "localhost", "host.docker.internal")


class ArmSpec(NamedTuple):
    cli_args: list
    mounts: list  # (host_path, container_path, "ro" | "rw")
    env: dict


def _names_host(value):
    return any(marker in str(value) for marker in HOST_MARKERS)


def study_mcp_env(mcp_env, urls):
    """The manifest's codescout env with its URL keys pointed at the study stack."""
    bad = {k: v for k, v in urls.items() if _names_host(v)}
    if bad:
        raise ValueError(f"study URLs name the host: {bad}")
    env = {}
    for key, value in mcp_env.items():
        if key in DROPPED_ENV_KEYS:
            continue
        env[key] = urls.get(key, value)
    left = {k: v for k, v in env.items() if _names_host(v)}
    if left:
        raise ValueError(f"codescout env keys still name the host and the study stack has no "
                         f"URL for them: {left}; a host URL never enters a container")
    return env


def _bare_args(model, session_id, task_prompt, extra):
    # pilot.py:233-238 minus `--effort <effort>`; `extra` goes in front of the prompt separator.
    return [CLI, "-p", "--model", model, "--session-id", session_id,
            "--setting-sources", "local", "--settings", SETTINGS,
            "--strict-mcp-config", "--dangerously-skip-permissions",
            "--tools", NATIVE_TOOLS,
            "--output-format", "json", "--max-budget-usd", MAX_BUDGET_USD,
            *extra, "--", task_prompt]


def arm_invocation(arm, model, task_prompt, manifest, urls, run_dir, *, session_id=None):
    """Write the run's config files under `run_dir` and return the arm's CLI args and mounts.

    `session_id` is keyword-only (an addition to the brief's signature): the Codex list carries
    `--session-id`, and the runner must know it to find the transcript. A fresh UUID otherwise.
    """
    if arm not in ARMS:
        raise ValueError(f"unknown arm {arm!r}; expected one of {ARMS}")
    session_id = session_id or str(uuid.uuid4())
    # Validate before writing anything, so a refused env leaves no half-made run dir.
    mcp_env = study_mcp_env(manifest["mcp_env"], urls) if arm != "bare" else None
    run_dir = Path(run_dir)
    config = run_dir / "config"
    config.mkdir(parents=True, exist_ok=False)
    settings = run_dir / "settings.json"
    settings.write_text("{}\n")  # pilot writes `settings or {}`; no arm gets settings here
    mounts = [(str(config), CONFIG_DIR, "rw"),
              (manifest["cli"]["path"], CLI, "ro"),
              (str(settings), SETTINGS, "ro")]
    extra = []
    if arm in ("cs", "full"):
        sab = run_dir / "sab"
        sab.mkdir()
        mcp = {"mcpServers": {"codescout": {
            "type": "stdio", "command": CODESCOUT_BIN, "args": list(CODESCOUT_ARGS),
            "env": mcp_env}}}
        (sab / "mcp.json").write_text(json.dumps(mcp, indent=2) + "\n")
        companion = f"{PLUGINS_ROOT}/{COMPANION}"
        mounts += [(str(sab / "mcp.json"), MCP_CONFIG, "ro"),
                   (manifest["plugins_cs"]["install_path"], companion, "ro")]
        extra += ["--mcp-config", MCP_CONFIG, "--plugin-dir", companion]
    if arm == "full":
        for plugin in manifest["plugins_full"]:
            if plugin["name"] == COMPANION:
                continue  # already mounted and passed by the cs layer
            target = f"{PLUGINS_ROOT}/{plugin['name']}"
            mounts.append((plugin["install_path"], target, "ro"))
            extra += ["--plugin-dir", target]
        for src in manifest["full_config_files"]:
            shutil.copyfile(src, config / Path(src).name)
    return ArmSpec(cli_args=_bare_args(model, session_id, task_prompt, extra),
                   mounts=mounts, env=dict(CONTAINER_ENV))


def docker_run_args(name, image, network, spec, env_file):
    """`docker run` argv for an agent container; only the `-v` mounts vary with the arm.

    The env goes through `env_file` (the runner adds its secret lines to the same file), and the
    container idles so the CLI can be `docker exec`-ed, as in pilot.run_pilot.
    """
    argv = ["run", "-d", "--name", name, "--network", network, *RESOURCES,
            "--env-file", str(env_file),
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges"]
    for host, target, mode in spec.mounts:
        argv += ["-v", f"{host}:{target}:{mode}"]
    return argv + [image, "sleep", "infinity"]


def env_file_lines(spec):
    """The non-secret lines of the run's env file (identical across arms)."""
    return "".join(f"{k}={v}\n" for k, v in sorted(spec.env.items()))
def prepare_index(image, out_image, network, env, data_dir, *, project="/testbed",
                  run="index", docker=None, clock=time.monotonic, timeout=7200):
    """Index `project` in a container of `image` against the study stack; commit `out_image`.

    The prepare step the Dockerfile cannot do (a build cannot join the internal network). `env`
    is the study codescout env (study_mcp_env, with the index-builder Qdrant URL). Writes
    `index-cost.json` (wall time, exit code) and `index-diff.txt` (`docker diff`: every path the
    index wrote, for the leakage scan) under `data_dir`. The committed image is what every arm
    of the task runs in, `bare` included. The container is always removed.
    """
    bad = {k: v for k, v in env.items() if _names_host(v)}
    if bad:
        raise ValueError(f"index env names the host: {bad}")
    docker = docker if docker is not None else pilot.docker
    data_dir = Path(data_dir)
    data_dir.mkdir(parents=True, exist_ok=True)
    name = f"sab-prep-{run}"
    docker("run", "-d", "--name", name, "--network", network, *RESOURCES,
           "--cap-drop", "ALL", "--security-opt", "no-new-privileges", image, "sleep", "infinity")
    try:
        exports = [a for k, v in sorted(env.items()) for a in ("-e", f"{k}={v}")]
        started = clock()
        result = docker("exec", *exports, "-w", project, name,
                        CODESCOUT_BIN, "index", "-p", project, timeout=timeout, check=False)
        wall = round(clock() - started, 3)
        (data_dir / "index.log").write_text(result.stdout)
        (data_dir / "index-cost.json").write_text(json.dumps(
            {"image": image, "out_image": out_image, "project": project,
             "wall_seconds": wall, "exit_code": result.returncode,
             "env_keys": sorted(env)}, indent=2) + "\n")
        if result.returncode:
            raise RuntimeError(f"codescout index failed (exit {result.returncode}); see "
                               f"{data_dir / 'index.log'}")
        (data_dir / "index-diff.txt").write_text(docker("diff", name).stdout)
        docker("commit", "--change", 'CMD ["bash"]', name, out_image)
    finally:
        docker("rm", "-f", name, check=False)
