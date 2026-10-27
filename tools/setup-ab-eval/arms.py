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
HOST_PATH_MARKERS = ("/home/",)

# prepare_index, run in the project dir: commit what the index wrote into the tree (C3).
INDEX_ARTIFACTS_COMMIT = (
    "set -e; for p in .codescout .gitignore; do if [ -e \"$p\" ]; then git add -f -- \"$p\"; "
    "fi; done; git diff --cached --quiet || git -c user.name=sab-prepare "
    "-c user.email=sab-prepare@example.invalid commit -q -m 'sab: codescout index artifacts'")
# prepare_index: the index's lock under /tmp/codescout-<uid>/ never enters the image (C7).
TMP_LOCK_REMOVAL = ("rm -f /tmp/codescout-*/codescout-index-*.lock; "
                    "rmdir /tmp/codescout-* 2>/dev/null; true")

# The Codex `--setting-sources local` (bare, cs); full alone also loads the user source, so the
# sanitized user settings.json and the frozen CLAUDE.md in its config dir apply (C2 ruling).
SETTING_SOURCES = "local"
FULL_SETTING_SOURCES = "user,local"

# C2 ruling: these user-settings keys are kept whatever their values hold.
USER_SETTINGS_KEPT_BY_NAME = ("outputStyle", "enabledPlugins", "permissions")
# Dropped by name, with the reason recorded in each full run's arm-record.json.
USER_SETTINGS_DROPPED = {
    "env": "the operator's process env: ANTHROPIC_BASE_URL is the host llm-proxy "
           "(localhost:8082) and the CODESCOUT_* keys travel in the MCP env (C2 ruling)",
    "statusLine": "an interactive status line command on a host path (C2 ruling)",
    # Deviation from the C2 generic rule, flagged for the controller: these values hold no host
    # path or URL, but the study runs every arm at the CLI default effort (Global Constraints:
    # no effort setting in any arm; the effective effort must be equal across the arms of a
    # task). Kept in full alone, they would change full's effort and void every task.
    "effortLevel": "an effort setting; every arm runs at the CLI default effort, and full alone "
                   "carrying one would fail the per-task effort-equality gate",
    "modelSettings": "per-model effortLevel settings; every arm runs at the CLI default effort, "
                     "and full alone carrying one would fail the per-task effort-equality gate",
    # Round-3 ruling (R2-F1, measured: controls/round2/verbose-probe/verbose-probe.json).
    "verbose": "changes output shape, thinking display and request count (cost); harness "
               "behaviour, not capability",
}


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
def _names_host_path(value):
    return any(marker in str(value) for marker in HOST_PATH_MARKERS)


def _filter_hooks(hooks):
    """Keep the hook commands that name no host path or URL; return (kept, dropped_events)."""
    kept, dropped = {}, {}
    for event, groups in hooks.items():
        out_groups, gone = [], []
        for group in groups:
            entries = [h for h in group.get("hooks", [])
                       if not (_names_host_path(json.dumps(h)) or _names_host(json.dumps(h)))]
            gone += [h.get("command", json.dumps(h)) for h in group.get("hooks", [])
                     if h not in entries]
            if entries:
                out_groups.append(dict(group, hooks=entries))
        if out_groups:
            kept[event] = out_groups
        if gone:
            dropped[f"hooks.{event}"] = ("hook command names a host path or URL, which does "
                                         f"not exist in the container: {gone}")
    return kept, dropped


