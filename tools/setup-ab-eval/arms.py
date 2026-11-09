"""Arm invocations: the three arms differ only in Claude Code CLI flags and mounts.

  bare  the Codex `pilot.run_pilot` argument list (pilot.py:233-238) minus `--effort`, with
        `--model <model>`: native tools, `--strict-mcp-config` with no server, an empty
        CLAUDE_CONFIG_DIR, `--setting-sources local` and an empty `--settings` file.
  cs    bare + `--mcp-config /opt/sab/mcp.json` (codescout, its env rewritten to the study
        stack) + `--plugin-dir /opt/plugins/codescout-companion`.
  full  cs + one `--plugin-dir` per `plugins_full` entry (the companion is not repeated), with
        the frozen CLAUDE.md and settings.json copied into the run's config dir, and the
        `Skill` and `Agent` tools added to `--tools` (final review I-8, operator decision
        2026-10-29: the setup of daily use invokes skills and starts subagents; bare and cs keep
        the Codex list).

FROZEN INPUTS (final review I-7). When the manifest carries `frozen_root` (the runner's
frozen.frozen_manifest()), every plugin dir and config file arm_invocation reads must lie under
it, or it raises: the runner reads the study's frozen copy, never the live profile.

What is the same in every arm, by construction: the image (the `bare` arm runs in the same
agent-cs image, so the bytes the agent can read are the same), the CLI binary, the model, the
prompt bytes, the container env (`ArmSpec.env`), the resource flags (`AGENT_RESOURCES`) and the
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
import re
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
# Final review I-8: the full arm also gets the Skill and Agent tools.
FULL_ONLY_TOOLS = ("Skill", "Agent")
FULL_TOOLS = ",".join((NATIVE_TOOLS, *FULL_ONLY_TOOLS))
MAX_BUDGET_USD = "1.25"
# Copied from pilot.run_pilot's `docker run` (--cpus 4 --memory 8g): the prepare step and the
# graders (tasks.private_grader / foreign_grader) keep it, as the Step-5 controls ran with it.
RESOURCES = ("--cpus", "4", "--memory", "8g")
# Fix round 2, operator decision 2026-10-30: every ARM container (docker_run_args) gets the same
# memory in every arm (rust-analyzer on the codescout repo peaked at 7.3-8.5 GB of 8 GB in an
# idle session). Round 3 ruling: --memory-swap equal to --memory, so the container has NO swap
# and a memory-heavy run fails fast as an OOM kill instead of swapping toward the timeout.
# Task 12 (operator 2026-10-30): 16g -> 24g after pilot attempt 1 measured memory.peak up to
# 14.8 GB (opus full, cs-p08) against 16 GB.
AGENT_MEMORY = "24g"
AGENT_RESOURCES = ("--cpus", "4", "--memory", AGENT_MEMORY, "--memory-swap", AGENT_MEMORY)
# The non-secret part of pilot's private.env; the token and proxy are the runner's (Task 8).
CONTAINER_ENV = {"CLAUDE_CONFIG_DIR": CONFIG_DIR, "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "8192",
                 "DISABLE_AUTOUPDATER": "1"}

DROPPED_ENV_KEYS = ("CODESCOUT_RERANKER_URL",)
# Minor 2 (round 4): an ALLOWLIST. Every URL host (scheme://host, or a scheme-less host:port)
# in a codescout env value or in full's settings must start with `sab-`, the study's own
# containers; anything else raises. A denylist of host names missed LAN IPs, the docker bridge
# gateway (172.17.0.1), [::1] and bare service names.
STUDY_HOST_PREFIX = "sab-"
# Task 7 (carried Task 4 minor 3): optional userinfo is skipped, so in `http://sab-x@10.0.0.1/`
# the host is 10.0.0.1, not the `sab-x` user name. A backslash ends the authority as `/` does
# (Task 7 review minor 1: WHATWG URL parsers treat it so for http), so `http://10.0.0.1\@sab-x/`
# names 10.0.0.1.
_SCHEME_URL = re.compile(r"[A-Za-z][A-Za-z0-9+.-]*://(?:[^/\\\s@\"'?#]*@)?(\[[^\]]*\]|[^/\\\s:\"'?#@]+)")
_HOST_PORT = re.compile(r"(?:^|[\s=,(`\"'])(\[[^\]]*\]|[A-Za-z0-9][A-Za-z0-9.-]*):\d{2,5}\b")
HOST_PATH_MARKERS = ("/home/",)
# Settings keys whose values are permission PATTERNS, not endpoints: kept by name (C2) and not
# held to the URL allowlist. The frozen permissions carry `Bash(git clone https://github.com/...)`
# and `WebFetch(domain:127.0.0.1)`, which grant nothing the internal network can reach.
URL_ALLOWLIST_EXEMPT_SETTINGS = ("permissions",)

# prepare_index, run in the project dir BEFORE the index: commit the project config that
# onboarding() wrote into the base (C3), so the index then records this commit as
# last_indexed_commit == HEAD. An ALLOWLIST (Task 7, carried Task 4 minor 2): only these files
# are added, so a rotated log or any other runtime file can never enter the base; a leftover
# one stays untracked and INDEX_STATE_EXCLUDE then refuses the dirty tree.
CONFIG_COMMIT_ALLOWLIST = (".codescout/project.toml", ".codescout/workspace.toml",
                           ".codescout/write.lock", ".gitignore")
INDEX_ARTIFACTS_COMMIT = (
    "set -e; for p in " + " ".join(CONFIG_COMMIT_ALLOWLIST) + "; do if [ -e \"$p\" ]; then "
    "git add -f -- \"$p\"; "
    "fi; done; git diff --cached --quiet || git -c user.name=sab-prepare "
    "-c user.email=sab-prepare@example.invalid commit -q -m 'sab: codescout project config'")
# prepare_index, run in the project dir AFTER the index (round-4 live finding). A tracked
# index-state.json cannot name the commit that contains it, so committing it left the index one
# commit behind HEAD, and the companion's SessionStart hook then spawned a background
# `codescout index` in every run (session-start.mjs:259-283). The index state stays in the
# image, untracked and excluded through .git/info/exclude (an agent's `git add -A` patch skips
# it). The script refuses an index not at HEAD and a tree that is not clean.
INDEX_STATE_EXCLUDE = (
    "set -e; "
    "[ \"$(git rev-parse --show-toplevel)\" = \"$(pwd -P)\" ] "
    "|| { echo 'project dir is not the repo root'; exit 2; }; "
    "ex=\"$(git rev-parse --git-path info/exclude)\"; mkdir -p \"$(dirname \"$ex\")\"; "
    "for p in /.codescout/index-state.json /.codescout/write.lock; do "
    "grep -qxF \"$p\" \"$ex\" 2>/dev/null || echo \"$p\" >> \"$ex\"; done; "
    "head=\"$(git rev-parse HEAD)\"; "
    "grep -q \"\\\"last_indexed_commit\\\": *\\\"$head\\\"\" .codescout/index-state.json "
    "|| { echo \"index-state.json is not at HEAD $head\"; exit 4; }; "
    "s=\"$(git status --porcelain)\"; "
    "[ -z \"$s\" ] || { echo 'tree not clean after prepare:'; echo \"$s\"; exit 3; }")
# prepare_index: the index's lock under /tmp/codescout-<uid>/ never enters the image (C7), nor
# do the prepare step's own files or the codescout home state its MCP session created. Runs
# only through `docker exec` in the prepare container (it names $HOME).
# The onboarding MCP session's XDG state under $HOME (Task 7, carried Task 4 minor 1; measured
# in controls/round4c/isolation/prepare/index-diff.txt): heartbeats, the librarian workspace and
# catalog, the server rendezvous slot and guide state. The study carries no catalog history
# (spec section 3, gate 3), and a run's codescout recreates what it needs. Empty parents go too.
HOME_STATE_PATHS = (".codescout", ".cache/codescout", ".config/librarian",
                    ".local/state/codescout", ".local/share/librarian")
HOME_STATE_REMOVAL = (
    "for p in " + " ".join(HOME_STATE_PATHS) + "; do rm -rf \"$HOME/$p\"; done; "
    "for d in .cache .config .local/state .local/share .local; do "
    "rmdir \"$HOME/$d\" 2>/dev/null; done; true")
TMP_LOCK_REMOVAL = ("rm -f /tmp/codescout-*/codescout-index-*.lock; "
                    "rmdir /tmp/codescout-* 2>/dev/null; rm -rf /tmp/sab-prepare; "
                    + HOME_STATE_REMOVAL)

# The Codex `--setting-sources local` (bare, cs); full alone also loads the user source, so the
# sanitized user settings.json and the frozen CLAUDE.md in its config dir apply (C2 ruling).
SETTING_SOURCES = "local"
FULL_SETTING_SOURCES = "user,local"

# Arm profiles. "v1" is what every registered study and screen ran with and stays the default, so
# their runs stay reproducible. "v2" (2026-11-09) answers two bugs found by the usage audit:
# ba34b0fc327743ca (cs listed Bash/Edit/Write, which the daily setup denies through the user
# settings' `permissions.deny`, so the companion hook blocked them call by call) and
# b8c35dc28bd3e8dd (`--setting-sources local` keeps the task's CLAUDE.md out of every arm).
# v2: cs lists only Read, Glob and Grep; every arm also loads the `project` source. bare keeps its
# native tools and full keeps FULL_TOOLS (its deny list comes from the user settings).
PROFILES = ("v1", "v2")
CS_TOOLS_V2 = "Read,Glob,Grep"
SETTING_SOURCES_V2 = "project,local"
FULL_SETTING_SOURCES_V2 = "user,project,local"

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


PREPARE_TMP = "/tmp/sab-prepare"

# Round-4 ruling I1. The codescout tools daily use advertises (21): the registry
# (src/server.rs CodeScoutServer::from_parts) as filtered by `current_capabilities`
# (src/server.rs:1131). Three are gated on `has_lsp` (Availability::RequiresLsp:
# src/tools/symbol/{references.rs:613, symbol_at.rs:482, call_graph/mod.rs:766}); has_lsp comes
# from `[project] languages` in .codescout/project.toml, which `codescout index` never writes, so
# without it the arm saw 18. Cross-checked against a live daily session's tool list (21 names).
CODESCOUT_TOOLS = ("approve_write", "call_graph", "create_file", "doc", "edit_code", "edit_file",
                   "get_guide", "grep", "index", "librarian", "library", "memory", "onboarding",
                   "read_file", "references", "run_command", "semantic_search", "symbol_at",
                   "symbols", "tree", "workspace")
LSP_GATED_TOOLS = ("call_graph", "references", "symbol_at")

# The daily effective embedding model (round-4 ruling I1.3). Model precedence
# (src/retrieval/config.rs:111-114, merge_embed_config): env CODESCOUT_EMBEDDING_MODEL >
# CODESCOUT_EMBEDDER_MODEL > CODESCOUT_EMBED_MODEL > [embeddings].model > `local:AllMiniLML6V2Q`.
# Measured 2026-10-27: ~/.claude's codescout env sets none of the three, so the daily project's
# /home/marius/work/claude/codescout/.codescout/project.toml `[embeddings] model =
# "CodeRankEmbed"` decides; kat and sdd set CODESCOUT_EMBED_MODEL=CodeRankEmbed. The URL is
# env-first in both (CODESCOUT_EMBEDDER_URL), so the project.toml url only mirrors daily's shape.
DAILY_EMBED_MODEL = "CodeRankEmbed"
DAILY_EMBEDDINGS_EFFECTIVE = {
    "model": "CodeRankEmbed (project.toml [embeddings].model; kat/sdd env CODESCOUT_EMBED_MODEL)",
    "url": "http://127.0.0.1:48081 (env CODESCOUT_EMBEDDER_URL, over project.toml "
           "http://127.0.0.1:48081/v1)",
    "dim": "768 (env CODESCOUT_MODEL_DIM)",
    "wire_model_name": "CodeRankEmbed-Q4_K_M.gguf (env CODESCOUT_EMBEDDER_MODEL_NAME)",
}

# Copied into the prepare container and run there; also exec'd by the tests on the TOML
# codescout's onboarding() wrote. Replaces exactly the [embeddings] table, keeps every other
# byte, and re-parses the result (tomllib, Python 3.11+, as in the agent image) before writing.
SET_EMBEDDINGS_PY = r'''import json
import re
import sys


def rewrite(text, model, url):
    table = "[embeddings]\nmodel = %s\nurl = %s\n" % (json.dumps(model), json.dumps(url))
    new, n = re.subn(r"(?ms)^\[embeddings\]\n.*?(?=^\[|\Z)", table + "\n", text)
    if n > 1:
        raise SystemExit("more than one [embeddings] table")
    if n == 0:
        new = text.rstrip("\n") + "\n\n" + table
    try:
        import tomllib
    except ImportError:
        return new
    doc = tomllib.loads(new)
    if doc.get("embeddings") != {"model": model, "url": url}:
        raise SystemExit("rewrite check failed: %r" % (doc.get("embeddings"),))
    return new


if __name__ == "__main__":
    path, model, url = sys.argv[1:4]
    with open(path) as f:
        text = f.read()
    with open(path, "w") as f:
        f.write(rewrite(text, model, url))
'''

# Run in the project dir after onboarding() and after the index: keep the project config
# (project.toml, workspace.toml) and the index state; remove the memories onboarding() writes
# (the study carries no project memories, spec section 3 gate 3 / section 9.1), its per-call
# subagent hand-off prompt in .codescout/tmp/ (32 KB of codescout instructions that every arm,
# bare included, would otherwise find in the tree; round-4 live finding) and the MCP session's
# runtime files (usage db, logs, lock holder).
PREPARE_CLEANUP = (
    "set -e; cd .codescout 2>/dev/null || exit 0; "
    "find . -type d \\( -name memories -o -name private-memories \\) -prune -exec rm -rf {} +; "
    "rm -rf tmp; rm -f usage.db usage.db-shm usage.db-wal write.lock.holder; "
    "find . -maxdepth 1 -name '*.log*' -delete; "
    "find . -mindepth 1 -type d -empty -delete; true")

# Minor 3: what each KEPT user-settings key does in full, where it is known.
KEPT_SETTING_EFFECTS = {
    "outputStyle": "applied: full's init shows `legible:Explanatory STE` and the style text "
                   "('Writing Style: about 80% of ASD-STE100') is in the request (round-2 capture)",
    "enabledPlugins": "read only by the marketplace lookup (`Looking up plugin ...`, not found, "
                      "skipped); the plugins load through --plugin-dir (round-1 debug log)",
    "permissions": "deny [Bash, Write, Edit] removes those native tools from full's request "
                   "(round-2 capture: Glob, Grep, Read remain); fix round 2 (N-1): only `deny` "
                   "and `defaultMode` are frozen, `allow`/`ask` are not copied (inert under "
                   "--dangerously-skip-permissions; init re-measured: controls/final-fix/"
                   "n1-full-init)",
    "autoMemoryEnabled": "false: CLI 2.1.309 schema 'When false, Claude will not read from or "
                         "write to the auto-memory directory'; removes the '# Memory' section "
                         "from full's system text (5591 -> 3573 chars against cs; round-4 A/B "
                         "attributes it to this key)",
    "fileCheckpointingEnabled": "false: CLI 2.1.309 schema 'Snapshot files before edits so "
                                "/rewind can restore them' (default true); no effect on the "
                                "request was seen (not isolated by A/B)",
    "skillOverrides": "{vet-ad-offer: user-invocable-only} names a user skill of "
                      "~/.claude/skills, which full does not carry (absent from full's 77 "
                      "skills, round-2 init): no effect",
    "model": "'sonnet': overridden by --model in every arm",
}
KEPT_SETTING_DEFAULT_EFFECT = ("no request-visible effect expected (interactive UI or "
                               "housekeeping setting); not measured")

class ArmSpec(NamedTuple):
    cli_args: list
    mounts: list  # (host_path, container_path, "ro" | "rw")
    env: dict




def foreign_hosts(value):
    """Every URL host in `value` (a string, or JSON-dumped) that does not start with `sab-`."""
    text = value if isinstance(value, str) else json.dumps(value)
    hosts = [m.group(1) for m in _SCHEME_URL.finditer(text)]
    # Scheme-less host:port, outside any scheme URL already seen.
    rest = _SCHEME_URL.sub(" ", text)
    hosts += [m.group(1) for m in _HOST_PORT.finditer(rest)]
    return [h for h in hosts if not h.startswith(STUDY_HOST_PREFIX)]
def expected_mcp_tools(arm):
    """The MCP tools an arm's init event must list: none for bare, the daily 21 for cs/full."""
    if arm not in ARMS:
        raise ValueError(f"unknown arm {arm!r}; expected one of {ARMS}")
    return [] if arm == "bare" else [f"mcp__codescout__{t}" for t in CODESCOUT_TOOLS]

