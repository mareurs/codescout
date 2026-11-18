"""Read-only campaign progress. Private outputs are counted, never opened."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess

def observe(root, pid=None):
    root = root.resolve(strict=True)
    assert root.is_dir() and (root / "registration.json").is_file()
    rows = [json.loads(path.read_bytes()) for path in sorted(root.glob("attempts/*/attempt.json"))]
    report = {
        "schema": "effort-campaign-observation-v1",
        "root": str(root),
        "producer_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "closed_attempt_receipts": len(rows),
        "attempts": [{key: row[key] for key in
                     ("attempt_index", "arm", "workflow_state", "measurement_invalid", "cost")} for row in rows],
        "phase_intents": [str(path.relative_to(root)) for path in sorted(root.glob("attempts/*/*/intent.json"))],
        "private_entry_markers": len(list(root.glob("grades/*/private-entry-started.json"))),
        "private_verdict_files": len(list(root.glob("grades/*/verdict.json"))),
        "actor_batch_closed": (root / "actor-batch-closed.json").is_file(),
        "result_written": (root / "result.json").is_file(),
        "scope": "Exact on-disk receipt/intent/entry-marker/verdict-file counts; private verdict contents unopened.",
    }
    if pid is not None:
        probe = subprocess.run(["ps", "-p", str(pid), "-o", "pid,ppid,etime,pcpu,stat,comm"],
                               capture_output=True, text=True, check=False)
        report["process"] = {"pid": pid, "probe_exit": probe.returncode, "observed": probe.stdout}
    return report

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--pid", type=int)
    args = parser.parse_args()
    print(json.dumps(observe(args.root, args.pid)), flush=True)

if __name__ == "__main__":
    main()
