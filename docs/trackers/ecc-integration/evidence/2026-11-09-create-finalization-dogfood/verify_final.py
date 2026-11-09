"""Recheck the OBSERVED survivor with stronger fixtures, then verify the final source."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
HERE=Path(__file__).resolve().parent
WORK=HERE.parents[4]/".worktrees/ecc-navigation-gate"
source=WORK/"src/librarian/tools/create.rs"
old=(HERE/"review/candidate.rs").read_bytes()
final=source.read_bytes()
assert old.split(b"#[cfg(test)]",1)[0]==final.split(b"#[cfg(test)]",1)[0],"Production changed during test-strengthening"
spec=json.loads((HERE/"review/M7-registration-before-transaction.json").read_text())
assert final.decode().count(spec["find"])==1
(HERE/"final-candidate.patch").write_bytes(subprocess.check_output(["git","diff","HEAD","--","src/librarian/tools/create.rs"],cwd=WORK))
env=dict(os.environ,TMPDIR="/home/marius/.cache/codescout-ecc-tmp",CODESCOUT_GATE_TMPFS_BASE="off")
env.pop("CARGO_TARGET_DIR",None)
records=[]
def run(label,args):
    before=hashlib.sha256(source.read_bytes()).hexdigest()
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE/(label+".log")).open("wb") as log:
        result=subprocess.run(args,cwd=WORK,env=env,stdout=log,stderr=subprocess.STDOUT)
    output=(HERE/(label+".log")).read_text()
    record=dict(label=label,command=args,exit_code=result.returncode,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_before=before,source_after=hashlib.sha256(source.read_bytes()).hexdigest(),summaries=re.findall(r"^test result:.*$",output,re.M),verdicts=re.findall(r"^mutation-probe: (?:KILLED|SURVIVED|INCONCLUSIVE).*$",output,re.M))
    assert record["source_before"]==record["source_after"]
    (HERE/(label+"-result.json")).write_text(json.dumps(record,indent=2)+"\n")
    records.append(record);print(json.dumps(record),flush=True)
    return result.returncode,output
rc,out=run("final-M7",["./scripts/mutation-probe.sh","--strict","--file","src/librarian/tools/create.rs","--find",spec["find"],"--replace",spec["replace"],"--","./scripts/with-slot.sh","--slot","0","cargo","test","--lib","librarian::tools::create::tests::","--","--test-threads=4"])
assert "mutation-probe: KILLED" in out and rc!=0,"Observed survivor must now be caught"
assert "failed_slug_finalization_leaves_no_partial_create" in out and "failed_augmentation_finalization_leaves_no_partial_create" in out
rc,out=run("final-module",["./scripts/with-slot.sh","--slot","0","cargo","test","--lib","librarian::tools::create::tests::","--","--test-threads=4"])
assert rc==0
(HERE/"final-source.json").write_text(json.dumps(dict(sha256=hashlib.sha256(final).hexdigest(),production_prefix_unchanged=True,production_prefix_sha256=hashlib.sha256(final.split(b"#[cfg(test)]",1)[0]).hexdigest(),initial_survivor="review/M7-result.json",rerun="final-M7-result.json",module="final-module-result.json"),indent=2)+"\n")
rc,out=run("final-build-and-gate",["./scripts/with-slot.sh","--slot","0",sys.executable,str(HERE/"verify_final_build.py")])
assert rc==0
