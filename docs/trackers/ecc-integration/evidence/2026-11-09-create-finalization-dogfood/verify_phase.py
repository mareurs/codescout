import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
HERE=Path(__file__).resolve().parent
WORK=HERE.parents[4]/".worktrees/ecc-navigation-gate"
phase=sys.argv[1]
label=sys.argv[2] if len(sys.argv)>2 else phase
assert phase in ("baseline","red","green","gate","build")
command=["./scripts/gate.sh","--check"] if phase=="gate" else ["cargo","build","--bin","codescout"] if phase=="build" else ["cargo","test","--lib","librarian::tools::create::tests","--","--nocapture"]
source=WORK/"src/librarian/tools/create.rs"
before=hashlib.sha256(source.read_bytes()).hexdigest()
env=dict(os.environ,TMPDIR="/home/marius/.cache/codescout-ecc-tmp",CODESCOUT_GATE_TMPFS_BASE="off")
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (HERE/(label+".log")).open("wb") as log:
    result=subprocess.run(command,cwd=WORK,env=env,stdout=log,stderr=subprocess.STDOUT)
record=dict(phase=phase,command=command,workspace=str(WORK),head=subprocess.check_output(["git","rev-parse","HEAD"],cwd=WORK,text=True).strip(),source_before=before,source_after=hashlib.sha256(source.read_bytes()).hexdigest(),target=env.get("CARGO_TARGET_DIR"),scratch=env["TMPDIR"],started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),exit_code=result.returncode)
(HERE/(label+"-result.json")).write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(record))
raise SystemExit(result.returncode)
