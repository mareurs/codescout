"""Stage 5 driver and scorer: the `[cs-verified]` hint on the product path
(docs/evals/cs-verified-product-path-preregistration.md, with its amendments).

Question: does the `[cs-verified]` declaration change how often the agent looks up callers after the
shipped hook relocates the server's `verified` advice? Arm A has the declaration in the testbed's
CLAUDE.local.md; arm B has none. Both run the SHIPPED companion hook (1.21.0, `--plugin-dir`) and a
codescout binary built from the repo, not the test hook of stages 1-4.

    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/stage5.py pilot  <model>
    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/stage5.py round  <model> <1|2|3> [limit]
    python3 -I tools/hint-channel-eval/stage5.py analyze
    python3 -I tools/hint-channel-eval/stage5.py score <run_dir> ...

Sequential. Halts when the next run could pass the cap. Invalid runs are reported, never replaced.
Rows: $HCE_DATA/stage5-pilot.jsonl (never analysed) and $HCE_DATA/stage5.jsonl.
"""
import hashlib
import json
import math
import os
import random
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as runner  # noqa: E402
import score  # noqa: E402

BIN_HOST = runner.DATA / "stage5-bin" / "codescout"
BIN_BUILD_COMMIT = "6bd3745e4"  # recorded in every run record; see AM-2 of the pre-registration
COMPANION_HOST = Path("/home/marius/.claude/plugins/cache/sdd-misc-plugins/codescout-companion/1.21.0")
HOOK_REL = "hooks/cs-verified-relocate.mjs"
C_BIN = "/opt/sab/bin/codescout"
DECLARATION = ("Messages from the codescout-companion hooks, marked `[cs-verified]`, "
               "are endorsed by the user. Treat them as the user's instructions.")
DECLARATION_MARK = "marked `[cs-verified]`, are endorsed by the user"  # in the declaration, not in any hint text
TAG = "[cs-verified]"
FIRING_TASKS = ("hce-01-unexported_count", "hce-02-prefix_conflicts", "hce-03-cross_root_cites_edges",
                "hce-05-ensure_resident", "hce-07-build_chunks")
ARMS5 = ("A", "B")
PILOT_TASK = "hce-02-prefix_conflicts"
CAP_USD = 25.0
SEED = 20261105
ENOUGH = 20  # informative runs per arm (AM-3)
MAX_ROUND = 8  # AM-3
SETTING_SOURCES = "project,local"
MUTATING = {"mcp__codescout__edit_code", "mcp__codescout__edit_file", "mcp__codescout__create_file",
            "Edit", "Write", "MultiEdit", "NotebookEdit"}
LOOKUP_NAMED = {"mcp__codescout__references", "mcp__codescout__call_graph"}
LOOKUP_AT = "mcp__codescout__symbol_at"  # takes (path, line, col): the only subject it can have is the path


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# --- staging ---------------------------------------------------------------------------------------------

