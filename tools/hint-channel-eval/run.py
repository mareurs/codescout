"""Runner for the hint channel and authority eval (docs/evals/harness-channel-hint-authority-preregistration.md,
id cffe594cbf26fd72). Stdlib only.

    python3 -I tools/hint-channel-eval/run.py prepare   # trees + offline cargo home (network)
    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/run.py run <task_id> <arm> <model> <rep>

Data lives outside the repo, under $HCE_DATA (default /home/marius/work/claude/hce-data):
    cargo-home/          seeded from S5's cargo home (copied, never modified there), then
                         `cargo fetch --locked` for every task's parent
    tasks/<task_id>/tree the task's parent as `git archive` (tracked files only), read-only master;
                         each run gets its own copy
    prepare.json         what prepare did, with the parent and lock hash of every tree
"""
import hashlib
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))  # `python3 -I` leaves the script dir off sys.path; hook.py lives here
REPO = HERE.parents[1]
TASKS = REPO / "docs/evals/data/2026-11-04-harness-channel-hints/tasks.jsonl"
DATA = Path(os.environ.get("HCE_DATA", "/home/marius/work/claude/hce-data"))
S5_CARGO_HOME = Path("/home/marius/work/claude/s5-work/cargo-home")
CARGO_HOME = DATA / "cargo-home"


def load_tasks():
    return [json.loads(line) for line in TASKS.read_text().splitlines() if line.strip()]


def sh(*args, **kw):
    return subprocess.run(args, check=True, **kw)


def export_tree(parent, dest):
    """Write `git archive <parent>` into dest (tracked files only, no .git).

    A tracked symlink whose target is absolute or outside the tree is skipped, never followed
    (`.claude/skills/claude-traces` points into the operator's home). Returns (created, skipped)."""
    skipped = []
    if dest.exists():
        return False, skipped
    tmp = dest.with_name(dest.name + ".partial")
    if tmp.exists():
        shutil.rmtree(tmp)
    tmp.mkdir(parents=True)
    blob = subprocess.run(["git", "-C", str(REPO), "archive", "--format=tar", parent],
                          check=True, capture_output=True).stdout

    def safe(member, path):
        try:
            return tarfile.data_filter(member, path)
        except (tarfile.AbsoluteLinkError, tarfile.LinkOutsideDestinationError):
            skipped.append(member.name)
            return None

    with tarfile.open(fileobj=io.BytesIO(blob)) as tar:
        tar.extractall(tmp, filter=safe)
    tmp.rename(dest)
    return True, skipped


def prepare():
    DATA.mkdir(parents=True, exist_ok=True)
    if not CARGO_HOME.exists():
        print(f"seeding {CARGO_HOME} from {S5_CARGO_HOME}", flush=True)
        shutil.copytree(S5_CARGO_HOME, CARGO_HOME, symlinks=True)
    record = {"cargo_home": str(CARGO_HOME), "tasks": []}
    for t in load_tasks():
        tree = DATA / "tasks" / t["task_id"] / "tree"
        made, skipped = export_tree(t["parent"], tree)
        env = dict(os.environ, CARGO_HOME=str(CARGO_HOME), RUSTUP_TOOLCHAIN="1.97.1")
        r = subprocess.run(["cargo", "fetch", "--locked", "--manifest-path", str(tree / "Cargo.toml")],
                           env=env, capture_output=True, text=True)
        lock = hashlib.sha256((tree / "Cargo.lock").read_bytes()).hexdigest()
        row = {"task_id": t["task_id"], "parent": t["parent"], "tree_created": made,
               "skipped_links": skipped,
               "cargo_lock_sha256": lock, "fetch_exit": r.returncode,
               "fetch_stderr_tail": r.stderr[-600:] if r.returncode else ""}
        record["tasks"].append(row)
        print(f"{t['task_id']}: tree {'created' if made else 'kept'}, fetch exit {r.returncode}", flush=True)
    (DATA / "prepare.json").write_text(json.dumps(record, indent=2) + "\n")
    bad = [r["task_id"] for r in record["tasks"] if r["fetch_exit"] != 0]
    print("prepare:", "OK" if not bad else f"FETCH FAILED for {bad}")
    return 0 if not bad else 1


