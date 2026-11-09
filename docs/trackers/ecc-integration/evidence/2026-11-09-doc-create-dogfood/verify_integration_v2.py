"""Post-integration gate with the gate's optional /tmp override disabled."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
import sys

workspace = Path(sys.argv[1]).resolve()
out = Path(__file__).resolve().parent
result_path = out / "integration-gate-off-result.json"
log_path = out / "integration-gate-off.log"
if result_path.exists() or log_path.exists():
    raise SystemExit("Refusing to overwrite integration evidence")
env = dict(os.environ, TMPDIR="/home/marius/.cache/codescout-ecc-tmp", CODESCOUT_GATE_TMPFS_BASE="off")
before = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace, text=True).strip()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
with log_path.open("wb") as log:
    result = subprocess.run(["./scripts/gate.sh", "--check"], cwd=workspace, env=env, stdout=log, stderr=subprocess.STDOUT)
record = {
    "workspace": str(workspace), "commit_before": before,
    "commit_after": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace, text=True).strip(),
    "command": "./scripts/gate.sh --check", "scratch": env["TMPDIR"],
    "CODESCOUT_GATE_TMPFS_BASE": "off", "target": env.get("CARGO_TARGET_DIR"),
    "started": started, "finished": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "exit_code": result.returncode, "log": str(log_path),
}
result_path.write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps(record), flush=True)
if result.returncode == 0:
    binary = Path(env["CARGO_TARGET_DIR"]) / "debug/codescout"
    with binary.open("rb") as f:
        fingerprint = hashlib.file_digest(f, "sha256").hexdigest()
    navigation = out.parent / "2026-11-09-navigation-dogfood"
    (navigation / "baseline-binary.json").write_text(json.dumps({"path": str(binary), "sha256": fingerprint, "commit": before}, indent=2) + "\n")
    with (navigation / "baseline-probe.log").open("wb") as log:
        probe = subprocess.run([sys.executable, str(navigation / "probe.py"), "--binary", str(binary), "--output", str(navigation / "baseline-mcp.json")], cwd=workspace, env=env, stdout=log, stderr=subprocess.STDOUT)
    (navigation / "baseline-probe-result.json").write_text(json.dumps({"exit_code": probe.returncode}, indent=2) + "\n")
    print(json.dumps({"baseline_navigation_probe_exit": probe.returncode}), flush=True)
raise SystemExit(result.returncode)
