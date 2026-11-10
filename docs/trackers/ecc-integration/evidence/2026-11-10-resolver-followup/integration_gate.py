"""Verify one exact integrated commit in an isolated retained tree, regardless of later peer commits."""
import datetime,hashlib,json,os,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
record=json.loads((HERE/'integration.json').read_text())
identity=record['integration']
WT=ROOT/'.worktrees/ecc-resolver-followup-integration-20261110'
def git(*args,cwd=ROOT):return subprocess.check_output(['git',*args],cwd=cwd)
if not WT.exists():
    subprocess.run(['git','worktree','add','--detach',str(WT),identity['commit']],cwd=ROOT,check=True)
assert git('rev-parse','HEAD',cwd=WT).decode().strip()==identity['commit']
assert git('rev-parse','HEAD^{tree}',cwd=WT).decode().strip()==identity['tree']
assert not git('status','--porcelain','--untracked-files=no',cwd=WT)
def check():
    for path,expected in record['source_sha256'].items():
        assert hashlib.sha256((WT/path).read_bytes()).hexdigest()==expected
check()
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
command=['./scripts/with-slot.sh','--slot','0','./scripts/gate.sh','--check']
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (HERE/'integration-gate.log').open('w') as log:
    result=subprocess.run(command,cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
check()
assert not git('diff','HEAD','--',cwd=WT)
out=dict(commit=identity['commit'],tree=identity['tree'],command=command,workspace=str(WT),exit_code=result.returncode,source_unchanged=True,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),scope='Exact retained integrated commit, not later experiments HEAD; peer dirty files excluded.')
(HERE/'integration-gate-result.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out),flush=True)
assert result.returncode==0
