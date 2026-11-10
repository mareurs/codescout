"""Gate a retained exact prospective integration tree, including the observed peer Python changes."""
import datetime,hashlib,json,os,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
WT=ROOT/'.worktrees/ecc-bfs-integrated-20261110'
def git(root,*args):return subprocess.check_output(['git',*args],cwd=root).decode().strip()
frozen=json.loads((HERE/'final-source.json').read_text())
base=git(ROOT,'rev-parse','HEAD')
subprocess.run(['git','worktree','add','--detach',str(WT),base],cwd=ROOT,check=True)
subprocess.run(['git','apply',str(HERE/'final-candidate.patch')],cwd=WT,check=True)
subprocess.run(['git','add','--',*frozen['paths']],cwd=WT,check=True)
tree=git(WT,'write-tree')
def hashes():return {p:hashlib.sha256((WT/p).read_bytes()).hexdigest() for p in frozen['paths']}
assert hashes()==frozen['paths']
record=dict(base=base,expected_tree=tree,workspace=str(WT),source=frozen,peer_changed_paths=git(ROOT,'diff','--name-only',frozen['base'],base).splitlines(),started=datetime.datetime.now(datetime.timezone.utc).isoformat())
(HERE/'integrated-tree-before.json').write_text(json.dumps(record,indent=2)+'\n')
env=dict(os.environ,TMPDIR='/home/marius/.cache/codescout-ecc-tmp',CODESCOUT_GATE_TMPFS_BASE='off');env.pop('CARGO_TARGET_DIR',None)
command=['./scripts/with-slot.sh','--slot','2','./scripts/gate.sh','--check']
with (HERE/'integrated-gate.log').open('wb') as log:
    p=subprocess.run(command,cwd=WT,env=env,stdout=log,stderr=subprocess.STDOUT)
record.update(exit_code=p.returncode,finished=datetime.datetime.now(datetime.timezone.utc).isoformat(),command=command,source_unchanged=hashes()==frozen['paths'],tree_after=git(WT,'write-tree'),unstaged_tracked_paths=git(WT,'diff','--name-only').splitlines())
(HERE/'integrated-gate-result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({k:record[k] for k in ['base','expected_tree','exit_code','source_unchanged','tree_after','unstaged_tracked_paths']}),flush=True)
assert p.returncode==0 and record['source_unchanged'] and record['tree_after']==tree and not record['unstaged_tracked_paths']
