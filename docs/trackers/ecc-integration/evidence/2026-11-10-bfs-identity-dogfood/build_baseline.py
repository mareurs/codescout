"""Build and pin the exact baseline executable before source edits."""
import hashlib,json,os,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-identity-20261110'
frozen=json.loads((HERE/'baseline.json').read_text())
def check():
    for row in frozen['inputs']:
        assert hashlib.sha256((WT/row['path']).read_bytes()).hexdigest()==row['sha256']
check()
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
with (HERE/'baseline-build.log').open('w') as log:
    result=subprocess.run(['./scripts/with-slot.sh','--slot','0','cargo','build','--no-default-features','--bin','codescout'],cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
check()
assert result.returncode==0
# with-slot records the bound target under the worktree target symlink.
target=WT/'target/debug/codescout'
assert target.is_file(),target
snapshot=ROOT/'.codescout/ecc-bfs-identity-private-20261110/baseline-codescout'
data=target.read_bytes()
snapshot.write_bytes(data)
snapshot.chmod(0o700)
record=dict(base=frozen['base_commit'],snapshot=str(snapshot),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data),source_unchanged=True,exit_code=result.returncode)
(HERE/'baseline-binary.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
