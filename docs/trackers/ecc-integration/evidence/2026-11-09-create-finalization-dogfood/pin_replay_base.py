import json
from pathlib import Path
import subprocess
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
ref="refs/ecc-replay/create-finalization-20261109"
base="038fffb3447b55900e617a251b6472486259e07c"
found=subprocess.run(["git","rev-parse","--verify","--quiet",ref],cwd=ROOT,text=True,capture_output=True)
if found.returncode==1:
    subprocess.run(["git","update-ref",ref,base,"0"*40],cwd=ROOT,check=True)
else:
    assert found.returncode==0 and found.stdout.strip()==base,(found.returncode,found.stdout,found.stderr)
record=dict(ref=ref,commit=base,tree=subprocess.check_output(["git","rev-parse",ref+"^{tree}"],cwd=ROOT,text=True).strip(),scope="Local immutable base pin; no push, branch switch or replay. Never overwrite a conflicting existing ref.")
(HERE/"replay-pin.json").write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(record))
