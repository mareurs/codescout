import hashlib, json, os, pathlib, subprocess, time
ROOT = pathlib.Path("/home/marius/work/claude/codescout/.worktrees/ecc-navigation-truth")
OUT = pathlib.Path(__file__).resolve().parent
SOURCE = ROOT / "src/tools/symbol/call_graph/mod.rs"
initial = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
find = "    let details = failures\n        .iter()\n        .take(SHOWN)"
replace = "    let details = failures\n        .iter()"
command = ["scripts/mutation-probe.sh", "--strict", "--file", str(SOURCE.relative_to(ROOT)), "--find", find, "--replace", replace, "--", "cargo", "test", "--lib", "tools::symbol::call_graph::tests"]
with (OUT / "m6-remove-detail-bound-unique.log").open("w") as handle:
    result = subprocess.run(command, cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT)
final = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
record = dict(name="m6-remove-detail-bound-unique", find=find, replace=replace, command=command, returncode=result.returncode, initial_source_hash=initial, final_source_hash=final, source_unchanged=initial==final)
(OUT / "mutation-bound-v2-results.json").write_text(json.dumps(record, indent=2)+"\n")
print(json.dumps(record))