def sanitize_user_settings(settings):
    """The frozen user settings.json as the full arm gets it (controller ruling C2).

    Returns (kept, dropped) where dropped maps each removed key to its reason. Kept by name:
    USER_SETTINGS_KEPT_BY_NAME. Dropped by name: USER_SETTINGS_DROPPED (with the reason
    recorded there). `hooks` keeps only the commands with no host path. Any other key is kept
    only if its value names no host path (/home/) and no host URL (127.0.0.1, localhost).
    """
    kept, dropped = {}, {}
    for key, value in settings.items():
        if key in USER_SETTINGS_KEPT_BY_NAME:
            kept[key] = value
        elif key in USER_SETTINGS_DROPPED:
            dropped[key] = USER_SETTINGS_DROPPED[key]
        elif key == "hooks":
            hooks, gone = _filter_hooks(value)
            if hooks:
                kept[key] = hooks
            dropped.update(gone)
        elif _names_host_path(json.dumps(value)):
            dropped[key] = "value names a host path (/home/), which does not exist in the container"
        elif _names_host(json.dumps(value)):
            dropped[key] = "value names a host URL (127.0.0.1/localhost); never into a container"
        else:
            kept[key] = value
    return kept, dropped


def _bare_args(model, session_id, task_prompt, extra, setting_sources=None):
    # pilot.py:233-238 minus `--effort <effort>`; `extra` goes in front of the prompt separator.
    # Only full changes the setting sources (C2 ruling); bare keeps the Codex `local`.
    return [CLI, "-p", "--model", model, "--session-id", session_id,
            "--setting-sources", setting_sources or SETTING_SOURCES, "--settings", SETTINGS,
            "--strict-mcp-config", "--dangerously-skip-permissions",
            "--tools", NATIVE_TOOLS,
            "--output-format", "json", "--max-budget-usd", MAX_BUDGET_USD,
            *extra, "--", task_prompt]


def arm_invocation(arm, model, task_prompt, manifest, urls, run_dir, *, session_id=None):
    """Write the run's config files under `run_dir` and return the arm's CLI args and mounts.

    `session_id` is keyword-only (an addition to the brief's signature): the Codex list carries
    `--session-id`, and the runner must know it to find the transcript. A fresh UUID otherwise.

    Also writes `run_dir/arm-record.json`: the arm, its setting sources, the files put in its
    config dir and every user-settings key dropped with its reason (C2 ruling).
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
    dropped = {}
    if arm in ("cs", "full"):
        sab = run_dir / "sab"
        sab.mkdir()
        servers = {"codescout": {
            "type": "stdio", "command": CODESCOUT_BIN, "args": list(CODESCOUT_ARGS),
            "env": mcp_env}}
        (sab / "mcp.json").write_text(json.dumps({"mcpServers": servers}, indent=2) + "\n")
        # C1 ruling: the companion hooks (detect.mjs) find codescout only in the config dir's
        # .claude.json, never through --mcp-config. --strict-mcp-config keeps this entry from
        # starting a second server; the mcp.json one is the server Claude Code runs.
        (config / ".claude.json").write_text(json.dumps({"mcpServers": servers}, indent=2)
                                             + "\n")
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
            if Path(src).name == "settings.json":
                kept, dropped = sanitize_user_settings(json.loads(Path(src).read_text()))
                (config / "settings.json").write_text(json.dumps(kept, indent=2) + "\n")
            else:
                shutil.copyfile(src, config / Path(src).name)
    sources = FULL_SETTING_SOURCES if arm == "full" else SETTING_SOURCES
    (run_dir / "arm-record.json").write_text(json.dumps({
        "arm": arm, "setting_sources": sources,
        "config_files": sorted(p.name for p in config.iterdir()),
        "settings_dropped": dropped}, indent=2) + "\n")
    return ArmSpec(cli_args=_bare_args(model, session_id, task_prompt, extra, sources),
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
        # C3 ruling: the index's .codescout/ and .gitignore go into the base commit, so an
        # agent's extracted patch (git add -A; git diff --cached <base>) excludes them.
        docker("exec", "-w", project, name, "sh", "-c", INDEX_ARTIFACTS_COMMIT)
        # C7 ruling: the index's /tmp lock never enters the image.
        docker("exec", name, "sh", "-c", TMP_LOCK_REMOVAL)
        (data_dir / "index-diff.txt").write_text(docker("diff", name).stdout)
        docker("commit", "--change", 'CMD ["bash"]', name, out_image)
    finally:
        docker("rm", "-f", name, check=False)
