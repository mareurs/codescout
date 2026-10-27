"""Task 4 live control: no model call, no token, no proxy. Raw evidence -> setup-ab-data/controls.

    python3 live_control.py <manifest.json> <step> [<step> ...]

Steps (each writes one JSON under CONTROLS; run them in this order):
  image      versions and `ldd` inside the agent image (network none)
  hooks      every codescout-companion hook command, run with a sample payload, in a cs container
  stack      internal network + study stack (stack.stack_up); VRAM before/after; sparse offline
  isolation  Task 3 Step 5: index a 3-file fixture with zq_marker_fn (arms.prepare_index),
             template it, run A writes memories, run B from the same template must find
             zq_marker_fn and must not see A's memories
  ra         rust-analyzer on the task repo under the arm memory cap: peak memory, OOM kills
  r10        `claude -p --output-format stream-json` per arm, no token, internal network: the
             init event (tools, MCP status, plugins) and hook events before the auth failure
  down       remove every object this control created (exact names), leave no sab- container

Every container is `sab-ctl-*`; teardown goes through stack.py's exact-name helpers.
"""
import json
import subprocess
import sys
import time
import uuid
from pathlib import Path

import arms
import pilot
import stack

DATA = Path("/home/marius/work/claude/setup-ab-data")
CONTROLS = DATA / "controls"
HERE = Path(__file__).resolve().parent
NET = "sab-ctl-net"
IMAGE = "sab-agent-cs-ruff-15309:f9b0fc9f8"
FIXTURE_IMAGE = "sab-ctl-fixture:zq"
INDEXED_IMAGE = "sab-ctl-indexed:zq"
TEMPLATE = "sab-tpl-ctl"
FIXTURE_DIR = "/home/agent/zq"
CANARY = "zq-isolation-canary-7f3a"
MODEL = "claude-sonnet-5-5"
PROMPT = "List the files in the repository root.\n"
RUNS = ("ctl-a", "ctl-b", "ctl-ra", "ctl-r10cs", "ctl-r10full")


def docker(*args, **kwargs):
    return pilot.docker(*args, **kwargs)


def save(name, value):
    path = CONTROLS / name
    path.write_text(json.dumps(value, indent=2) + "\n")
    print(f"wrote {path}", flush=True)
    return value


def vram():
    out = subprocess.run(["rocm-smi", "--showmeminfo", "vram", "--json"], capture_output=True,
                         text=True, check=True).stdout
    card = json.loads(out)["card0"]
    return {"total": int(card["VRAM Total Memory (B)"]),
            "used": int(card["VRAM Total Used Memory (B)"])}


def study_urls(manifest, run_url):
    urls = {"CODESCOUT_QDRANT_URL": run_url, "CODESCOUT_EMBEDDER_URL": "http://sab-dense:8080",
            "CODESCOUT_EMBED_URL": "http://sab-dense:8080/v1",
            "LIBRARIAN_EMBED_URL": "http://sab-dense:8080/v1",
            "CODESCOUT_SPARSE_EMBEDDER_URL": "http://sab-sparse:80"}
    return urls


def start_arm_container(name, image, spec, run_dir):
    env_file = run_dir / "control.env"
    env_file.write_text(arms.env_file_lines(spec))  # no token, no proxy: nothing else
    docker(*arms.docker_run_args(name, image, NET, spec, env_file))


# --- image ---------------------------------------------------------------------------------

def step_image(manifest):
    script = ("set -x; id; ldd --version | head -1; command -v codescout rust-analyzer node "
              "python3; codescout --version; codescout version; rust-analyzer --version; "
              "node --version; python3 --version; ldd /opt/sab/bin/codescout; "
              "ldd /opt/sab/bin/node; cat /opt/sab/codescout-version.json "
              "/opt/sab/rust-analyzer-version.txt; sha256sum /opt/sab/bin/*; "
              "cd /testbed && codescout constitution-check --project /testbed; echo rc=$?; "
              "codescout constitution-check --project /testbed --path src/lib.rs; echo rc=$?")
    r = docker("run", "--rm", "--network", "none", "--name", "sab-ctl-image", IMAGE,
               "bash", "-c", script, check=False, timeout=120)
    (CONTROLS / "image-check.log").write_text(r.stdout)
    inspect = json.loads(docker("image", "inspect", IMAGE).stdout)[0]
    save("image-check.json", {"image": IMAGE, "image_id": inspect["Id"],
                              "user": inspect["Config"]["User"],
                              "workdir": inspect["Config"]["WorkingDir"],
                              "path": [e for e in inspect["Config"]["Env"]
                                       if e.startswith("PATH=")],
                              "exit_code": r.returncode, "log": "image-check.log"})


