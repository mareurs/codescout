import hashlib, json, os, pathlib, subprocess, time
ROOT = pathlib.Path("/home/marius/work/claude/codescout/.worktrees/ecc-navigation-truth")
OUT = pathlib.Path(__file__).resolve().parent
SOURCE = ROOT / "src/tools/symbol/call_graph/mod.rs"
initial = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
result_file = OUT / "mutation-review-v3-results.json"
with (OUT / "baseline-v3.log").open("w") as handle:
    baseline = subprocess.run(["cargo", "test", "--lib", "tools::symbol::call_graph::tests"], cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT)
data = dict(initial_source_hash=initial, baseline_returncode=baseline.returncode, results=[])
result_file.write_text(json.dumps(data, indent=2)+"\n")
if baseline.returncode:
    raise SystemExit("Clean baseline failed; no mutations applied")
old = json.loads((OUT / "mutation-review-v2-results.json").read_text())
bound = json.loads((OUT / "mutation-bound-v2-results.json").read_text())
cases = [next(r for r in old["results"] if r["name"] == "m3-discard-existing-warning"), bound]
for case in cases:
    name = case["name"]+"-v3"
    started = time.time()
    log = OUT / (name+".log")
    with log.open("w") as handle:
        result = subprocess.run(case["command"], cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT)
    final = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    record = dict(name=name, command=case["command"], returncode=result.returncode, elapsed_seconds=round(time.time()-started, 2), source_hash=final, source_unchanged=initial==final, log=str(log))
    data["results"].append(record)
    result_file.write_text(json.dumps(data, indent=2)+"\n")
    print(json.dumps(record), flush=True)
    if initial != final:
        raise SystemExit("Source changed during review")