# --- run --------------------------------------------------------------------------------------
# Paths inside the container mirror tools/setup-ab-eval/arms.py so the agent sees the same layout.
IMAGE = "sab-agent-cs-cs-p16:f9b0fc9f8"   # codescout f9b0fc9f8 + rust-analyzer + node in /opt/sab/bin
CLI_HOST = Path("/home/marius/.cache/codescout-resident/codescout/behavior-transfer-20261029/claude-2.1.309")
CLI_SHA256 = "4d704a8aecbe038affa3000f498ae17c1b4f753add8facb8764120affc99b658"
COMPANION_HOST = Path("/home/marius/.claude/plugins/cache/sdd-misc-plugins/codescout-companion/1.20.21")
C_CLI, C_SETTINGS, C_CONFIG, C_MCP = "/opt/claude", "/opt/settings.json", "/run/config", "/opt/sab/mcp.json"
C_COMPANION, C_HOOK, C_STATE = "/opt/plugins/codescout-companion", "/opt/hce/hook.py", "/opt/hce-state"
NATIVE_TOOLS = "Bash,Read,Write,Edit,Glob,Grep"
MAX_BUDGET_USD = "1.25"
RUN_TIMEOUT_S = 1200
ARMS = ("F", "A0", "A1", "A2", "A3", "P", "L")
PROJECT_TOML_REL = ".codescout/project.toml"
PROJECT_TOML = '[project]\nname = "codescout"\nlanguages = ["rust"]\n'
LOOKUP_TOOLS = ("references", "call_graph", "symbol_at")


def read_token():
    """HCE_TOKEN_FILE: a regular file, mode 0600. No default: the operator names the paying account."""
    path = os.environ.get("HCE_TOKEN_FILE")
    if not path:
        raise SystemExit("HCE_TOKEN_FILE is not set; the operator names the token file")
    st = Path(path).lstat()
    if not stat.S_ISREG(st.st_mode) or (st.st_mode & 0o077):
        raise SystemExit(f"{path} must be a regular file with mode 0600")
    token = Path(path).read_text().strip()
    if not token or any(c.isspace() for c in token):
        raise SystemExit(f"{path} does not hold one token")
    return token


