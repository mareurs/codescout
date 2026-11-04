"""Stage 3 decoys by the rule fixed in AM-10, before any Stage 3 data.

For task i (0-based, tasks.jsonl order), try offsets k = 5, 6, ..., 14: the candidate is task
(i + k) mod 10's target fn and path. Accept the first candidate whose path differs from task i's
path, whose file exists in task i's tree, and whose `fn <name>` is defined exactly once across all
*.rs files of that tree. Usage: python3 -I make_decoys.py <tasks.jsonl> <hce_data> <out.json>"""
import json, re, sys
from pathlib import Path

tasks = [json.loads(l) for l in open(sys.argv[1])]
data, out = Path(sys.argv[2]), Path(sys.argv[3])
result = {}
for i, t in enumerate(tasks):
    tree = data / "tasks" / t["task_id"] / "tree"
    sources = {p: p.read_text(errors="ignore") for p in tree.rglob("*.rs")}
    for k in range(5, 15):
        c = tasks[(i + k) % len(tasks)]
        if c["task_id"] == t["task_id"] or c["path"] == t["path"]:
            continue
        pat = re.compile(r"\bfn\s+" + re.escape(c["fn"]) + r"\b")
        f = tree / c["path"]
        if f.exists() and pat.search(sources.get(f, "")) and sum(len(pat.findall(s)) for s in sources.values()) == 1:
            result[t["task_id"]] = {"decoy_fn": c["fn"], "decoy_path": c["path"], "offset": k}
            break
    else:
        raise SystemExit(f"no decoy for {t['task_id']}")
out.write_text(json.dumps(result, indent=2) + "\n")
for tid, d in result.items():
    print(tid, "->", d["decoy_fn"], d["decoy_path"], "offset", d["offset"])
