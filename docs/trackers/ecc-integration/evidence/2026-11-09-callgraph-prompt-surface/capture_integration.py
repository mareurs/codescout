"""Capture exact source/integration identity and completed verification job records."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
source="877003e9907a8a104d890e9e3f0b1b0d61748899"
integration="6586622a30f0d790db17b1bf234aa7962837f510"
def git(*args):return subprocess.check_output(["git",*args],cwd=ROOT)
def patch_id(commit):
    return subprocess.check_output(["git","patch-id","--stable"],input=git("show","--format=",commit)).decode().split()[0]
assert patch_id(source)==patch_id(integration)
assert git("show",source+":.codescout/system-prompt.md")==git("show",integration+":.codescout/system-prompt.md")
jobs=[]
for handle in ["00000066","00000067","0000006a","00000070"]:
    path=Path("/tmp/codescout-bg")/f"1520813-b902d740-bg_{handle}.json"
    value=json.loads(path.read_text())
    shutil.copyfile(path,HERE/f"job-{handle}.json")
    jobs.append(dict(handle="@bg_"+handle,state=value.get("state"),exit_code=value.get("exit_code")))
foreign=git("diff","--cached","--name-only").decode().splitlines()
record=dict(source_commit=source,integration_commit=integration,stable_patch_id=patch_id(source),same_patch_id=True,same_stored_prompt_blob=True,integration_tree=git("rev-parse",integration+"^{tree}").decode().strip(),source_file_sha256=hashlib.sha256(git("show",integration+":.codescout/system-prompt.md")).hexdigest(),jobs=jobs,foreign_staged_paths_at_evidence_capture=foreign,note="Integration commits exactly one stored-prompt line. Existing foreign staged path names remained present; no byte-identity claim without before/after blob snapshots. Frozen integration gate verifies this exact committed tree, excluding unrelated dirty shared-checkout files.")
(HERE/"integration.json").write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(record))