# --- hooks ---------------------------------------------------------------------------------

def hook_payloads(session):
    base = {"session_id": session, "transcript_path": f"/run/config/projects/x/{session}.jsonl",
            "cwd": "/testbed", "permission_mode": "bypassPermissions"}
    edit = {"file_path": "/testbed/README.md", "old_string": "Ruff", "new_string": "Ruff"}
    cs_edit = {"path": "crates/ruff/src/lib.rs", "symbol": "main", "action": "replace",
               "body": "fn main() {}"}

    def ev(name, **kw):
        return dict(base, hook_event_name=name, **kw)

    def pre(tool, tool_input):
        return ev("PreToolUse", tool_name=tool, tool_input=tool_input)

    def post(tool, tool_input, response):
        return ev("PostToolUse", tool_name=tool, tool_input=tool_input, tool_response=response)

    agent = {"description": "explore", "prompt": "Find the parser.",
             "subagent_type": "general-purpose"}
    return {
        "session-start.mjs": ev("SessionStart", source="startup"),
        "subagent-guidance.mjs": ev("SubagentStart", agent_id="a1", agent_type="general-purpose"),
        "agent-guide-snapshot.mjs": ev("SubagentStart", agent_id="a1",
                                       agent_type="general-purpose"),
        "constitution-brief.mjs": ev("UserPromptSubmit", prompt="Fix the bug."),
        "constitution-epoch-bump.mjs": ev("PreCompact", trigger="auto", custom_instructions=""),
        "worktree-write-guard.mjs": pre("mcp__codescout__edit_code", cs_edit),
        "constitution-guard.mjs": pre("Edit", edit),
        "pre-edit-dirty-check.mjs": pre("Edit", edit),
        "pre-tool-guard.mjs": pre("Read", {"file_path": "/testbed/README.md"}),
        "git-worktree-guard.mjs": pre("Bash", {"command": "git status"}),
        "pre-task-hint.mjs": pre("Agent", agent),
        "explore-inject.mjs": pre("Agent", agent),
        "pre-edit-hint.mjs": pre("mcp__codescout__edit_code", cs_edit),
        "principal-stamp.mjs": pre("mcp__codescout__symbols", {"path": "crates"}),
        "suspicious-zero-hint.mjs": post("Bash", {"command": "grep -c x README.md"},
                                         {"stdout": "0", "stderr": "", "interrupted": False}),
        "worktree-activate.mjs": post("EnterWorktree", {"name": "w"}, {"path": "/testbed"}),
        "cs-activate-project.mjs": post("mcp__codescout__workspace", {"action": "status"},
                                        [{"type": "text", "text": "{}"}]),
        "cs-liveness.mjs": post("mcp__codescout__symbols", {"path": "crates"},
                                [{"type": "text", "text": "{}"}]),
        "goal-stop-hook.mjs": ev("Stop", stop_hook_active=False),
        "agent-guide-restore.mjs": ev("SubagentStop", agent_id="a1", stop_hook_active=False),
    }