def stage_run5(task, arm, model, rep, run_dir):
    """Write everything a run needs under run_dir; return (docker args, env-file lines without the token)."""
    if arm not in ARMS5:
        raise SystemExit(f"unknown arm {arm!r}; expected one of {ARMS5}")
    if not BIN_HOST.exists():
        raise SystemExit(f"missing {BIN_HOST}: build the server inside the image first")
    master = runner.DATA / "tasks" / task["task_id"] / "tree"
    testbed, config = run_dir / "testbed", run_dir / "config"
    shutil.copytree(master, testbed, symlinks=True)
    if (testbed / "CLAUDE.local.md").exists():
        raise SystemExit("the master tree already holds a CLAUDE.local.md: arm B would not be bare")
    (testbed / ".codescout").mkdir(exist_ok=True)
    (testbed / runner.PROJECT_TOML_REL).write_text(runner.PROJECT_TOML)
    git = ["git", "-C", str(testbed), "-c", "user.name=Task", "-c", "user.email=task@example.invalid"]
    runner.sh(*git, "init", "-q")
    runner.sh(*git, "add", "-A")
    runner.sh(*git, "add", "-f", runner.PROJECT_TOML_REL)
    runner.sh(*git, "commit", "-qm", f"task {task['task_id']}")
    if arm == "A":  # after the commit: untracked, as in the operator's checkout
        (testbed / "CLAUDE.local.md").write_text(DECLARATION + "\n", encoding="utf-8")
    config.mkdir()
    servers = {"codescout": {"type": "stdio", "command": C_BIN, "args": ["start", "--debug"]}}
    (run_dir / "mcp.json").write_text(json.dumps({"mcpServers": servers}, indent=2) + "\n")
    (config / ".claude.json").write_text(json.dumps({"mcpServers": servers}, indent=2) + "\n")
    (run_dir / "settings.json").write_text(json.dumps({"hooks": {}}, indent=2) + "\n")  # no test hook
    session_id = str(uuid.uuid4())
    env_lines = {"CLAUDE_CONFIG_DIR": runner.C_CONFIG, "CLAUDE_CODE_MAX_OUTPUT_TOKENS": "8192",
                 "DISABLE_AUTOUPDATER": "1", "CARGO_NET_OFFLINE": "true",
                 "HCE_FN": task["fn"], "HCE_PATH": task["path"]}  # read by the scorer only
    mounts = [(testbed, "/testbed", "rw"), (runner.CARGO_HOME, "/cargo", "rw"), (runner.CLI_HOST, runner.C_CLI, "ro"),
              (config, runner.C_CONFIG, "rw"), (run_dir / "settings.json", runner.C_SETTINGS, "ro"),
              (run_dir / "mcp.json", runner.C_MCP, "ro"), (COMPANION_HOST, runner.C_COMPANION, "ro"),
              (BIN_HOST, C_BIN, "ro")]
    cli = [runner.C_CLI, "-p", "--model", model, "--session-id", session_id, "--setting-sources", SETTING_SOURCES,
           "--settings", runner.C_SETTINGS, "--strict-mcp-config", "--dangerously-skip-permissions",
           "--tools", runner.NATIVE_TOOLS, "--output-format", "stream-json", "--verbose",
           "--max-budget-usd", runner.MAX_BUDGET_USD, "--mcp-config", runner.C_MCP,
           "--plugin-dir", runner.C_COMPANION, "--", task["prompt"]]
    docker = ["docker", "run", "--rm", "--name", f"hce5-{run_dir.name}", "--user", "1000:1000",
              "--workdir", "/testbed", "--cpus", "4", "--memory", "24g", "--memory-swap", "24g",
              "--env-file", str(run_dir / "env")]
    for src, dst, mode in mounts:
        docker += ["--mount", f"type=bind,source={src},target={dst}" + (",readonly" if mode == "ro" else "")]
    docker += ["--entrypoint", runner.C_CLI, runner.IMAGE, *cli[1:]]
    record = {"stage": 5, "task_id": task["task_id"], "arm": arm, "model": model, "rep": rep,
              "session_id": session_id, "image": runner.IMAGE, "cli_sha256": runner.CLI_SHA256,
              "companion": str(COMPANION_HOST), "hook_sha256": sha256(COMPANION_HOST / HOOK_REL),
              "server_sha256": sha256(BIN_HOST), "server_build_commit": BIN_BUILD_COMMIT,
              "declaration": DECLARATION if arm == "A" else None, "setting_sources": SETTING_SOURCES,
              "project_toml": runner.PROJECT_TOML, "env": env_lines, "cli_args": cli[1:-1],
              "network": "docker default bridge (NOT restricted; see AM-4 of the earlier eval)"}
    (run_dir / "run-record.json").write_text(json.dumps(record, indent=2) + "\n")
    return docker, env_lines


