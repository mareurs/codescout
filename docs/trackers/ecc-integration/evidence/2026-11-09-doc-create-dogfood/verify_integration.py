"""Run the post-integration gate with durable output, outside-repository scratch."""
from pathlib import Path
import datetime
import json
import os
import subprocess
import sys

workspace = Path(sys.argv[1]).resolve()
out = Path(__file__).resolve().parent
result_path = out / "integration-gate-result.json"
log_path = out / "integration-gate.log"
if result_path.exists() or log_path.exists():
    raise SystemExit("Refusing to overwrite integration verification evidence")
env = dict(os.environ, TMPDIR="/home/marius/.cache/codescout-ecc-tmp")
before = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace, text=True).strip()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
with log_path.open("wb") as log:
    result = subprocess.run(["./scripts/gate.sh", "--check"], cwd=workspace, env=env, stdout=log, stderr=subprocess.STDOUT)
record = {
    "workspace": str(workspace), "commit_before": before,
    "commit_after": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace, text=True).strip(),
    "command": "./scripts/gate.sh --check", "scratch": env["TMPDIR"],
    "started": started, "finished": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "exit_code": result.returncode, "log": str(log_path),
}
result_path.write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps(record))
raise SystemExit(result.returncode)