def step_hooks(manifest, seeded=False):
    """Run every companion hook command with a sample payload in a cs container.

    `seeded`: also write the codescout mcpServers entry into the run's config/.claude.json, the
    place the companion's detect.mjs looks. The as-built cs arm passes codescout only through
    --mcp-config, so every hook sees HAS_CODESCOUT=false and exits early.
    """
    companion = Path(manifest["plugins_cs"]["install_path"])
    declared = json.loads((companion / "hooks" / "hooks.json").read_text())["hooks"]
    commands = sorted({h["command"] for groups in declared.values() for g in groups
                       for h in g["hooks"]})
    session = str(uuid.uuid4())
    payloads = hook_payloads(session)
    scripts = {c.split("/hooks/")[-1] for c in commands}
    missing = sorted(scripts - set(payloads))
    if missing:
        raise RuntimeError(f"no sample payload for hook scripts {missing}")
    run_dir = CONTROLS / ("hooks-run-seeded" if seeded else "hooks-run")
    spec = arms.arm_invocation("cs", MODEL, PROMPT, manifest,
                               study_urls(manifest, "http://sab-qdrant-none:6334"), run_dir,
                               session_id=session)
    if seeded:
        (run_dir / "config" / ".claude.json").write_text(
            (run_dir / "sab" / "mcp.json").read_text())
    net = NET + "-hooks"
    docker("network", "create", "--internal", net)
    name = "sab-ctl-hooks"
    try:
        env_file = run_dir / "control.env"
        env_file.write_text(arms.env_file_lines(spec))
        docker(*arms.docker_run_args(name, IMAGE, net, spec, env_file))
        results = []
        for command in commands:
            script = command.split("/hooks/")[-1]
            payload = json.dumps(payloads[script])
            started = time.monotonic()
            r = subprocess.run(["docker", "exec", "-i", "-w", "/testbed", "-e",
                                f"CLAUDE_PLUGIN_ROOT={arms.PLUGINS_ROOT}/{arms.COMPANION}",
                                "-e", "CLAUDE_PROJECT_DIR=/testbed", name, "sh", "-c",
                                command.replace("${CLAUDE_PLUGIN_ROOT}", "$CLAUDE_PLUGIN_ROOT")],
                               input=payload, capture_output=True, text=True, timeout=120)
            crashed = any(s in r.stderr for s in ("Cannot find module", "SyntaxError",
                                                  "ERR_MODULE_NOT_FOUND", "not found",
                                                  "ENOENT", "EACCES", "EROFS"))
            results.append({"command": command, "event": payloads[script]["hook_event_name"],
                            "exit_code": r.returncode,
                            "seconds": round(time.monotonic() - started, 2),
                            "stdout": r.stdout[-3000:], "stderr": r.stderr[-3000:],
                            "crash_marker_in_stderr": crashed})
        save("hooks-seeded.json" if seeded else "hooks.json",
             {"companion": str(companion), "seeded_claude_json": seeded,
              "companion_version": manifest["plugins_cs"]["version"],
              "container": name, "commands": len(commands), "results": results})
    finally:
        docker("rm", "-f", name, check=False)
        docker("network", "rm", net, check=False)


# --- stack ---------------------------------------------------------------------------------

def step_stack(manifest):
    before = vram()
    docker("network", "create", "--internal", NET)
    urls = stack.stack_up(NET, manifest)
    time.sleep(5)
    after = vram()
    sparse_logs = docker("logs", "sab-sparse", check=False).stdout
    sparse_env = docker("inspect", "-f", "{{json .Config.Env}}", "sab-sparse").stdout
    mounts = docker("inspect", "-f", "{{json .Mounts}}", "sab-sparse").stdout
    (CONTROLS / "sab-sparse.log").write_text(sparse_logs)
    (CONTROLS / "sab-dense.log").write_text(docker("logs", "sab-dense", check=False).stdout)
    save("stack.json", {"network": NET, "urls": urls, "vram_before": before, "vram_after": after,
                        "vram_delta_bytes": after["used"] - before["used"],
                        "sparse_env": json.loads(sparse_env), "sparse_mounts": json.loads(mounts),
                        "sparse_ready": True, "logs": ["sab-sparse.log", "sab-dense.log"]})


# --- isolation -----------------------------------------------------------------------------

FIXTURE = {
    "Cargo.toml": '[package]\nname = "zq"\nversion = "0.1.0"\nedition = "2021"\n',
    "src/lib.rs": ("pub mod util;\n\n/// Mixes a sample id into the checksum used by the report "
                   "writer.\npub fn zq_marker_fn(sample: u32) -> u32 {\n"
                   "    util::rotate(sample.wrapping_mul(7919)) ^ 0x5a5a\n}\n"),
    "src/util.rs": ("/// Rotates the bits of a value left by five places.\n"
                    "pub fn rotate(v: u32) -> u32 {\n    v.rotate_left(5)\n}\n"),
}


def probe(container, workdir, calls, out_name, timeout=900):
    calls_file = CONTROLS / f"{out_name}.calls.json"
    calls_file.write_text(json.dumps(calls))
    docker("cp", str(HERE / "mcp_probe.py"), f"{container}:/tmp/mcp_probe.py")
    docker("cp", str(calls_file), f"{container}:/tmp/calls.json")
    r = docker("exec", "-w", workdir, "-e", f"MCP_PROBE_TIMEOUT={timeout}", container,
               "python3", "/tmp/mcp_probe.py", arms.MCP_CONFIG, "/tmp/calls.json",
               check=False, timeout=timeout + 120)
    (CONTROLS / f"{out_name}.probe.json").write_text(r.stdout)
    server_log = docker("exec", container, "cat", "/tmp/mcp-probe-server.log", check=False)
    (CONTROLS / f"{out_name}.server.log").write_text(server_log.stdout)
    if r.returncode:
        raise RuntimeError(f"probe in {container} failed: {r.stdout[-2000:]}")
    return json.loads(r.stdout)


