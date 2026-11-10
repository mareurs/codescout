"""Bind a doc-comment-only patch, immutable executable, native observation and full gate."""
import datetime,hashlib,json,os,shutil,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WORKTREE=ROOT/'.worktrees/ecc-resolver-followup-20261110'
SOURCE='src/tools/symbol/call_edges/resolver.rs'
BASE=json.loads((HERE/'baseline.json').read_text())['base_commit']
def git(*args):return subprocess.check_output(['git',*args],cwd=WORKTREE)
assert git('rev-parse','HEAD').decode().strip()==BASE
assert git('diff','--name-only').decode().splitlines()==[SOURCE]
patch=git('diff','--unified=0','--',SOURCE)
for line in patch.decode().splitlines():
    if line.startswith(('+','-')) and not line.startswith(('+++','---')):
        assert line[1:].lstrip().startswith('///'),line
(HERE/'candidate.patch').write_bytes(git('diff','--',SOURCE))
frozen=hashlib.sha256((WORKTREE/SOURCE).read_bytes()).hexdigest()
(HERE/'candidate-source.json').write_text(json.dumps(dict(path=SOURCE,sha256=frozen,base=BASE,doc_comment_lines_only=True),indent=2)+'\n')
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
native=None
for label,cmd in [('build',['cargo','build','--bin','codescout']),('gate',['./scripts/gate.sh','--check'])]:
    assert hashlib.sha256((WORKTREE/SOURCE).read_bytes()).hexdigest()==frozen
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE/(label+'.log')).open('w') as log:
        result=subprocess.run(['./scripts/with-slot.sh','--slot','0',*cmd],cwd=WORKTREE,env=env,stdout=log,stderr=subprocess.STDOUT)
    assert hashlib.sha256((WORKTREE/SOURCE).read_bytes()).hexdigest()==frozen
    record=dict(command=cmd,workspace=str(WORKTREE),head=BASE,source_sha256=frozen,source_unchanged=True,exit_code=result.returncode,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat())
    (HERE/(label+'-result.json')).write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record),flush=True)
    assert result.returncode==0
    if label=='build':
        source=Path('/home/marius/.cache/codescout-gate/slot-0/debug/codescout')
        snapshot=ROOT/'.codescout/ecc-resolver-followup-private-20261110/codescout'
        shutil.copy2(source,snapshot)
        runtime=dict(snapshot=str(snapshot),sha256=hashlib.sha256(snapshot.read_bytes()).hexdigest(),base=BASE,source_sha256=frozen)
        (HERE/'binary.json').write_text(json.dumps(runtime,indent=2)+'\n')
        native_log=(HERE/'native.log').open('w')
        native=subprocess.Popen(['python3',str(HERE/'probe_bfs.py')],cwd=ROOT,env=env,stdout=native_log,stderr=subprocess.STDOUT)
assert native is not None
rc=native.wait()
native_log.close()
(HERE/'native-result.json').write_text(json.dumps(dict(exit_code=rc),indent=2)+'\n')
assert rc==0
print('doc-only candidate full gate and qualified native observation completed',flush=True)
