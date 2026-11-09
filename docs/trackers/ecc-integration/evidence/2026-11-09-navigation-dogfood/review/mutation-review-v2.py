import hashlib, json, os, pathlib, subprocess, time
ROOT = pathlib.Path("/home/marius/work/claude/codescout/.worktrees/ecc-navigation-truth")
OUT = pathlib.Path(__file__).resolve().parent
SOURCE = ROOT / "src/tools/symbol/call_graph/mod.rs"
mutations = [
("m1-suppress-failure-record", 'self.analysis_failures\n                    .lock()\n                    .unwrap()\n                    .insert(format!("\x60{symbol}\x60 direction={direction_label}: {e}"));', 'let _ = direction_label;'),
("m2-remove-outer-attachment", "        attach_analysis_failures(&mut out, &analysis_failures);", ""),
("m3-discard-existing-warning", 'Some(existing) => format!("{existing}\\n{warning}"),', 'Some(_existing) => warning,'),
("m4-warn-on-trustworthy-leaf", "    if failures.is_empty() {\n        return;\n    }\n    const SHOWN", "    const SHOWN"),
("m5-remove-detail-bound", "    const SHOWN: usize = 5;", "    const SHOWN: usize = usize::MAX;"),
]
initial = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
results = []
env = dict(os.environ, TMPDIR="/home/marius/.cache/codescout-ecc-tmp", CODEX_SESSION_ID="01a1bf68-e650-7f20-911e-b53f86597c27")
for name, find, replace in mutations:
    command = ["scripts/mutation-probe.sh", "--strict", "--file", str(SOURCE.relative_to(ROOT)), "--find", find, "--replace", replace, "--", "cargo", "test", "--lib", "tools::symbol::call_graph::tests"]
    started = time.time()
    log = OUT / (name + ".log")
    with log.open("w") as handle:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=handle, stderr=subprocess.STDOUT)
    source_hash = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    record = dict(name=name, find=find, replace=replace, command=command, returncode=result.returncode, elapsed_seconds=round(time.time()-started, 2), source_hash=source_hash, source_unchanged=(source_hash == initial), log=str(log))
    results.append(record)
    (OUT / "mutation-review-v2-results.json").write_text(json.dumps(dict(initial_source_hash=initial, results=results), indent=2) + "\n")
    print(json.dumps(record), flush=True)
    if source_hash != initial:
        raise SystemExit("Source changed during review")