def _sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def stage_run(task, arm, model, rep, run_dir):
    """Write everything a run needs under run_dir; return (docker args, the env-file lines without the token)."""
    from hook import DECLARATION  # same bytes as the A2 declaration
    master = DATA / "tasks" / task["task_id"] / "tree"
    testbed, config, state = run_dir / "testbed", run_dir / "config", run_dir / "state"
    shutil.copytree(master, testbed, symlinks=True)
    if arm == "A3":
        with open(testbed / "CLAUDE.md", "a", encoding="utf-8") as f:
            f.write("\n" + DECLARATION + "\n")
    # references, call_graph and symbol_at are gated on has_lsp, which comes from `[project] languages`
    # in this untracked file (setup-ab arms.py LSP_GATED_TOOLS). Without it the agent sees 18 tools and
    # cannot call references at all: Stage 1's first pass ran that way (AM-6). rust only: rust-analyzer
    # is the one language server in the image. Committed into the base, as setup-ab did.
    (testbed / ".codescout").mkdir(exist_ok=True)
    (testbed / PROJECT_TOML_REL).write_text(PROJECT_TOML)
    git = ["git", "-C", str(testbed), "-c", "user.name=Task", "-c", "user.email=task@example.invalid"]
    sh(*git, "init", "-q")
    sh(*git, "add", "-A")
    sh(*git, "add", "-f", PROJECT_TOML_REL)
    sh(*git, "commit", "-qm", f"task {task['task_id']}")
    config.mkdir()
    state.mkdir()
    servers = {"codescout": {"type": "stdio", "command": "/opt/sab/bin/codescout", "args": ["start", "--debug"]}}
    (run_dir / "mcp.json").write_text(json.dumps({"mcpServers": servers}, indent=2) + "\n")
    # setup-ab C1 ruling: the companion finds codescout only through the config dir's .claude.json.
    (config / ".claude.json").write_text(json.dumps({"mcpServers": servers}, indent=2) + "\n")
    settings = {"hooks": {"PostToolUse": [{"matcher": "*", "hooks": [
        {"type": "command", "command": f"python3 -I {C_HOOK}"}]}]}}
    (run_dir / "settings.json").write_text(json.dumps(settings, indent=2) + "\n")
    session_id = str(uuid.uuid4())
    env_lines = {"CLAUDE_CONFIG_DIR": C_CONFIG, "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "8192",
                 "DISABLE_AUTOUPDATER": "1", "CARGO_NET_OFFLINE": "true",
                 "HCE_ARM": arm, "HCE_FN": task["fn"], "HCE_PATH": task["path"], "HCE_STATE_DIR": C_STATE}
    mounts = [(testbed, "/testbed", "rw"), (CARGO_HOME, "/cargo", "rw"), (CLI_HOST, C_CLI, "ro"),
              (config, C_CONFIG, "rw"), (run_dir / "settings.json", C_SETTINGS, "ro"),
              (run_dir / "mcp.json", C_MCP, "ro"), (COMPANION_HOST, C_COMPANION, "ro"),
              (HERE / "hook.py", C_HOOK, "ro"), (state, C_STATE, "rw")]
    cli = [C_CLI, "-p", "--model", model, "--session-id", session_id, "--setting-sources", "local",
           "--settings", C_SETTINGS, "--strict-mcp-config", "--dangerously-skip-permissions",
           "--tools", NATIVE_TOOLS, "--output-format", "stream-json", "--verbose",
           "--max-budget-usd", MAX_BUDGET_USD, "--mcp-config", C_MCP, "--plugin-dir", C_COMPANION,
           "--", task["prompt"]]
    docker = ["docker", "run", "--rm", "--name", f"hce-{run_dir.name}", "--user", "1000:1000",
              "--workdir", "/testbed", "--cpus", "4", "--memory", "24g", "--memory-swap", "24g",
              "--env-file", str(run_dir / "env")]
    for src, dst, mode in mounts:
        docker += ["--mount", f"type=bind,source={src},target={dst}" + (",readonly" if mode == "ro" else "")]
    docker += ["--entrypoint", C_CLI, IMAGE, *cli[1:]]
    record = {"task_id": task["task_id"], "arm": arm, "model": model, "rep": rep, "session_id": session_id,
              "image": IMAGE, "cli_sha256": CLI_SHA256, "companion": str(COMPANION_HOST),
              "hook_sha256": _sha256(HERE / "hook.py"), "project_toml": PROJECT_TOML,
              "env": env_lines, "cli_args": cli[1:-1],
              "network": "docker default bridge (NOT restricted; see AM-4)"}
    (run_dir / "run-record.json").write_text(json.dumps(record, indent=2) + "\n")
    return docker, env_lines


