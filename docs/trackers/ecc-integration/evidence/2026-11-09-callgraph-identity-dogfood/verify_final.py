"""Verify final test-strengthened source and bind native evidence by binary identity."""
import datetime, hashlib, json, os, shutil, subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WORKTREE=ROOT/'.worktrees/ecc-callgraph-identity'
frozen=json.loads((HERE/'final-source.json').read_text())['files']
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
def check():
    assert all(hashlib.sha256((WORKTREE/p).read_bytes()).hexdigest()==h for p,h in frozen.items())
for label,command in [('final-build',['cargo','build','--bin','codescout']),('final-gate',['./scripts/gate.sh','--check'])]:
    check()
    started=datetime.datetime.now(datetime.timezone.utc).isoformat()
    with (HERE/(label+'.log')).open('w') as log:
        result=subprocess.run(['./scripts/with-slot.sh','--slot','0',*command],cwd=WORKTREE,env=env,stdout=log,stderr=subprocess.STDOUT)
    check()
    record=dict(command=command,workspace=str(WORKTREE),head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=WORKTREE,text=True).strip(),source_hashes=frozen,source_unchanged=True,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),exit_code=result.returncode)
    (HERE/(label+'-result.json')).write_text(json.dumps(record,indent=2)+'\n')
    print(json.dumps(record),flush=True)
    assert result.returncode==0
    if label=='final-build':
        binary=Path('/home/marius/.cache/codescout-gate/slot-0/debug/codescout')
        snapshot=ROOT/'.codescout/ecc-callgraph-identity-private-20261109/final-codescout'
        shutil.copy2(binary,snapshot)
        digest=hashlib.sha256(snapshot.read_bytes()).hexdigest()
        candidate=json.loads((HERE/'candidate-binary.json').read_text())
        same=digest==candidate['sha256']
        (HERE/'final-binary.json').write_text(json.dumps(dict(sha256=digest,snapshot=str(snapshot),source_hashes=frozen,identical_to_candidate=same,native_evidence='candidate-mcp-summary.json' if same else 'final-mcp-summary.json'),indent=2)+'\n')
        if not same:
            with (HERE/'final-native.log').open('w') as log:
                probe=subprocess.run(['python3',str(HERE/'probe_fixed.py'),'final'],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
            (HERE/'final-native-result.json').write_text(json.dumps(dict(exit_code=probe.returncode))+'\n')
            assert probe.returncode==0
print('final source-bound checks passed',flush=True)
