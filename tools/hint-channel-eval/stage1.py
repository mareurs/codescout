"""Stage 1 driver for the hint channel eval (AM-5 of
docs/evals/harness-channel-hint-authority-preregistration.md, id cffe594cbf26fd72).

Order: tasks in tasks.jsonl order; for task i (0-based) the arms are [P, F, A0] shifted left by
i mod 3. Sequential. Halts when cumulative spend exceeds STOP_USD. An invalid run is reported,
never replaced. Appends one scored row per run to $HCE_DATA/stage1.jsonl.

    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/stage1.py <model>
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as runner  # noqa: E402
import score  # noqa: E402

ARMS = ["P", "F", "A0"]
STOP_USD = 20.0


def plan():
    out = []
    for i, task in enumerate(runner.load_tasks()):
        k = i % len(ARMS)
        for arm in ARMS[k:] + ARMS[:k]:
            out.append((task["task_id"], arm))
    return out


def main(model):
    rows_path = runner.DATA / "stage1.jsonl"
    done = set()
    spent = 0.0
    if rows_path.exists():
        for line in rows_path.read_text().splitlines():
            r = json.loads(line)
            done.add((r["task_id"], r["arm"]))
            spent += r.get("cost_usd") or 0.0
    for n, (task_id, arm) in enumerate(plan(), 1):
        if (task_id, arm) in done:
            continue
        if spent > STOP_USD:
            print(f"STOP: spent ${spent:.3f} > ${STOP_USD}", flush=True)
            return 3
        p = subprocess.run([sys.executable, "-I", str(HERE / "run.py"), "run", task_id, arm, model, "1"],
                           capture_output=True, text=True)
        last = (p.stdout.strip().splitlines() or ["{}"])[-1]
        try:
            run_dir = json.loads(last)["run_dir"]
            row = score.score_dir(run_dir)
        except Exception as exc:  # recorded, never retried
            row = {"task_id": task_id, "arm": arm, "valid": False, "error": repr(exc),
                   "stdout_tail": p.stdout[-400:], "stderr_tail": p.stderr[-400:]}
        row.setdefault("task_id", task_id)
        row["run_exit"] = p.returncode
        spent += row.get("cost_usd") or 0.0
        with open(rows_path, "a") as f:
            f.write(json.dumps(row) + "\n")
        print(f"[{n:02d}/30] {task_id} {arm}: valid={row.get('valid')} refs={row.get('refs_before_stop')} "
              f"check={row.get('caller_check_before_stop')} cost={row.get('cost_usd')} spent=${spent:.3f}",
              flush=True)
    print(f"DONE: spent ${spent:.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