def text_of(call):
    result = call.get("result") or {}
    return "\n".join(c.get("text", "") for c in result.get("content", []))


def step_isolation(manifest):
    out = CONTROLS / "isolation"
    out.mkdir(exist_ok=True)
    src = out / "fixture-zq"
    for rel, body in FIXTURE.items():
        (src / rel).parent.mkdir(parents=True, exist_ok=True)
        (src / rel).write_text(body)
    subprocess.run(["git", "init", "-q", str(src)], check=True)
    subprocess.run(["git", "-C", str(src), "add", "."], check=True)
    subprocess.run(["git", "-C", str(src), "-c", "user.email=c@example.invalid", "-c",
                    "user.name=control", "commit", "-qm", "fixture"], check=True)
    # Fixture image: the agent image plus the fixture tree, owned by the agent user.
    docker("run", "-d", "--name", "sab-ctl-fixture", "--network", "none", IMAGE,
           "sleep", "infinity")
    try:
        docker("cp", "-a", str(src), f"sab-ctl-fixture:{FIXTURE_DIR}")
        docker("commit", "sab-ctl-fixture", FIXTURE_IMAGE)
    finally:
        docker("rm", "-f", "sab-ctl-fixture")
    index_env = arms.study_mcp_env(manifest["mcp_env"],
                                   study_urls(manifest, "http://sab-qdrant:6334"))
    arms.prepare_index(FIXTURE_IMAGE, INDEXED_IMAGE, NET, index_env, out / "prepare",
                       project=FIXTURE_DIR, run="ctl")
    stack.index_down()
    if docker("volume", "inspect", TEMPLATE, check=False).returncode == 0:
        raise RuntimeError(f"{TEMPLATE} already exists")
    docker("volume", "create", TEMPLATE)
    stack.copy_volume(stack.INDEX_VOLUME, TEMPLATE)

    results = {}
    calls = {
        "ctl-a": [["semantic_search", {"query": "function that mixes a sample id into the "
                                                "checksum"}],
                  ["memory", {"action": "remember", "bucket": "unstructured",
                              "title": "zq canary A",
                              "content": f"{CANARY}: run A wrote this semantic memory"}],
                  ["memory", {"action": "recall", "query": f"{CANARY} run A wrote this"}],
                  ["memory", {"action": "write", "topic": "zq-canary-a",
                              "content": f"{CANARY}: run A topic memory"}],
                  ["memory", {"action": "list"}]],
        "ctl-b": [["semantic_search", {"query": "function that mixes a sample id into the "
                                                "checksum"}],
                  ["memory", {"action": "recall", "query": f"{CANARY} run A wrote this"}],
                  ["memory", {"action": "list"}]],
    }
    for run in ("ctl-a", "ctl-b"):
        url = stack.qdrant_for_run(TEMPLATE, run, network=NET)
        spec = arms.arm_invocation("cs", MODEL, PROMPT, manifest, study_urls(manifest, url),
                                   out / run)
        name = f"sab-{run}"
        start_arm_container(name, INDEXED_IMAGE, spec, out / run)
        results[run] = probe(name, FIXTURE_DIR, calls[run], f"isolation-{run}")
        results[run]["qdrant_url"] = url
    a, b = results["ctl-a"], results["ctl-b"]
    checks = {
        "a_semantic_search_finds_marker": "zq_marker_fn" in text_of(a["calls"][0]),
        "a_recall_finds_its_canary": CANARY in text_of(a["calls"][2]),
        "a_list_shows_its_topic": "zq-canary-a" in text_of(a["calls"][4]),
        "b_semantic_search_finds_marker": "zq_marker_fn" in text_of(b["calls"][0]),
        "b_recall_does_not_see_a_canary": CANARY not in text_of(b["calls"][1]),
        "b_list_does_not_show_a_topic": "zq-canary-a" not in text_of(b["calls"][2]),
        "a_and_b_qdrant_differ": a["qdrant_url"] != b["qdrant_url"],
    }
    save("stack-isolation.json", {"template": TEMPLATE, "indexed_image": INDEXED_IMAGE,
                                  "fixture": FIXTURE_DIR, "canary": CANARY, "checks": checks,
                                  "pass": all(checks.values()),
                                  "probes": ["isolation-ctl-a.probe.json",
                                             "isolation-ctl-b.probe.json"],
                                  "index_cost": json.loads((out / "prepare" / "index-cost.json")
                                                           .read_text())})
    for run in ("ctl-a", "ctl-b"):
        docker("rm", "-f", f"sab-{run}", check=False)
        stack.run_down(run)


