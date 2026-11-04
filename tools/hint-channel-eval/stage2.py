"""Stage 2 driver for the hint channel eval (AM-8 of
docs/evals/harness-channel-hint-authority-preregistration.md, id cffe594cbf26fd72).

175 runs: reps 1 and 2 of all 10 tasks, rep 3 of the first five; blocks (rep, task) in that
order; within block j the arms [F, A0, A1, A2, A3, P, L] rotated left by j mod 7. Sequential.
Halts when cumulative spend exceeds STOP_USD. An invalid run is reported, never replaced.
Appends one scored row per run to $HCE_DATA/stage2.jsonl; re-running resumes after the last row.

    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/stage2.py <model>
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as runner  # noqa: E402
import score  # noqa: E402

ARMS = ["F", "A0", "A1", "A2", "A3", "P", "L"]
STOP_USD = 20.0


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
    rows_path = runner.DATA / "stage2.jsonl"
    done, spent = set(), 0.0
    if rows_path.exists():
        for line in rows_path.read_text().splitlines():
            r = json.loads(line)
            done.add((r["task_id"], r["arm"], r["rep"]))
            spent += r.get("cost_usd") or 0.0
    todo = plan()
    ran = 0
    for n, (task_id, arm, rep) in enumerate(todo, 1):
        if (task_id, arm, rep) in done:
            continue
        if limit is not None and ran >= limit:
            print(f"LIMIT: {ran} runs this invocation; re-run to resume", flush=True)
            return 0
        ran += 1
        if spent > STOP_USD:
            print(f"STOP: spent ${spent:.3f} > ${STOP_USD}", flush=True)
            return 3
        p = subprocess.run([sys.executable, "-I", str(HERE / "run.py"), "run", task_id, arm, model, str(rep)],
                           capture_output=True, text=True)
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
              f"pre={row.get('pre_hint_lookup')} late={row.get('late_lookup')} "
              f"cost={row.get('cost_usd')} spent=${spent:.3f}", flush=True)
    print(f"DONE: spent ${spent:.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else None))