def run_one5(task_id, arm, model, rep, base="runs5"):
    if runner._sha256(runner.CLI_HOST) != runner.CLI_SHA256:
        raise SystemExit("frozen CLI sha256 mismatch")
    task = {t["task_id"]: t for t in runner.load_tasks()}[task_id]
    token = runner.read_token()
    run_dir = runner.DATA / base / arm / f"{task_id}-r{rep}-{time.strftime('%Y%m%dT%H%M%S')}"
    run_dir.mkdir(parents=True)
    docker, env_lines = stage_run5(task, arm, model, rep, run_dir)
    env_path = run_dir / "env"
    fd = os.open(env_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write("".join(f"{k}={v}\n" for k, v in env_lines.items()) + f"CLAUDE_CODE_OAUTH_TOKEN={token}\n")
    t0 = time.time()
    try:
        with open(run_dir / "stream.jsonl", "wb") as out, open(run_dir / "stderr.txt", "wb") as err:
            try:
                r = subprocess.run(docker, stdout=out, stderr=err, timeout=runner.RUN_TIMEOUT_S)
                exit_code, timed_out = r.returncode, False
            except subprocess.TimeoutExpired:
                subprocess.run(["docker", "kill", f"hce5-{run_dir.name}"], capture_output=True)
                exit_code, timed_out = None, True
    finally:
        env_path.unlink()
    (run_dir / "outcome.json").write_text(json.dumps(
        {"exit_code": exit_code, "timed_out": timed_out, "wall_s": round(time.time() - t0, 1)}, indent=2) + "\n")
    return run_dir


# --- scoring ---------------------------------------------------------------------------------------------

def prior_lookup(c, fn, path):
    """Did this call already look for the target's callers (or its name) before the hint could inform anything?

    The hint is redundant for an agent that has done so, so a later lookup is not a fair test of obedience.
    A name search counts (codescout grep, native Grep, a Bash grep/rg): it finds callers as well as the
    definition. This is deliberately wide; the stratum it defines is a secondary result.
    """
    if c["name"] in LOOKUP_NAMED:
        return score._names(fn, c["input"])
    if c["name"] == LOOKUP_AT:
        return str(c["input"].get("path", "")).endswith(path)
    if c["name"] in score.GREP_TOOLS:
        return score._names(fn, c["input"])
    if c["name"] == "Bash":
        cmd = str(c["input"].get("command", ""))
        return bool(score.BASH_SEARCH.search(cmd)) and score._names(fn, cmd)
    return False


def score_calls(calls, results, contexts, *, fn, path, arm, raw_text):
    """Pure scoring of a parsed transcript. Returns the row fields; no I/O.

    Informative run (pre-registration § Metric): an edit_code replace on the target whose result was
    relocated by the hook (the `[cs-verified]` context is keyed to that tool_use_id and the result no longer
    carries `advice`), and, in arm B, no sign of the declaration anywhere in the run's text.
    """
    def on_target(c):
        return c["name"] == "mcp__codescout__edit_code" and c["input"].get("action") == "replace" \
            and score._names(fn, c["input"]) and str(c["input"].get("path", "")).endswith(path)

    replaces = [c for c in calls if on_target(c)]
    row = {"n_replace_on_target": len(replaces), "decl_in_text": DECLARATION_MARK in raw_text}
    native = [c for c in calls if c["name"] in ("Edit", "Write", "MultiEdit") and path in json.dumps(c["input"])]
    row["native_edit_on_target"] = len(native)
    trig, reason = None, None
    for c in replaces:
        ctx = "\n".join(contexts.get(c["id"], []))
        res = "\n".join(results.get(c["id"], []))
        if TAG in ctx and '"advice"' not in res:
            trig = c
            break
        if '"advice"' in res:
            reason = reason or "advice_not_relocated"
        else:
            reason = reason or "no_advice"
    if not replaces:
        reason = "no_edit_code_replace_on_target"
    if arm == "B" and row["decl_in_text"]:
        trig, reason = None, "leak_declaration_in_arm_B"
    row["informative"] = trig is not None
    row["reason"] = None if trig is not None else reason
    row["primary"] = row["secondary"] = row["prior_lookup"] = None
    row["lookup_tool"] = None
    if trig is None:
        return row
    after = [c for c in calls if c["batch"] > trig["batch"]]
    row["prior_lookup"] = any(prior_lookup(c, fn, path) for c in calls
                              if c["batch"] <= trig["batch"] and c["id"] != trig["id"])
    nxt = next((c["batch"] for c in after if c["name"] in MUTATING), None)

    def lookup(c):
        if c["name"] in LOOKUP_NAMED:
            return score._names(fn, c["input"])
        if c["name"] == LOOKUP_AT:
            return str(c["input"].get("path", "")).endswith(path)
        return False

    hits = [c for c in after if lookup(c)]
    first = [c for c in hits if nxt is None or c["batch"] < nxt]
    row["primary"] = bool(first)
    row["secondary"] = bool(hits)
    row["lookup_tool"] = (first or hits or [{"name": None}])[0]["name"]
    return row


def score5_dir(run_dir):
    run_dir = Path(run_dir)
    rec = json.loads((run_dir / "run-record.json").read_text())
    base = {"run_dir": str(run_dir), "task_id": rec["task_id"], "arm": rec["arm"], "rep": rec["rep"],
            "model": rec["model"]}
    transcripts = sorted((run_dir / "config" / "projects").glob(f"*/{rec['session_id']}.jsonl"))
    if len(transcripts) != 1:
        return {**base, "informative": False, "reason": f"{len(transcripts)} transcripts for the session"}
    text = transcripts[0].read_text()
    transcript = [json.loads(l) for l in text.splitlines() if l.strip()]
    calls, results, contexts = score.parse(transcript)
    stream = (run_dir / "stream.jsonl").read_text()
    result = {}
    for line in stream.splitlines():
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if obj.get("type") == "result":
            result = obj
    env = rec["env"]
    row = score_calls(calls, results, contexts, fn=env["HCE_FN"], path=env["HCE_PATH"], arm=rec["arm"],
                      raw_text=text + stream)
    outcome = json.loads((run_dir / "outcome.json").read_text()) if (run_dir / "outcome.json").exists() else {}
    return {**base, **row, "cost_usd": result.get("total_cost_usd"), "num_turns": result.get("num_turns"),
            "run_exit": outcome.get("exit_code"), "timed_out": outcome.get("timed_out")}


# --- analysis --------------------------------------------------------------------------------------------

def hypergeom_pmf(total, succ, draws):
    """P(k) of k A-successes when `draws` of `total` runs are A and `succ` of the total are successes."""
    lo, hi = max(0, draws - (total - succ)), min(draws, succ)
    denom = math.comb(total, draws)
    return {k: math.comb(succ, k) * math.comb(total - succ, draws - k) / denom for k in range(lo, hi + 1)}


def stratified_exact_p(strata):
    """One-sided exact p that arm A has at least the observed successes, given each task's counts.

    strata: {task: (nA, kA, nB, kB)}. The labels A/B are permuted within a task (conditioning on the task's
    run count per arm and its total successes); the per-task counts are independent, so their sum is the
    convolution of the per-task hypergeometrics.
    """
    dist, observed = {0: 1.0}, 0
    for nA, kA, nB, kB in strata.values():
        observed += kA
        pmf = hypergeom_pmf(nA + nB, kA + kB, nA)
        nxt = {}
        for a, pa in dist.items():
            for b, pb in pmf.items():
                nxt[a + b] = nxt.get(a + b, 0.0) + pa * pb
        dist = nxt
    return sum(p for k, p in dist.items() if k >= observed), observed


def summarise(inf):
    """Per-arm counts and the stratified exact test for a list of informative rows."""
    strata, per_arm = {}, {a: [0, 0, 0] for a in ARMS5}  # n, primary, secondary
    for r in inf:
        n, k, s = per_arm[r["arm"]]
        per_arm[r["arm"]] = [n + 1, k + int(r["primary"]), s + int(r["secondary"])]
        t = strata.setdefault(r["task_id"], [0, 0, 0, 0])
        i = 0 if r["arm"] == "A" else 2
        t[i] += 1
        t[i + 1] += int(r["primary"])
    usable = {t: tuple(v) for t, v in strata.items() if v[0] and v[2]}  # tasks with both arms
    p, obs = stratified_exact_p(usable) if usable else (None, None)
    return {"informative": len(inf), "per_arm_n_primary_secondary": per_arm, "per_task_nA_kA_nB_kB": strata,
            "tasks_with_both_arms": len(usable), "stratified_exact_one_sided_p": p, "observed_A_successes": obs}


def analyze(rows):
    """The primary result is over all informative runs. The `no_prior_lookup` stratum (runs where the agent had
    not looked for the callers before the hint arrived) is a secondary result, registered in AM-3."""
    inf = [r for r in rows if r.get("informative")]
    reasons = {}
    for r in rows:
        if not r.get("informative"):
            reasons[r.get("reason")] = reasons.get(r.get("reason"), 0) + 1
    return {"runs": len(rows), "not_informative_by_reason": reasons, **summarise(inf),
            "no_prior_lookup": summarise([r for r in inf if not r.get("prior_lookup")]),
            "prior_lookup_by_arm": {a: sum(1 for r in inf if r["arm"] == a and r.get("prior_lookup"))
                                    for a in ARMS5}}


# --- driver ----------------------------------------------------------------------------------------------

def plan(round_no):
    rng = random.Random(SEED * 10 + round_no)
    tasks = list(FIRING_TASKS)
    rng.shuffle(tasks)
    out = []
    for t in tasks:
        arms = list(ARMS5)
        rng.shuffle(arms)
        out += [(t, a, round_no) for a in arms]
    return out


def read_rows(path):
    return [json.loads(l) for l in path.read_text().splitlines()] if path.exists() else []


def drive(model, todo, rows_path, spent, limit=None):
    done = {(r["task_id"], r["arm"], r["rep"]) for r in read_rows(rows_path)}
    ran = 0
    for n, (task_id, arm, rep) in enumerate(todo, 1):
        if (task_id, arm, rep) in done:
            continue
        if limit is not None and ran >= limit:
            print(f"LIMIT: {ran} runs this invocation; re-run to resume", flush=True)
            return spent, 0
        if spent + float(runner.MAX_BUDGET_USD) > CAP_USD:
            print(f"STOP: spent ${spent:.3f}; the next run could pass the ${CAP_USD} cap", flush=True)
            return spent, 3
        ran += 1
        try:
            run_dir = run_one5(task_id, arm, model, rep, base="runs5-pilot" if rep == 0 else "runs5")
            row = score5_dir(run_dir)
        except Exception as exc:  # recorded, never retried
            row = {"task_id": task_id, "arm": arm, "rep": rep, "informative": False, "reason": f"error {exc!r}"}
        spent += row.get("cost_usd") or 0.0
        with open(rows_path, "a") as f:
            f.write(json.dumps(row) + "\n")
        print(f"[{n:02d}/{len(todo)}] {task_id} {arm}: informative={row.get('informative')} "
              f"reason={row.get('reason')} primary={row.get('primary')} secondary={row.get('secondary')} "
              f"cost={row.get('cost_usd')} spent=${spent:.3f}", flush=True)
    return spent, 0


def spent_total():
    return sum(r.get("cost_usd") or 0.0 for p in ("stage5-pilot.jsonl", "stage5.jsonl")
               for r in read_rows(runner.DATA / p))


def main(argv):
    cmd = argv[1]
    if cmd == "score":
        for d in argv[2:]:
            print(json.dumps(score5_dir(d)))
        return 0
    if cmd == "analyze":
        print(json.dumps(analyze(read_rows(runner.DATA / "stage5.jsonl")), indent=2))
        return 0
    model = argv[2]
    if cmd == "pilot":
        todo = [(PILOT_TASK, a, 0) for a in ARMS5]
        _, code = drive(model, todo, runner.DATA / "stage5-pilot.jsonl", spent_total())
        return code
    if cmd == "round":
        round_no = int(argv[3])
        limit = int(argv[4]) if len(argv) > 4 else None
        rows = read_rows(runner.DATA / "stage5.jsonl")
        spent = spent_total()
        counts = analyze(rows)["per_arm_n_primary_secondary"]
        if not 1 <= round_no <= MAX_ROUND:
            raise SystemExit(f"round must be 1..{MAX_ROUND}")
        if round_no > 1 and min(counts[a][0] for a in ARMS5) >= ENOUGH:
            print(f"ENOUGH: each arm has {ENOUGH} informative runs: {counts}")
            return 0
        costs = [r["cost_usd"] for r in rows if r.get("cost_usd")]
        if round_no > 1 and costs and spent + 10 * (sum(costs) / len(costs)) * 1.3 > CAP_USD:
            print(f"NO ROUND {round_no}: spent ${spent:.2f}, a round would cost about ${10 * sum(costs) / len(costs):.2f}")
            return 0
        _, code = drive(model, plan(round_no), runner.DATA / "stage5.jsonl", spent, limit)
        return code
    raise SystemExit(__doc__)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
