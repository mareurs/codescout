"""Retain initial red, then retry the identical frozen candidate without a source change."""
import datetime,hashlib,json,os,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WORKTREE=ROOT/'.worktrees/ecc-resolver-followup-20261110'
candidate=json.loads((HERE/'candidate-source.json').read_text())
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
def check():assert hashlib.sha256((WORKTREE/candidate['path']).read_bytes()).hexdigest()==candidate['sha256']
check()
# The original wrapper asserted on its red gate before recording child exit status.
# Retained seven terminal native responses/calibration are evidence; do not invent that exit code.
summary=json.loads((HERE/'bfs-summary.json').read_text())
assert summary['qualifying_controls_matched']==4 and summary['duplicate_depth2_mismatches']==3 and summary['scorer_controls_as_expected']==5
(HERE/'native-observation-result.json').write_text(json.dumps(dict(process_exit_observed=False,reason='Original wrapper exited at failed gate before native.wait; terminal raw cases, summary and independent review retained.',terminal_native_cases=summary['native_cases'],qualification_data_validated=True),indent=2)+'\n')
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
with (HERE/'gate-retry.log').open('w') as log:
    result=subprocess.run(['./scripts/with-slot.sh','--slot','0','./scripts/gate.sh','--check'],cwd=WORKTREE,env=env,stdout=log,stderr=subprocess.STDOUT)
check()
record=dict(command=['./scripts/with-slot.sh','--slot','0','./scripts/gate.sh','--check'],workspace=str(WORKTREE),head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=WORKTREE,text=True).strip(),source_sha256=candidate['sha256'],source_unchanged=True,exit_code=result.returncode,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),initial_red='gate-result.json',scope='Exact same source; a retry does not establish initial failure root cause.')
(HERE/'gate-retry-result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
assert result.returncode==0
