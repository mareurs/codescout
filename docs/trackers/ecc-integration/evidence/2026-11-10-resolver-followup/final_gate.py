"""Freeze the expanded two-file candidate, then verify the entire repository gate."""
import datetime,hashlib,json,os,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-resolver-followup-20261110'
PATHS=['src/tools/symbol/call_edges/resolver.rs','tests/observation_store.rs']
def git(*args):return subprocess.check_output(['git',*args],cwd=WT)
def hashes():return {p:hashlib.sha256((WT/p).read_bytes()).hexdigest() for p in PATHS}
base=git('rev-parse','HEAD').decode().strip()
assert base=='5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3'
assert sorted(git('diff','--name-only').decode().splitlines())==sorted(PATHS)
store='src/observations/store.rs'
assert (WT/store).read_bytes()==git('show',f'HEAD:{store}')
snapshot=dict(base=base,paths=hashes(),production_store_sha256=hashlib.sha256((WT/store).read_bytes()).hexdigest(),initial_comment_only='candidate-source.json',scope_extension='scope-extension-baseline.json',fixture_red='fixture-red.log',fixture_green='fixture-green.log')
(HERE/'final-source.json').write_text(json.dumps(snapshot,indent=2)+'\n')
(HERE/'final-candidate.patch').write_bytes(git('diff','--',*PATHS))
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off')
env.pop('CARGO_TARGET_DIR',None)
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
command=['./scripts/with-slot.sh','--slot','0','./scripts/gate.sh','--check']
with (HERE/'final-gate.log').open('w') as log:
    result=subprocess.run(command,cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
assert snapshot['paths']==hashes()
assert (WT/store).read_bytes()==git('show',f'HEAD:{store}')
record=dict(command=command,workspace=str(WT),head=base,paths=hashes(),production_store_unchanged=True,source_unchanged=True,exit_code=result.returncode,started=started,finished=datetime.datetime.now(datetime.timezone.utc).isoformat())
(HERE/'final-gate-result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
assert result.returncode==0