def study_mcp_env(mcp_env, urls):
    """The manifest's codescout env with its URL keys pointed at the study stack."""
    bad = {k: v for k, v in urls.items() if foreign_hosts(v)}
    if bad:
        raise ValueError(f"study URLs must name sab- hosts only: {bad}")
    env = {}
    for key, value in mcp_env.items():
        if key in DROPPED_ENV_KEYS:
            continue
        env[key] = urls.get(key, value)
    left = {k: v for k, v in env.items() if foreign_hosts(v)}
    if left:
        raise ValueError(f"codescout env keys name a non-study host and the study stack has no "
                         f"URL for them: {left}; only sab- hosts enter a container")
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
                       if not (_names_host_path(json.dumps(h)) or foreign_hosts(h))]
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
    recorded there). `hooks` keeps only the commands with no host path or non-study URL. Any
    other key is kept only if its value names no host path (/home/) and no URL host outside the
    study (the `sab-` allowlist, Minor 2). A key kept by name that carries a non-study URL
    raises, except the permission patterns in URL_ALLOWLIST_EXEMPT_SETTINGS.
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
        elif foreign_hosts(value):
            dropped[key] = (f"value names a non-study URL host {foreign_hosts(value)}; only sab- "
                            f"hosts enter a container")
        else:
            kept[key] = value
    bad = {k: foreign_hosts(v) for k, v in kept.items()
           if k not in URL_ALLOWLIST_EXEMPT_SETTINGS and foreign_hosts(v)}
    if bad:
        raise ValueError(f"settings kept by name carry non-study URL hosts: {bad}")
    return kept, dropped


