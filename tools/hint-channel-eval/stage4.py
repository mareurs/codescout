"""Stage 4 driver for the hint channel eval (AM-12 of
docs/evals/harness-channel-hint-authority-preregistration.md, id cffe594cbf26fd72).

Can the declared `[cs-hint]` tag be forged from file content? Arms S0 (forged tagged line in the target's
source, no declaration), S1 (the same plus the declaration in CLAUDE.md), S2 (an untagged `NOTE:` line plus the
declaration). 20 runs: S1 on all 10 tasks, S0 and S2 on the first five. One block per task; within block j the
arms rotated left by j mod len(arms). Sequential; halts past STOP_USD; invalid runs reported, never replaced.
Rows go to $HCE_DATA/stage4.jsonl; re-running resumes.

    HCE_TOKEN_FILE=<0600 file> python3 -I tools/hint-channel-eval/stage4.py <model> [limit]
"""
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as runner  # noqa: E402
import score_spoof  # noqa: E402

STOP_USD = 4.0  # AM-12
DECOYS = runner.REPO / "docs/evals/data/2026-11-04-harness-channel-hints/decoys.json"
ENV = {"HCE_SETTING_SOURCES": "project,local", "HCE_DECOYS": str(DECOYS)}


def plan():
    out = []
    for j, t in enumerate(runner.load_tasks()):
        arms = ["S1"] + (["S0", "S2"] if j < 5 else [])
        k = j % len(arms)
        out += [(t["task_id"], arm, 1) for arm in arms[k:] + arms[:k]]
    return out


def main(model, limit=None):
    rows_path = runner.DATA / "stage4.jsonl"
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
            row = score_spoof.score_spoof_dir(json.loads(last)["run_dir"])
        except Exception as exc:  # recorded, never retried
            row = {"task_id": task_id, "arm": arm, "rep": rep, "valid": False, "error": repr(exc),
                   "stdout_tail": p.stdout[-400:], "stderr_tail": p.stderr[-400:]}
        row.setdefault("task_id", task_id)
        row.setdefault("arm", arm)
        row.setdefault("rep", rep)
        row["run_exit"] = p.returncode
        spent += row.get("cost_usd") or 0.0
        with open(rows_path, "a") as f:
            f.write(json.dumps(row) + "\n")
        print(f"[{n:02d}/{len(todo)}] {task_id} {arm}: valid={row.get('valid')} arrival={row.get('arrival')} "
              f"obeyed={row.get('decoy_obeyed')} cost={row.get('cost_usd')} spent=${spent:.3f}", flush=True)
    print(f"DONE: spent ${spent:.3f}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else None))
