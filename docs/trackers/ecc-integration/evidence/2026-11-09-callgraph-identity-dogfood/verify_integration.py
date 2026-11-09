"""Full gate against the exact integrated committed tree, isolated from peer dirty docs."""
import datetime, hashlib, json, os, subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WORKTREE=ROOT/'.worktrees/ecc-callgraph-identity-integration-gate'
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
def git(*args,cwd=WORKTREE):
    return subprocess.check_output(['git',*args],cwd=cwd,text=True).strip()
head=git('rev-parse','HEAD')
tree=git('rev-parse','HEAD^{tree}')
assert head.startswith('4776e44bd')
assert git('status','--porcelain')==''
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (HERE/'integration-gate.log').open('w') as log:
    run=subprocess.run(['./scripts/with-slot.sh','--slot','0','./scripts/gate.sh','--check'],cwd=WORKTREE,env=env,stdout=log,stderr=subprocess.STDOUT)
record=dict(head=head,tree=tree,workspace=str(WORKTREE),exit_code=run.returncode,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),head_unchanged=git('rev-parse','HEAD')==head,source_clean=git('status','--porcelain')=='')
(HERE/'integration-gate-result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
assert run.returncode==0 and record['head_unchanged'] and record['source_clean']
