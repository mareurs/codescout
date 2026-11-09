"""Targeted prompt invariants then required full gate over the exact doc correction."""
from pathlib import Path
import datetime,hashlib,json,os,subprocess
HERE=Path(__file__).resolve().parent
WORK=HERE.parents[4]/".worktrees/ecc-navigation-gate"
file=WORK/".codescout/system-prompt.md"
env=dict(os.environ,TMPDIR="/home/marius/.cache/codescout-ecc-tmp",CODESCOUT_GATE_TMPFS_BASE="off")
results=[]
for name,command in [("prompt-tests",["cargo","test","--lib","prompt"]),("gate",["./scripts/gate.sh","--check"])]:
    before=hashlib.sha256(file.read_bytes()).hexdigest()
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE/(name+".log")).open("wb") as log:
        r=subprocess.run(command,cwd=WORK,env=env,stdout=log,stderr=subprocess.STDOUT)
    record={"name":name,"command":command,"workspace":str(WORK),"head":subprocess.check_output(["git","rev-parse","HEAD"],cwd=WORK,text=True).strip(),"source_before":before,"source_after":hashlib.sha256(file.read_bytes()).hexdigest(),"target":env.get("CARGO_TARGET_DIR"),"scratch":env["TMPDIR"],"CODESCOUT_GATE_TMPFS_BASE":"off","started":started,"finished":datetime.datetime.now(datetime.timezone.utc).isoformat(),"exit_code":r.returncode}
    results.append(record);(HERE/(name+"-result.json")).write_text(json.dumps(record,indent=2)+"\n")
    print(json.dumps(record),flush=True)
    if r.returncode:raise SystemExit(r.returncode)