def run_one(task_id, arm, model, rep):
    if arm not in ARMS:
        raise SystemExit(f"unknown arm {arm!r}; expected one of {ARMS}")
    if _sha256(CLI_HOST) != CLI_SHA256:
        raise SystemExit("frozen CLI sha256 mismatch")
    task = {t["task_id"]: t for t in load_tasks()}[task_id]
    token = read_token()
    run_dir = DATA / "runs" / arm / f"{task_id}-r{rep}-{time.strftime('%Y%m%dT%H%M%S')}"
    run_dir.mkdir(parents=True)
    docker, env_lines = stage_run(task, arm, model, rep, run_dir)
    env_path = run_dir / "env"
    fd = os.open(env_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write("".join(f"{k}={v}\n" for k, v in env_lines.items()) + f"CLAUDE_CODE_OAUTH_TOKEN={token}\n")
    t0 = time.time()
    try:
        with open(run_dir / "stream.jsonl", "wb") as out, open(run_dir / "stderr.txt", "wb") as err:
            try:
                r = subprocess.run(docker, stdout=out, stderr=err, timeout=RUN_TIMEOUT_S)
                exit_code, timed_out = r.returncode, False
            except subprocess.TimeoutExpired:
                subprocess.run(["docker", "kill", f"hce-{run_dir.name}"], capture_output=True)
                exit_code, timed_out = None, True
    finally:
        env_path.unlink()
    outcome = {"exit_code": exit_code, "timed_out": timed_out, "wall_s": round(time.time() - t0, 1)}
    (run_dir / "outcome.json").write_text(json.dumps(outcome, indent=2) + "\n")
    print(json.dumps({"run_dir": str(run_dir), **outcome}))
    return 0 if exit_code == 0 else 1



SMOKE_SCRIPT = r"""
set -u
echo "cli: $(/opt/claude --version 2>&1)"
echo "codescout: $(codescout --version 2>&1)"
echo "rust-analyzer: $(rust-analyzer --version 2>&1)"
echo "python3: $(python3 --version 2>&1)"
printf '%s' '{"tool_name":"mcp__codescout__symbols","tool_input":{"name":"'"$HCE_FN"'"},"tool_response":[{"type":"text","text":"x"}],"tool_use_id":"toolu_smoke"}' \
  | python3 -I /opt/hce/hook.py; echo
cat /opt/hce-state/events.jsonl
cd /testbed && timeout 300 cargo metadata --offline --format-version 1 >/dev/null && echo "cargo metadata: OK" || echo "cargo metadata: FAILED"
git -C /testbed log --oneline | head -1
git -C /testbed ls-files .codescout/project.toml
timeout 900 python3 -I /opt/hce-state/mcp_probe.py
"""

MCP_PROBE = r'''
import json, os, subprocess, sys, time
p = subprocess.Popen(["codescout", "start"], cwd="/testbed", stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
def send(m):
    p.stdin.write(json.dumps(m) + "\n"); p.stdin.flush()
def recv(mid):
    while True:
        line = p.stdout.readline()
        if not line:
            raise SystemExit(f"server closed before id {mid}")
        m = json.loads(line)
        if m.get("id") == mid:
            return m
def call(mid, name, args):
    t0 = time.time()
    send({"jsonrpc": "2.0", "id": mid, "method": "tools/call", "params": {"name": name, "arguments": args}})
    r = recv(mid)
    text = " ".join(c.get("text", "") for c in r.get("result", {}).get("content", []))
    return round(time.time() - t0, 1), r.get("result", {}).get("isError"), text
send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18",
      "capabilities": {}, "clientInfo": {"name": "hce-smoke", "version": "0"}}})
recv(1)
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
print("activate:", call(2, "workspace", {"action": "activate", "path": "/testbed"})[:2])
send({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {}})
names = sorted(t["name"] for t in recv(3)["result"]["tools"])
print(f"tools ({len(names)}):", names)
for want in ("references", "call_graph", "symbol_at"):
    print(f"  {want}: {'PRESENT' if want in names else 'MISSING'}")
if "references" in names:
    secs, err, text = call(4, "references", {"symbol": os.environ["HCE_FN"], "path": os.environ["HCE_PATH"]})
    print(f"references: {secs}s isError={err}: {text[:500]}")
p.terminate()
'''


def smoke(task_id, arm):
    """No model call: stage a real run, then check the container's tools, the hook, offline cargo,
    and over MCP that references is listed and answers on the target."""
    task = {t["task_id"]: t for t in load_tasks()}[task_id]
    run_dir = DATA / "smoke" / f"{task_id}-{arm}-{time.strftime('%Y%m%dT%H%M%S')}"
    run_dir.mkdir(parents=True)
    docker, env_lines = stage_run(task, arm, "none", 0, run_dir)
    (run_dir / "env").write_text("".join(f"{k}={v}\n" for k, v in env_lines.items()))
    (run_dir / "state" / "mcp_probe.py").write_text(MCP_PROBE)
    cut = docker.index("--entrypoint")
    cmd = docker[:cut] + ["--entrypoint", "sh", IMAGE, "-c", SMOKE_SCRIPT]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=1200)
    print(r.stdout + r.stderr)
    return r.returncode



def main(argv):
    if len(argv) >= 2 and argv[1] == "prepare":
        return prepare()
    if len(argv) == 4 and argv[1] == "smoke":
        return smoke(argv[2], argv[3])
    if len(argv) == 6 and argv[1] == "run":
        return run_one(argv[2], argv[3], argv[4], int(argv[5]))
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
