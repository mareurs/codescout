import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
subprocess.run([sys.executable,str(HERE/"verify_phase.py"),"build"],check=True)
binary=Path(os.environ["CARGO_TARGET_DIR"])/"debug/codescout"
private=ROOT/".codescout/ecc-create-finalization-private-20261109"
private.mkdir(exist_ok=True);(private/".gitignore").write_text("*\n")
before=hashlib.sha256(binary.read_bytes()).hexdigest()
snapshot=private/"candidate-codescout"
shutil.copy2(binary,snapshot)
after=hashlib.sha256(binary.read_bytes()).hexdigest()
assert before==after==hashlib.sha256(snapshot.read_bytes()).hexdigest()
(HERE/"binary.json").write_text(json.dumps(dict(source=str(binary),snapshot=str(snapshot),sha256=before,source_sha256=json.loads((HERE/"build-result.json").read_text())["source_after"]),indent=2)+"\n")
subprocess.run([sys.executable,str(HERE/"verify_phase.py"),"gate"],check=True)