def _bare_args(model, session_id, task_prompt, extra, setting_sources=None, tools=NATIVE_TOOLS):
    # pilot.py:233-238 minus `--effort <effort>`; `extra` goes in front of the prompt separator.
    # Only full changes the setting sources (C2 ruling) and the tool list (I-8); bare keeps the
    # Codex `local` and native tools.
    return [CLI, "-p", "--model", model, "--session-id", session_id,
            "--setting-sources", setting_sources or SETTING_SOURCES, "--settings", SETTINGS,
            "--strict-mcp-config", "--dangerously-skip-permissions",
            "--tools", tools,
            "--output-format", "json", "--max-budget-usd", MAX_BUDGET_USD,
            *extra, "--", task_prompt]


def _check_frozen_paths(arm, manifest):
    """I-7: with `frozen_root` set, every plugin dir and config file the arm reads lies under it."""
    root = manifest.get("frozen_root")
    if root is None:
        return
    root = Path(root).resolve()
    paths = [manifest["plugins_cs"]["install_path"]]
    if arm == "full":
        paths += [p["install_path"] for p in manifest["plugins_full"]]
        paths += list(manifest["full_config_files"])
    outside = [p for p in paths if not Path(p).resolve().is_relative_to(root)]
    if outside:
        raise ValueError(f"{arm} arm would read {outside} outside the frozen inputs {root}")

