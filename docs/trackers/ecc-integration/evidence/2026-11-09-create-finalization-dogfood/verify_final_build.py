import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
subprocess.run([sys.executable,str(HERE/"verify_phase.py"),"build","final-build"],check=True)
binary=Path(os.environ["CARGO_TARGET_DIR"])/"debug/codescout"
private=ROOT/".codescout/ecc-create-finalization-private-20261109"
before=hashlib.sha256(binary.read_bytes()).hexdigest()
snapshot=private/"final-codescout"
shutil.copy2(binary,snapshot)
assert before==hashlib.sha256(binary.read_bytes()).hexdigest()==hashlib.sha256(snapshot.read_bytes()).hexdigest()
(HERE/"final-binary.json").write_text(json.dumps(dict(source=str(binary),snapshot=str(snapshot),sha256=before,source_sha256=json.loads((HERE/"final-build-result.json").read_text())["source_after"]),indent=2)+"\n")
subprocess.run([sys.executable,str(HERE/"verify_phase.py"),"gate","final-gate"],check=True)