# --- rust-analyzer under the cap -------------------------------------------------------------

def step_ra(manifest):
    out = CONTROLS / "ra"
    out.mkdir(exist_ok=True)
    url = stack.qdrant_for_run(TEMPLATE, "ctl-ra", network=NET)
    spec = arms.arm_invocation("cs", MODEL, PROMPT, manifest, study_urls(manifest, url),
                               out / "run")
    name = "sab-ctl-ra"
    try:
        start_arm_container(name, IMAGE, spec, out / "run")
        calls = [["symbols", {"name": "Checker", "kind": "struct", "limit": 5}],
                 ["references", {"symbol": "Checker", "path":
                                 "crates/ruff_linter/src/checkers/ast/mod.rs"}]]
        try:
            result = probe(name, "/testbed", calls, "ra", timeout=1200)
            err = None
        except RuntimeError as e:
            result, err = None, str(e)
        cg = {f: docker("exec", name, "cat", f"/sys/fs/cgroup/{f}", check=False).stdout.strip()
              for f in ("memory.peak", "memory.max", "memory.events", "memory.current")}
        state = json.loads(docker("inspect", "-f", "{{json .State}}", name).stdout)
        ps = docker("exec", name, "ps", "-eo", "pid,rss,comm", check=False).stdout
        save("ra-memory.json", {"container": name, "cgroup": cg, "oom_killed": state["OOMKilled"],
                                "running": state["Running"], "ps_after": ps, "probe_error": err,
                                "calls": [{"tool": c["tool"], "seconds": c["seconds"],
                                           "error": c["error"], "probe_error": c["probe_error"],
                                           "text_head": text_of(c)[:600]}
                                          for c in (result or {}).get("calls", [])]})
    finally:
        docker("rm", "-f", name, check=False)
        stack.run_down("ctl-ra")


# --- R10: the stream before the API failure -------------------------------------------------

def summarize_stream(raw):
    events = []
    for line in raw.splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    init = next((e for e in events if e.get("type") == "system" and e.get("subtype") == "init"),
                None)
    hooks = [e for e in events if e.get("type") == "system"
             and str(e.get("subtype", "")).startswith("hook")]
    summary = {"events": len(events),
               "event_kinds": sorted({f"{e.get('type')}/{e.get('subtype', '')}"
                                      for e in events}),
               "init_emitted": init is not None}
    if init:
        tools = init.get("tools", [])
        summary.update({
            "tools": tools,
            "mcp_tools": sorted(t for t in tools if t.startswith("mcp__")),
            "mcp_servers": init.get("mcp_servers"),
            "plugins": init.get("plugins"),
            "skills_count": len(init.get("skills", []) or []),
            "skills_sample": (init.get("skills") or [])[:40],
            "slash_commands_count": len(init.get("slash_commands", []) or []),
            "output_style": init.get("output_style"),
            "model": init.get("model"),
            "apiKeySource": init.get("apiKeySource"),
            "permissionMode": init.get("permissionMode"),
            "init_keys": sorted(init),
        })
    summary["hook_events"] = [{k: e.get(k) for k in ("subtype", "hook_event", "hook_name",
                                                      "exit_code", "outcome")}
                              for e in hooks]
    summary["hook_events_raw_count"] = len(hooks)
    last = events[-1] if events else None
    summary["last_event"] = {k: (last.get(k) if last else None)
                             for k in ("type", "subtype", "is_error", "result")}
    return summary


