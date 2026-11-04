"""Stage 3 driver for the hint channel eval (AM-10 of
docs/evals/harness-channel-hint-authority-preregistration.md, id cffe594cbf26fd72).

Arms A1, A2, A3 (right hint) and W1, W2, W3 (the same with a decoy in the hint's lookup), all with
`--setting-sources project,local` so the project CLAUDE.md is loaded, and every arm carrying its
task's decoy (decoys.json) so the A arms measure the decoy floor. 150 runs: reps 1 and 2 of all 10
tasks and rep 3 of the first five; blocks (rep, task); within block j the arms rotated left by
j mod 6. Sequential; halts past STOP_USD; invalid runs reported, never replaced. Rows go to
$HCE_DATA/stage3.jsonl; re-running resumes.

    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/stage3.py <model> [limit]
"""
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as runner  # noqa: E402
import score  # noqa: E402

ARMS = ["A1", "A2", "A3", "W1", "W2", "W3"]
STOP_USD = 27.0  # AM-10: raised from 20 by the operator after block 1 measured ~$0.17 per run
DECOYS = runner.REPO / "docs/evals/data/2026-11-04-harness-channel-hints/decoys.json"
ENV = {"HCE_SETTING_SOURCES": "project,local", "HCE_DECOYS": str(DECOYS)}


def plan():
    tasks = runner.load_tasks()
    blocks = [(rep, t["task_id"]) for rep in (1, 2, 3) for i, t in enumerate(tasks) if rep < 3 or i < 5]
    out = []
    for j, (rep, task_id) in enumerate(blocks):
        k = j % len(ARMS)
        for arm in ARMS[k:] + ARMS[:k]:
            out.append((task_id, arm, rep))
    return out


def main(model, limit=None):
    rows_path = runner.DATA / "stage3.jsonl"
    done, spent = set(), 0.0
    if rows_path.exists():
        for line in rows_path.read_text().splitlines():
            r = json.loads(line)
            done.add((r["task_id"], r["arm"], r["rep"]))
            spent += r.get("cost_usd") or 0.0
    todo, ran = plan(), 0
    for n, (task_id, arm, rep) in enumerate(todo, 1):
        if (task_id, arm, rep) in done:
            continue
        if limit is not None and ran >= limit:
            print(f"LIMIT: {ran} runs this invocation; re-run to resume", flush=True)
            return 0
        if spent > STOP_USD:
            print(f"STOP: spent ${spent:.3f} > ${STOP_USD}", flush=True)
            return 3
        ran += 1
        p = subprocess.run([sys.executable, "-I", str(HERE / "run.py"), "run", task_id, arm, model, str(rep)],
                           capture_output=True, text=True, env={**os.environ, **ENV})
        last = (p.stdout.strip().splitlines() or ["{}"])[-1]
        try:
            row = score.score_dir(json.loads(last)["run_dir"])
        except Exception as exc:  # recorded, never retried
            row = {"task_id": task_id, "arm": arm, "rep": rep, "valid": False, "error": repr(exc),
                   "stdout_tail": p.stdout[-400:], "stderr_tail": p.stderr[-400:]}
        row.setdefault("task_id", task_id)
        row.setdefault("rep", rep)
        row["run_exit"] = p.returncode
        spent += row.get("cost_usd") or 0.0
        with open(rows_path, "a") as f:
            f.write(json.dumps(row) + "\n")
        print(f"[{n:03d}/{len(todo)}] {task_id} {arm} r{rep}: valid={row.get('valid')} "
              f"late={row.get('late_lookup')} decoy={row.get('decoy_late_lookup')} "
              f"src={row.get('setting_sources')} cost={row.get('cost_usd')} spent=${spent:.3f}", flush=True)
    print(f"DONE: spent ${spent:.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else None))
