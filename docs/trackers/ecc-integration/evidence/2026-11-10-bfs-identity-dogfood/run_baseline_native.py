"""Run the retained seven-case native qualification on this cycle's frozen executable."""
import hashlib,json,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
source=HERE.parent/'2026-11-10-resolver-followup/probe_bfs.py'
text=source.read_text()
text=text.replace("runtime=json.loads((HERE/'binary.json').read_text())","runtime=json.loads((HERE/'baseline-binary.json').read_text())")
text=text.replace("'.codescout/ecc-resolver-followup-private-20261110'","'.codescout/ecc-bfs-identity-private-20261110'")
text=text.replace("project('bfs-unique',True)","project('baseline-bfs-unique',True)").replace("project('bfs-duplicate',False)","project('baseline-bfs-duplicate',False)")
# Keep every baseline result under a separate directory so candidate output never overwrites it.
target=HERE/'baseline-native'
target.mkdir()
# Probe's root math and helper location were originally tied to one shallower directory.
text=text.replace("ROOT=HERE.parents[4]","ROOT=HERE.parents[5]")
text=text.replace("HERE.parent/'2026-11-09-doc-create-dogfood/transport-helper.py.txt'","HERE.parent.parent/'2026-11-09-doc-create-dogfood/transport-helper.py.txt'")
text=text.replace("(HERE/'baseline-binary.json')","(HERE.parent/'baseline-binary.json')")
script=target/'probe.py'
script.write_text(text)
with (target/'native.log').open('w') as log:
    result=subprocess.run([sys.executable,str(script)],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
record=dict(exit_code=result.returncode,script_sha256=hashlib.sha256(script.read_bytes()).hexdigest(),original_script_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),scope='Seven-case retained fixture freshly executed on this cycle baseline binary; no repair replay.')
(target/'process-result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
assert result.returncode==0