def step_r10(manifest):
    out = CONTROLS / "r10"
    out.mkdir(exist_ok=True)
    report = {"network": NET, "token": "none (no CLAUDE_CODE_OAUTH_TOKEN, no ANTHROPIC_API_KEY, "
              "empty CLAUDE_CONFIG_DIR)", "proxy": "none", "arms": {}}
    for arm in arms.ARMS:
        run = f"ctl-r10{arm}" if arm != "bare" else None
        url = (stack.qdrant_for_run(TEMPLATE, run, network=NET) if run
               else "http://sab-qdrant-unused:6334")
        spec = arms.arm_invocation(arm, MODEL, PROMPT, manifest, study_urls(manifest, url),
                                   out / arm, session_id=str(uuid.uuid4()))
        args = list(spec.cli_args)
        i = args.index("--output-format")
        args[i:i + 2] = ["--output-format", "stream-json", "--verbose", "--include-hook-events"]
        sep = args.index("--")
        args[sep:sep] = ["--debug-file", f"{arms.CONFIG_DIR}/r10-debug.log"]
        name = f"sab-ctl-r10-{arm}"
        try:
            start_arm_container(name, IMAGE, spec, out / arm)
            env = docker("exec", name, "env", check=False).stdout
            leaked = [l.split("=")[0] for l in env.splitlines()
                      if l.split("=")[0] in ("CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY",
                                             "ANTHROPIC_AUTH_TOKEN", "HTTPS_PROXY",
                                             "HTTP_PROXY")]
            if leaked:
                raise RuntimeError(f"credential/proxy env present in {name}: {leaked}")
            started = time.monotonic()
            try:
                r = subprocess.run(["docker", "exec", "-w", "/testbed", name, *args],
                                   capture_output=True, text=True, timeout=240)
                stdout, stderr, rc = r.stdout, r.stderr, r.returncode
            except subprocess.TimeoutExpired as e:
                stdout = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
                stderr = (e.stderr or b"").decode() if isinstance(e.stderr, bytes) else (e.stderr or "")
                rc = "timeout"
            (out / f"{arm}.stream.jsonl").write_text(stdout)
            (out / f"{arm}.stderr.txt").write_text(stderr)
            summary = summarize_stream(stdout)
            summary.update({"exit_code": rc, "seconds": round(time.monotonic() - started, 1),
                            "args": args, "mounts": spec.mounts,
                            "stream": f"r10/{arm}.stream.jsonl",
                            "debug_log": str(out / arm / "config" / "r10-debug.log")})
            report["arms"][arm] = summary
        finally:
            docker("rm", "-f", name, check=False)
            if run:
                stack.run_down(run)
    cs = report["arms"]["cs"]
    report["tools_flag_hides_mcp_tools"] = (
        None if not cs.get("init_emitted") else not cs.get("mcp_tools"))
    save("r10-streams.json", report)
    save("tools-flag.json", {
        "question": "does --tools Bash,Read,Write,Edit,Glob,Grep hide MCP tools?",
        "answer": report["tools_flag_hides_mcp_tools"],
        "evidence": {arm: {"init_emitted": s.get("init_emitted"),
                           "mcp_tools": s.get("mcp_tools"), "mcp_servers": s.get("mcp_servers"),
                           "tools": s.get("tools")}
                     for arm, s in report["arms"].items()},
        "streams": [f"r10/{a}.stream.jsonl" for a in arms.ARMS]})


# --- teardown ------------------------------------------------------------------------------

def step_down(manifest):
    removed = {"stack_down": stack.stack_down()}
    for run in RUNS:
        stack.run_down(run)
    removed["template"] = stack.remove_template(TEMPLATE)
    vols = docker("volume", "ls", "--format", "{{.Name}}").stdout.split()
    if stack.MODEL_CACHE in vols:
        docker("volume", "rm", stack.MODEL_CACHE)
        removed["model_cache"] = stack.MODEL_CACHE
    for image in (FIXTURE_IMAGE, INDEXED_IMAGE):
        docker("image", "rm", image, check=False)
    for net in (NET, NET + "-hooks"):
        docker("network", "rm", net, check=False)
    left = [n for n in docker("ps", "-a", "--format", "{{.Names}}").stdout.split()
            if n.startswith("sab-")]
    left_v = [v for v in docker("volume", "ls", "--format", "{{.Name}}").stdout.split()
              if v.startswith("sab-")]
    left_n = [n for n in docker("network", "ls", "--format", "{{.Name}}").stdout.split()
              if n.startswith("sab-")]
    removed.update({"left_containers": left, "left_volumes": left_v, "left_networks": left_n})
    save("teardown.json", removed)


STEPS = {"image": step_image, "hooks": step_hooks,
         "hooks-seeded": lambda m: step_hooks(m, seeded=True), "stack": step_stack,
         "isolation": step_isolation, "ra": step_ra, "r10": step_r10, "down": step_down}


def main(argv):
    manifest = json.loads(Path(argv[1]).read_text())
    CONTROLS.mkdir(parents=True, exist_ok=True)
    for step in argv[2:]:
        STEPS[step](manifest)


if __name__ == "__main__":
    main(sys.argv)