def arm_invocation(arm, model, task_prompt, manifest, urls, run_dir, *, session_id=None, profile="v1"):
    """Write the run's config files under `run_dir` and return the arm's CLI args and mounts.

    `session_id` is keyword-only (an addition to the brief's signature): the Codex list carries
    `--session-id`, and the runner must know it to find the transcript. A fresh UUID otherwise.

    `profile` (keyword-only, one of `PROFILES`, default "v1" = the arms every registered study ran):
    "v2" gives cs only Read, Glob and Grep and makes every arm load the `project` setting source.

    Also writes `run_dir/arm-record.json`: the arm, its profile, its setting sources, the files put
    in its config dir and every user-settings key dropped with its reason (C2 ruling).
    """
    if arm not in ARMS:
        raise ValueError(f"unknown arm {arm!r}; expected one of {ARMS}")
    if profile not in PROFILES:
        raise ValueError(f"unknown profile {profile!r}; expected one of {PROFILES}")
    session_id = session_id or str(uuid.uuid4())
    # Validate before writing anything, so a refused env leaves no half-made run dir.
    mcp_env = study_mcp_env(manifest["mcp_env"], urls) if arm != "bare" else None
    if arm != "bare":
        _check_frozen_paths(arm, manifest)
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
    if profile == "v2":
        sources = FULL_SETTING_SOURCES_V2 if arm == "full" else SETTING_SOURCES_V2
        tools = FULL_TOOLS if arm == "full" else CS_TOOLS_V2 if arm == "cs" else NATIVE_TOOLS
    else:
        sources = FULL_SETTING_SOURCES if arm == "full" else SETTING_SOURCES
        tools = FULL_TOOLS if arm == "full" else NATIVE_TOOLS
    kept_effects = {}
    if arm == "full":
        kept_settings = json.loads((config / "settings.json").read_text())
        kept_effects = {k: KEPT_SETTING_EFFECTS.get(k, KEPT_SETTING_DEFAULT_EFFECT)
                        for k in kept_settings}
    (run_dir / "arm-record.json").write_text(json.dumps({
        "arm": arm, "profile": profile, "setting_sources": sources, "tools": tools,
        "frozen_root": manifest.get("frozen_root"),
        "config_files": sorted(p.name for p in config.iterdir()),
        "expected_mcp_tools": expected_mcp_tools(arm),
        "settings_kept": kept_effects,
        "settings_dropped": dropped}, indent=2) + "\n")
    return ArmSpec(cli_args=_bare_args(model, session_id, task_prompt, extra, sources, tools),
                   mounts=mounts, env=dict(CONTAINER_ENV))


