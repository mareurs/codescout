"""Audit the Stage 1 rerun: availability, arrival, and for each references/call_graph call on the
target its batch relative to the trigger and the stop, and whether its result is an error.
Usage: python3 -I audit.py <hce_data>"""
import json, sys
from pathlib import Path

sys.path.insert(0, "/home/marius/work/claude/codescout/tools/hint-channel-eval")
import score

data = Path(sys.argv[1])
rows = [json.loads(l) for l in (data / "stage1.jsonl").read_text().splitlines()]
print("runs", len(rows), "| valid", sum(r["valid"] for r in rows),
      "| lookup_available", sum(r.get("lookup_available", False) for r in rows),
      "| arrival_ok", sum(r["arrival_ok"] for r in rows))
for r in rows:
    d = Path(r["run_dir"])
    rec = json.loads((d / "run-record.json").read_text())
    t = [json.loads(l) for l in next((d / "config/projects").glob(f"*/{rec['session_id']}.jsonl")).read_text().splitlines()]
    ev = [json.loads(l) for l in (d / "state/events.jsonl").read_text().splitlines()]
    calls, results, _ = score.parse(t)
    b = {c["id"]: c["batch"] for c in calls}
    tb = b[next(e for e in ev if e["event"] == "trigger")["tool_use_id"]]
    sb = b[next(e for e in ev if e["event"] == "stop")["tool_use_id"]]
    looks = [c for c in calls if c["name"] in score.LOOKUP_TOOLS and score._names(r["fn"], c["input"]) and c["batch"] < sb]
    desc = []
    for c in looks:
        res = " ".join(results.get(c["id"], []))
        err = "ERROR" if ("error" in res.lower()[:120] or not res.strip()) else "ok"
        where = "before-trigger" if c["batch"] < tb else ("same-batch-as-trigger" if c["batch"] == tb else "after-trigger")
        desc.append(f"{c['name'].split('__')[-1]}@{where}:{err}:{res[:60]!r}")
    print(f"{r['arm']:2} {r['task_id'][:28]:28} refs={str(r['refs_before_stop'])[0]} trig_b={tb} stop_b={sb} {desc}")
