"""Verify the frozen integrated commit; retain the earlier source gate separately."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
HERE = Path(__file__).resolve().parent
WORK = HERE.parents[4] / ".worktrees/ecc-navigation-gate"
HEAD = "6586622a30f0d790db17b1bf234aa7962837f510"
assert subprocess.check_output(["git","rev-parse","HEAD"],cwd=WORK,text=True).strip() == HEAD
env = dict(os.environ, TMPDIR="/home/marius/.cache/codescout-ecc-tmp", CODESCOUT_GATE_TMPFS_BASE="off")
file = WORK / ".codescout/system-prompt.md"
before = hashlib.sha256(file.read_bytes()).hexdigest()
assert before == "cc80985e14fbfa7ee6bd3716748b3832e6877cd5d5e7d15afea91dc153e95e29"
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
command = ["./scripts/gate.sh","--check"]
with (HERE/"integration-gate.log").open("wb") as log:
    result = subprocess.run(command,cwd=WORK,env=env,stdout=log,stderr=subprocess.STDOUT)
record = dict(command=command,head=HEAD,tree=subprocess.check_output(["git","rev-parse","HEAD^{tree}"],cwd=WORK,text=True).strip(),source_before=before,source_after=hashlib.sha256(file.read_bytes()).hexdigest(),workspace=str(WORK),target=env.get("CARGO_TARGET_DIR"),scratch=env["TMPDIR"],CODESCOUT_GATE_TMPFS_BASE="off",started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),exit_code=result.returncode)
(HERE/"integration-gate-result.json").write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(record))
raise SystemExit(result.returncode)