def docker_run_args(name, image, network, spec, env_file):
    """`docker run` argv for an agent container; only the `-v` mounts vary with the arm.

    The env goes through `env_file` (the runner adds its secret lines to the same file), and the
    container idles so the CLI can be `docker exec`-ed, as in pilot.run_pilot.
    """
    argv = ["run", "-d", "--name", name, "--network", network, *AGENT_RESOURCES,
            "--env-file", str(env_file),
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges"]
    for host, target, mode in spec.mounts:
        argv += ["-v", f"{host}:{target}:{mode}"]
    return argv + [image, "sleep", "infinity"]


def env_file_lines(spec):
    """The non-secret lines of the run's env file (identical across arms)."""
    return "".join(f"{k}={v}\n" for k, v in sorted(spec.env.items()))


# Controller Ruling A (Task 7 review): the frozen codescout binary post-dates every private fix
# and carried verbatim gold-patch strings for 5 private tasks, readable by every arm (bare too).
# The image every arm runs in therefore gets one derived layer over the prepared run image: the
# binary owned by root and mode 0711, so the agent user can execute it (the kernel maps an ELF it
# cannot read; such a process is also not dumpable) but cannot read, copy or open its bytes.
# /opt/sab/bin stays root-owned 0755: the agent cannot replace the file, and the other tools
# there (node, rust-analyzer, which hold no task content) stay listable, so the leak scan's
# agent-user view still reads them. A layer, not an agent-cs.Dockerfile change: the prepared run
# images and their Qdrant templates (hours of indexing) stay valid.
EXEC_ONLY_FILES = (CODESCOUT_BIN,)
EXEC_ONLY_MODE = "0711"
EXEC_ONLY_DOCKERFILE = (
    "ARG BASE\nFROM ${BASE}\nARG AGENT_USER=1000:1000\nUSER root\n"
    f"RUN chown root:root {' '.join(EXEC_ONLY_FILES)} "
    f"&& chmod {EXEC_ONLY_MODE} {' '.join(EXEC_ONLY_FILES)} "
    f"&& chown root:root {SAB_BIN} && chmod 0755 {SAB_BIN}\n"
    "USER ${AGENT_USER}\n")
EXEC_ONLY_LABEL = "sab.exec_only_base"


def exec_only_image(base, out_image, context, *, command=None):
    """Build `out_image` = `base` + EXEC_ONLY_DOCKERFILE, offline, labelled with base's image id."""
    command = command or pilot.command
    base_id = pilot.docker("image", "inspect", base, "--format", "{{.Id}}").stdout.strip()
    context = Path(context)
    context.mkdir(parents=True, exist_ok=True)
    (context / "Dockerfile").write_text(EXEC_ONLY_DOCKERFILE)
    r = command(["docker", "build", "--network", "none", "--build-arg", f"BASE={base}",
                 "--label", f"{EXEC_ONLY_LABEL}={base_id}", "-t", out_image,
                 "-f", str(context / "Dockerfile"), str(context)], timeout=1800, check=False)
    (context / "build.log").write_text(r.stdout)
    if r.returncode:
        raise RuntimeError(f"exec-only layer {out_image} failed (exit {r.returncode}); see "
                           f"{context / 'build.log'}")
    return out_image


def _toml_table(text, table):
    """One table of a TOML document as a dict, or None (tomllib, Python 3.11+)."""
    try:
        import tomllib
        return tomllib.loads(text).get(table)
    except (ImportError, ValueError):
        return None

def prepare_index(image, out_image, network, env, data_dir, *, project="/testbed",
                  run="index", embeddings=None, docker=None, clock=time.monotonic, timeout=7200):
    """Onboard and index `project` in a container of `image` against the study stack; commit it.

    The prepare step the Dockerfile cannot do (a build cannot join the internal network). `env`
    is the study codescout env (study_mcp_env, with the index-builder Qdrant URL). In order:

      1. codescout's own `onboarding()` over MCP (mcp_probe.py): it writes .codescout/project.toml
         with the detected languages, which turns on has_lsp and with it `references`,
         `symbol_at` and `call_graph` (round-4 ruling I1; no other codescout path writes it).
      2. `[embeddings]` set to the daily effective model (`embeddings`, default
         DAILY_EMBED_MODEL and the study dense URL): onboarding() writes a hardware
         recommendation instead, `local:AllMiniLML6V2Q` in a GPU-less container.
      3. PREPARE_CLEANUP: onboarding()'s memories and the session's runtime files go.
      4. `codescout index -p <project>`, timed (index-cost.json), then PREPARE_CLEANUP again.
      5. Only the config (CONFIG_COMMIT_ALLOWLIST) was committed into the repo's base, before
         the index (C3); the index state (index-state.json) stays untracked, excluded through
         .git/info/exclude, and must name HEAD. /tmp locks, the prepare files and the
         onboarding session's home state (HOME_STATE_PATHS) are removed (C7).

    Writes prepare.json (onboarding outcome, the embeddings comparison), onboarding.json,
    project.onboarded.toml, project.toml, index.log, index-cost.json and index-diff.txt under
    `data_dir`. The committed image is what every arm of the task runs in, `bare` included.
    The container is always removed.
    """
    bad = {k: v for k, v in env.items() if foreign_hosts(v)}
    if bad:
        raise ValueError(f"index env must name sab- hosts only: {bad}")
    if embeddings is None:
        base = env["CODESCOUT_EMBEDDER_URL"].rstrip("/")
        embeddings = {"model": DAILY_EMBED_MODEL,
                      "url": base if base.endswith("/v1") else base + "/v1"}
    if foreign_hosts(embeddings["url"]):
        raise ValueError(f"embeddings url must name a sab- host: {embeddings['url']}")
    docker = docker if docker is not None else pilot.docker
    data_dir = Path(data_dir)
    data_dir.mkdir(parents=True, exist_ok=True)
    config = f"{project}/.codescout/project.toml"
    mcp = {"mcpServers": {"codescout": {"type": "stdio", "command": CODESCOUT_BIN,
                                        "args": list(CODESCOUT_ARGS), "env": env}}}
    (data_dir / "prepare-mcp.json").write_text(json.dumps(mcp, indent=2) + "\n")
    (data_dir / "prepare-calls.json").write_text(json.dumps([["onboarding", {}]]) + "\n")
    (data_dir / "set_embeddings.py").write_text(SET_EMBEDDINGS_PY)
    record = {"image": image, "out_image": out_image, "project": project,
              "embeddings": {"daily_effective": DAILY_EMBEDDINGS_EFFECTIVE, "set": embeddings}}
    name = f"sab-prep-{run}"
    docker("run", "-d", "--name", name, "--network", network, *RESOURCES,
           "--cap-drop", "ALL", "--security-opt", "no-new-privileges", image, "sleep", "infinity")
    try:
        docker("exec", name, "mkdir", "-p", PREPARE_TMP)
        for src in (Path(__file__).resolve().parent / "mcp_probe.py",
                    data_dir / "prepare-mcp.json", data_dir / "prepare-calls.json",
                    data_dir / "set_embeddings.py"):
            docker("cp", str(src), f"{name}:{PREPARE_TMP}/{src.name}")
        # 1. onboarding() through codescout's own MCP surface.
        probe = docker("exec", "-w", project, "-e", "MCP_PROBE_TIMEOUT=900",
                       "-e", f"MCP_PROBE_STDERR={PREPARE_TMP}/onboarding-server.log", name,
                       "python3", f"{PREPARE_TMP}/mcp_probe.py", f"{PREPARE_TMP}/prepare-mcp.json",
                       f"{PREPARE_TMP}/prepare-calls.json", check=False, timeout=1200)
        (data_dir / "onboarding.json").write_text(probe.stdout)
        try:
            call = json.loads(probe.stdout)["calls"][0]
        except (ValueError, KeyError, IndexError):
            call = {"probe_error": "no parseable probe output"}
        result = call.get("result") or {}
        ok = (probe.returncode == 0 and not call.get("error") and not call.get("probe_error")
              and bool(result) and not result.get("isError"))
        text = "\n".join(c.get("text", "") for c in result.get("content", []))
        record["onboarding"] = {"ok": ok, "result_head": text[:2000],
                                "error": call.get("error") or call.get("probe_error")}
        if not ok:
            raise RuntimeError(f"codescout onboarding() failed in {name}; see "
                               f"{data_dir / 'onboarding.json'}")
        # 2. [embeddings] to the daily effective model.
        onboarded = docker("exec", name, "cat", config).stdout
        (data_dir / "project.onboarded.toml").write_text(onboarded)
        record["embeddings"]["onboarding_wrote"] = _toml_table(onboarded, "embeddings")
        docker("exec", name, "python3", f"{PREPARE_TMP}/set_embeddings.py", config,
               embeddings["model"], embeddings["url"])
        (data_dir / "project.toml").write_text(docker("exec", name, "cat", config).stdout)
        # 3. No project memories, no session state; then C3: the project config into the base
        # commit, so the index (step 4) records that commit and an agent's extracted patch
        # (git add -A; git diff --cached <base>) excludes the config.
        docker("exec", "-w", project, name, "sh", "-c", PREPARE_CLEANUP)
        docker("exec", "-w", project, name, "sh", "-c", INDEX_ARTIFACTS_COMMIT)
        # 4. The index, timed.
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
        docker("exec", "-w", project, name, "sh", "-c", PREPARE_CLEANUP)
        # 5. The index state stays untracked and excluded, at HEAD, in a clean tree (refused
        # otherwise). C7: no /tmp lock in the image.
        docker("exec", "-w", project, name, "sh", "-c", INDEX_STATE_EXCLUDE)
        docker("exec", name, "sh", "-c", TMP_LOCK_REMOVAL)
        (data_dir / "index-diff.txt").write_text(docker("diff", name).stdout)
        docker("commit", "--change", 'CMD ["bash"]', name, out_image)
    finally:
        (data_dir / "prepare.json").write_text(json.dumps(record, indent=2) + "\n")
        docker("rm", "-f", name, check=False)
