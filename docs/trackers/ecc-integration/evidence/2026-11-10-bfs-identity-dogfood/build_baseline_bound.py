"""Build and snapshot under the same source-bound target lease."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-identity-20261110'
frozen=json.loads((HERE/'baseline.json').read_text())
def check():
    for row in frozen['inputs']:
        assert hashlib.sha256((WT/row['path']).read_bytes()).hexdigest()==row['sha256']
check()
if '--leased' not in sys.argv:
    env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
    env.pop('CARGO_TARGET_DIR',None)
    result=subprocess.run(['./scripts/with-slot.sh','--slot','0',sys.executable,str(Path(__file__).resolve()),'--leased'],cwd=WT,env=env)
    assert result.returncode==0
    sys.exit(0)
with (HERE/'baseline-build-bound.log').open('w') as log:
    result=subprocess.run(['cargo','build','--no-default-features','--bin','codescout'],cwd=WT,stdout=log,stderr=subprocess.STDOUT)
check()
assert result.returncode==0
target=Path(os.environ['CARGO_TARGET_DIR'])/'debug/codescout'
data=target.read_bytes()
snapshot=ROOT/'.codescout/ecc-bfs-identity-private-20261110/baseline-codescout'
snapshot.write_bytes(data)
snapshot.chmod(0o700)
record=dict(base=frozen['base_commit'],snapshot=str(snapshot),leased_target=str(target),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data),source_unchanged=True,exit_code=result.returncode,snapshot_copied_while_lease_held=True)
(HERE/'baseline-binary.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
